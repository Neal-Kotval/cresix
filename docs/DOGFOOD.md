# Dogfood

C6 describes itself with the repository-root `c6.toml`:

- Dockerfile build strategy
- intended `/usr/local/bin/c6-server` service command and port 8787
- `/healthz` probe and resource request
- `local-qa` command pointing at `./qa/run.sh`

This proves that the manifest parser can validate a realistic C6 project. It
does **not** mean C6 builds the image, starts the service, or runs the QA job.

## Real dogfood gates

The local build team runs:

- repository contracts and reference validation;
- Rust format, Clippy, unit, integration, and adversarial tests;
- real-process API lifecycle tests using isolated data directories;
- real Unix-socket runner protocol tests with synthetic credentials;
- React unit/API tests and a production build;
- Playwright journeys against a real C6 backend;
- Compose model validation and credential-shaped file scans.

Runner dogfood talks directly to the simulation daemon. API dogfood verifies
that server run/deployment records say `dispatchAvailable: false`. Together
these tests demonstrate both sides without claiming they are integrated.

Phase 2 dogfood also builds the `c6` and `git-credential-c6` binaries, exercises
separate credential classes and immediate revocation, verifies canonical remote
discovery, and drives upload-pack against a temporary bare repository. The
transport must be explicitly enabled for the scenario and receive-pack must
remain unavailable. This proves read-only Git behavior, not push, hosting, or a
complete public-exposure posture.

The CLI's `--json` output is the first agent-consumable dogfood surface. Event
cursors, long polling, schedule dispatch, Docker execution, secrets, and MCP
remain future gates defined in the
[agent-first runtime specification](specs/AGENT_FIRST_RUNTIME.md).

## Why no hosted CI

The project intentionally makes one local command authoritative so a fork or
offline self-hoster can verify it without granting a hosted service repository
access. The absence of CI is not permission to skip gates: release evidence is
captured from `make team-qa`, `make team-smoke`, or `./qa/run.sh`.

## Adding a dogfood scenario

Prefer a user-visible invariant with an observable failure. Use temporary data,
synthetic secrets, deterministic clocks, and real process boundaries when they
matter. Never make a test depend on a developer's browser login, cloud account,
Codex token, home directory, or ambient Git configuration.
