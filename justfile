set dotenv-load

export C6_DATA_DIR := env_var_or_default("C6_DATA_DIR", ".c6")

default:
    @just --list

# Install browser dependencies.
setup:
    npm ci --prefix web

# Build the browser application served by C6.
build-web:
    npm run build --prefix web

# Start the complete local preview and reveal a new first-owner token locally.
start: build-web
    @just _serve false

# Start the complete preview with authenticated read-only Git enabled.
start-git: build-web
    @just _serve true

[private]
_serve git_http:
    #!/usr/bin/env bash
    set -euo pipefail
    token_file="$C6_DATA_DIR/bootstrap-token"
    local_url="http://127.0.0.1:${C6_PORT:-8787}"
    C6_GIT_HTTP_ENABLED="{{ git_http }}" cargo run -p c6-server &
    server_pid=$!
    cleanup() {
      kill "$server_pid" 2>/dev/null || true
    }
    trap cleanup EXIT INT TERM

    for _ in $(seq 1 150); do
      if [[ -f "$token_file" ]]; then
        printf '\n\033[1;36mC6 first-owner token\033[0m (local terminal only)\n'
        cat "$token_file"
        printf '\nPaste it into the claim screen. C6 deletes this file after claim.\n\n'
        break
      fi
      if command -v curl >/dev/null 2>&1 && status="$(curl --fail --silent "$local_url/api/v1/status" 2>/dev/null)"; then
        if [[ "$status" == *'"claimed":true'* ]]; then
          printf '\nC6 is ready at %s. This installation is already claimed.\n\n' "$local_url"
        else
          printf '\nC6 is ready at %s. The bootstrap token was supplied externally, so Just cannot reveal it.\n\n' "$local_url"
        fi
        break
      fi
      if ! kill -0 "$server_pid" 2>/dev/null; then
        break
      fi
      sleep 0.1
    done

    wait "$server_pid"

# Explicitly reveal the unclaimed local first-owner token.
bootstrap-token:
    #!/usr/bin/env bash
    set -euo pipefail
    token_file="$C6_DATA_DIR/bootstrap-token"
    if [[ ! -f "$token_file" ]]; then
      printf 'No bootstrap token file at %s. Start C6 first, or this installation is already claimed.\n' "$token_file" >&2
      exit 1
    fi
    cat "$token_file"

# Build and start the Compose topology in the background.
up:
    docker compose up --build --detach
    @printf 'C6 is starting at http://127.0.0.1:8787\nRun `just bootstrap-token-compose` to reveal a new first-owner token.\n'

# Explicitly reveal the unclaimed token inside the Compose service.
bootstrap-token-compose:
    @docker compose exec -T c6 sh -c 'test -f /var/lib/c6/bootstrap-token || { echo "No bootstrap token file. Start C6 first, or this installation is already claimed." >&2; exit 1; }; cat /var/lib/c6/bootstrap-token'

# Follow control-plane logs without printing bootstrap credentials.
logs:
    docker compose logs --follow c6

# Stop the Compose topology without deleting its data volumes.
down:
    docker compose down

# Run the complete local acceptance gate.
test:
    ./qa/run.sh
