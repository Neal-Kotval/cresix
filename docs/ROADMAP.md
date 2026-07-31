# C6 roadmap

This roadmap records product boundaries, not release promises. Completed items
must have working implementation and regression coverage; UI-only or schema-only
shapes remain unchecked.

## Local peer foundation

- [x] Rust workspace and versioned project manifest contract
- [x] Cumulative project-role policy with unit tests
- [x] GitHub-like project interface
- [x] Separate, authenticated runner process protocol with simulation backend
- [x] One-command, loopback-only Compose topology
- [x] Embedded SQLite control store and startup migrations
- [x] One-time owner claim, peer invitations, device sessions, and revocation
- [x] Durable projects and server-side authorization on control-plane operations
- [x] Explicit `serverAdministrator` session capability that workspace roles cannot grant
- [x] C6 Hub/Admin navigation and capability gate with legacy `/settings/*` compatibility
- [ ] Local backup/restore verification

## Git collaboration

- Bare-repository creation and repository browser
- Authenticated Git smart HTTP with revocable personal tokens
- Branches, commits, trees, blobs, and bounded diffs
- Pull requests with revision pinning and fast-forward-only merge
- Contributor/default-branch protection and audit events
- Forks that copy history but never credentials or runtime state

## Local hosting

- Dockerfile build pinned to a commit and resulting image digest
- Bounded workload execution without host Docker or filesystem access
- Authenticated application gateway and health-gated publish
- Immutable rollback that does not pretend to roll back mutable data
- Manual jobs, durable logs, cancellation, and interrupted-run recovery
- Five-field cron schedules with IANA timezones and duplicate prevention

## Agents and secrets

- Encrypted, write-only per-install secret storage
- Explicit per-job secret grants and log redaction
- Pinned Codex CLI runtime using a separately granted API key
- Deny-by-default network policy and resource enforcement
- Agent-generated proposal branches and pull requests

## Earned extensions

Only add these after a concrete deployment requires them:

- Durable re-authentication and owner recovery
- GitHub OAuth, company OIDC, SCIM, or group synchronization
- Operator-provided relay integration or a hosted C6 relay
- Remote runners and stronger microVM isolation
- Workload PostgreSQL, object buckets, or an external OCI registry
- Multi-server federation, high availability, or Kubernetes
- Dynamic plugins, CI checks, or merge queues

## Client surfaces

- C6 Hub remains the workspace/project collaboration interface.
- C6 Admin grows only around real installation operations supported by the
  single server; recorded or illustrative state stays labelled.
- A future `c6` CLI may provide thin API-client workflows for operators. It must
  not become a second control plane, own separate state, or bypass API
  authorization. No CLI command is implemented yet.
