# Architecture roadmap

This roadmap orders architectural risk. It does not promise dates. A stage is
complete only when implementation, negative tests, operations, documentation,
and truthful capability reporting agree.

## Current baseline

**Implemented local foundation**

- single sovereign installation with SQLite and bare Git;
- browser enrollment/session/CSRF and live local authorization;
- Hub/Admin, typed JSON APIs, CLI, and optional read-only Git smart HTTP;
- runtime, schedule, deployment, and secret-name records that explicitly do
  not dispatch;
- runner protocol with simulation only.

**Dogfood connected foundation**

- loopback Cloud claim/session, workspace, installation, binding, catalog,
  directory, revocation, and in-memory presence;
- outbound connector and serial bounded relay tested against an authenticated
  C6-compatible backend;
- no public account service, isolated relay-origin browser journey, concurrent
  relay, or production Cloud operations.

## Stage 1: harden the local authority

- recoverable/transferable administration and stronger login proof;
- request and enrollment rate limits;
- verified backup, restore, upgrade, and migration drills;
- operational health, disk-exhaustion handling, security response, and
  credential rotation;
- complete safe Git collaboration primitives before broadening transport.

This stage makes one installation credible for a small company without
introducing distributed authority.

## Stage 2: typed automation substrate

- caller/resource capability documents;
- strong versions/ETags and retry-safe idempotency keys;
- durable bounded event cursors and long polling;
- immutable schedule versions, occurrence claims, leases, and approvals;
- simulation-only end-to-end dispatch before real execution.

## Stage 3: C6R passive composition

- strict manifests and locks, safe Git acquisition and deterministic packing;
- direct dependency resolution and reviewable permission/provenance diffs;
- passive `content` and `agent_team` materialization;
- self-hostable immutable registry and package directory.

No active export executes in this stage.

## Stage 4: production connected Cloud

- public passkey/OIDC enrollment, recovery, membership, rate limiting, and
  abuse response;
- isolated per-installation relay origins with DNS/TLS and a complete real-C6
  browser authentication journey;
- concurrent admission-controlled relay, multi-node presence, HA, telemetry,
  rotation, incident runbooks, and restore drills;
- separate registrable domain for untrusted application content.

Cloud-to-local SSO remains a separate design decision. Production relay does
not require centralizing local repositories or authorization.

## Stage 5: secrets and one trusted runtime

- encrypted native secrets and narrow reference-provider adapters;
- immutable resolved run plans and a wired runner lease protocol;
- Docker adapter with enforceable resource, filesystem, network, provenance,
  log, cancellation, and recovery controls;
- run-scoped secret injection and proposal-only repository writes;
- one active C6R archetype enabled only after its own abuse-case gate.

## Stage 6: agent-native collaboration and hosting

- run/deploy/schedule CLI over stable APIs;
- MCP as a scoped API adapter;
- health-gated service publishing and explicit app domain isolation;
- additional active C6R kinds enabled independently;
- stronger runtime such as microVMs before hostile multi-tenant execution.

## Evidence-triggered, not scheduled

Federation, active-active storage, cross-registry dependency graphs,
transitive C6Rs, remote runners, object storage, and Kubernetes remain
exploratory. Adopt them only when observed deployments cannot be served by
single-authority migration, read-only mirrors, outbound workers, or ordinary
Git distribution. Each material authority change requires a new ADR and
rollback path.

## Cross-cutting gates

Every stage preserves:

- one explicit source and policy authority;
- no credential in source, logs, URLs, argv, or artifacts;
- least privilege and deny-by-default boundary validation;
- stable versioned protocols and typed unavailable states;
- safe cancellation, revocation, timeout, retry, and recovery semantics;
- standalone self-hosting without a mandatory Cresix account;
- documentation that distinguishes design from shipped behavior.

Product sequencing is summarized separately in the
[product roadmap](../product/ROADMAP.md). This
page records only architecture dependency order.
