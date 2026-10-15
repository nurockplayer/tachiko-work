#!/usr/bin/env bash
# Bounded orchestration and actual-runner tests. Cargo and pnpm are always mocks.
set -euo pipefail
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
scratch="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-v3-ci-test.XXXXXX")"
scratch="$(cd -- "${scratch}" && pwd -P)"
trap 'rm -rf -- "${scratch}"' EXIT
unset TACHIKO_V3_CI_EVIDENCE_ROOT TACHIKO_V3_ACCEPTANCE_DIAGNOSTIC_ROOT TACHIKO_V3_ACCEPTANCE_PLAYWRIGHT_OUTPUT_DIR

fail() { echo "v3-worker-ci-test: $*" >&2; exit 1; }
cases=0
expect_status() {
  local expected="$1"
  shift
  local status=0
  "$@" >"${scratch}/last.log" 2>&1 || status=$?
  [[ "${status}" -eq "${expected}" ]] || {
    cat "${scratch}/last.log" >&2
    fail "expected exit ${expected}, got ${status}: $*"
  }
  cases=$((cases + 1))
}

fixture="${scratch}/source"
mkdir -p "${fixture}/scripts" "${fixture}/acceptance/date-definition-save-v3" "${fixture}/bin"
cp "${repo_root}/scripts/v3-worker-ci.sh" "${fixture}/scripts/"
cat >"${fixture}/acceptance/date-definition-save-v3/run-native-capture.sh" <<'NATIVE'
#!/usr/bin/env bash
set -euo pipefail
root="${TACHIKO_V3_ACCEPTANCE_DIAGNOSTIC_ROOT}"
mkdir "${root}"
if [[ "${MOCK_CAPTURE_COUNT:-1}" != 0 ]]; then
  mkdir "${root}/capture.mock"
  printf 'partial capture\n' >"${root}/capture.mock/partial.twd"
fi
if [[ "${MOCK_CAPTURE_COUNT:-1}" == 2 ]]; then mkdir "${root}/capture.other"; fi
echo native >>"${MOCK_EVENTS}"
echo 'native diagnostic'
exit "${MOCK_NATIVE_STATUS:-0}"
NATIVE
cat >"${fixture}/acceptance/date-definition-save-v3/run-worker-acceptance.sh" <<'WORKER'
#!/usr/bin/env bash
set -euo pipefail
[[ -f "${TACHIKO_V3_ACCEPTANCE_CAPTURE_DIR}/partial.twd" ]]
[[ "${TACHIKO_V3_ACCEPTANCE_PLAYWRIGHT_OUTPUT_DIR}" == "${TACHIKO_V3_ACCEPTANCE_CAPTURE_DIR%/native/*}/playwright" ]]
mkdir "${TACHIKO_V3_ACCEPTANCE_PLAYWRIGHT_OUTPUT_DIR}"
echo worker >>"${MOCK_EVENTS}"
echo 'worker diagnostic'
exit "${MOCK_WORKER_STATUS:-0}"
WORKER
export MOCK_EVENTS="${scratch}/events"
orchestrator="${fixture}/scripts/v3-worker-ci.sh"
expect_status 0 "${BASH}" "${orchestrator}" "${scratch}/success"
[[ "$(<"${MOCK_EVENTS}")" == $'native\nworker' ]] || fail 'native/Worker order changed'
[[ -f "${scratch}/success/timing.txt" ]] || fail 'missing timing receipt'
for status in 1 77; do
  : >"${MOCK_EVENTS}"
  expect_status "${status}" env MOCK_NATIVE_STATUS="${status}" "${BASH}" "${orchestrator}" "${scratch}/native-${status}"
  [[ "$(<"${MOCK_EVENTS}")" == native ]] || fail 'Worker ran after native failure'
  [[ -f "${scratch}/native-${status}/native/capture.mock/partial.twd" ]] || fail 'lost partial capture'
  rg -q "exit_status=${status}" "${scratch}/native-${status}/timing.txt" || fail 'lost failure status'
done
for status in 1 77; do
  expect_status "${status}" env MOCK_WORKER_STATUS="${status}" "${BASH}" "${orchestrator}" "${scratch}/worker-${status}"
  rg -q 'worker diagnostic' "${scratch}/worker-${status}/worker-runner.log" || fail 'lost Worker log'
done
for count in 0 2; do
  : >"${MOCK_EVENTS}"
  expect_status 1 env MOCK_CAPTURE_COUNT="${count}" "${BASH}" "${orchestrator}" "${scratch}/count-${count}"
  [[ "$(<"${MOCK_EVENTS}")" == native ]] || fail 'Worker ran with ambiguous capture'
done
expect_status 1 "${BASH}" "${orchestrator}" "${scratch}/success"
expect_status 1 "${BASH}" "${orchestrator}" relative
expect_status 1 "${BASH}" "${orchestrator}" "${fixture}/evidence"
ln -s "${fixture}" "${scratch}/source-alias"
expect_status 1 "${BASH}" "${orchestrator}" "${scratch}/source-alias/evidence"

# Exercise the real shell runners with fresh, clean, committed fixture source.
cp "${repo_root}/acceptance/date-definition-save-v3/"run-{native-capture,worker-acceptance}.sh "${fixture}/acceptance/date-definition-save-v3/"
mkdir -p "${fixture}/packages/browser-client/runtime/"{src,tests} "${fixture}/packages/browser-client/e2e" \
  "${fixture}/acceptance/date-definition-save-v3/fixtures" "${fixture}/examples/experimental-designer-client/vendor/tachiko"
printf 'export_project_v3\n' >"${fixture}/packages/browser-client/runtime/src/lib.rs"
touch "${fixture}/packages/browser-client/runtime/tests/v3_project_acceptance.rs" \
  "${fixture}/packages/browser-client/e2e/v3-project-acceptance.spec.ts" \
  "${fixture}/acceptance/date-definition-save-v3/SEED.md" "${fixture}/acceptance/date-definition-save-v3/fixtures/mixed.csv"
printf '/examples/experimental-designer-client/vendor/\n' >"${fixture}/.gitignore"
cat >"${fixture}/bin/cargo" <<'CARGO'
#!/usr/bin/env bash
set -euo pipefail
echo cargo >>"${MOCK_EVENTS}"
printf 'partial native output\n' >"${TACHIKO_V3_ACCEPTANCE_CAPTURE_DIR}/partial.twd"
echo 'mock native failure'
exit 1
CARGO
cat >"${fixture}/bin/pnpm" <<'PNPM'
#!/usr/bin/env bash
set -euo pipefail
echo "$*" >>"${MOCK_EVENTS}"
[[ "$*" != *'exec tsc '* ]] || exit "${MOCK_TSC_STATUS:-0}"
output=''
while (($#)); do
  if [[ "$1" == --output ]]; then output="$2"; shift 2; else shift; fi
done
[[ -z "${MOCK_EXPECT_OUTPUT:-}" || "${output}" == "${MOCK_EXPECT_OUTPUT}" ]]
if [[ -n "${output}" ]]; then printf 'trace\n' >"${output}/trace.zip"; fi
echo 'actual Worker imports, explicitly exports and reopens every v3 profile'
echo 'Worker preserves typed failures and resets origin after replacement or close'
echo "${MOCK_WORKER_SUMMARY:-2 passed}"
exit "${MOCK_PLAYWRIGHT_STATUS:-0}"
PNPM
chmod +x "${fixture}/bin/"{cargo,pnpm}
git -C "${fixture}" init -q
git -C "${fixture}" add --all
git -C "${fixture}" -c user.name=Fixture -c user.email=fixture@example.invalid commit -qm fixture
export PATH="${fixture}/bin:${PATH}"
native="${fixture}/acceptance/date-definition-save-v3/run-native-capture.sh"
worker="${fixture}/acceptance/date-definition-save-v3/run-worker-acceptance.sh"
expect_status 1 env TACHIKO_V3_ACCEPTANCE_DIAGNOSTIC_ROOT="${scratch}/native-real" "${BASH}" "${native}"
captures=("${scratch}/native-real"/capture.*)
[[ -f "${captures[0]}/partial.twd" && -f "${captures[0]}/capture-lease.json" && ! -e "${captures[0]}/native-result.json" ]] || fail 'partial native capture was lost or sealed'
rg -q 'mock native failure' "${scratch}/native-real/native.log" || fail 'native diagnostic missing'
for output in "${scratch}/native-real" "${fixture}/diagnostics" "${scratch}/source-alias/diagnostics" relative; do
  : >"${MOCK_EVENTS}"
  expect_status 77 env TACHIKO_V3_ACCEPTANCE_DIAGNOSTIC_ROOT="${output}" "${BASH}" "${native}"
  [[ ! -s "${MOCK_EVENTS}" ]] || fail 'Cargo ran before path refusal'
done

kit="${fixture}/examples/experimental-designer-client/vendor/tachiko"
head="$(git -C "${fixture}" rev-parse HEAD)"
export MOCK_HEAD="${head}" MOCK_KIT="${kit}"
make_capture() {
  export TACHIKO_V3_ACCEPTANCE_CAPTURE_DIR="${scratch}/capture-${cases}"
  mkdir "${TACHIKO_V3_ACCEPTANCE_CAPTURE_DIR}"
  node --input-type=module <<'NODE'
import {writeFileSync} from 'node:fs';
import {join} from 'node:path';
const dir = process.env.TACHIKO_V3_ACCEPTANCE_CAPTURE_DIR;
const identity = {candidate_head: process.env.MOCK_HEAD, run_id: `mock-${process.pid}`};
writeFileSync(join(dir, 'capture-lease.json'), JSON.stringify({...identity, state: 'native_complete'}));
writeFileSync(join(dir, 'native-result.json'), JSON.stringify({...identity, status: 'native_cases_passed', required_case_count: 16, required_cases: Array(16).fill('mock')}));
writeFileSync(join(process.env.MOCK_KIT, 'artifact-manifest.json'), JSON.stringify({sourceCommit: identity.candidate_head, files: []}));
NODE
}
make_capture
expect_status 0 env TACHIKO_V3_ACCEPTANCE_PLAYWRIGHT_OUTPUT_DIR="${scratch}/pw-success" MOCK_EXPECT_OUTPUT="${scratch}/pw-success" "${BASH}" "${worker}"
[[ -f "${scratch}/pw-success/trace.zip" && -f "${TACHIKO_V3_ACCEPTANCE_CAPTURE_DIR}/worker-completion.json" ]] || fail 'Worker completion/output missing'
[[ ! -e "${TACHIKO_V3_ACCEPTANCE_CAPTURE_DIR}/trace.zip" ]] || fail 'Playwright polluted capture inventory'
make_capture
: >"${MOCK_EVENTS}"
expect_status 0 "${BASH}" "${worker}"
if rg -q -- '--output' "${MOCK_EVENTS}"; then fail 'default Worker output arguments changed'; fi
for mode in tsc playwright skipped missing; do
  make_capture
  case "${mode}" in
    tsc) expect_status 77 env MOCK_TSC_STATUS=2 "${BASH}" "${worker}" ;;
    playwright) expect_status 1 env MOCK_PLAYWRIGHT_STATUS=1 "${BASH}" "${worker}" ;;
    skipped) expect_status 1 env MOCK_WORKER_SUMMARY='2 passed 1 skipped' "${BASH}" "${worker}" ;;
    missing) expect_status 1 env MOCK_WORKER_SUMMARY='1 passed' "${BASH}" "${worker}" ;;
  esac
  [[ ! -e "${TACHIKO_V3_ACCEPTANCE_CAPTURE_DIR}/worker-completion.json" ]] || fail 'failed Worker sealed capture'
done
make_capture
ln -s "${TACHIKO_V3_ACCEPTANCE_CAPTURE_DIR}" "${scratch}/capture-alias"
for output in "${fixture}/results" "${scratch}/source-alias/results" "${TACHIKO_V3_ACCEPTANCE_CAPTURE_DIR}/results" "${scratch}/capture-alias/results" "${scratch}/pw-success" relative; do
  : >"${MOCK_EVENTS}"
  expect_status 77 env TACHIKO_V3_ACCEPTANCE_PLAYWRIGHT_OUTPUT_DIR="${output}" "${BASH}" "${worker}"
  [[ ! -s "${MOCK_EVENTS}" ]] || fail 'pnpm ran before path refusal'
done
node --input-type=module - "${MOCK_KIT}/artifact-manifest.json" <<'NODE'
import {writeFileSync} from 'node:fs';
writeFileSync(process.argv[2], JSON.stringify({sourceCommit: '0'.repeat(40), files: []}));
NODE
expect_status 1 "${BASH}" "${worker}"
rg -q 'NOTRUN_STALE_KIT_SOURCE' "${scratch}/last.log" || fail 'stale kit assertion bypassed'

# Bounded source checks bind the opt-in to hosted CI and to the existing kit.
node --input-type=module - "${repo_root}" <<'NODE'
import assert from 'node:assert/strict';
import {readFileSync, readdirSync, unlinkSync} from 'node:fs';
import {join} from 'node:path';
const root = process.argv[2];
const read = file => readFileSync(join(root, file), 'utf8');
const ci = read('.github/workflows/ci.yml');
assert.match(ci, /TACHIKO_V3_CI_EVIDENCE_ROOT="\$\{RUNNER_TEMP\}\/date-definition-save-v3" \\\n\s+bash scripts\/release-check.sh/);
assert.match(ci, /Preserve v3 native and Worker acceptance diagnostics\n\s+if: \$\{\{ always\(\) \}\}[\s\S]*?path: \$\{\{ runner.temp \}\}\/date-definition-save-v3\//);
assert.match(read('scripts/release-check.sh'), /bash scripts\/browser-client-check.sh/);
assert.match(read('scripts/browser-client-check.sh'), /bash "\$\{repo_root\}\/scripts\/v3-worker-ci-test.sh"/);
const smoke = read('scripts/experimental-designer-client-smoke.sh');
const hook = smoke.indexOf('bash "${repo_root}/scripts/v3-worker-ci.sh"');
assert.ok(hook > smoke.indexOf('bash "${acceptance_runner}" "${vendor_dir}" "${source_commit}"'));
assert.ok(hook < smoke.indexOf('\nsmoke_completed=1'));
assert.match(smoke, /if \[\[ -n "\$\{TACHIKO_V3_CI_EVIDENCE_ROOT:-\}" \]\]; then/);
// Remove only the mock runner's known typecheck logs, which retain /tmp defaults.
for (const file of readdirSync('/tmp')) {
  if (file.startsWith(`tachiko-v3-worker-typecheck-${process.env.MOCK_HEAD.slice(0,12)}-mock-`) && file.endsWith('.log')) unlinkSync(join('/tmp', file));
}
NODE
echo "v3 Worker CI: ${cases} mock cases and hosted wiring checks passed"
