# Git subsystem

## Implemented local store

`c6-git` manages bare repositories beneath `${C6_DATA_DIR}/git` using Git
plumbing invoked with argument arrays and a cleared environment—never shell
strings. Repository directory names are server-created project UUIDs.

The library supports:

- safe creation, opening, local-only import, and staged deletion;
- SHA-1 and SHA-256 object formats;
- bounded branches, commits, trees, file reads, and diffs;
- branch creation and atomic commits from bounded file changes;
- expected-old-value ref updates and merge/conflict analysis;
- traversal, symlink, `.git`, option-injection, and oversized-output refusal.

The server currently exposes bounded read endpoints and seeds a new project
repository with initial content. Other library capability is not automatically
a public product workflow.

## Implemented read-only smart HTTP

C6 serves the standard Git smart-HTTP upload-pack protocol at:

```text
GET  /git/<workspace>/<project>.git/info/refs?service=git-upload-pack
POST /git/<workspace>/<project>.git/git-upload-pack
```

Normal `git clone`, `git fetch`, and `git pull` work. Authentication is HTTP
Basic with exact username `c6` and a separately issued `c6g_v1_...` token as
the password. Browser cookies and CLI Bearer tokens are rejected. Each request
checks token expiry/revocation, active device and peer, optional resource
restriction, and current reader-or-higher membership.

The route is a deliberate preview opt-in: set `C6_GIT_HTTP_ENABLED=1` on the
server. The default is disabled, remote discovery reports `fetch: false`, and
the route returns unavailable. This guard does not make public exposure safe;
rate limiting, owner recovery, and production ingress hardening remain absent.

The adapter resolves public slugs through SQLite to a UUID repository and runs
`git http-backend` with an argument-only invocation, cleared/controlled
environment, bounded request/response/stderr sizes, timeouts, strict protocol
headers, and an allowlist of returned CGI headers. If Git is unavailable at
server startup, remote discovery reports fetch unavailable.

Successful/denied upload-pack requests are not yet written to a durable Git
access audit log. Successful token authentication updates credential
`lastUsedAt`; that metadata is not a complete access record. Durable push audit
and reconciliation belong to the deferred protected-push milestone.

## Not implemented

- Push/receive-pack or any ref mutation over the network
- SSH, dumb HTTP, anonymous access, LFS, archives, or partial clone filters
- Browser branch editing or complete PR merge workflow
- Remote URL import
- Public repositories or anonymous source access

Use the authenticated repository JSON API or read-only smart HTTP. Do not
mutate bare storage behind a running server; that bypasses authorization,
expected-ref checks, and audit records. Push is explicitly deferred until the
atomic authorization/audit design in the
[Phase 2 specification](specs/PHASE_2_GIT_AND_CLI.md) is complete.

## Safety invariants

- User slugs never become filesystem paths.
- Paths are canonicalized under one real root; symlinks are rejected.
- Revisions are full lowercase 40/64-character object IDs when pinning matters.
- Reads and subprocess output are bounded.
- Hooks and inherited Git configuration are not executed during managed import.
- Destructive deletion only targets a verified C6 bare repo staged under a
  private tombstone; SQL integration must complete before the API enables it.
