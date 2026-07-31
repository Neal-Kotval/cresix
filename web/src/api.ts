import { fixtureProject, fixtureProjects, fixtureSession } from "./fixtures";
import type { ProjectDetail, ProjectSummary, PullRequest, Run, Session } from "./types";

let csrfToken = "";
const demoEnabled = import.meta.env.VITE_C6_DEMO === "1";

export class ApiError extends Error { constructor(public status: number, message: string) { super(message); } }
function csrfCookie() { const encoded = document.cookie.split("; ").find((cookie) => cookie.startsWith("c6_csrf="))?.slice(8); return encoded ? decodeURIComponent(encoded) : ""; }
async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const mutating = init?.method && !["GET", "HEAD", "OPTIONS"].includes(init.method.toUpperCase());
  const currentCsrf = csrfCookie() || csrfToken;
  const response = await fetch(path, { ...init, credentials: "same-origin", headers: { "content-type": "application/json", ...(mutating && currentCsrf ? { "x-c6-csrf": currentCsrf } : {}), ...init?.headers } });
  if (!response.ok) { const detail = await response.json().catch(() => ({})) as { error?: string | { message?: string } }; const message = typeof detail.error === "string" ? detail.error : detail.error?.message; throw new ApiError(response.status, message ?? `C6 request failed (${response.status})`); }
  if (response.status === 204) return undefined as T;
  return response.json() as Promise<T>;
}
const post = <T>(path: string, body: unknown) => request<T>(path, { method: "POST", body: JSON.stringify(body) });
const handle = (name: string) => name.toLowerCase().replace(/[^a-z0-9]+/g, "-");
type SessionEnvelope = Omit<Session, "user"> & { user: { id: string; displayName: string; handle?: string } };
const normalizeSession = (value: SessionEnvelope): Session => ({ ...value, serverAdministrator: value.serverAdministrator === true, user: { ...value.user, handle: value.user.handle ?? handle(value.user.displayName) } });

async function projectDetail(summary: ProjectSummary): Promise<ProjectDetail> {
  const base = `/api/v1/projects/${summary.id}`;
  const [pulls, deployments, runs, commits] = await Promise.all([
    request<{ pullRequests: PullRequest[] }>(`${base}/pull-requests`).catch(() => ({ pullRequests: [] })),
    request<{ deployments: ProjectDetail["deployments"] }>(`${base}/deployments`).catch(() => ({ deployments: [] })),
    request<{ runs: Array<Run & { createdAt?: string }> }>(`${base}/runs`).catch(() => ({ runs: [] })),
    request<{ commits: Array<ProjectDetail["revisions"][number] | { oid: string; message: string; author_name: string; author_email: string; authored_at: string }> }>(`${base}/repository/commits`).catch(() => ({ commits: [] })),
  ]);
  return { ...summary, readme: "", pullRequests: pulls.pullRequests.map((pull) => ({ ...pull, author: pull.author ?? { id: "unknown", handle: "peer", displayName: "Trusted peer" } })), deployments: deployments.deployments, runs: runs.runs.map((run) => ({ ...run, trigger: run.trigger ?? "manual", startedAt: run.startedAt ?? run.createdAt ?? summary.updatedAt })), revisions: commits.commits.map((commit) => "oid" in commit ? { sha: commit.oid, message: commit.message, author: { id: commit.author_email, handle: handle(commit.author_name), displayName: commit.author_name }, createdAt: commit.authored_at } : commit) };
}

export const api = {
  bootstrap: async () => {
    try {
      const status = await request<{ claimed: boolean }>("/api/v1/status");
      if (!status.claimed) return { state: "unclaimed" as const, session: undefined, projects: [] };
      const [rawSession, projectEnvelope] = await Promise.all([request<SessionEnvelope>("/api/v1/session"), request<{ projects: ProjectSummary[] }>("/api/v1/projects")]);
      const session = normalizeSession(rawSession); csrfToken = session.csrfToken ?? "";
      return { state: "ready" as const, session, projects: projectEnvelope.projects, preview: false };
    } catch (error) {
      if (demoEnabled && (!(error instanceof ApiError) || error.status >= 500)) return { state: "ready" as const, session: fixtureSession, projects: fixtureProjects, preview: true };
      if (error instanceof ApiError && error.status === 401) return { state: "unauthorized" as const, session: undefined, projects: [] };
      throw error;
    }
  },
  project: async (slug: string) => {
    if (demoEnabled) { try { const envelope = await request<{ projects: ProjectSummary[] }>("/api/v1/projects"); const match = envelope.projects.find((item) => item.slug === slug); if (match) return { data: await projectDetail(match), preview: false }; } catch { return { data: { ...fixtureProject, slug, name: slug === "weeknote" ? "Weeknote" : title(slug) }, preview: true }; } }
    const envelope = await request<{ projects: ProjectSummary[] }>("/api/v1/projects");
    const match = envelope.projects.find((item) => item.slug === slug || item.id === slug);
    if (!match) throw new ApiError(404, "Project not found");
    return { data: await projectDetail(match), preview: false };
  },
  claim: (input: { token: string; displayName: string; deviceLabel: string; publicKey: string }) => post("/api/v1/bootstrap/claim", input),
  redeemInvite: (input: { token: string; displayName: string; deviceLabel: string; publicKey: string }) => post("/api/v1/invites/redeem", input),
  createProject: (input: { workspaceId: string; slug: string; name: string; description: string; defaultBranch: string }) => post<ProjectSummary>("/api/v1/projects", input),
  createWorkspace: (input: { slug: string; name: string }) => post<Session["workspaces"][number]>("/api/v1/workspaces", input),
  createInvite: (input: { role: string; expiresInMinutes: number; workspaceId?: string }) => post<{ id: string; token: string; expiresAt: string; inviteUrl: string }>("/api/v1/invites", input),
  peers: () => request<{ peers: Array<{ id: string; displayName: string; revokedAt?: string }> }>("/api/v1/peers"),
  invites: () => request<{ invites: Array<{ id: string; role: string; workspaceId?: string; expiresAt: string; redeemedAt?: string }> }>("/api/v1/invites"),
  run: (projectId: string, job: string, kind: Run["kind"], revisionSha?: string) => post<Run>(`/api/v1/projects/${projectId}/runs`, { job, kind, revisionSha }),
  deploy: (projectId: string, revisionSha: string, environment = "production") => post(`/api/v1/projects/${projectId}/deployments`, { revisionSha, environment }),
};
function title(value: string) { return value.replaceAll("-", " ").replace(/\b\w/g, (letter) => letter.toUpperCase()); }
