import {createHash} from "node:crypto";
import {lstat, readFile, realpath, writeFile} from "node:fs/promises";
import {basename, dirname, isAbsolute, relative, resolve} from "node:path";

const SHA256 = /^[0-9a-f]{64}$/;

function fail(message) { throw new Error(message); }
function sha256(bytes) { return createHash("sha256").update(bytes).digest("hex"); }
function isInside(candidate, parent) {
  const path = relative(parent, candidate);
  return path === "" || (!path.startsWith("..") && !isAbsolute(path));
}

async function regular(path, label) {
  if (!isAbsolute(path)) fail(`${label} must be absolute`);
  const metadata = await lstat(path);
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    fail(`${label} must be a non-symlink regular file`);
  }
  return realpath(path);
}

async function identity(path) {
  const bytes = await readFile(path);
  return {path, bytes: bytes.length, sha256: sha256(bytes)};
}

function exactKeys(value, expected, label) {
  if (!value || typeof value !== "object" || Array.isArray(value) ||
      JSON.stringify(Object.keys(value).sort()) !== JSON.stringify([...expected].sort())) {
    fail(`${label} keys mismatch`);
  }
}

export async function materializeFormalAdapterEnvelope({
  stdout,
  outputPath,
  caseId,
  contractSha256,
  sandboxProfileSha256,
  processGroupExtinct,
  adapterPackage,
}) {
  if (processGroupExtinct !== true) {
    fail("formal adapter output may only be materialized after process-group extinction");
  }
  const lines = stdout.split(/\r?\n/).filter((line) => line.length > 0);
  if (lines.length !== 1) fail("formal adapter scaffold must emit exactly one receipt line");
  const receipt = JSON.parse(lines[0]);
  exactKeys(receipt, [
    "scaffold", "case_id", "config_sha256", "probe_sha256", "probe_stdout_sha256",
    "probe_stderr_sha256", "sandbox_profile_sha256", "denied_read_roots_sha256", "envelope",
  ], "formal adapter receipt");
  if (
    receipt.scaffold !== "tachiko-candidate-adapter-v1" ||
    receipt.case_id !== caseId ||
    receipt.config_sha256 !== adapterPackage.config.sha256 ||
    receipt.probe_sha256 !== adapterPackage.probe.sha256 ||
    receipt.sandbox_profile_sha256 !== sandboxProfileSha256
  ) {
    fail("formal adapter receipt binding mismatch");
  }
  const envelopeKeys = caseId === "TW-05"
    ? ["contract_sha256", "adapter", "native", "wasm"]
    : caseId === "TW-09"
      ? ["contract_sha256", "adapter", "observations"]
      : fail("formal adapter envelope is only supported for TW-05 and TW-09");
  exactKeys(receipt.envelope, envelopeKeys, "formal adapter envelope");
  exactKeys(
    receipt.envelope.adapter,
    ["sha256", "behavior_implemented_by_adapter"],
    "formal adapter envelope identity",
  );
  if (
    receipt.envelope.contract_sha256 !== contractSha256 ||
    receipt.envelope.adapter.sha256 !== adapterPackage.scaffold.sha256 ||
    receipt.envelope.adapter.behavior_implemented_by_adapter !== false
  ) {
    fail("formal adapter envelope binding mismatch");
  }
  const parent = await realpath(dirname(resolve(outputPath)));
  const output = resolve(parent, basename(outputPath));
  const bytes = Buffer.from(`${JSON.stringify(receipt.envelope)}\n`);
  await writeFile(output, bytes, {mode: 0o600, flag: "wx"});
  return {
    path: output,
    bytes: bytes.length,
    sha256: sha256(bytes),
    materialized_after_process_group_extinction: true,
    source_stdout_sha256: sha256(stdout),
    receipt_sha256: sha256(lines[0]),
  };
}

export async function validateFormalAdapterPackage({
  adapterPath,
  configPath,
  integrityReceiptPath,
  expectedIntegrityReceiptSha256,
  benchmarkRoot,
  forbiddenRoots = [],
  context,
}) {
  const expectedPath = await realpath(resolve(
    benchmarkRoot,
    "evaluator/adapters/candidate-adapter.mjs",
  ));
  const adapter = await regular(adapterPath, "formal adapter scaffold");
  if (adapter !== expectedPath) {
    fail("formal adapter must be the sealed qualified candidate adapter scaffold");
  }
  const lockPath = await regular(
    resolve(benchmarkRoot, "evaluator/adapters/candidate-adapter-lock.json"),
    "formal adapter scaffold lock",
  );
  const lockBytes = await readFile(lockPath);
  const lock = JSON.parse(lockBytes.toString("utf8"));
  const adapterBytes = await readFile(adapter);
  if (
    lock.schema !== "tachiko-candidate-adapter-lock-v1" ||
    lock.path !== "evaluator/adapters/candidate-adapter.mjs" ||
    lock.sha256 !== sha256(adapterBytes) ||
    lock.bytes !== adapterBytes.length
  ) {
    fail("sealed qualified candidate adapter scaffold does not match its operational lock");
  }
  if (!configPath) fail("formal adapter config is required");
  const config = await regular(configPath, "formal adapter config");
  const configBytes = await readFile(config);
  const parsed = JSON.parse(configBytes.toString("utf8"));
  if (
    parsed?.schema !== "tachiko-candidate-adapter-v1" ||
    parsed.case_id !== context?.case_id ||
    JSON.stringify(Object.keys(parsed).sort()) !== JSON.stringify(["case_id", "probe", "schema"]) ||
    !parsed.probe ||
    JSON.stringify(Object.keys(parsed.probe).sort()) !==
      JSON.stringify(["arguments", "executable", "sha256"])
  ) {
    fail("formal adapter config contract or case binding mismatch");
  }
  const probeRequested = parsed?.probe?.executable;
  if (typeof probeRequested !== "string" || !isAbsolute(probeRequested)) {
    fail("formal adapter config must name an absolute probe executable");
  }
  const probe = await regular(probeRequested, "formal adapter probe");
  if (((await lstat(probe)).mode & 0o111) === 0) {
    fail("formal adapter probe must be executable");
  }
  if (!Array.isArray(parsed.probe.arguments) || parsed.probe.arguments.length === 0 ||
      parsed.probe.arguments.some((argument) =>
        typeof argument !== "string" || /<(?:contract|output|trusted-[^>]+)>/i.test(argument))) {
    fail("formal adapter probe arguments contain a forbidden contract or expected-value token");
  }
  for (const root of forbiddenRoots) {
    const canonicalRoot = await realpath(root);
    if (isInside(config, canonicalRoot)) {
      fail("formal adapter config is inside a forbidden root and is not disjoint");
    }
    if (isInside(probe, canonicalRoot)) {
      fail("formal adapter probe is inside a forbidden root and is not disjoint");
    }
    for (const argument of parsed.probe.arguments) {
      if (isAbsolute(argument) && isInside(resolve(argument), canonicalRoot)) {
        fail("formal adapter probe arguments expose a forbidden expected-value root");
      }
    }
  }
  if (!integrityReceiptPath || !expectedIntegrityReceiptSha256) {
    fail("formal adapter integrity receipt is required");
  }
  if (!SHA256.test(expectedIntegrityReceiptSha256)) {
    fail("formal adapter integrity receipt expected SHA-256 is invalid");
  }
  const integrityReceipt = await regular(
    integrityReceiptPath,
    "formal adapter integrity receipt",
  );
  for (const root of forbiddenRoots) {
    const canonicalRoot = await realpath(root);
    if (isInside(integrityReceipt, canonicalRoot)) {
      fail("formal adapter integrity receipt is inside a forbidden root and is not disjoint");
    }
  }
  const probeBytes = await readFile(probe);
  if (sha256(probeBytes) !== parsed.probe.sha256) fail("formal adapter probe SHA-256 mismatch");
  const integrityBytes = await readFile(integrityReceipt);
  if (sha256(integrityBytes) !== expectedIntegrityReceiptSha256) {
    fail("formal adapter integrity receipt SHA-256 mismatch");
  }
  const approval = JSON.parse(integrityBytes.toString("utf8"));
  const expectedBindings = {
    protocol_id: context?.protocol_id,
    phase: context?.phase,
    wave_id: context?.wave_id,
    run_id: context?.run_id,
    attempt_id: context?.attempt_id,
    candidate_id: context?.candidate_id,
    case_id: context?.case_id,
    capture_receipt_sha256: context?.capture_receipt_sha256,
    scaffold_sha256: sha256(adapterBytes),
    config_sha256: sha256(configBytes),
    probe_sha256: sha256(probeBytes),
  };
  if (approval.schema !== "tachiko-adapter-integrity-review-v1") {
    fail("formal adapter integrity receipt schema mismatch");
  }
  for (const [key, value] of Object.entries(expectedBindings)) {
    if (approval[key] !== value) fail(`formal adapter integrity receipt ${key} mismatch`);
  }
  if (
    approval.reviewer_eligible !== true || approval.reviewer_independent !== true ||
    typeof approval.reviewer_id !== "string" || approval.reviewer_id.length === 0 ||
    approval.no_expected_values !== true || approval.no_behavior_implementation !== true ||
    approval.actual_candidate_exercise !== true || approval.approved !== true
  ) {
    fail("formal adapter integrity receipt lacks an eligible independent approval");
  }
  return {
    scaffold: await identity(adapter),
    scaffold_lock: await identity(lockPath),
    config: await identity(config),
    probe: await identity(probe),
    integrity_receipt: await identity(integrityReceipt),
    approval,
  };
}
