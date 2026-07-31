import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";

describe("API request security", () => {
  beforeEach(() => { document.cookie = "c6_csrf=; Max-Age=0; path=/"; });

  it("sends the current CSRF cookie with unsafe requests", async () => {
    document.cookie = "c6_csrf=reload-safe-token; SameSite=Strict; path=/";
    const fetch = vi.fn().mockResolvedValue({ ok: true, json: async () => ({ id: "run-new" }) });
    vi.stubGlobal("fetch", fetch);
    await api.run("weeknote", "sync-activity", "cron");
    expect(fetch).toHaveBeenCalledWith("/api/v1/projects/weeknote/runs", expect.objectContaining({
      credentials: "same-origin", headers: expect.objectContaining({ "x-c6-csrf": "reload-safe-token" }),
    }));
  });

  it("does not send CSRF values on safe reads", async () => {
    document.cookie = "c6_csrf=private-proof; SameSite=Strict; path=/";
    const fetch = vi.fn().mockResolvedValue({ ok: true, json: async () => ({ projects: [] }) });
    vi.stubGlobal("fetch", fetch);
    await expect(api.project("weeknote")).rejects.toThrow("Project not found");
    expect(fetch).toHaveBeenCalledWith("/api/v1/projects", expect.objectContaining({ headers: expect.not.objectContaining({ "x-c6-csrf": expect.anything() }) }));
  });

  it("maps the UI credential type to the strict wire contract without leaking aliases", async () => {
    document.cookie = "c6_csrf=create-proof; SameSite=Strict; path=/";
    const wireCredential = { id: "credential-1", userId: "user-1", deviceId: "device-1", type: "git", label: "Laptop Git", scopes: ["git:read"], createdAt: "2026-07-31T00:00:00Z", expiresAt: "2026-08-30T00:00:00Z" };
    const fetch = vi.fn().mockResolvedValue({ ok: true, status: 201, json: async () => ({ credential: wireCredential, token: "c6g_v1_public_secret" }) });
    vi.stubGlobal("fetch", fetch);
    const created = await api.createCredential({ credentialType: "git", label: "Laptop Git", expiresAt: wireCredential.expiresAt, scopes: ["git:read"] });
    expect(created.credential.credentialType).toBe("git");
    const body = JSON.parse(fetch.mock.calls[0][1].body as string);
    expect(body).toMatchObject({ type: "git", label: "Laptop Git", scopes: ["git:read"] });
    expect(body).not.toHaveProperty("credentialType");
  });

  it("rejects malformed clone metadata at the frontend boundary", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true, status: 200, json: async () => ({ cloneUrl: "https://c6.example/repo.git" }) }));
    await expect(api.projectRemote("project-1")).rejects.toThrow("invalid clone details");
  });
});
