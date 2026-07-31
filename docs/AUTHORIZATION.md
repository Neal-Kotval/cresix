# Authorization

Authentication establishes a peer; authorization is always read from durable
server state. Request bodies never choose their actor.

## Two authority levels

The immutable **server administrator** manages global trust: invitations,
peers, devices, sessions, and audit access. A workspace role—even `owner`—does
not grant these operations. This prevents workspace membership from escalating
to installation administration.

Projects inherit access from workspace membership. Roles are cumulative:

| Role | Adds intended capability |
| --- | --- |
| Consumer | Use a hosted app (gateway deferred) |
| Reader | Read source and fork (fork API deferred) |
| Runner | Record/cancel run intent |
| Contributor | Branch/PR/review collaboration |
| Maintainer | Publish/schedule/secret metadata/project management |
| Owner | Workspace membership, audit, and deletion policy |

The table describes the stable policy contract in `c6-core`. An action can be
authorized but still return `501` or a recorded-only result when its backend is
deferred. Authorization never converts an unavailable capability into a fake
success.

## Current API enforcement

- Public: health, installation status, bootstrap claim, and invitation
  redemption (unsafe requests still require exact `Origin`).
- Authenticated: session, visible workspace/project reads, and manifest
  validation. Validation is read-only and does not require CSRF, but its POST
  request still requires the exact browser `Origin`.
- Server administrator: trust lists/revocations, invitations, and global audit.
- Workspace owner/maintainer/etc.: workspace and project endpoints enforce a
  minimum cumulative role before reading or mutating.
- Project repository reads require at least reader membership.
- Workspace/project destructive deletion is intentionally `501` until SQL/Git
  erasure can be coordinated safely.

## Review rule

Every new endpoint needs tests for unauthenticated access, the minimum allowed
role, every lower denied role, revoked state, CSRF/origin on mutations, and an
audit record when security-relevant state changes.
