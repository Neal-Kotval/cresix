#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$repo_root"

bash teams/c6-build-team/qa/validate-team.sh

if rg -n 'CorsLayer::permissive|allow_origin\(Any\)|Access-Control-Allow-Origin[^\n]*\*' \
  --glob '*.rs' --glob '*.ts' --glob '*.tsx' \
  --glob '!target/**' --glob '!web/node_modules/**' --glob '!web/dist/**' .; then
  echo "c6-build-team: permissive cross-origin policy detected" >&2
  exit 1
fi

if rg -n '/var/run/docker\.sock|--privileged|network_mode:[[:space:]]*host' \
  --glob 'compose*.yml' --glob 'compose*.yaml' --glob 'Dockerfile*' \
  --glob 'config/**' --glob 'crates/**' .; then
  echo "c6-build-team: forbidden control-plane host privilege detected" >&2
  exit 1
fi

if git ls-files | grep -E '(^|/)(\.env|id_rsa|id_ed25519|auth\.json|cookies?\.txt|.*\.(sqlite|sqlite3|db))$' >/dev/null; then
  echo "c6-build-team: tracked credential, session, or live database file detected" >&2
  exit 1
fi

if rg -n '(gh[opsu]_[A-Za-z0-9]{20,}|sk-[A-Za-z0-9_-]{20,}|-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----)' \
  --glob '!target/**' --glob '!web/node_modules/**' --glob '!web/dist/**' \
  --glob '!teams/c6-build-team/qa/check.sh' \
  --glob '!teams/c6-build-team/qa/security.sh' .; then
  echo "c6-build-team: possible committed credential detected" >&2
  exit 1
fi

# These crate suites are intentionally repeated outside the aggregate gate: they
# are the executable regression evidence for each C6 trust boundary.
cargo test -p c6-core
cargo test -p c6-git
cargo test -p c6-server
cargo test -p c6-runner
cargo test -p c6-cloud-core
cargo test -p c6-cloud
cargo test -p c6-connector

echo "c6-build-team: security regression gate passed"
