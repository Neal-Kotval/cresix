# Product

## Promise

C6 aims to make small, purpose-built software as easy to keep, inspect, and
share as a document. The repository, collaboration context, declared runtime,
and operational history should live together on infrastructure the operator
controls.

The current release is the collaboration/control-plane foundation. It persists
peers, workspaces, project metadata, local Git repositories, pull-request
records, schedules, runs, and deployments. It does not host applications,
accept Git pushes, execute jobs, store secret values, or provide recoverable
login.

## People

- **Solo builder:** keeps small tools on a laptop and wants understandable local
  state rather than a cloud account and many services.
- **Small trusted team:** shares source and operational intent through one
  owner-operated server.
- **Operator:** controls reachability, TLS termination, backups, upgrades, and
  the bootstrap administrator session.
- **Contributor:** reads or proposes changes within a workspace role but does
  not administer the server.

The current security boundary assumes these people trust one another not to run
hostile code. C6 still protects against accidents, stale authorization, token
replay, cross-site mutation, path traversal, and unsafe runner protocol input.

## Primary use cases

1. Start one C6 server locally and claim it without a hosted identity provider.
2. Invite a remote collaborator through a short-lived, single-use link.
3. Create a project with a seeded local Git repository and inspect its source.
4. Record pull requests, schedules, deployment intents, and run intents against
   real revisions.
5. Back up or move the complete installation as a small number of local data
   volumes.
6. Validate `c6.toml` declarations and test the runner/scheduler contracts
   without pretending workloads executed.

## Principles

- **Self-hosted authority:** the installation owns identity and authorization.
- **Remote-first, not proximity-based:** network location is never identity.
- **Git is source truth:** commits and trees stay in Git, not duplicated in SQL.
- **Recorded intent is not execution:** UI and APIs expose unsupported runtime
  states explicitly.
- **YAGNI infrastructure:** SQLite and local files before external databases,
  registries, orchestration, federation, or plugins.
- **Fail closed at boundaries:** stale roles, bad origins, unsafe paths,
  malformed schedules, and unauthenticated runner frames are rejected.
- **Local QA:** extensive reproducible gates without requiring hosted CI.

## Non-goals for this release

- Host applications or arbitrary TCP services
- Execute commands, containers, cron jobs, or agents
- Serve Git smart HTTP/SSH or accept network pushes
- Store or inject secret values
- Provide password, passkey, SSH-key, OAuth, OIDC, or recovery authentication
- Support anonymous/public projects, hostile multi-tenancy, HA, or federation
- Replace GitHub issues, project boards, CI checks, or merge queues

These boundaries keep the current product honest and define the next useful
vertical slices without implying they already exist.
