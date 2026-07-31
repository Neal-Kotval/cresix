#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
port="${C6_QA_PORT:-18787}"
base_url="http://127.0.0.1:$port"
log_file="${TMPDIR:-/tmp}/c6-build-team-smoke-$$.log"
server_pid=""

cleanup() {
  if [[ -n "$server_pid" ]] && kill -0 "$server_pid" 2>/dev/null; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  rm -f "$log_file"
}
trap cleanup EXIT INT TERM

cd "$repo_root"
C6_PORT="$port" cargo run -p c6-server >"$log_file" 2>&1 &
server_pid=$!

for _ in {1..60}; do
  if curl --fail --silent "$base_url/healthz" >/dev/null; then
    break
  fi
  if ! kill -0 "$server_pid" 2>/dev/null; then
    cat "$log_file" >&2
    exit 1
  fi
  sleep 0.25
done

curl --fail --silent "$base_url/healthz" | grep -q '"status":"ok"'
curl --fail --silent "$base_url/" | grep -q '<title>Weeknote · C6</title>'
curl --fail --silent "$base_url/api/v1/projects/weeknote" | grep -q '"slug":"weeknote"'

valid_payload='{"source":"version = 1\n[[services]]\nname = \"web\"\ncommand = \"./server\"\nport = 8080\n"}'
curl --fail --silent -X POST "$base_url/api/v1/manifest/validate" \
  -H 'content-type: application/json' --data-binary "$valid_payload" | grep -q '"valid":true'

invalid_payload='{"source":"version = 1\n[[services]]\nname = \"web\"\ncommand = \"./server\"\nport = 8080\nsecrets = [\"MISSING\"]\n"}'
curl --fail --silent -X POST "$base_url/api/v1/manifest/validate" \
  -H 'content-type: application/json' --data-binary "$invalid_payload" | grep -q '"valid":false'

run_payload='{"job":"friday-notes","kind":"agent"}'
curl --fail --silent -X POST "$base_url/api/v1/projects/weeknote/runs" \
  -H 'content-type: application/json' --data-binary "$run_payload" | grep -q '"status":"queued"'

echo "c6-build-team: live smoke test passed on $base_url"

