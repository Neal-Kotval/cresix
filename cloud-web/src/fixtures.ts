import type { DirectoryEntry, Installation, Session, Workspace } from "./types";

export const previewSession: Session = {
  account: { id: "acct-neal", handle: "neal", displayName: "Neal Kotval" },
  csrfToken: "preview-csrf-not-a-secret",
};

export const previewInstallations: Installation[] = [
  {
    id: "install-aurora",
    localServerId: "3d0a49a7-f65d-41ab-a598-86831b88ca96",
    routeId: "route-7fk2",
    label: "Neal’s laptop",
    state: "connected",
    lastSeenAt: "2026-07-31T16:42:00Z",
  },
  {
    id: "install-studio",
    localServerId: "321735cc-65af-4357-8833-e54388d38e68",
    routeId: "route-m2p8",
    label: "Studio mini",
    state: "offline",
    lastSeenAt: "2026-07-29T20:14:00Z",
  },
  {
    id: "install-retired",
    localServerId: "29c4033d-3032-4d62-a1e6-9b661071c60f",
    routeId: "route-r8v1",
    label: "Retired laptop",
    state: "revoked",
    lastSeenAt: "2026-07-20T09:02:00Z",
    revokedAt: "2026-07-21T13:30:00Z",
  },
];

export const previewWorkspaces: Workspace[] = [
  {
    id: "workspace-paper-street",
    namespace: "paper-street",
    name: "Paper Street tools",
    role: "owner",
    binding: {
      installationId: "install-aurora",
      localWorkspaceId: "5d133e64-3b28-40a1-96df-5f73c9076612",
      catalogRevision: 18,
    },
    projects: [
      {
        id: "project-weeknote",
        slug: "weeknote",
        name: "Weeknote",
        description: "Turns scattered project notes into the Friday update.",
        defaultBranch: "main",
        headSha: "8c3a40d9",
        updatedAt: "2026-07-31T16:39:00Z",
      },
      {
        id: "project-briefcase",
        slug: "briefcase",
        name: "Briefcase",
        description: "A private intake queue for small client requests.",
        defaultBranch: "main",
        headSha: "132be7af",
        updatedAt: "2026-07-30T12:08:00Z",
      },
    ],
  },
  {
    id: "workspace-home-lab",
    namespace: "home-lab",
    name: "Home lab",
    role: "owner",
    binding: {
      installationId: "install-studio",
      localWorkspaceId: "033bc482-79e5-405e-ad3e-da4df8f1f2a1",
      catalogRevision: 4,
    },
    projects: [
      {
        id: "project-garden-watch",
        slug: "garden-watch",
        name: "Garden watch",
        description: "Checks the greenhouse sensors every morning.",
        defaultBranch: "trunk",
        headSha: "d0913ae4",
        updatedAt: "2026-07-29T20:10:00Z",
      },
    ],
  },
];

export function previewDirectory(namespace: string, project: string): DirectoryEntry | undefined {
  const workspace = previewWorkspaces.find((item) => item.namespace === namespace);
  const entry = workspace?.projects.find((item) => item.slug === project);
  const installation = previewInstallations.find(
    (item) => item.id === workspace?.binding?.installationId,
  );
  if (!workspace || !entry || !installation) return undefined;
  return {
    workspace: { id: workspace.id, namespace: workspace.namespace, name: workspace.name },
    project: entry,
    installation: {
      routeId: installation.routeId,
      label: installation.label,
      state: installation.state,
      lastSeenAt: installation.lastSeenAt,
    },
    relayUrl:
      installation.state === "connected"
        ? `https://${installation.routeId}.relay.cresix.com/projects/${entry.slug}`
        : null,
  };
}
