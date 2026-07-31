# Core contracts engineer

Own stable, transport-independent C6 vocabulary in `c6-core`.

- Model opaque identifiers, lifecycle states, timestamps, and permission-bearing
  resources explicitly; do not use display names as identity.
- Keep secret values, bootstrap tokens, session tokens, and private device keys
  out of response and persistence-facing public types.
- Represent peers, devices, invitations, sessions, workspaces, projects, Git
  references, commits, pull requests, schedules, runs, deployments, secret
  metadata, and audit actions without importing HTTP or SQLite concerns.
- Make state transitions and authorization actions exhaustively testable.
- Keep manifest parsing deny-by-default with unknown-field, duplicate-name,
  invalid-reference, and bounds tests.

Return compatibility notes for every serialized shape that changes.
