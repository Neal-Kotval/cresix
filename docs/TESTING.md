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
| `c6-git` | temp-repo behavior, traversal, symlink, ref, merge, deletion safety |
| `c6-scheduler` | clock, DST, recovery, idempotency, concurrency decisions |
| `c6-runner` | framing, authentication, replay, validation, lifecycle, key/socket safety |
| `c6-server` | SQLite durability, enrollment, cookies/CSRF, roles, audit, repository/runtime APIs |
| Web unit/API | rendering, request/cookie behavior, truthful error states |
| Playwright | real-backend first-run and collaboration journeys |
| Black-box QA | binaries over real HTTP and Unix sockets |
| Packaging | Dockerfile/Compose shape and secret/socket boundaries |

## Required regression shape

Every defect should add the smallest test at the boundary that failed. Security
changes require positive and negative cases: valid use, unauthenticated use,
lower role, revoked credential, malformed input, replay/concurrency where
relevant, and proof that secrets do not enter logs or tracked files.

Use injected clocks instead of sleeps, temporary directories instead of `.c6`,
random loopback ports, synthetic high-entropy credentials, and complete cleanup.
Never inspect or reuse `~/.codex`, SSH private keys, browser profiles, or cloud
credentials.

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
