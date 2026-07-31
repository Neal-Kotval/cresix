# Vision

## Small software should be ordinary

Agents have made it inexpensive to create software for one person, one team,
or one peculiar workflow. Deployment and collaboration have not become equally
simple. A useful internal tool still tends to inherit infrastructure designed
for products with large audiences: several hosted services, deployment
pipelines, identity integration, secret plumbing, and operational ceremony.

Cresix exists to make purpose-built software as easy to keep, run, inspect, and
share as a document—without pretending software has the same security model as
a document.

We call this category **small software**:

- a tool whose useful audience may be one person or a small team;
- a wiki, dashboard, workflow aid, MCP server, agent team, scheduled report, or
  mini application built for a specific context;
- software where understandability and low operational cost matter more than
  hyperscale; and
- software that may later become a component of a larger project.

“Small” describes scope and audience, not importance or engineering quality.

## Product thesis

Cresix should be a compact, self-hostable software forge with first-class
hosting and agent affordances. It should keep source, collaboration context,
declared runtime intent, and operational history close together. Git remains
the source authority; a small local control plane supplies identity,
authorization, metadata, and a future safe runtime boundary.

The product is deliberately split:

- **C6** is the sovereign installation that owns local people, permissions,
  repositories, and operational state.
- **C6 Hub** is the collaboration surface for workspaces and projects.
- **C6 Admin** is the installation-operations surface.
- **Cresix Cloud** targets optional global account handles, account-scoped
  workspaces, a directory, and managed reachability. Today's implementation is
  a loopback dogfood service without public handles.
- **C6Rs** are the proposed composition unit for bringing small, reviewable
  capabilities into C6 projects or larger Git-hosted projects.

This yields a simple long-term promise:

> Build locally, keep authority understandable, share remotely, and compose
> the useful parts elsewhere.

## The experience we want

A solo builder should be able to start C6 on a laptop, claim it, create a
project, and understand where every durable byte lives. A teammate should be
able to receive an invitation, open Hub, clone with ordinary Git, and see what
the project declares. An operator should be able to move the same installation
to an always-on Linux box without redesigning the product.

When optional Cresix Cloud reaches production maturity, the builder should be
able to reserve a stable workspace URL and connect a private installation with
an outbound connector—without opening a router port or transferring local
source and authorization into Cloud.

Agents should use the same explicit authority as people. They should inspect
typed state, propose reviewable changes, trigger narrowly granted work, and
poll durable outcomes. They should not scrape privileged pages, borrow an
owner's credentials, or acquire the host's ambient permissions.

## What success looks like

Success is not “a smaller GitHub” measured by feature count. It is:

- a person can go from a repository to a shared, understandable tool with very
  little infrastructure knowledge;
- a team can inspect who can do what and revoke that access in one place;
- the same installation works on a laptop and an inexpensive server;
- an agent can discover capabilities and act through stable, auditable
  interfaces;
- reusable content and agent configurations can be pinned, reviewed, updated,
  and removed like source dependencies; and
- Cresix stays useful without a mandatory Cresix-operated service.

## Current horizon

- **Now:** C6 provides the local collaboration/control-plane foundation,
  read-only Git transport, a thin CLI, recorded runtime intent, a simulation
  runner boundary, and a loopback-only Cloud/connector dogfood slice.
- **Next:** make core Git collaboration complete, verify backup/restore, add a
  production-shaped isolated relay journey, and establish passive C6R
  materialization.
- **Later:** safely execute jobs, schedules, agents, and hosted applications;
  add secrets and pollable/MCP interfaces; then consider company identity and
  stronger multi-tenant isolation when real deployments require them.

See [Capabilities](CAPABILITIES.md) for precise current qualifications.
