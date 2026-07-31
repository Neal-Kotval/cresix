# Agent-centric product direction

## Why agent-centric

Agents make small software cheap to create and cheap to change. They also make
ambient authority, invisible state, and unreviewed execution more dangerous.
Cresix should optimize for agent legibility without turning an agent into the
installation owner.

Agent-centric means:

- state has stable IDs and typed JSON representations;
- capabilities are discoverable rather than guessed from a UI;
- long work has pollable, durable outcomes;
- mutations can be retried safely;
- source changes arrive as reviewable Git proposals;
- configuration, team definitions, and policies live in versioned source; and
- every runtime grant is narrow, explicit, and auditable.

It does not mean “put a chat box on every page.”

## One authority, several surfaces

```text
human browser ---- cookie + CSRF -----+
c6 CLI ---------- scoped Bearer ------+--> C6 HTTP authority
Git client ------- read-only Basic ----+
future MCP ------- scoped HTTP adapter-+
future runner ---- leased run grant ---+
```

The API remains authoritative. A future MCP server translates a narrow set of
HTTP operations; it must not query SQLite, mint broader credentials, or bypass
role checks. CLI and MCP should expose the same durable concepts rather than
maintaining their own state.

## Current foundation

**Now:**

- the `c6` CLI supports machine-readable JSON for its current read workflows;
- project, revision, run, deployment, and schedule concepts have stable typed
  representations;
- the server records authenticated run/deployment intent without claiming
  execution;
- a separate runner protocol enforces size, authentication, and replay
  boundaries while using only a simulation backend; and
- Git can version prompts, instructions, schemas, agent definitions, and team
  configurations as ordinary source.

These foundations do not provide a working hosted agent runtime.

## Next agent interface

The next useful API slice should add:

- a versioned capability document so clients can discover supported behavior;
- ETags or revisions for conditional reads and writes;
- client-supplied idempotency keys for retry-safe mutations;
- bounded cursor pagination;
- bounded long polling over durable event cursors;
- explicit terminal states and interrupted/unknown outcomes; and
- short-lived credentials with scopes and resource restrictions.

The goal is reliable polling and orchestration, not a second workflow engine.

## Agent changes to source

An agent should receive an ephemeral checkout pinned to an immutable revision.
With no repository grant, its files remain run artifacts. A future
`repository_write = "proposal"` grant may allow a service identity to create an
`agent/<job>/<run>` branch and pull-request record.

An agent should never receive direct write access to a protected/default branch.
The reviewed revision, generated diff, acting service identity, and run should
remain linked in audit history.

## Agent execution gates

Before C6 executes an agent or arbitrary command, it needs:

1. a denied-by-default runtime adapter pinned to source and image digests;
2. least-privilege filesystem mounts and process capabilities;
3. enforceable CPU, memory, time, and output limits;
4. default-denied network access with explicit destinations;
5. encrypted secret storage or approved reference providers;
6. per-job secret grants and run-scoped injection;
7. log redaction and sensitive-error handling;
8. durable leases, cancellation, interruption recovery, and concurrency rules;
9. approvals for capabilities that cross trust boundaries; and
10. complete audit from declared intent to terminal result.

Docker is a useful first adapter and an accident boundary for trusted code. It
is not, by itself, a hostile multi-tenant security boundary.

## CLI agent crons

The product direction includes scheduled CLI agents, but the safe abstraction
is a revision-pinned C6 job—not “run this user's local shell and Codex token at
cron time.” A schedule should bind to an immutable revision, configuration
digest, runtime requirements, grants, concurrency rule, and missed-run policy.

**Now:** cron declarations can be validated and stored; no daemon dispatches
them. **Next:** occurrences, leases, explicit `missed = skip`, cancellation,
and no automatic retry of unknown outcomes. **Later:** agent-specific runtimes
and credentials after secrets and network controls exist.

## MCP

There are two distinct MCP ideas:

- **C6 as an MCP surface:** a thin adapter lets agents inspect or operate C6
  through the same authenticated HTTP authority.
- **A C6R that contains an MCP server:** a reusable active component that must
  be reviewed, sandboxed, granted inputs/secrets/network, and explicitly
  activated.

Neither exists today. The first is a client adapter; the second is workload
execution. They should not share an implicit trust level.

## Human oversight

Hub should make agent state legible: requested action, source revision, grants,
approval state, logs, artifacts, proposed diff, and terminal outcome. Admin
should expose installation-wide runtime health and revocation, not become a
prompt console with unlimited host access.
