# C6 manual

C6 is an early, self-hosted control plane for small software. This manual
separates the working local collaboration system from the larger product idea.

## Start here

- [Product](PRODUCT.md): users, use cases, principles, and non-goals
- [Glossary](GLOSSARY.md): one meaning for every C6 term
- [Architecture](ARCHITECTURE.md): process and persistence boundaries
- [Roadmap](ROADMAP.md): implemented and deferred capabilities
- [Cresix Cloud connected-mode specification](specs/CRESIX_CLOUD_CONNECTED_MODE.md):
  optional accounts, directory, installation registration, and managed relay

## Trust and interfaces

- [Trust model](TRUST_MODEL.md): bootstrap, invitations, sessions, and lockout
- [Authorization](AUTHORIZATION.md): server administration and cumulative roles
- [HTTP API](API.md): endpoints, errors, cookies, CSRF, and example requests
- [CLI](CLI.md): server replacement checks, credentials, clone, remote setup, and diagnostics
- [Git](GIT.md): local repositories and authenticated read-only smart HTTP
- [Runner](RUNNER.md): authenticated protocol and simulation-only backend
- [Scheduler](SCHEDULER.md): validated cron semantics without dispatch

## Build and operate

- [Deployment](DEPLOYMENT.md): source, Compose, laptop, LAN, and reverse proxy patterns
- [Configuration](CONFIGURATION.md): environment variables and secure defaults
- [Operations](OPERATIONS.md): bootstrap, data, backup, restore, and upgrades
- [Storage](STORAGE.md): SQLite/Git ownership and consistency
- [Testing](TESTING.md): local QA and regression suites
- [Dogfood](DOGFOOD.md): what C6 tests about itself
- [Examples](EXAMPLES.md): versioned project-manifest examples

## Product quality and security

- [Web product](WEB.md): information architecture and truth conventions
- [Threat model](THREAT_MODEL.md): assets, boundaries, abuse cases, mitigations
- [Security policy](../SECURITY.md): deployment warning and reporting

## Design records

- [Phase 2 Git and CLI specification](specs/PHASE_2_GIT_AND_CLI.md): the staged
  read-only transport and protected-push design. Its “current state” section is
  the baseline when the spec was written; this manual records implemented truth.
- [ADR 0001](decisions/0001-single-authority-self-hosting.md): one authority
  per sovereign installation and replaceable ingress
- [ADR 0002](decisions/0002-optional-cresix-cloud-directory-and-relay.md): keep
  standalone sovereignty while adding an optional global directory and relay
- [Agent-first runtime specification](specs/AGENT_FIRST_RUNTIME.md): pollable
  APIs, schedules, runtime adapters, secrets, approvals, and future MCP
- [C6R composition specification](specs/C6R_COMPOSITION.md): immutable reusable
  content and agent-team composition, with active runtimes deliberately deferred

Every page uses **implemented** for behavior present and tested in this
revision, and **deferred** for design intent. A type, UI mock, or manifest field
alone is not treated as a running feature.
