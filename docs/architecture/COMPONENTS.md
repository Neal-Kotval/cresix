# Components and boundaries

## Executable and library map

| Component | Responsibility | Must not own | Status |
| --- | --- | --- | --- |
| `c6-server` | Local HTTP authority, sessions, authorization, project/runtime records, audit, static web, Git routing | Cloud accounts, connector presence, real container execution | Implemented |
| `c6-core` | Shared local domain DTOs and strict manifest parsing | Persistence, networking, policy decisions | Implemented |
| `c6-git` | Bounded access to UUID-addressed bare repositories | Identity, workspace policy, arbitrary filesystem paths | Implemented |
| `c6-scheduler` | Cron validation and next-occurrence calculation | Tick loop, occurrence claiming, dispatch | Implemented library only |
| `c6-client` | Typed local C6 HTTP client behavior | Token minting, role bypass, persistence | Implemented |
| `c6-cli` | Human/JSON CLI and credential helper | Resident daemon or second control plane | Implemented, narrow surface |
| `c6-runner` | Authenticated Unix-socket protocol and simulated lifecycle journal | Control-plane policy, Docker today, C6 data root | Dogfood simulation; disconnected |
| `c6-cloud` | Hosted-account preview, namespaces, installation registration, catalog projection, relay coordination | Local roles, repositories, runtime, secrets | Dogfood, loopback only |
| `c6-cloud-core` | Cloud DTO validation and relay frame state machine | I/O, persistence, credential storage | Implemented |
| `c6-connector` | Outbound Cloud session, catalog publication, bounded relay to one loopback origin | Arbitrary forwarding, local authorization, persistent database | Dogfood |
| `web` | C6 Hub and Admin browser surfaces | Authorization decisions | Implemented |
| `cloud-web` | Cloud claim, directory, registration, binding, and status UI | Local C6 session or policy | Dogfood |

## Cohesion rules

- Shared crates carry validation and wire types, not mutable global state.
- HTTP handlers authenticate and authorize before calling persistence or Git.
- Slugs are presentation identifiers; UUIDs select local records and disk paths.
- The CLI and future MCP use public typed APIs. They do not read SQLite or Git
  directories directly.
- The connector accepts one configured loopback upstream. It is not a generic
  reverse proxy and does not accept an upstream from relay traffic.
- Runner adapters receive a server-resolved, immutable run plan. Adapter types
  must not leak into public control-plane APIs.

## Process boundaries

The local server, runner, Cloud, and connector are separate processes because
their privileges differ:

```text
c6-server       owns local control data and repository policy
c6-runner       will own narrowly scoped workload execution
c6-cloud        owns preview directory and relay coordination; target is global
c6-connector    holds one Cloud credential and one catalog-read API credential
```

Collapsing these processes would broaden compromise impact. In particular,
the server must not inherit runtime-engine authority, and Cloud must not obtain
local database, Git-root, or secret-provider access.

## Web surface boundary

C6 Hub and C6 Admin are views over the same installation, cookie, API,
database, and release. Admin routes render only when the session reports the
explicit `serverAdministrator` capability. Workspace `owner` is not a synonym
for server administrator.

Cresix Cloud is a separate origin and session authority. Production project
navigation must deliberately cross from the account directory origin to an
isolated installation relay origin. Future untrusted project applications need
a separate registrable domain such as `*.cresixusercontent.com`.

## Dependency direction

Domain and protocol libraries may be consumed by binaries. Binaries must not
become shared libraries for one another. The intended direction is:

```text
c6-cli -> c6-client -> c6-core
c6-server -> c6-core + c6-git + c6-scheduler
c6-cloud -> c6-cloud-core
c6-connector -> c6-cloud-core + c6-core
c6-runner -> c6-core
```

Future C6R, event, secret-provider, and runtime-adapter modules should preserve
this direction through narrow interfaces rather than a generic plugin bus.
