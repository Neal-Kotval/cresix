# Capability ledger

This page is the shortest reliable answer to “what does Cresix support?” A
checked engineering roadmap item is summarized here only with its user-visible
qualification. Links point to the detailed evidence and boundaries.

## Local forge and identity

| Capability | Status | Product truth |
| --- | --- | --- |
| Start a local installation | **Now** | Source and Compose paths support a loopback-first, single-node C6 server. |
| Claim the first administrator | **Now** | One-time bootstrap creates the immutable server administrator and a browser session. There is no recovery or transfer. |
| Invite a peer | **Now** | The administrator can issue a short-lived, single-use invitation. Device keys are metadata, not proof-of-possession login. |
| Browser sessions and revocation | **Now** | Hashed cookie sessions, exact-origin CSRF, sliding expiry, peer/device/session controls. No password/passkey re-login. |
| Workspaces and project roles | **Now** | Durable role-checked workspaces and projects exist. Workspace membership management is not a complete Hub workflow. |
| Backup and restore | **Next** | The data boundary is declared; verified product-level backup/restore remains unchecked. |

Evidence: [trust model](../TRUST_MODEL.md), [authorization](../AUTHORIZATION.md),
[operations](../OPERATIONS.md), and [architecture](../architecture/README.md).

## Git and collaboration

| Capability | Status | Product truth |
| --- | --- | --- |
| Create local Git repositories | **Now** | A project receives a server-owned bare repository addressed internally by UUID. |
| Browse repository content | **Now** | Hub provides a bounded repository browser. |
| Clone, fetch, and pull | **Now** | Opt-in authenticated smart HTTP supports read-only `upload-pack` with a separate expiring Git credential. |
| `c6` CLI | **Now** | Aliases, login/status/logout, project list, clone, remote setup, diagnostics, and JSON output are implemented. |
| Pull-request records | **Now** | Proposal metadata is persisted; server-side merge execution is not implemented. |
| Git push and protected refs | **Next** | `receive-pack`, atomic ref policy, reconciliation, branch protection, and push audit are designed but absent. |
| Forks, checks, and merge queues | **Later** | Not part of the current product. |

Evidence: [Git manual](../GIT.md), [CLI manual](../CLI.md), and
[Phase 2 specification](../specs/PHASE_2_GIT_AND_CLI.md).

## Runtime, hosting, schedules, and secrets

| Capability | Status | Product truth |
| --- | --- | --- |
| Validate `c6.toml` | **Now** | Versioned project manifests and declared services/jobs/policy can be validated. |
| Record run/deployment intent | **Now** | Records are revision-pinned and explicitly report that dispatch is unavailable. |
| Record cron schedules | **Now** | Five-field cron and IANA timezones are validated and stored. No scheduler daemon dispatches them. |
| Runner protocol simulation | **Now** | A separate authenticated Unix-socket runner simulates lifecycle/log/cancellation. It executes no host command or container. |
| Host applications | **Next** | A denied-by-default Docker adapter, application gateway, health gating, and rollback model are required first. |
| Execute jobs and schedules | **Next** | Durable dispatch, leases, recovery, logs, cancellation, concurrency, and approvals remain to be built. |
| Store and inject secrets | **Next** | Names/metadata may be declared, but value storage returns not implemented. Encryption, grants, tmpfs injection, and redaction are required. |
| Hostile multi-tenant workloads | **Later** | Containers alone are insufficient; stronger isolation and operations would be required. |

Evidence: [runner](../RUNNER.md), [scheduler](../SCHEDULER.md), and
[agent-first runtime specification](../specs/AGENT_FIRST_RUNTIME.md).

## Agents and automation

| Capability | Status | Product truth |
| --- | --- | --- |
| Typed JSON reads through CLI/API | **Now** | The CLI has JSON output and uses scoped authenticated API operations. |
| Agent configuration in Git | **Now, as source** | Repositories can store and version arbitrary agent/team definitions; C6 does not activate them as runtimes. |
| Capability discovery, cursors, and event polling | **Next** | Stable capability documents, ETags, idempotency keys, cursor pages, and bounded long polling are specified, not implemented. |
| Agent proposal branches | **Next** | Planned only after execution and repository proposal grants are safe. |
| MCP adapter | **Later** | Intended as a thin projection of authenticated HTTP, not a database side door. |

## Cresix Cloud connected mode

| Capability | Status | Product truth |
| --- | --- | --- |
| Claim Cloud preview account | **Now, dogfood** | One-time account bootstrap works only while the preview binds to loopback. It is not production login. |
| Create Cloud workspace namespace | **Now, dogfood** | Uniqueness is enforced inside one running preview service/database only; it is not a production-global namespace guarantee. |
| Register and revoke installation | **Now, dogfood** | One-time hashed connector credential, registration, and fail-closed revocation are implemented. Revocation is irreversible in the preview; reissue, re-registration, and rebinding are absent. |
| Bind workspace and publish catalog | **Now, dogfood** | Bounded project discovery metadata is eventually consistent and never local authorization truth. |
| Outbound connector and reverse HTTP | **Now, dogfood** | Serial bounded transport is live-tested through the real connector against an authenticated C6-compatible backend. |
| Account-scoped public directory URL | **Next** | Target is `cresix.com/@{account}/{workspace}/{project}`. The preview has no public account handle and temporarily uses `/{workspace}/{project}`. |
| Public `cresix.com` accounts | **Next** | Enrollment, passkey/OIDC login, recovery, throttling, and abuse operations are absent. |
| Isolated relay-origin browser journey | **Next** | A real browser session through a per-installation origin into real C6, including reconnect and fallback, remains unverified and unshipped. |
| Multi-node relay and production operations | **Later** | HA presence, incident controls, custom domains, billing, and federation are deferred. |

Evidence: [connected-mode specification](../specs/CRESIX_CLOUD_CONNECTED_MODE.md)
and [Cloud ADR](../decisions/0002-optional-cresix-cloud-directory-and-relay.md).

## C6R composition

| Capability | Status | Product truth |
| --- | --- | --- |
| C6R product, format, and security design | **Now, design** | The decision test, manifest, lock, provenance, lifecycle, and staged threat model are accepted documentation. |
| Parse, resolve, lock, and verify C6Rs | **Next** | No `c6r.toml` parser, `c6r.lock` tool, resolver, or digest implementation exists. |
| Materialize passive content and agent teams | **Next** | Intended first implementation; materialization must not execute hooks or commands. |
| Activate MCPs, apps, services, jobs, or workflows | **Later** | Blocked on runtime, secrets, network, provenance, permissions, and audit controls. |

Evidence: [C6R composition specification](../specs/C6R_COMPOSITION.md).
