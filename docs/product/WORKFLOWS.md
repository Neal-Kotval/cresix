# Core workflows

These workflows describe product journeys and their current stopping points.

## 1. Start and claim a local C6

1. The operator starts the single-node service on loopback.
2. C6 emits a one-time bootstrap proof to the local startup channel.
3. The operator claims the installation and becomes its immutable server
   administrator.
4. C6 creates an authenticated browser session bound to the configured origin.
5. The operator enters Hub for projects or Admin for installation operations.

**Now:** implemented. The bootstrap proof must not be copied into durable logs
or documentation. There is no administrator recovery or transfer.

## 2. Create and clone a project

1. An authorized peer creates a workspace project.
2. C6 creates project metadata and a bare Git repository under its server-owned
   data root.
3. The peer authorizes the CLI and obtains a separate Git credential.
4. `c6 clone` discovers the canonical credential-free remote and invokes
   standard Git.
5. Git performs authenticated read-only smart HTTP.

**Now:** create, browse, clone, fetch, and pull are implemented. **Next:** push,
protected ref policy, revision-pinned review, and merge execution.

## 3. Invite a teammate

1. The server administrator creates a short-lived single-use invitation.
2. The invitation carries its bearer proof in the URL fragment.
3. The teammate reaches the same trusted HTTPS C6 origin and redeems it.
4. C6 creates the local peer/session and checks current roles on every action.
5. The teammate uses Hub, the CLI, and Git according to those roles.

**Now:** native invitation and session mechanics are implemented. The operator
must provide trusted HTTPS for non-loopback use. Same Wi-Fi or IP allowlisting
is neither required nor accepted as identity.

## 4. Share a standalone installation remotely

1. The operator runs C6 on a laptop or small server.
2. A reverse proxy, VPN, or tunnel supplies a stable trusted HTTPS origin.
3. `C6_PUBLIC_BASE_URL` is configured to that exact browser origin.
4. Peers use installation-local invitations and credentials.

**Now, configuration-supported:** C6 can enforce an exact external HTTPS origin,
but the operator owns and validates the proxy, VPN, tunnel, DNS, and TLS path;
the repository does not run a real public-ingress end-to-end test. Uptime equals
host and ingress uptime. Moving the public origin may require new browser
authentication and Git remote updates.

## 5. Connect an installation to Cresix Cloud

1. Claim the loopback Cloud preview and create a Cloud workspace.
2. Register an installation and copy the connector credential once.
3. Bind the Cloud workspace to a local workspace UUID.
4. Place the credential in an owner-only connector configuration.
5. Start the connector; it connects outbound and publishes a bounded catalog.
6. Inspect the Cloud directory doorway and connected/offline state.
7. Revoke the registration to terminate managed reachability.

**Now, dogfood:** these control-plane and connector mechanics work locally, and
reverse HTTP is tested against an authenticated compatible backend. **Next:**
production accounts and a real C6 browser journey on an isolated relay origin.
The current preview must not be exposed as public multi-tenant hosting.
Revocation is irreversible in dogfood: reissue, re-registration of the same
server, and rebinding the workspace are not implemented.

## 6. Declare an application, job, or schedule

1. Commit a versioned `c6.toml` with services, jobs, and policy.
2. Validate the declaration.
3. Record run, deployment, or schedule intent against a full revision.
4. Observe the explicit `dispatchAvailable: false` state.

**Now:** declaration, validation, and records. **Next:** an actual denied-by-
default runtime adapter, dispatch, durable logs, cancellation, recovery,
approvals, and secrets. No workload runs today.

## 7. Use a C6R in a larger project

Intended flow:

1. Select a cohesive C6R with a strict manifest.
2. Resolve an immutable Git revision and safe content closure.
3. Verify provenance and content digest.
4. Commit the exact result to `c6r.lock`.
5. Preview a deterministic diff.
6. Materialize passive content under a declared destination.
7. Update or remove it through a reviewable change.

**Now, design:** this workflow is specified but no command implements it.
**Next:** passive `content` and `agent_team` materialization. Active MCP, app,
service, job, and workflow kinds come only after runtime safety gates.

## 8. Move from laptop to server

Intended flow:

1. Quiesce writes and export the declared SQLite/Git data boundary.
2. Restore it on an always-on Linux box or cloud VM.
3. configure the new trusted HTTPS ingress.
4. Explicitly rebind clients and rotate credentials when appropriate.
5. Verify repository refs, metadata, audit, and authentication before retiring
   the old authority.

**Next:** the storage boundary exists, but end-to-end backup/restore verification
is still an open roadmap gate. Never run both restored copies as writable
authorities.
