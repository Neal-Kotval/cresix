#!/usr/bin/env bash

set -o errexit
set -o nounset
set -o pipefail

QA_ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
# shellcheck source=qa/lib.sh
source "$QA_ROOT/lib.sh"

for qa_dependency in bash cargo git curl python3 node npm; do
  qa_require "$qa_dependency"
done

cd "$REPO_ROOT"

qa_heading "Documentation contract"
python3 "$QA_ROOT/tests/docs_contract.py"

qa_run "Rust formatting" cargo fmt --all -- --check
qa_run "Rust component regressions" cargo test --workspace --all-targets
qa_run "Rust lint policy" cargo clippy --workspace --all-targets -- -D warnings

qa_run "Acceptance process binaries" cargo build -p c6-server -p c6-runner -p c6-cli -p c6-cloud -p c6-connector

qa_heading "Real-process API regressions"
python3 "$QA_ROOT/tests/api_regression.py"

qa_heading "Real-socket runner regressions"
python3 "$QA_ROOT/tests/runner_regression.py"

qa_heading "C6-on-C6 durable dogfood journey"
python3 "$QA_ROOT/tests/dogfood.py"

qa_heading "Authenticated Git and CLI dogfood journey"
python3 "$QA_ROOT/tests/git_cli_regression.py"

qa_heading "Frontend unit regressions"
npm --prefix web test -- --run

qa_heading "Frontend production build"
npm --prefix web run build

qa_heading "Cresix Cloud component regressions"
bash teams/c6-build-team/qa/cloud.sh

if ! npm --prefix web run 2>/dev/null | grep -q 'test:e2e'; then
  printf 'required QA suite missing: web package has no test:e2e script\n' >&2
  exit 1
fi

if ! npm --prefix web run 2>/dev/null | grep -q 'test:e2e:real'; then
  printf 'required QA suite missing: web package has no test:e2e:real script\n' >&2
  exit 1
fi

qa_heading "Headless website fixture and real-backend regressions"
# These suites use separate ports and data directories. Running them together
# keeps the comprehensive local gate fast without reducing browser coverage.
npm --prefix web run test:e2e &
qa_fixture_pid=$!
C6_E2E_SKIP_BUILD=1 node web/scripts/run-real-e2e.mjs &
qa_real_pid=$!
npm --prefix cloud-web run test:e2e &
qa_cloud_pid=$!
qa_fixture_status=0
qa_real_status=0
qa_cloud_status=0
wait "$qa_fixture_pid" || qa_fixture_status=$?
wait "$qa_real_pid" || qa_real_status=$?
wait "$qa_cloud_pid" || qa_cloud_status=$?
if (( qa_fixture_status != 0 || qa_real_status != 0 || qa_cloud_status != 0 )); then
  printf 'browser QA failed (hub-fixture=%s hub-real=%s cloud=%s)\n' "$qa_fixture_status" "$qa_real_status" "$qa_cloud_status" >&2
  exit 1
fi

if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
  qa_run "Compose packaging model" docker compose config --quiet
else
  qa_heading "Compose packaging model"
  qa_note "SKIP: Docker Compose is not installed; no containers were started."
fi

qa_heading "C6 local acceptance gate passed"
qa_note "No CI service was configured or invoked."
