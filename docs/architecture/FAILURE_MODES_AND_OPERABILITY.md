# Failure modes and operability

## Operating principle

Cresix prefers explicit unavailability to hidden failover, duplicated work, or
ambiguous authority. It has no automatic promotion of a second writer and no
automatic retry of a mutation with unknown outcome.

## Failure matrix

| Failure | Observable effect | Data/authority consequence | Operator response |
| --- | --- | --- | --- |
| C6 process stops | Hub, API, Git, scheduling unavailable | SQLite/Git remain on disk | Restart same release; inspect logs/health |
| Laptop sleeps or loses network | Remote access unavailable | Local authority remains canonical | Wake/reconnect or migrate to always-on host |
| HTTPS ingress fails | Remote clients cannot reach C6 | Local loopback and data survive | Repair/replace ingress; do not bypass auth |
| Cloud stops | Directory and managed relay unavailable | Local C6 and standalone ingress survive | Restart Cloud; connectors re-establish presence |
| Connector disconnects | Directory reports offline; in-flight relay fails | Local state unchanged | Inspect credential/revocation/network; reconnect |
| Installation revoked | Relay session terminates and cannot reconnect | Local installation remains usable; dogfood registration and binding stay reserved | Treat as irreversible in this preview; re-registration/rebinding recovery is unimplemented |
| Relay exchange times out/fails | Client gets explicit error; connection resets | No automatic replay | Caller decides whether a new request is safe |
| SQLite busy/corrupt | Mutation fails or startup cannot proceed | No fallback database authority | Stop writes, preserve files, restore verified backup |
| Git ref moved concurrently | Conditional update fails | Existing ref remains authoritative | Refresh and explicitly reconcile revisions |
| Audit insert fails | Security-sensitive mutation aborts | State and audit stay aligned | Repair storage before retry |
| Runner unknown outcome | Future run becomes `interrupted` | Never assumed failed or retried | Human inspects and chooses a new run |
| Secret provider unavailable | Future run preparation fails closed | No cached ambient fallback | Restore provider or explicitly change configuration |

## Health and diagnostics

The implemented local service exposes `/healthz`; Cloud/connector tests cover
connection, offline, and revocation states. Health should mean process
readiness, not that an installation is publicly safe, backed up, or able to
execute workloads. Capability responses must expose unavailable features such
as `dispatchAvailable: false`.

Logs exclude credential material and should avoid request paths, queries,
headers, and bodies at relay boundaries. Operators should collect bounded
service logs, disk usage, SQLite errors, connector state, backup age, and
certificate/ingress health. Production Cloud additionally needs per-route
admission metrics, rate-limit signals, abuse telemetry, and multi-node presence
diagnostics; these do not exist in dogfood.

## Upgrade, rollback, and recovery

Before upgrade, record the current source/release, stop services, and take a
coherent backup. Embedded migrations are forward startup actions; therefore
the operator needs a tested restore path, not an assumption that an older
binary can open a newer database. Verify server identity, claim state,
projects, Git reads, schedules, and audit after restore.

Changing the public URL changes cookie/Origin expectations and may require
re-authentication and Git/CLI rebinding. Running the old and restored copies as
writers creates unsupported split brain.

## Operational readiness gaps

Before company production use, Cresix needs tested administrator recovery or
transfer, login and request rate limiting, backup/restore drills, release and
migration policy, alerts, storage exhaustion handling, credential rotation,
security response, and hardened HTTPS deployment guidance. Production Cloud
also needs relay HA, isolated origins, multi-node presence, and abuse controls.
Dogfood installation revocation is intentionally fail-closed but currently
irreversible: there is no credential reissue, registration replacement, or
workspace rebinding API.

Runtime readiness is a separate gate: Docker isolation policy, resource and
egress enforcement, provenance, secret injection/redaction, cancellation,
reconciliation, and recovery drills must all work before execution is labeled
available.

Use the concrete [Operations runbook](../OPERATIONS.md),
[Deployment guide](../DEPLOYMENT.md), and [Testing guide](../TESTING.md).
