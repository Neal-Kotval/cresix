# Git subsystem

## Implemented local store

`c6-git` manages bare repositories beneath `${C6_DATA_DIR}/git` using Git
plumbing invoked with argument arrays and a cleared environment—never shell
strings. Repository directory names are server-created project UUIDs.

The library supports:

- safe creation, opening, local-only import, and staged deletion;
- SHA-1 and SHA-256 object formats;
- bounded branches, commits, trees, file reads, and diffs;
- branch creation and atomic commits from bounded file changes;
- expected-old-value ref updates and merge/conflict analysis;
- traversal, symlink, `.git`, option-injection, and oversized-output refusal.

The server currently exposes bounded read endpoints and seeds a new project
repository with initial content. Other library capability is not automatically
a public product workflow.

## Not implemented

- Git smart HTTP or SSH transport
- Network clone, fetch, push, or credential issuance
- Browser branch editing or complete PR merge workflow
- Remote URL import
- Public repositories or anonymous source access

To inspect current source, use the authenticated repository JSON API or open
the operator-owned bare repository locally while C6 is stopped. Do not mutate
bare storage behind a running server; that bypasses authorization, expected-ref
checks, and audit records.

## Safety invariants

- User slugs never become filesystem paths.
- Paths are canonicalized under one real root; symlinks are rejected.
- Revisions are full lowercase 40/64-character object IDs when pinning matters.
- Reads and subprocess output are bounded.
- Hooks and inherited Git configuration are not executed during managed import.
- Destructive deletion only targets a verified C6 bare repo staged under a
  private tombstone; SQL integration must complete before the API enables it.
