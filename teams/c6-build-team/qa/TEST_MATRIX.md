# C6 release test matrix

Use the relevant rows during feature work and all blocking rows before release.

| Surface | Required scenarios | Gate |
| --- | --- | --- |
| Manifest | Valid web/cron/agent; unknown field; duplicate name; unknown secret | Rust tests |
| Bootstrap | Private token file; no log leak; one owner; invalid proof; restart; replay | Rust/API/security |
| Sessions | HttpOnly/SameSite; CSRF; wrong Origin; fixation; revoke; expiry | Rust/API/security |
| Authorization | Each role allowed action; lower role and cross-workspace denied | Rust/API tests |
| Pairing | Valid invite; bounded role; expiry; replay; revoked device | Security tests |
| Git | Create/import; branch/history/tree/read; option/path/ref injection; conflicts | Rust/integration |
| Pull requests | Authorization; merge race/conflict; closed/merged immutability | Integration |
| Publishing | Revision pin; failed intent; explicit terminal status | Integration |
| Scheduler | Restart; duplicate tick; timezone/DST; forbid/allow/replace | Integration |
| Runner protocol | Key-file mode; auth mismatch; invalid frame; bounds; timeout; cancel; socket mode | Rust/isolation |
| Web flows | First boot; pairing; project/source; publish; schedule; run; revoke | Playwright |
| Web resilience | Direct URL; history; loading/empty/403/404/500/offline | Vitest/Playwright |
| Web access | Keyboard/focus; landmarks/names; reduced motion; narrow viewport | Playwright |
| Operations | Fresh install; restart persistence; backup/restore; bad data dir | Smoke/system |
| Dogfood | Fresh claim; `cresix` project; Git reads; run boundary; restart; real browser | Dogfood |
| Cloud identity | Loopback bootstrap; host-only cookie; Origin/CSRF; ownership; session revoke | Cloud/security |
| Cloud namespace | Validation; collision; enumeration-safe errors; immutable ID/route | Cloud tests |
| Installation | One-time credential; hashed verifier; cross-owner denial; revoke/fence | Cloud/security |
| Catalog | Owner/binding authorization; bounds; monotonic revision; no local mutation | Cloud tests |
| Connector | Owner-only config; fixed loopback upstream; no credential/header forwarding | Connector/security |
| Relay protocol | Invalid/duplicate/late frames; bounds; serial dogfood admission; protocol concurrency ceiling; timeout; cancellation | Contract/integration |
| Route ingress | Unknown/offline/revoked route; authority-derived selection; overload/failure status | Integration |
| Cloud web | Claim; workspace; installation; credential reveal; directory; disconnected/revoked | Vitest/Playwright |
| Connected dogfood | Fresh Cloud plus authenticated C6-compatible backend; real connector/relay; directory handoff; revoke; standalone survives | Dogfood |
| Documentation | Required product/architecture pages; spec status metadata; relative links; compatibility stubs | Docs contract |

The MVP's runner may simulate execution. Container escape, real workload egress,
and cross-project runtime isolation are release gates only when a real execution
backend is introduced; they must not be marked passing against a simulator.

The connected Cloud component gate may pass before a real relay lifecycle gate
exists. It must not be reported as production reachability, public account
security, relay HA, or end-to-end encryption. The delivery report names every
unimplemented or unexecuted connected row explicitly.

## Evidence format

For every executed check record the exact command, result, environment, and any
skipped scenario. A passing aggregate command does not replace targeted evidence
for the change's highest-risk behavior.
