#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$repo_root"

# This gate proves the connected-mode contracts, each process/UI boundary, and
# a real Cloud-to-connector-to-authenticated-C6-compatible relay lifecycle on
# loopback. The isolated-origin real-C6 browser journey remains deferred.
cargo test -p c6-cloud-core
cargo test -p c6-cloud
cargo test -p c6-connector
python3 qa/tests/cloud_connected_regression.py

if [[ ! -x cloud-web/node_modules/.bin/vitest ]]; then
  echo "c6-build-team: Cloud web dependencies are missing; run npm ci --prefix cloud-web" >&2
  exit 1
fi

npm test --prefix cloud-web
npm run build --prefix cloud-web
npm run test:e2e --prefix cloud-web

echo "c6-build-team: connected Cloud component gate passed"
