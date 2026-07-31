# HTTP API

C6 serves JSON under `/api/v1`, the React application, and `/healthz` from one
origin. Unknown `/api` paths return a structured JSON `404`; they never fall
through to the SPA. API responses use `Cache-Control: no-store` and all
responses receive content-type, referrer, permissions, and CSP headers.

## Conventions

- Default request body limit: 64 KiB; manifest validation: 1 MiB.
- Unsafe methods require an `Origin` exactly equal to `C6_PUBLIC_BASE_URL`.
- Authenticated state-changing methods also require matching `c6_csrf` cookie
  and `X-C6-CSRF` header. Authenticated manifest validation is a read-only POST
  and is the explicit exception; exact Origin still applies.
- Errors use `{"error":{"code":"...","message":"..."}}`.
- `400` means invalid input, `401` missing/expired authentication, `403`
  insufficient authority/CSRF/origin, `404` absent resource, `409` state
  conflict, `501` deliberately unavailable behavior, and `500` opaque failure.

## Endpoint map

| Surface | Endpoints | Current behavior |
| --- | --- | --- |
| Status | `GET /healthz`, `GET /api/v1/status` | Public health/version and claim state |
| Enrollment | `POST /bootstrap/claim`, `POST /invites/redeem` | Issue cookie sessions |
| Session | `GET|DELETE /session` | Renew/read or revoke current session |
| Trust admin | `GET|POST /invites`, `GET /peers`, `DELETE /peers/{id}`, `GET /devices`, `DELETE /devices/{id}`, `GET /sessions`, `DELETE /sessions/{id}` | Server-administrator only |
| Credentials | `GET|POST /credentials`, `DELETE /credentials/{id}` | Current peer lists/issues/revokes expiring CLI or read-only Git credentials; plaintext shown once |
| CLI | `GET /cli/whoami` | `api:read` Bearer identity and workspace summary |
| Workspaces | `GET|POST /workspaces`, `PUT|DELETE /workspaces/{id}` | Durable metadata; delete deferred |
| Projects | `GET|POST /projects`, `GET|PUT|DELETE /projects/{id}` | Durable metadata and seeded Git; delete deferred |
| Collaboration | `GET|POST /projects/{id}/pull-requests` | Persist/read PR records; merge deferred |
| Runtime metadata | `GET|POST` project `deployments`, `runs`, `schedules`, `secrets` | Recorded intent; schedules validated; no dispatch |
| Repository | project `repository/branches`, `commits`, `tree`, `files/{path}`, `remote` | Bounded JSON reads and credential-free canonical clone URL/capabilities |
| Audit | `GET /audit` | Latest installation audit records, administrator only |
| Manifest | `POST /manifest/validate` | Authenticated parse/validation without execution; exact Origin, no CSRF |

All table paths after the first column are relative to `/api/v1`.

## Mutation bodies

JSON uses camel case unless shown otherwise:

| Operation | Required/optional fields |
| --- | --- |
| Claim/redeem | `token`, `displayName`, `deviceLabel`, `publicKey` (opaque metadata, 32–8192 characters) |
| Create invite | `role`; optional `expiresInMinutes` (1–10,080), `workspaceId` |
| Create/update workspace | `slug`, `name` |
| Create/update project | `workspaceId`, `slug`, `name`; optional `description`, `defaultBranch` |
| Create pull request | `title`, `sourceBranch`; optional `body`, `targetBranch` |
| Record deployment | `revisionSha`; optional `environment` |
| Record run | `job`, `kind` (`command`, `cron`, `agent`); optional `revisionSha` |
| Create schedule | `job`, `cron`, `timezone`; optional `concurrency` (only `forbid`), `enabled` |
| Create secret metadata | `name` using uppercase letters, digits, and underscore |
| Validate manifest | `source` containing the TOML string |
| Create credential | `type` (`cli` or `git`), `label`, `scopes`; optional `expiresAt`, `restriction.workspaceId`, `restriction.projectId` |

Resource paths use server UUIDs, not workspace/project slugs. Repository commit,
tree, and file reads accept `revision` (default `main`); commit queries also
accept `limit`, and tree queries accept `recursive`. Values are still bounded
and validated by the Git layer.

Credential expiry defaults to 30 days and may not exceed 90 days. CLI
credentials accept `api:read` and `api:write`; the currently implemented CLI
uses read endpoints. Git credentials accept only `git:read`. `git:write` and
push are not available. Token type, scope, current peer/device state, live
workspace role, expiry/revocation, and optional resource restriction are all
checked at request time. No credential grants server-administrator authority.

## Non-browser authentication

The CLI uses `Authorization: Bearer c6c_v1_...`. Bearer requests do not use
CSRF; if they include `Origin`, it must still match. The Git router is outside
browser Origin/CSRF middleware and accepts only Basic username `c6` plus a
`c6g_v1_...` password. Mixed cookies and Authorization credentials fail.

Git smart HTTP is not JSON and is intentionally narrow:

```text
GET  /git/{workspace}/{project}.git/info/refs?service=git-upload-pack
POST /git/{workspace}/{project}.git/git-upload-pack
```

Receive-pack/push is not routed or advertised. See [Git](GIT.md) and
[CLI](CLI.md).

Read-only Git transport is disabled unless the operator sets
`C6_GIT_HTTP_ENABLED=1`. `/projects/{id}/remote` truthfully reports
`capabilities.fetch: false` while disabled and always reports `push: false` in
this phase.

## Cookie and CSRF flow

Use a cookie jar only on a trusted local machine. Obtain the bootstrap token as
described in [Operations](OPERATIONS.md), then claim:

```bash
export C6_URL=http://127.0.0.1:8787
read -rsp 'Bootstrap token: ' TOKEN; echo
printf '%s\n' "$TOKEN" | jq -Rn \
  --arg enrollment "c6-docs-$(openssl rand -hex 24)" \
  '{token:input,displayName:"Operator",deviceLabel:"docs",publicKey:$enrollment}' |
  curl --fail --silent --show-error \
    -H "origin: $C6_URL" -H 'content-type: application/json' \
    -c c6.cookies --data-binary @- "$C6_URL/api/v1/bootstrap/claim" |
  tee claim.json | jq .
unset TOKEN
```

The response and readable cookie contain the CSRF value. For a mutation after
reload, read `c6_csrf` from the cookie jar without printing it and send it in
both the jar and header:

```bash
CSRF="$(awk '$6 == "c6_csrf" {print $7}' c6.cookies)"
curl --fail --silent --show-error -b c6.cookies \
  -H "origin: $C6_URL" -H "x-c6-csrf: $CSRF" \
  -H 'content-type: application/json' \
  --data '{"slug":"my-team","name":"My Team"}' \
  "$C6_URL/api/v1/workspaces" | jq .
unset CSRF
```

Delete `claim.json` and `c6.cookies` when finished. They are credentials or
contain credentials. Browser code follows the same double-submit pattern.

## Recorded runtime truth

Creating a run or deployment validates authority and a real project revision,
persists an audit-backed record, and returns `status: "recorded"` with
`dispatchAvailable: false`. Secret value `PUT` returns `501`. Clients must
render those fields instead of substituting “queued,” “live,” or “deployed.”
