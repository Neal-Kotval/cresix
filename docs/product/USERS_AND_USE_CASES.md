# Users and use cases

## Primary users

### Solo builder

The solo builder makes a tool for a personal workflow, a client, a research
project, or a small community. They value a one-machine start, local state,
ordinary Git, and the ability to understand failure without learning a cloud
platform.

Jobs to be done:

- create a project and repository quickly;
- inspect and clone it with familiar tools;
- record how it should run;
- move it from a laptop to an always-on host; and
- share it selectively when it becomes useful to someone else.

### Small trusted team

The team builds bespoke tools, dashboards, wikis, prototypes, and automations.
It wants one collaboration space without adopting the process surface of a
large public forge.

Jobs to be done:

- discover team projects in one workspace;
- grant and revoke understandable roles;
- review proposed source changes;
- share a stable project doorway with remote teammates; and
- see declared schedules, deployments, and agent work without confusing intent
  with completed execution.

### Installation operator

The operator may be the builder or a technically responsible teammate. They own
the machine, ingress, upgrades, backups, and bootstrap administrator session.

Jobs to be done:

- claim and secure an installation;
- control global peer enrollment and revocation;
- diagnose storage, identity, Git, connector, and runtime health;
- migrate the complete data boundary; and
- know which availability and security properties C6 does not provide.

### Contributor

A contributor needs project-level participation, not server administration.
They primarily use Hub, the CLI, and Git. They should be able to propose work
without receiving installation-wide powers.

### Agent author or operator

This user turns recurring work into an agent team, job, schedule, MCP service,
or reusable C6R. They need immutable inputs, narrow grants, deterministic
status, and reviewable outputs—not a shell with the owner's ambient secrets.

### Cloud account holder

In the intended production connected mode, this person reserves a globally
unique namespace and manages an installation registration. The current preview
enforces uniqueness only inside one running Cloud database. Their Cresix Cloud
identity is deliberately separate from every installation-local peer in the
current design.

## Representative small software

Good fits include:

- a team handbook or project wiki;
- a sprint, incident, inventory, or experiment tracker;
- a scheduled report built from a narrow data source;
- a tiny internal dashboard or approval tool;
- a custom read-only MCP integration;
- an agent team definition for release, research, or QA work;
- a static microsite or prototype; and
- a reviewable bundle of prompts, instructions, templates, and schemas used
  inside a larger repository.

These examples describe the category. C6 does **not** execute or host all of
them today; see [Capabilities](CAPABILITIES.md).

## Poor fits today

C6 is not currently suitable for:

- anonymous public applications or hostile user-generated code;
- high-availability services with strict uptime requirements;
- large organizations that require recoverable SSO, SCIM, formal retention,
  or mature incident tooling immediately;
- compute-heavy or high-throughput workload orchestration;
- multi-region, multi-writer collaboration; or
- secrets-bearing production agents, because real workload execution and
  secret injection remain deferred.

## Trust assumption

The current release is for a solo user or small team that does not intentionally
run hostile code against the host. C6 still defends important control-plane
boundaries such as authorization, token replay, cross-site mutation, unsafe
paths, and malformed runner input. That is not equivalent to a hardened
multi-tenant sandbox.

## Adoption progression

A healthy adoption path is incremental:

1. **Now:** run standalone on loopback and use it as a local forge.
2. **Now:** invite trusted peers after providing a trusted HTTPS origin.
3. **Now, dogfood:** evaluate Cloud directory and outbound relay components on
   loopback; do not treat them as production public hosting.
4. **Next:** migrate an active team installation to an always-on Linux box and
   verify backup/restore.
5. **Later:** enable runtime, agent, secret, and company identity modules only
   after their security gates are implemented.
