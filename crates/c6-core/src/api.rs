//! Transport-neutral API request and response contracts.
//!
//! Secret material appears only in write requests and one-time creation
//! responses. Durable/list responses deliberately expose metadata only.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    Action, Commit, CredentialScope, CredentialType, Deployment, Device, Invite, InviteMetadata,
    Project, PullRequest, RepositoryFile, RepositoryRef, Role, Run, RunKind, Schedule,
    SecretMetadata, Session, User, Workspace,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
    NotImplemented,
    Internal,
}

/// Canonical JSON error envelope returned by the C6 HTTP API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiErrorResponse {
    pub error: ApiError,
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

// CLI and Git credentials ---------------------------------------------------

/// Optional upper bound on the resources a credential may access.
///
/// The server validates that a project belongs to the supplied workspace when
/// both identifiers are present. Neither identifier grants access by itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CredentialResourceRestriction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<Uuid>,
}

/// Browser-authenticated request to mint one revocable credential.
///
/// Expiry and scope/resource-policy bounds are validated by the server. This
/// request never accepts caller-selected token material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCredentialRequest {
    #[serde(rename = "type")]
    pub credential_type: CredentialType,
    pub label: String,
    /// Requested expiry. When absent, the server applies its documented
    /// bounded default (30 days in Phase 2.1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    pub scopes: Vec<CredentialScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restriction: Option<CredentialResourceRestriction>,
}

/// Read-safe credential metadata. Plaintext and verifier material are
/// deliberately unrepresentable in this type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CredentialMetadata {
    pub id: Uuid,
    pub user_id: Uuid,
    /// The browser device that issued this credential, when the authorizing
    /// session was device-bound. Legacy sessions may not carry this value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<Uuid>,
    #[serde(rename = "type")]
    pub credential_type: CredentialType,
    pub label: String,
    pub scopes: Vec<CredentialScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restriction: Option<CredentialResourceRestriction>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Returned exactly once when a credential is created. Only a verifier of
/// `token` may be persisted by the server; list responses use metadata only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCredentialResponse {
    pub credential: CredentialMetadata,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CredentialListResponse {
    pub credentials: Vec<CredentialMetadata>,
}

/// Immutable installation identity used by the CLI for server pinning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CliServerSummary {
    pub id: Uuid,
    pub name: String,
}

/// Minimal authenticated identity summary. It intentionally contains no
/// session, credential, device-key, or server-administrator material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CliUserSummary {
    pub id: Uuid,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CliWorkspaceSummary {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub role: Role,
}

/// Response from `/api/v1/cli/whoami`, used to verify a CLI token and pin the
/// immutable server identity before storing that token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CliWhoAmIResponse {
    pub server: CliServerSummary,
    pub user: CliUserSummary,
    pub workspaces: Vec<CliWorkspaceSummary>,
}

/// Project fields required by CLI discovery. This is intentionally smaller
/// than the Hub's project model and matches `/api/v1/projects` without making
/// a not-yet-hosted application URL mandatory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CliProjectSummary {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub default_branch: String,
    pub head_sha: String,
    pub published_sha: Option<String>,
    pub role: Role,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CliProjectListResponse {
    pub projects: Vec<CliProjectSummary>,
}

/// Git transport features currently available for one project. Capabilities
/// describe actual server behavior; Milestone 2.1 must report `push: false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteTransportCapabilities {
    pub fetch: bool,
    pub push: bool,
}

/// Canonical, credential-free remote discovered from the authority rather
/// than reconstructed by the CLI from slugs or storage paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRemoteResponse {
    pub project_id: Uuid,
    pub clone_url: String,
    pub capabilities: RemoteTransportCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleListResponse {
    pub schedules: Vec<Schedule>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn instant() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-07-31T12:34:56Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn credential_metadata() -> CredentialMetadata {
        CredentialMetadata {
            id: Uuid::parse_str("10000000-0000-4000-8000-000000000001").unwrap(),
            user_id: Uuid::parse_str("10000000-0000-4000-8000-000000000002").unwrap(),
            device_id: Some(Uuid::parse_str("10000000-0000-4000-8000-000000000003").unwrap()),
            credential_type: CredentialType::Cli,
            label: "Laptop CLI".into(),
            scopes: vec![CredentialScope::ApiRead, CredentialScope::ApiWrite],
            restriction: Some(CredentialResourceRestriction {
                workspace_id: Some(
                    Uuid::parse_str("10000000-0000-4000-8000-000000000004").unwrap(),
                ),
                project_id: None,
            }),
            created_at: instant(),
            expires_at: DateTime::parse_from_rfc3339("2026-08-30T12:34:56Z")
                .unwrap()
                .with_timezone(&Utc),
            last_used_at: None,
            revoked_at: None,
        }
    }

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

    #[test]
    fn create_credential_request_has_stable_json() {
        let request = CreateCredentialRequest {
            credential_type: CredentialType::Git,
            label: "Git on laptop".into(),
            expires_at: Some(instant()),
            scopes: vec![CredentialScope::GitRead],
            restriction: Some(CredentialResourceRestriction {
                workspace_id: None,
                project_id: Some(Uuid::parse_str("20000000-0000-4000-8000-000000000001").unwrap()),
            }),
        };

        assert_eq!(
            serde_json::to_value(request).unwrap(),
            json!({
                "type": "git",
                "label": "Git on laptop",
                "expiresAt": "2026-07-31T12:34:56Z",
                "scopes": ["git:read"],
                "restriction": {
                    "projectId": "20000000-0000-4000-8000-000000000001"
                }
            })
        );
    }

    #[test]
    fn credential_metadata_has_stable_secret_free_json() {
        let wire = serde_json::to_value(credential_metadata()).unwrap();
        assert_eq!(
            wire,
            json!({
                "id": "10000000-0000-4000-8000-000000000001",
                "userId": "10000000-0000-4000-8000-000000000002",
                "deviceId": "10000000-0000-4000-8000-000000000003",
                "type": "cli",
                "label": "Laptop CLI",
                "scopes": ["api:read", "api:write"],
                "restriction": {
                    "workspaceId": "10000000-0000-4000-8000-000000000004"
                },
                "createdAt": "2026-07-31T12:34:56Z",
                "expiresAt": "2026-08-30T12:34:56Z"
            })
        );
        let encoded = wire.to_string();
        assert!(!encoded.contains("token"));
        assert!(!encoded.contains("secret"));
        assert!(!encoded.contains("verifier"));
    }

    #[test]
    fn credential_list_cannot_deserialize_secret_material() {
        let mut metadata = serde_json::to_value(credential_metadata()).unwrap();
        metadata
            .as_object_mut()
            .unwrap()
            .insert("token".into(), json!("c6c_v1_public_secret"));
        let error = serde_json::from_value::<CredentialMetadata>(metadata).unwrap_err();
        assert!(error.to_string().contains("unknown field `token`"));
    }

    #[test]
    fn create_credential_request_rejects_unknown_fields_types_and_scopes() {
        let base = json!({
            "type": "cli",
            "label": "Laptop CLI",
            "expiresAt": "2026-07-31T12:34:56Z",
            "scopes": ["api:read"]
        });

        let mut unknown_field = base.clone();
        unknown_field
            .as_object_mut()
            .unwrap()
            .insert("admin".into(), json!(true));
        assert!(
            serde_json::from_value::<CreateCredentialRequest>(unknown_field)
                .unwrap_err()
                .to_string()
                .contains("unknown field `admin`")
        );

        let mut unknown_type = base.clone();
        unknown_type["type"] = json!("browser");
        assert!(serde_json::from_value::<CreateCredentialRequest>(unknown_type).is_err());

        let mut unknown_scope = base;
        unknown_scope["scopes"] = json!(["admin"]);
        assert!(serde_json::from_value::<CreateCredentialRequest>(unknown_scope).is_err());
    }

    #[test]
    fn create_credential_request_omits_expiry_for_server_default() {
        let request = CreateCredentialRequest {
            credential_type: CredentialType::Cli,
            label: "Headless CLI".into(),
            expires_at: None,
            scopes: vec![CredentialScope::ApiRead],
            restriction: None,
        };
        let wire = serde_json::to_value(&request).unwrap();
        assert_eq!(
            wire,
            json!({
                "type": "cli",
                "label": "Headless CLI",
                "scopes": ["api:read"]
            })
        );
        assert_eq!(
            serde_json::from_value::<CreateCredentialRequest>(wire).unwrap(),
            request
        );
    }

    #[test]
    fn credential_metadata_accepts_a_missing_legacy_device() {
        let mut metadata = credential_metadata();
        metadata.device_id = None;
        let wire = serde_json::to_value(&metadata).unwrap();
        assert!(wire.get("deviceId").is_none());
        assert_eq!(
            serde_json::from_value::<CredentialMetadata>(wire).unwrap(),
            metadata
        );
    }

    #[test]
    fn cli_project_list_matches_the_strict_live_api_shape() {
        let wire = json!({
            "projects": [{
                "id": "50000000-0000-4000-8000-000000000001",
                "workspaceId": "50000000-0000-4000-8000-000000000002",
                "slug": "weeknote",
                "name": "Weeknote",
                "description": "Small weekly notes",
                "defaultBranch": "main",
                "headSha": "0123456789abcdef",
                "publishedSha": null,
                "role": "contributor",
                "updatedAt": "2026-07-31T12:34:56Z"
            }]
        });
        let response: CliProjectListResponse = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(response).unwrap(), wire);

        let mut injected = wire;
        injected["projects"][0]["admin"] = json!(true);
        assert!(serde_json::from_value::<CliProjectListResponse>(injected).is_err());
    }

    #[test]
    fn api_error_envelope_is_strict_and_stable() {
        let response = ApiErrorResponse {
            error: ApiError {
                code: ErrorCode::Forbidden,
                message: "access denied".into(),
                request_id: Some("request-1".into()),
            },
        };
        assert_eq!(
            serde_json::to_value(response).unwrap(),
            json!({
                "error": {
                    "code": "forbidden",
                    "message": "access denied",
                    "requestId": "request-1"
                }
            })
        );
        assert!(
            serde_json::from_value::<ApiErrorResponse>(json!({
                "error": {"code": "forbidden", "message": "no", "debug": "secret"}
            }))
            .is_err()
        );
    }

    #[test]
    fn whoami_has_stable_json_and_rejects_privilege_injection() {
        let response = CliWhoAmIResponse {
            server: CliServerSummary {
                id: Uuid::parse_str("30000000-0000-4000-8000-000000000001").unwrap(),
                name: "Acme C6".into(),
            },
            user: CliUserSummary {
                id: Uuid::parse_str("30000000-0000-4000-8000-000000000002").unwrap(),
                display_name: "Neal".into(),
            },
            workspaces: vec![CliWorkspaceSummary {
                id: Uuid::parse_str("30000000-0000-4000-8000-000000000003").unwrap(),
                slug: "paper-street".into(),
                name: "Paper Street".into(),
                role: Role::Reader,
            }],
        };
        let wire = serde_json::to_value(response).unwrap();
        assert_eq!(
            wire,
            json!({
                "server": {
                    "id": "30000000-0000-4000-8000-000000000001",
                    "name": "Acme C6"
                },
                "user": {
                    "id": "30000000-0000-4000-8000-000000000002",
                    "displayName": "Neal"
                },
                "workspaces": [{
                    "id": "30000000-0000-4000-8000-000000000003",
                    "slug": "paper-street",
                    "name": "Paper Street",
                    "role": "reader"
                }]
            })
        );

        let mut injected = wire;
        injected["user"]["serverAdministrator"] = json!(true);
        assert!(serde_json::from_value::<CliWhoAmIResponse>(injected).is_err());
    }

    #[test]
    fn project_remote_has_stable_credential_free_json() {
        let response = ProjectRemoteResponse {
            project_id: Uuid::parse_str("40000000-0000-4000-8000-000000000001").unwrap(),
            clone_url: "https://c6.example/git/paper-street/weeknote.git".into(),
            capabilities: RemoteTransportCapabilities {
                fetch: true,
                push: false,
            },
        };
        assert_eq!(
            serde_json::to_value(response).unwrap(),
            json!({
                "projectId": "40000000-0000-4000-8000-000000000001",
                "cloneUrl": "https://c6.example/git/paper-street/weeknote.git",
                "capabilities": {"fetch": true, "push": false}
            })
        );
    }

    #[test]
    fn remote_capabilities_reject_unknown_or_missing_flags() {
        assert!(
            serde_json::from_value::<RemoteTransportCapabilities>(
                json!({"fetch": true, "push": false, "admin": true})
            )
            .is_err()
        );
        assert!(
            serde_json::from_value::<RemoteTransportCapabilities>(json!({"fetch": true})).is_err()
        );
    }
}
