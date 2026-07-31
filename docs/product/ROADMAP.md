# Product roadmap

This roadmap is ordered by coherent user outcomes. “Next” is not a date or a
guarantee; work advances when its acceptance and security gates can be met.
The [capability ledger](CAPABILITIES.md) remains the sole status authority.

## Now: a truthful local forge foundation

The current product can:

- run one sovereign local C6 authority;
- bootstrap an immutable administrator and enroll trusted peers;
- persist workspaces, projects, roles, repository metadata, pull-request
  records, schedules, run/deployment intent, and audit events;
- create and browse bare Git repositories;
- clone/fetch/pull through authenticated read-only smart HTTP;
- use a thin CLI for aliases, authentication, project listing, clone, remote
  setup, diagnostics, and JSON reads;
- validate project declarations without executing them;
- exercise a separate authenticated simulation runner protocol; and
- dogfood a loopback Cloud directory, registration, binding, catalog,
  connector, revocation, and serial reverse-HTTP path.

The current design also defines C6R composition and agent-first runtime
boundaries, but those designs are not shipped runtime capabilities.

## Next horizon 1: complete trustworthy source collaboration

Outcome: a small team can use C6 as the canonical collaborative Git forge for a
private project.

Required slices:

- authenticated protected push with atomic expected-ref policy;
- revision-pinned pull-request review and fast-forward-only merge;
- branch protection and durable read/write Git audit;
- production rate limits on credential and Git boundaries;
- complete workspace membership management; and
- verified backup, restore, and laptop-to-server migration.

Why first: source collaboration is the foundation on which runtime, agent
proposals, and reusable components depend.

## Next horizon 2: passive C6R composition

Outcome: teams can safely reuse wikis, templates, policies, and agent-team
definitions inside C6 or larger GitHub/Codeberg projects.

Required slices:

- strict `c6r.toml` parsing and schema errors;
- safe subdirectory-closure construction;
- immutable Git resolution and SHA-256 content digest;
- committed deterministic `c6r.lock`;
- status, validation, update preview, and removal commands;
- passive `content` and `agent_team` materialization; and
- negative tests for traversal, symlinks, hooks, submodules, credentials, and
  hostile rendered content.

No active command runs in this horizon.

## Next horizon 3: production-shaped connected sharing

Outcome: a remote teammate can use a stable Cresix doorway to reach a sovereign
C6 installation without the operator opening an inbound port.

Required slices:

- production account enrollment, passkey/OIDC login, recovery, recent-auth
  checks, throttling, abuse controls, and security notifications;
- globally unique account handles, account-scoped workspace uniqueness, and
  rename/tombstone/migration policy for the three-segment directory route;
- isolated per-installation relay origins;
- a real browser session through the relay into real C6;
- reconnect, concurrency, overload, offline, revoke, and standalone-fallback
  journeys;
- production TLS, origin, logging, incident, and presence operations; and
- a recipient-facing sharing and membership model that does not turn Cloud
  identity into implicit local authorization.

## Next horizon 4: bounded application and job runtime

Outcome: a trusted team can run revision-pinned small software without giving
the C6 control plane ambient host execution.

Required slices:

- denied-by-default Docker runtime adapter;
- pinned revisions and image/config digests;
- resource, filesystem, capability, and network controls;
- authenticated application gateway and health-gated publish;
- durable run logs, cancellation, interruption recovery, and immutable rollback
  semantics; and
- schedule occurrences, leases, missed-run and concurrency rules, plus approvals.

## Next horizon 5: secrets and agent proposals

Outcome: an agent can perform bounded work and return a reviewable source
proposal.

Required slices:

- AEAD-encrypted native secret values with a separately backed-up master key;
- 1Password/Doppler reference adapters where demanded;
- per-job grants, run-scoped injection, and log redaction;
- capability documents, cursors, polling, and retry-safe mutations;
- pinned agent runtimes with separately granted credentials; and
- service-identity proposal branches linked to runs and pull requests.

## Later: earned expansion

Only after the earlier outcomes work in real deployments:

- thin MCP access to stable authenticated C6 HTTP operations;
- activation of C6R tools, MCP servers, mini apps, services, jobs, and workflows;
- company OIDC, SCIM, group sync, retention, and compliance modules;
- remote runners and stronger microVM isolation;
- public projects and carefully isolated hosted applications;
- workload databases, object buckets, or registries;
- read-only mirrors, import/export, and cross-install collaboration; and
- federation or high availability only if single-authority deployments produce
  concrete unmet requirements.

## Sequencing rule

Do not pull a “Later” capability forward merely because its UI or schema is
easy. Pull it forward only with the authority, failure, migration, rollback,
security, and regression behavior that makes the user-facing claim true.
