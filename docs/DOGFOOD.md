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
