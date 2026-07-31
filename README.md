# C6

**An agent-centric software forge for small software.**

C6 is an open-source, self-hosted forge for small web applications, scheduled
jobs, and agent-built automations. It keeps ordinary Git, collaboration,
declared runtime intent, and eventually hosting together under one authority
the operator controls. Humans use the web and CLI; agents should be able to use
the same typed, pollable interfaces without a privileged side door.

The repository is named **cresix**. The product is called **C6**.

The repository now also contains a dogfood **Cresix Cloud** service: an
optional account, global-workspace directory, and outbound relay for sovereign
C6 installations. Standalone C6 remains supported and keeps authority over
source, local roles, and runtime state.

The [C6 manual](docs/README.md) covers product principles, trust,
authorization, API, storage, Git, runner, scheduler, web design, threat model,
deployment patterns, examples, dogfood, and testing.

> [!WARNING]
> C6 is currently a local development preview. The control plane has durable
> invitation-bound cookie sessions, expiring CLI/Git credentials, role checks,
> SQLite state, local Git repositories, a thin CLI, and opt-in authenticated
> read-only Git smart HTTP. It does not provide key-based login, Git push,
> application hosting, workload execution, owner recovery, or rate limiting. Some UI flows
> use labelled preview fixtures. Do not expose this revision to an untrusted
> network.

The Cloud preview is subject to the same warning. Its loopback bootstrap is not
a production account system, and its path-based local relay is only a dogfood
stand-in for the isolated per-installation origins required in production.

## Run the local preview

Requirements: Rust 1.95+, Node.js 22+, and npm.

With [Just](https://just.systems/) installed, the shortest path is:

```bash
just setup
just start
```

`just start` builds the web app, starts the complete foreground server, and
prints a newly generated first-owner token in that local terminal. It never
prints the token after claim. You can deliberately show the still-unclaimed
token again with `just bootstrap-token`. Use `just start-git` to opt into the
read-only Git preview.

The equivalent commands without Just are:

```bash
npm ci --prefix web
npm run build --prefix web
cargo run -p c6-server
```

Open [http://127.0.0.1:8787](http://127.0.0.1:8787). The Rust server serves the
React application and API from the same origin. To work on the UI with hot
reload, run these in separate terminals:

```bash
C6_PUBLIC_BASE_URL=http://127.0.0.1:5173 cargo run -p c6-server
npm run dev --prefix web
```

Then open exactly [http://127.0.0.1:5173](http://127.0.0.1:5173). The matching
public origin is required because Vite proxies API calls while C6 rejects
cross-origin mutations.

## Run the Cresix Cloud preview

Install both web workspaces once with `just setup`, then start Cloud in a
separate terminal:

```bash
just cloud-start
```

Open [http://127.0.0.1:8790](http://127.0.0.1:8790). The command prints the
new one-time claim token only in that local terminal and deletes its file after
claim. Use `just start-all` to run standalone C6 and the loopback Cloud preview
together, with separate data directories and separate claim tokens. The Docker
image contains both binaries, but the default Compose topology intentionally
starts only C6 and its runner: Cloud's loopback-only preview bootstrap must not
be weakened merely to cross a container network boundary.

Connected mode uses `c6-connector --config <owner-only-file>` and separate
Cloud/local credential files. See the
[connected example](examples/connected-cloud/README.md) and the
[Cloud connected-mode specification](docs/specs/CRESIX_CLOUD_CONNECTED_MODE.md).
The connector makes an outbound connection, so peers do not need the same
Wi-Fi, an inbound router port, or ngrok. The laptop must remain awake and
online, and a real public deployment still needs DNS, TLS, isolated relay
origins, production login/recovery, rate limits, and operational hardening.

Source runs bind to loopback by default. Keep the operating-system firewall
enabled and do not treat network reachability as authorization.

## Run the service topology

Docker Compose starts the C6 control plane and its separately authenticated
runner boundary. Durable control state and bare Git repositories live under one
data volume so a laptop installation can be backed up and moved as one unit.

```bash
cp .env.example .env
chmod 600 .env
docker compose config --quiet
docker compose up --build -d
docker compose ps
curl --fail http://127.0.0.1:8787/healthz
```

This topology is for development while authentication and sandbox enforcement
remain incomplete. See the [operator guide](docs/OPERATIONS.md) for data
locations, backup boundaries, upgrades, and troubleshooting, and the
[configuration reference](docs/CONFIGURATION.md) for every supported setting.

## Native invitation sessions

C6 will be self-hosted and remote-first without depending on a C6-operated
login service. One installation is authoritative for its own peers and access:

1. A new installation writes a one-time bootstrap token to its private data
   directory; the first owner uses it to claim the server.
2. That immutable server administrator creates a short-lived, single-use
   invitation with a workspace role.
3. The invitee redeems it and receives a revocable 30-day browser session.
4. C6 stores the peer, device label, membership, and audit event locally.
5. The server administrator can revoke a session, device record, or peer.

The server address or client IP is never an identity. Peers may be anywhere as
long as they can reach an operator-managed HTTPS gateway. C6 itself listens on
plaintext HTTP, so the gateway must terminate TLS and use a loopback or trusted
private backend hop. Setting an `https://` public URL does not enable TLS.
Native v1 has no password,
passkey, SSH-key, external-provider, or recovery login. A workspace owner is
not a server administrator. If the bootstrap administrator's sole 30-day
session is lost or revoked, or expires after 30 days without a successful
session read, global administration is locked out. Active use renews the
30-day window, but lost-cookie recovery remains a blocking preview limitation.

## CLI and read-only Git

Build the preview CLI and credential helper with `cargo build -p c6-cli`, then
put both binaries on `PATH` (for a source checkout,
`export PATH="$PWD/target/debug:$PATH"`). In
the Hub, open **Credentials** and create two different credentials: a CLI token
with `api:read`, and a Git token with `git:read`. Each plaintext token is shown
once. Restart the server with `C6_GIT_HTTP_ENABLED=1` to opt into read-only Git;
it is disabled by default while public-exposure hardening is incomplete.

```bash
cargo run -p c6-cli --bin c6 -- server add http://127.0.0.1:8787 \
  --name local --allow-http-localhost
read -rsp 'CLI token: ' C6_CLI_TOKEN; echo
printf '%s\n' "$C6_CLI_TOKEN" | cargo run -p c6-cli --bin c6 -- \
  auth login --server local --token-stdin --plaintext-store
unset C6_CLI_TOKEN
cargo run -p c6-cli --bin c6 -- project list --server local
cargo run -p c6-cli --bin c6 -- clone my-team/my-project
```

Read the CLI token into a non-exported shell variable with a hidden prompt, and
unset it immediately after login; do not put either token in command arguments,
URLs, environment configuration, or shell history.

On the first clone, Git requests username `c6` and the separate Git token as
the password. The preview helper stores tokens in an owner-only plaintext file;
see the [CLI guide](docs/CLI.md) before using it. Clone, fetch, and pull are
implemented. Push is rejected and not advertised.

## One authority, replaceable ingress

Each sovereign installation has one authoritative server. Run it on a laptop,
an always-on Linux host, or a small AWS VM, and put any remote route behind
trusted HTTPS. ngrok can be convenient for a laptop demo, but it is an optional
ingress adapter—never C6 identity, authorization, storage, or a required
service. See [Deployment](docs/DEPLOYMENT.md) and the accepted
[single-authority ADR](docs/decisions/0001-single-authority-self-hosting.md).

## Project contract

A C6 project is a Git repository with a versioned [`c6.toml`](examples/weeknote/c6.toml):

```toml
version = 1

[[services]]
name = "web"
command = "./server"
port = 8080

[[jobs]]
name = "friday-notes"
kind = "agent"
agent_config = "agents/friday-notes.toml"
schedule = "0 16 * * 5"
timezone = "America/New_York"
repository_write = "proposal"
secrets = ["OPENAI_API_KEY"]
```

Secret values do not belong in Git, manifests, forks, logs, or agent
configuration. The eventual secret store grants values to individual workloads
by name.

## C6 repositories as components

A **C6R** is a small, typed C6 repository—or a safe subdirectory closure—that
can be pinned and composed into a larger GitHub-scale project. Good candidates
include wiki/content packs, agent teams, focused tools, MCP servers, mini apps,
and automation packs. Ordinary language libraries, mutable data, credentials,
and tightly coupled monolith modules are not C6Rs.

The accepted v1 design is intentionally non-executing: it specifies future
materialization only for immutable `content` and `agent_team` exports, with
direct locked dependencies. It treats active tool/MCP/app/automation
descriptors as unavailable until C6
has real sandbox, secret, and grant enforcement. The complete manifest,
provenance, CLI, UI, security, and larger-repository contract is in the
[C6R composition specification](docs/specs/C6R_COMPOSITION.md).

C6 dogfoods this contract through the root [`c6.toml`](c6.toml). It declares
the intended Dockerfile-built server command, health probe, resource request,
and local `./qa/run.sh` regression job. C6 can validate this declaration; it
does not deploy the service or execute the job.

## Repository map

```text
crates/c6-core/    domain contracts, manifests, and authorization
crates/c6-cloud-core/ hosted account, catalog, and relay contracts
crates/c6-cloud/   optional hosted account/directory/relay dogfood service
crates/c6-connector/ outbound fixed-upstream connector
crates/c6-server/  HTTP control plane and React static serving
crates/c6-git/     Git repository service boundary
crates/c6-runner/  workload execution boundary
web/               React/Vite product interface
cloud-web/         React/Vite Cresix account and directory interface
examples/          shareable C6 projects
docs/              architecture, operations, and security guidance
teams/             versioned C6 agent-team configurations and local QA
```

## Verify locally

C6 intentionally uses local, reproducible gates rather than a hosted CI
dependency:

```bash
make team-qa
make team-smoke
```

The gates cover Rust formatting, Clippy, unit/integration tests, React tests,
the production web build, Compose validation, credential-shaped tracked files,
and live API smoke checks. Runner dogfood talks directly to the authenticated
simulation protocol; it does not imply that C6 dispatches recorded runs.

## Status and license

The implemented/deferred boundary is tracked in [the roadmap](docs/ROADMAP.md).
Security assumptions and reporting guidance are in [SECURITY.md](SECURITY.md).
C6 is licensed under the GNU Affero General Public License v3.0 only; see
[LICENSE](LICENSE).
