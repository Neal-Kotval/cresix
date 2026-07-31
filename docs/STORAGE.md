# Storage

## Layout

`C6_DATA_DIR` defaults to `.c6` for source runs and `/var/lib/c6` in Compose.
On Unix, C6 requires a real non-symlink directory and sets mode `0700`.

```text
C6_DATA_DIR/
├── c6.sqlite3       control metadata
├── c6.sqlite3-wal   SQLite write-ahead log while active
├── c6.sqlite3-shm   SQLite shared state while active
├── bootstrap-token  temporary mode-0600 first-claim secret
└── git/
    └── <project-UUID>.git  C6-created bare repositories
```

Runner state is separate (`C6_RUNNER_STATE_DIR`) because the runner is a
separate process boundary. Compose persists it in `runner-state`; its socket
volume also contains the mode-`0600` runner protocol key.

## SQLite ownership

SQLite contains settings, peers, devices, sessions, invitations, workspaces,
memberships, projects, PR records, deployment/run/schedule records, secret
metadata, credential metadata/verifiers, and audit events. Plaintext CLI/Git
tokens are returned once and are not stored by the server. SQLite runs with
WAL and foreign keys enabled. Embedded idempotent migrations run at startup;
there is no separate migration command.

Git remains authoritative for branches, commits, trees, and file bytes. SQL
stores repository identity and selected head metadata, not duplicated objects.

## Cross-store consistency

Project creation coordinates SQL with creation of a UUID-named repository.
Project deletion returns `501` until staged Git deletion and SQL transaction
semantics are integrated end to end. This is preferable to orphaning metadata
or erasing the wrong repository.

Audit writes are part of security-sensitive transactions. A failed audit insert
fails the associated mutation. Revision-taking APIs validate a full object ID
against the project's repository before recording intent.

## Backup lifecycle

Stop both services before copying data. A coherent backup contains `c6-data`,
`runner-state`, the exact source revision, and private operator configuration.
Do not copy only `c6.sqlite3` while WAL writes are active. The standalone
simulation runner key may regenerate because the control plane is not its
client.

Restore volumes to a clean installation, use the recorded C6 revision, and
verify server ID, claim state, projects, repository reads, schedules, and audit
history before allowing writes. See [Operations](OPERATIONS.md) for the runbook.

## Retention and deletion

No automatic retention policy exists. Runs/deployments are metadata records;
there are no workload logs or artifacts from the control plane. Peer revocation
preserves attribution and audit history. Destructive project/workspace erasure
is deferred rather than partially implemented.
