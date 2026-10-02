#!/usr/bin/env bash
set -euo pipefail

seed_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(cd "${seed_dir}/../.." && pwd -P)"
runtime_manifest="${repo_root}/packages/browser-client/runtime/Cargo.toml"

if ! rg -q 'export_project_v3' "${repo_root}/packages/browser-client/runtime/src"; then
  echo "NOTRUN_MISSING_METHOD: selected-v3 Rust adapter has no candidate runtime method; no build started" >&2
  exit 77
fi
if [[ -n "$(git -C "${repo_root}" status --porcelain)" ]]; then
  echo "NOTRUN_DIRTY_CANDIDATE: acceptance requires a clean committed candidate source tree" >&2
  exit 77
fi
for source_path in \
  packages/browser-client/runtime/src/lib.rs \
  packages/browser-client/runtime/tests/v3_project_acceptance.rs \
  packages/browser-client/e2e/v3-project-acceptance.spec.ts \
  acceptance/date-definition-save-v3/SEED.md \
  acceptance/date-definition-save-v3/fixtures/mixed.csv; do
  if ! git -C "${repo_root}" cat-file -e "HEAD:${source_path}" 2>/dev/null; then
    echo "NOTRUN_UNCOMMITTED_CANDIDATE_SOURCE: ${source_path} is absent from HEAD" >&2
    exit 77
  fi
done

candidate_head="$(git -C "${repo_root}" rev-parse --verify 'HEAD^{commit}')"
[[ "${candidate_head}" =~ ^[0-9a-f]{40}$ ]] || {
  echo "NOTRUN_INVALID_CANDIDATE_HEAD: expected one exact 40-hex commit" >&2
  exit 77
}
run_id="$(node --input-type=module -e 'process.stdout.write(crypto.randomUUID())')"
short_head="${candidate_head:0:12}"
capture_dir="$(mktemp -d "/tmp/tachiko-v3-capture-${short_head}.XXXXXX")"
target_dir="/tmp/tachiko-v3-cargo-target-${short_head}-${run_id}"
[[ ! -e "${target_dir}" ]] || {
  echo "NOTRUN_CARGO_TARGET_EXISTS: refusing stale target directory ${target_dir}" >&2
  exit 77
}

export TACHIKO_V3_ACCEPTANCE_CANDIDATE_HEAD="${candidate_head}"
export TACHIKO_V3_ACCEPTANCE_RUN_ID="${run_id}"
export TACHIKO_V3_ACCEPTANCE_CAPTURE_REQUIRED=1
export TACHIKO_V3_ACCEPTANCE_CAPTURE_DIR="${capture_dir}"
export CARGO_TARGET_DIR="${target_dir}"
export CARGO_BUILD_JOBS=1

node --input-type=module - "${capture_dir}" "${candidate_head}" "${run_id}" <<'NODE'
import { writeFileSync } from "node:fs";
const [dir, candidateHead, runId] = process.argv.slice(2);
writeFileSync(`${dir}/capture-lease.json`, JSON.stringify({
  state: "native_in_progress",
  candidate_head: candidateHead,
  run_id: runId,
  created_at: new Date().toISOString(),
}), { flag: "wx", mode: 0o600 });
NODE

log_path="/tmp/tachiko-v3-native-${short_head}-${run_id}.log"
if ! cargo test --manifest-path "${runtime_manifest}" --locked \
  --test v3_project_acceptance -- --test-threads=1 >"${log_path}" 2>&1; then
  cat "${log_path}" >&2
  if rg -q 'error: could not compile|error\[E[0-9]+\]|no method named|method not found' "${log_path}"; then
    echo "NOTRUN_BUILD_OR_API_SEAM: candidate acceptance did not compile; no behavior result was recorded" >&2
    exit 77
  fi
  echo "NATIVE_INCOMPLETE: partial artifacts remain unsealed at ${capture_dir}" >&2
  exit 1
fi
cat "${log_path}"

required_cases=(
  date_and_saved_definition_lossless_reopen
  date_only_lossless_reopen
  definition_only_lossless_reopen
  neither_lossless_reopen
  ordinary_export_keeps_the_mixed_legacy_refusal_control
  frozen_codec_and_bridge_profile_controls
  legacy_date_only_ingress_edit_reexport
  legacy_definition_only_ingress_edit_reexport
  legacy_neither_ingress_edit_reexport
  fresh_candidate_ordinary_legacy_exports_remain_v1_v2_compatible
  candidate_inspect_and_open_refuse_valid_v3_text_and_number_constraints
  exact_full_catalogue_opened_projection_boundary_and_plus_one
  version_gate_and_opaque_transfer_malformed_vectors_are_distinct_and_atomic
  successful_legacy_replacement_clears_v3_origin_marker
  successful_v3_replacement_preserves_origin_marker
  transfer_limit_and_oversized_inspect_preserve_the_current_v3_occurrence
)
for case_name in "${required_cases[@]}"; do
  if ! rg -F "test ${case_name} ... ok" "${log_path}" >/dev/null; then
    echo "NATIVE_INCOMPLETE_MISSING_CASE: ${case_name}; capture remains unsealed at ${capture_dir}" >&2
    exit 1
  fi
done

node --input-type=module - "${capture_dir}" "${candidate_head}" "${run_id}" "${log_path}" <<'NODE'
import { createHash } from "node:crypto";
import { lstatSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
const [dir, candidateHead, runId, logPath] = process.argv.slice(2);
const names = [
  "date-definition", "date-only", "definition-only", "neither",
  "legacy-ingress-date-only", "legacy-ingress-definition-only", "legacy-ingress-neither",
  "ordinary-date-only", "ordinary-definition-only", "ordinary-neither",
  "constrained-text", "constrained-number", "constrained-nondefault-text", "constrained-nondefault-number", "boundary-fresh-65536", "boundary-fresh-65537-probe",
];
const allowed = new Set(["capture-lease.json"]);
const artifacts = {};
for (const name of names) {
  for (const extension of ["twd", "source.json"]) {
    const file = `${name}.${extension}`;
    const absolute = path.join(dir, file);
    const stat = lstatSync(absolute);
    if (!stat.isFile() || stat.isSymbolicLink()) throw new Error(`NATIVE_INCOMPLETE_NONREGULAR: ${file}`);
    const bytes = readFileSync(absolute);
    const digest = createHash("sha256").update(bytes).digest("hex");
    artifacts[file] = digest;
    allowed.add(file);
    if (extension === "source.json") {
      const receipt = JSON.parse(bytes.toString("utf8"));
      if (receipt.producer_head !== candidateHead || receipt.capture_run_id !== runId || receipt.profile !== name) {
        throw new Error(`NATIVE_INCOMPLETE_RECEIPT_MISMATCH: ${file}`);
      }
    }
  }
}
const actual = readdirSync(dir).sort();
if (JSON.stringify(actual) !== JSON.stringify([...allowed].sort())) {
  throw new Error(`NATIVE_INCOMPLETE_FILE_SET: ${actual.join(",")}`);
}
const leasePath = path.join(dir, "capture-lease.json");
const lease = JSON.parse(readFileSync(leasePath, "utf8"));
if (lease.candidate_head !== candidateHead || lease.run_id !== runId || lease.state !== "native_in_progress") {
  throw new Error("NATIVE_INCOMPLETE_LEASE_MISMATCH");
}
writeFileSync(path.join(dir, "native-result.json"), JSON.stringify({
  status: "native_cases_passed",
  candidate_head: candidateHead,
  run_id: runId,
  native_log_sha256: createHash("sha256").update(readFileSync(logPath)).digest("hex"),
  required_case_count: 16,
  required_cases: [
    "date_and_saved_definition_lossless_reopen", "date_only_lossless_reopen",
    "definition_only_lossless_reopen", "neither_lossless_reopen",
    "ordinary_export_keeps_the_mixed_legacy_refusal_control", "frozen_codec_and_bridge_profile_controls",
    "legacy_date_only_ingress_edit_reexport", "legacy_definition_only_ingress_edit_reexport",
    "legacy_neither_ingress_edit_reexport", "fresh_candidate_ordinary_legacy_exports_remain_v1_v2_compatible",
    "candidate_inspect_and_open_refuse_valid_v3_text_and_number_constraints",
    "exact_full_catalogue_opened_projection_boundary_and_plus_one",
    "version_gate_and_opaque_transfer_malformed_vectors_are_distinct_and_atomic",
    "successful_legacy_replacement_clears_v3_origin_marker",
    "successful_v3_replacement_preserves_origin_marker",
    "transfer_limit_and_oversized_inspect_preserve_the_current_v3_occurrence",
  ],
  artifacts,
}, null, 2) + "\n", { flag: "wx", mode: 0o600 });
lease.state = "native_complete";
writeFileSync(leasePath, JSON.stringify(lease, null, 2) + "\n", { flag: "w", mode: 0o600 });
NODE

echo "NATIVE_CASES_PASSED candidate=${candidate_head} run_id=${run_id} capture=${capture_dir}"
echo "NEXT: export the matching kit, then seal this capture and run the historical/Worker lanes serially."
