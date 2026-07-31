# C6 architecture

## One-server topology

```text
browser / git client
        │
        ▼
┌──────────────────────── C6 control plane ────────────────────────┐
│ account + ACL │ Git authorization │ project API │ web gateway   │
│ pull requests │ deploy intent     │ scheduler   │ audit events  │
└──────────┬───────────────────────────────┬───────────────────────┘
           │ PostgreSQL                    │ signed Unix-socket work
           ▼                               ▼
    control database                 C6 runner daemon
                                     │
                             rootless project containers
                              │       │        │
                         workload  object    OCI image
                         Postgres   storage   registry
```

The control plane is unprivileged. It records intent and authorization. Only the
runner daemon will be allowed to create execution sandboxes, and requests to it
must be authenticated, narrowly typed, revision-pinned, and auditable.

## Stable concepts

- **Workspace:** membership and billing/operations boundary.
- **Project:** Git repository plus collaboration, hosting, data, and access.
- **Revision:** immutable Git commit.
- **Pull request:** proposed branch merge with an isolated preview.
- **Deployment:** immutable revision and image promoted to an environment.
- **Job:** named command, cron task, or agent declared by `c6.toml`.
- **Run:** one revision-pinned execution and its terminal outcome.
- **Secret:** write-only workspace value granted by name to selected workloads.
- **Runner:** execution host enrolled with the control plane.

## Persistence boundaries

Control-plane PostgreSQL and project PostgreSQL are separate services and use
unrelated credentials. Compromise of a project database credential must not
provide a route into identity, authorization, or secret metadata.

Git repositories live as bare repositories on a dedicated volume. Object data,
run artifacts, and OCI images live outside Git. A fork copies repository history
but does not copy data-plane resources or secret values.

## Agent writes

An agent receives an ephemeral checkout of a pinned revision. With no repository
grant, its writes remain run artifacts. With `repository_write = "proposal"`,
the runner may create `agent/<job>/<run>` and open a pull request under a service
identity. Direct writes to the default branch are never an agent capability.

