#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

cargo run --quiet --target-dir "$repo_root/target" \
  --manifest-path examples/manifest-validator/Cargo.toml -- \
  examples/static-site/c6.toml \
  examples/scheduled-report/c6.toml \
  examples/agent-proposal/c6.toml \
  examples/team-tracker/c6.toml \
  examples/weeknote/c6.toml
