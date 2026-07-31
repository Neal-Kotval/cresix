import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { fixtureProject, fixtureProjects, fixtureSession } from "./fixtures";

function response(data: unknown, ok = true) { return Promise.resolve({ ok, status: ok ? 200 : 503, json: async () => data }); }
function setRoute(path: string) { history.replaceState({}, "", path); }

describe("C6 application routes", () => {
  afterEach(() => cleanup());
  beforeEach(() => {
    setRoute("/");
    vi.stubGlobal("scrollTo", vi.fn());
    vi.stubGlobal("fetch", vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const path = String(input);
      if (path.endsWith("/status")) return response({ claimed: true });
      if (path.endsWith("/session")) return response(fixtureSession);
      if (path.endsWith("/projects")) return response({ projects: fixtureProjects });
      if (path.includes("/pull-requests")) return response({ pullRequests: fixtureProject.pullRequests });
      if (path.includes("/deployments")) return response({ deployments: fixtureProject.deployments });
      if (path.includes("/runs")) return response({ runs: fixtureProject.runs });
      if (path.includes("/repository/commits")) return response({ commits: fixtureProject.revisions });
      if (path.endsWith("/peers")) return response({ peers: [{ id: "u-neal", displayName: "Neal Kotval" }] });
      if (path.endsWith("/invites") && init?.method !== "POST") return response({ invites: [] });
      if (path.endsWith("/invites")) return response({ id: "invite-new", inviteUrl: "/join#token=opaque", expiresAt: new Date(Date.now() + 86_400_000).toISOString() });
      if (path.match(/\/projects\/[\w-]+$/)) return response(fixtureProject);
      return response({});
    }));
  });

  it("shows the workspace and filters projects", async () => {
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Your small software" })).toBeInTheDocument();
    expect(screen.getAllByRole("link", { name: /Weeknote/ }).length).toBeGreaterThan(0);
    fireEvent.change(screen.getByRole("textbox", { name: "Filter projects" }), { target: { value: "nothing here" } });
    expect(screen.getByText("No projects match")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Clear filter" }));
    expect(screen.getAllByRole("link", { name: /Receipt Box/ }).length).toBeGreaterThan(0);
  });

  it("navigates through the repository without a document reload", async () => {
    render(<App />);
    fireEvent.click((await screen.findAllByRole("link", { name: /Weeknote/ })).find((link) => link.classList.contains("project-main"))!);
    expect(await screen.findByRole("heading", { level: 1, name: "Weeknote" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("link", { name: "Files" }));
    expect(screen.getByRole("button", { name: /c6.toml/ })).toBeInTheDocument();
    expect(location.pathname).toBe("/projects/weeknote/files");
  });

  it("exposes commit, branch, pull request, and deployment history", async () => {
    setRoute("/projects/weeknote/branches"); render(<App />);
    expect(await screen.findByText("agent/friday-notes/42")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("link", { name: "Commits" }));
    expect(screen.getByText("Make summaries easier to scan")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("link", { name: /Pull requests/ }));
    expect(screen.getByText("Draft summaries from the week’s activity")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("link", { name: "Deployments" }));
    expect(screen.getByText("Metadata only in this release")).toBeInTheDocument();
  });

  it("labels illustrative jobs and keeps their controls inert", async () => {
    setRoute("/projects/weeknote/jobs"); render(<App />);
    const toggle = await screen.findByRole("button", { name: "Disable sync-activity" });
    expect(toggle).toBeDisabled();
    expect(screen.getAllByRole("button", { name: "Run" })[0]).toBeDisabled();
    expect(screen.getByText(/Illustrative job definitions/)).toBeInTheDocument();
  });

  it("opens a run log from history", async () => {
    setRoute("/projects/weeknote/runs"); render(<App />);
    fireEvent.click((await screen.findAllByRole("button", { name: /sync-activity/ }))[0]);
    expect(screen.getByRole("heading", { name: "Run intent #46" })).toBeInTheDocument();
    expect(screen.getByText(/No project code executed/)).toBeInTheDocument();
  });

  it("shows only secret metadata and grants", async () => {
    setRoute("/projects/weeknote/secrets"); render(<App />);
    expect(await screen.findByText("OPENAI_API_KEY")).toBeInTheDocument();
    expect(screen.getAllByText("friday-notes").length).toBeGreaterThan(0);
    expect(screen.queryByText(/sk-/)).not.toBeInTheDocument();
  });

  it("creates trusted peer invitations without pretending fixture approvals persist", async () => {
    setRoute("/settings/peers"); render(<App />);
    expect(await screen.findByRole("button", { name: "Approve" })).toBeDisabled();
    await waitFor(() => expect(screen.getByText("Local peer record")).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: "Invite peer" }));
    const dialog = screen.getByRole("heading", { name: "Invite a trusted peer" }).closest("form")!;
    fireEvent.change(within(dialog).getByRole("combobox"), { target: { value: "reader" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Create invite" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Invite created"));
  });

  it("supports server claim and missing invitation recovery", () => {
    setRoute("/claim"); const { unmount } = render(<App />);
    expect(screen.getByRole("textbox", { name: "Bootstrap token" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Claim server" })).toBeDisabled();
    unmount(); setRoute("/join"); render(<App />);
    expect(screen.getByRole("heading", { name: "Invitation unavailable" })).toBeInTheDocument();
  });

  it("scrubs pairing tokens from browser history immediately", async () => {
    setRoute("/join#token=sensitive-one-time-token"); render(<App />);
    await waitFor(() => expect(location.hash).toBe(""));
    expect(location.pathname).toBe("/join");
    expect(document.body.textContent).not.toContain("sensitive-one-time-token");
  });
});
