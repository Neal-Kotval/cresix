# Threat model

## Scope

This review covers the local C6 control plane, browser session boundary,
separate CLI/Git credentials, opt-in read-only Git smart HTTP, SQLite and Git
data, thin CLI/helper, and standalone simulation runner. It does not certify
public Git exposure, Git push, workload isolation, secret storage, hosting, or
hostile multi-tenancy because those capabilities/hardening are absent.

## Assets

- Bootstrap, invitation, session, CSRF, CLI, Git, and runner protocol credentials
- Peer identities, memberships, and global administrator authority
- Private source and Git history
- Project/runtime metadata and audit records
- Host filesystem paths and runner state

## Boundaries and actors

- Unauthenticated network client → Axum HTTP listener
- Browser JavaScript → HttpOnly session cookie / readable CSRF cookie
- Authenticated peer → workspace role checks
- Workspace owner → immutable server administrator boundary
- User input → SQLite, Git arguments, filesystem paths, cron parser
- Runner client → HMAC-authenticated Unix socket
- Operator/reverse proxy → plaintext C6 backend
- CLI → Bearer API and owner-only plaintext preview credential store
- Git/IDE → Basic Git token → bounded `git http-backend` upload-pack

Actors include an external attacker who can reach the listener, a malicious
invited peer, compromised browser content, accidental operator exposure, and a
local user able to tamper with paths or socket files.

## Principal abuse cases and controls

| Abuse case | Current control | Residual risk |
| --- | --- | --- |
| Guess/replay enrollment token | 256-bit randomness, hashes, expiry, atomic single use | Token disclosure still grants enrollment |
| Token leaked by URL/log | Invitation secret in fragment; bootstrap file not log | User can copy it to an unsafe channel |
| Cross-site mutation | exact Origin, Strict cookies, bound double-submit CSRF | Plaintext transport exposes bearer cookies |
| Stale access after removal | every request checks peer/device/session and membership | No central recovery from administrator loss |
| Workspace → server privilege escalation | immutable administrator checked separately | Administrator session is a single point of failure |
| Path/ref/argument injection | UUID paths, canonicalization, strict refs, argument arrays, cleared Git env | Git binary remains trusted dependency |
| Oversized input/output | HTTP, manifest, Git, runner frame/log limits | Disk exhaustion still needs operator monitoring |
| Runner request forgery/replay | mode-0600 socket/key, HMAC, timestamp, nonce cache, strict schema | Same-UID/key compromise defeats protocol auth |
| Host command/container escape | simulation never executes | No production workload capability exists |
| Public TLS misconfiguration | non-loopback bind requires explicit acknowledgement | C6 listener itself has no TLS |
| CLI/Git credential confusion | versioned token classes, separate parsers/scopes, mixed-cookie rejection | local preview store is plaintext; ordinary requests do not cryptographically pin server identity |
| Git protocol/path abuse | transport disabled by default, exact upload-pack routes, UUID resolution, cleared Git environment, limits/timeouts/header allowlist | no rate limiting; Git binary is trusted; fetch allow/deny lacks durable access audit |

## Deny-by-default decisions

- Read-only Git network transport disabled unless explicitly enabled
- No Git push/receive-pack, SSH, anonymous Git, or dumb HTTP
- No secret values
- No command/container/agent execution
- No project/workspace destructive API
- No anonymous access
- No default non-loopback listener
- No runner dispatch without a fully resolved policy

## Residual blockers before public/company use

Recoverable authentication and admin transfer, rate limiting, production TLS
guidance/tooling, encrypted local client storage, durable Git access audit,
encrypted secrets, real sandbox enforcement, backup restore drills with
populated data, retention controls, and dependency/update policy all remain
necessary. Docker is the intended first runtime adapter, but containers alone
do not establish hostile-tenant isolation. See the
[agent-first runtime design](specs/AGENT_FIRST_RUNTIME.md).
