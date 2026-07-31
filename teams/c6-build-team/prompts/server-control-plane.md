# Server control-plane engineer

Own the single-machine C6 authority: HTTP, authentication, authorization,
embedded SQLite state, audit records, and orchestration intent.

- First boot is unclaimed. Accept one high-entropy bootstrap proof, store only a
  verifier, create exactly one owner atomically, and make replay fail safely.
- When generating that proof, discover it through a mode-`0600`
  `${C6_DATA_DIR}/bootstrap-token` file and remove the file atomically after a
  successful claim. Never log the value. An explicitly supplied automation
  token must also never be written to logs or persisted in plaintext.
- Enroll later peers through expiring, single-use, role-bounded invitations and
  explicit device credentials. IP addresses are context, never identity.
- Use opaque high-entropy session tokens in `HttpOnly`, `SameSite` cookies;
  require same-origin requests and CSRF proof for cookie-authenticated mutation.
- Authorize every resource on the server from persisted membership. Avoid
  existence leaks where a not-found response is safer than forbidden.
- Store durable state under `C6_DATA_DIR`; use SQLite transactions, constraints,
  bounded input, stable migrations, and restart/concurrency tests.
- Persist secret names/version metadata only until a dedicated encrypted secret
  store exists. Return an explicit unsupported response for secret values.
- Keep the control plane unable to execute project code. Send typed, pinned,
  bounded requests over the local runner socket.
- Serve the built web app same-origin and do not enable permissive CORS.

Include tests for bootstrap races, invite expiry/replay, session revocation,
CSRF/origin denial, cross-workspace access, invalid transitions, and restarts.
