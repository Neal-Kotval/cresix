# C6

**A home for small software.**

C6 is an open-source, self-hosted collaboration and hosting platform for small
web apps, scheduled jobs, and agent-powered automations. It keeps the useful
parts of GitHub—real Git repositories, branches, pull requests, diffs, reviews,
forks, and attribution—and places the running software beside them.

The repository is named **cresix**. The product is called **C6**.

> [!IMPORTANT]
> This repository currently contains the first runnable vertical slice: the C6
> project website, stable Rust domain contracts, manifest validation, initial
> API and runner boundaries, persistence schema, and one-server packaging. Git
> protocol serving, production authentication, and container execution are
> represented by their interfaces but are not yet safe to use in production.

## See it on your laptop

Requirements: Rust 1.95+, Node 22+, and npm.

```bash
npm install --prefix web
npm run build --prefix web
cargo run -p c6-server
```

Open [http://localhost:8787](http://localhost:8787). The Rust server serves the
React application and API from the same port.

For UI development with hot reload, use two terminals:

```bash
cargo run -p c6-server
npm run dev --prefix web
```

Then open [http://localhost:5173](http://localhost:5173).

## Share it from a laptop

C6 listens on `0.0.0.0`, so another person on the same local network can open:

```text
http://<your-laptop-LAN-IP>:8787
```

Your operating-system firewall must permit inbound connections. Internet-wide
sharing additionally needs a public HTTPS route: either a domain and router
port-forwarding, or an operator-configured outbound tunnel. C6 does not silently
open a public tunnel because that changes the server's trust boundary.

The current slice is a seeded product preview, not a production authentication
system. Do not expose it to the internet yet.

## One-server distribution

Copy the example environment file, replace every placeholder secret, and start
the stack:

```bash
cp .env.example .env
docker compose up --build
```

The Compose topology includes separate control and workload PostgreSQL servers,
S3-compatible object storage, an OCI registry, the unprivileged C6 control
plane, and a separate runner boundary.

## What a C6 project contains

A project is a real Git repository. Its versioned [`c6.toml`](examples/weeknote/c6.toml)
declares what runs:

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

C6 stores secret values separately. Repositories, forks, logs, and manifests
contain only secret names.

## Authentication direction

GitHub OAuth is the convenient default when an operator configures it. C6 still
owns the internal user identity, project roles, SSH keys, tokens, and audit
history. A local server-owner recovery account remains available so a
self-hosted installation is not operationally dependent on GitHub.

See [`config/c6.example.toml`](config/c6.example.toml) for the provider seam.

## Repository layout

```text
crates/c6-core/    manifest, permissions, and API domain contracts
crates/c6-server/  Axum control plane and React static serving
crates/c6-runner/  isolated privileged execution boundary
web/               React/Vite project workspace
migrations/        PostgreSQL control-plane schema
examples/          shareable C6 projects
docs/              architecture and security boundaries
```

## Verification

```bash
cargo fmt --all -- --check
cargo test --workspace
npm test --prefix web
npm run build --prefix web
docker compose config --quiet
```

## License

C6 is licensed under the GNU Affero General Public License v3.0 only. See
[`LICENSE`](LICENSE).

