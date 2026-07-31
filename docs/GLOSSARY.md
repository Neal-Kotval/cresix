# Glossary

- **C6 / cresix:** C6 is the product; `cresix` is its repository and Compose
  project name.
- **Installation:** one authoritative C6 server, SQLite store, and Git root.
- **Standalone mode:** C6 running without a Cresix account or Cresix-operated
  dependency; the operator owns any remote HTTPS ingress.
- **Connected mode:** optional Cloud directory and managed reachability layered
  onto an otherwise sovereign local installation.
- **Cresix Cloud:** separate hosted authority for global accounts, namespaces,
  installation registrations, catalog projections, and relay presence. It is
  not source, runtime, secret, or local-authorization authority.
- **Cloud account:** hosted identity used for global namespace and installation
  administration. It is not a local C6 peer or login in the first slice.
- **Cloud workspace:** globally unique directory namespace with Cloud
  membership; it binds to, but does not replace, one local workspace.
- **Workspace binding:** mapping from a Cloud workspace to an installation and
  installation-local workspace UUID. It never grants a local role.
- **Catalog projection:** bounded, versioned project discovery metadata
  published by an installation. It is eventually consistent and never source
  or authorization truth.
- **Connector:** separate least-privilege process that authenticates outbound to
  Cloud and proxies only one configured loopback C6 origin.
- **Relay origin:** opaque per-installation browser origin selected by the Cloud
  route registry. It is intentionally distinct from `cresix.com` account cookies.
- **C6 Hub:** web surface for workspace and project collaboration. Its canonical
  first-slice routes are `/` and `/projects/*`. Workspace member management is
  not implemented yet.
- **C6 Admin:** web surface for installation operations. Its canonical
  first-slice routes are `/admin` and `/admin/access`; it is not another server.
- **`c6` CLI:** implemented thin client of the C6 HTTP API and standard Git.
  It owns local aliases/credentials but never control-plane state.
- **CLI credential:** expiring `c6c_v1_...` Bearer token capped by API scopes,
  resource restrictions, and live authority.
- **Git credential:** separate expiring `c6g_v1_...` Basic password for
  read-only smart HTTP; username is exactly `c6`.
- **Server administrator:** immutable peer created by bootstrap. This identity
  alone manages global invitations, peers, workspace creation, and the global
  audit log. Device and session endpoints are currently self-service.
- **`serverAdministrator`:** explicit authenticated-session capability used to
  gate C6 Admin. It is never inferred from a workspace role.
- **Peer:** local person record created by bootstrap or invitation redemption.
- **Device record:** label plus opaque `publicKey` metadata attached at
  enrollment. It is not proof of possession or a login method.
- **Session:** hashed 30-day bearer credential stored in an `HttpOnly` cookie,
  paired with a CSRF cookie. Valid session reads renew the 30-day window.
- **Invitation:** hashed, expiring, single-use bearer token created by the
  server administrator. Its URL token is kept in the fragment.
- **Workspace:** membership and role boundary for projects.
- **Workspace owner:** highest workspace role. It is not server administration.
- **Project:** workspace-owned metadata plus a C6-created bare Git repository.
- **Revision:** full lowercase SHA-1 or SHA-256 Git object ID.
- **Pull request:** persisted proposal metadata. Merge execution is deferred.
- **Manifest:** versioned `c6.toml` declaration of build, services, jobs, and
  policy; validation does not execute it.
- **Deployment/run intent:** audited record pinned to a revision with
  `dispatchAvailable: false`.
- **Schedule:** validated five-field cron and IANA timezone record. No daemon
  dispatches it in this release.
- **Occurrence:** deterministic `(schedule_id, occurrence_at)` scheduling key.
- **Runner:** separate Unix-socket daemon implementing an authenticated typed
  protocol with a simulation backend.
- **Simulation:** lifecycle/log/cancellation behavior that never invokes a host
  command, Docker, or a project workload.
- **Runtime adapter:** deferred deny-by-default execution backend contract.
  Docker is the intended first adapter but is not implemented or sufficient
  alone for hostile multi-tenancy.
- **Ingress:** operator-provided HTTPS reachability such as Caddy, a VPN, or
  ngrok. It is never C6 identity or authority.
- **Secret metadata:** validated name showing intended configuration. Secret
  value storage returns `501 Not Implemented`.
- **Public base URL:** exact browser origin used for cookies, mutations, and
  invitation links. It does not enable TLS on the C6 listener.
