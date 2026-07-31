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
| Workspaces | `GET|POST /workspaces`, `PUT|DELETE /workspaces/{id}` | Durable metadata; delete deferred |
| Projects | `GET|POST /projects`, `GET|PUT|DELETE /projects/{id}` | Durable metadata and seeded Git; delete deferred |
| Collaboration | `GET|POST /projects/{id}/pull-requests` | Persist/read PR records; merge deferred |
| Runtime metadata | `GET|POST` project `deployments`, `runs`, `schedules`, `secrets` | Recorded intent; schedules validated; no dispatch |
| Repository | project `repository/branches`, `commits`, `tree`, `files/{path}` | Bounded reads from local bare Git |
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

Resource paths use server UUIDs, not workspace/project slugs. Repository commit,
tree, and file reads accept `revision` (default `main`); commit queries also
accept `limit`, and tree queries accept `recursive`. Values are still bounded
and validated by the Git layer.

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
