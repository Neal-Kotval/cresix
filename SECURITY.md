# Security policy

Report suspected vulnerabilities privately to the repository owner. Do not
open a public issue containing exploit details, credentials, private source, or
data from a C6 installation.

## Deployment status

C6 is currently a development preview. Its control plane persists hashed
sessions, single-use invitations, current-role authorization, revocation, and
audit events. The `publicKey` enrollment value is opaque metadata: it is never
challenged and cannot authenticate a returning peer. Git network transport,
hosting, workload sandboxing, and encrypted secret values are not implemented.
**Do not expose this revision to the public internet or run untrusted code.** A
healthy process is not evidence of a secure deployment.

The intended first security boundary is one self-hosted installation serving a
small group whose members trust one another, while still protecting against
mistakes and stolen or revoked devices. Container isolation is not presented as
a sufficient boundary for hostile multi-tenancy.

## Identity and network model

C6 native v1 uses an owner-created, expiring, single-use invitation to issue a
revocable cookie session and local role record. The cookie is the authentication
credential. Device labels and `publicKey` values are metadata only. The server
address, source IP, LAN membership, VPN membership, and possession of a project
URL never establish identity.

There is no re-login or administrator recovery proof. The identity created by
bootstrap is the immutable server administrator; a workspace `owner` does not
gain global administration. An authenticated `GET /api/v1/session` renews the
session and cookies for 30 days. If that sole session is lost or revoked, or
expires after 30 days without renewal, invitation, peer, session, and audit
administration is locked out. Editing SQLite is not a supported recovery
procedure.

A regular invited peer who loses their cookie can receive a new invitation
from the still-authenticated server administrator. This creates a new local
peer record; it does not recover the old identity or the server administrator.

The current control-plane tests cover a mode-`0600` bootstrap-token file and its
post-claim deletion, invitation expiry and replay, hashed sessions, cookie
flags, CSRF double-submit checks, strict origin checks, peer revocation,
cumulative roles, and transactional audit events. That scope does not cover Git
transport or workload execution.

Public exposure remains blocked until C6 adds and verifies:

- durable re-authentication and an explicit owner recovery mechanism;
- throttling for bootstrap, pairing, and session abuse;
- authenticated Git transport with branch-level authorization;
- enforced workload isolation and encrypted, scoped secret injection.

## Invariants

Security-sensitive implementation must preserve these boundaries:

- The web/control-plane process never receives a Docker socket.
- Only the runner receives narrowly typed, authenticated, revision-pinned work.
- A project cannot access another project's filesystem, database, bucket,
  secrets, processes, or network namespace.
- Secret values are individually granted and never copied into Git, forks,
  logs, manifests, errors, command arguments, or artifacts.
- The trusted gateway strips client-supplied identity headers before generating
  its own authenticated identity assertion.
- Agents cannot write directly to a protected/default branch.
- Unknown runner outcomes are surfaced as interrupted and are not retried
  automatically.
- The SQLite control store and Git data directory are private to the C6 service
  user and are never served as static files or mounted into workloads.

## Operator responsibilities

Keep C6 and its dependencies pinned and patched, protect `.env` and backups,
restrict host and volume access, expose only the HTTPS gateway, and review audit
events. Do not mount the Docker socket into the control plane, share personal
Codex/ChatGPT session files, or use IP allowlisting as authentication.

See [the operator guide](docs/OPERATIONS.md) for the current deployment boundary
and recovery guidance.
