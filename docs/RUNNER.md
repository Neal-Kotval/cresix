# Runner

`c6-runner` is a separate authenticated protocol daemon. It is extensively
tested, but its only backend is simulation and the control plane does not call
it. Creating a C6 run records intent; it does not reach this daemon.

## Protocol v1

- Unix socket with exact mode `0600`
- Maximum frame size 256 KiB
- HMAC-SHA256 signed envelope with protocol version, request UUID, timestamp,
  16-byte nonce, encoded typed payload, and MAC
- Maximum one-minute clock skew and bounded replay cache
- Strict unknown-field rejection
- Commands: `ping`, `execute`, `cancel`, and `inspect`
- Typed client available for future policy-complete integration

Execution requests require non-nil run/workspace/project UUIDs, a full revision,
pinned manifest digest, resources, network policy, repository-write policy, and
simulation plan. Limits cap timeout at one hour and logs at one MiB.

## Key and socket safety

The runner uses `C6_RUNNER_AUTH_KEY` when explicitly set. Otherwise it safely
loads or atomically creates `C6_RUNNER_AUTH_KEY_FILE`, defaulting beside the
socket. Existing key files must be regular non-symlinks, mode `0600`, and at
least 32 bytes. Socket startup refuses regular files and symlinks rather than
deleting an attacker-chosen path.

## Simulation semantics

Simulation exercises accepted/running/terminal lifecycle, bounded UTF-8 logs,
timeout, cancellation, inspection, idempotent request replay, and durable
terminal results. It never spawns a command, contacts Docker, mounts a project,
uses a credential, or performs network egress.

## Integration blocker

The server cannot safely construct an execution request from its current
`POST /runs` body: it lacks manifest-resolved resource, egress,
repository-write, and simulation policy. Integration was deliberately declined
instead of inventing defaults. The next slice must resolve a validated manifest
at a pinned revision, persist the complete policy, and reconcile runner outcome
without automatic retry.

This daemon is not evidence of container isolation or hosting support.
