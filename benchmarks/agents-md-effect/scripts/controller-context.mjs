import {createHash, createHmac, timingSafeEqual} from "node:crypto";
import {lstat, readFile, readdir, realpath} from "node:fs/promises";
import {basename, dirname, isAbsolute, relative, resolve} from "node:path";

const ID = /^[0-9a-f]{32}$/;
const SHA256 = /^[0-9a-f]{64}$/;
const SHA1 = /^[0-9a-f]{40}$/;
const FORMAL_PHASES = new Set(["baseline_a", "variant_b"]);
const TERMINAL_DISPOSITIONS = new Set([
  "awaiting_review", "agent_timeout", "agent_failed", "invalid_discarded",
  "infrastructure_failed",
]);
const CONTEXT_STAGE_SEQUENCE = [
  "attempt_registration",
  "base_workspace_preparation",
  "same_wave_base_control",
  "candidate_workspace_preparation",
  "candidate_preflight",
  "provider_auth_preflight",
  "agent_launch",
  "agent_process",
  "overlay_identity_postcheck",
  "candidate_capture",
  "validation_preparation",
  "core_validation",
  "controller_evidence_context",
];

export function contextSha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (value && typeof value === "object") {
    return `{${Object.keys(value).sort().map((key) =>
      `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(",")}}`;
  }
  return JSON.stringify(value);
}

function canonicalBytes(value) {
  return Buffer.from(`${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function inside(candidate, parent) {
  const path = relative(parent, candidate);
  return path === "" || (!path.startsWith("..") && !isAbsolute(path));
}

async function regularIdentity(path, label, expected = null) {
  if (!isAbsolute(path ?? "")) throw new Error(`${label} must use an absolute path`);
  const requested = resolve(path);
  const metadata = await lstat(requested);
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    throw new Error(`${label} must be a non-symlink regular file`);
  }
  const canonical = await realpath(requested);
  const bytes = await readFile(canonical);
  const identity = {path: canonical, bytes: bytes.length, sha256: contextSha256(bytes)};
  if (expected && (
    expected.path !== identity.path || expected.bytes !== identity.bytes ||
    expected.sha256 !== identity.sha256
  )) {
    throw new Error(`${label} identity mismatch`);
  }
  return {...identity, content: bytes};
}

async function appendOnlyPrefixIdentity(path, label, expected) {
  if (!isAbsolute(path ?? "") || !expected || !Number.isSafeInteger(expected.bytes) ||
      expected.bytes < 1 || !SHA256.test(expected.sha256 ?? "")) {
    throw new Error(`${label} append-only identity is invalid`);
  }
  const requested = resolve(path);
  const metadata = await lstat(requested);
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    throw new Error(`${label} must be a non-symlink regular file`);
  }
  const canonical = await realpath(requested);
  const content = await readFile(canonical);
  const prefix = content.subarray(0, expected.bytes);
  if (canonical !== expected.path || content.length < expected.bytes ||
      contextSha256(prefix) !== expected.sha256) {
    throw new Error(`${label} append-only prefix mismatch`);
  }
  return {path: canonical, bytes: expected.bytes, sha256: expected.sha256, content};
}

async function verifyControllerBundleTree(bundleManifestIdentity, manifest, artifactDir) {
  const bundleRoot = resolve(dirname(bundleManifestIdentity.path), "controller-bundle");
  const rootMetadata = await lstat(bundleRoot);
  if (!rootMetadata.isDirectory() || rootMetadata.isSymbolicLink() ||
      !inside(bundleRoot, artifactDir)) {
    throw new Error("controller issuance bundle tree root mismatch");
  }
  const observed = [{
    path: ".",
    type: "directory",
    mode: Number(rootMetadata.mode & 0o777),
  }];
  async function walk(directory, prefix = "") {
    const names = await readdir(directory);
    names.sort((left, right) => Buffer.from(left).compare(Buffer.from(right)));
    for (const name of names) {
      const relativePath = prefix ? `${prefix}/${name}` : name;
      const absolute = resolve(directory, name);
      const metadata = await lstat(absolute);
      if (metadata.isSymbolicLink()) {
        throw new Error(`controller issuance bundle tree contains a symlink: ${relativePath}`);
      }
      if (metadata.isDirectory()) {
        observed.push({
          path: relativePath,
          type: "directory",
          mode: Number(metadata.mode & 0o777),
        });
        await walk(absolute, relativePath);
      } else if (metadata.isFile()) {
        const content = await readFile(absolute);
        observed.push({
          path: relativePath,
          type: "file",
          mode: Number(metadata.mode & 0o777),
          bytes: content.length,
          sha256: contextSha256(content),
        });
      } else {
        throw new Error(`controller issuance bundle tree contains an unsupported node: ${relativePath}`);
      }
    }
  }
  await walk(bundleRoot);
  if (canonicalJson(observed) !== canonicalJson(manifest.entries)) {
    throw new Error("controller issuance bundle tree differs from its immutable manifest");
  }
  return {root: bundleRoot, entries: observed.length, verified: true};
}

function sameRun(left, right, label, {includeProtocol = true} = {}) {
  const keys = ["phase", "wave_id", "run_id", "attempt_id", "candidate_id", "case_id"];
  if (includeProtocol) keys.unshift("protocol_id");
  for (const key of keys) {
    if (left?.[key] !== right?.[key]) throw new Error(`${label} ${key} mismatch`);
  }
}

function containsIdentity(entries, identity) {
  return entries?.some((entry) =>
    entry.path === identity.path && entry.bytes === identity.bytes && entry.sha256 === identity.sha256);
}

function verifyEntryHash(entry, label) {
  const {entry_sha256: claimed, ...body} = entry;
  if (!SHA256.test(claimed ?? "") || contextSha256(canonicalBytes(body)) !== claimed) {
    throw new Error(`${label} entry hash mismatch`);
  }
}

function stageVerifiesControllerBundle(receipt, bundleIdentity) {
  const verification = receipt?.controller_bundle_verification;
  return verification?.schema === "tachiko-controller-bundle-verification-v1" &&
    verification.verified === true &&
    verification.tree_sha256 === bundleIdentity.sha256 &&
    verification.manifest?.path === bundleIdentity.path &&
    verification.manifest?.bytes === bundleIdentity.bytes &&
    verification.manifest?.sha256 === bundleIdentity.sha256;
}

function canonicalTimestamp(value, label) {
  if (typeof value !== "string") throw new Error(`${label} timestamp is invalid`);
  let normalized;
  try { normalized = new Date(value).toISOString(); } catch { /* handled below */ }
  if (normalized !== value) throw new Error(`${label} timestamp is invalid`);
}

async function verifyFormalIssuance({
  context,
  contextIdentity,
  issuancePath,
  expectedIssuanceSha256,
  authorizationPath,
  expectedAuthorizationSha256,
  registryPath,
  expectedRegistrySha256,
}) {
  if (!issuancePath || !SHA256.test(expectedIssuanceSha256 ?? "")) {
    throw new Error("formal controller context requires a controller issuance path and expected SHA-256");
  }
  const issuanceIdentity = await regularIdentity(issuancePath, "controller issuance");
  if (issuanceIdentity.sha256 !== expectedIssuanceSha256) {
    throw new Error("controller issuance SHA-256 mismatch");
  }
  if (!authorizationPath || !SHA256.test(expectedAuthorizationSha256 ?? "")) {
    throw new Error("formal controller context requires an external authorization trust anchor");
  }
  if (!registryPath || !SHA256.test(expectedRegistrySha256 ?? "")) {
    throw new Error("formal controller context requires an external registry trust anchor");
  }
  const issuance = JSON.parse(issuanceIdentity.content.toString("utf8"));
  const {issuer_hmac_sha256: claimedHmac, ...issuanceBody} = issuance;
  if (
    issuance.schema !== "tachiko-controller-context-issuance-v1" ||
    issuance.classification !== "formal_authorized_attempt" ||
    issuance.formal_result_eligible !== true ||
    !Array.isArray(issuance.stage_receipts) ||
    issuance.stage_receipts.length !== CONTEXT_STAGE_SEQUENCE.length ||
    !SHA256.test(claimedHmac ?? "")
  ) {
    throw new Error("controller issuance contract mismatch");
  }
  sameRun(issuance, context, "controller issuance");
  if (
    issuance.context?.path !== contextIdentity.path ||
    issuance.context?.bytes !== contextIdentity.bytes ||
    issuance.context?.sha256 !== contextIdentity.sha256
  ) {
    throw new Error("controller issuance context identity mismatch");
  }

  const authorizationIdentity = await regularIdentity(
    authorizationPath,
    "external formal authorization trust anchor",
  );
  if (authorizationIdentity.sha256 !== expectedAuthorizationSha256 ||
      authorizationIdentity.path !== issuance.formal_authorization?.path ||
      authorizationIdentity.bytes !== issuance.formal_authorization?.bytes ||
      authorizationIdentity.sha256 !== issuance.formal_authorization?.sha256) {
    throw new Error("controller issuance external authorization trust anchor mismatch");
  }
  const authorization = JSON.parse(authorizationIdentity.content.toString("utf8"));
  if (
    authorization.schema !== "tachiko-formal-run-authorization-v1" ||
    typeof authorization.authorization_token !== "string" ||
    authorization.authorization_token.length < 32 ||
    authorizationIdentity.sha256 !== context.formal_authorization_sha256
  ) {
    throw new Error("controller issuance formal authorization mismatch");
  }
  sameRun(authorization, context, "formal authorization", {includeProtocol: false});
  const expectedHmac = createHmac("sha256", authorization.authorization_token)
    .update(canonicalJson(issuanceBody))
    .digest();
  const observedHmac = Buffer.from(claimedHmac, "hex");
  if (observedHmac.length !== expectedHmac.length || !timingSafeEqual(observedHmac, expectedHmac)) {
    throw new Error("controller issuance authentication mismatch");
  }

  const registryIdentity = await regularIdentity(
    registryPath,
    "external attempt registry trust anchor",
  );
  if (registryIdentity.sha256 !== expectedRegistrySha256 ||
      registryIdentity.path !== issuance.attempt_registry_entry?.path ||
      registryIdentity.bytes !== issuance.attempt_registry_entry?.bytes ||
      registryIdentity.sha256 !== issuance.attempt_registry_entry?.sha256) {
    throw new Error("controller issuance external registry trust anchor mismatch");
  }
  const registry = JSON.parse(registryIdentity.content.toString("utf8"));
  const expectedSlotKey = contextSha256(Buffer.from(
    `${context.protocol_id}:${context.phase}:${context.wave_id}:${context.case_id}\n`,
    "utf8",
  ));
  if (
    registry.schema !== "tachiko-controller-attempt-registry-v1" ||
    registry.slot_key_sha256 !== expectedSlotKey ||
    basename(registryIdentity.path) !== `${expectedSlotKey}.json` ||
    registry.uniqueness_scope !== "protocol_id:phase:wave_id:case_id" ||
    registry.formal_authorization_sha256 !== authorizationIdentity.sha256 ||
    registry.provider_auth_qualification_sha256 !==
      context.provider_auth_qualification_sha256 ||
    registry.artifact_dir !== issuance.artifact_dir ||
    authorization.attempt_registry_dir !== dirname(registryIdentity.path) ||
    !inside(contextIdentity.path, registry.artifact_dir) ||
    !inside(issuanceIdentity.path, registry.artifact_dir) ||
    inside(authorizationIdentity.path, registry.artifact_dir) ||
    inside(registryIdentity.path, registry.artifact_dir) ||
    inside(authorizationIdentity.path, registry.source_repo) ||
    inside(registryIdentity.path, registry.source_repo)
  ) {
    throw new Error("controller issuance atomic registry binding mismatch");
  }
  sameRun(registry, context, "attempt registry entry");
  for (const field of [
    "variant_sha256", "agent_executable_sha256", "agent_args_sha256",
    "rustup_home_template_sha256", "pnpm_home_template_sha256",
    "cargo_home_template_sha256", "provider_auth_qualification_sha256",
  ]) {
    if (!SHA256.test(registry[field] ?? "") || authorization[field] !== registry[field]) {
      throw new Error(`controller issuance attempt registry ${field} commitment mismatch`);
    }
  }

  const ledgerIdentity = await appendOnlyPrefixIdentity(
    issuance.attempt_ledger?.path,
    "controller issuance attempt ledger",
    issuance.attempt_ledger,
  );
  if (!inside(ledgerIdentity.path, registry.artifact_dir)) {
    throw new Error("controller issuance attempt ledger escaped its artifact directory");
  }
  const ledgerLines = ledgerIdentity.content.toString("utf8").trim().split("\n");
  if (ledgerLines.length < 1 || ledgerLines.length > 2) {
    throw new Error("controller issuance attempt ledger has an invalid append-only length");
  }
  const registration = JSON.parse(ledgerLines[0]);
  if (registration.schema !== "tachiko-controller-attempt-entry-v1" ||
      registration.disposition !== "registered" || registration.attempt_number !== 1 ||
      registration.replacement_role !== "initial" ||
      registration.previous_attempt_entry_sha256 !== null) {
    throw new Error("controller issuance attempt ledger registration mismatch");
  }
  canonicalTimestamp(registration.registered_at, "attempt ledger registration");
  sameRun(registration, context, "attempt ledger registration");
  verifyEntryHash(registration, "attempt ledger registration");
  let terminalEntry = null;
  if (ledgerLines.length === 2) {
    const terminal = JSON.parse(ledgerLines[1]);
    if (terminal.schema !== "tachiko-controller-attempt-entry-v1" ||
        !TERMINAL_DISPOSITIONS.has(terminal.disposition) || terminal.attempt_number !== 1 ||
        terminal.resampling_performed !== false ||
        !Number.isSafeInteger(terminal.launch_count) || terminal.launch_count < 0 ||
        terminal.launch_count > 1 ||
        !SHA256.test(terminal.final_stage_receipt_sha256 ?? "") ||
        terminal.previous_attempt_entry_sha256 !== registration.entry_sha256) {
      throw new Error("controller issuance terminal ledger append mismatch");
    }
    canonicalTimestamp(terminal.terminal_at, "attempt ledger terminal");
    sameRun(terminal, context, "attempt ledger terminal");
    verifyEntryHash(terminal, "attempt ledger terminal");
    terminalEntry = terminal;
  }

  const bundleIdentity = await regularIdentity(
    issuance.controller_bundle_manifest?.path,
    "controller issuance bundle manifest",
    issuance.controller_bundle_manifest,
  );
  const bundleManifest = JSON.parse(bundleIdentity.content.toString("utf8"));
  if (bundleManifest.schema !== "tachiko-controller-bundle-manifest-v1" ||
      !Array.isArray(bundleManifest.entries) || !inside(bundleIdentity.path, registry.artifact_dir)) {
    throw new Error("controller issuance bundle manifest mismatch");
  }
  await verifyControllerBundleTree(bundleIdentity, bundleManifest, registry.artifact_dir);
  const captureIdentity = await regularIdentity(
    issuance.capture_receipt?.path,
    "controller issuance candidate capture",
    issuance.capture_receipt,
  );
  if (captureIdentity.sha256 !== context.capture_receipt_sha256 ||
      !inside(captureIdentity.path, registry.artifact_dir)) {
    throw new Error("controller issuance candidate capture mismatch");
  }
  const captureReceipt = JSON.parse(captureIdentity.content.toString("utf8"));
  if (!SHA1.test(context.candidate_tree ?? "") ||
      !SHA256.test(context.raw_tree_digest_sha256 ?? "") ||
      captureReceipt.candidate_tree !== context.candidate_tree ||
      captureReceipt.raw_tree_digest_sha256 !== context.raw_tree_digest_sha256) {
    throw new Error("controller issuance captured candidate tree binding mismatch");
  }

  let prior = null;
  let contextStageIdentity = null;
  for (const [index, expectedIdentity] of issuance.stage_receipts.entries()) {
    const stageIdentity = await regularIdentity(
      expectedIdentity.path,
      `controller issuance stage receipt ${index}`,
      expectedIdentity,
    );
    if (!inside(stageIdentity.path, registry.artifact_dir)) {
      throw new Error("controller issuance stage receipt escaped its artifact directory");
    }
    const receipt = JSON.parse(stageIdentity.content.toString("utf8"));
    if (
      receipt.schema !== "tachiko-controller-stage-receipt-v1" ||
      receipt.stage !== CONTEXT_STAGE_SEQUENCE[index] || receipt.stage_order !== index ||
      receipt.prior_receipt_sha256 !== prior ||
      receipt.payload_sha256 !== contextSha256(canonicalBytes(receipt.payload))
    ) {
      throw new Error("controller issuance stage chain mismatch");
    }
    sameRun(receipt, context, `controller stage ${receipt.stage}`);
    if (
      receipt.formal_authorization?.path !== authorizationIdentity.path ||
      receipt.formal_authorization?.bytes !== authorizationIdentity.bytes ||
      receipt.formal_authorization?.sha256 !== authorizationIdentity.sha256 ||
      receipt.attempt_registry_entry?.path !== registryIdentity.path ||
      receipt.attempt_registry_entry?.bytes !== registryIdentity.bytes ||
      receipt.attempt_registry_entry?.sha256 !== registryIdentity.sha256 ||
      receipt.provider_auth_qualification_sha256 !==
        context.provider_auth_qualification_sha256 ||
      receipt.infrastructure_identity_sha256 !== bundleIdentity.sha256 ||
      !stageVerifiesControllerBundle(receipt, bundleIdentity)
    ) {
      throw new Error("controller issuance stage authorization or infrastructure binding mismatch");
    }
    for (const artifact of [...(receipt.inputs ?? []), ...(receipt.outputs ?? [])]) {
      if (artifact.path === ledgerIdentity.path) {
        await appendOnlyPrefixIdentity(
          artifact.path,
          `controller stage ${receipt.stage} append-only ledger`,
          artifact,
        );
      } else {
        await regularIdentity(artifact.path, `controller stage ${receipt.stage} artifact`, artifact);
      }
    }
    if (receipt.stage === "attempt_registration" &&
        (!containsIdentity(receipt.outputs, ledgerIdentity) ||
          !containsIdentity(receipt.outputs, bundleIdentity))) {
      throw new Error("controller issuance registration stage omits ledger or bundle evidence");
    }
    if (receipt.stage === "candidate_capture" && !containsIdentity(receipt.outputs, captureIdentity)) {
      throw new Error("controller issuance capture stage omits captured candidate evidence");
    }
    if (receipt.stage === "controller_evidence_context") {
      if (!containsIdentity(receipt.outputs, contextIdentity) ||
          !containsIdentity(receipt.inputs, captureIdentity)) {
        throw new Error("controller issuance context stage omits context or capture evidence");
      }
      contextStageIdentity = stageIdentity;
    }
    prior = stageIdentity.sha256;
  }

  if (!contextStageIdentity || issuance.context_stage_receipt_sha256 !== contextStageIdentity.sha256) {
    throw new Error("controller issuance context-stage commitment mismatch");
  }
  const issuanceStagePath = resolve(
    dirname(contextStageIdentity.path),
    `${String(CONTEXT_STAGE_SEQUENCE.length).padStart(2, "0")}-controller_context_issuance.json`,
  );
  const issuanceStageIdentity = await regularIdentity(
    issuanceStagePath,
    "controller issuance stage receipt",
  );
  const issuanceStage = JSON.parse(issuanceStageIdentity.content.toString("utf8"));
  if (
    issuanceStage.schema !== "tachiko-controller-stage-receipt-v1" ||
    issuanceStage.stage !== "controller_context_issuance" ||
    issuanceStage.stage_order !== CONTEXT_STAGE_SEQUENCE.length ||
    issuanceStage.prior_receipt_sha256 !== contextStageIdentity.sha256 ||
    issuanceStage.payload_sha256 !== contextSha256(canonicalBytes(issuanceStage.payload)) ||
    issuanceStage.payload?.controller_issuance_sha256 !== issuanceIdentity.sha256 ||
    !containsIdentity(issuanceStage.inputs, contextIdentity) ||
    !containsIdentity(issuanceStage.inputs, authorizationIdentity) ||
    !containsIdentity(issuanceStage.inputs, registryIdentity) ||
    !containsIdentity(issuanceStage.inputs, ledgerIdentity) ||
    !containsIdentity(issuanceStage.inputs, bundleIdentity) ||
    !containsIdentity(issuanceStage.inputs, captureIdentity) ||
    !containsIdentity(issuanceStage.outputs, issuanceIdentity) ||
    issuanceStage.formal_authorization?.sha256 !== authorizationIdentity.sha256 ||
    issuanceStage.attempt_registry_entry?.sha256 !== registryIdentity.sha256 ||
    issuanceStage.infrastructure_identity_sha256 !== bundleIdentity.sha256 ||
    !stageVerifiesControllerBundle(issuanceStage, bundleIdentity)
  ) {
    throw new Error("controller issuance was not committed by the authorized stage chain");
  }
  sameRun(issuanceStage, context, "controller issuance stage");

  let verifiedFinalStageSha256 = issuanceStageIdentity.sha256;
  let expectedLaterStageOrder = CONTEXT_STAGE_SEQUENCE.length + 1;
  const laterStageNames = (await readdir(dirname(contextStageIdentity.path)))
    .filter((name) => {
      const match = /^(\d+)-.+\.json$/.exec(name);
      return match && Number(match[1]) > CONTEXT_STAGE_SEQUENCE.length;
    })
    .sort((left, right) => Number(left.split("-", 1)[0]) - Number(right.split("-", 1)[0]));
  for (const name of laterStageNames) {
    const stageIdentity = await regularIdentity(
      resolve(dirname(contextStageIdentity.path), name),
      `controller post-issuance stage ${expectedLaterStageOrder}`,
    );
    const receipt = JSON.parse(stageIdentity.content.toString("utf8"));
    if (
      receipt.schema !== "tachiko-controller-stage-receipt-v1" ||
      receipt.stage_order !== expectedLaterStageOrder ||
      receipt.prior_receipt_sha256 !== verifiedFinalStageSha256 ||
      receipt.payload_sha256 !== contextSha256(canonicalBytes(receipt.payload)) ||
      receipt.formal_authorization?.sha256 !== authorizationIdentity.sha256 ||
      receipt.attempt_registry_entry?.sha256 !== registryIdentity.sha256 ||
      receipt.infrastructure_identity_sha256 !== bundleIdentity.sha256 ||
      !stageVerifiesControllerBundle(receipt, bundleIdentity)
    ) {
      throw new Error("controller post-issuance stage chain mismatch");
    }
    sameRun(receipt, context, `controller post-issuance stage ${receipt.stage}`);
    for (const artifact of [...(receipt.inputs ?? []), ...(receipt.outputs ?? [])]) {
      if (artifact.path === ledgerIdentity.path) {
        await appendOnlyPrefixIdentity(
          artifact.path,
          `controller post-issuance stage ${receipt.stage} append-only ledger`,
          artifact,
        );
      } else {
        await regularIdentity(
          artifact.path,
          `controller post-issuance stage ${receipt.stage} artifact`,
          artifact,
        );
      }
    }
    verifiedFinalStageSha256 = stageIdentity.sha256;
    expectedLaterStageOrder += 1;
  }
  if (terminalEntry && terminalEntry.final_stage_receipt_sha256 !== verifiedFinalStageSha256) {
    throw new Error("controller terminal does not reach the final verified stage receipt");
  }

  return {
    issuance,
    issuance_path: issuanceIdentity.path,
    issuance_sha256: issuanceIdentity.sha256,
    authorization_path: authorizationIdentity.path,
    authorization_sha256: authorizationIdentity.sha256,
    registry_path: registryIdentity.path,
    registry_sha256: registryIdentity.sha256,
  };
}

export function constructionEvidenceContext() {
  return {
    context: null,
    context_path: null,
    context_sha256: null,
    issuance: null,
    issuance_path: null,
    issuance_sha256: null,
    authorization_path: null,
    authorization_sha256: null,
    registry_path: null,
    registry_sha256: null,
    classification: "construction_pilot_only",
    formal_result_eligible: false,
  };
}

export async function loadControllerContext({
  path,
  expectedSha256,
  issuancePath,
  expectedIssuanceSha256,
  authorizationPath,
  expectedAuthorizationSha256,
  registryPath,
  expectedRegistrySha256,
  required = false,
} = {}) {
  if (!path) {
    if (required) throw new Error("controller context is required");
    return constructionEvidenceContext();
  }
  if (!isAbsolute(path) || !SHA256.test(expectedSha256 ?? "")) {
    throw new Error("controller context requires an absolute path and expected SHA-256");
  }
  const contextIdentity = await regularIdentity(path, "controller context");
  if (contextIdentity.sha256 !== expectedSha256) {
    throw new Error("controller context SHA-256 mismatch");
  }
  const context = JSON.parse(contextIdentity.content.toString("utf8"));
  const formal = FORMAL_PHASES.has(context.phase);
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
  let issuance = {
    issuance: null, issuance_path: null, issuance_sha256: null,
    authorization_path: null, authorization_sha256: null,
    registry_path: null, registry_sha256: null,
  };
  if (formal) {
    if (
      context.classification !== "formal_authorized_attempt" ||
      context.formal_result_eligible !== true ||
      !SHA256.test(context.formal_authorization_sha256 ?? "") ||
      !SHA256.test(context.provider_auth_qualification_sha256 ?? "")
    ) {
      throw new Error("formal controller context lacks an external authorization binding");
    }
    issuance = await verifyFormalIssuance({
      context,
      contextIdentity,
      issuancePath,
      expectedIssuanceSha256,
      authorizationPath,
      expectedAuthorizationSha256,
      registryPath,
      expectedRegistrySha256,
    });
  } else if (
    context.classification !== "construction_pilot_only" ||
    context.formal_result_eligible !== false ||
    context.formal_authorization_sha256 !== null ||
    context.provider_auth_qualification_sha256 !== null
  ) {
    throw new Error("construction controller context may not claim formal eligibility");
  } else if (
    issuancePath || expectedIssuanceSha256 || authorizationPath || expectedAuthorizationSha256 ||
    registryPath || expectedRegistrySha256
  ) {
    throw new Error("construction controller context may not carry a formal controller issuance");
  }
  return {
    context,
    context_path: contextIdentity.path,
    context_sha256: expectedSha256,
    ...issuance,
    classification: context.classification,
    formal_result_eligible: context.formal_result_eligible,
  };
}
