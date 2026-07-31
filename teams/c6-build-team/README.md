# c6-build-team

`c6-build-team` is the versioned product-engineering team for C6. It is designed
to take one bounded feature or bug from clarified intent through implementation,
security review, verification, and operator-facing documentation.

The configuration is intentionally plain TOML and Markdown. C6 does not yet
execute team manifests, so [`team.toml`](team.toml) is the runtime contract being
developed alongside the agent runner. The QA scripts in [`qa/`](qa/) work now.

## Team shape

| Agent | Owns | Default output |
| --- | --- | --- |
| `lead` | Scope, delegation, integration, final evidence | Delivery report |
| `product-architect` | Boundaries, contracts, data/control flow | Design note |
| `rust-platform` | Rust domain, API, persistence, scheduler | Code + tests |
| `web-experience` | React product experience and accessibility | Code + UI tests |
| `identity-security` | Pairing, authorization, credentials, abuse cases | Security review |
| `runtime-isolation` | Runner, containers, networking, secrets | Isolation review/code |
| `qa-reliability` | Test strategy, failure reproduction, release gates | Verification report |
| `docs-operator` | Setup, migrations, backup, failure recovery | Documentation |
| `release-reviewer` | Independent final diff and evidence review | Approve/block verdict |

## Workflow

1. `lead` turns the request into acceptance criteria and identifies affected
   trust boundaries.
2. `product-architect` is used only when the change alters a public contract,
   ownership boundary, persistence, execution, or deployment topology.
3. The relevant implementation agents work on non-overlapping surfaces.
4. `identity-security` or `runtime-isolation` reviews any affected trust boundary.
5. `qa-reliability` runs targeted tests and then the full release gate.
6. `docs-operator` updates user/operator material when behavior or operations change.
7. `release-reviewer` independently checks the final diff and evidence.

Agents never share credentials. Any agent-generated repository change must land
on a proposal branch and must not bypass the final reviewer.

## Run the QA tooling

Fast structural validation:

```bash
bash teams/c6-build-team/qa/validate-team.sh
```

Complete repository gate:

```bash
bash teams/c6-build-team/qa/check.sh
```

Live API smoke test on an isolated port:

```bash
bash teams/c6-build-team/qa/smoke.sh
```

The smoke test starts and stops its own C6 server. Set `C6_QA_PORT` to override
its default port.

