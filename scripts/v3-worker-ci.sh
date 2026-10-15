#!/usr/bin/env bash
# Run against the smoke journey's exact kit before its successful cleanup.
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
[[ "$#" -eq 1 && "$1" == /* && ! -e "$1" && ! -L "$1" ]] || {
  echo "v3-worker-ci: expected a fresh absolute evidence directory" >&2
  exit 1
}
evidence_parent="$(cd -- "$(dirname -- "$1")" && pwd -P)"
evidence="${evidence_parent}/$(basename -- "$1")"
case "${evidence}/" in "${repo_root}/"*)
  echo "v3-worker-ci: evidence must be outside the source checkout" >&2
  exit 1 ;;
esac
mkdir -- "${evidence}"

# The runners retain all identity, inventory, case-count and completion checks.
# Keep diagnostics even when native capture exits before it can seal its files.
started="${SECONDS}"
finish() {
  local status="$?"
  trap - EXIT
  printf 'elapsed_seconds=%s\nexit_status=%s\n' "$((SECONDS - started))" "${status}" >"${evidence}/timing.txt"
  exit "${status}"
}
trap finish EXIT
TACHIKO_V3_ACCEPTANCE_DIAGNOSTIC_ROOT="${evidence}/native" \
  bash "${repo_root}/acceptance/date-definition-save-v3/run-native-capture.sh" \
  2>&1 | tee "${evidence}/native-runner.log"

# mktemp owns this single fresh capture directory; never parse console output.
captures=("${evidence}/native"/capture.*)
[[ "${#captures[@]}" -eq 1 && -d "${captures[0]}" ]] || {
  echo "v3-worker-ci: expected exactly one native capture" >&2
  exit 1
}
TACHIKO_V3_ACCEPTANCE_CAPTURE_DIR="${captures[0]}" \
  TACHIKO_V3_ACCEPTANCE_PLAYWRIGHT_OUTPUT_DIR="${evidence}/playwright" \
  bash "${repo_root}/acceptance/date-definition-save-v3/run-worker-acceptance.sh" \
  2>&1 | tee "${evidence}/worker-runner.log"
