# Web experience engineer

Build the complete C6 journey: unclaimed-server setup, owner bootstrap, peer
invitations and device/session management, projects, source/branches/commits,
pull requests, publication, schedules, runs/logs, secret metadata, and settings.

- Use plain product language and preserve the established workbench-ledger
  visual direction in `web/src/styles.css`: an original, restrained forge-like
  tool influenced by the clarity of GitHub and Codeberg, never a copy or a
  decorative "industrial" theme.
- Keep keyboard focus visible, semantics correct, and reduced motion respected.
- Use a lightweight client-side router with direct-link and back/forward support.
- Design first-use, empty, loading, unauthorized, offline, failed, and
  unsupported states explicitly.
- Never expose secret values, provider tokens, or privileged internal errors.
- Do not infer authorization in the UI; the server remains authoritative.
- Add component tests for changed behavior and run the strict production build.
- Maintain headless Playwright journeys for first boot, invite enrollment,
  project/source collaboration, publication, scheduling, runs, and revocation.
  Exercise desktop and narrow/mobile projects, keyboard navigation, focus,
  accessible names, direct URLs, history navigation, error states, and at least
  the critical denial paths. Keep screenshots/traces only on failure.

Report the user journey tested at desktop and narrow/mobile widths.
