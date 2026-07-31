//! Transport-neutral API request and response contracts.
//!
//! Secret material appears only in write requests and one-time creation
//! responses. Durable/list responses deliberately expose metadata only.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    Action, Commit, Deployment, Device, Invite, InviteMetadata, Project, PullRequest,
    RepositoryFile, RepositoryRef, Role, Run, RunKind, Schedule, SecretMetadata, Session, User,
    Workspace,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    BadRequest,
    Unauthenticated,
    Forbidden,
    NotFound,
    Conflict,
    RateLimited,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapStatusResponse {
    pub claimed: bool,
    pub server_id: Uuid,
    pub server_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimServerRequest {
    pub recovery_code: String,
    pub display_name: String,
    pub device: RegisterDeviceRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterDeviceRequest {
    pub label: String,
    pub credential_kind: crate::CredentialKind,
    pub credential_id: String,
    pub public_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInviteRequest {
    pub role: Role,
    pub expires_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<Uuid>,
}

/// Returned exactly once. Only a digest of `token` may be persisted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInviteResponse {
    pub invite: InviteMetadata,
    pub token: String,
    pub invite_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedeemInviteRequest {
    pub token: String,
    pub display_name: String,
    pub device: RegisterDeviceRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedeemInviteResponse {
    pub invite: Invite,
    pub session: Session,
    pub device: Device,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResponse {
    pub session: Session,
    pub user: User,
    pub workspaces: Vec<Workspace>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspaceRequest {
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectRequest {
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_branch")]
    pub default_branch: String,
}

fn default_branch() -> String {
    "main".into()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectListResponse {
    pub projects: Vec<Project>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryRefsResponse {
    pub refs: Vec<RepositoryRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryTreeResponse {
    pub revision_sha: String,
    pub path: String,
    pub entries: Vec<RepositoryFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitListResponse {
    pub commits: Vec<Commit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePullRequestRequest {
    pub title: String,
    #[serde(default)]
    pub body: String,
    pub source_branch: String,
    pub target_branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestListResponse {
    pub pull_requests: Vec<PullRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateScheduleRequest {
    pub job: String,
    pub cron: String,
    pub timezone: String,
    pub concurrency: crate::Concurrency,
    #[serde(default = "enabled")]
    pub enabled: bool,
}

fn enabled() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerRunRequest {
    pub job: String,
    pub kind: RunKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_sha: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunListResponse {
    pub runs: Vec<Run>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishRequest {
    pub revision_sha: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishResponse {
    pub deployment: Deployment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PutSecretRequest {
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretListResponse {
    pub secrets: Vec<SecretMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationCheckResponse {
    pub action: Action,
    pub allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleListResponse {
    pub schedules: Vec<Schedule>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_metadata_response_cannot_contain_a_value() {
        let response = SecretListResponse { secrets: vec![] };
        let encoded = toml::to_string(&response).unwrap();
        assert!(!encoded.contains("value"));
    }

    #[test]
    fn project_request_defaults_are_wire_compatible() {
        let request: CreateProjectRequest = toml::from_str(
            r#"
slug = "notes"
name = "Notes"
"#,
        )
        .unwrap();
        assert_eq!(request.default_branch, "main");
        assert!(request.description.is_empty());
    }

    #[test]
    fn page_omits_an_absent_cursor() {
        let page: Page<u8> = Page {
            items: vec![1, 2],
            next_cursor: None,
        };
        let encoded = toml::to_string(&page).unwrap();
        assert!(!encoded.contains("nextCursor"));
    }
}
