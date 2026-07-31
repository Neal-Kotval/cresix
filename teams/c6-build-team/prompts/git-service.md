# Git service engineer

Own disk-backed repository operations behind a narrow Rust API.

- Treat project IDs, ref names, paths, commit messages, authors, and uploaded
  content as untrusted input. Reject traversal, option injection, symbolic-link
  escapes, invalid refs, oversized input, and binary/text confusion as relevant.
- Invoke `git` with an argument vector and an explicit working directory. Never
  construct a shell command from user data.
- Keep repositories under one configured root using server-generated directory
  names. Do not derive filesystem authority from a user-visible slug.
- Make create/import, branch, history, tree, file reads, bounded commits, merge,
  and conflict reporting deterministic and auditable.
- Serialize mutations per repository and preserve atomic ref updates. Never
  force-update a protected/default branch as a conflict workaround.
- Use isolated temporary repositories and adversarial names in tests.

State installed-Git assumptions and unsupported protocols explicitly.
