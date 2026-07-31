# Testing

## Primary gates

```bash
make team-qa       # static, unit, integration, web, build, Compose checks
make team-smoke    # clean server lifecycle and HTTP contract
./qa/run.sh        # consolidated local acceptance suite
```

No command invokes hosted CI. Some real loopback/socket tests may require local
sandbox permission; a skipped security-boundary test is not a pass.

## Test layers

| Layer | Purpose |
| --- | --- |
| `c6-core` | manifest and cumulative authorization contracts |
| `c6-git` | temp-repo behavior, traversal, symlink, refs, smart-HTTP CGI bounds/timeouts/header filtering, merge, deletion safety |
| `c6-client` | origin/redirect/response bounds, token redaction, canonical remote validation |
| `c6-cli` | config permissions/symlinks, credential separation, secret stdin, project refs, Git delegation/helper targets, JSON errors |
| `c6-scheduler` | clock, DST, recovery, idempotency, concurrency decisions |
| `c6-runner` | framing, authentication, replay, validation, lifecycle, key/socket safety |
| `c6-server` | SQLite durability, enrollment, cookies/CSRF, scoped credential lifecycle/live revocation, roles, read-only Git allow/deny, repository/runtime APIs |
| Web unit/API | rendering, request/cookie behavior, truthful error states |
| Playwright | real-backend first-run and collaboration journeys |
| Black-box QA | binaries over real HTTP and Unix sockets |
| Packaging | Dockerfile/Compose shape and secret/socket boundaries |
| `c6-cloud-core` | cloud identifier, route, catalog, frame, and limit contracts |
| `c6-cloud` | account/bootstrap, namespace, installation, binding, catalog, revocation, and relay authorization |
| `c6-connector` | fixed-upstream enforcement, credential handling, frame state machine, bounds, fencing, and reconnect policy |
| Cloud web | account claim, workspace/installation setup, directory doorway, disconnected/revoked and accessible states |

## Required regression shape

Every defect should add the smallest test at the boundary that failed. Security
changes require positive and negative cases: valid use, unauthenticated use,
lower role, revoked credential, malformed input, replay/concurrency where
relevant, and proof that secrets do not enter logs or tracked files.

Git transport tests must cover the default-disabled feature gate, wrong token
class, mixed cookie/auth, expiry/revocation/device/role/restriction changes,
receive-pack denial, malformed protocol/query/path/content type, request and
response bounds, timeout, redirect refusal, and absence of credentials in URLs,
argv, Git config, logs, audit payloads, and fixtures. `lastUsedAt` is not a
durable access audit; tests must not describe it as one.

Use injected clocks instead of sleeps, temporary directories instead of `.c6`,
random loopback ports, synthetic high-entropy credentials, and complete cleanup.
Never inspect or reuse `~/.codex`, SSH private keys, browser profiles, or cloud
credentials.

Connected-mode tests must use loopback listeners, temporary Cloud and C6 data
directories, synthetic connector credentials, and a test-controlled route
authority. Required negative cases include wrong Origin/CSRF, namespace
collision, cross-owner installation access, stale catalog revisions, unknown or
revoked route, credential replay, arbitrary-upstream attempts, forbidden
forwarding headers, oversized bodies/frames, queue/concurrency saturation,
disconnect during mutation, and replacement-connection fencing. Tests must
also prove standalone C6 starts and remains usable with Cloud absent.

## Before handing off

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
npm test --prefix web -- --run
npm run build --prefix web
docker compose config --quiet
git diff --check
```

Then run the real-process suites appropriate to the changed boundary and record
any environment-caused skip explicitly.
