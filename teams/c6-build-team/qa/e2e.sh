#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$repo_root"

if [[ ! -x web/node_modules/.bin/playwright ]]; then
  echo "c6-build-team: Playwright is missing; run npm ci --prefix web and install its Chromium browser" >&2
  exit 1
fi

if ! node -e 'const p=require("./web/package.json"); process.exit(p.scripts?.["test:e2e"] ? 0 : 1)'; then
  echo "c6-build-team: web/package.json must define the test:e2e local gate" >&2
  exit 1
fi

npm run test:e2e --prefix web

echo "c6-build-team: headless browser gate passed"
