# Product model

## The product in one picture

```text
              optional Cloud layer (preview-local today)
  account handle -> workspace -> project directory -> installation route
                                             ^
                                             | outbound connector
                                             |
  browser / c6 CLI / Git -----------> one sovereign C6 installation
                                      | Hub: collaboration
                                      | Admin: operations
                                      | API: one authority
                                      +-> SQLite metadata
                                      +-> bare Git repositories
                                      +-> bounded runner protocol (simulation)

  future C6R composition -----------> pinned content in C6 or larger Git repos
```

## C6 installation

An installation is one authoritative C6 server, SQLite control store, and bare
Git root. It owns local peers, sessions, roles, projects, pull-request records,
schedules, run/deployment intent, audit history, and canonical repository refs.
It is the unit of operation, backup, migration, and failure.

An installation can run without Cresix Cloud. It can be reached remotely through
any operator-chosen trusted HTTPS ingress.

## C6 Hub

Hub is the workspace and project collaboration experience. It presents project
discovery, repository views, pull-request records, and recorded runtime intent.
It is a view onto the one C6 authority, not a separate service or database.

**Now:** Hub exists on the local C6 server. Workspace member management and
actual pull-request merge execution remain incomplete.

## C6 Admin

Admin is the installation-operations experience. It exposes server state,
global invitations and peer records, access control, and the audit boundary.
Its global operations require the explicit `serverAdministrator` session
capability; owning a workspace does not grant it.

**Now:** bootstrap creates the immutable administrator. There is no recovery or
administrator transfer, so losing the last administrator session can lock out
global administration.

## C6 HTTP API

The API is the authority shared by browser, CLI, Git authorization, and future
agent surfaces. Interfaces may differ in credential type and presentation, but
must reach the same live authorization decisions and durable state.

## `c6` CLI and standard Git

The CLI owns local server aliases and scoped credentials. It can authenticate,
inspect status, list projects, clone, configure a remote, and diagnose common
problems. It is not a resident daemon or privileged side door.

Git clients use a separate expiring, revocable, read-only credential for smart
HTTP clone/fetch/pull. Browser cookies, CLI Bearer tokens, and Git Basic tokens
are intentionally not interchangeable.

## Runner and scheduler boundary

C6 records run, deployment, and schedule intent. A separate authenticated
runner process validates a bounded Unix-socket protocol and simulates lifecycle,
logs, and cancellation. The control plane does not dispatch to it, and no host
command or container workload executes in the current release.

Docker is the intended first runtime adapter, not the current implementation
and not sufficient alone for hostile multi-tenancy.

## Cresix Cloud

Cloud is the intended optional authority for account subjects, globally unique
account handles, and account-scoped workspace slugs, plus installation
registrations, workspace bindings, bounded catalog projections, and relay
presence. The current preview has no public account handle and instead enforces
one globally unique workspace namespace inside one Cloud database. Cloud does
not own local source, roles, sessions, runtime state, or secret values.

The target friendly directory shape is
`cresix.com/@{account}/{workspace}/{project}`. Production local C6 traffic is
intended to move to a distinct, opaque per-installation
relay origin so unrelated C6 cookie authorities never share the account origin.

**Now, dogfood:** a loopback-only Cloud service and connector implement account
bootstrap, workspace/installation registration, binding, catalog publication,
revocation, and bounded serial reverse HTTP against a compatible backend. The
temporary directory route is `/{workspace}/{project}`. Account handles, the
three-segment target route, its migration/redirect policy, the production
account flow, and the isolated-origin browser journey are not shipped.

## Connector

The connector is a least-privilege, outbound-only process. One connector serves
one installation and proxies only one configured loopback HTTP origin. It is
not a generic forward proxy, local authorization provider, or repository sync.

## C6R

A C6R is the proposed reusable composition unit for a coherent small capability
stored in Git. It may be an entire repository or a safe subdirectory closure,
with a strict manifest, immutable source revision, content digest, declared
inputs, exports, and requirements.

**Now:** the format and threat model are specified only. There is no parser,
resolver, lockfile implementation, or materializer in this revision.

## Authority invariants

- One installation is authoritative for one local project's mutable control
  plane and canonical Git remote.
- Cloud directory metadata cannot authorize a local C6 request.
- Network location and IP address never establish identity.
- Git owns source; SQLite owns control-plane metadata.
- Recorded intent never implies execution.
- Interfaces and adapters cannot bypass server-side authorization.
- Disconnecting Cloud leaves standalone data and local access intact.

See the [architecture handbook](../architecture/README.md) for process,
persistence, and consistency details.
