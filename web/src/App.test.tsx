import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";

describe("C6 project page", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({
        id: "project-1", workspaceId: "workspace-1", slug: "weeknote", name: "Weeknote",
        description: "A tiny shared app", defaultBranch: "main", headSha: "7c1a840",
        publishedSha: "2fa39bd", role: "owner", updatedAt: new Date().toISOString(),
        readme: "# Weeknote", revisions: [], pullRequests: [], deployments: [], runs: [],
      }),
    }));
  });

  it("centers the working software and its lifecycle", async () => {
    render(<App />);
    expect(await screen.findByRole("heading", { level: 1, name: "Weeknote" })).toBeInTheDocument();
    expect(screen.getAllByText("Published")).toHaveLength(2);
    expect(screen.getByText("Next run")).toBeInTheDocument();
  });
});
