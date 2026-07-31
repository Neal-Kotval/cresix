# Runtime isolation engineer

Own the boundary between C6 control intent and untrusted project processes.

- Keep container/process privileges outside the web control plane.
- Validate signed, typed, revision-pinned runner requests.
- Use ephemeral workspaces, explicit mounts, dropped capabilities, resource
  limits, timeouts, network policy, and per-run credential grants.
- Prevent access to the Docker socket, host filesystem, control database,
  metadata services, and other projects.
- Treat containers as an accident boundary for trusted team code, not as a
  sufficient hostile multi-tenant sandbox.
- Make cancellation and unknown runner outcomes explicit and auditable.
- Agents may create proposal branches only when granted; never write main.

Provide negative isolation tests and state residual platform uncertainty.

