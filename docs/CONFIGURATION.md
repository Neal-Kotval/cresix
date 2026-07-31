# C6 configuration reference

The MVP server and runner are configured with environment variables. The
checked-in [`config/c6.example.toml`](../config/c6.example.toml) records the
longer-term configuration shape, but the binaries do not load TOML yet.

## Server

| Variable | Default | Purpose |
| --- | --- | --- |
| `C6_BIND` | `127.0.0.1` | Process listen address. Non-loopback plaintext HTTP is refused unless explicitly acknowledged. Compose uses `0.0.0.0` inside its isolated container network and separately restricts the published host address. |
| `C6_PORT` | `8787` | TCP port inside the process/container. |
| `C6_PUBLIC_BASE_URL` | `http://127.0.0.1:8787` | Exact browser origin used for mutation-origin checks and invitation links. Only lowercase `http://` or `https://` origins are accepted; credentials, paths, queries, and fragments are rejected. HTTPS enables the session cookie's `Secure` flag but does not add TLS to the listener. |
| `C6_DATA_DIR` | `.c6` | Private directory containing `c6.sqlite3`, its WAL files, and `git/` bare repositories. Use one server process per directory. |
| `C6_WEB_DIST` | `web/dist` | Built React assets served by the control plane. |
| `C6_BOOTSTRAP_TOKEN` | random at first initialization | Optional first-owner claim token. If omitted, C6 writes the random value once to `${C6_DATA_DIR}/bootstrap-token` with mode `0600` and deletes it after claim. An environment-supplied value is hashed but never logged or written in plaintext. It is ignored after initialization. |
| `C6_INSECURE_HTTP` | unset | Setting exactly `1` permits the plaintext listener on non-loopback `C6_BIND` and logs a warning. Use only for an explicitly protected private/container hop or trusted development network. It does not make sessions confidential. |
| `C6_GIT_HTTP_ENABLED` | unset/`false` | `1` or `true` enables authenticated read-only Git smart HTTP; `0` or `false` disables it. Other values fail startup. It defaults off because rate limiting and production exposure hardening are incomplete. It never enables push. |
| `RUST_LOG` | service defaults | Rust tracing filter, for example `c6_server=debug,tower_http=info`. Logs may contain operational metadata; protect them. |

`C6_PUBLIC_BASE_URL` is security-sensitive. It must match the origin peers open
in their browser; otherwise valid mutations are rejected. C6 always listens on
plaintext HTTP. For remote use, terminate TLS at an operator-managed gateway,
keep the backend on loopback when possible, and set the public URL to the HTTPS
browser origin. Every non-loopback `C6_BIND` is refused unless
`C6_INSECURE_HTTP=1` explicitly acknowledges the private/container hop; an
`https://` public URL does not bypass that guard.

## Runner

| Variable | Default | Purpose |
| --- | --- | --- |
| `C6_RUNNER_SOCKET` | `/tmp/c6-runner.sock` | Unix socket for authenticated simulation clients. Compose uses `/run/c6/runner.sock`; the current control plane is not a client. |
| `C6_RUNNER_STATE_DIR` | `/tmp/c6-runner-state` | Private request journal and runner state. Compose persists `/var/lib/c6-runner`. |
| `C6_RUNNER_AUTH_KEY` | unset | Optional explicit request-authentication key of at least 32 UTF-8 bytes. It takes precedence over the key file and is intended for tests or managed secret injection. |
| `C6_RUNNER_AUTH_KEY_FILE` | `runner.key` beside the socket | Private authentication key file. Compose uses `/run/c6/runner.key`. The runner atomically creates 32 random bytes with mode `0600` when it is absent. |

An existing key file must be a regular, non-symlink file with exact mode `0600`
and at least 32 bytes; unsafe files fail closed. Never put its contents in Git,
a manifest, command arguments, or an issue. The current C6 server does not
dispatch work to the runner, whose simulation backend executes no host commands
or containers.

## Compose-only settings

| Variable | Default | Purpose |
| --- | --- | --- |
| `C6_BIND_ADDRESS` | `127.0.0.1` | Host interface receiving published port 8787. Keep loopback unless an authenticated HTTPS/VPN route is ready. |

Changing `C6_PORT` changes the host port through Compose; the container remains
on 8787. `C6_BIND_ADDRESS` is a Docker host-publishing setting and is distinct
from the process-level `C6_BIND`. Set `C6_PUBLIC_BASE_URL` to the actual
peer-facing URL rather than the container name or internal address.

## CLI

| Variable | Default | Purpose |
| --- | --- | --- |
| `C6_CONFIG_DIR` | platform user configuration directory plus `c6` | Override the directory containing owner-only `config.toml` and `credentials.json`. |
| `C6_ALLOW_PLAINTEXT_CREDENTIALS` | unset | Exact value `1` is equivalent to the CLI's explicit `--plaintext-store` opt-in. The preview store is not encrypted. |

CLI server origins require HTTPS except for loopback HTTP explicitly added with
`--allow-http-localhost`. See [CLI](CLI.md).

## Deferred runtime configuration

There are no supported Docker runtime, event-poll, MCP, secret-store master
key, 1Password, or Doppler settings today. Do not invent environment variables
for those features. Their intended boundaries are described in the
[agent-first runtime specification](specs/AGENT_FIRST_RUNTIME.md); settings
become part of this reference only when a working vertical slice ships.

## Connected-mode configuration

Standalone C6 has no Cresix Cloud setting and must continue to start without a
Cloud account or network dependency. Connected mode is implemented by separate
`c6-cloud` and `c6-connector` processes; the local C6 server must not receive a
Cloud account session or connector secret.

### Cloud service

| Variable | Default | Purpose |
| --- | --- | --- |
| `C6_CLOUD_BIND` | `127.0.0.1` | Cloud listener address. This dogfood revision refuses every non-loopback bind. |
| `C6_CLOUD_PORT` | `8790` | Cloud listener port. |
| `C6_CLOUD_PUBLIC_ORIGIN` | `http://127.0.0.1:${C6_CLOUD_PORT}` | Exact HTTP(S) browser origin. Paths, queries, fragments, and embedded credentials are rejected. |
| `C6_CLOUD_DATA_DIR` | `.c6-cloud` | Private Cloud SQLite and first-account bootstrap-token directory. This is separate from `C6_DATA_DIR`. |
| `C6_CLOUD_WEB_DIR` | `cloud-web/dist` | Built Cresix Cloud web assets. |

On an unclaimed service, Cloud writes the one-time account bootstrap proof to
`${C6_CLOUD_DATA_DIR}/bootstrap-token`. Keep the directory owner-only and show
the token only in a local trusted terminal. Claim consumes the proof. There is
no environment-variable override because putting a bootstrap proof into shared
process configuration broadens its exposure.

### Connector file

`c6-connector --config <path>` reads a strict TOML file. On Unix the config and
each credential file must be regular, owner-only files (mode `0600`) with no
additional hard links. The config keys are:

| Key | Meaning |
| --- | --- |
| `cloud_origin` | Bare HTTPS Cloud origin; loopback HTTP is accepted only with the explicit dogfood flag below. |
| `local_origin` | Exactly `http://127.0.0.1:<port>`; other hosts, schemes, paths, or missing ports are rejected. |
| `installation_id` | Cloud-issued installation UUID. |
| `binding_id` | Cloud-issued workspace-binding UUID. |
| `local_workspace_id` | Existing installation-local workspace UUID. |
| `cloud_credential_file` | Separate file containing the one-time-issued connector credential. |
| `local_credential_file` | Separate file containing a local C6 API credential for bounded catalog reads. |
| `allow_insecure_cloud_loopback` | Defaults `false`; set `true` only for local HTTP dogfood. |
| `catalog_interval_seconds` | Defaults `60`; accepted range 10 through 86,400. |
| `request_timeout_seconds` | Defaults `30`; accepted range 1 through 120. |
| `max_in_flight` | Defaults `32`; accepted range 1 through 32. |

See the credential-free
[`connector.example.toml`](../examples/connected-cloud/connector.example.toml).
Never put a connector credential in Git, a URL, the config file, process
arguments, catalog metadata, or shared logs.

## Secret handling

Treat `.env` as private operator configuration even though the default Compose
file no longer requires a shared secret in it. Restrict it to the owner
(`chmod 600 .env`). `docker compose config` interpolates values into its output,
so inspect that output before sharing it.
