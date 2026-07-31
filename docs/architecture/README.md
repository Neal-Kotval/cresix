# Cresix architecture handbook

This handbook is the architectural map for Cresix (C6). It explains which
process owns which decisions, where data lives, how trust crosses a boundary,
and how the implemented dogfood slice differs from the intended product.

It is deliberately a synthesis, not a second normative specification. Code and
tests determine current behavior; the product capability ledger records its
user-visible status; specifications define target contracts; and ADRs record
decision rationale. A target specification never overrides contrary runtime
evidence about what ships today.

## Status vocabulary

Every page uses the same labels:

- **Implemented** — present in this repository and covered by local tests.
- **Dogfood** — implemented for trusted, loopback evaluation but explicitly
  unsuitable for hostile public or company-production traffic.
- **Target** — agreed direction with a defined boundary, but not implemented.
- **Exploratory** — a possible extension that still requires evidence and an
  ADR or specification.

Validation, metadata recording, or a simulated runner is not described as
execution. A Cloud directory entry is not described as local authorization.

## Reading paths

For product and engineering orientation, read:

1. [System context](SYSTEM_CONTEXT.md)
2. [Components and boundaries](COMPONENTS.md)
3. [Domain and data ownership](DOMAIN_AND_DATA.md)
4. [Trust and authorization](TRUST_AND_AUTHORIZATION.md)
5. [Deployment topologies](DEPLOYMENT_TOPOLOGIES.md)

For implementation work, continue with:

- [Storage and consistency](STORAGE_AND_CONSISTENCY.md)
- [API and protocol boundaries](API_AND_PROTOCOLS.md)
- [Connector and relay](CONNECTOR_AND_RELAY.md)
- [Failure modes and operability](FAILURE_MODES_AND_OPERABILITY.md)
- [Extension points](EXTENSION_POINTS.md)

Future product architecture is separated so it cannot be mistaken for current
capability:

- [C6R composition architecture](C6R_ARCHITECTURE.md)
- [Agent and runtime architecture](AGENT_AND_RUNTIME_ARCHITECTURE.md)
- [Architecture roadmap](ROADMAP.md)

## Related authoritative sources

- [ADR 0001: one sovereign authority per installation](../decisions/0001-single-authority-self-hosting.md)
- [ADR 0002: optional Cloud directory and relay](../decisions/0002-optional-cresix-cloud-directory-and-relay.md)
- [Connected-mode specification](../specs/CRESIX_CLOUD_CONNECTED_MODE.md)
- [C6R composition specification](../specs/C6R_COMPOSITION.md)
- [Agent-first runtime specification](../specs/AGENT_FIRST_RUNTIME.md)
- [Git and CLI specification](../specs/PHASE_2_GIT_AND_CLI.md)

The [capability ledger](../product/CAPABILITIES.md) is the product-level status
authority. The older [architecture overview](../ARCHITECTURE.md) is retained
only as a compatibility link to this handbook.
