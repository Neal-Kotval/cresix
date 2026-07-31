# C6R: reusable compositions for small software

Status: accepted design; implementation not started
Owners: `c6-build-team`
Target: staged delivery after the connected-mode foundation
Last updated: 2026-07-31
Capability status: [C6R composition](../product/CAPABILITIES.md#c6r-composition)

## 1. Decision

A **C6R** is a Git-addressed, immutable, reusable composition unit for C6. It
is a safe directory closure rooted at a strict `c6r.toml` manifest. The closure
may be an entire Git repository or one subdirectory of a larger repository.

That distinction is deliberate. A small tool may naturally occupy its own
repository, while a company monorepo may contain a handbook, an agent team, and
several small applications that need independent versions. Requiring one Git
repository per C6R would make monorepos awkward; treating every repository as a
C6R would make package boundaries and permissions ambiguous. The manifest root,
not the Git repository boundary, defines the reusable unit.

A C6R is not a control-plane plugin and is never loaded into the C6 server
process. A consuming C6 project remains the policy and authority root. It
selects components, pins them in a committed lockfile, maps inputs and secret
slots, and explicitly grants any requested capability.

The first implementation is deliberately small:

- parse a strict, versioned `c6r.toml`;
- resolve an immutable Git commit and safe subdirectory;
- produce and verify deterministic content digests and lock records;
- support direct dependencies only;
- passively materialize `content` and `agent_team` exports for inspection and
  composition;
- validate active `command`, `mcp`, `service`, `app`, `job`, and `workflow`
  descriptors, but never execute, schedule, host, or connect them.

Active descriptors remain unavailable until C6 has the sandbox, secret,
network, runtime, provenance, and audit gates described in the
[agent-first runtime specification](AGENT_FIRST_RUNTIME.md). Validation is not
installation, and materialization is not execution.

## 2. The decision test

Something should be a C6R only when all of these are true:

1. It is a coherent unit that can be named and versioned independently.
2. It is intentionally shareable or useful in more than one composition.
3. It has a small, stable, declared interface rather than relying on ambient
   files, credentials, or host configuration.
4. Its complete source closure can be enumerated and digested without running
   code.
5. A consumer can review its inputs, outputs, dependencies, and capability
   requirements before accepting it.

One-off project wiring belongs directly in the project. Code used only through
a language ecosystem should generally remain a Cargo, npm, Python, or similar
package. Copy-and-edit scaffolding is a **template**, not a versioned component:
templates intentionally diverge after creation, while a C6R stays pinned and
receives reviewable updates.

## 3. What is and is not a C6R

### Good C6R shapes

| Shape | C6R boundary | Initial behavior |
| --- | --- | --- |
| Handbook or wiki | Immutable Markdown, navigation, and assets | Materialize and browse content |
| Agent team | Agent profiles, prompts, skills, and team wiring | Materialize and inspect configuration |
| Tool | Source plus a declared command interface | Validate only |
| MCP server | Source, protocol metadata, and a declared transport | Validate only |
| Mini application | Service, assets, jobs, and data requirements | Validate only |
| Command or cron job | Command contract, limits, and named outputs | Validate only |
| Workflow module | Typed steps and artifact edges | Validate shape only; runtime deferred |

A wiki's published pages can be a content C6R. Mutable comments, drafts, access
records, and database state cannot. An agent team can be a C6R when its prompts,
profiles, and tool requirements are portable; an API key or a developer's
ambient Codex session cannot.

### Things that must not be C6Rs

- secret values, credentials, tokens, private keys, or provider sessions;
- customer data, database snapshots, mutable wiki state, logs, or run artifacts;
- an arbitrary repository merely because it contains code;
- ordinary language libraries already served by a language package manager;
- mutable branches or tags presented as immutable releases;
- authentication, authorization, storage, or policy plugins loaded into C6;
- peer identities, workspace roles, installation configuration, or host paths;
- an image or binary without a source identity, interface, and provenance;
- post-install scripts or arbitrary code that runs while resolving a package.

## 4. Concepts and authority

- **Package identity:** a registry-qualified namespace and package name, such as
  `cresix.com/acme/release-team`.
- **Source identity:** Git remote, full commit object ID, normalized C6R
  subdirectory, and deterministic content digest.
- **Release:** one immutable mapping from package identity and semantic version
  to a source identity and provenance statement.
- **Export:** a named, typed interface made available by a C6R.
- **Component:** one alias-qualified use of a C6R in a consuming project.
- **Materialization:** deterministic preparation of passive files in a
  read-only generated tree. It does not run package code.
- **Requirement:** authority an active export would need if it were eventually
  run. A requirement is a request, never a grant.
- **Lock:** an exact, reviewable record of every selected release and its source
  identity.

Git remains source truth. A registry may retain a content-addressed snapshot so
a lock remains recoverable if the original Git host disappears, but that
snapshot is immutable distribution data, not editable source authority.

## 5. Strict `c6r.toml` schema

The manifest is TOML with `schema = 1`. Every table denies unknown fields.
Names use 1–63 lowercase ASCII letters, digits, and hyphens, beginning and
ending with an alphanumeric character. Paths are normalized UTF-8 relative
paths. Absolute paths, `..`, backslashes, NUL, empty components, and symlinks
are rejected.

```toml
schema = 1

[package]
name = "release-team"
version = "1.2.0"
description = "Agents that prepare release proposals"
license = "Apache-2.0"
files = ["README.md", "agents", "skills"]

[inputs.model]
type = "string"
required = true

[secrets.model-api]
description = "Credential for the selected model provider"

[exports.handbook]
kind = "content"
path = "README.md"
format = "markdown"

[exports.release]
kind = "agent_team"
path = "agents/team.toml"

[exports.release-notes-server]
kind = "mcp"
transport = "stdio"
command = ["./bin/release-notes-mcp"]
protocol = "2025-06-18"

[requirements.release-notes-server]
network = ["api.openai.com:443"]
secret_slots = ["model-api"]
repository_write = "proposal"
max_cpu = 1.0
max_memory_mb = 1024
```

The `package.name` and `package.version` are assertions checked against the
publication request. They do not create a global identity by themselves.

### 5.1 Package fields

`package` contains:

- `name`: unqualified package name;
- `version`: exact SemVer without a mutable channel such as `latest`;
- `description`: bounded plain text;
- `license`: SPDX expression or `NOASSERTION`;
- `files`: non-empty list of files or directories forming the closure.

`c6r.toml` is always included. A package with direct dependencies also includes
its adjacent `c6r.lock`. Entries in `files` may overlap, but resolve to one
deduplicated normalized path set. Directories are traversed recursively with
bounded depth, file count, individual size, and total size. Version 1 rejects
all symlinks, Git submodules, sockets, devices, and case- or
Unicode-normalization collisions.

### 5.2 Inputs and secret slots

Inputs are named, typed values. Version 1 supports bounded `string`, `boolean`,
`integer`, `enum`, and lists of those types. An input may declare `required`, a
non-secret default, a description, and type-specific bounds.

Input substitution occurs only in fields whose schema explicitly accepts an
input reference. C6 never performs string interpolation over TOML, commands,
paths, or URLs. This prevents a harmless setting from becoming a shell or path
injection primitive.

Secret declarations are logical slots containing a name and description only.
A consuming project maps each selected slot to one of its own secret names.
Secret values never appear in a manifest, lock, package archive, registry
record, materialized tree, diff, log, or provenance statement.

### 5.3 Exports

Each export name is unique within the package. Its `kind` selects a strict,
versioned payload schema:

- `content`: content root or file and format; Markdown is the initial format;
- `agent_team`: one configuration entry point and its passive file closure;
- `command`: argv, working-directory contract, named inputs and artifacts;
- `mcp`: `stdio` or bounded HTTP descriptor, protocol compatibility, and
  command or service reference;
- `service`: argv, named port, health contract, and static assets;
- `app`: composition of service, content/assets, jobs, and data requirements;
- `job`: command or agent reference, timeout ceiling, trigger capability, and
  named artifacts;
- `workflow`: typed steps and artifact edges without an expression language.

Only `content` and `agent_team` are materializable in the first implementation.
They are passive even when their text describes actions. C6 may render, search,
or validate their files, but it does not invoke an agent, tool, command, model,
MCP transport, web server, or workflow.

Rendered package content is hostile input. Markdown previews strip raw HTML,
scripts, event attributes, and unsafe URL schemes; proxy or block remote images,
send a restrictive referrer policy and CSP, and eventually use a separate
untrusted-content origin. Until those controls exist, C6 shows escaped source
instead of rich HTML. Agent-team text receives the same treatment and is never
silently inserted into an agent context.

New active command descriptors use argv arrays. They do not inherit the current
project manifest's shell-like `command` string. Bridging older project commands
requires an explicit future manifest-version adapter; the resolver must not
silently reinterpret them.

### 5.4 Requirements

Requirements are keyed by export and may declare maximum resources, network
destinations, repository access, secret slots, scratch storage, data services,
or future runtime capabilities. Passive exports must have no active
requirements.

For an active export, eventual authority is:

```text
effective capability = package requirement
                     ∩ consumer's explicit grant
                     ∩ caller's live C6 role
                     ∩ operator/runtime policy
                     ∩ enforcement available on this host
```

Intersection does not mean silently reducing a workload's needs. If the full
declared requirement is not granted and enforceable, preparation fails closed.
Dependencies cannot grant permissions to their parent, and a parent's grant is
not inherited by a dependency.

## 6. Dependencies and lockfiles

Version 1 supports **direct dependencies only**:

```toml
[[dependencies]]
alias = "writing-guide"
package = "cresix.com/acme/writing-guide"
version = "^2.1"
exports = ["style-guide"]
```

An alias is unique. References use qualified names such as
`writing-guide.style-guide`; dependency files or exports never merge implicitly
into the parent's namespace.

A version 1 dependency must not itself contain dependencies. Resolution rejects
that release with a typed `transitive_dependencies_unsupported` error. This
keeps the first resolver, permission review, provenance graph, and offline
cache understandable. A consumer that needs two reusable units lists both
directly. Cycles are therefore impossible by construction, but duplicate
aliases, incompatible duplicate package identities, excessive dependency
counts, and excessive total bytes still fail validation.

SemVer ranges belong in manifests. Exact source identities belong in locks.
An independently published C6R with dependencies commits an adjacent
`c6r.lock`. A consuming C6 project commits `c6.lock`. Both use the same entry
shape:

```toml
lock_version = 1

[[packages]]
alias = "release"
package = "cresix.com/acme/release-team"
version = "1.2.3"
git = "https://example.invalid/acme/automation.git"
git_object_format = "sha1"
commit = "0123456789abcdef0123456789abcdef01234567"
subdir = "components/release-team"
manifest_sha256 = "..."
content_sha256 = "..."
provenance_sha256 = "..."
```

Tags and branches may help a resolver discover a version but never appear as
the immutable authority in a lock. A production or published composition
rejects dirty worktrees, moving references, unresolved entries, and local path
overrides.

The lock records the Git object format. A full commit is 40 lowercase hex
characters for SHA-1 or 64 for SHA-256; the independent normalized
`content_sha256` remains the selected tree's content-integrity authority.

Remote acquisition is deny-by-default. Version 1 accepts reviewed HTTPS
origins only; it rejects `file:`, SSH, `ext::`, userinfo, query credentials,
redirects to another origin, submodules, and arbitrary Git remote helpers. Git
runs with prompts and inherited credential helpers/config disabled, a bounded
protocol allowlist, and explicit project-scoped read credentials when needed.
Pack bytes, object and ref counts, file counts, and time are bounded. Source
credentials never enter the lock, cache remote configuration, error text, or
logs. Support for another transport requires a separate threat review.

## 7. Safe paths, materialization, and cache

The source identity's `subdir` is resolved beneath the checked-out full commit.
Neither a manifest path nor a Git tree entry may escape it. Validation happens
against Git tree entries before materialization and again while writing the
bounded destination. Archive entries use `/` separators and canonical path
ordering; digests normalize metadata that should not affect content identity,
including timestamps and host ownership. Executable mode remains meaningful
for future active descriptors and is included in the digest.

Client cache entries live under the platform cache directory, conceptually:

```text
${XDG_CACHE_HOME}/c6/c6r/sha256/<content-digest>/
```

Server cache entries live beneath:

```text
${C6_DATA_DIR}/cache/c6r/sha256/<content-digest>/
```

The implementation uses platform-appropriate paths rather than assuming those
literal Unix variables. Cache directories are untrusted accelerators: every
read verifies the expected digest, writes use a temporary directory followed
by atomic rename, and a corrupt entry is quarantined or ignored. Cache presence
never grants authorization and cache deletion never changes the committed lock.

Passive exports materialize into a generated, read-only composition tree keyed
by the root composition digest:

```text
.c6/materialized/<composition-digest>/<component-alias>/<export-name>/
```

This directory is build output and is not committed. Components cannot overlay
the repository, one another, or C6's data directory. An `agent_team` export is
data at this stage: materialization must not make its referenced executable,
model, tool, or MCP endpoint available.

## 8. Provenance and publication

The minimum provenance statement binds:

- package identity and exact version;
- publisher account or self-hosted registry principal;
- source installation/project identity where applicable;
- Git remote, full commit, and subdirectory;
- manifest, lock, and content digests;
- schema, packer, and registry protocol versions;
- publication timestamp and registry identity.

A registry rejects publication of a second content digest for an existing
package identity and version. The release is immutable.

A hosted registry should sign an append-only publication statement. A
self-hosted registry has its own trust root. Signatures establish which
registry or publisher asserted the bytes; they do not establish that code is
safe, correct, non-malicious, or appropriately licensed. The web and CLI must
not collapse “signed,” “verified publisher,” and “safe to run” into one badge.

SBOMs, build attestations, transparency logs, vulnerability reports, and
source-to-image provenance are later additions. External APIs and model
providers may remain behaviorally nondeterministic even when source, config,
model identifier, and runtime version are pinned.

## 9. Using C6Rs in a project

A future project-manifest schema adds components without changing the meaning
of current `c6.toml` version 1 projects:

```toml
[[components]]
alias = "release"
package = "cresix.com/acme/release-team"
version = "^1.2"
exports = ["handbook", "release"]

[components.inputs]
model = "gpt-example"

[components.secrets]
model-api = "OPENAI_API_KEY"

[components.grants]
repository_write = "proposal"
network = ["api.openai.com:443"]
cpu = 1.0
memory_mb = 1024
```

The first implementation accepts only selected passive exports, so the active
grant block above can be validated and previewed but cannot be activated.
Selecting an active export produces a truthful `runtime_unavailable` result.

In a larger GitHub-style repository, local development may use a safe relative
path:

```toml
[[components]]
alias = "handbook"
path = "components/team-handbook"
exports = ["docs"]
```

Path dependencies are development inputs and make a composition
non-publishable. Publishing the component or root composition resolves that
path to the repository's full commit and subdirectory and writes the immutable
identity into the appropriate lock. No package may read its repository parent
or sibling directories merely because they share a Git checkout.

## 10. CLI workflow

C6R is the artifact format; it does not require a second package-manager
binary. The existing `c6` CLI should grow reviewable subcommands:

```text
c6 package init [--kind content|agent-team|...]
c6 package validate [--json]
c6 package pack [--json]
c6 package publish [--json]

c6 component add <package>@<constraint> --as <alias>
c6 component lock
c6 component graph
c6 component permissions
c6 component verify
c6 component materialize
c6 component update [alias]
```

`validate` parses manifests, paths, descriptors, references, and limits without
running code. `pack` produces a deterministic digest and archive. `add` first
shows the selected exports, direct dependencies, provenance, secret slots, and
capability requirements, then proposes manifest and lock changes. It never
runs the package. `update` shows version, source, digest, interface, dependency,
and permission changes before replacing the lock. Human output is a projection
of typed `--json` results.

Local path development is explicit and visibly non-reproducible. A developer
may materialize passive exports and inspect their exact generated tree. Future
sandbox tests for active exports must use fixtures and synthetic credentials,
not production secrets or ambient host authority.

## 11. Hub and discovery experience

Package URLs must not collide with project URLs. A hosted directory uses an
explicit route such as:

```text
https://cresix.com/r/<namespace>/<package>
https://cresix.com/r/<namespace>/<package>/v/<version>
```

The package page shows:

- README and archetype badges;
- exports and typed interfaces;
- direct dependencies;
- required permissions, network destinations, and secret slot names;
- exact version, source commit/subdirectory, digests, and provenance;
- yanked, deprecated, or advisory state;
- compatible C6/schema/runtime versions;
- a copyable install command and the diff it would propose.

Discovery may filter by export kind, protocol, runtime compatibility, license,
and permission footprint. Download count is not a trust score. `public`,
`workspace`, and `private` visibility require authorization on both metadata
and artifact retrieval; an unlisted URL is not access control.

Cresix Cloud is one optional directory and registry implementation. Standalone
C6 can use path and immutable Git dependencies or a self-hosted registry. A
Cresix account must not be required to parse, lock, cache, verify, or
materialize a C6R.

## 12. Examples

### Content-only team handbook

`acme/team-handbook` exports Markdown and images. It has no secrets,
requirements, commands, or dependencies. C6 can materialize and browse it in
the first phase.

### Release agent team

`acme/release-team` exports agent profiles, prompts, and skill metadata. The
configuration declares a model input, an API-key secret slot, network access,
and proposal-only repository writes. C6 can inspect the team today, but cannot
run it until the consumer maps the secret, grants the capabilities, and a
sandboxed agent runtime enforces them.

### Linear reader MCP server

`acme/linear-reader` exports an MCP descriptor requiring a `linear-token` slot
and outbound TLS to `api.linear.app:443`. The manifest and tool schema can be
validated. C6 does not start the process or connect it to an agent in the first
implementation.

### Status board mini app

`acme/statusboard` exports a web service, a polling job, static assets, and a
PostgreSQL requirement. It contains no database credential. Initial C6R support
can explain the composition and its unmet runtime capabilities, but cannot
build, host, schedule, migrate, or inject secrets into it.

The declared package file closure is not a hermetic build or runtime closure.
Cargo, npm, container bases, model providers, and other ecosystem dependencies
remain outside it. Active exports cannot execute until C6 defines required
ecosystem lockfiles, source-to-artifact provenance, offline or explicitly
bounded build networking, and verification of the resulting artifact. A C6R
Git/content pin alone does not make transitive build inputs reproducible or
trusted.

## 13. Lifecycle, update, and rollback

- Published versions are immutable. **Yank** prevents new resolution but does
  not invalidate existing locks. **Deprecation** names a successor and reason.
- A security advisory is structured metadata, separate from yank and
  deprecation. A registry never rewrites a user's lock remotely.
- Updating a component is an ordinary Git change: manifest and lock diff,
  validation, review, commit, and later deployment. Rollback reverts the Git
  change and uses the prior immutable digest.
- Namespace or package rename creates a new identity or an audited ownership
  transfer. Old names may redirect for discovery, but existing locks retain
  their original qualified identity and provenance.
- Unsupported manifest or lock schema versions fail closed. Migration tooling
  generates a reviewable diff; startup never rewrites manifests automatically.
- Runtime data belongs to a project component instance, not a package release.
  Future removal must enumerate schedules, grants, secret mappings,
  deployments, and persistent state before an explicit orphan or delete choice.
- A package update never runs a state migration automatically. Stateful
  migrations require a future, separately approved revision-pinned plan with
  backup and rollback semantics.
- A registry retains immutable release artifacts according to a declared
  retention policy. Local caches remain disposable and content-verified.

## 14. Staged roadmap

### Stage A: format and integrity

- strict manifest and lock DTOs;
- decision-test examples and negative fixtures;
- safe tree closure, deterministic packing and SHA-256 digest;
- path, symlink, normalization, size, collision, and archive abuse tests;
- local path and full Git-commit sources.

No materialization or remote publication is implied.

### Stage B: small composition

- direct dependency resolution and `transitive_dependencies_unsupported`;
- semantic-version selection with exact lock output;
- conflict, count, size, and provenance verification;
- typed input, secret-slot, requirement, and permission-diff output;
- passive `content` and `agent_team` materialization only.

The UI and CLI say `validated`, `locked`, and `materialized`, never `installed`,
`running`, `connected`, or `deployed`.

### Stage C: self-hostable sharing

- immutable artifact and provenance publication/retrieval;
- workspace/private authorization and revocation;
- Hub package pages, search, versions, yank, and deprecation;
- offline content-addressed cache verification.

The registry API is self-hostable; Cresix Cloud is not mandatory.

### Stage D: active preparation

- resolve active descriptors into an immutable `ResolvedComposition`;
- capability, secret, network, resource, and repository-grant review;
- runtime compatibility and provenance gates;
- recorded unavailable states while execution remains disabled.

### Stage E: sandboxed execution

Enable one active archetype at a time only after its corresponding C6 runtime
milestone has end-to-end and abuse-case evidence. Command execution, MCP
attachment, agent invocation, schedules, services, apps, workflows, secret
injection, and repository proposal writes are separate gates. Enabling one does
not implicitly enable the others.

## 15. Explicit deferrals

- transitive dependency resolution and cross-registry graphs;
- more than one version of a package identity in one composition;
- feature unification or conditional dependency expressions;
- mutable channels such as `latest` as lock identities;
- build, install, lifecycle, or migration scripts;
- automatic execution after add, lock, materialize, publish, or update;
- workflow expression languages and active DAG scheduling;
- dynamic C6 server plugins;
- OCI images as the package or source authority;
- automatic Dockerfile generation;
- stateful application migrations;
- hosted hostile multi-tenant execution;
- trust scores, ratings, full transparency logs, SBOMs, and vulnerability
  scanning;
- registry federation and global namespace consensus.

## 16. Architectural tensions

### Repository versus package

“C6 repository” is memorable, but a one-to-one repository rule would fight
monorepos and make small components expensive. C6R therefore means a safe Git
tree closure. A whole repository is simply the convenient case where the
manifest root and repository root coincide.

### Git truth versus durable distribution

Retaining an immutable artifact duplicates Git bytes, but locks are not useful
if the only source disappears. The registry snapshot is a verified cache bound
to Git provenance, never an editable second source of truth.

### One artifact family versus one permission model

Content, agents, MCPs, and applications have different risks. They share a
strict package envelope, while typed export schemas and deny-by-default
requirements preserve their different security meanings. Passive support must
not become a shortcut for active execution.

### Convenience versus least privilege

Automatic transitive installation and inherited grants would make composition
easy but security review opaque. Version 1 uses direct dependencies, explicit
exports, explicit secret mappings, and explicit grants so the complete boundary
fits in one review.

### SemVer versus behavioral trust

SemVer describes an interface promise, not safety or deterministic behavior.
Every update remains a reviewable source, provenance, dependency, interface,
and permission change—even when its version range says it is compatible.

### Cloud discovery versus self-hosted authority

The registry is a protocol and storage boundary. Cresix Cloud may provide the
best hosted experience, but local validation, Git pinning, locks, caches, and a
self-hosted registry remain first-class so C6R does not make sovereign C6
installations dependent on one vendor.
