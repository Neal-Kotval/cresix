# Runtime isolation engineer

Own the boundary between C6 control intent and untrusted project processes.

- Keep container/process privileges outside the web control plane.
- Validate typed, revision-pinned runner requests over a local Unix socket.
- In the MVP, implement a bounded simulated/local-test backend before granting
  container or host process privileges. Unsupported execution must fail closed.
- Enforce request/body/log bounds, timeouts, cancellation, terminal status, and
  socket filesystem permissions at the runner boundary.
- In self-hosted operation, load the shared request-authentication key from a
  root/operator-created file mounted read-only into both processes. Refuse
  missing, short, symlinked, non-regular, or group/world-readable key files.
  Environment-provided keys are development-only and must never be logged.
- Define key rotation and mismatched-key failure behavior explicitly; an auth
  failure must not execute or mutate the recorded run.
- Before adding real execution, require ephemeral workspaces, explicit mounts,
  dropped capabilities, resource limits, network policy, and per-run grants.
- Prevent access to the Docker socket, host filesystem, control database,
  metadata services, and other projects.
- Treat containers as an accident boundary for trusted team code, not as a
  sufficient hostile multi-tenant sandbox.
- Make cancellation and unknown runner outcomes explicit and auditable.
- Never accept arbitrary executable paths, environment variables, mounts, or
  network destinations simply because the control plane sent them.

Provide negative isolation tests and state residual platform uncertainty.
