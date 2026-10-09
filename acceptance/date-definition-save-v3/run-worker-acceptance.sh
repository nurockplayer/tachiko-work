#!/usr/bin/env bash
set -euo pipefail

seed_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(cd "${seed_dir}/../.." && pwd -P)"
package_dir="${repo_root}/packages/browser-client"
capture_dir="${TACHIKO_V3_ACCEPTANCE_CAPTURE_DIR:-}"
[[ -n "${capture_dir}" && -d "${capture_dir}" ]] || {
  echo "NOTRUN_MISSING_CANDIDATE_CAPTURE: native fresh capture directory is required" >&2
  exit 77
}
kit_dir="${repo_root}/examples/experimental-designer-client/vendor/tachiko"
[[ -f "${kit_dir}/artifact-manifest.json" ]] || {
  echo "NOTRUN_MISSING_EXPORTED_KIT: export the exact candidate kit before Worker acceptance" >&2
  exit 77
}

identity="$(node --input-type=module - "${capture_dir}" "${kit_dir}" <<'NODE'
import { createHash } from "node:crypto";
import { lstatSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
const [captureDir, kitDir] = process.argv.slice(2);
const readJson = (file) => JSON.parse(readFileSync(path.join(captureDir, file), "utf8"));
const lease = readJson("capture-lease.json");
const native = readJson("native-result.json");
if (lease.state !== "native_complete" || native.status !== "native_cases_passed" ||
    lease.candidate_head !== native.candidate_head || lease.run_id !== native.run_id ||
    native.required_case_count !== 16 || native.required_cases?.length !== 16) {
  throw new Error("NOTRUN_STALE_OR_PARTIAL_CAPTURE: all required native acceptance cases must pass first");
}
const manifestBytes = readFileSync(path.join(kitDir, "artifact-manifest.json"));
const kit = JSON.parse(manifestBytes.toString("utf8"));
if (kit.sourceCommit !== native.candidate_head) throw new Error("NOTRUN_STALE_KIT_SOURCE: kit source commit differs from native candidate");
const kitDigest = createHash("sha256").update(manifestBytes).digest("hex");
const actual = [];
function walk(relative = "") {
  for (const entry of readdirSync(path.join(kitDir, relative), { withFileTypes: true })) {
    const name = relative ? `${relative}/${entry.name}` : entry.name;
    const full = path.join(kitDir, name);
    const stat = lstatSync(full);
    if (stat.isSymbolicLink()) throw new Error(`NOTRUN_KIT_SYMLINK: ${name}`);
    if (stat.isDirectory()) walk(name);
    else if (stat.isFile() && name !== "artifact-manifest.json") actual.push(name);
    else if (!stat.isFile()) throw new Error(`NOTRUN_KIT_NONREGULAR: ${name}`);
  }
}
walk(); actual.sort();
const declared = kit.files.map((file) => file.path).sort();
if (JSON.stringify(actual) !== JSON.stringify(declared)) throw new Error("NOTRUN_STALE_OR_PARTIAL_KIT: kit inventory differs from artifact manifest");
for (const file of kit.files) {
  const bytes = readFileSync(path.join(kitDir, file.path));
  if (createHash("sha256").update(bytes).digest("hex") !== file.sha256) throw new Error(`NOTRUN_KIT_HASH_MISMATCH: ${file.path}`);
}
const log = path.join(captureDir, "worker-acceptance.log");
if (readdirSync(captureDir).includes("worker-completion.json") || readdirSync(captureDir).includes("worker-acceptance.log")) {
  throw new Error("NOTRUN_CAPTURE_NOT_FRESH: refusing an existing Worker result or log");
}
process.stdout.write(`${native.candidate_head}\n${native.run_id}\n${kitDigest}\n${log}\n`);
NODE
)"
candidate_head="$(printf '%s\n' "${identity}" | sed -n '1p')"
run_id="$(printf '%s\n' "${identity}" | sed -n '2p')"
kit_id="$(printf '%s\n' "${identity}" | sed -n '3p')"
log_path="$(printf '%s\n' "${identity}" | sed -n '4p')"

export TACHIKO_V3_ACCEPTANCE_CAPTURE_DIR="${capture_dir}"
export TACHIKO_V3_ACCEPTANCE_CANDIDATE_HEAD="${candidate_head}"
export TACHIKO_V3_ACCEPTANCE_RUN_ID="${run_id}"
export TACHIKO_V3_ACCEPTANCE_KIT_ID="${kit_id}"
typecheck_log="/tmp/tachiko-v3-worker-typecheck-${candidate_head:0:12}-${run_id}.log"
if ! pnpm --dir "${package_dir}" exec tsc --ignoreConfig --noEmit \
  --target ES2024 --module ESNext --moduleResolution Bundler \
  --lib ES2024,DOM,DOM.Iterable,WebWorker --allowImportingTsExtensions \
  --rewriteRelativeImportExtensions --strict --noUncheckedIndexedAccess \
  --exactOptionalPropertyTypes --useUnknownInCatchVariables --skipLibCheck \
  --types node e2e/v3-project-acceptance.spec.ts playwright.v3-acceptance.config.ts \
  >"${typecheck_log}" 2>&1; then
  cat "${typecheck_log}" >&2
  echo "NOTRUN_WORKER_TEST_BUILD: strict acceptance-source typecheck failed; no browser behavior result was recorded" >&2
  exit 77
fi
if ! pnpm --dir "${package_dir}" exec playwright test \
  --config playwright.v3-acceptance.config.ts --reporter=list >"${log_path}" 2>&1; then
  cat "${log_path}" >&2
  if rg -q 'NOTRUN_MISSING_METHOD|error TS[0-9]+|Cannot find module|is not assignable to type|does not satisfy the constraint' "${log_path}"; then
    echo "NOTRUN_WORKER_METHOD_OR_BUILD_SEAM: Worker capability or test build is unavailable; no behavior result was recorded" >&2
    exit 77
  fi
  echo "WORKER_INCOMPLETE: no completion manifest was written" >&2
  exit 1
fi
cat "${log_path}"
if ! rg -F "2 passed" "${log_path}" >/dev/null || rg -F "skipped" "${log_path}" >/dev/null; then
  echo "WORKER_INCOMPLETE: exactly two required Worker cases must pass and none may be skipped" >&2
  exit 1
fi
for case_name in \
  "actual Worker imports, explicitly exports and reopens every v3 profile" \
  "Worker preserves typed failures and resets origin after replacement or close"; do
  if ! rg -F "${case_name}" "${log_path}" >/dev/null; then
    echo "WORKER_INCOMPLETE_MISSING_CASE: ${case_name}" >&2
    exit 1
  fi
done

node --input-type=module - "${capture_dir}" "${candidate_head}" "${run_id}" "${kit_id}" "${log_path}" <<'NODE'
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
const [dir, candidateHead, runId, kitId, logPath] = process.argv.slice(2);
const nativeBytes = readFileSync(path.join(dir, "native-result.json"));
const lease = JSON.parse(readFileSync(path.join(dir, "capture-lease.json"), "utf8"));
if (lease.state !== "native_complete" || lease.candidate_head !== candidateHead || lease.run_id !== runId) throw new Error("WORKER_INCOMPLETE_STALE_NATIVE_RESULT");
const log = readFileSync(logPath);
writeFileSync(path.join(dir, "worker-completion.json"), `${JSON.stringify({
  status: "native_and_worker_cases_passed",
  candidate_head: candidateHead,
  capture_run_id: runId,
  kit_artifact_manifest_sha256: kitId,
  native_result_sha256: createHash("sha256").update(nativeBytes).digest("hex"),
  worker_log_sha256: createHash("sha256").update(log).digest("hex"),
  required_worker_case_count: 2,
  required_worker_cases: [
    "actual Worker imports, explicitly exports and reopens every v3 profile",
    "Worker preserves typed failures and resets origin after replacement or close",
  ],
}, null, 2)}\n`, { flag: "wx", mode: 0o600 });
lease.state = "worker_complete";
lease.kit_artifact_manifest_sha256 = kitId;
writeFileSync(path.join(dir, "capture-lease.json"), `${JSON.stringify(lease, null, 2)}\n`, { flag: "w", mode: 0o600 });
NODE

echo "NATIVE_AND_WORKER_CASES_PASSED candidate=${candidate_head} run_id=${run_id} kit_manifest=${kit_id} capture=${capture_dir}"
