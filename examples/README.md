# C6 examples

These projects show the version 1 `c6.toml` contract for small software. They
are deliberately small enough to copy and adapt, but they are not packaged
applications: placeholder commands such as `./bin/serve-site` document what a
future C6 runner should start.

Today C6 can parse and validate these manifests and record their declared
services, jobs, resource limits, schedules, repository-write policy, and secret
names. Validation does **not** prove that a referenced executable or agent
runtime exists, and it does not execute any command in these directories.

## Catalog

| Example | Contract demonstrated |
| --- | --- |
| [`static-site`](static-site/) | One small HTTP service with a health endpoint |
| [`scheduled-report`](scheduled-report/) | A bounded recurring command with an explicit timezone |
| [`agent-proposal`](agent-proposal/) | An agent configuration recorded as metadata with proposal-only repository writes |
| [`team-tracker`](team-tracker/) | A web service declaring database, file, and secret dependencies |
| [`weeknote`](weeknote/) | A fuller composition of a service, cron sync, and proposal agent |

No example contains a credential or token value. Secret declarations are names
and human-readable descriptions only; provide values through the operator's
secret-management path when that runtime boundary is implemented.

## Validate locally

Run the repository-local validator from the repository root:

```sh
./examples/validate.sh
```

The helper passes each example through `c6_core::ProjectManifest::parse`, the
same parser used by C6. It only reads files and reports validation results.
