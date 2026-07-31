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
});
