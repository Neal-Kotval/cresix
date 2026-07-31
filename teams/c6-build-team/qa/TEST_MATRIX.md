# C6 release test matrix

Use the relevant rows during feature work and all blocking rows before release.

| Surface | Required scenarios | Gate |
| --- | --- | --- |
| Manifest | Valid web/cron/agent; unknown field; duplicate name; unknown secret | Rust tests |
| Authorization | Each role allowed action; every lower role denied | Rust/API tests |
| Pairing | Valid invite; expiry; replay; wrong origin; revoked device | Security tests |
| Git | HTTPS/SSH clone and push; protected branch denial; attribution | Integration |
| Pull requests | Preview isolation; merge race; closed/merged immutability | Integration |
| Publishing | Revision pin; failed health check; explicit rollback | Integration |
| Scheduler | Restart; duplicate tick; timezone/DST; forbid/allow/replace | Integration |
| Agent | Missing credential; proposal-only write; timeout; egress denial | Isolation |
| Project data | Cross-project database, bucket, volume, and network denial | Isolation |
| Web | Loading, empty, unauthorized, failed, narrow viewport, keyboard | UI/E2E |
| Operations | Fresh install; upgrade; backup; restore; disk exhaustion | System |

## Evidence format

For every executed check record the exact command, result, environment, and any
skipped scenario. A passing aggregate command does not replace targeted evidence
for the change's highest-risk behavior.

