import type {
  CloudStatus,
  DirectoryEntry,
  Installation,
  InstallationCreated,
  Session,
  Workspace,
} from "./types";

type JsonObject = Record<string, unknown>;

export class ApiError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

function camelize<T>(value: unknown): T {
  if (Array.isArray(value)) return value.map(camelize) as T;
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, entry]) => [
        key.replace(/_([a-z])/g, (_, letter: string) => letter.toUpperCase()),
        camelize(entry),
      ]),
    ) as T;
  }
  return value as T;
}

async function request<T>(path: string, init: RequestInit = {}, csrfToken?: string): Promise<T> {
  const headers = new Headers(init.headers);
  if (init.body) headers.set("content-type", "application/json");
  if (csrfToken) headers.set("x-c6-csrf", csrfToken);
  const response = await fetch(`/api/v1${path}`, {
    ...init,
    headers,
    credentials: "same-origin",
  });
  if (!response.ok) {
    let message = `Request failed (${response.status})`;
    try {
      const body = (await response.json()) as { error?: string; message?: string };
      message = body.message ?? body.error ?? message;
    } catch {
      // Error bodies are not required to be JSON.
    }
    throw new ApiError(message, response.status);
  }
  if (response.status === 204) return undefined as T;
  return camelize<T>(await response.json());
}

function object(value: unknown): JsonObject {
  return (value && typeof value === "object" ? value : {}) as JsonObject;
}

function normalizeWorkspace(value: unknown): Workspace {
  const item = object(value);
  return {
    id: String(item.id ?? ""),
    namespace: String(item.namespace ?? ""),
    name: String(item.name ?? ""),
    role: (item.role as Workspace["role"]) ?? "member",
    binding: item.binding as Workspace["binding"],
    projects: Array.isArray(item.projects) ? item.projects.map(normalizeProject) : [],
  };
}

function normalizeInstallation(value: unknown): Installation {
  const item = object(value);
  const rawState = String(item.connectionState ?? item.state ?? "disconnected");
  const state = rawState === "disconnected" ? "offline" : rawState;
  return {
    id: String(item.id ?? ""),
    localServerId: String(item.localServerId ?? ""),
    routeId: String(item.routeId ?? ""),
    label: String(item.label ?? ""),
    state: state as Installation["state"],
    lastSeenAt: String(item.connectedAt ?? item.lastSeenAt ?? "") || undefined,
    revokedAt: String(item.revokedAt ?? "") || undefined,
  };
}

function normalizeProject(value: unknown): DirectoryEntry["project"] {
  const item = object(value);
  return {
    id: String(item.id ?? item.localProjectId ?? ""),
    slug: String(item.slug ?? ""),
    name: String(item.name ?? ""),
    description: String(item.description ?? ""),
    defaultBranch: String(item.defaultBranch ?? "main"),
    headSha: String(item.headSha ?? ""),
    updatedAt: String(item.updatedAt ?? ""),
  };
}

export const cloudApi = {
  status: async () => {
    const status = await request<JsonObject>("/status");
    return { claimed: Boolean(status.claimed), dogfood: true } satisfies CloudStatus;
  },
  claim: (proof: string, handle: string, displayName: string) =>
    request<Session>("/bootstrap/claim", {
      method: "POST",
      body: JSON.stringify({ bootstrapToken: proof, handle, displayName }),
    }),
  session: () => request<Session>("/session"),
  signOut: (csrf: string) => request<void>("/session", { method: "DELETE" }, csrf),
  workspaces: async () => {
    const response = await request<JsonObject>("/workspaces");
    return (Array.isArray(response) ? response : response.workspaces as unknown[] ?? []).map(
      normalizeWorkspace,
    );
  },
  createWorkspace: async (csrf: string, namespace: string, name: string) =>
    normalizeWorkspace(
      await request<unknown>(
        "/workspaces",
        { method: "POST", body: JSON.stringify({ namespace, name }) },
        csrf,
      ),
    ),
  installations: async () => {
    const response = await request<JsonObject>("/installations");
    return (Array.isArray(response) ? response : response.installations as unknown[] ?? []).map(
      normalizeInstallation,
    );
  },
  createInstallation: async (csrf: string, label: string, localServerId: string) => {
    const response = await request<JsonObject>(
      "/installations",
      { method: "POST", body: JSON.stringify({ label, localServerId }) },
      csrf,
    );
    return {
      ...normalizeInstallation(response.installation),
      connectorCredential: String(response.connectorToken ?? ""),
    } satisfies InstallationCreated;
  },
  revokeInstallation: (csrf: string, id: string) =>
    request<void>(`/installations/${encodeURIComponent(id)}`, { method: "DELETE" }, csrf),
  bindWorkspace: (csrf: string, workspaceId: string, installationId: string, localWorkspaceId: string) =>
    request<unknown>(
      `/workspaces/${encodeURIComponent(workspaceId)}/binding`,
      {
        method: "POST",
        body: JSON.stringify({
          installationId,
          localWorkspaceId,
        }),
      },
      csrf,
    ),
  directory: async (namespace: string, project: string) => {
    const response = await request<JsonObject>(
      `/directory/${encodeURIComponent(namespace)}/${encodeURIComponent(project)}`,
    );
    const installation = normalizeInstallation(response.installation);
    return {
      workspace: normalizeWorkspace(response.workspace),
      project: normalizeProject(response.project),
      installation,
      relayUrl: installation.state === "connected" ? String(response.relayUrl ?? "") : null,
    } satisfies DirectoryEntry;
  },
};
