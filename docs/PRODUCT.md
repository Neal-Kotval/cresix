# Product

## Promise

C6 is an agent-centric, self-hostable software forge for small, purpose-built
software. The repository, collaboration context, declared runtime, hosting
intent, and operational history should live together on infrastructure the
operator controls. Humans and agents should act through the same narrow,
auditable interfaces: web for oversight, CLI for composition, and eventually
pollable HTTP/MCP surfaces for automation.

The current release is the collaboration/control-plane foundation. It persists
peers, workspaces, project metadata, local Git repositories, pull-request
records, schedules, runs, and deployments. It issues separate CLI/Git
credentials, supports a thin CLI, and can serve opt-in authenticated read-only
Git smart HTTP. It does not host applications, accept Git pushes, execute jobs,
store secret values, or provide recoverable login.

## Standalone and connected modes

C6 has one local product authority and two deployment choices:

- **Standalone C6** requires no Cresix account or Cresix-operated service. The
  operator provides a reachable HTTPS origin when remote access is needed.
- **Connected C6** keeps that local authority and adds a Cresix Cloud account,
  globally unique workspace namespace, directory listing, installation
  registration, and outbound managed relay.

Connected mode does not upload Git repositories, runtime records, local
sessions, local roles, or secret values into Cresix Cloud. A Cloud account is
not a local C6 login, and Cloud workspace ownership does not confer local
server-administrator authority. Disconnecting or revoking the connector stops
managed reachability but does not delete or disable the standalone installation.

The intended sharing doorway is `https://cresix.com/{workspace}/{project}`.
Actual C6 traffic crosses an explicit origin boundary to an opaque,
per-installation relay origin. This avoids putting several independent C6
cookie and CSRF authorities under one browser origin. See the
[connected-mode specification](specs/CRESIX_CLOUD_CONNECTED_MODE.md).

The current dogfood implementation includes the separate Cloud account and
directory service, typed relay contracts, and an outbound connector with a
fixed loopback upstream. Component tests cover those boundaries. A complete
browser request through the Cloud relay into a live C6 installation is not yet
release-verified and must not be described as working public hosting.

## Two web surfaces, one C6

C6 presents two product surfaces over the same server, session, API, and data
authority:

- **C6 Hub** is where peers collaborate inside workspaces and projects: project
  discovery, repository views, pull-request records, and recorded runtime
  intent. Workspace member management remains deferred.
- **C6 Admin** is where the installation administrator operates the local C6
  installation: server state, global access invitations and peer records, audit,
  and the boundary around future storage and runner operations. Personal device
  and session endpoints remain self-service rather than global Admin powers.

This is an information-architecture split, not two control planes or two
deployments. Hub and Admin must never disagree about identity, membership,
project state, or audit history because one C6 server remains authoritative for
all of them.

The implemented `c6` CLI follows the same rule. It is a thin authenticated API
client and Git launcher, not a daemon, a second data authority, or a way to
bypass server-side authorization. Its current commands cover server aliases,
login/status/logout, project listing, clone, remote setup, and diagnostics.

## People

- **Solo builder:** keeps small tools on a laptop and wants understandable local
  state rather than a cloud account and many services.
- **Small trusted team:** shares source and operational intent through one
  owner-operated server.
- **Operator:** controls reachability, TLS termination, backups, upgrades, and
  the bootstrap administrator session through C6 Admin and host tooling.
- **Contributor:** reads or proposes changes within a workspace role but does
  not administer the server; their primary surface is C6 Hub.
- **Cloud account holder:** reserves a global namespace and registers a local
  installation for discovery/reachability. This identity remains distinct from
  every installation-local peer in the initial connected design.

The current security boundary assumes these people trust one another not to run
hostile code. C6 still protects against accidents, stale authorization, token
replay, cross-site mutation, path traversal, and unsafe runner protocol input.

## Primary use cases

1. Start one C6 server locally and claim it without a hosted identity provider.
2. Invite a remote collaborator through a short-lived, single-use link.
3. Create a project with a seeded local Git repository, clone it, and fetch it
   with standard Git using a separate read-only credential.
4. Record pull requests, schedules, deployment intents, and run intents against
   real revisions.
5. Back up or move the complete installation as a small number of local data
   volumes.
6. Validate `c6.toml` declarations and test the runner/scheduler contracts
   without pretending workloads executed.

Hub and Admin organize these use cases without widening the implemented
capability set: recorded run/deployment metadata remains non-executing, and
Admin does not automate TLS, backups, hosting, or runner dispatch.

## Principles

- **Self-hosted authority:** the installation owns identity and authorization.
- **Optional centralization:** Cloud owns only global account, namespace,
  directory, and route state; connected mode is never required for local use.
- **Remote-first, not proximity-based:** network location is never identity.
- **Git is source truth:** commits and trees stay in Git, not duplicated in SQL.
- **Recorded intent is not execution:** UI and APIs expose unsupported runtime
  states explicitly.
- **YAGNI infrastructure:** SQLite and local files before external databases,
  registries, orchestration, federation, or plugins.
- **Fail closed at boundaries:** stale roles, bad origins, unsafe paths,
  malformed schedules, and unauthenticated runner frames are rejected.
- **Local QA:** extensive reproducible gates without requiring hosted CI.
- **Agent legibility:** JSON, explicit state, stable IDs, and future cursors are
  preferred over scraping screens or granting ambient host access.
- **Reviewable reuse:** future C6Rs pin reusable content and agent-team
  compositions to immutable Git source and digests; resolution never runs code.

## Non-goals for this release

- Host applications or arbitrary TCP services
- Execute commands, containers, cron jobs, or agents
- Accept Git push or serve SSH, anonymous Git, LFS, or dumb HTTP
- Store or inject secret values
- Provide password, passkey, SSH-key, OAuth, OIDC, or recovery authentication
- Support anonymous/public projects, hostile multi-tenancy, HA, or federation
- Replace GitHub issues, project boards, CI checks, or merge queues
- Treat a Cloud session as local C6 SSO or the relay as end-to-end encrypted

These boundaries keep the current product honest and define the next useful
vertical slices without implying they already exist.
