import type { ProjectDetail, Run } from "./types";

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...init,
    headers: { "content-type": "application/json", ...init?.headers },
  });
  if (!response.ok) {
    throw new Error(`C6 request failed (${response.status})`);
  }
  return response.json() as Promise<T>;
}

export const api = {
  project: (slug: string) => request<ProjectDetail>(`/api/v1/projects/${slug}`),
  run: (slug: string, job: string, kind: Run["kind"]) =>
    request<Run>(`/api/v1/projects/${slug}/runs`, {
      method: "POST",
      body: JSON.stringify({ job, kind }),
    }),
  publish: (slug: string) =>
    request(`/api/v1/projects/${slug}/publish`, { method: "POST", body: "{}" }),
};
