# Web product

The React application presents a GitHub-like home for small software: workspace
navigation, projects, source, pull requests, deployments, jobs, runs, secrets,
peers, server settings, first claim, and invitation redemption.

## Information architecture

- **Installation flows:** unclaimed status routes to claim; a fragment invite
  routes to join; an authenticated session routes to the workspace.
- **Workspace home:** project discovery and creation.
- **Project spine:** source → review → recorded runtime intent.
- **Trust settings:** invitations, peers, devices, and sessions.
- **Server settings:** local health, reachability, and operational limitations.

## Truth conventions

The UI must distinguish four sources:

1. **Live:** returned by an authenticated API.
2. **Preview fixture:** explicitly labelled sample data used to demonstrate an
   unsupported screen.
3. **Recorded:** durable intent such as a run/deployment with no dispatch.
4. **Deferred:** disabled or explanatory behavior, never a success toast.

It must not translate `recorded` into queued/running, show a deployment as live,
claim secrets are stored, imply `publicKey` is authentication, or imply the
simulation runner invokes host processes. API failure cannot silently become a
successful mutation; fixture fallback is presentation-only and visibly marked.

## Browser security

The app sends same-origin credentials, reads only the non-HttpOnly CSRF cookie,
and mirrors it into `X-C6-CSRF` for mutations. Invitation tokens are parsed from
the URL fragment. The session cookie remains inaccessible to JavaScript.

Vite development must start the server with
`C6_PUBLIC_BASE_URL=http://127.0.0.1:5173`; otherwise strict origin checks
correctly reject proxied mutations.

## Quality bar

Core journeys need loading, empty, unauthenticated, unauthorized, API-failure,
and narrow-screen states. Keyboard focus, semantic controls, readable contrast,
and stable layout are regression requirements. Playwright tests drive a real
C6 backend for claim, workspace/project creation, and trust flows; component
tests cover rendering and API edge behavior.
