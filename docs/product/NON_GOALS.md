# Non-goals and deliberate constraints

Non-goals keep Cresix from becoming a vague promise to replace every forge,
cloud, and agent platform. Some may be revisited; none should be smuggled in as
an incidental implementation detail.

## Not Big Software infrastructure

Cresix is not optimizing first for millions of users, multi-region writes,
elastic fleets, or many independent control-plane replicas. A single
installation, SQLite, bare Git, local storage, and replaceable ingress are the
default until measured workloads require more.

## Not GitHub feature parity

C6 borrows the legibility of a software forge, not its entire product surface.
It does not currently replace:

- issues and general project management;
- CI checks, merge queues, or broad marketplace actions;
- social discovery, stars, or public profiles;
- LFS, SSH Git, anonymous Git, or arbitrary Git server features; or
- organization-scale policy and compliance suites.

Features should be added when they make small software easier to create, run,
share, or compose—not merely because another forge has them.

## Not peer-to-peer control-plane replication

C6 supports direct peer enrollment and flexible networking. It does not let
multiple servers accept authoritative writes and later reconcile identity,
roles, Git refs, schedules, audit, or secrets. Git already distributes source
work; one server retains control-plane authority.

## Not mandatory SaaS

Standalone C6 must remain useful without a Cresix account. Target Cresix Cloud
may own globally coordinated namespace and managed reachability, but cannot
become an undeclared dependency for local startup, local authorization, Git
data, or recovery.

## Not source-IP authentication

Same Wi-Fi, a known IP, VPN membership, possession of a URL, or tunnel-provider
identity does not grant a C6 role. Network controls may reduce exposure, but
identity and authorization remain explicit application decisions.

## Not execution by declaration

The current product records manifests, schedules, runs, and deployment intent;
it does not execute applications, cron jobs, agents, or arbitrary commands.
Docker is a future runtime adapter, not a synonym for a secure runtime.

## Not a secret manager today

C6 does not store or inject secret values. Future native encryption or
1Password/Doppler references must include key backup, per-run grants, injection,
redaction, revocation, and failure semantics. Source repositories and C6R locks
must never contain secret values.

## Not ambient Codex credential hosting

An operator's Codex CLI token or environment is not a safe shared runtime
credential. Future agent jobs require separately granted, scoped credentials
with an explicit owner, lifetime, runtime, network policy, and audit trail.

## Not an arbitrary plugin installer

C6Rs do not execute install hooks or bypass parent policy. Passive materialization
comes before active tools. A C6R containing an MCP server, app, service, or job
is source plus declarations until separately activated under runtime controls.

## Not hostile multi-tenancy yet

The current trust boundary assumes a small trusted team. Containers can reduce
accidents but do not alone isolate malicious code. Public user-generated
workloads require stronger isolation, abuse controls, quotas, incident response,
and an application origin separated from Cloud and C6 authentication domains.

## Not production Cresix Cloud yet

The loopback Cloud implementation is a dogfood vertical slice. It does not
provide production account enrollment/recovery, global operation across
deployments, isolated real-C6 browser routing, rate limiting, relay HA, custom
domains, billing, or public multi-tenant hardening.

## Not availability by implication

A laptop that is asleep is offline. A single server that fails is unavailable.
A stale directory entry is not proof of service health. Cresix should state
these outcomes plainly and prefer verified backup/restore before claiming more.
