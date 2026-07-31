# API and protocol boundaries

## Boundary catalog

| Boundary | Transport and credential | Authority | Current state |
| --- | --- | --- | --- |
| Browser to local C6 | JSON/HTML over HTTP(S), cookie + CSRF | Local C6 | Implemented |
| CLI to local C6 | `/api/v1`, scoped Bearer | Local C6 | Implemented, narrow |
| Git client to local C6 | smart HTTP upload-pack, Basic `c6:<token>` | Local C6 + Git refs | Implemented read-only, opt-in |
| Browser to Cloud | separate `/api/v1`, Cloud cookie + CSRF | Cresix Cloud | Dogfood loopback |
| Connector to Cloud | JSON catalog HTTP and `c6-relay-v1` WebSocket | Cloud registration | Dogfood |
| Connector to local C6 | fixed loopback HTTP; catalog-read API credential is separate from relayed client credentials | Local C6 | Dogfood catalog/backend seam |
| Server to runner | authenticated, versioned Unix socket | Local C6 intent | Protocol/simulation implemented; not wired |
| Agent/MCP to C6 | same typed HTTP contracts | Local C6 | Target |
| C6R registry | immutable metadata/artifact API | Registry only for distribution | Target |

## HTTP design rules

The local and Cloud APIs both use `/api/v1`, but they are unrelated origins,
cookies, databases, and principal namespaces. Clients must configure the
authority they expect and must not carry credentials across redirects.

Local JSON endpoints use bounded bodies, structured errors, `no-store`, exact
Origin validation, and server-selected actor identity. Resource routes use
UUIDs. Unknown `/api` paths return JSON `404`, not the SPA. Machine-readable
capability fields describe unavailable behavior; they do not grant it.

Read-only smart HTTP is intentionally outside browser CSRF middleware, accepts
only upload-pack, and is disabled unless the operator opts in. Receive-pack and
push are not routed or advertised.

## Protocol evolution

- Wire schemas are versioned and reject unsupported versions.
- Unknown security-relevant fields fail closed; compatibility must be explicit.
- Identifiers are opaque to clients unless the schema states otherwise.
- Credentials are redacted from URLs, locks, errors, logs, and persisted remote
  configuration.
- Mutations that may be retried need explicit idempotency keys before clients
  automate retries.
- Concurrent edits should use `ETag` and `If-Match`; stale writes return a
  typed precondition failure.
- Long-lived event reads should use bounded pages, opaque durable cursors, and
  capped long polling instead of tight loops.

The last three rules are **target architecture**, not current API behavior.
There is no durable event cursor, generic idempotency layer, webhook, or MCP
server today.

## Runner protocol

The implemented runner accepts size-limited authenticated requests over a
permission-restricted Unix socket and journals simulated lifecycle events. The
server does not dispatch to it. The target adapter protocol takes only a
resolved immutable run plan and exposes prepare, start, inspect, cancel, and
cleanup semantics. Unknown outcome becomes `interrupted`; it is never retried
automatically.

## C6R contracts

The accepted C6R design defines strict versioned `c6r.toml` and lock schemas,
immutable Git/content identities, typed exports, inputs, secret slots, and
requirements. No parser, resolver, registry, or materializer is implemented.
Future CLI and web experiences must project typed results from the same
contracts and distinguish `validated`, `locked`, and `materialized` from
`running` or `deployed`.

See [HTTP API](../API.md), [CLI](../CLI.md), [Git](../GIT.md),
[runner protocol](../RUNNER.md), and the
[agent-first runtime specification](../specs/AGENT_FIRST_RUNTIME.md).
