import type { Branch, Invite, Job, Peer, ProjectDetail, ProjectSummary, SecretMetadata, Session } from "./types";

const now = Date.now();
const ago = (hours: number) => new Date(now - hours * 3_600_000).toISOString();
const ahead = (hours: number) => new Date(now + hours * 3_600_000).toISOString();
export const fixtureSession: Session = { user: { id: "u-neal", handle: "neal", displayName: "Neal Kotval" }, workspaces: [{ id: "w-paper", slug: "paper-street", name: "Paper Street", role: "owner" }] };
export const fixtureProject: ProjectDetail = {
  id: "p-weeknote", workspaceId: "w-paper", slug: "weeknote", name: "Weeknote", description: "A tiny shared app for making the weekly update less painful.", defaultBranch: "main", headSha: "7c1a840d9f3", publishedSha: "2fa39bd830e", role: "owner", appUrl: "https://weeknote.c6.local", updatedAt: ago(1), readme: "# Weeknote",
  revisions: [
    { sha: "7c1a840d9f3", message: "Make summaries easier to scan", author: fixtureSession.user, createdAt: ago(1) },
    { sha: "09cf33ad771", message: "Tune Friday agent prompt", author: { id: "u-amy", handle: "amy", displayName: "Amy Chen" }, createdAt: ago(7) },
    { sha: "2fa39bd830e", message: "Add activity source filters", author: fixtureSession.user, createdAt: ago(25) },
    { sha: "981eedb77c0", message: "Create Weeknote", author: fixtureSession.user, createdAt: ago(72) },
  ],
  pullRequests: [{ number: 12, title: "Draft summaries from the week’s activity", sourceBranch: "agent/friday-notes/42", targetBranch: "main", author: { id: "svc-agent", handle: "friday-notes", displayName: "Friday Notes" }, status: "open", updatedAt: ago(2), preview: { id: "d-pr12", revisionSha: "ae031ad", environment: "preview", status: "ready", url: "https://pr-12.weeknote.c6.local", createdAt: ago(2) } }],
  deployments: [
    { id: "d-prod-18", revisionSha: "2fa39bd830e", environment: "production", status: "ready", url: "https://weeknote.c6.local", createdAt: ago(25) },
    { id: "d-preview-19", revisionSha: "7c1a840d9f3", environment: "preview", status: "ready", url: "https://preview.weeknote.c6.local", createdAt: ago(1) },
    { id: "d-prod-17", revisionSha: "981eedb77c0", environment: "production", status: "superseded", createdAt: ago(73) },
  ],
  runs: [
    { id: "run-46", job: "sync-activity", kind: "cron", revisionSha: "2fa39bd830e", status: "succeeded", trigger: "schedule · hourly", startedAt: ago(1), finishedAt: ago(.96) },
    { id: "run-45", job: "friday-notes", kind: "agent", revisionSha: "2fa39bd830e", status: "succeeded", trigger: "schedule · Neal", startedAt: ago(20), finishedAt: ago(19.8) },
    { id: "run-44", job: "sync-activity", kind: "cron", revisionSha: "2fa39bd830e", status: "failed", trigger: "schedule · hourly", startedAt: ago(25), finishedAt: ago(24.9) },
  ],
};
export const fixtureProjects: ProjectSummary[] = [fixtureProject, { ...fixtureProject, id: "p-receipts", slug: "receipt-box", name: "Receipt Box", description: "Collect, tag, and export team receipts.", headSha: "a812c90", publishedSha: "a812c90", updatedAt: ago(8), appUrl: "https://receipts.c6.local" }, { ...fixtureProject, id: "p-proto", slug: "prototype-room", name: "Prototype Room", description: "Share product prototypes without deployment chores.", headSha: "381a1f9", publishedSha: undefined, updatedAt: ago(50), appUrl: undefined }];
export const fixturePeers: Peer[] = [
  { id: "peer-neal", name: "Neal Kotval", handle: "neal", role: "owner", status: "active", joinedAt: ago(720), lastSeenAt: ago(0), devices: [{ id: "dev-mac", name: "Neal’s browser", kind: "browser", fingerprint: "Opaque session record", addedAt: ago(720), lastUsedAt: ago(0) }, { id: "dev-other", name: "Secondary browser", kind: "device", fingerprint: "Opaque device record", addedAt: ago(700), lastUsedAt: ago(2) }] },
  { id: "peer-amy", name: "Amy Chen", handle: "amy", role: "maintainer", status: "active", joinedAt: ago(300), lastSeenAt: ago(4), devices: [{ id: "dev-amy", name: "Amy’s browser", kind: "browser", fingerprint: "Opaque session record", addedAt: ago(300), lastUsedAt: ago(4) }] },
  { id: "peer-jo", name: "Jordan Park", handle: "jordan", role: "contributor", status: "pending", devices: [] },
];
export const fixtureInvites: Invite[] = [{ id: "invite-jo", code: "c6://join/7FQ9-K2PM", role: "contributor", expiresAt: ahead(6), status: "pending", requestedBy: "Jordan Park" }];
export const fixtureJobs: Job[] = [
  { id: "job-sync", name: "sync-activity", kind: "cron", command: "cargo run --bin sync", schedule: "0 * * * *", timezone: "America/New_York", enabled: true, lastStatus: "succeeded", nextRunAt: ahead(1) },
  { id: "job-notes", name: "friday-notes", kind: "agent", command: "codex exec prompts/friday.md", schedule: "0 16 * * 5", timezone: "America/New_York", enabled: true, lastStatus: "succeeded", nextRunAt: ahead(72) },
  { id: "job-export", name: "export-pdf", kind: "command", command: "npm run export", enabled: true },
];
export const fixtureSecrets: SecretMetadata[] = [{ name: "OPENAI_API_KEY", scope: "project", grants: ["friday-notes"], updatedAt: ago(96), updatedBy: "Neal", version: 2 }, { name: "LINEAR_API_TOKEN", scope: "workspace", grants: ["sync-activity"], updatedAt: ago(200), updatedBy: "Amy", version: 1 }];
export const fixtureBranches: Branch[] = [{ name: "main", sha: "7c1a840d9f3", updatedAt: ago(1), protected: true }, { name: "agent/friday-notes/42", sha: "ae031ad", updatedAt: ago(2), protected: false, ahead: 2, behind: 0 }, { name: "amy/summary-layout", sha: "c39da01", updatedAt: ago(7), protected: false, ahead: 3, behind: 1 }];
export const runLog = `[16:00:02] Accepted run metadata\n[16:00:02] Pinned requested revision 2fa39bd\n[16:00:02] Entered safe simulation mode\n[16:00:02] No project code executed\n[16:00:02] No network access requested\n[16:00:02] Marked simulation complete`;
