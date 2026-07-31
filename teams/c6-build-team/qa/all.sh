#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$repo_root"

bash teams/c6-build-team/qa/check.sh
bash teams/c6-build-team/qa/security.sh
bash teams/c6-build-team/qa/cloud.sh
bash teams/c6-build-team/qa/dogfood.sh

echo "c6-build-team: all local release gates passed"
