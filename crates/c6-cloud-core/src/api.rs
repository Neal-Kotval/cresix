use crate::{
    AccountHandle, AccountSummary, CatalogProject, CloudWorkspaceSummary, InstallationLabel,
    InstallationSummary, ProjectSlug, SecretToken, TokenClass, WorkspaceBindingSummary,
    WorkspaceNamespace,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const MAX_DISPLAY_NAME_LEN: usize = 100;
pub const MAX_WORKSPACE_NAME_LEN: usize = 100;
pub const MAX_PROJECT_NAME_LEN: usize = 120;
pub const MAX_PROJECT_DESCRIPTION_LEN: usize = 2_000;
pub const MAX_BRANCH_LEN: usize = 255;
pub const MAX_HEAD_SHA_LEN: usize = 128;
pub const MAX_CATALOG_PROJECTS: usize = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudErrorCode {
    BadRequest,
    Unauthenticated,
    Forbidden,
    NotFound,
    Conflict,
    PayloadTooLarge,
    RateLimited,
    Unavailable,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloudApiError {
    pub code: CloudErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ContractValidationError {
    #[error("{field} must be between {min} and {max} bytes")]
    Length {
        field: &'static str,
        min: usize,
        max: usize,
    },
    #[error("{field} must not contain control characters")]
    ControlCharacters { field: &'static str },
    #[error("catalog contains too many projects")]
    CatalogTooLarge,
    #[error("catalog revision must be non-zero")]
    ZeroCatalogRevision,
    #[error("credential belongs to the wrong authentication surface")]
    WrongTokenClass,
}

fn bounded_text(
    value: &str,
    field: &'static str,
    min: usize,
    max: usize,
) -> Result<(), ContractValidationError> {
    if !(min..=max).contains(&value.len()) {
        return Err(ContractValidationError::Length { field, min, max });
    }
    if value.chars().any(char::is_control) {
        return Err(ContractValidationError::ControlCharacters { field });
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloudStatusResponse {
    pub claimed: bool,
    pub service_name: String,
    pub relay_authority: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BootstrapClaimRequest {
    pub bootstrap_token: SecretToken,
    pub handle: AccountHandle,
    pub display_name: String,
}

impl BootstrapClaimRequest {
    pub fn validate(&self) -> Result<(), ContractValidationError> {
        if self.bootstrap_token.parsed().class != TokenClass::Bootstrap {
            return Err(ContractValidationError::WrongTokenClass);
        }
        bounded_text(&self.display_name, "displayName", 1, MAX_DISPLAY_NAME_LEN)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BootstrapClaimResponse {
    pub account: AccountSummary,
    /// Bound to the newly issued host-only session cookie.
    pub csrf_token: SecretToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloudSessionResponse {
    pub account: AccountSummary,
    pub csrf_token: SecretToken,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCloudWorkspaceRequest {
    pub namespace: WorkspaceNamespace,
    pub name: String,
}

impl CreateCloudWorkspaceRequest {
    pub fn validate(&self) -> Result<(), ContractValidationError> {
        bounded_text(&self.name, "name", 1, MAX_WORKSPACE_NAME_LEN)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloudWorkspaceListResponse {
    pub workspaces: Vec<CloudWorkspaceDirectorySummary>,
}

/// Authenticated Cloud directory projection for one workspace. Local C6
/// remains authoritative for the represented project and its permissions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloudWorkspaceDirectorySummary {
    #[serde(flatten)]
    pub workspace: CloudWorkspaceSummary,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<WorkspaceBindingSummary>,
    pub projects: Vec<CatalogProject>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegisterInstallationRequest {
    pub local_server_id: Uuid,
    pub label: InstallationLabel,
}

/// Returned once. Durable/list contracts cannot represent connector plaintext.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegisterInstallationResponse {
    pub installation: InstallationSummary,
    pub connector_token: SecretToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstallationListResponse {
    pub installations: Vec<InstallationSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateWorkspaceBindingRequest {
    pub installation_id: Uuid,
    pub local_workspace_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogProjectInput {
    pub local_project_id: Uuid,
    pub slug: ProjectSlug,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub default_branch: String,
    pub head_sha: String,
    pub updated_at: DateTime<Utc>,
}

impl CatalogProjectInput {
    pub fn validate(&self) -> Result<(), ContractValidationError> {
        bounded_text(&self.name, "projects[].name", 1, MAX_PROJECT_NAME_LEN)?;
        bounded_text(
            &self.description,
            "projects[].description",
            0,
            MAX_PROJECT_DESCRIPTION_LEN,
        )?;
        bounded_text(
            &self.default_branch,
            "projects[].defaultBranch",
            1,
            MAX_BRANCH_LEN,
        )?;
        bounded_text(&self.head_sha, "projects[].headSha", 1, MAX_HEAD_SHA_LEN)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PutCatalogRequest {
    pub binding_id: Uuid,
    pub revision: u64,
    pub projects: Vec<CatalogProjectInput>,
}

impl PutCatalogRequest {
    pub fn validate(&self) -> Result<(), ContractValidationError> {
        if self.revision == 0 {
            return Err(ContractValidationError::ZeroCatalogRevision);
        }
        if self.projects.len() > MAX_CATALOG_PROJECTS {
            return Err(ContractValidationError::CatalogTooLarge);
        }
        self.projects
            .iter()
            .try_for_each(CatalogProjectInput::validate)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogAcceptedResponse {
    pub binding_id: Uuid,
    pub revision: u64,
    pub accepted_projects: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevokeInstallationResponse {
    pub installation: InstallationSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DirectoryProjectResponse {
    pub workspace: CloudWorkspaceSummary,
    pub project: CatalogProject,
    pub installation: InstallationSummary,
    /// Absolute relay target. The dogfood service returns a same-origin,
    /// cookie-stripping transport path; production requires an isolated origin.
    pub relay_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelayHeartbeatResponse {
    pub installation_id: Uuid,
    pub generation: u64,
    pub observed_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_requests_reject_unknown_fields_and_invalid_identifiers() {
        let json = r#"{"namespace":"valid-name","name":"Good","owner":true}"#;
        assert!(serde_json::from_str::<CreateCloudWorkspaceRequest>(json).is_err());
        let json = r#"{"namespace":"../etc","name":"Bad"}"#;
        assert!(serde_json::from_str::<CreateCloudWorkspaceRequest>(json).is_err());
    }

    #[test]
    fn request_validation_bounds_user_controlled_text() {
        let request = CreateCloudWorkspaceRequest {
            namespace: WorkspaceNamespace::new("valid-name").unwrap(),
            name: "x".repeat(MAX_WORKSPACE_NAME_LEN + 1),
        };
        assert!(request.validate().is_err());

        let project = CatalogProjectInput {
            local_project_id: Uuid::nil(),
            slug: ProjectSlug::new("demo").unwrap(),
            name: "Demo".into(),
            description: "log\nforge".into(),
            default_branch: "main".into(),
            head_sha: "abc".into(),
            updated_at: Utc::now(),
        };
        assert!(project.validate().is_err());
    }

    #[test]
    fn bootstrap_rejects_a_connector_credential() {
        let request = BootstrapClaimRequest {
            bootstrap_token: SecretToken::parse(
                "c6x_v1_AAAAAAAAAAAAAAAA_BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
            )
            .unwrap(),
            handle: AccountHandle::new("owner").unwrap(),
            display_name: "Owner".into(),
        };
        assert_eq!(
            request.validate(),
            Err(ContractValidationError::WrongTokenClass)
        );
    }

    #[test]
    fn catalog_rejects_zero_revision_and_excess_entries() {
        let mut request = PutCatalogRequest {
            binding_id: Uuid::nil(),
            revision: 0,
            projects: vec![],
        };
        assert_eq!(
            request.validate(),
            Err(ContractValidationError::ZeroCatalogRevision)
        );
        request.revision = 1;
        request.projects = (0..=MAX_CATALOG_PROJECTS)
            .map(|_| CatalogProjectInput {
                local_project_id: Uuid::nil(),
                slug: ProjectSlug::new("demo").unwrap(),
                name: "Demo".into(),
                description: String::new(),
                default_branch: "main".into(),
                head_sha: "abc".into(),
                updated_at: Utc::now(),
            })
            .collect();
        assert_eq!(
            request.validate(),
            Err(ContractValidationError::CatalogTooLarge)
        );
    }
}
