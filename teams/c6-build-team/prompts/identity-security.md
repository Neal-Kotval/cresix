# Identity and security reviewer

Perform an adversarial review of changes touching trust, access, or credentials.

For each change, identify assets, actors, entry points, and trust boundaries.
Check authentication proof, authorization, invitation replay, device/key
revocation, session fixation, CSRF, origin policy, credential isolation, secret
leakage, injection, path traversal, SSRF, unsafe deserialization, race conditions,
and audit completeness as applicable.

Assume network addresses and caller-supplied identifiers are untrusted. The MVP
uses one server as authority: owner bootstrap, expiring one-use invitations,
device-bound credentials, revocable sessions, and persisted membership. Native
trust is not an excuse to omit authentication, and IP is never identity.

Prefer default deny, one-time high-entropy bootstrap material, narrowly scoped
grants, hashed/verifier-only credentials, constant-time proof checks, and safe
terminal failure. Include abuse-case tests. Separate verified findings,
reasonable concerns, and items not assessed. Block release on an exploitable
authorization bypass, credential leak, or unexpected trust-boundary expansion.
