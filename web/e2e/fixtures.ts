import type { Page, Route } from "@playwright/test";
import { fixtureProject, fixtureProjects, fixtureSession } from "../src/fixtures";

export async function mockC6(page: Page, options: { empty?: boolean; delay?: number; unauthorized?: boolean; serverAdministrator?: boolean; noWorkspaces?: boolean } = {}) {
  await page.route("**/api/v1/**", async (route: Route) => {
    if (options.delay) await new Promise((resolve) => setTimeout(resolve, options.delay));
    const url = new URL(route.request().url());
    const path = url.pathname;
    if (path.endsWith("/status")) return route.fulfill({ json: { claimed: true, authentication: "peer_trust" } });
    if (options.unauthorized && path.includes("/projects/")) return route.fulfill({ status: 403, json: { error: "forbidden" } });
    if (path.endsWith("/session")) return route.fulfill({ json: { ...fixtureSession, workspaces: options.noWorkspaces ? [] : fixtureSession.workspaces, serverAdministrator: options.serverAdministrator ?? fixtureSession.serverAdministrator, csrfToken: "fresh-session-proof" } });
    if (path.endsWith("/workspaces") && route.request().method() === "POST") return route.fulfill({ status: 201, json: fixtureSession.workspaces[0] });
    if (path === "/api/v1/projects" && route.request().method() === "POST") return route.fulfill({ status: 201, json: { ...fixtureProject, name: "Release Notes", slug: "release-notes" } });
    if (path === "/api/v1/projects") return route.fulfill({ json: { projects: options.empty ? [] : fixtureProjects } });
    if (path.endsWith("/pull-requests")) return route.fulfill({ json: { pullRequests: fixtureProject.pullRequests } });
    if (path.endsWith("/deployments")) return route.fulfill({ json: { deployments: fixtureProject.deployments } });
    if (path.endsWith("/repository/commits")) return route.fulfill({ json: { commits: fixtureProject.revisions } });
    if (path.endsWith("/runs") && route.request().method() === "GET") return route.fulfill({ json: { runs: fixtureProject.runs } });
    if (path.endsWith("/runs") && route.request().method() === "POST") return route.fulfill({ status: 202, json: { ...fixtureProject.runs[0], id: "run-queued", status: "queued", trigger: "neal (manual)" } });
    if (path.endsWith("/publish")) return route.fulfill({ status: 202, json: { accepted: true } });
    if (path.endsWith("/trust/pair")) return route.fulfill({ status: 201, json: { serverName: "neal-macbook", workspaceName: "Paper Street", inviterName: "Neal" } });
    if (path.endsWith("/invites") && route.request().method() === "POST") return route.fulfill({ status: 201, json: { id: "invite-new", token: "opaque", expiresAt: new Date(Date.now() + 86_400_000).toISOString(), inviteUrl: "/join#token=opaque" } });
    if (path.endsWith("/invites")) return route.fulfill({ json: { invites: [] } });
    if (path.endsWith("/peers")) return route.fulfill({ json: { peers: [{ id: fixtureSession.user.id, displayName: fixtureSession.user.displayName }] } });
    if (path.endsWith("/invites/redeem")) return route.fulfill({ status: 201, json: { user: { id: "peer-new", displayName: "New peer" } } });
    if (path.includes("/projects/")) return route.fulfill({ json: fixtureProject });
    return route.fulfill({ status: 404, json: { error: "not found" } });
  });
}
