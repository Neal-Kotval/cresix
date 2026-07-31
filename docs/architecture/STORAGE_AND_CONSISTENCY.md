# Storage and consistency

## Local storage boundary

`C6_DATA_DIR` is one coherent local backup boundary:

```text
C6_DATA_DIR/
├── c6.sqlite3          control metadata
├── c6.sqlite3-wal      active SQLite WAL
├── c6.sqlite3-shm      active SQLite shared state
├── bootstrap-token     temporary mode-0600 claim proof
└── git/
    └── <project-uuid>.git
```

On Unix, the directory is real, non-symlink, and mode `0700`. Project slugs do
not become disk paths. SQLite runs with foreign keys, WAL, and bounded busy
timeout. Embedded idempotent migrations run during startup.

Git is authoritative for objects, trees, commits, and refs. SQLite is
authoritative for identity, policy, project identity, collaboration metadata,
runtime intent, credential verifiers, and audit. Duplicating Git bytes into SQL
would create two source authorities and is avoided.

## Transaction boundaries

Security-sensitive multi-record changes are SQLite transactions, including
their audit event. Audit failure aborts the mutation. Authentication queries
join current revocation state on every request.

SQLite and Git cannot participate in one atomic transaction. Operations use
ordered effects and narrow compensation: for example, a failed project
metadata transaction may remove only the newly created UUID repository.
Destructive deletion remains unavailable until safe staged semantics exist.
Git ref updates use an expected old object ID so concurrency fails rather than
silently publishing an unreviewed revision.

## Other state roots

| Owner | Durable state | Ephemeral state |
| --- | --- | --- |
| `c6-runner` | Private simulation journal and protocol key | Active request processing |
| `c6-cloud` | Separate SQLite account/directory/registration store | Connector presence, opaque live-session identity, and in-flight relay exchanges |
| `c6-connector` | Owner-only config and credential files only | Backoff and request state; wire generation is currently constant `1` |

Cloud storage must not contain local Git objects, local role/session verifiers,
runtime state, or secret values. A Cloud catalog is a versioned projection and
may lag the local authority.

## Backup and restore

Stop both local services before copying data. A coherent recovery set contains
the entire C6 data root, runner state if its records matter, exact C6 release,
and private operator configuration. Do not copy only the main SQLite file while
WAL writes are active. Restore to a clean installation and verify server ID,
claim state, sessions, repository refs, schedules, and audit before allowing
writes.

Cloud and connector state have different recovery semantics. Cloud database
backup preserves registrations but not presence. Connector credential files
need a deliberate private backup decision. In-place credential rotation,
credential reissue, re-registration of the same local server, and rebinding a
dogfood workspace after revocation are not implemented. Revocation is therefore
irreversible in the current preview even though local C6 remains intact.

## Future C6R storage

C6R locks live in source and are authoritative selection records. Future
content-addressed caches under a platform cache or C6 data root are disposable:
every read re-verifies the digest, writes use temporary directories and atomic
rename, and corruption is ignored or quarantined. Materialized passive exports
are read-only generated output keyed by composition digest and never overlay
source, sibling packages, or control data.

A registry may retain an immutable snapshot bound to Git provenance so locked
content survives source-host disappearance. It is distribution storage, not an
editable source authority.

See [Storage](../STORAGE.md) and [Operations](../OPERATIONS.md) for the current
layout and runbooks.
