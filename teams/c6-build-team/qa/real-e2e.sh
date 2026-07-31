#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
port="${C6_QA_REAL_E2E_PORT:-18788}"
base_url="http://127.0.0.1:$port"
qa_dir="$(mktemp -d "${TMPDIR:-/tmp}/c6-build-team-real-e2e.XXXXXX")"
data_dir="$qa_dir/data"
log_file="$qa_dir/server.log"
bootstrap_token="qa-$(od -An -N24 -tx1 /dev/urandom | tr -d ' \n')"
server_pid=""

cleanup() {
  if [[ -n "$server_pid" ]] && kill -0 "$server_pid" 2>/dev/null; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  rm -rf "$qa_dir"
}
trap cleanup EXIT INT TERM

show_server_log() {
  sed "s/$bootstrap_token/[redacted-bootstrap-token]/g" "$log_file" >&2
}

cd "$repo_root"

if [[ ! -x web/node_modules/.bin/playwright ]]; then
  echo "c6-build-team: Playwright is missing; run npm ci --prefix web and install its Chromium browser" >&2
  exit 1
fi

npm run build --prefix web
C6_PORT="$port" \
C6_PUBLIC_BASE_URL="$base_url" \
C6_DATA_DIR="$data_dir" \
C6_BOOTSTRAP_TOKEN="$bootstrap_token" \
  cargo run -p c6-server >"$log_file" 2>&1 &
server_pid=$!

for _ in {1..120}; do
  if curl --fail --silent "$base_url/healthz" >/dev/null; then
    break
  fi
  if ! kill -0 "$server_pid" 2>/dev/null; then
    show_server_log
    exit 1
  fi
  sleep 0.25
done

if ! curl --fail --silent "$base_url/healthz" >/dev/null; then
  echo "c6-build-team: real-backend browser server did not become healthy" >&2
  show_server_log
  exit 1
fi

if rg -F "$bootstrap_token" "$log_file" >/dev/null; then
  echo "c6-build-team: environment-provided bootstrap token leaked to server logs" >&2
  exit 1
fi
[[ ! -e "$data_dir/bootstrap-token" ]]

C6_REAL_BACKEND=1 \
C6_E2E_BASE_URL="$base_url" \
C6_E2E_BOOTSTRAP_TOKEN="$bootstrap_token" \
  npm run test:e2e:real --prefix web

[[ ! -e "$data_dir/bootstrap-token" ]]
if rg -F "$bootstrap_token" "$log_file" >/dev/null; then
  echo "c6-build-team: browser claim leaked its bootstrap token to server logs" >&2
  exit 1
fi

echo "c6-build-team: real-backend Playwright gate passed"
