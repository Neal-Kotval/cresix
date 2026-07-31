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

version="$(sed -n 's/^version = \([0-9][0-9]*\)$/\1/p' "$team_file")"
[[ "$version" == "1" ]] || fail "unsupported or missing team manifest version"

name="$(awk '/^\[/{exit} /^name = /{gsub(/^name = "|"$/, ""); print}' "$team_file")"
[[ "$name" == "c6-build-team" ]] || fail "unexpected team name: $name"

agent_pairs="$(awk '
  function emit() { if (in_agent && id != "" && config != "") print id "|" config }
  /^\[\[agents\]\]/ { emit(); in_agent=1; id=""; config=""; next }
  /^\[\[/ { emit(); in_agent=0; next }
  in_agent && /^id = / { id=$0; gsub(/^id = "|"$/, "", id) }
  in_agent && /^config = / { config=$0; gsub(/^config = "|"$/, "", config) }
  END { emit() }
' "$team_file")"
[[ -n "$agent_pairs" ]] || fail "team.toml declares no agents"

ids=""
agent_count=0
while IFS='|' read -r declared_id relative_config; do
  [[ -n "$declared_id" && -n "$relative_config" ]] || continue
  agent_count=$((agent_count + 1))
  config="$team_root/$relative_config"
  [[ -f "$config" ]] || fail "missing agent config: $relative_config"
  id="$(sed -n 's/^id = "\([^"]*\)"$/\1/p' "$config")"
  [[ -n "$id" ]] || fail "$relative_config has no id"
  [[ "$id" == "$declared_id" ]] \
    || fail "$relative_config id '$id' does not match declared id '$declared_id'"
  if printf '%s\n' "$ids" | grep -Fx "$id" >/dev/null; then
    fail "duplicate agent id: $id"
  fi
  ids="${ids}${id}
"

  prompt="$(sed -n 's/^instructions = "\([^"]*\)"$/\1/p' "$config")"
  [[ -n "$prompt" ]] || fail "$relative_config has no instructions"
  prompt_path="$(cd "$(dirname "$config")" && pwd)/$prompt"
  [[ -f "$prompt_path" ]] || fail "$relative_config references missing prompt: $prompt"
  [[ -s "$prompt_path" ]] || fail "$relative_config references an empty prompt"

  grep -Eq '^name = "[^"]+"$' "$config" || fail "$relative_config has no name"
  grep -Eq '^runtime = "[^"]+"$' "$config" || fail "$relative_config has no runtime"
  grep -Eq '^role = "[^"]+"$' "$config" || fail "$relative_config has no role"
  grep -Eq '^output = "[^"]+"$' "$config" || fail "$relative_config has no output"
  grep -Eq '^may_edit = (true|false)$' "$config" || fail "$relative_config has invalid may_edit"
  grep -Eq '^scope = \[.+\]$' "$config" || fail "$relative_config has no scope"
done <<< "$agent_pairs"

orchestrator="$(sed -n 's/^orchestrator = "\([^"]*\)"$/\1/p' "$team_file")"
printf '%s\n' "$ids" | grep -Fx "$orchestrator" >/dev/null \
  || fail "orchestrator is not a declared agent"

max_parallel="$(sed -n 's/^max_parallel = \([0-9][0-9]*\)$/\1/p' "$team_file")"
[[ -n "$max_parallel" ]] || fail "max_parallel is missing"
(( max_parallel >= 1 && max_parallel <= agent_count )) \
  || fail "max_parallel must be between 1 and the declared agent count"

gate_names=""
while IFS= read -r command; do
  script="${command#bash }"
  [[ "$script" == teams/c6-build-team/qa/*.sh ]] \
    || fail "gate command escapes the team QA directory: $command"
  [[ -f "$repo_root/$script" ]] || fail "gate references missing script: $script"
  bash -n "$repo_root/$script" || fail "gate has invalid Bash syntax: $script"
done < <(sed -n 's/^command = "\(bash [^"]*\)"$/\1/p' "$team_file")

while IFS= read -r gate_name; do
  [[ -n "$gate_name" ]] || continue
  if printf '%s\n' "$gate_names" | grep -Fx "$gate_name" >/dev/null; then
    fail "duplicate gate name: $gate_name"
  fi
  gate_names="${gate_names}${gate_name}
"
done < <(awk '/^\[\[gates\]\]/{in_gate=1; next} /^\[/{in_gate=0} in_gate && /^name = /{gsub(/^name = "|"$/, ""); print}' "$team_file")

gate_count="$(printf '%s\n' "$gate_names" | sed '/^$/d' | wc -l | tr -d ' ')"
[[ "$gate_count" -gt 0 ]] || fail "team.toml declares no gates"

echo "c6-build-team: $agent_count agents and $gate_count gates are valid"
