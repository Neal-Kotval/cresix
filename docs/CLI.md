# C6 CLI

`c6` is a thin client of one C6 authority. It owns local aliases and expiring
credentials, calls the same authorized HTTP API as other clients, and delegates
source transfer to the installed `git` executable. It never reads the server's
SQLite database or Git root and is not a daemon or second control plane.

## Implemented commands

```text
c6 version
c6 server add <origin> [--name <alias>] [--allow-http-localhost]
c6 server list
c6 server use <alias>
c6 auth login [--server <alias>] --token-stdin --plaintext-store
c6 auth status [--server <alias>]
c6 auth logout [--server <alias>]
c6 project list [--server <alias>] [--workspace <slug>]
c6 clone <workspace>/<project> [directory] [--server <alias>]
c6 remote add <workspace>/<project> [--name c6] [--server <alias>]
c6 doctor [--server <alias>]
```

Add accepts HTTPS by default. Loopback HTTP requires the explicit
`--allow-http-localhost` preview opt-in. `server add` stores the installation's
server ID, and login/doctor compare it to detect an accidental replacement at
the configured origin. This is not cryptographic pinning and is not checked
before every request; ordinary API calls and the Git helper match the configured
origin. The client refuses HTTP redirects and cross-origin clone URLs, bounds
responses, and supports `--json` for machine-readable success and error output.

## Credential setup

Create credentials in Hub at `/credentials`. A CLI credential and Git
credential are intentionally different:

- `c6c_v1_...` is sent as an API Bearer token. `c6 auth login` reads it only
  from stdin and verifies it with `/api/v1/cli/whoami` before storing it.
- `c6g_v1_...` is the password for Git Basic authentication with username
  `c6`. Browser cookies and CLI tokens cannot authenticate Git.

The preview credential store is an owner-only plaintext file under the
platform configuration directory (override the directory with
`C6_CONFIG_DIR`). Login therefore requires `--plaintext-store`; the same
opt-in can be supplied with `C6_ALLOW_PLAINTEXT_CREDENTIALS=1`. This is a
headless fallback, not a claim that tokens are encrypted at rest. Never place
tokens in URLs, command arguments, shell history, Git config, or logs.

`c6 clone` installs `git-credential-c6` for that clone. On first use, enter
username `c6` and paste the Git token as the password; Git's `store` helper
operation records it for that exact configured C6 origin and repository path.
`c6 remote add` adds the credential helper to an existing checkout.

## Agent use

Agents can use `--json` and exit codes today for bounded discovery and
diagnostics. Long-running run status, event cursors, webhooks, an MCP server,
and broad mutation commands are not implemented. Those future surfaces must
wrap the same API and live authorization decisions rather than introduce agent
credentials with ambient administrator or host access.

See [Git](GIT.md), [API](API.md), and the
[Phase 2 specification](specs/PHASE_2_GIT_AND_CLI.md).
