export type Role = "consumer" | "reader" | "runner" | "contributor" | "maintainer" | "owner";
export type RunStatus = "recorded" | "queued" | "running" | "succeeded" | "failed" | "interrupted" | "cancelled";

export interface User { id: string; handle: string; displayName: string; }
export interface Workspace { id: string; slug: string; name: string; role: Role; }
export interface Session { user: User; workspaces: Workspace[]; serverAdministrator: boolean; csrfToken?: string; }
export interface ProjectSummary { id: string; workspaceId: string; slug: string; name: string; description: string; defaultBranch: string; headSha: string; publishedSha?: string; role: Role; updatedAt: string; appUrl?: string; }
export interface Deployment { id: string; revisionSha: string; environment: "preview" | "production"; status: "recorded" | "queued" | "building" | "ready" | "failed" | "superseded"; url?: string; createdAt: string; }
export interface Revision { sha: string; message: string; author: User; createdAt: string; }
export interface PullRequest { number: number; title: string; sourceBranch: string; targetBranch: string; author: User; status: "open" | "merged" | "closed"; preview?: Deployment; updatedAt: string; }
export interface Run { id: string; job: string; kind: "command" | "cron" | "agent"; revisionSha: string; status: RunStatus; trigger: string; startedAt: string; finishedAt?: string; }
export interface ProjectDetail extends ProjectSummary { readme: string; revisions: Revision[]; pullRequests: PullRequest[]; deployments: Deployment[]; runs: Run[]; }

export interface Peer { id: string; name: string; handle: string; role: Role; status: "active" | "pending" | "revoked"; joinedAt?: string; lastSeenAt?: string; devices: Device[]; }
export interface Device { id: string; name: string; kind: "browser" | "device"; fingerprint: string; addedAt: string; lastUsedAt?: string; }
export interface Invite { id: string; code: string; role: Role; expiresAt: string; status: "ready" | "pending" | "used"; requestedBy?: string; }
export interface Job { id: string; name: string; kind: Run["kind"]; command: string; schedule?: string; timezone?: string; enabled: boolean; lastStatus?: RunStatus; nextRunAt?: string; }
export interface SecretMetadata { name: string; scope: "project" | "workspace"; grants: string[]; updatedAt: string; updatedBy: string; version: number; }
export interface Branch { name: string; sha: string; updatedAt: string; protected: boolean; ahead?: number; behind?: number; }

export type CredentialType = "cli" | "git";
export type CredentialScope = "api:read" | "api:write" | "git:read" | "git:write";
export interface CredentialMetadata {
  id: string;
  deviceId: string;
  label: string;
  credentialType: CredentialType;
  scopes: CredentialScope[];
  restriction?: { workspaceId?: string; projectId?: string };
  createdAt: string;
  expiresAt: string;
  lastUsedAt?: string;
  revokedAt?: string;
}
export interface ProjectRemote {
  projectId: string;
  cloneUrl: string;
  capabilities: { fetch: boolean; push: boolean };
}
