# Web product

The React application contains two named surfaces served by one C6 server: C6
Hub for workspace/project collaboration and C6 Admin for installation
operations. They share one session and API; this is navigation and
responsibility separation, not a second frontend deployment or control plane.

## First-slice route contract

- **Installation flows:** unclaimed status routes to claim; a fragment invite
  routes to join; an authenticated session enters C6 Hub by default.
- **C6 Hub:** `/` is workspace/project discovery and `/projects/*` is the
  project spine. Workspace membership management is deferred; the project UI
  does not misroute members to installation-wide peer controls.
- **C6 Admin:** `/admin` presents installation state and operational boundaries;
  `/admin/access` presents global invitations and peers. Personal device and
  session endpoints are self-service and do not grant cross-peer visibility.
- **Compatibility:** legacy `/settings/server` and `/settings/peers` redirect to
  their canonical Admin routes. New navigation uses Hub/Admin routes.

Browser routes select a surface while all data and mutations still pass through
`/api/v1` on the same origin. No Hub/Admin API split exists.

## Capability and role boundary

The authenticated session exposes a boolean `serverAdministrator` capability.
Admin navigation and installation-wide actions use that capability. A workspace
role of `owner` does not set it and must never be treated as an equivalent.

Peers without the capability remain in Hub. Direct Admin navigation must render
an explicit unauthorized state rather than silently showing fixtures or
inferring privilege from workspace membership.

The implemented `c6` CLI is another thin client of these same API and
capability checks. Hub's `/credentials` page issues, lists, and revokes
separate CLI and read-only Git credentials. Plaintext is shown once; metadata
views never recover it. Push remains visibly unavailable.

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

The surface split adds regression requirements for Hub/Admin navigation,
`serverAdministrator` gating, workspace-owner denial in Admin, and legacy
`/settings/*` compatibility.

## Separate Cresix Cloud surface

`cloud-web` is not another route inside local C6 Hub/Admin. It owns the preview
Cloud account, workspace, installation, and directory experience. Workspace
uniqueness is currently local to one preview database; a production-global
account service is deferred. The target directory URL
`cresix.com/@{account}/{workspace}/{project}` must show an explicit handoff
to a separate opaque relay origin; it must not imply that a Cloud session grants
access to local C6. Disconnected, offline, revoked, and not-found states are
first-class rather than hidden behind a generic loading state.

The signature route seam makes the authority transition legible: Cloud account
to Cloud workspace to directory project to isolated installation origin.
One-time connector credentials are shown only in the authenticated creation
response, are never placed in a URL or fixture, and cannot be recovered from the
page after dismissal. The Cloud visual and interaction contract is detailed in
the [connected-mode specification](specs/CRESIX_CLOUD_CONNECTED_MODE.md).

The current loopback preview has no public account handle and temporarily uses
`/{workspace}/{project}`. UI fixtures must not present that temporary route as
the settled public namespace model.
