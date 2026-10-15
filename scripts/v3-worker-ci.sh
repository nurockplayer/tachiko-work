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

node --input-type=module - "${repo_root}" <<'NODE'
import {createHash} from 'node:crypto';
import {readFileSync} from 'node:fs';
import {join} from 'node:path';
const root = process.argv[2];
const manifest = 'acceptance/date-definition-save-v3/SEED.SHA256SUMS';
const entries = readFileSync(join(root, manifest), 'utf8').trimEnd().split('\n');
let failed = false;
for (const [index, line] of entries.entries()) {
  const entry = /^([a-f0-9]{64}) [ *](.+)$/.exec(line);
  if (!entry) {
    console.error(`v3-worker-ci: invalid seed manifest entry: ${manifest}:${index + 1}`);
    failed = true;
    continue;
  }
  const [, expected, path] = entry;
  try {
    const actual = createHash('sha256').update(readFileSync(join(root, path))).digest('hex');
    if (actual !== expected) {
      console.error(`v3-worker-ci: seed checksum mismatch: ${path}`);
      failed = true;
    }
  } catch (error) {
    console.error(`v3-worker-ci: cannot verify seed path: ${path}: ${error.message}`);
    failed = true;
  }
}
if (failed) process.exit(1);
NODE

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
