# Cresix product handbook

This folder explains what Cresix is, who it is for, what exists today, and how
the product is intended to grow. It is a product handbook, not an API manual or
a promise that every described direction has shipped.

## Status language

Every capability in this handbook uses one of three labels:

- **Now** means implemented in this repository and covered by the current local
  verification suite. A qualification beside the label is part of the claim.
- **Next** means the next coherent product boundary we intend to earn. It is a
  direction, not a release date or compatibility promise.
- **Later** means a plausible extension that remains contingent on observed
  demand, security work, and architectural fit.

Detailed specifications may describe more than the implementation. The
[capability ledger](CAPABILITIES.md) is the product-level source of truth for
what users can rely on in this revision.

## Read the product

1. [Vision](VISION.md) — the problem, thesis, and desired end state
2. [Principles](PRINCIPLES.md) — rules used to make product decisions
3. [Users and use cases](USERS_AND_USE_CASES.md) — who C6 serves and why
4. [Product model](PRODUCT_MODEL.md) — C6, Hub, Admin, Cloud, the connector,
   CLI, Git, and C6Rs
5. [Capabilities](CAPABILITIES.md) — an evidence-linked Now/Next/Later ledger
6. [Core workflows](WORKFLOWS.md) — the journeys the product should make easy
7. [Collaboration and sharing](COLLABORATION_AND_SHARING.md) — trust,
   invitations, Git collaboration, and remote reachability
8. [Agent-centric product](AGENT_CENTRIC.md) — how agents participate without
   gaining ambient authority
9. [C6Rs](C6R.md) — reusable small-software components and their boundaries
10. [Deployment modes](DEPLOYMENT_MODES.md) — laptop, server, VM, standalone,
    and connected operation
11. [Non-goals](NON_GOALS.md) — constraints that keep Cresix understandable
12. [Roadmap](ROADMAP.md) — staged outcomes rather than a feature wish list
13. [Open questions](OPEN_QUESTIONS.md) — decisions that require evidence

## Product and engineering documentation

This handbook answers “why” and “for whom.” The existing technical manual
answers “how”:

- [Capability ledger](CAPABILITIES.md)
- [Architecture handbook](../architecture/README.md)
- [Product roadmap](ROADMAP.md)
- [Glossary](../GLOSSARY.md)
- [Trust model](../TRUST_MODEL.md)
- [Deployment guide](../DEPLOYMENT.md)
- [Cresix Cloud connected-mode specification](../specs/CRESIX_CLOUD_CONNECTED_MODE.md)
- [Agent-first runtime specification](../specs/AGENT_FIRST_RUNTIME.md)
- [C6R composition specification](../specs/C6R_COMPOSITION.md)

When this handbook and executable behavior disagree, the implementation and
its tests determine what exists; the capability ledger and affected handbook
page must then be corrected together.
