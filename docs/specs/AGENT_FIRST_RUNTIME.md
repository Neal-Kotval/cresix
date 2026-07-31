# Agent-first runtime, schedules, secrets, and polling

Status: accepted design; implementation not started beyond listed foundations
Owners: C6 maintainers
Last updated: 2026-07-31
Capability status: [Agents and automation](../product/CAPABILITIES.md#agents-and-automation)

## 1. Outcome

C6 should make a small piece of software easy for a human or agent to create,
inspect, run, schedule, host, and collaborate on without turning the agent into
an installation administrator. Web, CLI, HTTP polling, and a future MCP server
are clients of one C6 authority. They share typed capabilities, live role
checks, immutable revision selection, audit, and the same deny-by-default
runtime policy.

This document defines the intended seams. It is not a claim that C6 executes or
hosts workloads today.

## 2. Current truth

The present revision provides only foundations:

- `c6.toml` models command, cron, and agent jobs plus resource, network,
  repository-write, and secret-name intent.
- The server validates manifests and records run, deployment, and schedule
  intent with `dispatchAvailable: false`.
- `c6-scheduler` is a deterministic library used to validate cron expressions
  and compute a next occurrence. No tick loop or dispatcher is wired.
- `c6-runner` is an authenticated Unix-socket daemon with a simulation backend.
  The control plane does not call it.
- No command or container is spawned. Docker is absent from the runner and the
  server has no Docker socket.
- Secret metadata can be recorded, but secret value writes return `501` and no
  values are stored or injected.
- The CLI exposes JSON for current discovery and Git workflows. There is no
  run/deploy/schedule CLI, event cursor, long poll, webhook, or MCP server.

Anything below is deferred until its milestone has implementation and negative
security tests.

## 3. Design rules

1. **One authority:** only the C6 server resolves identity, roles, revisions,
   schedules, grants, approvals, and durable state.
2. **Immutable execution:** a run selects a full Git object ID, manifest digest,
   resolved configuration digest, runtime adapter version, and secret grants
   before dispatch.
3. **Typed capabilities:** clients discover explicit capabilities and render
   unavailable behavior; they never infer execution from a manifest field.
4. **Retry-safe automation:** mutations carry an idempotency key; reads use
   versions/cursors. Network retry must not create a second authoritative run.
5. **Least privilege:** runtime access starts empty. Files, CPU/memory/time,
   egress, repository writes, and secrets require named bounded grants.
6. **No ambient agent identity:** a workload never inherits the server owner’s
   browser, CLI, Git, Codex, cloud, Docker, or host credentials.
7. **Ingress is reachability:** ngrok or any other tunnel terminates/routes
   traffic only. It never establishes C6 identity or runtime authority.

## 4. Capability and API shape

The server should publish a versioned capability document for each caller and
resource. A capability is descriptive, not a bearer grant; authorization is
re-evaluated when the operation occurs.

```json
{
  "revision": "capability-document-etag",
  "capabilities": {
    "runs.create": false,
    "runs.cancel": false,
    "events.longPoll": false,
    "runtime.adapters": [],
    "secrets.providers": []
  }
}
```

Machine clients use the same DTOs and error codes as the web app. The CLI adds
commands only after the underlying API is stable and supports `--json` on every
command. Human text is a projection of typed output, not the protocol.

Resource reads should return strong `ETag` values. Conditional mutation uses
`If-Match` so an agent cannot silently overwrite a newer schedule or approval.
`412 Precondition Failed` means refresh and make a new decision; clients must
not loop blindly.

## 5. Events and polling

Agents need an efficient wait primitive, not rapid list polling. The authority
should expose an append-only, installation-scoped event sequence filtered to
the caller's current visibility:

```text
GET /api/v1/events?cursor=<opaque>&limit=<bounded>
GET /api/v1/events?cursor=<opaque>&wait=25s&limit=<bounded>
```

- A cursor is opaque, monotonic for one installation, and contains no secret.
- Responses contain `nextCursor`, bounded events, and an explicit heartbeat
  when a long poll times out unchanged.
- The maximum wait and page size are server-capped. Disconnect cancels the
  waiter without cancelling any run.
- Visibility is checked when events are returned. A membership downgrade must
  not leak later events or allow an old cursor to recover hidden payloads.
- Retention expiry returns a typed `cursor_expired` response with a safe
  resynchronization point; it never silently skips history.
- Event payloads use stable IDs and state summaries, not logs, secret values,
  credentials, or arbitrary environment data.
- Webhooks and streaming transports are separate later adapters over the same
  durable sequence; clients may always fall back to bounded polling.

Initial event kinds should be narrowly versioned: run state, schedule change,
occurrence decision, deployment state, proposal created, approval requested,
and credential/grant revocation. Audit events remain a distinct privileged
record even when one action produces both.

## 6. Schedules and occurrences

A schedule points to immutable execution inputs:

```text
schedule ID
job name
full revision SHA
manifest digest
resolved configuration digest
cron + IANA timezone
concurrency policy
missed-occurrence policy
approval policy
```

Editing creates a new schedule version. Existing occurrences retain their
original version and digests. The scheduler persists a checkpoint and claims
an occurrence atomically using `(schedule_id, scheduled_at_utc)` as the durable
idempotency key.

The initial production policy is intentionally small:

- missed occurrences: `skip`;
- automatic retries: none;
- concurrency: `forbid`;
- one active lease per occurrence with bounded expiry and explicit renewal;
- an expired lease yields `interrupted`/unknown outcome and is not
  automatically rerun;
- cancellation is a requested transition until the runtime confirms terminal
  state;
- clock regression fails the tick rather than duplicating work.

Approval can be `none` or `required`. A required occurrence remains pending
without a runtime lease until an authorized human approves the exact revision,
configuration digest, and grants. Editing any of those inputs invalidates the
approval. Agents may request approval and poll it; they may not self-approve
unless a future explicit role capability says so.

## 7. Runtime adapter boundary

The runner should depend on a narrow `RuntimeAdapter`, not Docker-specific
types in the control-plane API:

```text
prepare(resolved run) -> prepared handle
start(handle) -> runtime ID
inspect(runtime ID) -> observed state
cancel(runtime ID) -> requested/terminal result
cleanup(runtime ID) -> idempotent result
```

The adapter receives only a server-resolved run plan. Unknown fields,
unsupported policy, missing limits, mutable image tags, unverified mounts, and
unavailable enforcement fail before start. Runtime state is reconciled into C6
state; adapter success is not directly exposed as authority.

### Docker first

Docker is the initial intended adapter because it is common on laptops and
small Linux/AWS hosts. It must be deny-by-default:

- pinned image digest and recorded build provenance;
- non-root user, read-only root filesystem, dropped capabilities, no-new-
  privileges, restrictive seccomp, bounded pids/CPU/memory/time, and explicit
  tmpfs/scratch limits;
- no host Docker socket, host network, privileged mode, device pass-through,
  arbitrary bind mount, or access to C6 data/credential directories;
- repository checkout mounted read-only unless a proposal workspace grant was
  resolved; repository mutation is returned as an artifact for server policy;
- no egress unless destinations/protocols are explicitly granted and the host
  can actually enforce them;
- bounded logs with redaction and safe truncation.

Docker alone is insufficient for hostile multi-tenancy. Kernel sharing,
daemon privilege, image supply chain, side channels, network enforcement, and
host-operator compromise remain risks. Public untrusted execution requires a
stronger adapter such as microVMs plus independent hardening evidence.

## 8. Secrets

Secret references in `c6.toml` are names, never values. A resolved run receives
only values granted to its job and revision. Two provider classes are planned:

### Native store

- Encrypt each value with an AEAD scheme using unique nonces and authenticated
  metadata binding installation, secret name, version, and scope.
- Keep the master key in a mode-`0600`, non-symlink file outside
  `C6_DATA_DIR`. It is backed up and restored separately from the data snapshot;
  loss makes ciphertext unrecoverable, while copying the data volume alone
  must not expose values.
- Never accept the master key through a manifest, CLI argument, URL, log, or
  database field. Startup fails closed on unsafe ownership/permissions.
- Version writes; never return plaintext from list/read APIs. Rotation and
  deletion produce audit records without the value.

### Reference adapters

1Password and Doppler adapters should store an opaque provider reference, not a
copied value. Provider authentication is operator configuration and must be
narrowly scoped. Resolution happens at run preparation, uses bounded timeout,
does not follow arbitrary URLs, and records provider/name/version metadata
without credentials or values. Provider outage fails the run closed.

### Injection

The runner delivers resolved values through run-scoped in-memory/tmpfs files,
owned only by the workload UID and removed during cleanup. Values do not enter
argv, image layers, Git, persistent environment/config files, run DTOs, events,
logs, crash reports, or artifacts. Environment injection should be exceptional
because it is commonly exposed by process/debug tooling.

## 9. MCP

A future C6 MCP server is a thin adapter over the authenticated API. It may
offer tools such as project discovery, run creation, state inspection, bounded
event polling, cancellation, and proposal creation only after those HTTP
operations exist. It must:

- use a scoped CLI/service credential and verify the expected server ID during
  explicit connection/authentication checks (stronger pinning needs its own
  transport design);
- expose typed schemas and explicit capability/deferred errors;
- preserve ETag, idempotency, cursor, and approval semantics;
- avoid direct SQLite, Git-root, runner-socket, Docker, or secret-provider
  access;
- never turn model intent into administrator authority or bypass a required
  approval.

MCP is not required for non-agent clients and is not a new control plane.

## 10. Deployment behavior

The same authority/runtime split supports three journeys:

- **Laptop:** suitable for interactive work; sleep/offline means no polls,
  scheduling, or runtime. Missed occurrences remain `skip`.
- **Always-on Linux:** recommended small-team host; supervised services,
  private backend, HTTPS ingress, runtime isolation, disk monitoring, and
  verified backups.
- **AWS/cloud VM:** the same single-node artifact on persistent storage; expose
  only HTTPS ingress and keep the backend/runtime daemon private. Kubernetes,
  RDS, and a hosted C6 control plane are not prerequisites.

ngrok may supply a stable HTTPS tunnel for evaluation, especially on a laptop.
It remains optional and replaceable; C6 credentials and live authorization are
still required for every operation.

## 11. Delivery gates

Ship in vertical slices, each with end-to-end and abuse-case tests:

1. capability document, ETags, idempotency keys, and CLI JSON contracts;
2. durable cursor pages and bounded long polling without execution;
3. immutable schedule versions, occurrence claims, leases, skip/no-retry, and
   approvals driving simulation only;
4. native encrypted secrets and reference-adapter interfaces, with no runtime
   injection yet;
5. Docker `RuntimeAdapter` behind explicit operator enablement and trusted-code
   warning;
6. run-scoped secret injection, egress/resource enforcement, reconciliation,
   cancellation, and recovery drills;
7. hosting gateway and health-gated publish;
8. MCP adapter after the corresponding HTTP/CLI contracts are stable.

No stage may relabel `recorded` intent as queued/running, treat a scheduler
calculation as dispatch, treat simulation as Docker, or treat Docker as a
complete security boundary.

## 12. Related records

- [Architecture](../ARCHITECTURE.md)
- [Runner](../RUNNER.md)
- [Scheduler](../SCHEDULER.md)
- [Threat model](../THREAT_MODEL.md)
- [Single-authority ADR](../decisions/0001-single-authority-self-hosting.md)
- [Phase 2 Git and CLI specification](PHASE_2_GIT_AND_CLI.md)
