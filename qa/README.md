# C6 local acceptance QA

This directory is C6's independent, black-box acceptance layer. It deliberately
does not contain CI configuration. Run the complete local gate from the
repository root:

```sh
./qa/run.sh
```

The gate exercises compiled C6 processes and public protocols where those
interfaces exist. Package unit tests remain useful, but they are not treated as
a substitute for the API, restart, authorization, and runner-boundary suites.

## Requirements

- Bash, Cargo, Git, curl, Python 3, Node.js, and npm.
- Playwright's Chromium browser for the headless website suite. If the browser
  is not installed, the harness reports the missing optional system dependency
  and explains how to install it; it does not label that suite as passing.
- Docker is optional. When available, `docker compose config` validates the
  self-hosting topology without starting or mutating containers.

All test state is created below a fresh temporary directory and removed on
exit. Test credentials are synthetic and are never printed. Server and runner
output is redirected away from the console and removed with the test state.

## Suites

| Suite | Boundary covered |
| --- | --- |
| `component` | Rust unit/regression tests, including repository semantics |
| `api` | First boot, authentication, CSRF/origin checks, authorization, invites, persistence |
| `runner` | Unix-socket framing, malformed/oversized input, duplicate IDs, timeouts |
| `dogfood` | Fresh C6 claim, `cresix` project, peer, repo, schedule/run/deploy, runner dispatch, restart |
| `web` | Frontend unit/build, fixture regressions, and required real-backend headless journey |
| `packaging` | Compose rendering and release-artifact sanity checks |

See [COVERAGE.md](COVERAGE.md) for the expected abuse cases and the distinction
between required coverage and future/unsupported behavior.
