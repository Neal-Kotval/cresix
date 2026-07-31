#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$repo_root"

bash teams/c6-build-team/qa/validate-team.sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

if [[ ! -x web/node_modules/.bin/vitest ]]; then
  echo "c6-build-team: web dependencies are missing; run npm ci --prefix web" >&2
  exit 1
fi

npm test --prefix web
npm run build --prefix web
docker compose config --quiet

if git ls-files | grep -E '(^|/)(\.env|id_rsa|id_ed25519|auth\.json)$' >/dev/null; then
  echo "c6-build-team: tracked credential-shaped file detected" >&2
  exit 1
fi

if git grep -En '(gh[opsu]_[A-Za-z0-9]{20,}|sk-[A-Za-z0-9_-]{20,})' -- ':!teams/c6-build-team/qa/check.sh'; then
  echo "c6-build-team: possible committed credential detected" >&2
  exit 1
fi

if grep -n '/var/run/docker.sock' compose.yaml >/dev/null; then
  echo "c6-build-team: the control-plane topology must not mount docker.sock" >&2
  exit 1
fi

echo "c6-build-team: repository gate passed"

