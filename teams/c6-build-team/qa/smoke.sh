#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
port="${C6_QA_PORT:-18787}"
base_url="http://127.0.0.1:$port"
qa_dir="$(mktemp -d "${TMPDIR:-/tmp}/c6-build-team-smoke.XXXXXX")"
data_dir="$qa_dir/data"
cookie_jar="$qa_dir/cookies"
headers_file="$qa_dir/headers"
body_file="$qa_dir/body"
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

start_server() {
  C6_PORT="$port" \
  C6_PUBLIC_BASE_URL="$base_url" \
  C6_DATA_DIR="$data_dir" \
  C6_BOOTSTRAP_TOKEN="$bootstrap_token" \
    cargo run -p c6-server >>"$log_file" 2>&1 &
  server_pid=$!

  for _ in {1..120}; do
    if curl --fail --silent "$base_url/healthz" >/dev/null; then
      return
    fi
    if ! kill -0 "$server_pid" 2>/dev/null; then
      show_server_log
      exit 1
    fi
    sleep 0.25
  done
  echo "c6-build-team: server did not become healthy" >&2
  show_server_log
  exit 1
}

stop_server() {
  if [[ -n "$server_pid" ]] && kill -0 "$server_pid" 2>/dev/null; then
    kill "$server_pid"
    wait "$server_pid" 2>/dev/null || true
  fi
  server_pid=""
}

status_code() {
  curl --silent --output "$body_file" --write-out '%{http_code}' "$@"
}

cd "$repo_root"
if [[ ! -f web/dist/index.html ]]; then
  if [[ ! -x web/node_modules/.bin/vite ]]; then
    echo "c6-build-team: web dependencies are missing; run npm ci --prefix web" >&2
    exit 1
  fi
  npm run build --prefix web
fi
start_server

if rg -F "$bootstrap_token" "$log_file" >/dev/null; then
  echo "c6-build-team: environment-provided bootstrap token leaked to server logs" >&2
  exit 1
fi
[[ ! -e "$data_dir/bootstrap-token" ]]

curl --fail --silent "$base_url/healthz" | grep -q '"status":"ok"'
curl --fail --silent "$base_url/api/v1/status" | grep -q '"claimed":false'
curl --fail --silent "$base_url/" | grep -q 'id="root"'

[[ "$(status_code "$base_url/api/v1/projects")" == "401" ]]

claim_payload="$(printf '{"token":"%s","displayName":"QA Owner","deviceLabel":"QA Browser","publicKey":"qa-public-key-abcdefghijklmnopqrstuvwxyz"}' "$bootstrap_token")"
claim_code="$(printf '%s' "$claim_payload" | curl --silent \
  --dump-header "$headers_file" --output "$body_file" --write-out '%{http_code}' \
  --cookie-jar "$cookie_jar" -X POST "$base_url/api/v1/bootstrap/claim" \
  -H "origin: $base_url" -H 'content-type: application/json' --data-binary @-)"
[[ "$claim_code" == "201" ]]
[[ ! -e "$data_dir/bootstrap-token" ]]
if rg -F "$bootstrap_token" "$log_file" >/dev/null; then
  echo "c6-build-team: bootstrap claim leaked its token to server logs" >&2
  exit 1
fi
grep -Eiq '^set-cookie:.*HttpOnly' "$headers_file"
grep -Eiq '^set-cookie:.*SameSite=Strict' "$headers_file"
csrf_token="$(sed -n 's/.*"csrfToken":"\([^"]*\)".*/\1/p' "$body_file")"
[[ -n "$csrf_token" ]]

curl --fail --silent --cookie "$cookie_jar" "$base_url/api/v1/session" | grep -q '"displayName":"QA Owner"'

replay_code="$(printf '%s' "$claim_payload" | curl --silent --output "$body_file" --write-out '%{http_code}' \
  -X POST "$base_url/api/v1/bootstrap/claim" -H "origin: $base_url" \
  -H 'content-type: application/json' --data-binary @-)"
[[ "$replay_code" == "409" ]]

workspace_payload='{"slug":"qa-team","name":"QA Team"}'
missing_csrf_code="$(printf '%s' "$workspace_payload" | curl --silent --output "$body_file" --write-out '%{http_code}' \
  --cookie "$cookie_jar" -X POST "$base_url/api/v1/workspaces" \
  -H "origin: $base_url" -H 'content-type: application/json' --data-binary @-)"
[[ "$missing_csrf_code" == "403" ]]

wrong_origin_code="$(printf '%s' "$workspace_payload" | curl --silent --output "$body_file" --write-out '%{http_code}' \
  --cookie "$cookie_jar" -X POST "$base_url/api/v1/workspaces" \
  -H 'origin: https://evil.example' -H "x-c6-csrf: $csrf_token" \
  -H 'content-type: application/json' --data-binary @-)"
[[ "$wrong_origin_code" == "403" ]]

workspace_code="$(printf '%s' "$workspace_payload" | curl --silent --output "$body_file" --write-out '%{http_code}' \
  --cookie "$cookie_jar" -X POST "$base_url/api/v1/workspaces" \
  -H "origin: $base_url" -H "x-c6-csrf: $csrf_token" \
  -H 'content-type: application/json' --data-binary @-)"
[[ "$workspace_code" == "201" ]]
workspace_id="$(sed -n 's/.*"id":"\([^"]*\)".*/\1/p' "$body_file")"
[[ -n "$workspace_id" ]]

project_payload="$(printf '{"workspaceId":"%s","slug":"cresix","name":"Cresix","description":"C6 dogfooding itself"}' "$workspace_id")"
project_code="$(printf '%s' "$project_payload" | curl --silent --output "$body_file" --write-out '%{http_code}' \
  --cookie "$cookie_jar" -X POST "$base_url/api/v1/projects" \
  -H "origin: $base_url" -H "x-c6-csrf: $csrf_token" \
  -H 'content-type: application/json' --data-binary @-)"
[[ "$project_code" == "201" ]]
project_id="$(sed -n 's/.*"id":"\([^"]*\)".*/\1/p' "$body_file")"
[[ -n "$project_id" ]]

curl --fail --silent --cookie "$cookie_jar" \
  "$base_url/api/v1/projects/$project_id/repository/branches" | grep -q '"name":"main"'
curl --fail --silent --cookie "$cookie_jar" \
  "$base_url/api/v1/projects/$project_id/repository/commits?revision=main&limit=10" | grep -q '"commits":\['
curl --fail --silent --cookie "$cookie_jar" \
  "$base_url/api/v1/projects/$project_id/repository/tree?revision=main&recursive=true" | grep -q '"path":"c6.toml"'

run_code="$(printf '%s' '{"job":"dogfood","kind":"command","revisionSha":"HEAD"}' | curl --silent \
  --output "$body_file" --write-out '%{http_code}' --cookie "$cookie_jar" \
  -X POST "$base_url/api/v1/projects/$project_id/runs" \
  -H "origin: $base_url" -H "x-c6-csrf: $csrf_token" \
  -H 'content-type: application/json' --data-binary @-)"
[[ "$run_code" == "202" ]]
grep -q '"status":"queued"' "$body_file"

secret_code="$(printf '%s' '{"value":"must-not-be-accepted"}' | curl --silent --output "$body_file" --write-out '%{http_code}' \
  --cookie "$cookie_jar" -X PUT "$base_url/api/v1/projects/$project_id/secrets/API_KEY/value" \
  -H "origin: $base_url" -H "x-c6-csrf: $csrf_token" \
  -H 'content-type: application/json' --data-binary @-)"
[[ "$secret_code" == "501" ]]

stop_server
start_server

curl --fail --silent "$base_url/api/v1/status" | grep -q '"claimed":true'
curl --fail --silent --cookie "$cookie_jar" "$base_url/api/v1/projects" | grep -q '"slug":"cresix"'
curl --fail --silent --cookie "$cookie_jar" "$base_url/api/v1/projects/$project_id/runs" | grep -q '"job":"dogfood"'
restart_replay_code="$(printf '%s' "$claim_payload" | curl --silent --output "$body_file" --write-out '%{http_code}' \
  -X POST "$base_url/api/v1/bootstrap/claim" -H "origin: $base_url" \
  -H 'content-type: application/json' --data-binary @-)"
[[ "$restart_replay_code" == "409" ]]
if rg -F "$bootstrap_token" "$log_file" >/dev/null; then
  echo "c6-build-team: restart leaked the configured bootstrap token" >&2
  exit 1
fi

echo "c6-build-team: live bootstrap, authorization, and restart regression passed on $base_url"
