# Cresix Cloud connected mode

Status: accepted for a dogfood vertical slice

## Product thesis

Cresix Cloud makes a sovereign C6 installation easy to find and reach. It does
not move the installation's repositories, runtime records, secrets, or local
authorization into a second control plane.

The product has two explicit operating modes:

- **Standalone C6** keeps the current local account, invitation, URL, and
  operator-managed ingress model. It does not require a Cresix account.
- **Connected C6** adds a Cresix account, a global workspace namespace, a
  directory entry, and an outbound connector to a managed relay. Disconnecting
  leaves the local installation intact.

This split applies YAGNI to centralization: Cresix centralizes only the pieces
that must be global—identity, namespace reservation, discovery, and
reachability—while local C6 remains the software authority.

## URLs and browser origins

The friendly URL is a hosted directory page:

```text
https://cresix.com/{workspace}/{project}
```

Opening the project crosses an explicit origin boundary to its installation:

```text
https://{opaque-route}.relay.cresix.com/projects/{project-slug}
```

The route identifier is opaque and stable across workspace renames. One route
origin represents one C6 installation, even when that installation contains
several workspaces.

The hosted directory must not transparently proxy unrelated C6 authorities
under paths on `cresix.com`. C6 browser sessions use installation-wide cookie
names and exact-origin CSRF validation; putting several installations on one
origin would collapse those boundaries. Hosted account cookies must be
host-only and must never use `Domain=.cresix.com`.

Future arbitrary project applications must use a separate registrable domain,
such as `*.cresixusercontent.com`, so untrusted software cannot become a
sibling of account or relay origins.

## Authority and data ownership

### Cresix Cloud owns

- immutable cloud account subjects and account sessions;
- globally unique workspace namespaces and cloud memberships;
- installation registrations, route identifiers, connector credential
  verifiers, revocation, and presence;
- bindings from a cloud workspace to a local installation and workspace UUID;
- a bounded, versioned project catalog projection; and
- cloud security and administration audit events.

### Local C6 owns

- local peers, devices, sessions, invitations, and the server administrator;
- workspace roles and project authorization;
- Git objects, refs, pull requests, and repository credentials;
- run, deployment, schedule, secret, and runtime state; and
- the authoritative local audit trail.

Cloud IDs and slugs are never filesystem paths or implicit local permissions.
A Cresix workspace owner does not become a local server administrator. In this
phase, a Cloud account does not provide local C6 single sign-on.

## Dogfood identity model

The locally runnable Cloud service uses a one-time bootstrap proof generated in
an owner-only data directory. Bootstrap is permitted only while Cloud binds to
a loopback address. Claim atomically creates the first account, consumes the
proof, and issues a host-only browser session.

This is deliberately not the production `cresix.com` login design. Public
hosting requires a reviewed passkey or OIDC adapter, account recovery, recent
authentication for destructive operations, login and enrollment rate limits,
security notifications, and incident-response tooling. Provider subjects must
be linked by immutable provider ID rather than mutable handle or email.

## Entities

All internal identities are UUIDs. User-facing handles and slugs are validated
presentation identifiers.

```text
Account
  id, handle, display_name, created_at, disabled_at

CloudSession
  id, account_id, verifier, csrf_verifier, created_at, expires_at, revoked_at

CloudWorkspace
  id, namespace, name, owner_account_id, created_at

CloudMembership
  workspace_id, account_id, role(owner|maintainer|member)

Installation
  id, local_server_id, route_id, owner_account_id, label,
  credential_public_id, credential_verifier, created_at, revoked_at

WorkspaceBinding
  workspace_id, installation_id, local_workspace_id, catalog_revision

CatalogProject
  binding_id, local_project_id, slug, name, description,
  default_branch, head_sha, updated_at

CloudAudit
  id, actor_type, actor_id, action, target_id, created_at
```

The dogfood audit intentionally stores no arbitrary detail payload. Typed
target categories and bounded redacted detail are deferred with an audit query
API; credentials and request data must never be added as generic details.

One installation may bind several Cloud workspaces. One Cloud workspace has at
most one active installation binding in v1. Namespace rename and reuse are
disabled in the dogfood slice.

## HTTP API

The dogfood API is versioned under `/api/v1`.

### Browser and directory

```text
GET    /api/v1/status
POST   /api/v1/bootstrap/claim
GET    /api/v1/session
DELETE /api/v1/session
GET    /api/v1/workspaces
POST   /api/v1/workspaces
GET    /api/v1/installations
POST   /api/v1/installations
DELETE /api/v1/installations/{id}
POST   /api/v1/workspaces/{id}/binding
PUT    /api/v1/installations/{id}/catalog
GET    /api/v1/directory/{namespace}/{project}
GET    /{namespace}/{project}
```

Installation creation reveals a connector credential exactly once. Cloud
stores only a verifier. Mutations require the browser session, exact Origin,
and a session-bound double-submit CSRF token.

Catalog writes are accepted only for the registered installation credential,
are limited in count and field sizes, and must monotonically advance the
binding revision. They never create or mutate local projects.

### Relay

```text
GET /api/v1/relay/connect         WebSocket subprotocol c6-relay-v1
ANY request on a registered relay authority
```

The directory API returns only intentionally published catalog metadata,
connection state, and the relay URL. Unknown namespaces and private entries
must not leak additional account information through distinct auth errors.

## Connector and relay protocol

The connector opens one outbound WebSocket and proxies only one configured
local HTTP origin, which defaults to exactly `http://127.0.0.1:8787`. It is not
a forward proxy and cannot select an arbitrary upstream per request.

The dogfood protocol multiplexes request UUIDs over strict JSON control frames
and bounded binary body frames:

```text
ClientHello -> ServerReady
RequestStart -> zero or more request chunks -> RequestEnd
ResponseStart -> zero or more response chunks -> ResponseEnd
Cancel | RequestFailed | Ping | Pong
```

Unknown fields, duplicate request starts, unknown IDs, chunks after completion,
invalid methods or headers, oversized frames, and illegal state transitions are
rejected. A newer authenticated connection fences the previous installation
generation. Disconnection fails in-flight requests without retrying mutations.

Both ends remove hop-by-hop headers and all client-supplied forwarding and
internal routing headers. Relay and connector credentials are never forwarded
to C6. The relay selects an installation from the validated public authority,
not from a request header supplied by the client.

Protocol ceilings are 64 KiB chunks, 16 MiB request bodies, 64 MiB responses,
and at most 32 concurrent requests per installation. The current dogfood relay
deliberately advertises and executes one serial request with one bounded queue
and one total deadline. The connector reconnects transient failures with
exponential backoff and jitter; authentication and revocation failures stop
until configuration changes.

The v1 relay does not support raw TCP, UDP, tunneled WebSocket upgrades,
trailers, transparent TLS, automatic request retry, or multi-relay HA.

## Security model

### Assets and actors

Assets include Cloud sessions, connector credentials, workspace namespace
control, route bindings, local C6 cookies and Git credentials in transit, and
private source moved through the relay. Threat actors include anonymous public
clients, malicious Cloud members, a stolen connector credential, a compromised
relay, and a compromised local user or connector process.

### Controls in the dogfood slice

- deny-by-default browser and installation authorization;
- high-entropy, hashed, independently revocable session and connector secrets;
- host-only browser cookies, exact Origin checks, and bound CSRF tokens;
- strict lowercase namespace and project-slug validation;
- UUID storage paths and parameterized SQLite access;
- loopback-only dev bootstrap and loopback-only connector upstream;
- fixed route-to-installation mappings and active-connection fencing;
- request, response, concurrency, queue, and deadline limits;
- sensitive-data-safe logging that excludes paths, queries, headers, and
  bodies; and
- truthful `401`, `404`, `413`, `502`, and `504` states in the dogfood relay;
  `429` admission control and the production offline `503` contract remain
  required with concurrent isolated-origin ingress.

### Residual risks

Cresix's edge terminates TLS and can observe proxied cookies, Git credentials,
source, and response bodies. This is trusted ingress, not relay-blind
end-to-end encryption. A stolen connector credential can impersonate or
disrupt that installation's tunnel, though it cannot directly authenticate as
a local user. The dogfood account bootstrap, absent public rate limiting,
single-node relay registry, and existing local C6 recovery limitations block
production public or hostile multi-tenant use.

## User journeys

1. Claim a loopback Cloud preview and create a globally unique workspace.
2. Register a local C6 installation and copy its one-time connector credential.
3. Bind the Cloud workspace to an existing local workspace UUID.
4. Start `c6-connector` with an owner-only credential file.
5. Publish a bounded catalog projection and observe the installation become
   connected.
6. Bookmark `cresix.com/{workspace}/{project}` as the claimed account's stable
   directory URL.
7. The workspace member sees the directory doorway and, once an isolated relay
   origin exists, deliberately opens the installation origin where local C6
   authentication remains authoritative.
8. Revoking the installation terminates ingress; standalone/local access and
   all local data remain intact.

## Visual system

Cloud uses Forge ink `#18231F`, Mineral `#F3F6F4`, Paper `#FFFFFF`, Relay green
`#276B54`, Route blue `#315FA8`, and Signal amber `#B86E17`. Brand/display type
uses the platform rounded face sparingly; body uses the platform interface
face; route IDs, commands, and revisions use monospace.

The signature element is the **route seam**, a compact, truthful path from
account to workspace to project to the isolated installation port:

```text
@neal -- paper-street -- weeknote --> route-7fk2.relay.cresix.com
```

The UI favors grouped repository-style rows and rules over metric cards,
gradients, or decorative network diagrams. It must remain keyboard accessible,
responsive to 320 CSS pixels, status-readable without color, and motion-free
under `prefers-reduced-motion`.

## Migration, disconnect, and rollback

Connecting is additive: it preserves the local `server_id`, UUIDs, database,
repositories, roles, and sessions. Changing `C6_PUBLIC_BASE_URL` changes the
browser origin, so users must authenticate again and may need to update Git
remotes.

Disconnecting stops and removes the connector credential and revokes the Cloud
installation/route. The Cloud projection may remain stale until explicitly
tombstoned, but it must be shown offline and never treated as authoritative.
Loopback or operator-managed standalone ingress continues to work.

## Explicit deferrals

- production passkey/OIDC login and account recovery;
- Cloud account enrollment, invitations, membership management, and
  recipient-facing sharing;
- Cloud-to-local SSO and membership synchronization;
- Cloud mutation of local projects, roles, Git, or runtime state;
- custom domains and namespace rename;
- several installations serving one workspace;
- public repositories and anonymous local access;
- Git push, SSH, application hosting, secrets, and real agent execution;
- relay-blind encryption, relay HA, billing, and federation.

## Acceptance gates

- Existing standalone QA remains green.
- Cloud core validators and serialization have unit and abuse tests.
- Browser bootstrap, CSRF, workspace, registration, binding, revocation, and
  directory flows have positive and negative API tests.
- Headless Chromium covers the hosted directory, responsive layout, keyboard
  navigation, connected and offline states, and origin-transition copy.
- A real local test runs Cloud, a loopback backend, and the connector; verifies
  status, headers, binary bodies, concurrent requests, offline behavior,
  revocation, and no automatic mutation retry.
- Source scanning and captured logs show no plaintext session, connector, Git,
  or bootstrap credentials.
- Documentation and UI describe the phase as dogfood and do not claim secure
  public multi-tenant readiness.
