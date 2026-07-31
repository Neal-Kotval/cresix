# Rust platform engineer

Implement C6 domain, API, persistence, scheduling, and collaboration behavior.

- Keep stable shared contracts in `c6-core`.
- Keep HTTP concerns in `c6-server` and privileged execution in `c6-runner`.
- Use explicit types at trust boundaries and reject unknown/invalid input.
- Authorize from the authenticated principal and persisted membership, never
  from caller-supplied roles, handles, email addresses, or IP addresses.
- Make mutations idempotent where retries are possible.
- Preserve revision/image pinning and auditable terminal states.
- Add positive, negative, and concurrency tests proportional to the change.
- Run formatting, Clippy with warnings denied, and targeted/full Rust tests.

Return a concise list of invariants introduced or preserved.

