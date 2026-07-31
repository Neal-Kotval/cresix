# C6

**A home for small software.**

C6 is an open-source, self-hosted workspace for small web applications,
scheduled jobs, and agent-powered automations. It combines Git-native project
history and collaboration with the machinery needed to run the result.

The repository is named **cresix**. The product is called **C6**.

The [C6 manual](docs/README.md) covers product principles, trust,
authorization, API, storage, Git, runner, scheduler, web design, threat model,
deployment patterns, examples, dogfood, and testing.

> [!WARNING]
> C6 is currently a local development preview. The control plane has durable
> invitation-bound cookie sessions, role checks, SQLite state, and local Git
> repositories. It does not provide key-based login, Git network transport,
> hosting, or workload execution. The product UI also uses preview fixtures for
> unsupported flows. Do not expose this revision to an untrusted network.

## Run the local preview

Requirements: Rust 1.95+, Node.js 22+, and npm.

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

C6 dogfoods this contract through the root [`c6.toml`](c6.toml). It declares
the intended Dockerfile-built server command, health probe, resource request,
and local `./qa/run.sh` regression job. C6 can validate this declaration; it
does not deploy the service or execute the job.

## Repository map

```text
crates/c6-core/    domain contracts, manifests, and authorization
crates/c6-server/  HTTP control plane and React static serving
crates/c6-git/     Git repository service boundary
crates/c6-runner/  workload execution boundary
web/               React/Vite product interface
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
