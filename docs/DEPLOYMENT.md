# Deployment patterns

C6 is a development preview. “Deployment” here means installing the C6 control
plane, not hosting a project workload.

## Source on one laptop

```bash
npm ci --prefix web
npm run build --prefix web
C6_DATA_DIR=.c6 cargo run -p c6-server
```

Open `http://127.0.0.1:8787`. The process binds loopback by default and writes
all control/Git data under `.c6`.

## Docker Compose on one laptop

```bash
cp .env.example .env
chmod 600 .env
docker compose up --build -d
docker compose ps
curl --fail http://127.0.0.1:8787/healthz
```

Docker publishes C6 only to host loopback. The server and standalone simulation
runner have separate read-only containers and persistent data volumes. C6 has
no runner socket mount or Docker socket.

Authenticated read-only Git HTTP is also disabled by default. Set
`C6_GIT_HTTP_ENABLED=1` in the private `.env` only for a deliberate trusted
evaluation; this does not enable push or make internet exposure production-safe.

## Trusted LAN evaluation

Direct LAN HTTP exposes session/invitation data to anyone able to observe the
network and is not recommended. It requires all of:

```dotenv
C6_BIND_ADDRESS=0.0.0.0
C6_PUBLIC_BASE_URL=http://192.0.2.10:8787
```

The container already acknowledges its private non-loopback hop internally.
For a source run, also set `C6_BIND=0.0.0.0 C6_INSECURE_HTTP=1`. Use a host
firewall and a trusted isolated network. IP reachability is never identity.

## Reverse proxy / remote peers

Preferred shape:

```text
browser ── HTTPS ──> operator reverse proxy ── HTTP loopback ──> C6
```

Set `C6_PUBLIC_BASE_URL=https://c6.example.com` so cookie and Origin semantics
match the browser. Keep Docker's `C6_BIND_ADDRESS=127.0.0.1` when the proxy runs
on the host. If a container/private-network proxy requires a non-loopback C6
backend, set `C6_INSECURE_HTTP=1`, isolate that network, and understand the hop
is plaintext. An HTTPS public URL does not turn on backend TLS.

The proxy should preserve host/scheme semantics, avoid caching APIs, cap request
sizes/connections, and expose no data volume, runner socket, or internal port.

### Optional tunnels, including ngrok

ngrok is still useful for a laptop evaluation when an operator wants a public
HTTPS URL without configuring inbound routing. It is optional and replaceable
by Tailscale, Cloudflare Tunnel, Caddy, or another HTTPS gateway. Configure the
stable tunnel origin as `C6_PUBLIC_BASE_URL`, keep the C6 listener on loopback,
and protect the tunnel account. A tunnel address or tunnel-provider identity
never authenticates a C6 peer; C6 sessions and separate CLI/Git credentials are
still required. Do not use an ephemeral URL without understanding that origin
changes invalidate browser origin assumptions and require CLI/remote rebinding.

## Always-on Linux host

This is the recommended future shape for a small team: run the same Compose
stack or supervised binaries on one host, persist the data volumes, keep C6 on
loopback/private networking, terminate HTTPS at a reverse proxy or tunnel, and
automate coordinated, verified backups. The current preview still lacks owner
recovery, rate limiting, workload execution, and hardened public exposure; an
always-on host improves availability but does not remove those blockers.

## AWS or another cloud VM

Run the same single-authority artifact on a small VM with a persistent attached
disk. Allow inbound HTTPS only to the operator-managed gateway, deny direct
public access to port 8787 and runner sockets, and snapshot/export the complete
data boundary. C6 does not require Kubernetes, RDS, object storage, or a vendor
control plane. Restore/migration must leave only one writable authority.

## Unsupported patterns

- Direct public-internet C6 listener
- Multiple C6 processes sharing one data directory
- Multiple control-plane replicas or network filesystems
- Kubernetes/HA, remote runners, or federation
- Public/anonymous projects
- Treating C6 as a project hosting runtime

See [ADR 0001](decisions/0001-single-authority-self-hosting.md) for the
authority/portability decision and the
[agent-first runtime specification](specs/AGENT_FIRST_RUNTIME.md) for deferred
hosting and sandbox design.

See [Configuration](CONFIGURATION.md) and [Operations](OPERATIONS.md) for exact
settings and backup/upgrade procedures.
