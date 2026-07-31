# Example projects

The [`examples/`](../examples/) catalog demonstrates version 1 manifest shapes.
Every example is intentionally non-executing: commands may be placeholders,
secret entries are names only, and successful validation proves schema/policy
consistency—not deployability or sandbox enforcement.

| Example | Use it to study | Not provided |
| --- | --- | --- |
| [Static site](../examples/static-site/) | one service, port, health path, small resources | server binary or hosting |
| [Scheduled report](../examples/scheduled-report/) | five-field cron, IANA timezone, timeout, `forbid` | scheduler daemon or report command |
| [Agent proposal](../examples/agent-proposal/) | agent-config path, no egress, proposal-only write intent | Codex, credentials, parsing/execution of agent config |
| [Team tracker](../examples/team-tracker/) | service plus cron, database/files/secret declarations | provisioning, secret value, executables |
| [Weeknote](../examples/weeknote/) | composed service, sync job, scheduled agent, egress intent | application, OpenAI call, runtime isolation |
| [Connected Cloud](../examples/connected-cloud/) | owner-only connector config and separate credential files | account, credentials, DNS, or production relay |

Validate all examples with the same parser used by the server:

```bash
./examples/validate.sh
```

The validator reads each `c6.toml`, calls `ProjectManifest::parse`, and returns
nonzero on an invalid contract. It does not inspect command existence, access
the network, read credentials, or submit anything to the runner.

## Adapting an example

Copy the smallest relevant manifest into a real Git repository, replace
placeholder commands with repository-relative paths, retain explicit resource
budgets/timezones, and commit dependency lockfiles. Never add secret values.
The current C6 UI/API can validate and record the declaration; running it is a
future capability.

For agent automation, treat example manifests as reviewable declarations, not
permissions or runnable prompts. An agent may use current CLI `--json` output to
discover the project, but C6 does not yet poll events, dispatch the declared
job, resolve a secret, start Docker, or create a proposal branch. The intended
runtime flow is documented separately in the
[agent-first runtime specification](specs/AGENT_FIRST_RUNTIME.md).

The connected example is not a project manifest and is therefore not processed
by `examples/validate.sh`. Its placeholder UUIDs and paths must be replaced only
after an authenticated Cloud installation/binding flow supplies real values.
