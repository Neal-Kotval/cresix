use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Action, Role, manifest::Concurrency};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: Uuid,
    pub handle: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub default_branch: String,
    pub head_sha: String,
    pub published_sha: Option<String>,
    pub role: Role,
    pub app_url: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Revision {
    pub sha: String,
    pub message: String,
    pub author: User,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub source_branch: String,
    pub target_branch: String,
    pub author: User,
    pub status: PullRequestStatus,
    pub preview: Option<Deployment>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullRequestStatus {
    Open,
    Merged,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Deployment {
    pub id: Uuid,
    pub revision_sha: String,
    pub environment: Environment,
    pub status: DeploymentStatus,
    pub url: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    Preview,
    Production,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentStatus {
    Queued,
    Building,
    Ready,
    Failed,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: Uuid,
    pub job: String,
    pub kind: RunKind,
    pub revision_sha: String,
    pub status: RunStatus,
    pub trigger: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunKind {
    Command,
    Cron,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Interrupted,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    #[serde(flatten)]
    pub project: Project,
    pub readme: String,
    pub revisions: Vec<Revision>,
    pub pull_requests: Vec<PullRequest>,
    pub deployments: Vec<Deployment>,
    pub runs: Vec<Run>,
}

// Identity and trust ---------------------------------------------------------

/// A human principal local to one C6 installation. `label` is display-only;
/// authorization is always bound to `id` and a verified device credential.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Peer {
    pub id: Uuid,
    pub label: String,
    pub status: PeerStatus,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerStatus {
    Active,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: Uuid,
    pub peer_id: Uuid,
    pub label: String,
    pub credential_kind: CredentialKind,
    /// Opaque, non-secret identifier supplied by the credential protocol.
    pub credential_id: String,
    pub status: DeviceStatus,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialKind {
    Passkey,
    SshEd25519,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceStatus {
    Active,
    Revoked,
}

/// Safe representation for lists and audit views. Invite tokens are never a
/// field on this type and therefore cannot leak through ordinary reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteMetadata {
    pub id: Uuid,
    pub role: Role,
    pub workspace_id: Option<Uuid>,
    pub status: InviteStatus,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Invite {
    #[serde(flatten)]
    pub metadata: InviteMetadata,
    pub peer_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InviteStatus {
    Open,
    PendingApproval,
    Consumed,
    Revoked,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: Uuid,
    pub peer_id: Uuid,
    pub device_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Membership {
    pub workspace_id: Uuid,
    pub peer_id: Uuid,
    pub role: Role,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Repository and collaboration ---------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryRef {
    pub name: String,
    pub kind: RepositoryRefKind,
    pub sha: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryRefKind {
    Branch,
    Tag,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryFile {
    pub path: String,
    pub name: String,
    pub kind: RepositoryEntryKind,
    pub sha: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryEntryKind {
    File,
    Directory,
    Symlink,
    Submodule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileContent {
    pub path: String,
    pub sha: String,
    pub size_bytes: u64,
    pub encoding: ContentEncoding,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentEncoding {
    Utf8,
    Base64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub sha: String,
    pub message: String,
    pub author: CommitIdentity,
    pub committer: CommitIdentity,
    pub parent_shas: Vec<String>,
    pub authored_at: DateTime<Utc>,
    pub committed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitIdentity {
    pub name: String,
    pub email: String,
    pub peer_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestDetail {
    #[serde(flatten)]
    pub pull_request: PullRequest,
    pub body: String,
    pub base_sha: String,
    pub head_sha: String,
    pub merge_sha: Option<String>,
    pub reviews: Vec<PullRequestReview>,
    pub created_at: DateTime<Utc>,
    pub merged_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestReview {
    pub id: Uuid,
    pub author: User,
    pub state: ReviewState,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    Commented,
    Approved,
    ChangesRequested,
}

// Runtime, deployment, and secrets -----------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    pub id: Uuid,
    pub project_id: Uuid,
    pub job: String,
    pub cron: String,
    pub timezone: String,
    pub concurrency: Concurrency,
    pub enabled: bool,
    pub next_run_at: Option<DateTime<Utc>>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunDetail {
    #[serde(flatten)]
    pub run: Run,
    pub project_id: Uuid,
    pub schedule_id: Option<Uuid>,
    pub image_digest: Option<String>,
    pub manifest_digest: String,
    pub triggered_by: RunTrigger,
    pub exit_code: Option<i32>,
    pub log_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum RunTrigger {
    Manual { peer_id: Uuid },
    Schedule { occurrence_id: String },
    Deployment { deployment_id: Uuid },
    Agent { parent_run_id: Uuid },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentDetail {
    #[serde(flatten)]
    pub deployment: Deployment,
    pub project_id: Uuid,
    pub image_digest: Option<String>,
    pub manifest_digest: String,
    pub initiated_by: Uuid,
    pub finished_at: Option<DateTime<Utc>>,
}

/// Read-safe secret information. Secret plaintext is intentionally absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretMetadata {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub version: u64,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    pub id: Uuid,
    pub actor_peer_id: Option<Uuid>,
    pub action: Action,
    pub resource_kind: String,
    pub resource_id: String,
    pub occurred_at: DateTime<Utc>,
    pub source_ip: Option<String>,
}

#[cfg(test)]
mod contract_tests {
    use super::*;

    fn instant() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-01-02T03:04:05Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn invite_metadata_has_no_credential_material() {
        let invite = InviteMetadata {
            id: Uuid::nil(),
            role: Role::Reader,
            workspace_id: None,
            status: InviteStatus::Open,
            created_by: Uuid::nil(),
            created_at: instant(),
            expires_at: instant(),
            consumed_at: None,
        };
        let wire = toml::to_string(&invite).unwrap();
        assert!(!wire.contains("token"));
        assert!(!wire.contains("secret"));
        assert!(wire.contains("status = \"open\""));
    }

    #[test]
    fn run_trigger_is_explicitly_tagged() {
        let trigger = RunTrigger::Schedule {
            occurrence_id: "daily@2026-01-02T03:04:05Z".into(),
        };
        let wire = toml::to_string(&trigger).unwrap();
        assert!(wire.contains("kind = \"schedule\""));
        assert!(wire.contains("occurrenceId"));
        assert_eq!(toml::from_str::<RunTrigger>(&wire).unwrap(), trigger);
    }

    #[test]
    fn repository_entry_kinds_have_stable_wire_names() {
        #[derive(Serialize)]
        struct Wire {
            kind: RepositoryEntryKind,
        }
        let wire = toml::to_string(&Wire {
            kind: RepositoryEntryKind::Submodule,
        })
        .unwrap();
        assert_eq!(wire.trim(), "kind = \"submodule\"");
    }

    #[test]
    fn revoked_device_round_trips_without_public_key() {
        let device = Device {
            id: Uuid::nil(),
            peer_id: Uuid::nil(),
            label: "Laptop".into(),
            credential_kind: CredentialKind::Passkey,
            credential_id: "credential-id".into(),
            status: DeviceStatus::Revoked,
            created_at: instant(),
            last_used_at: None,
            revoked_at: Some(instant()),
        };
        let wire = toml::to_string(&device).unwrap();
        assert!(!wire.contains("publicKey"));
        assert_eq!(toml::from_str::<Device>(&wire).unwrap(), device);
    }
}
