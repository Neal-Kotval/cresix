# Operating C6

This guide describes the current single-machine Docker Compose distribution.
C6 is still a development preview: use it on loopback or a trusted development
network only. Public hosting is blocked on durable peer authentication and
enforced workload isolation.

## Host requirements

- A Docker Engine compatible with Compose v2
- Enough disk for the SQLite control store, Git repositories, run data, and
  project workspaces
- `curl` for the health check
- A firewall that denies unsolicited inbound traffic by default

Clone a tagged or commit-pinned C6 revision. Avoid running an arbitrary moving
branch on a shared host because application and migration versions must move
together.

The repository's root `c6.toml` is also a dogfood project manifest. Its service
command matches the final image's `/usr/local/bin/c6-server` entrypoint and its
`local-qa` job invokes `./qa/run.sh`. C6 can validate and display that contract
today; the simulation runner does not execute the service or QA job yet.
The QA suite tests the runner protocol directly and separately from these
recorded manifest declarations.

## First start

Create a private operator environment file and replace every placeholder:

```bash
cp .env.example .env
chmod 600 .env
docker compose config --quiet
docker compose up --build -d
docker compose ps
curl --fail http://127.0.0.1:8787/healthz
```

On first start, the runner atomically creates `/run/c6/runner.key` with 32 random
bytes and exact mode `0600`. It never logs the contents. The current control
plane does not dispatch runner work; the key protects the standalone simulation
protocol.

The control plane implements one-time owner claim, single-use invitations,
hashed bearer sessions, revocation, role checks, and audit records. It does not
implement key-based login or owner recovery. A successful health check means
the process and local data store are serving, not that the installation is
ready for untrusted users.

### Verify first-owner claim locally

On a new data volume, C6 writes a 256-bit bootstrap token to
`${C6_DATA_DIR}/bootstrap-token` with mode `0600` and logs only that path. Read
it locally; do not copy it into shell history or shared logs. If
`C6_BOOTSTRAP_TOKEN` was explicitly provided, C6 stores only its hash and does
not create the file or log the value. Paste the token into the first-run claim
screen, or verify the same control-plane flow with `curl` and `jq`:

```bash
export C6_URL=http://127.0.0.1:8787
docker compose exec -T c6 cat /var/lib/c6/bootstrap-token
read -rsp 'Paste the one-time token: ' C6_CLAIM_TOKEN; echo
ENROLLMENT_ID="$(openssl rand -hex 32)"

CLAIM_RESPONSE="$(printf '%s\n' "$C6_CLAIM_TOKEN" | \
  jq -Rn --arg key "$ENROLLMENT_ID" \
    '{token:input,displayName:"Owner",deviceLabel:"operator",publicKey:$key}' | \
  curl --fail --silent --show-error \
    --cookie-jar c6.cookies \
    --header "origin: $C6_URL" \
    --header 'content-type: application/json' \
    --data-binary @- \
    "$C6_URL/api/v1/bootstrap/claim")"
CSRF_TOKEN="$(jq -r '.session.csrfToken' <<<"$CLAIM_RESPONSE")"
curl --fail --silent --show-error \
  --cookie c6.cookies \
  "$C6_URL/api/v1/session" | jq .
unset C6_CLAIM_TOKEN ENROLLMENT_ID CLAIM_RESPONSE CSRF_TOKEN
```

`c6.cookies` and `CSRF_TOKEN` are credentials. Keep them private and remove the
cookie jar after local verification. Claim is transactional and can succeed
only once; C6 stores a hash and removes the generated token file after a
successful claim. The `publicKey` field is unverified opaque metadata and does
not support key-based login.

Native v1 has no administrator recovery credential. The bootstrap identity is
the immutable server administrator, and a workspace `owner` cannot replace it.
Do not clear or revoke that administrator's cookie: when its sole 30-day session
is lost, revoked, or expires through 30 days of inactivity, global
administration is locked out. Normal authenticated session reads renew both
cookies and slide expiry forward 30 days, but this is not recovery. The current
build is therefore not a durable company deployment.

## Network exposure

Keep C6 on `127.0.0.1` during development. Access from another machine should
eventually use an operator-controlled domain, reverse proxy or VPN, and HTTPS.
The Compose process binds its container interface, but Docker publishes that
port only to host loopback by default. This is not equivalent to LAN exposure.
C6's listener is always plaintext HTTP. `C6_PUBLIC_BASE_URL=https://...` marks
cookies `Secure` and establishes the browser origin; it does not enable TLS on
the backend listener.

For future remote use, the external route must:

- terminate modern TLS and forward to a loopback backend when possible;
- set `C6_INSECURE_HTTP=1` only when a protected container/private backend hop
  genuinely requires non-loopback `C6_BIND`;
- preserve the original scheme and host accurately;
- deny direct access to the runner socket and any project workload ports;
- avoid caching authenticated application or API responses; and
- impose request-size, connection, and rate limits appropriate for the host.

IP allowlists and VPNs are useful additional network restrictions, but IP
addresses must never identify or authorize a peer. NAT, DHCP, proxies, and
shared networks make that unsafe and unreliable.

## Persistent data

Compose uses named volumes. `docker volume inspect <name>` is the source of
truth for their host locations.

| Volume suffix | Contents | Required for recovery |
| --- | --- | --- |
| `c6-data` | SQLite control state, bare Git repositories, and server data | Yes |
| `runner-state` | runner request journal and execution state | Yes |
| `runner-socket` | Unix socket and generated runner protocol key | No; regenerated for this non-integrated simulation runner |

The Compose project name prefixes actual volume names (normally `cresix_`). Do
not infer paths or names in backup automation; resolve them using
`docker compose config --volumes` and `docker volume inspect`.

## Backup and restore boundary

A usable backup is a coordinated snapshot of `c6-data`, `runner-state`, and the
exact C6 revision. Copying only the SQLite file is insufficient because Git
repositories and runner state must describe the same point in time. The current
simulation runner's generated protocol key may be regenerated because the
server has no dispatch integration.

C6 does not yet ship an online backup coordinator. For a consistent offline
backup:

1. Record `git rev-parse HEAD` and save `.env` separately in an encrypted
   secret manager.
2. Stop control-plane writes and simulation requests with
   `docker compose stop c6 runner`.
3. Confirm both processes have stopped with `docker compose ps`.
4. Snapshot or archive `c6-data` and `runner-state`
   using the host's volume-backup mechanism.
5. Restart with `docker compose up -d` and confirm `/healthz`.

To restore, use a clean Docker host or a new Compose project name, recreate the
empty named volumes, restore each archive to its matching volume, restore the
same `.env` values, check out the recorded C6 revision, then start the stack.
Validate project metadata, repositories, and workload data before allowing
writes. Never restore untrusted archives as root without first checking for
absolute paths, `..` traversal, links, ownership, and unexpected device files.

## Upgrade and rollback

There is no supported in-place migration/rollback workflow yet. Before an
upgrade:

1. Read the release notes and inspect new migrations.
2. Take and verify a complete offline backup.
3. Pin the target tag or commit and run `make team-qa`.
4. Rebuild with `docker compose build --pull` and start with
   `docker compose up -d`.
5. Check service health and inspect `docker compose logs c6 runner`.

Database migrations can be forward-only. Rolling container images back does
not roll data back; restore the coordinated pre-upgrade backup when a release
requires an incompatible migration.

## Troubleshooting

`docker compose ps` shows container and health state. `docker compose logs c6`
and `docker compose logs runner` show control-plane and runner startup errors.

Common failure modes:

- **Port 8787 is occupied:** choose another host port in `.env` with `C6_PORT`.
- **Web UI is missing during a source run:** run `npm ci --prefix web` and
  `npm run build --prefix web`, or set `C6_WEB_DIST` to an existing build.
- **The runner restarts repeatedly:** inspect its key-file error. An existing
  `/run/c6/runner.key` must be a regular non-symlink file, mode `0600`, and at
  least 32 bytes. Do not weaken its permissions to make startup succeed.
- **The database is locked:** stop duplicate C6 server processes that share one
  data directory. The embedded store is for one authoritative control process.
- **Another laptop cannot connect:** check host binding, OS firewall, routing,
  and TLS proxy configuration. Direct plaintext LAN testing additionally
  requires an explicit `C6_INSECURE_HTTP=1` risk acknowledgement. Do not disable
  authentication or TLS as a production fix.
- **Disk is full:** stop writes first, identify the growing volume, preserve a
  backup, and expand or safely prune only documented rebuildable data.

Never paste `.env`, cookie jars, bootstrap/invitation tokens, runner keys, or
unredacted logs into an issue.
