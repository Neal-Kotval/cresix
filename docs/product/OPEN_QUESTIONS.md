# Open product questions

These are unresolved choices, not hidden roadmap commitments. Each question
includes the evidence that should drive a decision.

## Which small-software category should be the first runtime wedge?

Candidates include static sites, scheduled reports, internal dashboards, and
read-only MCP servers. Static sites have the smallest secret and process
surface; scheduled reports test the agent/schedule thesis more directly.

Decide from repeated dogfood usage, required integrations, and the smallest
runtime slice that can meet the full security and rollback gates.

## What is the first indispensable C6R shape?

The likely starting set is `content` and `agent_team`, because both can be
materialized without executing code. The open question is whether a team wiki,
QA/build-team definition, or general template bundle produces the clearest
cross-project value.

Decide after testing real candidate repositories against the selection test,
safe-closure rules, update diffs, and removal semantics.

## Should materialized C6R content be committed?

Options include always committing generated content, always reproducing it from
the lock, or a per-package policy. Committing improves ordinary forge review
and offline legibility; reproduction reduces copied source but depends more on
resolver availability and deterministic tools.

Prefer committed passive content initially unless repository size or generated
noise proves materially harmful.

## How should production Cresix accounts authenticate?

Passkeys reduce password handling; OIDC fits companies; both require immutable
provider-subject linking, recovery, recent-auth controls, and abuse operations.
Email magic links are easy to start but make email delivery and mailbox security
part of the authority.

Decide with a dedicated threat model and recovery design before public hosting.
Do not reuse the loopback bootstrap mechanism on the public internet.

## How do Cloud members become local collaborators?

Current Cloud and local identities are separate by design. Possible future
models are explicit local invitation after Cloud discovery, a reviewed account
link, or company identity synchronization. Silent Cloud-to-local role mapping
would widen the Cloud trust boundary.

Prefer explicit local acceptance until real team workflows demonstrate that a
linking protocol is worth its recovery and offboarding complexity.

## What should stable sharing URLs guarantee?

A directory URL can remain stable while an installation is offline or its relay
route changes. The target hierarchy is
`/@{account}/{workspace}/{project}`; the preview's two-segment route is
temporary. Open questions include immutable account identity versus mutable
handle, account/workspace rename redirects, collision and tombstone/reuse
policy, migration of preview URLs, custom domains, private-result disclosure,
and how long stale catalog entries remain visible.

Decide before production account-handle launch; the current preview has no
public account handle and workspace uniqueness exists only within one running
Cloud database.

## Is trusted relay termination acceptable for target users?

The proposed Cresix edge can observe relayed cookies, credentials, source, and
bodies. This matches ordinary managed ingress but may not satisfy some company
or regulated deployments. Relay-blind transport would substantially change
browser authentication, routing, abuse handling, and debugging.

Measure deployment requirements before designing end-to-end encryption. Keep
standalone operator-managed ingress available regardless.

## Where is the boundary between C6 and a larger forge?

C6 should remain excellent for small software while letting C6Rs flow into
GitHub/Codeberg projects. Open questions include publishing/discovery, provenance
signing, lock update automation, and whether C6 ever mirrors metadata back to a
larger forge.

Prefer Git-native artifacts and explicit adapters over a bidirectional metadata
sync until a concrete workflow demands it.

## Which secret model should ship first?

A native encrypted store improves self-contained operation; reference adapters
to 1Password or Doppler reduce duplicate custody for teams already using them.
Both still need per-job grants, injection, redaction, loss behavior, and audit.

Decide from deployment evidence and backup/recovery design. Never default to
borrowing environment variables or the operator's agent credentials.

## When is Docker isolation enough?

For a trusted small team, a hardened container can be a practical accident
boundary. It is not sufficient for hostile public code. The product must define
which threat model a deployment selects and prevent the weaker boundary from
being marketed as multi-tenant isolation.

Use observed workload trust and company requirements to decide when remote
runners or microVMs become necessary.

## When should company identity be added?

OIDC, SCIM, group sync, administrator transfer, and policy controls can make C6
useful inside companies, but may overwhelm the small-team core. Add them after
recoverable native administration and real company adoption show which adapter
surface is needed.

## What evidence would justify federation?

Federation should be reconsidered only if independent organizations must accept
writes during partitions, no one can host a canonical authority, or residency
requirements cannot be met with export, mirrors, and remote runners. A new ADR
would need identity, conflict, revocation, audit, Git-ref, secret, scheduler,
recovery, compatibility, and rollback semantics.

Until then, one write authority per project is a product constraint.

## How this list is maintained

Close a question only with one of:

- a documented decision and its evidence;
- an explicit decision to defer with a revisit trigger; or
- a shipped implementation whose behavior and regression tests answer it.

New questions should identify the user outcome, trust boundary, reversible
options, and evidence needed. They should not be used as an unprioritized idea
backlog.
