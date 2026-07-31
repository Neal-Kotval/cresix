# Collaboration and sharing

## The collaboration model

C6 is decentralized across installations and centralized within each
installation. One installation is the authority for its local peers,
workspaces, roles, projects, Git refs, and collaboration records. Standard Git
gives contributors local history and offline work; C6 is the canonical remote
when reachable.

This is intentionally not multi-writer peer-to-peer replication. “P2P” in C6
means people can enroll directly and connect over a chosen network path without
a mandatory vendor account. It does not mean several servers reconcile roles,
audit, schedules, or competing ref updates.

## Native trust

The native sharing sequence is:

```text
one-time invitation -> local peer -> revocable session/credentials
                                      |
                                      v
                              current workspace role
                                      |
                                      v
                              authorized operation
```

Important boundaries:

- possession of an invitation is temporary enrollment authority;
- network reachability, IP address, LAN membership, and tunnel identity are not
  C6 identity;
- workspace roles do not grant C6 Admin access;
- browser, CLI, and Git credentials are separate and independently scoped; and
- current membership is read at operation time so revocation is effective.

## Sharing over distance

Teammates do not need to be on the same Wi-Fi. They need a trusted HTTPS route
to the authoritative installation. Standalone operators can choose a reverse
proxy, VPN, or tunnel such as Caddy, Tailscale, ngrok, or Cloudflare Tunnel.
Those services provide reachability and TLS; they do not replace C6 identity.
This path is configuration-supported and operator-owned; the repository does
not certify a public TLS/tunnel deployment end to end.

A laptop can be the host. When it sleeps, disconnects, or closes, Hub, Git, and
future scheduled work become unavailable. An always-on Linux machine is the
preferred small-team shape.

## Optional Cloud sharing

Cresix Cloud is intended to add a stable directory doorway:

```text
cresix.com/@{account}/{workspace}/{project}
```

Actual installation traffic must use a separate per-installation relay origin.
This preserves browser cookie and CSRF boundaries between unrelated C6 servers.
The connector dials outward, so the host need not expose an inbound port.

The directory is a projection, not authority. It may say a project exists or an
installation is online, but local C6 still decides who can open, clone, mutate,
or administer it. A Cloud account is not a local C6 login.

**Current qualification:** the loopback dogfood service has no public account
handle and temporarily routes `/{workspace}/{project}` with workspace
uniqueness inside its own database. Registration, binding, catalog, revocation,
and reverse transport are implemented, but the target route, public account,
and isolated-origin real-C6 browser journeys are not.

## Repository collaboration

**Now:** repository creation, browsing, separate read-only Git credentials,
clone/fetch/pull, and pull-request metadata.

**Next:** authenticated push, protected refs, revision-pinned review, atomic
merge rules, and durable Git audit. Until these exist, C6 is not a complete
replacement for a mature collaborative Git forge.

**Later:** forks, checks, merge queues, and broader project-management features
only when small-software workflows demonstrate the need.

## Company use

The one-authority shape can fit a company because access, revocation, audit,
backup, and incident ownership remain clear. The current implementation is not
company-production-ready. Material blockers include recoverable authentication,
administrator recovery/transfer, verified backup/restore, rate limiting,
production ingress operations, and hardened workload isolation.

Future OIDC, passkey, SCIM, or group synchronization should be adapters into
the local principal and authorization model. They should not create a parallel
permission system.

## Shareability without accidental publicity

“Easy to share” does not mean anonymous by default. The desired sequence is:

- a stable, legible link;
- an explicit trust or account boundary;
- a clear destination and current availability state;
- least-privilege access that can be revoked; and
- no transfer of hidden source, credentials, or local authority to a directory
  service.

Public projects may be added later, but they require an explicit public-access
model rather than treating an unguessable URL as authorization.
