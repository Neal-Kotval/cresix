# Local regression and abuse coverage

This matrix is intentionally specific. A checked row must be exercised by an
automated test against real C6 behavior; documentation or a mock response does
not count.

| Area | Required behavior | Acceptance boundary |
| --- | --- | --- |
| Documentation | Canonical handbooks exist; local links resolve; specifications expose status; legacy overview links do not become competing ledgers | Offline documentation contract |
| Bootstrap | A fresh server is unclaimed; exactly one valid owner claim wins | Real server process + embedded database |
| Bootstrap replay | Reusing or racing a claim credential cannot create another owner | Real HTTP requests |
| Persistence | Owner, invite, project, and revocation state survive restart | Restart with the same `C6_DATA_DIR` |
| Invites | One-time, expiring invite; assigned role cannot be escalated by request body | Real HTTP requests and database state |
| Sessions | Secure cookie attributes; unknown/revoked/expired sessions fail closed | HTTP response headers and API |
| CSRF/origin | Cookie-authenticated mutations require a matching CSRF token and trusted origin | Cross-origin and missing-token requests |
| Authorization | Viewer/member/admin/owner actions are server-enforced | Separate real peer sessions |
| Peer lifecycle | Device/session revocation takes effect immediately | Existing cookie replay |
| Repository | Reject traversal, invalid refs, unsafe paths, and conflicting writes | `c6-git` tests and repository API when exposed |
| Git smart HTTP | Authenticated real clone/fetch/ls-remote; cookie, wrong class/token, revoked token, and receive-pack fail closed | Real server plus installed Git client |
| CLI | Server pinning, stdin-only login, project discovery, clone/remote helper wiring, owner-only local state, restart | Real `c6` and `git-credential-c6` processes |
| Runner framing | Reject malformed JSON, unknown operations, oversized frames, and invalid IDs | Real Unix socket |
| Runner replay | Duplicate request IDs do not execute twice | Real Unix socket |
| Runner limits | Timeout and output bounds produce terminal structured results | Real Unix socket |
| Website | Onboarding, projects, repository, peers, schedules, runs, settings, error/empty states | Headless Chromium |
| Packaging | Compose model renders without starting services | Docker Compose parser |

## Explicit non-claims

The local gate cannot prove host hardening, public-internet TLS termination,
container isolation, disaster recovery, or resistance to a compromised host.
Those require deployment-specific controls and separate security testing. A
simulated runner backend is tested only for its protocol and policy behavior;
it is not evidence of production-grade workload isolation.
The Git/CLI journey uses loopback HTTP and the explicit owner-only plaintext
credential-store fallback. It does not validate a platform keychain, public TLS,
large-pack resource exhaustion, concurrent fetch limits, or Milestone 2.2 push
policy. Receive-pack is expected to remain unavailable.
The Phase 2.1 journey also does not cover credential expiry, peer/device
revocation cascading to issued tokens, concurrent Git capacity, descendant
process cleanup after forced termination, or CLI server-pin behavior after an
installation is replaced at the same origin.
