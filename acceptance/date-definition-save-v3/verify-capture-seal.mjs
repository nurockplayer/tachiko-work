// Read-only seal verification. The historical Rust reader consumes these exact
// verified snapshots, rather than reopening potentially changed artifact paths.
import { createHash } from "node:crypto";
import { lstatSync, readFileSync, readdirSync } from "node:fs";
import { resolve, join } from "node:path";
import { pathToFileURL } from "node:url";

export const NATIVE_CASES = [
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
];
export const WORKER_CASES = [
  "actual Worker imports, explicitly exports and reopens every v3 profile",
  "Worker preserves typed failures and resets origin after replacement or close",
];
export const PROFILES = [
  "date-definition", "date-only", "definition-only", "neither",
  "legacy-ingress-date-only", "legacy-ingress-definition-only", "legacy-ingress-neither",
  "ordinary-date-only", "ordinary-definition-only", "ordinary-neither",
  "constrained-text", "constrained-number", "constrained-nondefault-text", "constrained-nondefault-number",
  "boundary-fresh-65536", "boundary-fresh-65537-probe",
];
const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");
const requireEqual = (actual, expected, label) => {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error(`UNSEALED_${label}`);
};
const regularBytes = (file) => {
  const stat = lstatSync(file);
  if (!stat.isFile() || stat.isSymbolicLink()) throw new Error(`UNSEALED_NONREGULAR: ${file}`);
  return readFileSync(file);
};

export function verifyCaptureSeal(captureRoot, candidateHead, runId, kitRoot) {
  if (!/^[0-9a-f]{40}$/.test(candidateHead ?? "") ||
      !/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(runId ?? "")) {
    throw new Error("UNSEALED_IDENTITY");
  }
  if (!captureRoot || !kitRoot) throw new Error("UNSEALED_REQUIRED_DIRECTORY");
  const names = PROFILES.flatMap((profile) => [`${profile}.source.json`, `${profile}.twd`]).sort();
  const controls = ["capture-lease.json", "native-result.json", "worker-acceptance.log", "worker-completion.json"];
  requireEqual(readdirSync(captureRoot).sort(), [...names, ...controls].sort(), "CAPTURE_INVENTORY");
  const snapshot = new Map([...names, ...controls].map((name) => [name, regularBytes(join(captureRoot, name))]));
  const json = (name) => JSON.parse(snapshot.get(name).toString("utf8"));
  const lease = json("capture-lease.json");
  const native = json("native-result.json");
  const completion = json("worker-completion.json");
  requireEqual([lease.state, lease.candidate_head, lease.run_id], ["worker_complete", candidateHead, runId], "LEASE");
  requireEqual([native.status, native.candidate_head, native.run_id, native.required_case_count, native.required_cases],
    ["native_cases_passed", candidateHead, runId, NATIVE_CASES.length, NATIVE_CASES], "NATIVE_COMPLETION");
  requireEqual([completion.status, completion.candidate_head, completion.capture_run_id,
    completion.required_worker_case_count, completion.required_worker_cases],
  ["native_and_worker_cases_passed", candidateHead, runId, WORKER_CASES.length, WORKER_CASES], "WORKER_COMPLETION");
  requireEqual(Object.keys(native.artifacts ?? {}).sort(), names, "ARTIFACT_INVENTORY");
  for (const name of names) {
    requireEqual(digest(snapshot.get(name)), native.artifacts[name], `ARTIFACT_HASH: ${name}`);
    if (name.endsWith(".source.json")) {
      const profile = name.slice(0, -".source.json".length);
      const kind = profile.startsWith("ordinary-") ? "ordinary_legacy_export" :
        profile.startsWith("constrained-") ? "valid_storage_v3_constraint_input" :
          profile.startsWith("boundary-fresh-65537") ? "fresh_admission_probe" : "selected_v3_export";
      const receipt = json(name);
      requireEqual([receipt.producer_head, receipt.capture_run_id, receipt.profile, receipt.kind],
        [candidateHead, runId, profile, kind], `RECEIPT: ${name}`);
    }
  }
  requireEqual(digest(snapshot.get("native-result.json")), completion.native_result_sha256, "NATIVE_RESULT_LINK");
  requireEqual(digest(snapshot.get("worker-acceptance.log")), completion.worker_log_sha256, "WORKER_LOG_LINK");
  const workerLog = snapshot.get("worker-acceptance.log").toString("utf8");
  if (!workerLog.includes("2 passed") || workerLog.includes("skipped") || WORKER_CASES.some((name) => !workerLog.includes(name))) {
    throw new Error("UNSEALED_WORKER_LOG_CASES");
  }

  const kitManifestBytes = regularBytes(join(kitRoot, "artifact-manifest.json"));
  const kit = JSON.parse(kitManifestBytes.toString("utf8"));
  const kitDigest = digest(kitManifestBytes);
  requireEqual([kit.sourceCommit, completion.kit_artifact_manifest_sha256, lease.kit_artifact_manifest_sha256],
    [candidateHead, kitDigest, kitDigest], "KIT_IDENTITY");
  if (!Array.isArray(kit.files)) throw new Error("UNSEALED_KIT_MANIFEST");
  const actualKitFiles = [];
  function walk(relative = "") {
    for (const name of readdirSync(join(kitRoot, relative))) {
      const child = relative ? `${relative}/${name}` : name;
      const stat = lstatSync(join(kitRoot, child));
      if (stat.isSymbolicLink()) throw new Error(`UNSEALED_KIT_SYMLINK: ${child}`);
      if (stat.isDirectory()) walk(child);
      else if (stat.isFile() && child !== "artifact-manifest.json") actualKitFiles.push(child);
      else if (!stat.isFile()) throw new Error(`UNSEALED_KIT_NONREGULAR: ${child}`);
    }
  }
  walk();
  const declared = kit.files.map((file) => {
    if (typeof file.path !== "string" || file.path.startsWith("/") || file.path.includes("\\") ||
        file.path.split("/").some((part) => part === "" || part === "." || part === "..") ||
        !/^[0-9a-f]{64}$/.test(file.sha256 ?? "")) throw new Error("UNSEALED_KIT_MEMBER");
    return file.path;
  }).sort();
  requireEqual(actualKitFiles.sort(), declared, "KIT_INVENTORY");
  for (const file of kit.files) requireEqual(digest(regularBytes(join(kitRoot, file.path))), file.sha256, `KIT_HASH: ${file.path}`);
  return { candidate_head: candidateHead, run_id: runId, kit_artifact_manifest_sha256: kitDigest,
    artifacts: Object.fromEntries(names.map((name) => [name, snapshot.get(name).toString("hex")])) };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  process.stdout.write(`${JSON.stringify(verifyCaptureSeal(...process.argv.slice(2)))}\n`);
}
