# Deployment modes

## Two independent choices

Cresix deployment has two axes:

1. **Where C6 runs:** laptop, always-on Linux host, or cloud virtual machine.
2. **How it is reached:** standalone operator-managed ingress or optional
   Cresix Cloud connected mode.

A cloud VM does not imply Cresix Cloud, and a laptop can eventually use
connected mode. In every case, one C6 installation remains authoritative.

## Laptop

Best for personal use, development, and early team dogfood.

- **Now:** run from source or Compose, keep the backend on loopback, and use
  local browser/CLI/Git workflows.
- For remote peers, add a stable trusted HTTPS reverse proxy, VPN, or tunnel.
- Availability follows the laptop: sleep, shutdown, or network loss stops Hub,
  Git, Cloud relay presence, and any future schedules.
- Back up the declared data root before treating it as durable team storage.

Laptop hosting is valid; it is not always-on hosting.

## Always-on Linux box

Recommended target for a small team.

- Run the same single-node artifact as a supervised service or Compose stack.
- Bind C6 to loopback or a protected private interface.
- Expose only a trusted HTTPS ingress.
- Persist the C6 data root and runner state with restrictive ownership.
- Automate backups and periodically restore them into an isolated verification
  environment.

**Next:** Cresix needs a verified backup/restore journey before presenting this
as a production-ready team appliance.

## AWS or another cloud VM

Use a small VM and persistent attached storage; no managed database,
Kubernetes, registry, or vendor control plane is required by the architecture.
Firewall the C6 backend from the public internet and expose only the trusted
HTTPS edge. VM snapshots are useful but do not replace application-aware
backup and restore verification.

## Standalone mode

Standalone C6 has no Cresix account or Cresix-operated dependency. The operator
chooses DNS, TLS, and ingress. Peers enroll into the installation itself.

Benefits:

- complete local authority and no vendor availability dependency;
- replaceable ingress provider; and
- clearest data, backup, and incident boundary.

Costs:

- the operator owns reachability, TLS, uptime, and backup; and
- sharing URLs depend on the operator's chosen domain or tunnel address.

## Connected mode

Target connected C6 adds a Cresix account handle, account-scoped workspace,
directory listing, installation registration, and outbound relay. It does not
upload local Git, roles, sessions, runtime records, or secrets to Cloud.

Benefits intended for production:

- stable `cresix.com/@{account}/{workspace}/{project}` directory links;
- no inbound router or firewall change for the C6 host; and
- managed availability state and route discovery.

Costs and trust:

- the managed TLS edge can observe relayed cookies, credentials, source, and
  response bodies; the relay is not end-to-end encrypted;
- a Cloud outage removes directory and managed ingress; and
- account and relay operations add a separate public security boundary.

**Now, dogfood:** Cloud and connector run locally, with namespace uniqueness
inside that single preview database and bounded reverse HTTP verified against a
compatible backend. The service refuses non-loopback binding. It is not a
deployable public account service, and its same-origin dogfood doorway is not
the intended production browser route. It temporarily uses
`/{workspace}/{project}` and has no public account handle.

## Ingress choices

ngrok, Tailscale, Cloudflare Tunnel, Caddy, and conventional reverse proxies are
all replaceable options. Cresix should provide recipes, not make any one of them
identity or authorization infrastructure.

Rules that do not vary by provider:

- public credentials cross only trusted HTTPS;
- the configured public base URL must match the browser origin exactly;
- authenticated Git must not follow cross-origin redirects with credentials;
- changing the origin is an explicit migration; and
- installation identity remains distinct from hostname.

## Unsupported deployment shapes

- several writable restores of one installation;
- multiple active control-plane replicas;
- public exposure of the raw C6 HTTP backend;
- public deployment of the loopback Cloud preview;
- hostile multi-tenant application execution;
- treating a tunnel provider or source IP as login; and
- sharing untrusted project applications on the Cloud account or C6 relay
  registrable domain.

Operational commands and configuration live in the [deployment guide](../DEPLOYMENT.md).
