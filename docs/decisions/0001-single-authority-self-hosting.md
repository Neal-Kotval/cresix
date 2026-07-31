# ADR 0001: Sovereign installations with one authority

- Status: accepted for the current product phase
- Date: 2026-07-31
- Decision owners: C6 maintainers
- Revisit when: one of the evidence triggers below is met
- Implementation status: see the [capability ledger](../product/CAPABILITIES.md)

## Decision

C6 will support many independent, self-hosted installations. Each installation has exactly one authoritative C6 server for identity, authorization, project metadata, schedules, runs, secrets, audit events, and the canonical Git remote. A server may run on a laptop, an always-on Linux machine, or a small cloud virtual machine such as EC2 or Lightsail.

C6 will not implement multi-writer peer-to-peer replication or a mandatory hosted C6 identity/control service in the current phase. Network access is provided by a replaceable HTTPS ingress: a reverse proxy, VPN, or outbound tunnel. The ingress provides reachability only; it is never an identity or authorization authority.

In short:

> C6 is decentralized across installations, centralized within an installation, and Git-distributed at the edge.

## What “peer-to-peer” can mean

Three different ideas have been called P2P in product discussion. They have different costs and should not be conflated.

1. **Peer enrollment** lets an installation owner invite another person without requiring GitHub, Google, or a C6-hosted account. C6 supports this direction.
2. **Peer networking** lets clients reach an installation over a LAN, VPN, tunnel, or public HTTPS endpoint. C6 remains transport-neutral and supports this direction.
3. **Peer authority or federation** lets several independently administered nodes accept writes and reconcile them. This requires distributed identity, permissions, Git-ref, metadata, scheduler, audit, secret, and conflict protocols. C6 defers this.

Git already decentralizes the part that benefits most from it. A developer can clone, inspect history, branch, commit, merge, and rebase while offline. The C6 remote becomes the canonical collaboration point when it is reachable. Replicating the control plane would add conflict and revocation problems without improving that local Git workflow.

## Why one authority is the smallest coherent design

### Sharing

The intended sharing journey is:

1. Start C6 from one installation artifact and one persistent data directory.
2. Claim the installation and configure a stable HTTPS origin.
3. Invite a peer using installation-local identity.
4. The peer uses C6 Hub, authorizes the CLI, and uses an ordinary Git remote.

Peers do not need to be on the same Wi-Fi and do not need a C6 vendor account. They only need network access to the installation's stable HTTPS origin.

### Trust and revocation

An installation-local credential proves identity to that installation. Current membership and role are checked at the authority for every operation. IP address, LAN membership, physical proximity, tunnel-provider identity, and the Basic-auth username do not establish C6 identity.

This gives a company one clear administration, revocation, audit, backup, and incident-response boundary. True federation would make offboarding, recovery, and permission changes ambiguous during partitions.

### Git consistency

Bare Git repositories are authoritative for advertised refs. SQLite is authoritative for peers, roles, projects, pull requests, schedules, runs, and audit. Local clones can diverge while offline; ordinary Git requires the user to merge or rebase after reconnecting. C6 does not silently reconcile divergent refs.

### Agents and schedules

Cron and agent runtimes need one intent authority for the chosen revision, secret grants, concurrency rules, deduplication, leases, and audit. Allowing arbitrary peers to schedule authoritative work risks duplicate or conflicting execution. If distributed execution becomes necessary, remote runners should connect outbound to the authority and accept revision-pinned, leased jobs. That distributes execution without distributing control.

## Supported deployment journeys

### Laptop

Run one binary or Compose stack with a local data directory. Use loopback access for the owner and a stable HTTPS tunnel or VPN address for collaborators. If the laptop sleeps, shuts down, or loses connectivity, Hub, Git, agents, and schedules stop. Laptop scheduling is therefore best-effort and C6 should say so in the UI and `c6 doctor`.

### Always-on Linux box

Run the same artifact as a supervised service or Compose stack. Bind the C6 backend to loopback or a private interface, terminate HTTPS at a reverse proxy/tunnel, persist the data directory, and automate verified backups. This is the recommended small-team deployment.

### AWS or another cloud VM

Run the same single-node artifact on a small virtual machine with a persistent attached disk. Expose only the HTTPS ingress, deny direct public access to the C6 backend, and snapshot or export the data directory. C6 does not require Kubernetes, RDS, object storage, or a vendor control plane merely because the host is in a cloud.

### Company

The single-authority shape is appropriate for a company because it has a clear policy and audit boundary and can later accept an OIDC or passkey identity adapter. The current product is not yet company-production-ready: recoverable login, administrator recovery/transfer, rate limiting, verified backup/restore, and hardened runtime isolation remain required gates.

## Reachability and tunnel policy

- The public origin must use trusted HTTPS before session, CLI, or Git credentials cross the network.
- Plaintext HTTP is acceptable only on loopback or an explicitly protected private hop.
- Tailscale, ngrok, Cloudflare Tunnel, Caddy, and conventional reverse proxies are optional adapters, not dependencies in the C6 trust model.
- Authenticated Git requests must not follow cross-origin redirects with credentials.
- A stable installation identifier must be distinct from its URL so clients can tell a moved installation from a replacement at a reused hostname.
- A proprietary tunnel can be an operator's long-term deployment choice without becoming C6 identity, authorization, or data authority.

## Portability instead of replication

The near-term availability and migration mechanism is verified backup/restore:

1. Stop or quiesce writes.
2. Export the complete SQLite and Git data root with its installation identifier.
3. Restore it on an always-on Linux box or cloud VM.
4. Configure the new stable HTTPS origin.
5. Explicitly rebind clients and Git remotes; revoke and reissue credentials when the threat model requires it.
6. Verify repository refs, project metadata, audit history, and authentication before retiring the old authority.

Only one restored copy may resume as the write authority. Running both copies as writable nodes would create an unsupported split brain.

## Extensibility boundaries

C6 should preserve narrow, typed seams rather than introduce a generic federation or plugin bus:

- ingress remains external and provider-neutral;
- authentication adapters resolve to the same installation-local principal;
- public project slugs resolve to installation-owned UUID repository paths;
- Git authorization remains a live local role decision;
- runners connect outbound and receive revision-pinned jobs with bounded grants;
- backup implementations operate on the declared SQLite/Git storage boundary;
- exported identifiers are qualified by installation ID where cross-installation meaning is needed.

Likely modules include OIDC/passkey authentication, remote-runner registration, ingress recipes, and backup targets. Each should be added only when its use case is demonstrated.

## Consequences

Benefits:

- no mandatory vendor account or hosted control plane;
- simple mental model, backup boundary, and failure behavior;
- ordinary offline Git continues to work;
- company administration and revocation remain understandable;
- laptop-to-server migration is possible without changing architecture;
- tunnels and cloud providers remain replaceable.

Costs:

- an unavailable authority means no Hub mutations, invites, fetch/push, administration, or new run scheduling;
- a laptop host is not a reliable always-on service;
- two installations cannot jointly accept authoritative writes for one C6 project;
- operators own ingress, uptime, and backup unless a third party offers those services.

These costs are explicit product constraints, not hidden failover promises.

## Alternatives considered

### True multi-writer P2P now

This could avoid choosing a canonical host, but would require a global identity and node-trust model, conflict semantics for every mutable entity, Git-ref reconciliation, partition-aware revocation, signed audit provenance, secret locality rules, scheduler leases, relay infrastructure, protocol versioning, and substantially harder recovery. It is disproportionate to current evidence.

### Mandatory hosted C6 login and control plane

This could simplify initial discovery and account recovery, but would make an open-source self-hosted installation dependent on a vendor service and introduce a global security and availability authority. Companies may eventually opt into an external identity module, but it should not be required.

### One authority with replaceable ingress (chosen)

This meets the demonstrated need—small software that can be installed and shared remotely—while keeping data and authority with the operator. It is also the cleanest base from which to add remote runners, mirrors, or carefully designed federation later.

## Evidence required to reconsider federation

Reopen this decision only after repeated observed demand demonstrates one or more of these conditions:

- independently administered organizations must collaborate on one project and none may host its canonical authority;
- users must accept Hub, permission, or pull-request writes through long partitions and later reconcile them automatically;
- data-residency rules require authoritative copies in several jurisdictions with defined conflict ownership;
- cross-install export, Git remotes, bundles, and read-only mirrors have failed and the remaining friction is a leading adoption blocker;
- a meaningful class of deployments cannot expose or tunnel a stable authority, and a relay/mesh demonstrably satisfies their policies;
- real runtime workloads require opportunistic peer execution and cannot be served by authority-managed remote runners.

Before federation begins, a new ADR must define global identity, installation trust, project home authority, conflict rules for every mutable entity, Git-ref reconciliation, revocation during partitions, audit provenance, secret ownership, scheduler leases, relay threats, disaster recovery, protocol compatibility, and a rollback path to one authority.

Until then, prefer remote runners, explicit Git remotes, bundles, export/import, and read-only mirrors. They solve narrower problems while preserving one write authority per project.
