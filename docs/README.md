# C6 manual

C6 is an early, self-hosted control plane for small software. This manual
separates the working local collaboration system from the larger product idea.

## Start here

- [Product](PRODUCT.md): users, use cases, principles, and non-goals
- [Glossary](GLOSSARY.md): one meaning for every C6 term
- [Architecture](ARCHITECTURE.md): process and persistence boundaries
- [Roadmap](ROADMAP.md): implemented and deferred capabilities

## Trust and interfaces

- [Trust model](TRUST_MODEL.md): bootstrap, invitations, sessions, and lockout
- [Authorization](AUTHORIZATION.md): server administration and cumulative roles
- [HTTP API](API.md): endpoints, errors, cookies, CSRF, and example requests
- [Git](GIT.md): local repositories and the absent network transport
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

Every page uses **implemented** for behavior present and tested in this
revision, and **deferred** for design intent. A type, UI mock, or manifest field
alone is not treated as a running feature.
