# c6-build-team

`c6-build-team` is C6's versioned product-engineering playbook. It is designed
to take a bounded change—or a coordinated MVP slice—from clarified intent
through implementation, security review, local regression testing, and
operator-facing documentation.

The configuration is intentionally plain TOML and Markdown. C6 does not yet
execute team manifests, so [`team.toml`](team.toml) is the runtime contract being
developed alongside the agent runner. The QA scripts in [`qa/`](qa/) work now.

## Team shape

| Agent | Owns | Default output |
| --- | --- | --- |
| `lead` | Scope, delegation, integration, final evidence | Delivery report |
| `product-architect` | Boundaries, contracts, data/control flow | Design note |
| `core-contracts` | Stable domain, wire, lifecycle, and permission types | Code + tests |
| `git-service` | Validated disk-backed Git operations | Code + tests |
| `server-control-plane` | HTTP, embedded SQLite, auth, audit, orchestration | Code + tests |
| `cloud-contracts` | Cloud identifiers, catalog, route, and relay wire contracts | Code + tests |
| `cloud-control-plane` | Global accounts/namespaces, installation registry, directory, relay authority | Code + tests |
| `connector-relay` | Fixed-upstream outbound connector and bounded relay transport | Code + tests |
| `rust-platform` | Cross-crate Rust integration | Code + tests |
| `web-experience` | React product experience and accessibility | Code + UI tests |
| `cloud-web-experience` | Cresix account, workspace, installation, and directory UX | Code + UI tests |
| `identity-security` | Pairing, authorization, credentials, abuse cases | Security review |
| `runtime-isolation` | Typed runner boundary, simulation, future isolation | Isolation review/code |
| `qa-reliability` | Test strategy, failure reproduction, release gates | Verification report |
| `docs-operator` | Setup, migrations, backup, failure recovery | Documentation |
| `release-reviewer` | Independent final diff and evidence review | Approve/block verdict |

## Workflow

1. `lead` turns the request into acceptance criteria and identifies affected
   trust boundaries.
2. `product-architect` is used only when the change alters a public contract,
   ownership boundary, persistence, execution, or deployment topology.
3. The relevant implementation agents work on non-overlapping surfaces. The
   normal MVP split is contracts, Git, control plane, runner, and web.
4. `identity-security` or `runtime-isolation` reviews any affected trust boundary.
5. `qa-reliability` runs targeted tests and then the full release gate.
6. `docs-operator` updates user/operator material when behavior or operations change.
7. `release-reviewer` independently checks the final diff and evidence.

Agents never share credentials or embed them in fixtures. Branch, commit, push,
and release actions follow the repository owner's explicit authority and must
not bypass the final reviewer.

## Product assumptions

Standalone C6 is one self-hosted authority backed by embedded SQLite and disk
Git repositories. Native peer trust means bootstrap plus invitation-based
device enrollment; it does not mean decentralized replication, and an IP
address is never an identity. The local web application is same-origin.

Optional connected mode adds a separate Cresix Cloud account/directory authority
and a least-privilege outbound connector. Cloud catalog data is discovery, not
local authorization or source truth. Cloud account sessions never authenticate
local C6, and standalone startup cannot require Cloud. The relay is trusted TLS
ingress and can observe traffic; it is not end-to-end encrypted. Production
identity/recovery, public abuse controls, relay isolation/HA, real workload
execution, federation, and plugin infrastructure remain deferred until
implemented and tested.

## Run the QA tooling

Run every blocking local gate (the release default):

```bash
bash teams/c6-build-team/qa/all.sh
```

Fast structural validation:

```bash
bash teams/c6-build-team/qa/validate-team.sh
```

Complete repository gate:

```bash
bash teams/c6-build-team/qa/check.sh
```

Security-focused static and regression gate:

```bash
bash teams/c6-build-team/qa/security.sh
```

Connected Cloud contract, service, connector, and web component gate:

```bash
bash teams/c6-build-team/qa/cloud.sh
```

Headless browser, responsive, and accessibility gate:

```bash
bash teams/c6-build-team/qa/e2e.sh
```

Live API smoke test on an isolated port:

```bash
bash teams/c6-build-team/qa/smoke.sh
```

Self-hosting dogfood (fresh C6, typed runner, then real-backend browser):

```bash
bash teams/c6-build-team/qa/dogfood.sh
```

The smoke test starts and stops its own C6 server. Set `C6_QA_PORT` to override
its default port. These are local gates by design; this playbook does not add or
require hosted CI.

The MVP currently records run intent through HTTP and verifies the typed runner
independently. Until the server dispatches that intent over the runner socket,
the dogfood gate must be reported as two verified boundaries—not as proof of an
end-to-end executed workload.

Similarly, the Cloud component gate does not by itself prove a request crossed
the relay into a local installation. The connected dogfood claim requires a
fresh Cloud and C6 lifecycle, real connector, directory handoff, revoke/disconnect
checks, and standalone survival. Missing that lifecycle must be reported as an
open gate, not silently downgraded to component coverage.

See [`TEST_MATRIX.md`](qa/TEST_MATRIX.md) for release coverage and
[`TEAM_SCHEMA.md`](TEAM_SCHEMA.md) for the manifest conventions.
