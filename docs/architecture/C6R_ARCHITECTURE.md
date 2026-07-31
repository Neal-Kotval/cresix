# C6R composition architecture

Status: **accepted target design; not implemented**

## Purpose

A C6R is a Git-addressed, immutable, reusable composition unit for small
software. It lets a larger GitHub-style repository consume a handbook, agent
team, tool, custom MCP server, mini app, job, or workflow through a reviewable
manifest and lock rather than copy/paste or ambient setup.

The C6R boundary is a safe directory closure rooted at `c6r.toml`. It may be a
whole repository or a subdirectory of a monorepo. The manifest root—not the
repository root—defines what the component can see and package.

## Architectural pipeline

```mermaid
flowchart LR
    Source[Git remote + full commit + subdirectory]
    Validate[Strict manifest and safe tree validation]
    Resolve[Direct dependency resolution]
    Lock[Committed exact lock and provenance]
    Cache[Verified content-addressed cache]
    Materialize[Read-only passive composition tree]
    Runtime[Future resolved active run plan]

    Source --> Validate --> Resolve --> Lock --> Cache --> Materialize
    Materialize -. later gated .-> Runtime
```

Git remains source authority. A registry maps package identity/version to an
immutable source identity and may retain verified distribution bytes. A
consumer project remains policy authority: it selects exports, maps inputs and
secret slots, and grants capabilities.

## Package envelope

All C6Rs share strict, versioned package identity, file closure, inputs, secret
slots, exports, requirements, and direct dependencies. Export kind selects a
different typed risk boundary:

- `content` and `agent_team`: first planned passive materialization only;
- `command`, `mcp`, `service`, `app`, `job`, and `workflow`: descriptor
  validation only until each runtime capability ships independently.

An agent-team export is data. Materializing its prompts does not insert them
into an agent context or make its tools, model, or MCP endpoints available.
Rendered Markdown and agent text are hostile input; until safe rendering and
content-origin controls exist, escaped source is preferable to rich HTML.

## Integrity and acquisition

Immutable identity combines a reviewed HTTPS Git remote, full SHA-1 or SHA-256
commit, normalized subdirectory, manifest SHA-256, deterministic content
SHA-256, and provenance digest. Tags and branches may aid selection but cannot
appear as lock authority.

Version 1 rejects SSH, `file:`, remote helpers, userinfo/query credentials,
cross-origin redirects, submodules, unsafe paths, symlinks, and unbounded Git
packs. Credential helpers and prompts are disabled; credentials never enter
locks, cache remotes, errors, or logs.

Locks are committed review artifacts. Caches and materialized trees are
disposable and content-verified. Path dependencies are allowed only for local
development and make a composition non-publishable until resolved to an exact
commit and subdirectory.

## Dependency and permission model

The first resolver supports direct dependencies only. A dependency containing
dependencies fails with `transitive_dependencies_unsupported`. Alias-qualified
exports avoid implicit namespace merging. This bounds provenance and
permission review; it is a deliberate constraint, not an incomplete recursive
resolver claim.

Requirements never grant authority. An active component can prepare only when
the consuming project explicitly maps its secret slots and grants all required
resources, network, repository, and runtime capabilities—and when local policy
and the adapter can enforce every grant. Dependencies inherit no authority.

## Registry boundary

Cresix Cloud may host the best package directory at an explicit `/r/...` route,
but registry protocols remain self-hostable. A Cresix account is never required
to parse, lock, verify, cache, or materialize a C6R. Publication is immutable;
yank affects new resolution but never rewrites an existing lock. Signatures
identify an attesting registry or publisher, not code safety.

## Stage gates

1. Strict formats, safe tree closure, deterministic pack, and abuse fixtures.
2. Direct resolution, exact locks, permission diffs, and passive
   materialization.
3. Self-hostable immutable registry, authorization, provenance, and Hub pages.
4. Resolved active descriptors that remain truthfully unavailable.
5. One separately reviewed sandboxed export kind at a time.

The normative schema, examples, limits, lifecycle, and deferrals are in the
[C6R composition specification](../specs/C6R_COMPOSITION.md).
