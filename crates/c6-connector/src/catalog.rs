use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use c6_cloud_core::{
    CatalogAcceptedResponse, CatalogProjectInput, CloudApiError, ProjectSlug, PutCatalogRequest,
};
use c6_core::{CliProjectListResponse, CliProjectSummary};
use reqwest::{Client, StatusCode, redirect::Policy};
use thiserror::Error;
use tokio::time::{Instant, MissedTickBehavior, interval_at};
use tracing::{info, warn};

use crate::LoadedConfig;

const MAX_LOCAL_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
static LAST_REVISION: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CatalogError {
    #[error("local C6 rejected its connector credential")]
    LocalAuthenticationRejected,
    #[error("Cresix Cloud rejected its connector credential")]
    CloudAuthenticationRejected,
    #[error("local project catalog is not valid for Cloud publication")]
    InvalidLocalCatalog,
    #[error("a catalog response violated the Cloud protocol")]
    Protocol,
    #[error("catalog publication failed")]
    Transport,
    #[error("Cloud rejected the catalog update")]
    Rejected,
}

impl CatalogError {
    pub fn is_authentication_rejection(&self) -> bool {
        matches!(
            self,
            Self::LocalAuthenticationRejected | Self::CloudAuthenticationRejected
        )
    }
}

/// Publishes snapshots serially at the configured interval. A publication is
/// always completed before another begins, and missed ticks are skipped rather
/// than accumulated. Authentication failures require operator action and end
/// the loop; bounded transient/protocol failures are retried.
pub async fn run_periodic(config: &LoadedConfig) -> Result<(), CatalogError> {
    let period = std::time::Duration::from_secs(config.config.catalog_interval_seconds);
    let mut ticker = interval_at(Instant::now() + period, period);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        match publish_snapshot(config).await {
            Ok(accepted) => info!(
                projects = accepted.accepted_projects,
                "refreshed local project catalog"
            ),
            Err(error) if error.is_authentication_rejection() => return Err(error),
            Err(error) => {
                // CatalogError deliberately contains no response bodies, URLs,
                // request paths, tokens, or lower-level client error chains.
                warn!(error = %error, "catalog refresh failed; retrying later");
            }
        }
    }
}

pub async fn publish_snapshot(
    config: &LoadedConfig,
) -> Result<CatalogAcceptedResponse, CatalogError> {
    let client = Client::builder()
        .redirect(Policy::none())
        .timeout(config.request_timeout())
        .build()
        .map_err(|_| CatalogError::Transport)?;
    let local_url = config
        .local_origin
        .join("/api/v1/projects")
        .map_err(|_| CatalogError::Protocol)?;
    let response = client
        .get(local_url)
        .bearer_auth(config.credentials.local())
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|_| CatalogError::Transport)?;
    if matches!(
        response.status(),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
    ) {
        return Err(CatalogError::LocalAuthenticationRejected);
    }
    if !response.status().is_success() {
        return Err(CatalogError::Rejected);
    }
    let projects: CliProjectListResponse = decode_bounded(response).await?;
    let request = snapshot_request(config, projects.projects)?;

    let path = format!(
        "/api/v1/installations/{}/catalog",
        config.config.installation_id
    );
    let cloud_url = config
        .cloud_origin
        .join(&path)
        .map_err(|_| CatalogError::Protocol)?;
    let response = client
        .put(cloud_url)
        .bearer_auth(config.credentials.cloud().expose_secret())
        .header("Accept", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|_| CatalogError::Transport)?;
    if matches!(
        response.status(),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
    ) {
        return Err(CatalogError::CloudAuthenticationRejected);
    }
    if !response.status().is_success() {
        // Decode only to validate the bounded public error shape; do not copy
        // arbitrary server text (or request URLs) into connector logs.
        let _: CloudApiError = decode_bounded(response)
            .await
            .map_err(|_| CatalogError::Rejected)?;
        return Err(CatalogError::Rejected);
    }
    let accepted: CatalogAcceptedResponse = decode_bounded(response).await?;
    if accepted.binding_id != config.config.binding_id || accepted.revision != request.revision {
        return Err(CatalogError::Protocol);
    }
    Ok(accepted)
}

fn snapshot_request(
    config: &LoadedConfig,
    projects: Vec<CliProjectSummary>,
) -> Result<PutCatalogRequest, CatalogError> {
    let projects = projects
        .into_iter()
        .filter(|project| project.workspace_id == config.config.local_workspace_id)
        .map(project_input)
        .collect::<Result<Vec<_>, _>>()?;
    let request = PutCatalogRequest {
        binding_id: config.config.binding_id,
        revision: next_revision(),
        projects,
    };
    request
        .validate()
        .map_err(|_| CatalogError::InvalidLocalCatalog)?;
    Ok(request)
}

fn project_input(project: CliProjectSummary) -> Result<CatalogProjectInput, CatalogError> {
    let project = CatalogProjectInput {
        local_project_id: project.id,
        slug: ProjectSlug::new(project.slug).map_err(|_| CatalogError::InvalidLocalCatalog)?,
        name: project.name,
        description: project.description,
        default_branch: project.default_branch,
        head_sha: project.head_sha,
        updated_at: project.updated_at,
    };
    project
        .validate()
        .map_err(|_| CatalogError::InvalidLocalCatalog)?;
    Ok(project)
}

fn next_revision() -> u64 {
    let clock = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64;
    LAST_REVISION
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |last| {
            Some(clock.max(last.saturating_add(1)).max(1))
        })
        .unwrap_or_else(|last| last.saturating_add(1))
}

async fn decode_bounded<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, CatalogError> {
    if response
        .content_length()
        .is_some_and(|len| len > MAX_LOCAL_RESPONSE_BYTES as u64)
    {
        return Err(CatalogError::Protocol);
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|_| CatalogError::Transport)?;
    if bytes.len() > MAX_LOCAL_RESPONSE_BYTES {
        return Err(CatalogError::Protocol);
    }
    serde_json::from_slice(&bytes).map_err(|_| CatalogError::Protocol)
}

#[cfg(test)]
mod tests {
    use super::*;
    use c6_core::Role;
    use chrono::Utc;
    use uuid::Uuid;

    fn project(workspace_id: Uuid, slug: &str) -> CliProjectSummary {
        CliProjectSummary {
            id: Uuid::new_v4(),
            workspace_id,
            slug: slug.into(),
            name: "Small app".into(),
            description: "A bounded projection".into(),
            default_branch: "main".into(),
            head_sha: "abcdef".into(),
            published_sha: None,
            role: Role::Owner,
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn revision_is_strictly_monotonic_in_process() {
        let one = next_revision();
        let two = next_revision();
        assert!(two > one);
    }

    #[test]
    fn only_authentication_rejections_are_terminal_for_periodic_publication() {
        assert!(CatalogError::LocalAuthenticationRejected.is_authentication_rejection());
        assert!(CatalogError::CloudAuthenticationRejected.is_authentication_rejection());
        for retryable in [
            CatalogError::InvalidLocalCatalog,
            CatalogError::Protocol,
            CatalogError::Transport,
            CatalogError::Rejected,
        ] {
            assert!(!retryable.is_authentication_rejection());
        }
    }

    #[test]
    fn project_conversion_validates_cloud_boundaries() {
        let workspace = Uuid::new_v4();
        let converted = project_input(project(workspace, "weeknote")).unwrap();
        assert_eq!(converted.slug.as_str(), "weeknote");
        assert_eq!(converted.head_sha, "abcdef");
        assert_eq!(
            project_input(project(workspace, "../escape")),
            Err(CatalogError::InvalidLocalCatalog)
        );
    }
}
