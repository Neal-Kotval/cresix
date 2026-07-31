#!/usr/bin/env bash

set -o errexit
set -o nounset
set -o pipefail

QA_ROOT=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(CDPATH='' cd -- "$QA_ROOT/.." && pwd)
export QA_ROOT REPO_ROOT

qa_heading() {
  printf '\n==> %s\n' "$1"
}

qa_note() {
  printf '    %s\n' "$1"
}

qa_require() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf 'required QA dependency is missing: %s\n' "$1" >&2
    return 1
  fi
}

qa_run() {
  qa_heading "$1"
  shift
  "$@"
}
