use crate::{AccountHandle, InstallationLabel, ProjectSlug, WorkspaceNamespace};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountSummary {
    pub id: Uuid,
    pub handle: AccountHandle,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudWorkspaceRole {
    Owner,
    Maintainer,
    Member,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloudWorkspaceSummary {
    pub id: Uuid,
    pub namespace: WorkspaceNamespace,
    pub name: String,
    pub owner_account_id: Uuid,
    pub role: CloudWorkspaceRole,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallationConnectionState {
    Connected,
    Disconnected,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstallationSummary {
    pub id: Uuid,
    pub local_server_id: Uuid,
    /// Opaque routing identity. It is neither a namespace nor permission.
    pub route_id: String,
    pub owner_account_id: Uuid,
    pub label: InstallationLabel,
    pub credential_public_id: String,
    pub connection_state: InstallationConnectionState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connected_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceBindingSummary {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub installation_id: Uuid,
    pub local_workspace_id: Uuid,
    pub catalog_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogProject {
    pub binding_id: Uuid,
    pub local_project_id: Uuid,
    pub slug: ProjectSlug,
    pub name: String,
    pub description: String,
    pub default_branch: String,
    pub head_sha: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloudMembership {
    pub workspace_id: Uuid,
    pub account_id: Uuid,
    pub role: CloudWorkspaceRole,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloudAuditEvent {
    pub id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<Uuid>,
    pub action: String,
    pub target_type: String,
    pub target_id: Uuid,
    /// Bounded, redacted JSON text. Never contains session or connector proofs.
    pub details: String,
    pub created_at: DateTime<Utc>,
}
