# C6 architecture

## Local-first topology

```text
browser
  ├── C6 Hub   /, /projects/*
  └── C6 Admin /admin, /admin/access
                │
                ▼
┌────────────────────── C6 control plane ──────────────────────┐
│ peer sessions │ authorization │ project API │ Git read API  │
│ pull requests │ runtime records │ schedules │ audit events  │
└─────────┬────────────────────────────────────────────────────┘
          ▼
  SQLite + bare Git               C6 runner process
  under C6_DATA_DIR               authenticated Unix socket
                                  simulation journal only
                                  (not wired to control plane)
```

One C6 installation is authoritative for its peers, projects, and execution
records. It is remote-first—peers may connect from anywhere through an
operator-provided HTTPS gateway—but it does not require a central C6 service.
The gateway terminates TLS; C6's loopback/private backend listener remains HTTP.

C6 Hub and C6 Admin are two web surfaces of this one installation. They share
the HTTP server, API namespace, cookie session, authorization checks, SQLite
database, Git root, configuration, backup boundary, and release lifecycle.
Legacy `/settings/*` web routes redirect to their canonical Admin destinations;
they do not establish another API or authority.

A future `c6` CLI may call the same authenticated API for operator workflows.
It is explicitly a thin client—not a resident process, privileged side door,
or second control plane—and has no implemented commands today.

The MVP deliberately uses an embedded SQLite control store and on-disk bare Git
repositories. A single local data directory makes a laptop install portable and
recoverable. PostgreSQL, object storage, a private registry, remote runners,
and multiple control-plane replicas are deferred until real use requires them.

## Trust and authorization

Native peer trust separates proof of possession from authorization:

```text
one-time bootstrap / invitation / cookie session
                          │
                          ▼
                 authenticated peer
                          │
                          ▼
              durable workspace/project role
                          │
                          ▼
                 authorized operation
```

Bearer material is high entropy and stored only as a cryptographic hash. An IP
address or reachable URL is never identity. Every protected operation derives
its actor from server-side authentication, reads current membership, and
records security-sensitive changes in the audit log.

Native v1 has no cryptographic device identity or re-login proof. An invitation
issues a 30-day cookie session; the submitted device label and `publicKey` are
unverified metadata. The bootstrap identity is the immutable server
administrator; workspace ownership does not grant that power. Losing or
revoking its sole session permanently locks global administration in this MVP.
Successful session reads slide expiry forward by 30 days, but there is no
lost-cookie recovery. This limitation is explicit rather than hidden behind an
IP address or a pretend device-key flow.

The authenticated session contract exposes `serverAdministrator` as an
explicit capability. C6 Admin uses that capability to gate installation-wide
operations. It is not inferred from the `owner` workspace role: workspace
owners act in Hub, while only the immutable bootstrap identity is the server
administrator.

## Process boundary

The control plane records authenticated intent. It does not receive the Docker
socket or an ambient capability to execute arbitrary host commands. The runner
protocol accepts size-limited, versioned, authenticated requests over a
permission-restricted Unix socket, but the current control plane does not
dispatch requests to it.

The runner validates request identity and replay protection, writes execution
state to its private state directory, and reports explicit lifecycle events. An
unknown outcome becomes `interrupted`; C6 never retries it automatically. The
current backend simulates bounded execution and never invokes a host command or
container runtime.

Containers are an accident boundary for trusted team code, not a sufficient
boundary for mutually hostile tenants. Public multi-tenant hosting requires a
stronger sandbox such as microVMs.

## Stable concepts

- **Workspace:** local membership and policy boundary.
- **C6 Hub:** workspace/project collaboration surface.
- **C6 Admin:** installation-operations surface gated by session capability.
- **Peer:** installation-local person with one or more revocable credentials.
- **Server administrator:** immutable bootstrap identity that manages trust;
  distinct from a workspace `owner`.
- **Project:** local Git repository plus collaboration and declared runtime metadata.
- **Revision:** immutable, full Git object ID.
- **Pull request:** proposed branch update pinned to reviewed revisions.
- **Deployment:** immutable revision and image promoted to an environment.
- **Job:** named command, cron task, or agent declared by `c6.toml`.
- **Run:** one revision-pinned execution and its terminal outcome.
- **Secret:** write-only value granted by name to selected workloads; value
  storage is deferred until encryption and runner injection are implemented.
- **Runner:** the locally authenticated execution process.

## Persistence and consistency

The default data root is selected through `C6_DATA_DIR`. SQLite metadata, Git
repositories, and server-owned run data live beneath it. Runner journaling uses
a distinct private state directory. Paths are derived from server-created UUIDs
rather than user-supplied slugs.

SQLite runs in WAL mode with foreign keys and a bounded busy timeout. Mutations
that change authorization or multiple records are transactional, and audit
failure aborts the associated security-sensitive mutation. Git remains the
authority for refs, commits, trees, and blobs; C6 does not duplicate them into
SQLite.

Operations that cross SQLite and Git must compensate safely. A repository
created before a failed metadata transaction may remove only that newly created
UUID directory. Ref updates use expected old object IDs so concurrent movement
fails rather than merging or publishing an unreviewed revision.

## Agent writes

An agent receives an ephemeral checkout of a pinned revision. With no repository
grant, its writes remain run artifacts. A future
`repository_write = "proposal"` grant may create an `agent/<job>/<run>` branch
and pull request under a service identity. Direct writes to a protected/default
branch are never an agent capability.

Agent execution remains disabled until C6 can encrypt credentials, grant them
per job, restrict egress, redact logs, and prevent use of the server owner's
ambient Codex credentials.
