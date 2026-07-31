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

## Unsupported patterns

- Direct public-internet C6 listener
- Multiple C6 processes sharing one data directory
- Multiple control-plane replicas or network filesystems
- Kubernetes/HA, remote runners, or federation
- Public/anonymous projects
- Treating C6 as a project hosting runtime

See [Configuration](CONFIGURATION.md) and [Operations](OPERATIONS.md) for exact
settings and backup/upgrade procedures.
