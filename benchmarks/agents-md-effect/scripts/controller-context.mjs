import {createHash} from "node:crypto";
import {lstat, readFile, realpath} from "node:fs/promises";
import {isAbsolute, resolve} from "node:path";

const ID = /^[0-9a-f]{32}$/;
const SHA256 = /^[0-9a-f]{64}$/;

export function contextSha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

export function constructionEvidenceContext() {
  return {
    context: null,
    context_path: null,
    context_sha256: null,
    classification: "construction_pilot_only",
    formal_result_eligible: false,
  };
}

export async function loadControllerContext({path, expectedSha256, required = false} = {}) {
  if (!path) {
    if (required) throw new Error("controller context is required");
    return constructionEvidenceContext();
  }
  if (!isAbsolute(path) || !SHA256.test(expectedSha256 ?? "")) {
    throw new Error("controller context requires an absolute path and expected SHA-256");
  }
  const metadata = await lstat(resolve(path));
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    throw new Error("controller context must be a non-symlink regular file");
  }
  const canonical = await realpath(resolve(path));
  const bytes = await readFile(canonical);
  if (contextSha256(bytes) !== expectedSha256) {
    throw new Error("controller context SHA-256 mismatch");
  }
  const context = JSON.parse(bytes.toString("utf8"));
  const formal = ["baseline_a", "variant_b"].includes(context.phase);
  if (
    context.schema !== "tachiko-controller-evidence-context-v1" ||
    typeof context.protocol_id !== "string" ||
    !/^TW-0[1-9]$/.test(context.case_id ?? "") ||
    ![context.wave_id, context.run_id, context.attempt_id, context.candidate_id]
      .every((value) => ID.test(value ?? "")) ||
    !SHA256.test(context.capture_receipt_sha256 ?? "") ||
    !["construction_pilot_only", "baseline_a", "variant_b"].includes(context.phase)
  ) {
    throw new Error("controller context contract mismatch");
  }
  if (formal) {
    if (
      context.classification !== "formal_authorized_attempt" ||
      context.formal_result_eligible !== true ||
      !SHA256.test(context.formal_authorization_sha256 ?? "")
    ) {
      throw new Error("formal controller context lacks an external authorization binding");
    }
  } else if (
    context.classification !== "construction_pilot_only" ||
    context.formal_result_eligible !== false ||
    context.formal_authorization_sha256 !== null
  ) {
    throw new Error("construction controller context may not claim formal eligibility");
  }
  return {
    context,
    context_path: canonical,
    context_sha256: expectedSha256,
    classification: context.classification,
    formal_result_eligible: context.formal_result_eligible,
  };
}
