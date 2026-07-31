#!/usr/bin/env bash
set -euo pipefail

team_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo_root="$(cd "$team_root/../.." && pwd)"
team_file="$team_root/team.toml"

fail() {
  echo "c6-build-team: $*" >&2
  exit 1
}

[[ -f "$team_file" ]] || fail "team.toml is missing"

configs="$(sed -n 's/^config = "\([^"]*\)"$/\1/p' "$team_file")"
[[ -n "$configs" ]] || fail "team.toml declares no agents"

ids=""
agent_count=0
while IFS= read -r relative_config; do
  [[ -n "$relative_config" ]] || continue
  agent_count=$((agent_count + 1))
  config="$team_root/$relative_config"
  [[ -f "$config" ]] || fail "missing agent config: $relative_config"
  id="$(sed -n 's/^id = "\([^"]*\)"$/\1/p' "$config")"
  [[ -n "$id" ]] || fail "$relative_config has no id"
  if printf '%s\n' "$ids" | grep -Fx "$id" >/dev/null; then
    fail "duplicate agent id: $id"
  fi
  ids="${ids}${id}
"

  prompt="$(sed -n 's/^instructions = "\([^"]*\)"$/\1/p' "$config")"
  [[ -n "$prompt" ]] || fail "$relative_config has no instructions"
  prompt_path="$(cd "$(dirname "$config")" && pwd)/$prompt"
  [[ -f "$prompt_path" ]] || fail "$relative_config references missing prompt: $prompt"
done <<< "$configs"

orchestrator="$(sed -n 's/^orchestrator = "\([^"]*\)"$/\1/p' "$team_file")"
printf '%s\n' "$ids" | grep -Fx "$orchestrator" >/dev/null \
  || fail "orchestrator is not a declared agent"

while IFS= read -r command; do
  script="${command#bash }"
  [[ -f "$repo_root/$script" ]] || fail "gate references missing script: $script"
done < <(sed -n 's/^command = "\(bash [^"]*\)"$/\1/p' "$team_file")

echo "c6-build-team: $agent_count agents and all gates are valid"
