# Native trust model

## Implemented enrollment

On first initialization, C6 creates a server ID and 256-bit bootstrap token. By
default the plaintext token exists only in mode-`0600`
`${C6_DATA_DIR}/bootstrap-token`; SQLite stores its SHA-256 hash. A successful
claim atomically creates the immutable server administrator, device record,
session, and audit event, deletes the hash, then removes the token file.

The administrator can issue an invitation for 1–10,080 minutes with one of the
six workspace roles. Only the token hash is persisted. The returned URL uses
`/join#token=...`, so browsers do not send the bearer value in HTTP request
paths or referrers. Redemption atomically consumes the invitation, creates a
new peer/device/session, and adds workspace membership when scoped.

## Session proof

C6 v1 authenticates the `c6_session` cookie—not a device key. The cookie is
`HttpOnly`, `SameSite=Strict`, path `/`, and `Secure` when the public URL is
HTTPS. `c6_csrf` is readable by same-origin JavaScript and must match both the
`X-C6-CSRF` header and the hash bound to the session for unsafe requests.

`GET /api/v1/session` requires both still-valid cookies, atomically extends the
database expiry by 30 days, and reissues both cookies. Revoked/expired peers,
devices, or sessions cannot renew.

## Explicit limitations

- `publicKey` is unique opaque enrollment metadata; C6 never challenges it.
- There is no password, passkey, external identity, or key-based re-login.
- Re-inviting a regular peer creates a new identity; it does not recover the old
  one.
- The bootstrap identity is the immutable server administrator. Workspace
  owners cannot replace it.
- Losing/revoking the administrator cookie, or letting it expire without a
  session read for 30 days, permanently locks global administration.
- Source IP, LAN/VPN membership, and server URL possession never grant access.

These constraints make the release appropriate for local evaluation, not a
durable company identity system.

## Revocation

The administrator can revoke peer, device, and session records. Authentication
queries check current revocation state on every request, so revocation takes
effect without waiting for cookie expiry. Self-revocation and administrator
edge cases are guarded to avoid accidental privilege loss where implemented;
there is still no recovery bypass.
