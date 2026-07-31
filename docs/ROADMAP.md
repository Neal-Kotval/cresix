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

- [x] Bare-repository creation and bounded repository browser
- [x] Separate, expiring, revocable CLI and read-only Git credentials
- [x] Opt-in authenticated upload-pack for standard clone/fetch/pull
- [x] Thin `c6` CLI, typed client, credential helper, canonical remote discovery,
  `--json`, clone, remote setup, and diagnostics
- [ ] Durable read-only Git access audit and production rate limits
- [ ] Protected push/receive-pack with atomic ref policy and reconciliation
- [ ] Pull-request revision pinning and fast-forward-only merge
- [ ] Contributor/default-branch protection and push audit events
- [ ] Forks that copy history but never credentials or runtime state

## Local hosting

- [ ] Docker `RuntimeAdapter` pinned to revision/image digest and denied by default
- [ ] Bounded workload execution without host Docker or C6 filesystem access
- [ ] Authenticated application gateway and health-gated publish
- [ ] Immutable rollback that does not pretend to roll back mutable data
- [ ] Manual jobs, durable logs, cancellation, and interrupted-run recovery
- [ ] Immutable revision/config-digest schedules with occurrences, leases,
  missed=`skip`, no initial retries, concurrency and approvals

## Agents and secrets

- [x] CLI JSON output and typed read capability foundations
- [ ] Capability documents, ETags, idempotent mutation keys, cursor pages, and
  bounded event long polling
- [ ] Native AEAD-encrypted secret values with separately backed-up master key
- [ ] 1Password/Doppler reference adapters and run-scoped tmpfs injection
- [ ] Explicit per-job secret grants and log redaction
- [ ] Pinned agent runtime using a separately granted API key
- [ ] Deny-by-default network policy and resource enforcement
- [ ] Agent-generated proposal branches and pull requests
- [ ] Thin MCP adapter over stable authenticated HTTP operations

## Reusable C6 compositions

- [x] C6R composition, authority, manifest, lock, and threat-model specification
- [ ] Strict `c6r.toml` parser and deterministic safe-closure digest
- [ ] Committed `c6r.lock` with exact Git revision and content digest
- [ ] Passive `content` and `agent_team` materialization without code execution
- [ ] Reviewable update diff and local validation/status commands
- [ ] Active command, MCP, app, service, job, and workflow activation only after
  runtime, secret, network, provenance, and audit gates exist

See the [C6R composition specification](specs/C6R_COMPOSITION.md). C6Rs do not
replace language package managers, local policy, or the C6 sandbox boundary.

## Earned extensions

Only add these after a concrete deployment requires them. The accepted Cloud
dogfood slice pulls a narrow directory/relay experiment forward without
centralizing local authority:

### Optional Cresix Cloud

- [x] Connected-mode specification and authority/origin decision
- [x] Loopback-only Cloud account bootstrap and host-only session
- [x] Global workspace namespace and installation registration
- [x] One-time, hashed, independently revocable connector credential
- [x] Bounded project catalog projection and directory doorway
- [x] Outbound connector component with fixed local origin, bounded protocol,
  catalog publication, and reconnect behavior
- [x] Release-verified serial reverse HTTP transport with offline/revoke and
  negative header/cookie security cases against a C6-compatible backend
- [ ] Isolated per-installation relay origins with a real C6 browser session,
  reconnect, concurrency/overload, and standalone fallback journey
- [ ] Production account login/recovery, throttling, abuse controls, relay
  isolation, HA presence, and incident operations

Checked items have working components and targeted local coverage. The
remaining browser-origin journey stays unchecked until a real browser request
crosses an isolated relay origin into local C6, then proves reconnect and
standalone fallback. A Cloud account must not become local C6 SSO, and
standalone startup must not acquire a Cloud dependency.

### Later, demand-driven extensions

- Durable re-authentication and owner recovery
- GitHub OAuth, company OIDC, SCIM, or group synchronization
- Remote runners and stronger microVM isolation
- Workload PostgreSQL, object buckets, or an external OCI registry
- Multi-server federation, high availability, or Kubernetes
- Dynamic plugins, CI checks, or merge queues

## Client surfaces

- C6 Hub remains the workspace/project collaboration interface.
- C6 Admin grows only around real installation operations supported by the
  single server; recorded or illustrative state stays labelled.
- The implemented `c6` CLI remains a thin client: it owns aliases and preview
  credentials, not control-plane state, and cannot bypass API authorization.
- Pollable HTTP and a future MCP adapter must remain projections of the one
  authority. See the
  [agent-first runtime specification](specs/AGENT_FIRST_RUNTIME.md).
