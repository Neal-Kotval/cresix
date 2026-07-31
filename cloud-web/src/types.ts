export type InstallationState = "connected" | "offline" | "revoked";

export interface Account {
  id: string;
  handle: string;
  displayName: string;
}

export interface Session {
  account: Account;
  csrfToken: string;
}

export interface Workspace {
  id: string;
  namespace: string;
  name: string;
  role: "owner" | "maintainer" | "member";
  binding?: WorkspaceBinding;
  projects: Project[];
}

export interface WorkspaceBinding {
  installationId: string;
  localWorkspaceId: string;
  catalogRevision: number;
}

export interface Installation {
  id: string;
  localServerId: string;
  routeId: string;
  label: string;
  state: InstallationState;
  lastSeenAt?: string;
  revokedAt?: string;
}

export interface InstallationCreated extends Installation {
  connectorCredential: string;
}

export interface Project {
  id: string;
  slug: string;
  name: string;
  description: string;
  defaultBranch: string;
  headSha: string;
  updatedAt: string;
}

export interface DirectoryEntry {
  workspace: Pick<Workspace, "id" | "namespace" | "name">;
  project: Project;
  installation: Pick<Installation, "routeId" | "label" | "state" | "lastSeenAt">;
  relayUrl: string | null;
}

export interface CloudStatus {
  claimed: boolean;
  dogfood: boolean;
}
