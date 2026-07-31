# C6 documentation

C6 is an early, self-hosted control plane and software forge for small
software. These documents deliberately separate product intent, current
capability, system architecture, target specifications, and operational
reference so a design is never mistaken for shipped behavior.

## Start here

- [Product handbook](product/README.md): vision, users, principles, workflows,
  C6Rs, agent direction, deployment choices, non-goals, and roadmap
- [Capability ledger](product/CAPABILITIES.md): the canonical product-level
  answer to what works now, what is dogfood-only, and what is deferred
- [Architecture handbook](architecture/README.md): authority, components, data,
  trust boundaries, deployment topologies, protocols, failures, and evolution
- [Glossary](GLOSSARY.md): one meaning for every C6 term

## Document authority

| Question | Canonical source |
| --- | --- |
| Why does C6 exist, and for whom? | [Product handbook](product/README.md) |
| What capability can a user rely on now? | [Capability ledger](product/CAPABILITIES.md), verified against code and tests |
| Which component owns a decision or datum? | [Architecture handbook](architecture/README.md) |
| What is an exact current interface or operator procedure? | The relevant reference/manual page below |
| What contract are we trying to implement? | A status-labelled document in [`specs/`](specs/) |
| Why was an architectural choice accepted? | An ADR in [`decisions/`](decisions/) |

Code and regression tests are the ultimate evidence for implemented behavior.
Specifications can describe unimplemented targets; ADRs record rationale and
do not prove delivery. When documentation disagrees with behavior, correct the
capability ledger and the affected page together.

## Trust and interfaces

- [Trust model](TRUST_MODEL.md): bootstrap, invitations, sessions, and lockout
- [Authorization](AUTHORIZATION.md): server administration and cumulative roles
- [HTTP API](API.md): endpoints, errors, cookies, CSRF, and examples
- [CLI](CLI.md): credentials, clone, remote setup, and diagnostics
- [Git](GIT.md): local repositories and authenticated read-only smart HTTP
- [Runner](RUNNER.md): authenticated protocol and simulation-only backend
- [Scheduler](SCHEDULER.md): validated cron semantics without dispatch

## Build and operate

- [Deployment](DEPLOYMENT.md): source, Compose, laptop, LAN, and proxy patterns
- [Configuration](CONFIGURATION.md): environment variables and secure defaults
- [Operations](OPERATIONS.md): bootstrap, data, backup, restore, and upgrades
- [Storage](STORAGE.md): SQLite/Git ownership and consistency
- [Testing](TESTING.md): local QA and regression suites
- [Dogfood](DOGFOOD.md): what C6 proves about itself
- [Examples](EXAMPLES.md): versioned project-manifest examples
- [Web product](WEB.md): information architecture and truth conventions
- [Threat model](THREAT_MODEL.md): assets, boundaries, abuse cases, mitigations
- [Security policy](../SECURITY.md): deployment warning and reporting

## Specifications and decisions

- [Cresix Cloud connected mode](specs/CRESIX_CLOUD_CONNECTED_MODE.md)
- [C6R composition](specs/C6R_COMPOSITION.md)
- [Agent-first runtime](specs/AGENT_FIRST_RUNTIME.md)
- [Git and CLI](specs/PHASE_2_GIT_AND_CLI.md)
- [ADR 0001: sovereign installations](decisions/0001-single-authority-self-hosting.md)
- [ADR 0002: optional Cloud directory and relay](decisions/0002-optional-cresix-cloud-directory-and-relay.md)

Every capability claim should use the handbook vocabulary: **Now**,
**Now, dogfood**, **Next**, or **Later**. Validation, a schema, a UI fixture, or
recorded intent alone is not execution.
