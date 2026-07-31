# C6 roadmap

## Slice 1 — visible foundation

- [x] Rust workspace and stable manifest contract
- [x] Project roles and permission tests
- [x] GitHub-like project website
- [x] Project, pull request, deployment, and run API shapes
- [x] Separate runner process boundary
- [x] PostgreSQL persistence schema
- [x] One-server Compose topology
- [x] LAN-sharing documentation
- [ ] Persistent repository/API store wiring
- [ ] First-user setup and GitHub OAuth redirect

## Slice 2 — collaboration

- Git smart HTTP and SSH transport
- Account tokens and SSH keys
- Repository browser, branches, commits, diffs, and pull-request mutations
- Invitations and cumulative project roles
- Preview lifecycle without CI/check ceremony

## Slice 3 — hosting

- Auto-build and Dockerfile builders
- Rootless container execution, health checks, quotas, and logs
- Authenticated app gateway and signed identity
- Explicit publish and immutable rollback
- Per-project PostgreSQL and object buckets

## Slice 4 — jobs and agents

- Durable scheduler and concurrency policies
- Encrypted, write-only secrets
- Pinned Codex CLI runtime using a separately granted API key
- Egress policy and resource enforcement
- Agent-generated proposal branches and pull requests

## Slice 5 — portable hosting

- Outbound mTLS runner enrollment
- Safe operator-configured tunnel workflow
- Backups, restores, upgrades, and operational health
- Stronger microVM isolation before hostile multi-tenant hosting

