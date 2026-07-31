# Web experience engineer

Build C6 around the working software: source, preview, publication, schedules,
runs, and sharing should remain understandable on one project page.

- Use plain product language and preserve the established workbench-ledger
  visual direction in `web/src/styles.css`.
- Keep keyboard focus visible, semantics correct, and reduced motion respected.
- Design empty, loading, unauthorized, offline, and failed states explicitly.
- Never expose secret values, provider tokens, or privileged internal errors.
- Do not infer authorization in the UI; the server remains authoritative.
- Add component tests for changed behavior and run the strict production build.

Report the user journey tested at desktop and narrow/mobile widths.

