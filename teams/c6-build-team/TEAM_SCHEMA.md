# Team manifest conventions

`team.toml` is a versioned, human-readable coordination contract. C6 does not
execute it yet. The local validator enforces the subset below so the playbook
can evolve without pretending a runtime exists.

## Top level

| Key | Meaning |
| --- | --- |
| `version` | Manifest format version; currently `1` |
| `name` | Stable team name |
| `description` | Human-readable purpose |
| `orchestrator` | ID of one declared agent |
| `max_parallel` | Advisory concurrency cap, from `1` through declared agents |

`[policy]` records default repository, secret, network, and review constraints.
Every `[[agents]]` entry has an `id` and a config path. Every `[[gates]]` entry
has a unique name and a repository-relative `bash` command under this team's
`qa/` directory.

## Agent config

An agent TOML file declares a matching unique `id`, display `name`, `runtime`,
`role`, Markdown `instructions`, output kind, `may_edit`, and one or more owned
file globs in `scope`. `delegates` is valid only for an orchestrator. Scope is a
coordination boundary, not an operating-system sandbox.

## Local gate contract

All gate scripts use Bash strict mode, resolve the repository from their own
location, avoid credentials and organization data, and return non-zero for a
blocking failure. They must be runnable without a hosted CI service. A skipped
tool or scenario is not a pass: scripts either fail with an actionable install
message or the delivery report records why the gate was not required.

`validate-team.sh` checks this structural contract. `check.sh` runs compilation,
lint, unit/component tests, production web build, topology validation, and basic
secret hygiene. `security.sh` checks forbidden trust-boundary regressions and
security-focused tests. `e2e.sh` owns headless browser regression.
`smoke.sh` owns a fresh, isolated live server lifecycle and restart behavior.
`dogfood.sh` composes that lifecycle, the typed runner suite, and real-backend
Playwright as C6's self-hosting expectation. `all.sh` runs the complete release
sequence; these commands are intentionally local and are not hosted workflows.
