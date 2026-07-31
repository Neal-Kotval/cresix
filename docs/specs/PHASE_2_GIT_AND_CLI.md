# Phase 2: Git-native collaboration and the C6 CLI

Status: partially implemented; read-only Git and the thin CLI are shipped
Owners: `c6-build-team`
Remaining target: protected push, merge, audit, and production hardening
Last updated: 2026-07-31
Capability status: [Git and collaboration](../product/CAPABILITIES.md#git-and-collaboration)

## 1. Executive decision

C6 will be a standard authenticated Git remote. People use `git clone`,
`git fetch`, `git pull`, and `git push`; C6 does not invent another source
transfer protocol. The `c6` command is a thin client for server discovery,
authentication, project lookup, safe remote setup, diagnostics, and later
non-Git C6 workflows.

The canonical remote is:

```text
https://<c6-origin>/git/<workspace-slug>/<project-slug>.git
```

The public contract includes fetch and push, but delivery is deliberately split:

- **Milestone 2.1 — remote foundation:** typed credential lifecycle,
  authenticated read-only Git smart HTTP, `c6` configuration/authentication,
  `c6 clone`, `c6 remote add`, `c6 doctor`, and `git-credential-c6`.
- **Milestone 2.2 — protected push:** receive-pack, atomic ref policy,
  per-repository mutation serialization, durable push audit/reconciliation, and
  normal `git push` to allowed branches.

Milestone 2.1 must not advertise receive-pack. Milestone 2.2 must not ship
partially: accepting objects without atomic ref authorization and recoverable
audit is worse than explicitly denying push.

## 2. Product outcome

After Milestone 2.1, a trusted peer can:

```bash
c6 server add https://c6.example --name work
c6 auth login --server work --token-stdin
c6 project list --server work
c6 clone paper-street/weeknote
cd weeknote
git pull
```

After Milestone 2.2, the same checkout supports:

```bash
git switch -c neal/shorter-summary
git push -u origin neal/shorter-summary
```

There is no separate `c6 push` or `c6 pull` protocol. If convenience aliases are
ever added, they may only delegate to the installed Git executable and must
preserve Git's arguments, output, and exit status. `c6 publish`, `c6 run`, and
`c6 deploy` remain distinct future C6 operations pinned to committed revisions.

## 3. Historical baseline and delivered delta

When this specification was written, C6 had:

- one authoritative Rust server, SQLite store, and Git root;
- bare repositories at `${C6_DATA_DIR}/git/<project-UUID>.git`;
- safe bounded local Git operations through `c6-git`;
- browser cookie sessions with exact-Origin and double-submit CSRF protection;
- durable workspace roles and immediate peer/device/session revocation;
- a same-origin Hub/Admin web application;
- no network Git transport, personal access credentials, CLI crate, durable
  re-login proof, owner recovery, or rate limiting.

The current revision has since delivered separate expiring CLI and read-only
Git credentials, the `c6` CLI/client crates, a credential helper, canonical
remote discovery, and opt-in authenticated smart HTTP `upload-pack` for
clone/fetch/pull. Git push, protected-ref reconciliation, merge execution,
durable read-access audit, owner recovery, and rate limiting remain absent. The
[capability ledger](../product/CAPABILITIES.md#git-and-collaboration), not this
historical snapshot, is the current status authority.

The design uses Git's documented smart HTTP protocol and `git http-backend`
rather than implementing pkt-line negotiation. Git documents that smart HTTP
supports clone/fetch/pull and push, that clients append protocol paths to one
repository URL, and that servers should use standard HTTP authentication rather
than require cookies:

- <https://git-scm.com/docs/git-http-backend>
- <https://git-scm.com/docs/http-protocol>
- <https://git-scm.com/docs/gitcredentials>

## 4. Non-negotiable principles

1. **One authority.** The server owns identity, authorization, project lookup,
   repositories, and audit. The CLI never reads SQLite or C6's Git root.
2. **Git remains Git.** Normal Git clients and IDEs work against one HTTP remote.
3. **Credentials are not interchangeable.** Browser cookies, CLI bearer tokens,
   and Git Basic credentials have separate parsing and policy paths.
4. **Roles stay live.** A token caps access; it never snapshots or grants a role.
   Every request intersects token scope with current user/device state and
   current workspace membership.
5. **Slugs are presentation.** Public workspace/project slugs resolve through
   SQLite to a server UUID. No slug becomes a filesystem path.
6. **Default deny.** Unknown services, refs, scopes, credential types, paths,
   redirect targets, and protocol extensions fail closed.
7. **Secrets do not travel in names.** Credentials never enter remote URLs,
   query parameters, argv, environment variables, Git config, logs, audit,
   crash reports, fixtures, or shell history.
8. **Push is atomic policy.** A multi-ref push succeeds only if every update is
   authorized. One denied update rejects the entire transaction.
9. **Truthful release labels.** Read-only transport is called read-only. C6 does
   not claim production-safe public Git until the public-exposure blockers in
   this specification are closed.

## 5. Architecture

```text
browser
  cookie + CSRF + exact Origin
         │
         ├─────────────── C6 JSON API ───────────────┐
         │                                            │
c6 CLI                                                │
  Bearer token ── c6-client ──────────────────────────┤
                                                      ▼
git / IDE                                    C6 authorization
  Basic c6:<git-token>                              │
         │                                           ├── SQLite identity/membership
         ▼                                           ├── audit/token metadata
/git/{workspace}/{project}.git/*                    └── project UUID resolution
         │
         ▼
bounded smart-HTTP adapter
         │ argv + controlled CGI environment
         ▼
git http-backend ─────────────── `${C6_DATA_DIR}/git/<UUID>.git`
```

The Git router is served by the same C6 origin, but it is outside browser
Origin/CSRF middleware. It has its own authentication, request limits, timeout,
concurrency, response-header allowlist, and audit behavior. CORS remains off.

### 5.1 Crate ownership

- `c6-core`: canonical wire DTOs, scope names, stable errors, and output models.
- `c6-git`: validated repository resolution plus the bounded smart-HTTP child
  adapter and later repository mutation coordinator.
- `c6-server`: token persistence, authentication, live authorization, routing,
  project lookup, rate limiting, audit, and reconciliation.
- `c6-client`: new typed HTTP client; URL validation, auth injection, redirect
  refusal, response limits, and typed errors. No SQLite, Axum, or Git mechanics.
- `c6-cli`: new `c6` and `git-credential-c6` binaries; config/credential storage,
  command UX, and Git delegation.
- `web`: self-service creation/list/revocation of credentials with plaintext
  shown once and never recoverable.

## 6. Authentication and credential lifecycle

### 6.1 Credential classes

| Surface | Authentication | CSRF/Origin | Permitted use |
| --- | --- | --- | --- |
| Browser | `c6_session` HttpOnly cookie + bound CSRF cookie | exact Origin and CSRF on mutation | Hub/Admin JSON API |
| CLI API | `Authorization: Bearer <CLI token>` | no CSRF; exact Origin required if an Origin header is present | scoped non-admin API |
| Git HTTP | Basic username `c6`, token as password | no cookie, CSRF, or Origin requirement | upload-pack/receive-pack only |

Ambiguous credentials fail. A request containing both cookie identity and an
Authorization credential is rejected on CLI/Git routes. Browser session tokens,
bootstrap tokens, invitation tokens, and opaque `publicKey` metadata never
authenticate CLI or Git requests.

### 6.2 Token format

Tokens are versioned and recognizable so parsers can reject the wrong class:

```text
c6c_v1_<public-id>_<256-bit-secret>   # CLI API
c6g_v1_<public-id>_<256-bit-secret>   # Git HTTP
```

Exact encoding is URL-safe base64 without padding. The public ID is random and
supports indexed lookup; it is not authority. The server stores only token ID,
a SHA-256 verifier, user/device ID, label, type, scopes, optional
workspace/project restriction, created/expiry/last-used/revoked timestamps.
Verifier comparison is constant-time. Plaintext is returned once.

### 6.3 Scope model

Initial scopes:

- `api:read`
- `api:write`
- `git:read`
- `git:write`

No token grants server administration in Phase 2. Browser session plus CSRF is
required to issue or revoke credentials. A token's scope is an upper bound:

```text
effective permission = valid token
                     ∩ active peer/device
                     ∩ live membership
                     ∩ current role capability
                     ∩ optional token resource restriction
```

Reader or higher may fetch. Contributor or higher may push an allowed
non-default branch. Maintainer or higher may fast-forward the default branch
only when Milestone 2.2 explicitly enables that policy. Revocation, expiry,
peer/device revocation, and membership downgrade take effect on the next request.

### 6.4 API

All paths are relative to `/api/v1`:

| Method and path | Authority | Behavior |
| --- | --- | --- |
| `POST /credentials` | browser session + CSRF | create one CLI or Git credential; reveal plaintext once |
| `GET /credentials` | current peer | metadata only, never verifier/plaintext |
| `DELETE /credentials/{id}` | owner of credential | revoke idempotently |
| `GET /cli/whoami` | `api:read` bearer | server/user/workspace summary for login verification |
| `GET /projects/{id}/remote` | authenticated visible project | canonical clone URL and transport capabilities |

Inputs include type, label, expiry, scopes, and optional resource restriction.
Expiry is mandatory and bounded; the default is 30 days and maximum is 90 days
for this preview. Labels are 1–80 printable characters. Invalid scope/type
combinations return `400`; insufficient live authority returns `403` or uniform
`404` when revealing existence would leak a project.

## 7. Smart HTTP contract

### 7.1 Canonical URL and route allowlist

Only these smart routes exist:

```text
GET  /git/{workspace}/{project}.git/info/refs?service=git-upload-pack
POST /git/{workspace}/{project}.git/git-upload-pack

# Milestone 2.2 only
GET  /git/{workspace}/{project}.git/info/refs?service=git-receive-pack
POST /git/{workspace}/{project}.git/git-receive-pack
```

Milestone 2.1 denies receive-pack and never advertises it. Dumb HTTP, anonymous
read, SSH, `git://`, LFS, archives, submodules as a hosting feature, partial
clone filters, arbitrary object paths, and all other services are unsupported.

Reject extra query parameters, invalid/mixed encoding, NUL, encoded slash,
backslash, dot segments, repeated separators, alternate `.git` spellings,
unsupported methods/content types, and oversized `Git-Protocol` headers.

### 7.2 CGI adapter

After authentication, live authorization, and UUID resolution, C6 spawns:

```text
git http-backend
```

with an argv-only invocation and a cleared environment. C6 sets only a controlled
`PATH`, locale, private `HOME`, `GIT_CONFIG_NOSYSTEM=1`, null global config,
canonical `GIT_PROJECT_ROOT`, rewritten UUID `PATH_INFO`, exact request method,
query/content metadata, validated Git protocol, and non-secret actor identifier.
The public slugs and credential never become filesystem or child authority.

The adapter:

- streams request bytes to stdin and stdout bytes to the client with backpressure;
- caps/parses CGI headers before emitting an HTTP response;
- allowlists Status, Content-Type, Cache-Control, Expires, and protocol headers;
- bounds child stderr and never forwards internal stderr to the client;
- terminates and reaps the child on timeout, disconnect, input/output overflow,
  or malformed CGI output;
- never runs repository-provided or user-provided hooks.

### 7.3 Initial resource limits

Defaults are operator-configurable within safe bounds:

| Limit | Default |
| --- | --- |
| advertisement timeout | 30 seconds |
| fetch/push wall time | 10 minutes |
| push request bytes | 100 MiB |
| fetch response bytes | 1 GiB |
| CGI header bytes | 32 KiB |
| captured child stderr | 16 KiB |
| ref commands per push | 64 |
| concurrent Git children | 8 |
| concurrent operations per principal | 2 |
| concurrent receive-pack per repository | 1 |

Known-oversized Content-Length is rejected before spawn. Streaming counters
remain authoritative for chunked or dishonest requests. Capacity/disk-floor
failure returns `503`; size returns `413`; timeout returns `504` when HTTP is
still available. Once Git framing has begun, errors use a bounded safe pkt-line
message where possible and never HTML or JSON.

## 8. Protected push contract

Milestone 2.2 accepts only `refs/heads/*` and rejects HEAD, pseudo/symbolic refs,
tags, notes, replace refs, hidden/internal refs, malformed refs, and every other
namespace. All pushes are fast-forward-only. Branch deletion and force push are
disabled in the initial release.

Policy:

| Actor | Non-default branch | Default branch |
| --- | --- | --- |
| Consumer | deny | deny |
| Reader | deny | deny |
| Runner | deny | deny |
| Contributor | create / fast-forward | deny |
| Maintainer | create / fast-forward | fast-forward |
| Owner | create / fast-forward | fast-forward |

Enforcement runs inside receive-pack's ref transaction through a C6-owned,
owner-only pre-receive helper. It receives immutable server-set actor, role,
project, default branch, and request ID; client push options never select policy.
Missing, changed, or failed policy tooling rejects the whole push. Repository
config also enables object fsck and denies non-fast-forwards/deletes as defense
in depth.

Every Git mutation, browser commit/merge, deletion, and maintenance operation
uses the same keyed per-project coordinator. Upload-pack may run concurrently
subject to global limits; receive-pack is exclusive for a repository.

### 8.1 Git/SQLite recovery

Git is authoritative for refs and cannot participate in a SQLite transaction.
After receive-pack commits refs, a C6-owned post-receive helper atomically writes
a durable event beneath `${C6_DATA_DIR}/spool/git-push/`. The server drains the
event idempotently into SQLite, updates `projects.head_sha` from the actual
default ref, and writes success audit. Startup reconciliation drains the spool
and compares every project default ref with SQL.

Audit distinguishes accepted attempt, committed refs, reconciled success, and
failure. It records request/credential/user/project IDs, service, ref names,
old/new OIDs, counts, bytes, duration, and safe outcome—never secrets, pack data,
commit bodies, Basic headers, or full query strings.

## 9. CLI contract

### 9.1 Packages

- `c6-client`: typed reusable client library.
- `c6-cli`: package installing binaries `c6` and `git-credential-c6`.

The CLI requires an installed supported Git executable. It invokes Git with an
argument vector, never a shell string. It does not embed libgit2 or inspect C6
server storage.

### 9.2 Commands implemented in Milestone 2.1

```text
c6 version
c6 server add <origin> [--name <alias>] [--allow-http-localhost]
c6 server list
c6 server use <alias>
c6 auth login [--server <alias>] --token-stdin
c6 auth status [--server <alias>]
c6 auth logout [--server <alias>]
c6 project list [--server <alias>] [--workspace <slug>]
c6 clone <workspace>/<project> [directory] [--server <alias>]
c6 remote add <workspace>/<project> [--name c6] [--server <alias>]
c6 doctor [--server <alias>]
git-credential-c6 get|store|erase
```

`auth login` verifies the credential with `/cli/whoami` before storing it.
Secrets are accepted only from stdin. `clone` and `remote add` obtain the remote
URL from the server rather than reconstructing disk/path authority. Git remains
responsible for clone/fetch/pull/push behavior.

### 9.3 Configuration and credentials

Config contains no secrets:

```toml
version = 1
default_server = "work"

[servers.work]
base_url = "https://c6.example"
server_id = "immutable-server-id"
```

URLs must be an origin: no credentials, path, query, or fragment. HTTPS is
required except explicit loopback HTTP. TLS verification cannot be disabled.
The verified server ID is recorded and compared during explicit login and
`doctor` checks so an accidental replacement can be detected before changing
the saved association. Ordinary authenticated requests and the Git helper trust
the configured HTTPS origin; this is not cryptographic server-ID pinning and
does not add a preflight before every credential use. Changing the saved
association requires explicit operator action and never silently migrates
credentials.

Credential storage is abstracted behind a narrow trait. Preferred storage is
the OS credential manager. A headless OSS fallback may use an atomic owner-only
regular file, but only after explicit opt-in; it must refuse symlinks,
non-regular files, wrong ownership, or permissions broader than `0600`, and must
warn that the token is plaintext at rest. Test overrides use `C6_CONFIG_DIR` and
must not affect the normal location.

`git-credential-c6` implements Git's stdin/stdout credential protocol. It
returns a Git token only for an exact configured origin and matching repository
path. Unknown hosts/paths return no credential. The remote URL and `.git/config`
never contain a secret.

### 9.4 Output and exit behavior

Human output uses stdout for result data and stderr for progress/diagnostics.
Non-TTY and `--json` modes never prompt. JSON is versioned:

```json
{"version":1,"ok":true,"data":{}}
{"version":1,"ok":false,"error":{"code":"forbidden","message":"...","requestId":"..."}}
```

Credentials are never serializable output. Stable exit groups:

| Exit | Meaning |
| --- | --- |
| 0 | success |
| 1 | internal/unknown |
| 2 | usage |
| 10 | missing/invalid authentication |
| 11 | forbidden |
| 12 | not found |
| 13 | conflict |
| 20 | network/TLS/timeout |
| 21 | server/protocol mismatch |
| 30 | local config/credential failure |
| 31 | delegated Git failure |

## 10. Browser experience

Hub project pages show the canonical clone URL only when read transport is
actually available. The copy control copies a credential-free URL. Project
settings provide a self-service “Access credentials” panel:

- create separate CLI or Git credential;
- choose label, expiry, scopes, and optional project restriction;
- reveal plaintext once in a no-store dialog;
- copy explicitly with no DOM fallback that leaves the token rendered;
- list metadata and last-used time;
- revoke immediately.

The UI never describes `publicKey` metadata as login proof, never embeds tokens
in clone commands, and never labels read-only transport as push-capable.

## 11. Error and compatibility contract

- Missing/malformed/invalid Git credential: `401` plus
  `WWW-Authenticate: Basic realm="C6 Git", charset="UTF-8"`.
- Authenticated invisible/missing project: uniform `404`.
- Visible project but insufficient service authority: `403`.
- Unsupported route/service/media/protocol: `400`, `404`, or `415` as specified
  by the route; never SPA HTML.
- Credential endpoints use the existing structured JSON error envelope.
- The server refuses cross-origin redirects and the client refuses to forward
  Authorization across origins or downgrade HTTPS.
- Existing browser cookies and APIs remain compatible. No migration converts
  cookies or `publicKey` metadata into new credentials.
- Database migration is additive. Rollback can leave credential rows ignored by
  an older binary; operators must back up before downgrade.

## 12. Threat model and abuse cases

Assets: source history, unpublished branches, credentials, membership, default
branch integrity, Git/SQLite consistency, disk/memory/CPU, audit evidence.

Actors: trusted peers, compromised peer device, unauthenticated internet client,
malicious browser origin, tunnel/proxy, project code author, local operator.

Required defenses and negative tests include:

- credential-class confusion, cookie-on-Git, bearer-on-browser, dual credentials;
- malformed/oversized Basic, token, service, protocol, CGI header, pkt/ref input;
- expiry/revocation/peer/device revocation and live role/scope downgrade;
- cross-workspace/project access and uniform non-enumerating failures;
- credentials in URL/userinfo/query/remote/config/argv/env/log/error/audit/trace;
- traversal, double encoding, backslash, NUL, slug collision/rename;
- contributor default-branch update, force/delete/tag/notes/replace/hidden refs;
- mixed allowed/denied multi-ref transaction atomicity;
- repository or user hooks/config inheritance and tampered policy helper;
- slowloris, forged length, pack bomb, byte/time/concurrency/disk exhaustion;
- disconnect cleanup, child crash, zombie/temp-file leakage;
- simultaneous pushes, fetch during push, deletion/maintenance races;
- crash after Git ref commit and before SQLite/audit, spool replay, reconciliation;
- malicious redirect, HTTPS downgrade, spoofed forwarded address;
- credential-store symlink/mode/ownership/corruption/concurrent-write behavior.

## 13. Rate limits and observability

Token creation and failed authentication are rate-limited per credential public
ID, peer when known, and trustworthy source context. Source IP is context, never
identity or the only lockout key. Forwarded addresses are ignored unless the
direct proxy is explicitly trusted.

Logs redact Authorization, Cookie, credentials, invite fragments, pack bytes,
and query strings. Authentication failures are aggregated to avoid audit-log
amplification. Metrics may include operation counts, safe outcomes, duration,
bytes, child saturation, spool depth, reconciliation count, and disk-floor
refusals; they contain no ref content or credential material.

## 14. Delivery plan and ownership

### Milestone 2.1 — remote foundation

1. Reconcile canonical `c6-core` wire DTOs and golden JSON tests.
2. Add credential schema, issuance/list/revoke, live token authentication, and
   credential-class negative tests.
3. Add bounded read-only smart HTTP adapter and exact route allowlist.
4. Add `c6-client`, `c6-cli`, safe config/credential storage, helper protocol,
   clone/remote/doctor flows.
5. Add Hub credential management and truthful remote URL presentation.
6. Dogfood real `c6 clone`, `git fetch`, and `git pull` across restart and
   revocation using a fresh server.

### Milestone 2.2 — protected push

1. Add shared per-repository mutation coordinator.
2. Add owner-only pre/post-receive helpers and policy integrity checks.
3. Add durable spool, SQL head synchronization, startup reconciliation, and
   exact audit outcomes.
4. Enable receive-pack only after full ref/role/scope and abuse tests pass.
5. Dogfood contributor branch push, maintainer default fast-forward, reader
   denial, non-fast-forward denial, revocation, concurrency, and crash recovery.

### Build-team file ownership

- core contracts: `crates/c6-core/**`
- Git adapter/policy: `crates/c6-git/**`
- credentials/routes/audit: `crates/c6-server/**`, `migrations/**`
- typed client/CLI/helper: `crates/c6-client/**`, `crates/c6-cli/**`
- Hub UX: `web/**`
- regression harness: `qa/**`, `teams/c6-build-team/qa/**`
- operator/product docs: `README.md`, `docs/**`, examples

Shared workspace manifests and lockfiles are integrated by the build lead.

## 15. Acceptance gates

Milestone 2.1 is complete only when:

- real Git clone/fetch/pull works through C6 smart HTTP with no token in remote;
- consumer/anonymous/invalid/expired/revoked/cross-workspace access is denied;
- reader succeeds and live downgrade/revocation fails on the next request;
- Git POST succeeds without Origin while browser cross-origin mutation remains
  denied;
- CLI config, token input, credential helper, JSON, exits, and redaction pass;
- C6 restarts preserve repository, token metadata, and revocation;
- desktop/mobile UI and all local build-team gates pass;
- receive-pack is demonstrably unavailable and not advertised.

Milestone 2.2 is complete only when:

- contributor branch push and subsequent clone/pull work with normal Git;
- default branch, namespace, fast-forward, delete, force, and mixed-ref policies
  match this spec for every role;
- repository mutations serialize and resource limits fail safely;
- actual refs, SQL head, durable spool, and audit reconcile after induced crashes;
- secrets are absent from remotes, config, argv, env, logs, audit, traces, and test
  artifacts;
- the full local release gate and an independent security/release review approve.

## 16. Public-exposure gate

Completing functional smart HTTP does not by itself make C6 safe for arbitrary
public-internet or hostile multi-tenant use. Before that claim, C6 also needs:

- durable device re-login proof and installation-administrator recovery;
- throttling for bootstrap, invitations, credential issuance, and Git auth;
- stable HTTPS ingress with explicit trusted-proxy handling;
- reviewed keychain/helper behavior on supported operating systems;
- bounded Git backend, quotas, redaction, monitoring, and backup/restore drills;
- security review of dependencies and the complete negative test matrix.

Until those are closed, the feature is for a trusted self-hosted installation
behind operator-managed HTTPS. This limitation must appear in setup and UI.

## 17. Deliberate deferrals

- SSH and `git://` transports
- anonymous/public repositories
- custom `c6 push`/`c6 pull` protocol
- Git LFS, archive, dumb HTTP, partial clone, and public submodule hosting
- branch deletion, force push, mutable tags, and arbitrary server hooks
- GitHub OAuth, company OIDC, SCIM, federation, or hosted C6 identity
- multi-replica control plane or shared remote Git storage
- automatic ngrok lifecycle inside the server
- package installers, auto-update, shell completion, and GUI credential managers
  beyond the minimum supported credential-store abstraction
- publish/run/deploy commands until those operations are real rather than
  recorded-only or simulated

## 18. Rollout and rollback

Milestone 2.1 starts disabled unless Git capability checks pass. `c6 doctor`
reports installed Git version, `git http-backend` availability, origin/TLS,
credential-store readiness, and server transport capabilities. Operators can
disable Git transport without disabling Hub/Admin JSON APIs.

Milestone 2.2 has a separate receive-pack feature switch defaulting off during
upgrade. Rollback disables receive-pack first, drains/reconciles the spool, backs
up SQLite and Git together, then returns to read-only transport. A rollback must
never delete credential or spool data automatically.
