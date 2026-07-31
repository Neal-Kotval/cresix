# C6Rs: reusable small-software components

## Product idea

A **C6R** is a reusable, reviewable composition stored in Git. It packages one
coherent capability that can be used by a C6 project or incorporated into a
larger repository that may continue to live on GitHub, Codeberg, or another
forge.

C6Rs are the bridge between two useful scales:

- small software should be independently understandable and shareable; and
- useful small capabilities should not become trapped in one standalone app.

The design is accepted, but the tooling is not implemented in this revision.

## What should be a C6R

A good C6R has a narrow purpose, a bounded safe content closure, explicit inputs
and exports, and a useful review boundary. Likely shapes include:

- **Content:** wiki sections, handbooks, templates, schemas, policies, prompt
  libraries, or design-system fragments.
- **Agent teams:** versioned agent roles, instructions, QA procedures, and
  orchestration configuration.
- **Tools:** a small deterministic helper with declared runtime requirements.
- **MCP servers:** narrow integrations with explicit tools, network needs, and
  secret slots.
- **Mini apps:** focused interfaces such as a status board or team tracker.
- **Services and jobs:** one bounded backend, scheduled report, or automation.
- **Workflows:** compositions of declared capabilities whose dependencies and
  grants remain visible.

The latter five are active kinds. They are product direction, not currently
safe to activate.

## What should not be a C6R

A C6R should not be:

- a replacement for a language package manager;
- an arbitrary repository installer with lifecycle hooks;
- an entire monolith split only for branding;
- a hidden bundle of transitive commands and credentials;
- a mutable URL or branch consumed without a lock and digest;
- a copy of local user state, sessions, or secret values;
- an opaque container whose behavior cannot be reviewed; or
- a mechanism for bypassing the parent project's policy or C6 sandbox.

## Decision test

A candidate is a good C6R when most answers are yes:

1. Does it provide one capability with a name a user can understand?
2. Can its complete source closure be bounded safely?
3. Can inputs, exports, requirements, and destinations be declared?
4. Is an immutable revision and content digest meaningful?
5. Can a reviewer understand an update as a focused diff?
6. Can removal be deterministic without deleting user-owned files?
7. Would reuse across at least two projects reduce meaningful duplication?

If the component exists only to share a language library, use that language's
package system. If it cannot be bounded or reviewed, keep it inside the parent
repository until it can.

## Repository or subdirectory

A C6R may be an entire Git repository or a declared subdirectory with a safe,
deterministic closure. This lets a small component live independently or inside
a larger monorepo. The resolved unit must not escape its root through symlinks,
path traversal, submodules, hooks, or implicit files elsewhere in the checkout.

## Manifest, lock, and provenance

The proposed contract has three layers:

- `c6r.toml` declares package identity, kind, inputs, exports, requirements,
  dependencies, and safe paths;
- `c6r.lock` records exact source URL, immutable Git object ID, content digest,
  dependency results, and provenance facts; and
- the materialized tree is a deterministic, reviewable projection placed under
  an explicit destination.

Mutable versions are selection hints, not authority. A lock must pin the exact
Git revision and SHA-256 digest of the normalized safe closure. Resolution must
not execute hooks, render active content, contact undeclared hosts, or borrow
ambient Git credentials.

## Relationship to bigger projects

A larger project should be able to consume a C6R without moving its canonical
repository into C6. The parent repository commits its own `c6r.toml` references
and `c6r.lock`; materialized content can be committed or reproduced according
to an explicit policy. Git review shows the incoming or updated content.

C6 can eventually add discovery and ergonomics, but it does not own the parent
repository merely because that repository uses a C6R. Local policy and the
parent forge's review controls continue to apply.

## Permission model

Artifact kind does not imply permission. `content` and `agent_team` can be
materialized passively. A declared command, MCP server, app, service, job, or
workflow requires separate activation and explicit runtime, network, secret,
filesystem, and repository grants.

Installing source is not permission to execute it.

## Product stages

- **Now, design:** selection test, strict format, lock, provenance, lifecycle,
  and threat boundaries are specified.
- **Next:** implement strict parsing, safe-closure hashing, immutable resolution,
  a committed lockfile, status/validation commands, and passive `content` plus
  `agent_team` materialization.
- **Later:** add self-hostable discovery and reviewable update flows, then
  prepare active kinds without executing them.
- **Later, gated:** activate tools, MCPs, apps, services, jobs, and workflows
  only after the runtime security gates in [Agent-centric direction](AGENT_CENTRIC.md).

The normative technical design is [C6R composition](../specs/C6R_COMPOSITION.md).
