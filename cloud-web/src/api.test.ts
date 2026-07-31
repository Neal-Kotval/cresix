import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError, cloudApi } from "./api";

afterEach(() => vi.restoreAllMocks());

describe("cloudApi", () => {
  it("normalizes snake_case API responses", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(
        JSON.stringify({
          account: { id: "a1", handle: "neal", display_name: "Neal" },
          csrf_token: "csrf",
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      ),
    );

    await expect(cloudApi.session()).resolves.toMatchObject({
      account: { displayName: "Neal" },
      csrfToken: "csrf",
    });
  });

  it("binds mutations to the session CSRF token", async () => {
    const fetchSpy = vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ id: "w1", namespace: "lab" }), { status: 200 }),
    );

    await cloudApi.createWorkspace("session-csrf", "lab", "Lab");

    const [, init] = fetchSpy.mock.calls[0];
    expect(new Headers(init?.headers).get("x-c6-csrf")).toBe("session-csrf");
    expect(init?.credentials).toBe("same-origin");
  });

  it("keeps live workspace bindings and catalog projects discoverable", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ workspaces: [{
        id: "w1",
        namespace: "paper-street",
        name: "Paper Street",
        role: "owner",
        binding: {
          id: "b1",
          workspaceId: "w1",
          installationId: "i1",
          localWorkspaceId: "local-w1",
          catalogRevision: 4,
        },
        projects: [{
          bindingId: "b1",
          localProjectId: "p1",
          slug: "weeknote",
          name: "Weeknote",
          description: "Tiny notes",
          defaultBranch: "main",
          headSha: "abc123",
          updatedAt: "2026-07-31T12:00:00Z",
        }],
      }] }), { status: 200 }),
    );

    await expect(cloudApi.workspaces()).resolves.toMatchObject([{
      binding: { installationId: "i1", catalogRevision: 4 },
      projects: [{ id: "p1", slug: "weeknote" }],
    }]);
  });

  it("does not leak non-JSON error bodies", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("gateway detail", { status: 503 }));

    await expect(cloudApi.installations()).rejects.toEqual(
      expect.objectContaining({ status: 503, message: "Request failed (503)" }),
    );
  });
});
