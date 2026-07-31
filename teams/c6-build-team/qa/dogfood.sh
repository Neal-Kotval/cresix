#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$repo_root"

# A release must create and restart a fresh C6 installation through HTTP,
# validate the typed runner boundary, then drive a real C6 backend in a browser.
# The lifecycle smoke deliberately names its project `cresix`.
bash teams/c6-build-team/qa/smoke.sh
cargo test -p c6-runner
bash teams/c6-build-team/qa/e2e.sh
bash teams/c6-build-team/qa/real-e2e.sh

echo "c6-build-team: self-hosted dogfood gate passed"
