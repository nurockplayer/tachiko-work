#!/usr/bin/env node

import {spawnSync} from "node:child_process";
import {createHash, createHmac} from "node:crypto";
import {createReadStream, existsSync} from "node:fs";
import {
  appendFile,
  chmod,
  copyFile,
  cp,
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  readlink,
  realpath,
  readdir,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import {basename, dirname, isAbsolute, relative, resolve} from "node:path";
import {fileURLToPath, pathToFileURL} from "node:url";
import {loadControllerContext} from "./controller-context.mjs";
import {
  DENY_NETWORK_PROFILE,
  probeNetworkSandbox,
  runNetworkSandboxed,
} from "./network-sandbox.mjs";
import {
  PROCESS_CONTAINMENT_PROFILE,
  runProcessGroupOnce,
} from "./process-group-supervisor.mjs";
import {
  prepareFreshHomeForKeyring,
  validateProviderAuthQualification,
  verifyChatGptKeyringStatus,
} from "./provider-auth.mjs";

export {validateFormalAdapterPackage} from "./adapter-integrity.mjs";

export function formalAdapterOracleArguments(adapterPackage) {
  return [
    "--adapter-file", adapterPackage.scaffold.path,
    "--adapter-config", adapterPackage.config.path,
    "--adapter-integrity-receipt", adapterPackage.integrity_receipt.path,
    "--expected-adapter-integrity-sha256", adapterPackage.integrity_receipt.sha256,
    ...(adapterPackage.probe_source
      ? ["--adapter-probe-source", adapterPackage.probe_source.path]
      : []),
    ...(adapterPackage.candidate_manifest
      ? ["--candidate-raw-manifest", adapterPackage.candidate_manifest.path]
      : []),
    ...(adapterPackage.probe_build_receipt
      ? [
        "--adapter-probe-file", adapterPackage.probe.path,
        "--expected-adapter-probe-sha256", adapterPackage.probe.sha256,
        "--adapter-probe-build-receipt", adapterPackage.probe_build_receipt.path,
        "--expected-adapter-probe-build-receipt-sha256",
        adapterPackage.probe_build_receipt.sha256,
        "--adapter-build-stage-receipt", adapterPackage.adapter_build_stage_receipt.path,
        "--expected-adapter-build-stage-receipt-sha256",
        adapterPackage.adapter_build_stage_receipt.sha256,
      ]
      : []),
  ];
}

function formalControllerTrustArguments(common) {
  if (!common.formal_result_eligible) return [];
  return [
    "--formal-authorization", common.formal_authorization.path,
    "--expected-formal-authorization-sha256", common.formal_authorization.sha256,
    "--attempt-registry-entry", common.attempt_registry_entry.path,
    "--expected-attempt-registry-entry-sha256", common.attempt_registry_entry.sha256,
  ];
}

export function formalControllerIssuanceRecord({body, authorizationToken}) {
  if (typeof authorizationToken !== "string" || authorizationToken.length < 32 ||
      body?.schema !== "tachiko-controller-context-issuance-v1" ||
      body?.classification !== "formal_authorized_attempt" ||
      body?.formal_result_eligible !== true) {
    fail("formal controller issuance inputs are invalid");
  }
  return {
    ...body,
    issuer_hmac_sha256: createHmac("sha256", authorizationToken)
      .update(canonicalJson(body))
      .digest("hex"),
  };
}

export function requireResumeContextBindings(common, context, state) {
  for (const key of [
    "protocol_id", "phase", "classification", "formal_result_eligible", "wave_id", "run_id",
    "attempt_id", "candidate_id", "case_id",
  ]) {
    if (common?.[key] !== context?.[key]) fail(`controller evidence context resume ${key} mismatch`);
  }
  if (context.capture_receipt_sha256 !== state?.bound_receipts?.capture_sha256) {
    fail("controller evidence context resume capture_receipt_sha256 mismatch");
  }
  if (context.formal_authorization_sha256 !== (common.formal_authorization?.sha256 ?? null)) {
    fail("controller evidence context resume formal_authorization_sha256 mismatch");
  }
  if (context.provider_auth_qualification_sha256 !==
      (common.provider_auth_qualification_sha256 ?? null)) {
    fail("controller evidence context resume provider_auth_qualification_sha256 mismatch");
  }
}

const scriptDir = dirname(fileURLToPath(import.meta.url));
const benchmarkDir = resolve(scriptDir, "..");
const CONTROL_ARTIFACTS = [
  "environment-lock.json",
  "evaluator/cases.json",
  "evaluator/oracle-lock.json",
  "evaluator/core-score-lock.json",
  "evaluator/authority-lock.json",
  "evaluator/production-oracles.json",
];
const ID = /^[0-9a-f]{32}$/;
const SHA256 = /^[0-9a-f]{64}$/;
const RUN_ROOT = /^r-[0-9a-f]{32}$/;
const MAX_PROCESS_BYTES = 128 * 1024 * 1024;
const GIT_CONFIGURATION = [
  "-c", "core.hooksPath=/dev/null",
  "-c", "core.attributesFile=/dev/null",
  "-c", "core.autocrlf=false",
  "-c", "protocol.file.allow=always",
];
let committedFailureHandler = null;

function usage() {
  console.error(
    "usage: node run-controller.mjs --case TW-01 --source-repo /abs/repo " +
      "--variant-file /abs/AGENTS.md --expected-variant-sha256 <sha256> " +
      "--phase construction_pilot_only --run-root /abs/r-<32hex> " +
      "--artifact-dir /abs/new-artifacts --attempt-registry-dir /abs/controller-registry " +
      "--agent-executable /abs/agent " +
      "--agent-args-file /abs/args.json --timeout-seconds 3600 --wave-id <32hex> " +
      "--run-id <32hex> --attempt-id <32hex> --candidate-id <32hex> " +
      "[--model-catalog-file /external/locked-catalog.json] " +
      "[--provider-auth-qualification /external/provider-auth.json " +
      "--operator-keychain /external/login.keychain-db] " +
      "[--construction-smoke true] [--adapter-file /abs/adapter.mjs " +
      "--expected-adapter-sha256 <sha256>] [--adapter-config /external/config.json " +
      "--adapter-probe-source /external/probe.rs " +
      "--adapter-integrity-receipt /external/review.json " +
      "--expected-adapter-integrity-sha256 <sha256>] [--cargo-home-template /abs/template] " +
      "[--rustup-home-template /abs/template] [--authorization-file /external/auth.json]. " +
      "Formal phases require authorization-file, model-catalog-file, cargo-home-template, " +
      "and rustup-home-template.",
  );
  process.exit(2);
}

function fail(message) { throw new Error(message); }
function sha256(bytes) { return createHash("sha256").update(bytes).digest("hex"); }
function canonicalBytes(value) { return Buffer.from(`${JSON.stringify(value, null, 2)}\n`, "utf8"); }

function parseArgs(argv) {
  if (argv.length % 2 !== 0) usage();
  const allowed = new Set([
    "case", "source-repo", "variant-file", "expected-variant-sha256", "phase",
    "run-root", "artifact-dir", "agent-executable", "agent-args-file", "timeout-seconds",
    "attempt-registry-dir",
    "wave-id", "run-id", "attempt-id", "candidate-id", "construction-smoke",
    "adapter-file", "expected-adapter-sha256", "adapter-config", "expected-adapter-config-sha256",
    "adapter-integrity-receipt", "expected-adapter-integrity-sha256", "adapter-probe-source",
    "cargo-home-template", "authorization-file", "custodian-id", "resume-artifact-dir",
    "model-catalog-file", "rustup-home-template", "provider-auth-qualification",
    "operator-keychain",
  ]);
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const flag = argv[index];
    const value = argv[index + 1];
    if (!flag?.startsWith("--") || value === undefined) usage();
    const key = flag.slice(2);
    if (!allowed.has(key) || values.has(key)) usage();
    values.set(key, value);
  }
  return values;
}

function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (value && typeof value === "object") {
    return `{${Object.keys(value).sort().map((key) =>
      `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(",")}}`;
  }
  return JSON.stringify(value);
}

export function frozenFormalAgentArguments(
  lock,
  workspacePath = "<workspace>",
  modelCatalogPath = "<model-catalog>",
) {
  const enabled = new Set(lock.controlled_agent.enabled_features);
  const explicitlyDisabled = [
    "apps", "auth_elicitation", "browser_use", "browser_use_external",
    "browser_use_full_cdp_access", "computer_use", "goals", "hooks", "image_generation",
    "memories", "multi_agent", "multi_agent_v2", "plugins", "secret_auth_storage",
    "skill_search", "tool_suggest", "view_image", "workspace_dependencies",
  ];
  const featureArgs = [];
  const sequence = [
    ["disable", "apps"], ["disable", "auth_elicitation"], ["disable", "browser_use"],
    ["disable", "browser_use_external"], ["disable", "browser_use_full_cdp_access"],
    ["enable", "code_mode_host"], ["disable", "computer_use"],
    ["enable", "enable_request_compression"], ["disable", "fast_mode"],
    ["disable", "goals"], ["disable", "guardian_approval"], ["disable", "hooks"],
    ["disable", "image_generation"], ["disable", "in_app_browser"],
    ["disable", "in_app_chat"], ["disable", "in_app_dictation"], ["disable", "in_app_updates"],
    ["disable", "memories"], ["disable", "mentions_v2"], ["disable", "multi_agent"],
    ["disable", "multi_agent_v2"], ["disable", "personality"], ["disable", "plugin_sharing"],
    ["disable", "plugins"], ["disable", "recommended_plugins"],
    ["disable", "remote_compaction_v2"], ["disable", "remote_plugin"],
    ["disable", "secret_auth_storage"], ["disable", "shell_snapshot"], ["enable", "shell_tool"],
    ["disable", "skill_mcp_dependency_install"], ["disable", "skill_search"],
    ["disable", "tool_call_mcp_elicitation"], ["disable", "tool_suggest"],
    ["disable", "unbounded_connection_retries"], ["enable", "unified_exec"],
    ["disable", "view_image"], ["disable", "workspace_dependencies"],
  ];
  for (const [mode, name] of sequence) {
    if (mode === "enable" && !enabled.has(name)) fail(`environment lock does not enable ${name}`);
    if (mode === "disable" && explicitlyDisabled.includes(name) &&
        !lock.controlled_agent.disabled_capabilities.includes(name)) {
      fail(`environment lock does not disable ${name}`);
    }
    featureArgs.push(`--${mode}`, name);
  }
  return [
    "exec", "--cd", workspacePath, "--model", lock.controlled_agent.model_id,
    "--sandbox", lock.controlled_agent.sandbox_mode, "--ephemeral", "--ignore-user-config",
    "--ignore-rules", "--strict-config", ...featureArgs,
    "-c", 'cli_auth_credentials_store="keyring"',
    "-c", `model_reasoning_effort=${JSON.stringify(lock.controlled_agent.reasoning_effort)}`,
    "-c", 'model_reasoning_summary="none"', "-c", 'model_verbosity="low"',
    "-c", `service_tier=${JSON.stringify(lock.controlled_agent.service_tier)}`,
    "-c", 'web_search="disabled"', "-c", "agents.enabled=false",
    "-c", "tools.experimental_request_user_input.enabled=false",
    "-c", "tools.update_plan.enabled=false", "-c", "skills.bundled.enabled=false",
    "-c", "skills.include_instructions=false", "-c", "include_apps_instructions=false",
    "-c", "include_collaboration_mode_instructions=false", "-c", "include_environment_context=false",
    "-c", "orchestrator.skills.enabled=false", "-c", "orchestrator.mcp.enabled=false",
    "-c", `model_catalog_json=${JSON.stringify(modelCatalogPath)}`,
    "-c", `approval_policy=${JSON.stringify(lock.controlled_agent.approval_policy)}`,
    "-c", "sandbox_workspace_write.network_access=false", "-c", 'shell_environment_policy.inherit="all"',
    "-c", "shell_environment_policy.ignore_default_excludes=false",
    "-c", "shell_environment_policy.experimental_use_profile=false", "--json", "-",
  ];
}

export function requireFormalAuthorizationCommitments(authorization, commitments) {
  for (const [field, actual] of Object.entries(commitments)) {
    if (authorization[field] !== actual) fail(`formal authorization ${field} mismatch`);
  }
}

export function requireFormalTiming(caseEntry, timeoutSeconds, authorization) {
  const expectedTimeoutSeconds = caseEntry.time_limit_minutes * 60;
  if (timeoutSeconds !== expectedTimeoutSeconds) {
    fail("formal timeout-seconds must equal the exact frozen case time limit");
  }
  requireFormalAuthorizationCommitments(authorization, {
    timeout_seconds: expectedTimeoutSeconds,
    termination_grace_seconds: 10,
  });
  return {timeout_seconds: expectedTimeoutSeconds, termination_grace_seconds: 10};
}

export function requireFormalFreeSpace(preflightReceipt, lock, formal) {
  const requiredMinimum = lock.controlled_runner.minimum_free_bytes_before_each_run;
  const observed = preflightReceipt.free_space?.bytes;
  if (!Number.isSafeInteger(observed) || observed < 0) fail("preflight free-space observation is invalid");
  if (formal && observed < requiredMinimum) {
    fail(`insufficient free space for formal attempt: ${observed} bytes; need ${requiredMinimum}`);
  }
  return {
    required_minimum_bytes: requiredMinimum,
    observed_bytes: observed,
    enforced: formal,
  };
}

export function pendingResultState(formalAttemptEligible) {
  return {
    formal_result_eligible: formalAttemptEligible === true,
    formal_attempt_authorized: formalAttemptEligible === true,
    result_state: "awaiting_score_freeze",
  };
}

export async function commitTerminalEntry({
  ledgerPath,
  markerPath,
  terminal,
  onCommitted = () => {},
  markerWriter = writeFile,
}) {
  await appendFile(ledgerPath, `${JSON.stringify(terminal)}\n`, {encoding: "utf8"});
  // The append-only ledger is authoritative. Flip state before the best-effort
  // convenience marker so marker persistence can never cause a second append.
  onCommitted();
  try {
    await markerWriter(markerPath, canonicalBytes(terminal), {mode: 0o600, flag: "wx"});
    return {marker_written: true, marker_error: null};
  } catch (error) {
    return {
      marker_written: false,
      marker_error: error instanceof Error ? error.message : String(error),
    };
  }
}

function isInside(candidate, parent) {
  const path = relative(parent, candidate);
  return path === "" || (!path.startsWith("..") && !isAbsolute(path));
}

async function requireRegular(path, label) {
  if (!isAbsolute(path)) fail(`${label} must be absolute`);
  const input = resolve(path);
  const info = await lstat(input);
  if (!info.isFile() || info.isSymbolicLink()) fail(`${label} must be a non-symlink regular file`);
  return realpath(input);
}

async function requireDirectory(path, label) {
  if (!isAbsolute(path)) fail(`${label} must be absolute`);
  const input = resolve(path);
  const info = await lstat(input);
  if (!info.isDirectory() || info.isSymbolicLink()) fail(`${label} must be a real directory`);
  return realpath(input);
}

async function prospective(path, label) {
  if (!isAbsolute(path)) fail(`${label} must be absolute`);
  const absolute = resolve(path);
  if (existsSync(absolute)) fail(`${label} must not already exist`);
  const parent = await realpath(dirname(absolute));
  const info = await lstat(parent);
  if (!info.isDirectory() || info.isSymbolicLink()) fail(`${label} parent must be a real directory`);
  return resolve(parent, basename(absolute));
}

async function emptyOwnedDirectory(path) {
  for (const name of await readdir(path)) {
    await rm(resolve(path, name), {recursive: true, force: true});
  }
}

function command(path, args, options = {}) {
  const result = spawnSync(path, args, {
    cwd: options.cwd,
    env: options.env,
    input: options.input,
    encoding: options.encoding ?? "utf8",
    timeout: options.timeout,
    maxBuffer: options.maxBuffer ?? MAX_PROCESS_BYTES,
  });
  if (!options.allowFailure && (result.error || result.status !== 0)) {
    fail(
      `${path} ${args.join(" ")} failed: ` +
        (result.error?.message ?? result.stderr ?? result.stdout ?? `exit ${result.status}`),
    );
  }
  return result;
}

function isolatedGitEnvironment(environment) {
  const result = {...environment};
  for (const key of Object.keys(result)) {
    if (/^GIT_(?:DIR|WORK_TREE|INDEX_FILE|OBJECT_DIRECTORY|COMMON_DIR|CONFIG_PARAMETERS)$/.test(key) ||
        /^GIT_CONFIG_(?:KEY|VALUE)_\d+$/.test(key)) delete result[key];
  }
  return {
    ...result,
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_CONFIG_GLOBAL: "/dev/null",
    GIT_ATTR_NOSYSTEM: "1",
    GIT_ALTERNATE_OBJECT_DIRECTORIES: "",
  };
}

function git(args, cwd, environment, allowFailure = false) {
  return command("rtk", ["proxy", "git", ...GIT_CONFIGURATION, ...args], {
    cwd,
    env: isolatedGitEnvironment(environment),
    allowFailure,
    encoding: "utf8",
  });
}

async function fileIdentity(path, relativePath = undefined) {
  const bytes = await readFile(path);
  return {path: relativePath ?? path, bytes: bytes.length, sha256: sha256(bytes)};
}

async function authorityPacket(caseId, root = benchmarkDir) {
  const lock = JSON.parse(await readFile(resolve(root, "evaluator/authority-lock.json"), "utf8"));
  const entry = lock.cases?.find((candidate) => candidate.id === caseId);
  if (!entry) fail(`missing authority lock for ${caseId}`);
  return canonicalBytes({
    schema: "tachiko-review-authority-packet-v1",
    protocol_id: lock.protocol_id,
    case_id: caseId,
    base_commit: entry.base_commit,
    assignment_cutoff: entry.assignment_cutoff,
    task_authority: entry.task_authority,
    claims: entry.claims,
  });
}

async function changedCandidateReviewSources(validationWorkspace, captureReceipt) {
  const sources = [];
  for (const path of captureReceipt.changed_files ?? []) {
    const absolute = resolve(validationWorkspace, path);
    if (!isInside(absolute, validationWorkspace)) fail("captured candidate path escaped validation workspace");
    if (!existsSync(absolute)) continue; // Deletions remain visible in the patch.
    const info = await lstat(absolute);
    let bytes;
    if (info.isFile() && !info.isSymbolicLink()) bytes = await readFile(absolute);
    else if (info.isSymbolicLink()) bytes = Buffer.from(await readlink(absolute), "utf8");
    else fail(`review-visible candidate path is not a file or symlink: ${path}`);
    sources.push({path: `candidate-files/${path}`, role: "candidate_checkout", bytes});
  }
  return sources;
}

function sanitizeEnvironment(base, overrides) {
  const environment = {};
  for (const key of ["TERM", "TERM_PROGRAM", "COLORTERM"]) {
    if (base[key]) environment[key] = base[key];
  }
  return {
    ...environment,
    ...overrides,
    LANG: "C.UTF-8",
    LC_ALL: "C.UTF-8",
    TZ: "UTC",
    CARGO_INCREMENTAL: "0",
    CARGO_NET_OFFLINE: "true",
    CARGO_TERM_COLOR: "never",
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_CONFIG_GLOBAL: "/dev/null",
    GIT_ATTR_NOSYSTEM: "1",
    GIT_ALTERNATE_OBJECT_DIRECTORIES: "",
  };
}

function findExecutable(name) {
  const result = spawnSync("/usr/bin/which", [name], {encoding: "utf8", env: process.env});
  if (result.status !== 0 || !result.stdout.trim()) fail(`required executable unavailable: ${name}`);
  return result.stdout.trim();
}

async function runtimeToolIdentity(name, path, expectedSha256 = null, versionArgs = ["--version"]) {
  if (!isAbsolute(path)) fail(`formal runtime ${name} must be absolute`);
  const canonical = await requireRegular(await realpath(resolve(path)), `formal runtime ${name}`);
  const identity = await fileIdentity(canonical);
  if (expectedSha256 && identity.sha256 !== expectedSha256) {
    fail(`formal runtime ${name} SHA-256 differs from the environment lock`);
  }
  if (versionArgs && ((await lstat(canonical)).mode & 0o111)) {
    const observed = command(canonical, versionArgs, {allowFailure: true, encoding: "utf8"});
    identity.version_exit_code = observed.status;
    identity.version_stdout = String(observed.stdout ?? "").trim();
    identity.version_stderr = String(observed.stderr ?? "").trim();
    if (observed.error || observed.status !== 0) fail(`formal runtime ${name} version probe failed`);
  }
  return {name, source_path: canonical, expected_sha256: expectedSha256, ...identity};
}

export async function inspectFormalRuntime(lock, rustupHomeTemplate) {
  const rustupHome = await requireDirectory(rustupHomeTemplate, "trusted Rustup home template");
  const rustupHomeTree = await hashContentTree(rustupHome);
  const rustup = await requireRegular(findExecutable("rustup"), "formal runtime rustup");
  const rustTools = new Map();
  for (const name of ["cargo", "rustc", "rustfmt", "cargo-clippy"]) {
    rustTools.set(name, command(rustup, ["which", name], {
      encoding: "utf8",
      env: {...process.env, RUSTUP_HOME: rustupHome},
    }).stdout.trim());
  }
  const nodeSource = lock.toolchain.node.construction_source_path ?? lock.toolchain.node.path;
  const pnpmSource = lock.toolchain.pnpm.construction_source_path ?? lock.toolchain.pnpm.path;
  const pnpmHomeTree = await hashContentTree(dirname(await realpath(pnpmSource)));
  if (pnpmHomeTree.digest_kind !== lock.toolchain.pnpm.tree_digest_kind ||
      pnpmHomeTree.manifest_sha256 !== lock.toolchain.pnpm.tree_sha256 ||
      pnpmHomeTree.entries !== lock.toolchain.pnpm.tree_entries ||
      pnpmHomeTree.file_bytes !== lock.toolchain.pnpm.tree_file_bytes) {
    fail("formal pnpm runtime tree differs from the environment lock");
  }
  const specifications = [
    ["node", nodeSource, lock.toolchain.node.binary_sha256],
    ["bash", lock.toolchain.bash.path, lock.toolchain.bash.binary_sha256],
    ["git", lock.toolchain.git.path, lock.toolchain.git.binary_sha256],
    ["rtk", lock.toolchain.rtk.path, lock.toolchain.rtk.binary_sha256],
    ["pnpm", pnpmSource, lock.toolchain.pnpm.binary_sha256],
    ["rustup", rustup, null],
    ["cargo", rustTools.get("cargo"), null],
    ["rustc", rustTools.get("rustc"), null],
    ["rustfmt", rustTools.get("rustfmt"), null],
    ["cargo-clippy", rustTools.get("cargo-clippy"), null],
    ["codex-code-mode-host", lock.controlled_agent.code_mode_host_path,
      lock.controlled_agent.code_mode_host_sha256, null],
  ];
  const tools = [];
  for (const [name, path, expected, versionArgs] of specifications) {
    tools.push(await runtimeToolIdentity(name, path, expected, versionArgs === undefined ? ["--version"] : versionArgs));
  }
  tools.sort((left, right) => left.name.localeCompare(right.name));
  const byName = Object.fromEntries(tools.map((tool) => [tool.name, tool]));
  const expectedVersions = [
    ["node", `v${lock.toolchain.node.version}`],
    ["pnpm", lock.toolchain.pnpm.version],
    ["git", `git version ${lock.toolchain.git.version}`],
    ["rtk", `rtk ${lock.toolchain.rtk.version}`],
    ["cargo", `cargo ${lock.toolchain.rust_primary.cargo}`],
    ["rustc", `rustc ${lock.toolchain.rust_primary.rustc}`],
  ];
  for (const [name, expected] of expectedVersions) {
    if (byName[name].version_stdout.split("\n").at(-1) !== expected) {
      fail(`formal runtime ${name} version differs from the environment lock`);
    }
  }
  if (!byName.bash.version_stdout.startsWith(`GNU bash, version ${lock.toolchain.bash.version.match(/GNU bash ([^()]+)/)?.[1] ?? ""}`)) {
    fail("formal runtime bash version differs from the environment lock");
  }
  const identityDocument = {
    schema: "tachiko-formal-runtime-identity-v1",
    tools,
    rustup_home: rustupHomeTree,
    pnpm_home: pnpmHomeTree,
  };
  return {
    tools,
    rustup_home: rustupHomeTree,
    pnpm_home: pnpmHomeTree,
    identity_sha256: sha256(canonicalBytes(identityDocument)),
  };
}

export async function inspectFormalCargoHome(templatePath, lock) {
  const observed = await hashContentTree(templatePath);
  const expected = lock.offline_dependency_cache;
  if (observed.digest_kind !== expected.digest_kind ||
      observed.manifest_sha256 !== expected.tree_sha256 ||
      observed.entries !== expected.tree_entries ||
      observed.file_bytes !== expected.tree_file_bytes) {
    fail("formal Cargo home template differs from the frozen offline dependency cache");
  }
  return observed;
}

async function inspectFormalModelCatalog(sourcePath, catalogLock, modelId) {
  const source = await requireRegular(sourcePath, "trusted formal model catalog");
  const bytes = await readFile(source);
  if (bytes.length !== catalogLock.bytes || sha256(bytes) !== catalogLock.raw_sha256) {
    fail("formal model catalog bytes or SHA-256 differ from the environment lock");
  }
  let catalog;
  try { catalog = JSON.parse(bytes.toString("utf8")); } catch { fail("formal model catalog is not JSON"); }
  const model = catalog?.models?.find((entry) => entry.slug === modelId);
  if (!model || sha256(`${canonicalJson(catalog)}\n`) !== catalogLock.canonical_catalog_sha256 ||
      sha256(`${canonicalJson(model)}\n`) !== catalogLock.model_record_sha256 ||
      sha256(`${model.base_instructions}\n`) !== catalogLock.base_instructions_sha256) {
    fail("formal model catalog semantics differ from the environment lock");
  }
  return {...await fileIdentity(source), canonical_catalog_sha256: catalogLock.canonical_catalog_sha256,
    model_record_sha256: catalogLock.model_record_sha256,
    base_instructions_sha256: catalogLock.base_instructions_sha256};
}

export async function stageFormalModelCatalog({sourcePath, destinationPath, catalogLock, modelId}) {
  const inspected = await inspectFormalModelCatalog(sourcePath, catalogLock, modelId);
  if (!isAbsolute(destinationPath) || existsSync(destinationPath)) {
    fail("formal model catalog destination must be a new absolute path");
  }
  await mkdir(dirname(destinationPath), {recursive: true, mode: 0o700});
  await copyFile(inspected.path, destinationPath);
  await chmod(destinationPath, 0o400);
  const staged = await fileIdentity(destinationPath);
  if (staged.bytes !== inspected.bytes || staged.sha256 !== inspected.sha256) {
    fail("staged formal model catalog differs from its trusted source");
  }
  return {...staged, source: inspected};
}

export async function stageToolBin(toolBin, formalRuntime = null) {
  await mkdir(toolBin, {mode: 0o700});
  if (formalRuntime) {
    const stagedRustupHome = resolve(dirname(toolBin), "rustup-home");
    const stagedPnpmHome = resolve(dirname(toolBin), "pnpm-home");
    await cloneTreeCopyOnWrite(formalRuntime.rustup_home.path, stagedRustupHome);
    await cloneTreeCopyOnWrite(formalRuntime.pnpm_home.path, stagedPnpmHome);
    const stagedRustupTree = await hashContentTree(stagedRustupHome);
    for (const key of ["digest_kind", "entries", "file_bytes", "manifest_sha256"]) {
      if (stagedRustupTree[key] !== formalRuntime.rustup_home[key]) {
        fail(`staged Rustup home ${key} differs from its preregistered template`);
      }
    }
    const stagedPnpmTree = await hashContentTree(stagedPnpmHome);
    for (const key of ["digest_kind", "entries", "file_bytes", "manifest_sha256"]) {
      if (stagedPnpmTree[key] !== formalRuntime.pnpm_home[key]) {
        fail(`staged pnpm home ${key} differs from its preregistered template`);
      }
    }
    const staged = [];
    for (const tool of formalRuntime.tools) {
      const launcher = resolve(toolBin, tool.name);
      const isRustComponent = ["cargo", "rustc", "rustfmt", "cargo-clippy"].includes(tool.name);
      const isPnpm = tool.name === "pnpm";
      let executionPath;
      if (isRustComponent) {
        if (!isInside(tool.source_path, formalRuntime.rustup_home.path)) {
          fail(`formal runtime ${tool.name} escaped the Rustup home template`);
        }
        executionPath = resolve(stagedRustupHome, relative(formalRuntime.rustup_home.path, tool.source_path));
        await symlink(executionPath, launcher);
        executionPath = await realpath(executionPath);
      } else if (isPnpm) {
        if (!isInside(tool.source_path, formalRuntime.pnpm_home.path)) {
          fail("formal pnpm executable escaped the pnpm runtime template");
        }
        executionPath = resolve(stagedPnpmHome, relative(formalRuntime.pnpm_home.path, tool.source_path));
        await symlink(executionPath, launcher);
        executionPath = await realpath(executionPath);
      } else {
        await copyFile(tool.source_path, launcher, 0);
        await chmod(launcher, 0o500);
        executionPath = await realpath(launcher);
      }
      const identity = await fileIdentity(executionPath);
      if (identity.bytes !== tool.bytes || identity.sha256 !== tool.sha256) {
        fail(`staged formal runtime ${tool.name} differs from its preregistered identity`);
      }
      staged.push({...tool, launcher_path: launcher, staged_path: executionPath});
    }
    return {
      tools: staged,
      rustup_home_path: stagedRustupHome,
      rustup_home: stagedRustupTree,
      pnpm_home_path: stagedPnpmHome,
      pnpm_home: stagedPnpmTree,
      identity_sha256: formalRuntime.identity_sha256,
    };
  }
  const candidates = new Map([
    ["node", process.execPath],
    ["bash", findExecutable("bash")],
    ["git", findExecutable("git")],
    ["rtk", findExecutable("rtk")],
    ["rustup", findExecutable("rustup")],
  ]);
  const rustup = candidates.get("rustup");
  for (const name of ["cargo", "rustc", "rustfmt", "cargo-clippy"]) {
    const resolved = command(rustup, ["which", name], {encoding: "utf8"}).stdout.trim();
    candidates.set(name, resolved);
  }
  for (const optional of ["pnpm", "jq", "wasm-bindgen", "wasm-pack"]) {
    try { candidates.set(optional, findExecutable(optional)); } catch { /* optional */ }
  }
  const tools = [];
  for (const [name, input] of candidates) {
    const canonical = await realpath(input);
    const bytes = await readFile(canonical);
    const link = resolve(toolBin, name);
    await symlink(canonical, link);
    tools.push({name, path: canonical, staged_path: link, bytes: bytes.length, sha256: sha256(bytes)});
  }
  tools.sort((left, right) => left.name.localeCompare(right.name));
  return tools;
}

export async function comparePreflightToolIdentities(tools, preflightReceipt, formal = false) {
  const mappings = [
    ["bash", "bash", true],
    ["cargo", "cargo", true],
    ["clippy", "cargo-clippy", true],
    ["git", "git", true],
    ["node", "node", true],
    ["rtk", "rtk", true],
    ["rustc", "rustc", true],
    ["rustfmt", "rustfmt", true],
    ["rustup", "rustup", true],
  ];
  const comparedNames = [];
  for (const [observedName, toolName, stagedWhenFormal] of mappings) {
    const observed = preflightReceipt.binaries?.[observedName];
    const expected = tools.find((tool) => tool.name === toolName);
    if (!observed || !expected) fail(`preflight omitted required runtime tool ${observedName}`);
    const expectedPathInput = formal && stagedWhenFormal
      ? expected.staged_path
      : (expected.source_path ?? expected.path);
    const expectedPath = await realpath(expectedPathInput);
    if (observed.path !== expectedPath || observed.bytes !== expected.bytes ||
        observed.sha256 !== expected.sha256) {
      fail(`preflight runtime identity differs for ${observedName}`);
    }
    comparedNames.push(observedName);
  }
  return {all_required_matched: true, compared_names: comparedNames};
}

export async function verifyStagedRuntimeArtifacts(stagedRuntime, lock) {
  const environment = {...process.env, RUSTUP_HOME: stagedRuntime.rustup_home_path};
  const observations = [];
  for (const tool of stagedRuntime.tools) {
    const identity = await fileIdentity(await realpath(tool.staged_path));
    if (identity.bytes !== tool.bytes || identity.sha256 !== tool.sha256) {
      fail(`formal staged-runtime preflight identity differs for ${tool.name}`);
    }
    let version = null;
    if (tool.version_stdout !== undefined) {
      const observed = command(tool.staged_path, ["--version"], {
        env: environment,
        encoding: "utf8",
        allowFailure: true,
      });
      if (observed.error || observed.status !== 0 || String(observed.stdout).trim() !== tool.version_stdout) {
        fail(`formal staged-runtime execution probe failed for ${tool.name}`);
      }
      version = String(observed.stdout).trim();
    }
    observations.push({name: tool.name, ...identity, version});
  }
  const rustup = stagedRuntime.tools.find((tool) => tool.name === "rustup").staged_path;
  const stagedRustc = stagedRuntime.tools.find((tool) => tool.name === "rustc").staged_path;
  const rustupWhich = command(rustup, ["which", "rustc"], {env: environment, encoding: "utf8"}).stdout.trim();
  if (await realpath(rustupWhich) !== await realpath(stagedRustc)) {
    fail("formal staged Rustup resolved rustc outside the staged toolchain");
  }
  const compatibility = command(rustup, ["run", "1.85.0", "rustc", "--version"], {
    env: environment,
    encoding: "utf8",
  }).stdout.trim();
  if (compatibility !== `rustc ${lock.toolchain.rust_compatibility.rustc}`) {
    fail("formal staged Rust compatibility toolchain differs from the environment lock");
  }
  const targets = command(rustup, [
    "target", "list", "--installed", "--toolchain", "1.97.1",
  ], {env: environment, encoding: "utf8"}).stdout.trim().split("\n");
  for (const target of lock.toolchain.rust_primary.targets) {
    if (!targets.includes(target)) fail(`formal staged Rust target is missing: ${target}`);
  }
  return {
    schema: "tachiko-formal-staged-runtime-preflight-v1",
    all_staged_artifacts_verified: true,
    rustup_home_path: stagedRuntime.rustup_home_path,
    rustup_home: stagedRuntime.rustup_home,
    pnpm_home_path: stagedRuntime.pnpm_home_path,
    pnpm_home: stagedRuntime.pnpm_home,
    tools: observations,
  };
}

async function copyControls(artifactDir, sourceBenchmarkDir = benchmarkDir) {
  const artifacts = [];
  for (const path of CONTROL_ARTIFACTS) {
    const source = resolve(sourceBenchmarkDir, path);
    const destination = resolve(artifactDir, path);
    await mkdir(dirname(destination), {recursive: true, mode: 0o700});
    await copyFile(source, destination);
    await chmod(destination, 0o600);
    artifacts.push(await fileIdentity(destination, path));
  }
  return {artifacts, sha256: sha256(Buffer.from(`${JSON.stringify(artifacts)}\n`, "utf8"))};
}

async function hashInfrastructureTree(root, seal = false) {
  const entries = [];
  async function requireMode(path, info, expected) {
    const observed = Number(info.mode & 0o777);
    if (!seal && observed !== expected) {
      fail(`controller bundle mode changed: ${path} (${observed.toString(8)} != ${expected.toString(8)})`);
    }
  }
  async function walk(directory, prefix = "") {
    const names = await readdir(directory);
    names.sort((left, right) => Buffer.from(left).compare(Buffer.from(right)));
    for (const name of names) {
      const path = prefix ? `${prefix}/${name}` : name;
      const absolute = resolve(directory, name);
      const info = await lstat(absolute);
      if (info.isSymbolicLink()) fail(`controller bundle contains a symlink: ${path}`);
      if (info.isDirectory()) {
        const mode = 0o700;
        entries.push({path, type: "directory", mode});
        await walk(absolute, path);
        await requireMode(path, info, mode);
        if (seal) await chmod(absolute, mode);
      } else if (info.isFile()) {
        const bytes = await readFile(absolute);
        const mode = path === "scripts/process-coalition-control" ? 0o500 : 0o400;
        entries.push({path, type: "file", mode, bytes: bytes.length, sha256: sha256(bytes)});
        await requireMode(path, info, mode);
        if (seal) await chmod(absolute, mode);
      } else fail(`controller bundle contains an unsupported node: ${path}`);
    }
  }
  const rootInfo = await lstat(root);
  if (!rootInfo.isDirectory() || rootInfo.isSymbolicLink()) {
    fail("controller bundle root is not a regular directory");
  }
  entries.push({path: ".", type: "directory", mode: 0o700});
  await walk(root);
  await requireMode(".", rootInfo, 0o700);
  if (seal) await chmod(root, 0o700);
  const manifestBytes = canonicalBytes({schema: "tachiko-controller-bundle-manifest-v1", entries});
  return {entries: entries.length, bytes: manifestBytes, sha256: sha256(manifestBytes)};
}

async function assertControllerBundleIntact({
  root, manifestPath, expectedSha256, expectedEntries, label,
}) {
  let observed;
  let manifestIdentity;
  try {
    [observed, manifestIdentity] = await Promise.all([
      hashInfrastructureTree(root),
      fileIdentity(manifestPath),
    ]);
  } catch (error) {
    fail(`controller executable bundle changed ${label}: ${error instanceof Error ? error.message : String(error)}`);
  }
  if (observed.sha256 !== expectedSha256 || manifestIdentity.sha256 !== expectedSha256 ||
      (expectedEntries !== undefined && observed.entries !== expectedEntries)) {
    fail(`controller executable bundle changed ${label}`);
  }
  return {
    schema: "tachiko-controller-bundle-verification-v1",
    label,
    entries: observed.entries,
    tree_sha256: observed.sha256,
    manifest: manifestIdentity,
    verified: true,
  };
}

function quoteSandboxPath(path) {
  return path.replaceAll("\\", "\\\\").replaceAll('"', '\\"');
}

export function candidateAccessProfile({
  protectedRoots = [], protectedPaths = [], restrictedRoots = [],
  allowedReadRoots = [], allowedWriteRoots = [], writeProtectedRoots = [],
  writeProtectedPaths = [], baseProfile = PROCESS_CONTAINMENT_PROFILE,
}) {
  const roots = [...new Set(protectedRoots.map((path) => resolve(path)))].sort();
  const paths = [...new Set(protectedPaths.map((path) => resolve(path)))].sort();
  const restricted = [...new Set(restrictedRoots.map((path) => resolve(path)))].sort();
  const readable = [...new Set(allowedReadRoots.map((path) => resolve(path)))].sort();
  const writable = [...new Set(allowedWriteRoots.map((path) => resolve(path)))].sort();
  const writeDeniedRoots = [...new Set(writeProtectedRoots.map((path) => resolve(path)))].sort();
  const writeDeniedPaths = [...new Set(writeProtectedPaths.map((path) => resolve(path)))].sort();
  const policy = `${baseProfile}${restricted.map((path) =>
    `(deny file-read* (literal "${quoteSandboxPath(path)}"))\n` +
    `(deny file-read* (subpath "${quoteSandboxPath(path)}"))\n` +
    `(deny file-write* (literal "${quoteSandboxPath(path)}"))\n` +
    `(deny file-write* (subpath "${quoteSandboxPath(path)}"))\n`).join("")}${readable.map((path) =>
    `(allow file-read-metadata (literal "${quoteSandboxPath(dirname(path))}"))\n` +
    `(allow file-read* (literal "${quoteSandboxPath(path)}"))\n` +
    `(allow file-read* (subpath "${quoteSandboxPath(path)}"))\n`).join("")}${writable.map((path) =>
    `(allow file-write* (literal "${quoteSandboxPath(path)}"))\n` +
    `(allow file-write* (subpath "${quoteSandboxPath(path)}"))\n`).join("")}${roots.map((path) =>
    `(deny file-read* (literal "${quoteSandboxPath(path)}"))\n` +
    `(deny file-read* (subpath "${quoteSandboxPath(path)}"))\n` +
    `(deny file-write* (literal "${quoteSandboxPath(path)}"))\n` +
    `(deny file-write* (subpath "${quoteSandboxPath(path)}"))\n`).join("")}${paths.map((path) =>
    `(deny file-read* (literal "${quoteSandboxPath(path)}"))\n` +
    `(deny file-write* (literal "${quoteSandboxPath(path)}"))\n`).join("")}${writeDeniedRoots.map((path) =>
    `(deny file-write* (literal "${quoteSandboxPath(path)}"))\n` +
    `(deny file-write* (subpath "${quoteSandboxPath(path)}"))\n`).join("")}${writeDeniedPaths.map((path) =>
    `(deny file-write* (literal "${quoteSandboxPath(path)}"))\n`).join("")}`;
  return {
    schema: "tachiko-candidate-access-profile-v1",
    protected_roots: roots,
    protected_paths: paths,
    restricted_roots: restricted,
    allowed_read_roots: readable,
    allowed_write_roots: writable,
    write_protected_roots: writeDeniedRoots,
    write_protected_paths: writeDeniedPaths,
    profile: policy,
    profile_sha256: sha256(policy),
  };
}

function trustedHelperWriteProfile({protectedRoots = [], protectedPaths = []}) {
  const roots = [...new Set(protectedRoots.map((path) => resolve(path)))].sort();
  const paths = [...new Set(protectedPaths.map((path) => resolve(path)))].sort();
  const profile = `(version 1)\n(allow default)\n${roots.map((path) =>
    `(deny file-write* (literal "${quoteSandboxPath(path)}"))\n` +
    `(deny file-write* (subpath "${quoteSandboxPath(path)}"))\n`).join("")}${paths.map((path) =>
    `(deny file-write* (literal "${quoteSandboxPath(path)}"))\n`).join("")}`;
  return {roots, paths, profile, profile_sha256: sha256(profile)};
}

async function fileSha256(path) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest("hex");
}

async function hashContentTree(root) {
  const canonicalRoot = await requireDirectory(root, "content-addressed tree");
  const entries = [];
  let fileBytes = 0;
  async function walk(directory) {
    const names = await readdir(directory);
    names.sort();
    for (const name of names) {
      const absolute = resolve(directory, name);
      const path = relative(canonicalRoot, absolute);
      const info = await lstat(absolute);
      if (info.isDirectory()) {
        entries.push({path, type: "directory"});
        await walk(absolute);
      } else if (info.isSymbolicLink()) {
        entries.push({path, type: "symlink", target: await readlink(absolute)});
      } else if (info.isFile()) {
        entries.push({path, type: "file", bytes: info.size, sha256: await fileSha256(absolute)});
        fileBytes += info.size;
      } else fail(`unsupported content-addressed tree entry: ${absolute}`);
    }
  }
  await walk(canonicalRoot);
  const manifest = Buffer.from(`${JSON.stringify(entries)}\n`, "utf8");
  return {
    path: canonicalRoot,
    digest_kind: "paths-types-content-v1",
    entries: entries.length,
    file_bytes: fileBytes,
    manifest_sha256: sha256(manifest),
  };
}

async function cloneTreeCopyOnWrite(source, destination) {
  if (existsSync(destination)) fail("copy-on-write clone destination already exists");
  await mkdir(dirname(destination), {recursive: true, mode: 0o700});
  const result = command("/bin/cp", ["-cR", source, destination], {allowFailure: true});
  if (result.error || result.status !== 0) {
    fail(`copy-on-write tree clone failed: ${result.stderr || result.stdout}`);
  }
  await makeTreeOwnerAccessible(destination);
}

async function makeTreeOwnerAccessible(root) {
  async function walk(path) {
    const info = await lstat(path);
    if (info.isSymbolicLink()) return;
    if (info.isDirectory()) {
      await chmod(path, info.mode | 0o700);
      for (const name of await readdir(path)) await walk(resolve(path, name));
    } else if (info.isFile()) {
      await chmod(path, info.mode | 0o600);
    } else fail(`unsupported staged runtime node: ${path}`);
  }
  await walk(root);
}

async function sealReadOnlyTree(root) {
  async function walk(path) {
    const info = await lstat(path);
    if (info.isSymbolicLink()) fail(`trusted validation tree contains a symlink: ${path}`);
    if (info.isDirectory()) {
      for (const name of await readdir(path)) await walk(resolve(path, name));
      await chmod(path, 0o500);
    } else if (info.isFile()) {
      await chmod(path, (info.mode & 0o111) === 0 ? 0o400 : 0o500);
    } else fail(`trusted validation tree contains an unsupported node: ${path}`);
  }
  await walk(root);
}

async function assertEmptyDirectory(path, label) {
  const info = await lstat(path);
  if (!info.isDirectory() || info.isSymbolicLink() || (await readdir(path)).length !== 0) {
    fail(`${label} is not a fresh empty directory`);
  }
}

async function assertReadOnlyTree(root, label) {
  async function walk(path) {
    const info = await lstat(path);
    if (info.isSymbolicLink()) fail(`${label} contains a symlink: ${path}`);
    if ((info.mode & 0o222) !== 0) fail(`${label} contains a writable node: ${path}`);
    if (info.isDirectory()) {
      for (const name of await readdir(path)) await walk(resolve(path, name));
    } else if (!info.isFile()) fail(`${label} contains an unsupported node: ${path}`);
  }
  await walk(root);
}

export async function prepareBaseWorkspace(sourceRepo, baseCommit, baseTree, targetCommit, workspace, trustedDir, environment) {
  await mkdir(trustedDir, {mode: 0o700});
  const bare = resolve(trustedDir, "source.git");
  const bundle = resolve(trustedDir, "base.bundle");
  git(["clone", "--bare", "--no-local", "--no-hardlinks", sourceRepo, bare], trustedDir, environment);
  git([`--git-dir=${bare}`, "update-ref", "refs/heads/control", baseCommit], trustedDir, environment);
  git([`--git-dir=${bare}`, "bundle", "create", bundle, "refs/heads/control"], trustedDir, environment);
  git([`--git-dir=${bare}`, "bundle", "verify", bundle], trustedDir, environment);
  git(["clone", "--branch", "control", bundle, workspace], dirname(workspace), environment);
  git(["remote", "remove", "origin"], workspace, environment);
  const head = git(["rev-parse", "HEAD^{commit}"], workspace, environment).stdout.trim();
  const tree = git(["rev-parse", "HEAD^{tree}"], workspace, environment).stdout.trim();
  if (head !== baseCommit || tree !== baseTree) fail("base-control workspace identity mismatch");
  if (git(["cat-file", "-e", `${targetCommit}^{commit}`], workspace, environment, true).status === 0) {
    fail("ground-truth commit leaked into base-control workspace");
  }
  if (existsSync(resolve(workspace, "AGENTS.md"))) fail("base-control workspace exposes AGENTS.md");
  return {workspace, historical_base_commit: head, historical_base_tree: tree, ground_truth_commit_absent: true};
}

function overlayIdentity(stat, bytes) {
  return {
    schema: "tachiko-agents-overlay-identity-v1",
    path: "AGENTS.md",
    type: "regular",
    device: stat.dev.toString(),
    inode: stat.ino.toString(),
    uid: stat.uid.toString(),
    gid: stat.gid.toString(),
    mode: Number(stat.mode & 0o7777n),
    bytes: bytes.length,
    sha256: sha256(bytes),
  };
}

function parseFinalMessage(stdout) {
  let final = null;
  for (const line of stdout.toString("utf8").split(/\r?\n/)) {
    if (!line.trim()) continue;
    let event;
    try { event = JSON.parse(line); } catch { continue; }
    if (event?.type === "item.completed" && event.item?.type === "agent_message" &&
        typeof event.item.text === "string") final = event.item.text;
    else if (typeof event?.final_message === "string") final = event.final_message;
    else if (event?.type === "turn.completed" && typeof event.final_response === "string") {
      final = event.final_response;
    }
  }
  return final ?? "";
}

async function runAgentOnce(
  executable,
  args,
  cwd,
  environment,
  taskBytes,
  timeoutMilliseconds,
  terminationGraceMilliseconds,
  kernelContainmentProfile,
) {
  return runProcessGroupOnce({
    executable,
    args,
    cwd,
    environment,
    input: taskBytes,
    timeoutMilliseconds,
    terminationGraceMilliseconds,
    maxOutputBytes: MAX_PROCESS_BYTES,
    kernelContainmentProfile,
  });
}

export async function runCoreValidation(
  caseId,
  candidateRoot,
  outputDir,
  environment,
  constructionSmoke,
  common,
  controllerBenchmarkDir,
  trustedShell = "/bin/bash",
  kernelContainmentProfile,
) {
  const lock = JSON.parse(await readFile(resolve(controllerBenchmarkDir, "evaluator/core-score-lock.json"), "utf8"));
  const entry = lock.cases.find((candidate) => candidate.id === caseId);
  if (!entry) fail(`missing core-score lock for ${caseId}`);
  await mkdir(outputDir, {mode: 0o700});
  const commands = [];
  let allPassed = true;
  const networkEnforcement = constructionSmoke
    ? {mode: "construction_smoke_not_executed", probe_denied: null}
    : await probeNetworkSandbox({nodeExecutable: process.execPath, environment});
  for (const [index, spec] of entry.validation_checks.entries()) {
    if (constructionSmoke) {
      commands.push({id: spec.id, command: spec.command, execution: "construction_smoke_not_executed", exit_code: null});
      continue;
    }
    const commandTmp = await mkdtemp(resolve(environment.TMPDIR, "command-"));
    await chmod(commandTmp, 0o700);
    await assertEmptyDirectory(commandTmp, `core command ${spec.id} TMP`);
    const commandEnvironment = {
      ...environment,
      TMPDIR: commandTmp,
      TMP: commandTmp,
      TEMP: commandTmp,
    };
    const result = await runNetworkSandboxed({
      executable: trustedShell,
      args: ["--noprofile", "--norc", "-c", spec.command],
      cwd: candidateRoot,
      environment: commandEnvironment,
      timeoutMilliseconds: 1_800_000,
      terminationGraceMilliseconds: 10_000,
      maxOutputBytes: MAX_PROCESS_BYTES,
      profile: kernelContainmentProfile,
    });
    const stdout = result.stdout;
    const stderr = result.stderr;
    const finalTmpIdentity = await hashContentTree(commandTmp);
    await rm(commandTmp, {recursive: true, force: false});
    const stdoutPath = resolve(outputDir, `${String(index).padStart(2, "0")}.stdout`);
    const stderrPath = resolve(outputDir, `${String(index).padStart(2, "0")}.stderr`);
    await Promise.all([writeFile(stdoutPath, stdout), writeFile(stderrPath, stderr)]);
    const passed = result.exit_code === 0 && !result.spawn_error && !result.timed_out &&
      result.process_group_extinct_before_capture;
    allPassed &&= passed;
    commands.push({
      id: spec.id,
      command: spec.command,
      started_at: result.started_at,
      completed_at: result.completed_at,
      duration_seconds: result.duration_seconds,
      deadline_seconds: 1800,
      exit_code: result.exit_code,
      signal: result.signal,
      spawn_error: result.spawn_error,
      timed_out: result.timed_out,
      process_group_created: result.process_group_created,
      termination_grace_seconds: result.termination_grace_seconds,
      termination_grace_intervals: result.termination_grace_intervals,
      termination_deadline_reused_for_cleanup: result.termination_deadline_reused_for_cleanup,
      termination_signal_sent: result.termination_signal_sent,
      kill_signal_sent: result.kill_signal_sent,
      signal_actions: result.signal_actions,
      descendant_cleanup_required: result.descendant_cleanup_required,
      process_group_extinct_before_capture: result.process_group_extinct_before_capture,
      process_containment: result.process_containment,
      network_sandbox: result.network_sandbox,
      temporary_root: {
        path: commandTmp,
        initial_entries: 0,
        initial_manifest_sha256: sha256("[]\n"),
        final_entries: finalTmpIdentity.entries,
        final_manifest_sha256: finalTmpIdentity.manifest_sha256,
        inspected_after_process_extinction: result.process_group_extinct_before_capture,
        removed_before_next_command: !existsSync(commandTmp),
      },
      stdout: await fileIdentity(stdoutPath),
      stderr: await fileIdentity(stderrPath),
    });
  }
  const receipt = {
    schema: "tachiko-controller-core-validation-v1",
    ...common,
    construction_smoke: constructionSmoke,
    commands_executed: !constructionSmoke,
    candidate_access_profile_sha256: kernelContainmentProfile
      ? sha256(kernelContainmentProfile)
      : null,
    network_enforcement: networkEnforcement,
    commands,
    all_commands_passed: allPassed,
  };
  const receiptPath = resolve(outputDir, "receipt.json");
  await writeFile(receiptPath, canonicalBytes(receipt), {mode: 0o600, flag: "wx"});
  return {receipt, receiptPath};
}

async function resumeWithAdapter(args) {
  const allowed = new Set([
    "resume-artifact-dir", "adapter-file", "expected-adapter-sha256", "adapter-config",
    "expected-adapter-config-sha256", "adapter-integrity-receipt",
    "expected-adapter-integrity-sha256", "adapter-probe-source", "custodian-id",
  ]);
  for (const key of args.keys()) if (!allowed.has(key)) fail(`--${key} is not valid while resuming`);
  if (!args.has("resume-artifact-dir")) usage();
  const artifactDir = await requireDirectory(args.get("resume-artifact-dir"), "resume artifact directory");
  const statePath = await requireRegular(
    resolve(artifactDir, "awaiting-trusted-adapter.json"),
    "adapter pause receipt",
  );
  if (existsSync(resolve(artifactDir, "terminal.json"))) fail("attempt is already terminal");
  const stateBytes = await readFile(statePath);
  const state = JSON.parse(stateBytes.toString("utf8"));
  if (state.schema !== "tachiko-controller-adapter-pause-v1" ||
      state.disposition !== "awaiting_trusted_adapter" || state.launch_count !== 1 ||
      state.resampling_performed !== false || state.artifact_dir !== artifactDir) {
    fail("invalid adapter pause receipt");
  }
  const commonKeys = [
    "protocol_id", "phase", "classification", "formal_result_eligible", "wave_id", "run_id",
    "attempt_id", "candidate_id", "case_id", "control_sha256", "formal_authorization",
    "environment_identity_sha256", "attempt_registry_entry", "infrastructure_identity_sha256",
    "formal_runtime_identity_sha256", "staged_model_catalog", "effective_agent_args_sha256",
    "provider_auth_qualification_sha256",
  ];
  const common = Object.fromEntries(commonKeys.map((key) => [key, state[key]]));
  if (![common.wave_id, common.run_id, common.attempt_id, common.candidate_id].every((id) => ID.test(id ?? "")) ||
      !/^TW-0[1-9]$/.test(common.case_id ?? "") || !SHA256.test(common.control_sha256 ?? "") ||
      !SHA256.test(common.environment_identity_sha256 ?? "")) fail("adapter pause binding is invalid");
  const registryPath = await requireRegular(common.attempt_registry_entry?.path, "attempt registry entry");
  const registryBytes = await readFile(registryPath);
  if (registryBytes.length !== common.attempt_registry_entry.bytes ||
      sha256(registryBytes) !== common.attempt_registry_entry.sha256 || isInside(registryPath, artifactDir)) {
    fail("attempt registry entry changed before adapter resume");
  }

  const controlArtifacts = [];
  for (const path of CONTROL_ARTIFACTS) {
    controlArtifacts.push(await fileIdentity(resolve(artifactDir, path), path));
  }
  const controlSha256 = sha256(Buffer.from(`${JSON.stringify(controlArtifacts)}\n`, "utf8"));
  if (controlSha256 !== common.control_sha256) fail("frozen controls changed before adapter resume");
  const controllerBenchmarkDir = await requireDirectory(
    state.controller_benchmark_dir,
    "controller benchmark bundle",
  );
  if (!isInside(controllerBenchmarkDir, artifactDir)) fail("controller benchmark bundle escaped artifacts");
  const infrastructureManifestPath = await requireRegular(
    resolve(artifactDir, "controller-bundle-manifest.json"),
    "controller bundle manifest",
  );
  async function verifyControllerBundle(label) {
    return assertControllerBundleIntact({
      root: controllerBenchmarkDir,
      manifestPath: infrastructureManifestPath,
      expectedSha256: state.controller_bundle_sha256,
      label,
    });
  }
  const infrastructure = await hashInfrastructureTree(controllerBenchmarkDir);
  if (infrastructure.sha256 !== state.controller_bundle_sha256 ||
      infrastructure.sha256 !== common.infrastructure_identity_sha256) {
    fail("controller executable bundle changed before adapter resume");
  }
  await verifyControllerBundle("before loading adapter-resume controller modules");
  const {validateFormalAdapterPackage: validateBundledFormalAdapterPackage} = await import(
    pathToFileURL(resolve(controllerBenchmarkDir, "scripts/adapter-integrity.mjs")).href
  );
  const loadedControllerContext = await loadControllerContext({
    path: state.controller_context_path,
    expectedSha256: state.controller_context_sha256,
    issuancePath: state.controller_issuance_path,
    expectedIssuanceSha256: state.controller_issuance_sha256,
    authorizationPath: common.formal_result_eligible ? common.formal_authorization?.path : undefined,
    expectedAuthorizationSha256: common.formal_result_eligible
      ? common.formal_authorization?.sha256 : undefined,
    registryPath: common.formal_result_eligible ? common.attempt_registry_entry?.path : undefined,
    expectedRegistrySha256: common.formal_result_eligible
      ? common.attempt_registry_entry?.sha256 : undefined,
    required: common.formal_result_eligible,
  });
  const controllerContextPath = loadedControllerContext.context_path;
  const controllerContext = loadedControllerContext.context;
  const controllerIssuancePath = loadedControllerContext.issuance_path;
  const controllerIssuanceSha256 = loadedControllerContext.issuance_sha256;
  requireResumeContextBindings(common, controllerContext, state);
  const adapterTmp = await requireDirectory(
    controllerContext.adapter_write_allowed_roots?.[0],
    "fresh adapter TMP",
  );
  if (controllerContext.adapter_write_allowed_roots.length !== 1 ||
      controllerContext.adapter_tmp_initial_sha256 !== sha256("[]\n") ||
      (await readdir(adapterTmp)).length !== 0) {
    fail("fresh adapter TMP changed before adapter resume");
  }
  const environmentReceipt = JSON.parse(
    await readFile(resolve(artifactDir, "environment-receipt.json"), "utf8"),
  );
  if (environmentReceipt.environment_identity_sha256 !== common.environment_identity_sha256) {
    fail("environment identity changed before adapter resume");
  }
  const validationEnvironmentReceipt = JSON.parse(await readFile(
    state.validation_environment_receipt_path,
    "utf8",
  ));
  if (validationEnvironmentReceipt.schema !==
        "tachiko-controller-trusted-validation-environment-v1" ||
      validationEnvironmentReceipt.candidate_environment_identity_sha256 !==
        common.environment_identity_sha256 ||
      validationEnvironmentReceipt.agent_environment_inherited !== false ||
      validationEnvironmentReceipt.created_after_agent_extinction !== true ||
      validationEnvironmentReceipt.cargo_home_read_only !== true) {
    fail("trusted validation environment receipt is invalid before adapter resume");
  }
  const environment = validationEnvironmentReceipt.environment;
  const validationEnvironmentRoot = await requireDirectory(
    state.validation_environment_root,
    "trusted validation environment root",
  );
  if (!isInside(validationEnvironmentRoot, state.run_root)) {
    fail("trusted validation environment root escaped the neutral run root");
  }
  for (const key of ["HOME", "CODEX_HOME", "TMPDIR", "CARGO_HOME"]) {
    const path = await requireDirectory(environment[key], `trusted validation ${key}`);
    if (!isInside(path, validationEnvironmentRoot) ||
        environmentReceipt.environment[key] === environment[key]) {
      fail(`trusted validation ${key} crosses the candidate environment boundary`);
    }
  }
  const observedValidationCargo = await hashContentTree(environment.CARGO_HOME);
  for (const key of ["digest_kind", "entries", "file_bytes", "manifest_sha256"]) {
    if (observedValidationCargo[key] !== validationEnvironmentReceipt.cargo_home[key]) {
      fail(`trusted validation Cargo home ${key} changed before adapter resume`);
    }
  }
  await Promise.all([
    assertEmptyDirectory(environment.HOME, "trusted validation HOME before resume"),
    assertEmptyDirectory(environment.CODEX_HOME, "trusted validation CODEX_HOME before resume"),
    assertEmptyDirectory(environment.TMPDIR, "trusted validation base TMP before resume"),
  ]);
  async function freshResumeValidationEnvironment(label) {
    const path = await mkdtemp(resolve(validationEnvironmentRoot, `${label}-tmp-`));
    await chmod(path, 0o700);
    await assertEmptyDirectory(path, `trusted resumed ${label} TMP`);
    return {...environment, TMPDIR: path, TMP: path, TEMP: path};
  }
  async function verifyResumeValidationGuard(label) {
    const receiptIdentity = await fileIdentity(state.validation_environment_receipt_path);
    if (receiptIdentity.sha256 !== state.bound_receipts?.validation_environment_sha256) {
      fail(`trusted validation environment receipt changed ${label}`);
    }
    const cargoIdentity = await hashContentTree(environment.CARGO_HOME);
    for (const key of ["digest_kind", "entries", "file_bytes", "manifest_sha256"]) {
      if (cargoIdentity[key] !== validationEnvironmentReceipt.cargo_home[key]) {
        fail(`trusted validation Cargo home ${key} changed ${label}`);
      }
    }
    await Promise.all([
      assertReadOnlyTree(environment.CARGO_HOME, "trusted validation Cargo home"),
      assertEmptyDirectory(environment.HOME, "trusted validation HOME"),
      assertEmptyDirectory(environment.CODEX_HOME, "trusted validation CODEX_HOME"),
    ]);
    return {
      label,
      receipt_sha256: receiptIdentity.sha256,
      cargo_manifest_sha256: cargoIdentity.manifest_sha256,
      verified: true,
    };
  }
  await verifyResumeValidationGuard("before adapter resume inputs");
  let adapter;
  let adapterBytes;
  let adapterConfig = null;
  let formalAdapterPackage = null;
  const preflightReceipt = JSON.parse(await readFile(state.preflight_receipt_path, "utf8"));
  if (common.formal_result_eligible) {
    const scaffold = resolve(controllerBenchmarkDir, "evaluator/adapters/candidate-adapter.mjs");
    await verifyResumeValidationGuard("before resumed formal adapter validation/build");
    formalAdapterPackage = await validateBundledFormalAdapterPackage({
      adapterPath: args.get("adapter-file") ?? scaffold,
      configPath: args.get("adapter-config"),
      integrityReceiptPath: args.get("adapter-integrity-receipt"),
      expectedIntegrityReceiptSha256: args.get("expected-adapter-integrity-sha256"),
      benchmarkRoot: controllerBenchmarkDir,
      forbiddenRoots: [
        state.source_repo, artifactDir, controllerBenchmarkDir, state.validation_workspace,
        state.run_root, state.original_candidate_workspace,
      ].filter(Boolean),
      context: controllerContext,
      candidateRoot: state.validation_workspace,
      probeSourcePath: args.get("adapter-probe-source"),
      buildRoot: resolve(artifactDir, "formal-tw09-probe-build"),
      cargoPath: preflightReceipt.binaries.cargo.path,
      cargoSha256: preflightReceipt.binaries.cargo.sha256,
      rustcPath: preflightReceipt.binaries.rustc.path,
      rustcSha256: preflightReceipt.binaries.rustc.sha256,
      candidateManifestPath: resolve(state.capture_dir, "raw-manifest.json"),
      environment,
    });
    await verifyResumeValidationGuard("after resumed formal adapter validation/build");
    adapter = formalAdapterPackage.scaffold.path;
    adapterBytes = await readFile(adapter);
    adapterConfig = formalAdapterPackage.config.path;
    if (args.has("expected-adapter-sha256") &&
        args.get("expected-adapter-sha256") !== formalAdapterPackage.scaffold.sha256) {
      fail("trusted adapter SHA-256 mismatch");
    }
    if (args.has("expected-adapter-config-sha256") &&
        args.get("expected-adapter-config-sha256") !== formalAdapterPackage.config.sha256) {
      fail("trusted adapter config SHA-256 mismatch");
    }
  } else {
    for (const key of ["adapter-file", "expected-adapter-sha256"]) {
      if (!args.has(key)) usage();
    }
    if (!SHA256.test(args.get("expected-adapter-sha256"))) fail("invalid expected adapter SHA-256");
    adapter = await requireRegular(args.get("adapter-file"), "trusted adapter");
    adapterBytes = await readFile(adapter);
    if (sha256(adapterBytes) !== args.get("expected-adapter-sha256")) {
      fail("trusted adapter SHA-256 mismatch");
    }
  }
  if (!common.formal_result_eligible && args.has("adapter-config")) {
    adapterConfig = await requireRegular(args.get("adapter-config"), "trusted adapter config");
    const bytes = await readFile(adapterConfig);
    if (!SHA256.test(args.get("expected-adapter-config-sha256") ?? "") ||
        sha256(bytes) !== args.get("expected-adapter-config-sha256")) {
      fail("trusted adapter config SHA-256 mismatch");
    }
  }
  const variant = await requireRegular(state.registered_variant_path, "registered variant");
  const variantBytes = await readFile(variant);
  if (sha256(variantBytes) !== state.registered_variant_sha256) fail("registered variant changed before resume");
  const registeredTask = await requireRegular(state.registered_task_path, "registered task");
  const registeredTaskBytes = await readFile(registeredTask);
  if (sha256(registeredTaskBytes) !== state.registered_task_sha256) fail("registered task changed before resume");
  const pausedValidationWorkspace = await requireDirectory(state.validation_workspace, "validation workspace");
  if (!isInside(pausedValidationWorkspace, artifactDir)) fail("validation workspace escaped controller artifacts");
  for (const [path, label] of [
    [state.core_receipt_path, "core receipt"], [state.capture_receipt_path, "capture receipt"],
    [state.validation_receipt_path, "validation receipt"], [state.process_receipt_path, "process receipt"],
    [state.final_message_path, "final message"], [state.preflight_receipt_path, "preflight receipt"],
    [state.base_control_receipt_path, "base-control receipt"],
    [state.validation_environment_receipt_path, "trusted validation environment receipt"],
  ]) {
    const canonical = await requireRegular(path, label);
    if (!isInside(canonical, artifactDir)) fail(`${label} escaped controller artifacts`);
  }
  const boundReceipts = [
    [state.preflight_receipt_path, state.bound_receipts?.preflight_sha256, "preflight"],
    [state.base_control_receipt_path, state.bound_receipts?.base_control_sha256, "base-control"],
    [state.process_receipt_path, state.bound_receipts?.process_sha256, "process"],
    [state.capture_receipt_path, state.bound_receipts?.capture_sha256, "capture"],
    [state.validation_receipt_path, state.bound_receipts?.validation_sha256, "validation"],
    [state.core_receipt_path, state.bound_receipts?.core_sha256, "core"],
    [state.validation_environment_receipt_path,
      state.bound_receipts?.validation_environment_sha256, "trusted validation environment"],
  ];
  for (const [path, expected, label] of boundReceipts) {
    if (!SHA256.test(expected ?? "") || sha256(await readFile(path)) !== expected) {
      fail(`${label} receipt changed before adapter resume`);
    }
  }
  if (!isInside(await realpath(state.capture_dir), artifactDir)) fail("candidate capture escaped controller artifacts");
  const helperNodeExecutable = await requireRegular(
    environmentReceipt.helper_node?.path,
    "recorded controller Node executable",
  );
  const helperNodeIdentity = await fileIdentity(helperNodeExecutable);
  if (helperNodeIdentity.bytes !== environmentReceipt.helper_node.bytes ||
      helperNodeIdentity.sha256 !== environmentReceipt.helper_node.sha256) {
    fail("recorded controller Node executable changed before adapter resume");
  }
  const [cases, production] = await Promise.all([
    readFile(resolve(artifactDir, "evaluator/cases.json"), "utf8").then(JSON.parse),
    readFile(resolve(artifactDir, "evaluator/production-oracles.json"), "utf8").then(JSON.parse),
  ]);
  const caseEntry = cases.cases.find((entry) => entry.id === common.case_id);
  const productionCase = production.cases.find((entry) => entry.id === common.case_id);
  if (!caseEntry || !productionCase ||
      !productionCase.oracle_commands.some((entry) => entry.command_template.includes("<trusted-adapter-file>"))) {
    fail("paused case does not require a trusted adapter");
  }

  const stageDir = await requireDirectory(resolve(artifactDir, "stage-receipts"), "stage receipt directory");
  const names = (await readdir(stageDir)).sort();
  let priorStageReceiptSha256 = null;
  let stageOrder = 0;
  let lastStageReceipt = null;
  for (const name of names) {
    const bytes = await readFile(resolve(stageDir, name));
    const receipt = JSON.parse(bytes.toString("utf8"));
    if (receipt.stage_order !== stageOrder || receipt.prior_receipt_sha256 !== priorStageReceiptSha256) {
      fail("stage chain is invalid before resume");
    }
    for (const key of commonKeys) {
      if (canonicalJson(receipt[key]) !== canonicalJson(common[key])) {
        fail(`stage chain ${key} is invalid before resume`);
      }
    }
    if (receipt.payload_sha256 !== sha256(canonicalBytes(receipt.payload))) {
      fail(`stage payload hash is invalid before resume: ${receipt.stage}`);
    }
    if (receipt.controller_bundle_verification?.verified !== true ||
        receipt.controller_bundle_verification.tree_sha256 !== state.controller_bundle_sha256 ||
        receipt.controller_bundle_verification.manifest?.sha256 !== state.controller_bundle_sha256) {
      fail(`stage controller bundle verification is invalid before resume: ${receipt.stage}`);
    }
    for (const identity of [...(receipt.inputs ?? []), ...(receipt.outputs ?? [])]) {
      const path = await requireRegular(identity.path, `stage artifact ${receipt.stage}`);
      const observed = await fileIdentity(path);
      if (observed.bytes !== identity.bytes || observed.sha256 !== identity.sha256) {
        fail(`stage artifact changed before resume: ${receipt.stage}`);
      }
    }
    priorStageReceiptSha256 = sha256(bytes);
    stageOrder += 1;
    lastStageReceipt = receipt;
  }
  if (names.length === 0 || !names.at(-1).endsWith("-awaiting_trusted_adapter.json")) {
    fail("adapter pause is not the final stage");
  }
  const observedStateIdentity = await fileIdentity(statePath);
  if (!(lastStageReceipt.outputs ?? []).some((entry) =>
    entry.path === statePath && entry.bytes === observedStateIdentity.bytes &&
    entry.sha256 === observedStateIdentity.sha256)) {
    fail("adapter pause receipt is not bound by the final stage");
  }
  const ledgerPath = await requireRegular(resolve(artifactDir, "attempt-ledger.jsonl"), "attempt ledger");
  const lines = (await readFile(ledgerPath, "utf8")).trim().split("\n").map(JSON.parse);
  if (lines.length === 2 && lines[1].disposition !== "registered") {
    const markerPath = resolve(artifactDir, "terminal.json");
    if (!existsSync(markerPath)) {
      try { await writeFile(markerPath, canonicalBytes(lines[1]), {mode: 0o600, flag: "wx"}); }
      catch { /* the append-only ledger remains authoritative */ }
    }
    fail("attempt ledger already contains a terminal outcome");
  }
  if (lines.length !== 1 || lines[0].disposition !== "registered" || lines[0].attempt_id !== common.attempt_id) {
    fail("attempt ledger is not resumable");
  }
  const {entry_sha256: registeredEntrySha256, ...registeredEntryBody} = lines[0];
  if (registeredEntrySha256 !== sha256(canonicalBytes(registeredEntryBody))) {
    fail("registered attempt ledger entry hash is invalid");
  }
  async function writeStage(stage, payload, inputPaths = [], outputPaths = []) {
    const validationGuard = await verifyResumeValidationGuard(`before resumed stage ${stage}`);
    const bundleVerification = await verifyControllerBundle(`before resumed stage ${stage}`);
    const [inputs, outputs] = await Promise.all([
      Promise.all(inputPaths.map((path) => fileIdentity(path))),
      Promise.all(outputPaths.map((path) => fileIdentity(path))),
    ]);
    const receipt = {
      schema: "tachiko-controller-stage-receipt-v1", ...common, stage, stage_order: stageOrder,
      prior_receipt_sha256: priorStageReceiptSha256,
      controller_bundle_verification: bundleVerification,
      trusted_validation_environment_verification: validationGuard,
      inputs, outputs,
      payload_sha256: sha256(canonicalBytes(payload)), payload, completed_at: new Date().toISOString(),
    };
    const path = resolve(stageDir, `${String(stageOrder).padStart(2, "0")}-${stage}.json`);
    const bytes = canonicalBytes(receipt);
    await writeFile(path, bytes, {mode: 0o600, flag: "wx"});
    priorStageReceiptSha256 = sha256(bytes);
    stageOrder += 1;
    return path;
  }
  const resumeHelperProtectedRoots = [
    controllerBenchmarkDir,
    state.source_repo,
    dirname(registryPath),
    stageDir,
    state.original_candidate_workspace,
    state.validation_workspace,
    state.capture_dir,
    state.validation_environment_root,
    environment.CARGO_HOME,
    environment.HOME,
    environment.CODEX_HOME,
  ].filter(Boolean);
  const resumeHelperProtectedPaths = [
    infrastructureManifestPath,
    statePath,
    ledgerPath,
    registryPath,
    state.registered_variant_path,
    state.registered_task_path,
    state.preflight_receipt_path,
    state.base_control_receipt_path,
    state.process_receipt_path,
    state.capture_receipt_path,
    state.validation_receipt_path,
    state.core_receipt_path,
    common.formal_authorization?.path,
  ].filter(Boolean);
  async function runBundledHelper(relativeScript, helperArguments, options = {}, {
    extraProtectedRoots = [], extraProtectedPaths = [], nestedProcessSupervisor = false,
  } = {}) {
    await verifyResumeValidationGuard(`before resumed trusted helper ${relativeScript}`);
    await verifyControllerBundle(`before resumed trusted helper ${relativeScript}`);
    const writePolicy = trustedHelperWriteProfile({
      protectedRoots: [...resumeHelperProtectedRoots, ...extraProtectedRoots],
      protectedPaths: [...resumeHelperProtectedPaths, ...extraProtectedPaths],
    });
    const protectionArgument = JSON.stringify({
      schema: "tachiko-supervised-write-protection-v1",
      protected_roots: writePolicy.roots,
      protected_paths: writePolicy.paths,
    });
    const result = nestedProcessSupervisor
      ? command(helperNodeExecutable, [
        resolve(controllerBenchmarkDir, relativeScript),
        ...helperArguments,
        "--supervised-write-protection-json", protectionArgument,
      ], options)
      : command("/usr/bin/sandbox-exec", [
        "-p", writePolicy.profile,
        helperNodeExecutable,
        resolve(controllerBenchmarkDir, relativeScript),
        ...helperArguments,
      ], options);
    await verifyResumeValidationGuard(`after resumed trusted helper ${relativeScript}`);
    await verifyControllerBundle(`after resumed trusted helper ${relativeScript}`);
    result.controller_bundle_write_protection = {
      schema: "tachiko-trusted-helper-write-protection-v1",
      protected_roots: writePolicy.roots,
      protected_paths: writePolicy.paths,
      profile_sha256: writePolicy.profile_sha256,
      active: true,
      enforcement_scope: nestedProcessSupervisor
        ? "every_inner_coalition_profile"
        : "outer_trusted_helper_process",
    };
    return result;
  }
  const resumeLockPath = resolve(artifactDir, "adapter-resume-lock.json");
  await writeFile(resumeLockPath, canonicalBytes({
    schema: "tachiko-controller-adapter-resume-lock-v1",
    ...common,
    adapter_sha256: sha256(adapterBytes),
    acquired_at: new Date().toISOString(),
  }), {mode: 0o600, flag: "wx"});
  let resumeTerminalCommitted = false;
  const registryTerminalPath = registryPath.endsWith(".json")
    ? `${registryPath.slice(0, -5)}.terminal.json`
    : `${registryPath}.terminal.json`;
  async function commitResumeTerminal(terminal) {
    const outcome = await commitTerminalEntry({
      ledgerPath,
      markerPath: resolve(artifactDir, "terminal.json"),
      terminal,
      onCommitted() { resumeTerminalCommitted = true; },
    });
    if (!existsSync(registryTerminalPath)) {
      try {
        const externalTerminal = {
          schema: "tachiko-controller-registry-terminal-v1",
          ...common,
          disposition: terminal.disposition,
          resampling_performed: false,
          launch_count: terminal.launch_count,
          attempt_registry_entry_sha256: common.attempt_registry_entry.sha256,
          local_terminal_entry_sha256: terminal.entry_sha256,
          detail: terminal.detail,
          terminal_at: terminal.terminal_at,
        };
        externalTerminal.entry_sha256 = sha256(canonicalBytes(externalTerminal));
        await writeFile(registryTerminalPath, canonicalBytes(externalTerminal), {mode: 0o600, flag: "wx"});
      } catch (error) {
        if (error?.code !== "EEXIST") throw error;
      }
    }
    return outcome;
  }
  committedFailureHandler = async (error) => {
    if (resumeTerminalCommitted) return;
    const disposition = /review packet|strict UTF-8|binary control/i.test(String(error))
      ? "invalid_discarded"
      : "infrastructure_failed";
    const terminal = {
      schema: "tachiko-controller-attempt-entry-v1", ...common, disposition, attempt_number: 1,
      previous_attempt_entry_sha256: lines[0].entry_sha256,
      final_stage_receipt_sha256: priorStageReceiptSha256,
      resampling_performed: false,
      launch_count: 1,
      detail: {launch_count: 1, resumed_same_attempt: true, error: String(error)},
      terminal_at: new Date().toISOString(),
    };
    terminal.entry_sha256 = sha256(canonicalBytes(terminal));
    await commitResumeTerminal(terminal);
  };

  if (formalAdapterPackage?.probe_build_receipt) {
    const buildStagePath = await writeStage("formal_adapter_build", {
      case_id: common.case_id,
      capture_receipt_sha256: controllerContext.capture_receipt_sha256,
      candidate_tree: controllerContext.candidate_tree,
      raw_tree_digest_sha256: controllerContext.raw_tree_digest_sha256,
      probe_sha256: formalAdapterPackage.probe.sha256,
      probe_build_receipt_sha256: formalAdapterPackage.probe_build_receipt.sha256,
      cargo_home_manifest_sha256:
        formalAdapterPackage.probe_build_receipt.cargo_home_manifest_sha256,
      trusted_validation_environment_sha256:
        state.bound_receipts.validation_environment_sha256,
      sealed_controller_builder: true,
    }, [
      formalAdapterPackage.config.path,
      formalAdapterPackage.integrity_receipt.path,
      formalAdapterPackage.probe_source.path,
      formalAdapterPackage.candidate_manifest.path,
    ], [
      formalAdapterPackage.probe.path,
      formalAdapterPackage.probe_build_receipt.path,
    ]);
    formalAdapterPackage.adapter_build_stage_receipt = await fileIdentity(buildStagePath);
  }

  const captureReceipt = JSON.parse(await readFile(state.capture_receipt_path, "utf8"));
  const sourceRepo = await requireDirectory(state.source_repo, "trusted source repository");
  const sourceInfo = await lstat(sourceRepo, {bigint: true});
  const expectedSourceIdentity = state.source_repo_identity;
  if (sourceRepo !== expectedSourceIdentity?.path || sourceInfo.dev.toString() !== expectedSourceIdentity.device ||
      sourceInfo.ino.toString() !== expectedSourceIdentity.inode || sourceInfo.uid.toString() !== expectedSourceIdentity.uid ||
      sourceInfo.gid.toString() !== expectedSourceIdentity.gid) {
    fail("trusted source repository identity changed before adapter resume");
  }
  const validationWorkspace = resolve(artifactDir, "resume-validation-workspace");
  const validationPreparationDir = resolve(artifactDir, "resume-validation-preparation");
  const reconstruction = await runBundledHelper("scripts/prepare-validation.mjs", [
    "--case", common.case_id,
    "--source-repo", sourceRepo,
    "--patch-file", resolve(state.capture_dir, "candidate.patch"),
    "--capture-receipt", state.capture_receipt_path,
    "--workspace", validationWorkspace,
    "--trusted-dir", validationPreparationDir,
  ], {env: await freshResumeValidationEnvironment("prepare"), allowFailure: true});
  const reconstructedValidationReceiptPath = resolve(
    validationPreparationDir,
    "validation-preparation-receipt.json",
  );
  if (reconstruction.status !== 0 || !existsSync(reconstructedValidationReceiptPath)) {
    fail(`same-candidate validation reconstruction failed: ${reconstruction.stderr}`);
  }
  const validationReceipt = JSON.parse(await readFile(reconstructedValidationReceiptPath, "utf8"));
  if (validationReceipt.candidate_commit !== captureReceipt.candidate_commit ||
      validationReceipt.candidate_tree !== captureReceipt.candidate_tree ||
      validationReceipt.raw_tree_digest_sha256 !== captureReceipt.raw_tree_digest_sha256) {
    fail("same-candidate validation reconstruction identity mismatch");
  }
  await writeStage("resume_validation_reconstruction", {
    candidate_commit: validationReceipt.candidate_commit,
    candidate_tree: validationReceipt.candidate_tree,
    raw_tree_digest_sha256: validationReceipt.raw_tree_digest_sha256,
    agent_relaunched: false,
  }, [state.capture_receipt_path, resolve(state.capture_dir, "candidate.patch")], [reconstructedValidationReceiptPath]);
  const coreReceipt = JSON.parse(await readFile(state.core_receipt_path, "utf8"));
  const processReceipt = JSON.parse(await readFile(state.process_receipt_path, "utf8"));
  const oracleDir = resolve(artifactDir, "production-oracles");
  let oracleReceiptPath;
  if (state.construction_smoke) {
    await mkdir(oracleDir, {mode: 0o700});
    const oracleReceipt = {
      schema: "tachiko-controller-oracle-smoke-v1", ...common, construction_smoke: true,
      commands_executed: false, adapter: {path: adapter, bytes: adapterBytes.length, sha256: sha256(adapterBytes)},
      production_runner: await fileIdentity(resolve(controllerBenchmarkDir, "scripts/run-oracles.mjs")),
      case_command_count: productionCase.oracle_commands.length,
      case_assertion_count: productionCase.assertions.length,
      assessment_mode: "machine_and_or_subjective",
    };
    oracleReceiptPath = resolve(oracleDir, "oracle-run.json");
    await writeFile(oracleReceiptPath, canonicalBytes(oracleReceipt), {mode: 0o600, flag: "wx"});
  } else {
    const oracleArguments = [
      resolve(controllerBenchmarkDir, "scripts/run-oracles.mjs"), "--case", common.case_id,
      "--candidate-root", validationWorkspace, "--trusted-dir", oracleDir,
      "--expected-control-sha256", common.control_sha256,
      "--trusted-shell", preflightReceipt.binaries.bash.path,
      "--expected-shell-sha256", preflightReceipt.binaries.bash.sha256,
      "--trusted-cargo", preflightReceipt.binaries.cargo.path,
      "--expected-cargo-sha256", preflightReceipt.binaries.cargo.sha256,
      "--trusted-rustc", preflightReceipt.binaries.rustc.path,
      "--expected-rustc-sha256", preflightReceipt.binaries.rustc.sha256,
      "--candidate-commit", validationReceipt.candidate_commit,
      "--trusted-validation-environment-receipt",
      state.validation_environment_receipt_path,
      "--expected-validation-environment-sha256",
      state.bound_receipts.validation_environment_sha256,
      ...(formalAdapterPackage
        ? formalAdapterOracleArguments(formalAdapterPackage)
        : ["--adapter-file", adapter, ...(adapterConfig ? ["--adapter-config", adapterConfig] : [])]),
      "--controller-context", controllerContextPath,
      "--expected-controller-context-sha256", state.controller_context_sha256,
      ...(controllerIssuancePath ? [
        "--controller-issuance", controllerIssuancePath,
        "--expected-controller-issuance-sha256", controllerIssuanceSha256,
      ] : []),
      ...formalControllerTrustArguments(common),
      ...(common.formal_result_eligible ? ["--require-formal-context", "true"] : []),
    ];
    const oracleResult = await runBundledHelper("scripts/run-oracles.mjs", oracleArguments.slice(1), {
      env: {...environment, TMPDIR: adapterTmp, TMP: adapterTmp, TEMP: adapterTmp},
      allowFailure: true,
    }, {extraProtectedRoots: [validationWorkspace,
      environment.HOME, environment.CODEX_HOME, environment.CARGO_HOME],
    extraProtectedPaths: [state.validation_environment_receipt_path],
    nestedProcessSupervisor: true});
    oracleReceiptPath = resolve(oracleDir, "oracle-run.json");
    if (!existsSync(oracleReceiptPath)) fail(`production oracle resume failed without receipt: ${oracleResult.stderr}`);
  }
  const oracleReceipt = JSON.parse(await readFile(oracleReceiptPath, "utf8"));
  if (common.formal_result_eligible &&
      oracleReceipt.controller_issuance_sha256 !== controllerIssuanceSha256) {
    fail("production oracle resume does not bind the controller issuance");
  }
  await writeStage("production_oracles", {
    resumed_same_attempt: true,
    adapter_sha256: sha256(adapterBytes),
    construction_smoke: state.construction_smoke,
    overall_status: oracleReceipt.overall_status ?? "not_executed",
    controller_context_sha256: state.controller_context_sha256,
    controller_issuance_sha256: controllerIssuanceSha256,
    adapter_integrity_receipt_sha256: formalAdapterPackage?.integrity_receipt.sha256 ?? null,
    adapter_probe_build_receipt_sha256:
      formalAdapterPackage?.probe_build_receipt?.sha256 ?? null,
    adapter_build_stage_receipt_sha256:
      formalAdapterPackage?.adapter_build_stage_receipt?.sha256 ?? null,
  }, [
    statePath, state.core_receipt_path, adapter, resumeLockPath, controllerContextPath,
    ...(controllerIssuancePath ? [controllerIssuancePath] : []),
    ...(formalAdapterPackage ? [formalAdapterPackage.scaffold_lock.path,
      formalAdapterPackage.config.path, formalAdapterPackage.probe.path,
      formalAdapterPackage.integrity_receipt.path,
      ...(formalAdapterPackage.probe_source ? [formalAdapterPackage.probe_source.path] : []),
      ...(formalAdapterPackage.candidate_manifest
        ? [formalAdapterPackage.candidate_manifest.path] : []),
      ...(formalAdapterPackage.probe_build_receipt
        ? [formalAdapterPackage.probe_build_receipt.path] : []),
      ...(formalAdapterPackage.adapter_build_stage_receipt
        ? [formalAdapterPackage.adapter_build_stage_receipt.path] : [])] : []),
  ], [oracleReceiptPath]);

  const reviewInputDir = resolve(artifactDir, "review-input");
  await mkdir(reviewInputDir, {mode: 0o700});
  const taskBytes = registeredTaskBytes;
  const reviewSources = [
    {path: "task.txt", role: "task", bytes: taskBytes},
    {path: "authority.json", role: "authority", bytes: await authorityPacket(common.case_id, controllerBenchmarkDir)},
    {path: "candidate-checkout.json", role: "candidate_checkout", bytes: await readFile(resolve(state.capture_dir, "raw-manifest.json"))},
    {path: "candidate.diff", role: "candidate_diff", bytes: await readFile(resolve(state.capture_dir, "candidate.patch"))},
    {path: "candidate-validation.json", role: "candidate_validation", bytes: canonicalBytes({core: coreReceipt, oracles: oracleReceipt})},
    {path: "final-message.txt", role: "final_message", bytes: await readFile(state.final_message_path)},
    ...await changedCandidateReviewSources(validationWorkspace, captureReceipt),
  ];
  reviewSources.sort((left, right) => Buffer.from(left.path).compare(Buffer.from(right.path)));
  const reviewArtifacts = [];
  for (const source of reviewSources) {
    const destination = resolve(reviewInputDir, source.path);
    await mkdir(dirname(destination), {recursive: true, mode: 0o700});
    await writeFile(destination, source.bytes, {mode: 0o600, flag: "wx"});
    reviewArtifacts.push({path: source.path, roles: [source.role], bytes: source.bytes.length, sha256: sha256(source.bytes)});
  }
  const reviewInputManifestPath = resolve(artifactDir, "review-input-manifest.json");
  await writeFile(reviewInputManifestPath, canonicalBytes({schema: "tachiko-review-packet-input-v1", artifacts: reviewArtifacts}), {mode: 0o600, flag: "wx"});
  const reviewOutputDir = resolve(artifactDir, "review-output");
  const reviewTerminalPath = resolve(artifactDir, "review-terminal.json");
  const reviewResult = await runBundledHelper("scripts/build-review-packet.mjs", [
    "--case-id", common.case_id,
    "--candidate-id", common.candidate_id, "--input-root", reviewInputDir,
    "--input-manifest", reviewInputManifestPath, "--variant", variant,
    "--contract", resolve(controllerBenchmarkDir, "evaluator/contracts/review-packet-blinding-v1.json"),
    "--output-dir", reviewOutputDir, "--terminal-receipt", reviewTerminalPath,
    "--custodian-id", args.get("custodian-id") ?? "internal-custodian",
    "--custodian-eligible", "true", "--frozen-at", state.frozen_at,
    "--controller-context", controllerContextPath,
    "--expected-controller-context-sha256", state.controller_context_sha256,
    ...(controllerIssuancePath ? [
      "--controller-issuance", controllerIssuancePath,
      "--expected-controller-issuance-sha256", controllerIssuanceSha256,
    ] : []),
    ...formalControllerTrustArguments(common),
    ...(common.formal_result_eligible ? ["--require-formal-context", "true"] : []),
  ], {env: await freshResumeValidationEnvironment("review-build"), allowFailure: true}, {
    extraProtectedRoots: [validationWorkspace],
  });
  const reviewReceiptPath = resolve(reviewOutputDir, "receipt.json");
  if (reviewResult.status !== 0 || !existsSync(reviewReceiptPath)) {
    fail(`review packet construction failed after adapter resume: ${reviewResult.stderr}`);
  }
  const reviewReceipt = JSON.parse(await readFile(reviewReceiptPath, "utf8"));
  if (!reviewReceipt.safe_to_release || (common.formal_result_eligible &&
      reviewReceipt.controller_issuance_sha256 !== controllerIssuanceSha256)) {
    fail("review packet is not safe to release or lacks the controller issuance binding");
  }
  const standaloneScanReceiptPath = resolve(artifactDir, "review-standalone-scan.json");
  const scanResult = await runBundledHelper("scripts/scan-review-packet.mjs", [
    "--packet-dir", resolve(reviewOutputDir, "packet"),
    "--contract", resolve(controllerBenchmarkDir, "evaluator/contracts/review-packet-blinding-v1.json"),
    "--variant", variant,
    "--receipt", standaloneScanReceiptPath,
    "--controller-context", controllerContextPath,
    "--expected-controller-context-sha256", state.controller_context_sha256,
    ...(controllerIssuancePath ? [
      "--controller-issuance", controllerIssuancePath,
      "--expected-controller-issuance-sha256", controllerIssuanceSha256,
    ] : []),
    ...formalControllerTrustArguments(common),
    ...(common.formal_result_eligible ? ["--require-formal-context", "true"] : []),
  ], {env: await freshResumeValidationEnvironment("review-scan"), allowFailure: true}, {
    extraProtectedRoots: [validationWorkspace, reviewInputDir],
  });
  if (scanResult.status !== 0 || !existsSync(standaloneScanReceiptPath)) {
    fail(`standalone review packet scan failed after adapter resume: ${scanResult.stderr}`);
  }
  const standaloneScanReceipt = JSON.parse(await readFile(standaloneScanReceiptPath, "utf8"));
  if (!standaloneScanReceipt.safe_to_release ||
      standaloneScanReceipt.packet_tree_sha256 !== reviewReceipt.rendered_packet_sha256 ||
      standaloneScanReceipt.controller_context_sha256 !== state.controller_context_sha256 ||
      (common.formal_result_eligible &&
        standaloneScanReceipt.controller_issuance_sha256 !== controllerIssuanceSha256)) {
    fail("standalone review packet scan does not bind resumed packet and controller context");
  }
  await writeStage("review_packet", {
    safe_to_release: true, semantic_scoring_performed: false, resumed_same_attempt: true,
    controller_context_sha256: state.controller_context_sha256,
    controller_issuance_sha256: controllerIssuanceSha256,
    standalone_scan_receipt_sha256: sha256(await readFile(standaloneScanReceiptPath)),
  }, [reviewInputManifestPath, controllerContextPath,
    ...(controllerIssuancePath ? [controllerIssuancePath] : [])], [
    reviewReceiptPath, reviewTerminalPath, standaloneScanReceiptPath,
  ]);

  const disposition = processReceipt.timed_out
    ? "agent_timeout"
    : processReceipt.spawn_error || processReceipt.exit_code !== 0 ? "agent_failed" : "awaiting_review";
  const resultSkeleton = {
    schema: "tachiko-controller-result-skeleton-v1", ...common, disposition,
    ...pendingResultState(common.formal_result_eligible),
    no_resampling: true, launch_count: 1, resumed_same_attempt: true,
    candidate_commit: captureReceipt.candidate_commit,
    candidate_tree: captureReceipt.candidate_tree,
    raw_tree_digest_sha256: captureReceipt.raw_tree_digest_sha256,
    preflight_receipt_sha256: sha256(await readFile(state.preflight_receipt_path)),
    base_control_receipt_sha256: sha256(await readFile(state.base_control_receipt_path)),
    process_receipt_sha256: sha256(await readFile(state.process_receipt_path)),
    capture_receipt_sha256: sha256(await readFile(state.capture_receipt_path)),
    validation_receipt_sha256: sha256(await readFile(state.validation_receipt_path)),
    core_receipt_sha256: sha256(await readFile(state.core_receipt_path)),
    oracle_receipt_sha256: sha256(await readFile(oracleReceiptPath)),
    review_receipt_sha256: sha256(await readFile(reviewReceiptPath)),
    review_scan_receipt_sha256: sha256(await readFile(standaloneScanReceiptPath)),
    controller_context_sha256: state.controller_context_sha256,
    controller_issuance_sha256: controllerIssuanceSha256,
    adapter_integrity_receipt_sha256: formalAdapterPackage?.integrity_receipt.sha256 ?? null,
    scores_recorded: false,
    semantic_review_pending: true,
    limitations: [
      "provider-internal immutable deployment identity unavailable",
      "current-user host isolation rather than dedicated provider or OS accounts",
      "multi-reviewer scoring panel not completed by the controller",
    ],
  };
  const resultPath = resolve(artifactDir, "result-skeleton.json");
  await writeFile(resultPath, canonicalBytes(resultSkeleton), {mode: 0o600, flag: "wx"});
  await writeStage("result_skeleton", resultSkeleton, [reviewReceiptPath,
    ...(controllerIssuancePath ? [controllerIssuancePath] : [])], [resultPath]);

  const terminal = {
    schema: "tachiko-controller-attempt-entry-v1", ...common, disposition, attempt_number: 1,
    previous_attempt_entry_sha256: lines[0].entry_sha256,
    final_stage_receipt_sha256: priorStageReceiptSha256,
    resampling_performed: false, launch_count: 1,
    detail: {
      launch_count: 1, resumed_same_attempt: true, adapter_sha256: sha256(adapterBytes),
      adapter_integrity_receipt_sha256: formalAdapterPackage?.integrity_receipt.sha256 ?? null,
      controller_context_sha256: state.controller_context_sha256,
      controller_issuance_sha256: controllerIssuanceSha256,
      result_skeleton_sha256: sha256(await readFile(resultPath)),
    },
    terminal_at: new Date().toISOString(),
  };
  terminal.entry_sha256 = sha256(canonicalBytes(terminal));
  await commitResumeTerminal(terminal);
  committedFailureHandler = null;
  console.log(JSON.stringify({artifact_dir: artifactDir, disposition, launch_count: 1, resumed_same_attempt: true}));
  if (disposition !== "awaiting_review") process.exitCode = 1;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.has("resume-artifact-dir")) return resumeWithAdapter(args);
  for (const key of [
    "case", "source-repo", "variant-file", "expected-variant-sha256", "phase", "run-root",
    "artifact-dir", "agent-executable", "agent-args-file", "timeout-seconds", "wave-id",
    "run-id", "attempt-id", "candidate-id", "attempt-registry-dir",
  ]) if (!args.has(key)) usage();
  const caseId = args.get("case");
  const phase = args.get("phase");
  const constructionSmoke = args.get("construction-smoke") === "true";
  if (!/^TW-0[1-9]$/.test(caseId)) fail("invalid case ID");
  if (!["construction_pilot_only", "baseline_a", "variant_b"].includes(phase)) fail("invalid phase");
  if (args.has("construction-smoke") && !constructionSmoke) fail("--construction-smoke only accepts true");
  for (const key of ["wave-id", "run-id", "attempt-id", "candidate-id"]) {
    if (!ID.test(args.get(key))) fail(`${key} must be opaque lowercase 128-bit hex`);
  }
  if (!SHA256.test(args.get("expected-variant-sha256"))) fail("invalid expected variant SHA-256");
  const timeoutSeconds = Number(args.get("timeout-seconds"));
  if (!Number.isSafeInteger(timeoutSeconds) || timeoutSeconds < 1 || timeoutSeconds > 86400) {
    fail("timeout-seconds must be an integer from 1 through 86400");
  }

  // Formal authorization is checked before any run or artifact path is created.
  if (phase !== "construction_pilot_only" && !args.has("authorization-file")) {
    fail("external formal authorization file is required before preparation or launch");
  }
  if (phase !== "construction_pilot_only" && !args.has("provider-auth-qualification")) {
    fail("external provider auth qualification is required before registration or launch");
  }
  if (phase !== "construction_pilot_only" && !args.has("operator-keychain")) {
    fail("operator keychain is required before formal registration or launch");
  }
  if (constructionSmoke && phase !== "construction_pilot_only") fail("construction smoke is forbidden in formal phases");

  const sourceRepo = await requireDirectory(args.get("source-repo"), "source repository");
  const attemptRegistryDir = await requireDirectory(args.get("attempt-registry-dir"), "attempt registry directory");
  let authorizationIdentity = null;
  let formalAuthorization = null;
  if (phase !== "construction_pilot_only") {
    const authorizationPath = await requireRegular(args.get("authorization-file"), "external formal authorization");
    if (isInside(authorizationPath, sourceRepo) || isInside(authorizationPath, benchmarkDir)) {
      fail("external formal authorization must not be stored in the repository");
    }
    const authorizationBytes = await readFile(authorizationPath);
    formalAuthorization = JSON.parse(authorizationBytes.toString("utf8"));
    if (formalAuthorization.schema !== "tachiko-formal-run-authorization-v1" ||
        formalAuthorization.phase !== phase || formalAuthorization.wave_id !== args.get("wave-id") ||
        formalAuthorization.run_id !== args.get("run-id") ||
        formalAuthorization.attempt_id !== args.get("attempt-id") ||
        formalAuthorization.candidate_id !== args.get("candidate-id") ||
        formalAuthorization.case_id !== caseId ||
        formalAuthorization.attempt_registry_dir !== attemptRegistryDir ||
        typeof formalAuthorization.authorization_token !== "string" || formalAuthorization.authorization_token.length < 32) {
      fail("external formal authorization is invalid");
    }
    authorizationIdentity = {
      path: authorizationPath,
      bytes: authorizationBytes.length,
      sha256: sha256(authorizationBytes),
    };
  }

  const variantFile = await requireRegular(args.get("variant-file"), "variant file");
  const agentExecutable = await requireRegular(args.get("agent-executable"), "agent executable");
  const agentArgsFile = await requireRegular(args.get("agent-args-file"), "agent args file");
  if (((await lstat(agentExecutable)).mode & 0o111) === 0) fail("agent executable is not executable");
  const variantBytes = await readFile(variantFile);
  if (sha256(variantBytes) !== args.get("expected-variant-sha256")) fail("variant SHA-256 mismatch");
  const agentArgsBytes = await readFile(agentArgsFile);
  const agentExecutableIdentity = await fileIdentity(agentExecutable);
  const agentArgumentsIdentity = await fileIdentity(agentArgsFile);
  const agentArguments = JSON.parse(agentArgsBytes.toString("utf8"));
  if (!Array.isArray(agentArguments) || agentArguments.some((value) => typeof value !== "string")) {
    fail("agent args file must contain a JSON string array");
  }
  const runRoot = await prospective(args.get("run-root"), "run root");
  const artifactDir = await prospective(args.get("artifact-dir"), "artifact directory");
  if (!RUN_ROOT.test(basename(runRoot))) fail("run root must use r-<32-lowercase-hex>");
  if (isInside(runRoot, artifactDir) || isInside(artifactDir, runRoot)) fail("run root and artifact directory must be disjoint");
  if (isInside(runRoot, sourceRepo) || isInside(sourceRepo, runRoot) ||
      isInside(artifactDir, sourceRepo) || isInside(sourceRepo, artifactDir)) {
    fail("run/artifact paths and source repository must be disjoint");
  }
  for (const path of [runRoot, artifactDir, sourceRepo]) {
    if (isInside(path, attemptRegistryDir) || isInside(attemptRegistryDir, path)) {
      fail("attempt registry must be disjoint from run, artifact, and source paths");
    }
  }
  let frozenEnvironmentLock = null;
  let formalRuntime = null;
  let formalCatalogSource = null;
  let formalCatalogInspection = null;
  let formalEffectiveArguments = null;
  let formalCargoHomeInspection = null;
  let formalNetworkSandboxIdentity = null;
  let formalRustupHomeTemplate = null;
  let formalCargoHomeTemplate = null;
  let providerAuthQualification = null;
  let providerAuthQualificationIdentity = null;
  let operatorKeychain = null;
  const stagedModelCatalogPath = resolve(runRoot, "runtime", "model-catalog.json");
  if (formalAuthorization) {
    frozenEnvironmentLock = JSON.parse(await readFile(resolve(benchmarkDir, "environment-lock.json"), "utf8"));
    const providerAuthQualificationPath = await requireRegular(
      args.get("provider-auth-qualification"),
      "external provider auth qualification",
    );
    operatorKeychain = await requireRegular(args.get("operator-keychain"), "operator keychain");
    for (const [path, label] of [
      [providerAuthQualificationPath, "provider auth qualification"],
      [operatorKeychain, "operator keychain"],
    ]) {
      if (isInside(path, sourceRepo) || isInside(path, benchmarkDir) ||
          isInside(path, runRoot) || isInside(path, artifactDir)) {
        fail(`external ${label} must be disjoint from repositories and run artifacts`);
      }
    }
    const providerAuthQualificationBytes = await readFile(providerAuthQualificationPath);
    providerAuthQualification = JSON.parse(providerAuthQualificationBytes.toString("utf8"));
    validateProviderAuthQualification(providerAuthQualification, {
      runRoot,
      codexBinarySha256: frozenEnvironmentLock.controlled_agent.codex_binary_sha256,
      modelId: frozenEnvironmentLock.controlled_agent.model_id,
      reasoningEffort: frozenEnvironmentLock.controlled_agent.reasoning_effort,
    });
    if (providerAuthQualification.keychain_path_sha256 !==
        sha256(Buffer.from(operatorKeychain, "utf8"))) {
      fail("operator keychain path differs from the provider auth qualification");
    }
    providerAuthQualificationIdentity = {
      path: providerAuthQualificationPath,
      bytes: providerAuthQualificationBytes.length,
      sha256: sha256(providerAuthQualificationBytes),
    };
    const frozenCodexPath = await realpath(frozenEnvironmentLock.controlled_agent.codex_binary_path);
    if (agentExecutable !== frozenCodexPath ||
        agentExecutableIdentity.sha256 !== frozenEnvironmentLock.controlled_agent.codex_binary_sha256) {
      fail("formal agent executable does not match the frozen Codex binary");
    }
    if (phase === "baseline_a" && sha256(variantBytes) !== frozenEnvironmentLock.baseline_a_agents.sha256) {
      fail("Baseline A variant does not match the frozen environment lock");
    }
    const expectedArguments = frozenFormalAgentArguments(frozenEnvironmentLock);
    if (JSON.stringify(agentArguments) !== JSON.stringify(expectedArguments)) {
      fail("formal agent arguments do not match the frozen model/config invocation");
    }
    if (!args.has("model-catalog-file")) {
      fail("formal phase requires an explicit trusted model catalog source");
    }
    if (!args.has("rustup-home-template") || !args.has("cargo-home-template")) {
      fail("formal phase requires explicit trusted Rustup and Cargo home templates");
    }
    formalCatalogSource = await requireRegular(args.get("model-catalog-file"), "trusted formal model catalog");
    if (isInside(formalCatalogSource, sourceRepo) || isInside(formalCatalogSource, benchmarkDir) ||
        isInside(formalCatalogSource, runRoot) || isInside(formalCatalogSource, artifactDir)) {
      fail("trusted formal model catalog source must be external to repositories and run artifacts");
    }
    formalCatalogInspection = await inspectFormalModelCatalog(
      formalCatalogSource,
      frozenEnvironmentLock.controlled_agent.bundled_model_catalog,
      frozenEnvironmentLock.controlled_agent.model_id,
    );
    if (providerAuthQualification.model_catalog_sha256 !== formalCatalogInspection.sha256) {
      fail("formal model catalog differs from the provider auth qualification");
    }
    const rustupHomeTemplate = await requireDirectory(
      args.get("rustup-home-template"),
      "trusted Rustup home template",
    );
    const cargoHomeTemplate = await requireDirectory(
      args.get("cargo-home-template"),
      "trusted Cargo home template",
    );
    formalRustupHomeTemplate = rustupHomeTemplate;
    formalCargoHomeTemplate = cargoHomeTemplate;
    for (const [path, label] of [
      [rustupHomeTemplate, "Rustup home template"],
      [cargoHomeTemplate, "Cargo home template"],
    ]) {
      if (isInside(path, sourceRepo) || isInside(path, benchmarkDir) ||
          isInside(path, runRoot) || isInside(path, artifactDir)) {
        fail(`trusted ${label} must be external to repositories and run artifacts`);
      }
    }
    formalRuntime = await inspectFormalRuntime(frozenEnvironmentLock, rustupHomeTemplate);
    formalNetworkSandboxIdentity = await fileIdentity("/usr/bin/sandbox-exec");
    if (providerAuthQualification.sandbox_executable_sha256 !==
        formalNetworkSandboxIdentity.sha256) {
      fail("formal candidate sandbox differs from the provider auth qualification");
    }
    formalCargoHomeInspection = await inspectFormalCargoHome(cargoHomeTemplate, frozenEnvironmentLock);
    formalEffectiveArguments = frozenFormalAgentArguments(
      frozenEnvironmentLock,
      resolve(runRoot, "workspace"),
      stagedModelCatalogPath,
    );
    const codeModeHost = formalRuntime.tools.find((tool) => tool.name === "codex-code-mode-host");
    requireFormalAuthorizationCommitments(formalAuthorization, {
      agent_executable_sha256: agentExecutableIdentity.sha256,
      agent_args_sha256: agentArgumentsIdentity.sha256,
      variant_sha256: sha256(variantBytes),
      model_catalog_sha256: formalCatalogInspection.sha256,
      code_mode_host_sha256: codeModeHost.sha256,
      formal_runtime_identity_sha256: formalRuntime.identity_sha256,
      effective_agent_args_sha256: sha256(canonicalBytes(formalEffectiveArguments)),
      rustup_home_template_sha256: formalRuntime.rustup_home.manifest_sha256,
      pnpm_home_template_sha256: formalRuntime.pnpm_home.manifest_sha256,
      cargo_home_template_sha256: formalCargoHomeInspection.manifest_sha256,
      sandbox_executable_sha256: formalNetworkSandboxIdentity.sha256,
      provider_auth_qualification_sha256: providerAuthQualificationIdentity.sha256,
    });
  }

  const manifests = await Promise.all([
    readFile(resolve(benchmarkDir, "evaluator/cases.json"), "utf8").then(JSON.parse),
    readFile(resolve(benchmarkDir, "evaluator/production-oracles.json"), "utf8").then(JSON.parse),
  ]);
  const caseEntry = manifests[0].cases.find((entry) => entry.id === caseId);
  const productionCase = manifests[1].cases.find((entry) => entry.id === caseId);
  if (!caseEntry || !productionCase) fail(`missing frozen case ${caseId}`);
  const terminationGraceSeconds = constructionSmoke ? 0.25 : 10;
  if (formalAuthorization) requireFormalTiming(caseEntry, timeoutSeconds, formalAuthorization);
  const frozenTaskBytes = await readFile(resolve(benchmarkDir, caseEntry.task_file));
  if (sha256(frozenTaskBytes) !== caseEntry.task_sha256 || frozenTaskBytes.length !== caseEntry.task_bytes) {
    fail("frozen task identity changed before attempt registration");
  }
  const registryKey = sha256(Buffer.from(
    `${manifests[0].protocol_id}:${phase}:${args.get("wave-id")}:${caseId}\n`,
    "utf8",
  ));
  const registryEntryPath = resolve(attemptRegistryDir, `${registryKey}.json`);
  const registryTerminalPath = resolve(attemptRegistryDir, `${registryKey}.terminal.json`);
  const registryEntry = {
    schema: "tachiko-controller-attempt-registry-v1",
    protocol_id: manifests[0].protocol_id,
    phase,
    wave_id: args.get("wave-id"),
    run_id: args.get("run-id"),
    attempt_id: args.get("attempt-id"),
    candidate_id: args.get("candidate-id"),
    case_id: caseId,
    slot_key_sha256: registryKey,
    uniqueness_scope: "protocol_id:phase:wave_id:case_id",
    run_root: runRoot,
    artifact_dir: artifactDir,
    source_repo: sourceRepo,
    variant_sha256: sha256(variantBytes),
    agent_executable_sha256: agentExecutableIdentity.sha256,
    agent_args_sha256: agentArgumentsIdentity.sha256,
    rustup_home_template_sha256: formalRuntime?.rustup_home.manifest_sha256 ?? null,
    pnpm_home_template_sha256: formalRuntime?.pnpm_home.manifest_sha256 ?? null,
    cargo_home_template_sha256: formalCargoHomeInspection?.manifest_sha256 ?? null,
    formal_authorization_sha256: authorizationIdentity?.sha256 ?? null,
    provider_auth_qualification_sha256: providerAuthQualificationIdentity?.sha256 ?? null,
    registered_at: new Date().toISOString(),
  };
  const registryEntryBytes = canonicalBytes(registryEntry);
  try {
    await writeFile(registryEntryPath, registryEntryBytes, {mode: 0o600, flag: "wx"});
  } catch (error) {
    if (error?.code === "EEXIST") {
      fail("wave/case/phase slot is already registered; resampling denied");
    }
    throw error;
  }
  let externalTerminalCommitted = false;
  async function externalTerminalize(disposition, detail = {}) {
    if (externalTerminalCommitted || existsSync(registryTerminalPath)) return;
    const body = {
      schema: "tachiko-controller-registry-terminal-v1",
      protocol_id: registryEntry.protocol_id,
      phase,
      wave_id: registryEntry.wave_id,
      run_id: registryEntry.run_id,
      attempt_id: registryEntry.attempt_id,
      candidate_id: registryEntry.candidate_id,
      case_id: registryEntry.case_id,
      slot_key_sha256: registryKey,
      disposition,
      resampling_performed: false,
      launch_count: detail.launch_count ?? 0,
      attempt_registry_entry_sha256: sha256(registryEntryBytes),
      detail,
      terminal_at: new Date().toISOString(),
    };
    body.entry_sha256 = sha256(canonicalBytes(body));
    try {
      await writeFile(registryTerminalPath, canonicalBytes(body), {mode: 0o600, flag: "wx"});
      externalTerminalCommitted = true;
    } catch (error) {
      if (error?.code === "EEXIST") externalTerminalCommitted = true;
      else throw error;
    }
  }
  // Registration is the point of no return. From this exact point onward,
  // every thrown setup error has an immutable external terminal disposition,
  // even if the run/artifact filesystems cannot be created.
  committedFailureHandler = async (error) => externalTerminalize("infrastructure_failed", {
    launch_count: 0,
    error: error instanceof Error ? error.message : String(error),
  });
  const registryIdentity = await fileIdentity(registryEntryPath);

  await mkdir(artifactDir, {mode: 0o700});
  await mkdir(runRoot, {mode: 0o700});
  const controllerBenchmarkDir = resolve(artifactDir, "controller-bundle");
  await cp(benchmarkDir, controllerBenchmarkDir, {recursive: true, force: false, errorOnExist: true});
  const infrastructure = await hashInfrastructureTree(controllerBenchmarkDir, true);
  const infrastructureManifestPath = resolve(artifactDir, "controller-bundle-manifest.json");
  await writeFile(infrastructureManifestPath, infrastructure.bytes, {mode: 0o600, flag: "wx"});
  async function verifyControllerBundle(label) {
    return assertControllerBundleIntact({
      root: controllerBenchmarkDir,
      manifestPath: infrastructureManifestPath,
      expectedSha256: infrastructure.sha256,
      expectedEntries: infrastructure.entries,
      label,
    });
  }
  await verifyControllerBundle("before loading controller modules");
  const {validateFormalAdapterPackage: validateBundledFormalAdapterPackage} = await import(
    pathToFileURL(resolve(controllerBenchmarkDir, "scripts/adapter-integrity.mjs")).href
  );
  const workspace = resolve(runRoot, "workspace");
  const baseWorkspace = resolve(runRoot, "control");
  const home = resolve(runRoot, "home");
  const codexHome = resolve(runRoot, "codex-home");
  const tmp = resolve(runRoot, "tmp");
  const cargoHome = resolve(runRoot, "cargo-home");
  const toolBin = resolve(runRoot, "tool-bin");
  await Promise.all([
    mkdir(home, {mode: 0o700}), mkdir(codexHome, {mode: 0o700}),
    mkdir(tmp, {mode: 0o700}),
    mkdir(resolve(artifactDir, "stage-receipts"), {mode: 0o700}),
  ]);
  if (args.has("cargo-home-template")) {
    const template = await requireDirectory(args.get("cargo-home-template"), "Cargo home template");
    if (formalAuthorization) {
      await cloneTreeCopyOnWrite(template, cargoHome);
      const stagedCargoHome = await hashContentTree(cargoHome);
      for (const key of ["digest_kind", "entries", "file_bytes", "manifest_sha256"]) {
        if (stagedCargoHome[key] !== formalCargoHomeInspection[key]) {
          fail(`staged Cargo home ${key} differs from its preregistered template`);
        }
      }
    } else {
      await cp(template, cargoHome, {recursive: true, force: false, errorOnExist: true});
    }
  } else await mkdir(cargoHome, {mode: 0o700});
  const stagedRuntime = await stageToolBin(toolBin, formalRuntime);
  const tools = Array.isArray(stagedRuntime) ? stagedRuntime : stagedRuntime.tools;
  const helperNodeExecutable = formalAuthorization
    ? tools.find((tool) => tool.name === "node")?.staged_path
    : process.execPath;
  if (!helperNodeExecutable) fail("staged controller Node executable is missing");
  const stagedModelCatalog = formalAuthorization
    ? await stageFormalModelCatalog({
      sourcePath: formalCatalogSource,
      destinationPath: stagedModelCatalogPath,
      catalogLock: frozenEnvironmentLock.controlled_agent.bundled_model_catalog,
      modelId: frozenEnvironmentLock.controlled_agent.model_id,
    })
    : null;
  const environment = sanitizeEnvironment(process.env, {
    HOME: home,
    CODEX_HOME: codexHome,
    TMPDIR: tmp,
    PATH: `${toolBin}:/usr/bin:/bin:/usr/sbin:/sbin`,
    CARGO_HOME: cargoHome,
    RUSTUP_HOME: formalAuthorization
      ? stagedRuntime.rustup_home_path
      : process.env.RUSTUP_HOME ?? resolve(process.env.HOME, ".rustup"),
    PNPM_HOME: formalAuthorization ? stagedRuntime.pnpm_home_path : toolBin,
  });
  delete environment.CARGO_TARGET_DIR;
  const formalRuntimePreflight = formalAuthorization
    ? await verifyStagedRuntimeArtifacts(stagedRuntime, frozenEnvironmentLock)
    : null;
  const formalRuntimePreflightPath = resolve(artifactDir, "formal-runtime-preflight.json");
  if (formalRuntimePreflight) {
    await writeFile(
      formalRuntimePreflightPath,
      canonicalBytes(formalRuntimePreflight),
      {mode: 0o600, flag: "wx"},
    );
  }
  const controls = await copyControls(artifactDir, controllerBenchmarkDir);
  const registeredVariantPath = resolve(artifactDir, "registered-variant.bin");
  const registeredTaskPath = resolve(artifactDir, "registered-task.txt");
  await writeFile(registeredVariantPath, variantBytes, {mode: 0o400, flag: "wx"});
  await writeFile(registeredTaskPath, frozenTaskBytes, {mode: 0o400, flag: "wx"});
  const common = {
    protocol_id: manifests[0].protocol_id,
    phase,
    classification: phase === "construction_pilot_only" ? "construction_pilot_only" : "formal_authorized_attempt",
    formal_result_eligible: phase !== "construction_pilot_only",
    wave_id: args.get("wave-id"),
    run_id: args.get("run-id"),
    attempt_id: args.get("attempt-id"),
    candidate_id: args.get("candidate-id"),
    case_id: caseId,
    control_sha256: controls.sha256,
    infrastructure_identity_sha256: infrastructure.sha256,
    attempt_registry_entry: registryIdentity,
    formal_authorization: authorizationIdentity,
    provider_auth_qualification_sha256: providerAuthQualificationIdentity?.sha256 ?? null,
    formal_runtime_identity_sha256: formalRuntime?.identity_sha256 ?? null,
    staged_model_catalog: stagedModelCatalog,
    effective_agent_args_sha256: formalEffectiveArguments
      ? sha256(canonicalBytes(formalEffectiveArguments))
      : sha256(canonicalBytes(agentArguments)),
  };
  const environmentObservation = {
    schema: "tachiko-controller-environment-v1",
    ...common,
    environment: Object.fromEntries(Object.keys(environment).sort().map((key) => [key, environment[key]])),
    tools,
    helper_node: await fileIdentity(helperNodeExecutable),
    formal_runtime: formalRuntime,
    formal_runtime_preflight: formalRuntimePreflight,
    network_sandbox: formalNetworkSandboxIdentity,
  };
  environmentObservation.environment_identity_sha256 = sha256(canonicalBytes(environmentObservation));
  common.environment_identity_sha256 = environmentObservation.environment_identity_sha256;
  const environmentReceiptPath = resolve(artifactDir, "environment-receipt.json");
  await writeFile(environmentReceiptPath, canonicalBytes(environmentObservation), {mode: 0o600, flag: "wx"});

  let trustedValidationGuard = null;
  async function verifyTrustedValidationGuard(label) {
    if (!trustedValidationGuard) return null;
    const observedReceipt = await fileIdentity(trustedValidationGuard.receiptPath);
    if (observedReceipt.sha256 !== trustedValidationGuard.receiptSha256) {
      fail(`trusted validation environment receipt changed ${label}`);
    }
    const observedCargo = await hashContentTree(trustedValidationGuard.cargoHome);
    for (const key of ["digest_kind", "entries", "file_bytes", "manifest_sha256"]) {
      if (observedCargo[key] !== trustedValidationGuard.cargoIdentity[key]) {
        fail(`trusted validation Cargo home ${key} changed ${label}`);
      }
    }
    await Promise.all([
      assertReadOnlyTree(trustedValidationGuard.cargoHome,
        "trusted validation Cargo home"),
      assertEmptyDirectory(trustedValidationGuard.home,
        "trusted validation HOME"),
      assertEmptyDirectory(trustedValidationGuard.codexHome,
        "trusted validation CODEX_HOME"),
    ]);
    return {
      label,
      receipt_sha256: observedReceipt.sha256,
      cargo_manifest_sha256: observedCargo.manifest_sha256,
      verified: true,
    };
  }

  let stageOrder = 0;
  let priorStageReceiptSha256 = null;
  const stageDir = resolve(artifactDir, "stage-receipts");
  const stageReceiptIdentities = [];
  async function writeStage(stage, payload, inputPaths = [], outputPaths = []) {
    const validationGuard = await verifyTrustedValidationGuard(`before stage ${stage}`);
    const bundleVerification = await verifyControllerBundle(`before stage ${stage}`);
    const [inputs, outputs] = await Promise.all([
      Promise.all(inputPaths.map((path) => fileIdentity(path))),
      Promise.all(outputPaths.map((path) => fileIdentity(path))),
    ]);
    const receipt = {
      schema: "tachiko-controller-stage-receipt-v1",
      ...common,
      stage,
      stage_order: stageOrder,
      prior_receipt_sha256: priorStageReceiptSha256,
      controller_bundle_verification: bundleVerification,
      trusted_validation_environment_verification: validationGuard,
      inputs,
      outputs,
      payload_sha256: sha256(canonicalBytes(payload)),
      payload,
      completed_at: new Date().toISOString(),
    };
    const path = resolve(stageDir, `${String(stageOrder).padStart(2, "0")}-${stage}.json`);
    const bytes = canonicalBytes(receipt);
    await writeFile(path, bytes, {mode: 0o600, flag: "wx"});
    stageReceiptIdentities.push({path, bytes: bytes.length, sha256: sha256(bytes)});
    priorStageReceiptSha256 = sha256(bytes);
    stageOrder += 1;
    return {path, bytes, receipt};
  }

  let terminalWritten = false;
  let launchCount = 0;
  const ledgerPath = resolve(artifactDir, "attempt-ledger.jsonl");
  const registrationBody = {
    schema: "tachiko-controller-attempt-entry-v1",
    ...common,
    disposition: "registered",
    attempt_number: 1,
    replacement_role: "initial",
    previous_attempt_entry_sha256: null,
    registered_at: new Date().toISOString(),
  };
  registrationBody.entry_sha256 = sha256(canonicalBytes(registrationBody));
  await writeFile(ledgerPath, `${JSON.stringify(registrationBody)}\n`, {mode: 0o600, flag: "wx"});
  async function terminalize(disposition, detail) {
    if (terminalWritten || existsSync(resolve(artifactDir, "terminal.json"))) fail("terminal outcome already exists");
    const terminal = {
      schema: "tachiko-controller-attempt-entry-v1",
      ...common,
      disposition,
      attempt_number: 1,
      previous_attempt_entry_sha256: registrationBody.entry_sha256,
      final_stage_receipt_sha256: priorStageReceiptSha256,
      resampling_performed: false,
      launch_count: detail.launch_count ?? 0,
      detail,
      terminal_at: new Date().toISOString(),
    };
    terminal.entry_sha256 = sha256(canonicalBytes(terminal));
    await commitTerminalEntry({
      ledgerPath,
      markerPath: resolve(artifactDir, "terminal.json"),
      terminal,
      onCommitted() { terminalWritten = true; },
    });
    await externalTerminalize(disposition, detail);
    return terminal;
  }
  committedFailureHandler = async (error) => {
    if (terminalWritten) return;
    try {
      await terminalize("infrastructure_failed", {
        launch_count: launchCount,
        error: error instanceof Error ? error.message : String(error),
      });
    } catch {
      await externalTerminalize("infrastructure_failed", {
        launch_count: launchCount,
        error: error instanceof Error ? error.message : String(error),
      });
    }
  };

  const helperProtectedRoots = [
    controllerBenchmarkDir,
    sourceRepo,
    attemptRegistryDir,
    stageDir,
  ];
  const helperProtectedPaths = [
    infrastructureManifestPath,
    registeredVariantPath,
    registeredTaskPath,
    environmentReceiptPath,
    formalRuntimePreflight ? formalRuntimePreflightPath : null,
    ledgerPath,
    registryEntryPath,
    variantFile,
    agentArgsFile,
    authorizationIdentity?.path,
    providerAuthQualificationIdentity?.path,
    operatorKeychain,
    formalCatalogSource,
  ].filter(Boolean);
  async function runBundledHelper(relativeScript, helperArguments, options = {}, {
    protectCandidateWorkspace = true,
    extraProtectedRoots = [],
    extraProtectedPaths = [],
    nestedProcessSupervisor = false,
  } = {}) {
    await verifyTrustedValidationGuard(`before trusted helper ${relativeScript}`);
    await verifyControllerBundle(`before trusted helper ${relativeScript}`);
    const writePolicy = trustedHelperWriteProfile({
      protectedRoots: [
        ...helperProtectedRoots,
        ...(protectCandidateWorkspace ? [workspace] : []),
        ...extraProtectedRoots,
      ],
      protectedPaths: [...helperProtectedPaths, ...extraProtectedPaths],
    });
    const protectionArgument = JSON.stringify({
      schema: "tachiko-supervised-write-protection-v1",
      protected_roots: writePolicy.roots,
      protected_paths: writePolicy.paths,
    });
    const result = nestedProcessSupervisor
      ? command(helperNodeExecutable, [
        resolve(controllerBenchmarkDir, relativeScript),
        ...helperArguments,
        "--supervised-write-protection-json", protectionArgument,
      ], options)
      : command("/usr/bin/sandbox-exec", [
        "-p", writePolicy.profile,
        helperNodeExecutable,
        resolve(controllerBenchmarkDir, relativeScript),
        ...helperArguments,
    ], options);
    await verifyTrustedValidationGuard(`after trusted helper ${relativeScript}`);
    await verifyControllerBundle(`after trusted helper ${relativeScript}`);
    result.controller_bundle_write_protection = {
      schema: "tachiko-trusted-helper-write-protection-v1",
      protected_roots: writePolicy.roots,
      protected_paths: writePolicy.paths,
      profile_sha256: writePolicy.profile_sha256,
      active: true,
      enforcement_scope: nestedProcessSupervisor
        ? "every_inner_coalition_profile"
        : "outer_trusted_helper_process",
    };
    return result;
  }

  try {
    await writeStage(
      "attempt_registration",
      registrationBody,
      [registryEntryPath, registeredVariantPath, registeredTaskPath, agentExecutable, agentArgsFile],
      [
        ledgerPath,
        environmentReceiptPath,
        infrastructureManifestPath,
        ...(formalRuntimePreflight ? [formalRuntimePreflightPath] : []),
      ],
    );

    const basePreparationDir = resolve(artifactDir, "base-preparation");
    const basePreparation = await prepareBaseWorkspace(
      sourceRepo,
      caseEntry.historical_base_commit,
      caseEntry.historical_base_tree,
      caseEntry.ground_truth_commit,
      baseWorkspace,
      basePreparationDir,
      environment,
    );
    const basePreparationReceiptPath = resolve(basePreparationDir, "receipt.json");
    await writeFile(basePreparationReceiptPath, canonicalBytes({...common, ...basePreparation}), {mode: 0o600, flag: "wx"});
    await writeStage("base_workspace_preparation", basePreparation, [environmentReceiptPath], [basePreparationReceiptPath]);

    const baseReceiptPath = resolve(artifactDir, "base-control-receipt.json");
    const trustedBaseShell = tools.find((tool) => tool.name === "bash");
    if (!trustedBaseShell) fail("registered Bash is missing before same-wave base controls");
    const baseArguments = [
      resolve(controllerBenchmarkDir, "scripts/capture-base-control-evidence.mjs"),
      "--case", caseId,
      "--workspace", baseWorkspace,
      "--receipt", baseReceiptPath,
      "--log-dir", resolve(artifactDir, "base-control-logs"),
      "--controller-bound", "true",
      "--phase", phase,
      "--wave-id", common.wave_id,
      "--run-id", common.run_id,
      "--attempt-id", common.attempt_id,
      "--candidate-id", common.candidate_id,
      "--control-sha256", common.control_sha256,
      "--environment-receipt", environmentReceiptPath,
      "--trusted-shell", trustedBaseShell.launcher_path ?? trustedBaseShell.staged_path,
      "--expected-shell-sha256", trustedBaseShell.sha256,
      ...(constructionSmoke ? ["--construction-smoke", "true"] : []),
    ];
    const baseResult = await runBundledHelper(
      "scripts/capture-base-control-evidence.mjs",
      baseArguments.slice(1),
      {env: environment, allowFailure: true},
      {protectCandidateWorkspace: false, nestedProcessSupervisor: true},
    );
    if (!existsSync(baseReceiptPath)) fail(`base control failed without a receipt: ${baseResult.stderr}`);
    const baseReceipt = JSON.parse(await readFile(baseReceiptPath, "utf8"));
    if (!baseReceipt.all_commands_passed) fail("same-wave base controls failed");
    await writeStage("same_wave_base_control", baseReceipt, [environmentReceiptPath], [baseReceiptPath]);

    const candidatePreparationDir = resolve(artifactDir, "candidate-preparation");
    const prepareResult = await runBundledHelper("scripts/prepare-case.mjs", [
      "--case", caseId,
      "--source-repo", sourceRepo,
      "--variant-file", registeredVariantPath,
      "--workspace", workspace,
      "--trusted-dir", candidatePreparationDir,
      "--expected-variant-sha256", args.get("expected-variant-sha256"),
    ], {env: environment, allowFailure: true}, {protectCandidateWorkspace: false});
    const preparationReceiptPath = resolve(candidatePreparationDir, "preparation-receipt.json");
    if (prepareResult.status !== 0 || !existsSync(preparationReceiptPath)) {
      fail(`candidate preparation failed: ${prepareResult.stderr}`);
    }
    await writeStage("candidate_workspace_preparation", JSON.parse(await readFile(preparationReceiptPath, "utf8")), [registeredVariantPath], [preparationReceiptPath]);

    // Trusted Git/rtk preparation can create controller caches under HOME on
    // macOS. They are not agent inputs: erase them before the neutral preflight
    // and launch, then require either the exact empty-tree check or the formal
    // credential-free Keychain metadata identity.
    await Promise.all([emptyOwnedDirectory(home), emptyOwnedDirectory(codexHome)]);
    let formalKeyringMetadata = null;
    if (formalAuthorization) {
      formalKeyringMetadata = await prepareFreshHomeForKeyring({
        home,
        keychainPath: operatorKeychain,
      });
      if (JSON.stringify(formalKeyringMetadata.metadata) !==
          JSON.stringify(providerAuthQualification.keychain_metadata)) {
        fail("fresh HOME Keychain metadata differs from the provider auth qualification");
      }
      if (formalKeyringMetadata.keychain_path_sha256 !==
          providerAuthQualification.keychain_path_sha256) {
        fail("fresh HOME keychain path differs from the provider auth qualification");
      }
    }

    const overlayPath = resolve(workspace, "AGENTS.md");
    const overlayBytes = await readFile(overlayPath);
    const overlayStat = await lstat(overlayPath, {bigint: true});
    const expectedOverlayIdentity = overlayIdentity(overlayStat, overlayBytes);
    const overlayIdentityPath = resolve(artifactDir, "overlay-identity.json");
    await writeFile(overlayIdentityPath, canonicalBytes(expectedOverlayIdentity), {mode: 0o600, flag: "wx"});

    const preflightReceiptPath = resolve(artifactDir, "preflight-receipt.json");
    const preflight = await runBundledHelper("scripts/preflight-run.mjs", [
      "--workspace", workspace,
      "--home", home,
      "--codex-home", codexHome,
      "--artifact-dir", artifactDir,
      "--receipt", preflightReceiptPath,
      "--expected-agents-sha256", args.get("expected-variant-sha256"),
      "--expected-control-sha256", controls.sha256,
      ...(formalKeyringMetadata ? [
        "--expected-keychain-metadata-sha256", formalKeyringMetadata.metadata.sha256,
        "--expected-keychain-metadata-bytes", String(formalKeyringMetadata.metadata.bytes),
      ] : []),
    ], {env: environment, allowFailure: true});
    if (preflight.status !== 0 || !existsSync(preflightReceiptPath)) fail(`candidate preflight failed: ${preflight.stderr}`);
    const preflightReceipt = JSON.parse(await readFile(preflightReceiptPath, "utf8"));
    if (preflightReceipt.valid !== true) fail("candidate preflight receipt is invalid");
    const freeSpace = requireFormalFreeSpace(
      preflightReceipt,
      frozenEnvironmentLock ?? JSON.parse(await readFile(resolve(controllerBenchmarkDir, "environment-lock.json"), "utf8")),
      Boolean(formalAuthorization),
    );
    const toolIdentityComparison = await comparePreflightToolIdentities(
      tools,
      preflightReceipt,
      Boolean(formalAuthorization),
    );
    await writeStage("candidate_preflight", {
      valid: true,
      tool_identity_comparison: toolIdentityComparison,
      model_catalog_identity_verified: stagedModelCatalog !== null,
      code_mode_host_identity_verified: formalRuntime !== null,
      free_space: freeSpace,
      provider_auth_mode: preflightReceipt.provider_auth?.mode ?? "none",
    }, [environmentReceiptPath], [preflightReceiptPath]);

    let providerAuthStatus = null;
    if (formalAuthorization) {
      providerAuthStatus = await verifyChatGptKeyringStatus({
        codexExecutable: agentExecutable,
        codexHome,
        environment,
      });
      if (providerAuthStatus.keyring_account !== providerAuthQualification.keyring_account) {
        fail("formal keyring account differs from the provider auth qualification");
      }
      await writeStage("provider_auth_preflight", {
        method: providerAuthStatus.method,
        mode: providerAuthStatus.mode,
        keyring_account: providerAuthStatus.keyring_account,
        auth_json_present: providerAuthStatus.auth_json_present,
        qualification_sha256: providerAuthQualificationIdentity.sha256,
        keychain_path_sha256: formalKeyringMetadata.keychain_path_sha256,
        metadata_sha256: formalKeyringMetadata.metadata.sha256,
      }, [preflightReceiptPath, providerAuthQualificationIdentity.path], []);
    }

    const effectiveAgentArguments = formalEffectiveArguments ?? agentArguments;
    const formalCodeModeHost = formalRuntime?.tools.find((tool) => tool.name === "codex-code-mode-host") ?? null;
    if (formalCodeModeHost) {
      const observedCodeModeHost = await fileIdentity(formalCodeModeHost.source_path);
      if (observedCodeModeHost.bytes !== formalCodeModeHost.bytes ||
          observedCodeModeHost.sha256 !== formalCodeModeHost.sha256) {
        fail("formal code-mode host changed before agent launch");
      }
    }
    const candidateAccess = candidateAccessProfile({
      protectedRoots: [
        artifactDir,
        attemptRegistryDir,
        sourceRepo,
        baseWorkspace,
        formalRustupHomeTemplate,
        formalCargoHomeTemplate,
      ].filter(Boolean),
      protectedPaths: [
        variantFile,
        agentArgsFile,
        authorizationIdentity?.path,
        providerAuthQualificationIdentity?.path,
        formalCatalogSource,
      ].filter(Boolean),
      restrictedRoots: [runRoot],
      allowedReadRoots: [
        workspace,
        home,
        codexHome,
        tmp,
        cargoHome,
        toolBin,
        dirname(stagedModelCatalogPath),
        stagedRuntime.rustup_home_path,
        stagedRuntime.pnpm_home_path,
      ].filter(Boolean),
      allowedWriteRoots: [workspace, home, codexHome, tmp, cargoHome],
    });
    await writeStage("agent_launch", {
      spawn_count_before_stage: 0,
      executable: agentExecutableIdentity,
      arguments_file: agentArgumentsIdentity,
      effective_arguments_sha256: sha256(canonicalBytes(effectiveAgentArguments)),
      task_sha256: caseEntry.task_sha256,
      timeout_seconds: timeoutSeconds,
      termination_grace_seconds: terminationGraceSeconds,
      base_control_stage_order: 2,
      provider_auth: providerAuthStatus,
      candidate_access: {
        protected_roots: candidateAccess.protected_roots,
        protected_paths: candidateAccess.protected_paths,
        restricted_roots: candidateAccess.restricted_roots,
        allowed_read_roots: candidateAccess.allowed_read_roots,
        allowed_write_roots: candidateAccess.allowed_write_roots,
        profile_sha256: candidateAccess.profile_sha256,
        bundle_and_trusted_artifacts_denied: true,
      },
    }, [preflightReceiptPath, baseReceiptPath,
      ...(providerAuthQualificationIdentity ? [providerAuthQualificationIdentity.path] : [])], []);

    const taskBytes = await readFile(registeredTaskPath);
    launchCount = 1;
    const processResult = await runAgentOnce(
      agentExecutable,
      effectiveAgentArguments,
      workspace,
      environment,
      taskBytes,
      timeoutSeconds * 1000,
      terminationGraceSeconds * 1000,
      candidateAccess.profile,
    );
    if (!processResult.process_group_extinct_before_capture) {
      fail("agent coalition was not extinct before trusted post-run verification");
    }
    await verifyControllerBundle("after agent extinction before trusted capture");
    processResult.process_containment.candidate_trusted_roots_denied = true;
    processResult.process_containment.candidate_access = {
      protected_roots: candidateAccess.protected_roots,
      protected_paths: candidateAccess.protected_paths,
      restricted_roots: candidateAccess.restricted_roots,
      allowed_read_roots: candidateAccess.allowed_read_roots,
      allowed_write_roots: candidateAccess.allowed_write_roots,
      profile_sha256: candidateAccess.profile_sha256,
    };
    if (JSON.stringify(await fileIdentity(agentExecutable)) !== JSON.stringify(agentExecutableIdentity) ||
        JSON.stringify(await fileIdentity(agentArgsFile)) !== JSON.stringify(agentArgumentsIdentity)) {
      fail("agent executable or argument bytes changed during the one-shot launch");
    }
    if (formalCodeModeHost) {
      const observedCodeModeHost = await fileIdentity(formalCodeModeHost.source_path);
      if (observedCodeModeHost.bytes !== formalCodeModeHost.bytes ||
          observedCodeModeHost.sha256 !== formalCodeModeHost.sha256) {
        fail("formal code-mode host changed during the one-shot launch");
      }
    }
    const processDir = resolve(artifactDir, "process");
    await mkdir(processDir, {mode: 0o700});
    const stdoutPath = resolve(processDir, "stdout.raw");
    const stderrPath = resolve(processDir, "stderr.raw");
    const finalMessagePath = resolve(processDir, "final-message.txt");
    const finalMessage = parseFinalMessage(processResult.stdout);
    await Promise.all([
      writeFile(stdoutPath, processResult.stdout, {mode: 0o600, flag: "wx"}),
      writeFile(stderrPath, processResult.stderr, {mode: 0o600, flag: "wx"}),
      writeFile(finalMessagePath, finalMessage === "" ? "" : `${finalMessage}\n`, {mode: 0o600, flag: "wx"}),
    ]);
    const processReceipt = {
      schema: "tachiko-controller-process-v1",
      ...common,
      spawn_count: 1,
      resampling_performed: false,
      deadline_seconds: timeoutSeconds,
      ...Object.fromEntries(Object.entries(processResult).filter(([key]) => !["stdout", "stderr"].includes(key))),
      stdout: await fileIdentity(stdoutPath),
      stderr: await fileIdentity(stderrPath),
      final_message: await fileIdentity(finalMessagePath),
    };
    const processReceiptPath = resolve(processDir, "receipt.json");
    await writeFile(processReceiptPath, canonicalBytes(processReceipt), {mode: 0o600, flag: "wx"});
    await writeStage("agent_process", processReceipt, [preflightReceiptPath], [stdoutPath, stderrPath, finalMessagePath, processReceiptPath]);

    const overlayAfterBytes = await readFile(overlayPath);
    const overlayAfterStat = await lstat(overlayPath, {bigint: true});
    const overlayAfter = overlayIdentity(overlayAfterStat, overlayAfterBytes);
    if (JSON.stringify(overlayAfter) !== JSON.stringify(expectedOverlayIdentity)) {
      fail("post-run overlay identity changed");
    }
    await writeStage("overlay_identity_postcheck", {identity_equal: true, overlay: overlayAfter}, [overlayIdentityPath], []);

    if (processResult.spawn_error) {
      const resultSkeleton = {
        schema: "tachiko-controller-result-skeleton-v1",
        ...common,
        formal_result_eligible: false,
        formal_attempt_authorized: common.formal_result_eligible,
        disposition: "agent_failed",
        no_resampling: true,
        launch_count: 1,
        candidate_commit: null,
        candidate_tree: null,
        raw_tree_digest_sha256: null,
        process_receipt_sha256: sha256(await readFile(processReceiptPath)),
        scores_recorded: false,
        semantic_review_pending: false,
        failure_stage: "agent_spawn",
      };
      const resultPath = resolve(artifactDir, "result-skeleton.json");
      await writeFile(resultPath, canonicalBytes(resultSkeleton), {mode: 0o600, flag: "wx"});
      await writeStage("result_skeleton", resultSkeleton, [processReceiptPath], [resultPath]);
      await terminalize("agent_failed", {
        launch_count: 1,
        spawn_error: processResult.spawn_error,
        result_skeleton_sha256: sha256(await readFile(resultPath)),
      });
      console.log(JSON.stringify({artifact_dir: artifactDir, disposition: "agent_failed", launch_count: 1}));
      process.exitCode = 1;
      return;
    }

    // Candidate-writable HOME/CARGO_HOME/TMP are launch-only inputs. Trusted
    // validation starts from a new tree created after coalition extinction;
    // no candidate process can have retained a descriptor into it.
    const validationEnvironmentRoot = resolve(runRoot, "trusted-validation");
    const validationHome = resolve(validationEnvironmentRoot, "home");
    const validationCodexHome = resolve(validationEnvironmentRoot, "codex-home");
    const validationTmp = resolve(validationEnvironmentRoot, "tmp");
    const validationCargoHome = resolve(validationEnvironmentRoot, "cargo-home");
    await mkdir(validationEnvironmentRoot, {mode: 0o700});
    await Promise.all([
      mkdir(validationHome, {mode: 0o700}),
      mkdir(validationCodexHome, {mode: 0o700}),
      mkdir(validationTmp, {mode: 0o700}),
    ]);
    if (formalCargoHomeTemplate) {
      await cloneTreeCopyOnWrite(formalCargoHomeTemplate, validationCargoHome);
    } else {
      await mkdir(validationCargoHome, {mode: 0o700});
    }
    for (const name of ["config", "config.toml"]) {
      if (existsSync(resolve(validationCargoHome, name))) {
        fail(`trusted validation Cargo home contains forbidden ${name}`);
      }
    }
    const validationCargoIdentity = await hashContentTree(validationCargoHome);
    if (formalAuthorization) {
      for (const key of ["digest_kind", "entries", "file_bytes", "manifest_sha256"]) {
        if (validationCargoIdentity[key] !== formalCargoHomeInspection[key]) {
          fail(`trusted validation Cargo home ${key} differs from its preregistered template`);
        }
      }
    }
    await sealReadOnlyTree(validationCargoHome);
    await Promise.all([
      assertEmptyDirectory(validationHome, "trusted validation HOME"),
      assertEmptyDirectory(validationCodexHome, "trusted validation CODEX_HOME"),
      assertEmptyDirectory(validationTmp, "trusted validation TMP"),
      chmod(validationHome, 0o500),
      chmod(validationCodexHome, 0o500),
    ]);
    const validationEnvironment = sanitizeEnvironment(process.env, {
      HOME: validationHome,
      CODEX_HOME: validationCodexHome,
      TMPDIR: validationTmp,
      PATH: `${toolBin}:/usr/bin:/bin:/usr/sbin:/sbin`,
      CARGO_HOME: validationCargoHome,
      RUSTUP_HOME: environment.RUSTUP_HOME,
      PNPM_HOME: environment.PNPM_HOME,
    });
    const validationEnvironmentObservation = {
      schema: "tachiko-controller-trusted-validation-environment-v1",
      ...common,
      created_after_agent_extinction: true,
      agent_environment_inherited: false,
      candidate_environment_identity_sha256: common.environment_identity_sha256,
      environment: Object.fromEntries(Object.keys(validationEnvironment).sort()
        .map((key) => [key, validationEnvironment[key]])),
      pristine_home: true,
      pristine_codex_home: true,
      pristine_tmp: true,
      cargo_home_template_verified: true,
      cargo_home_read_only: true,
      cargo_home: validationCargoIdentity,
      cargo_template: formalCargoHomeInspection ?? null,
      stage_tmp_policy: "fresh_disjoint_tmp_per_candidate-executing_validation_stage",
    };
    validationEnvironmentObservation.validation_environment_identity_sha256 = sha256(
      canonicalBytes(validationEnvironmentObservation),
    );
    const validationEnvironmentReceiptPath = resolve(
      artifactDir,
      "trusted-validation-environment.json",
    );
    await writeFile(
      validationEnvironmentReceiptPath,
      canonicalBytes(validationEnvironmentObservation),
      {mode: 0o400, flag: "wx"},
    );
    trustedValidationGuard = {
      receiptPath: validationEnvironmentReceiptPath,
      receiptSha256: sha256(await readFile(validationEnvironmentReceiptPath)),
      cargoHome: validationCargoHome,
      cargoIdentity: validationCargoIdentity,
      home: validationHome,
      codexHome: validationCodexHome,
    };
    await verifyTrustedValidationGuard("immediately after trusted validation creation");

    async function freshValidationStageEnvironment(label) {
      const path = await mkdtemp(resolve(validationEnvironmentRoot, `${label}-tmp-`));
      await chmod(path, 0o700);
      await assertEmptyDirectory(path, `trusted validation ${label} TMP`);
      return {...validationEnvironment, TMPDIR: path, TMP: path, TEMP: path};
    }

    const exclusionsPath = resolve(artifactDir, "capture-exclusions.json");
    await writeFile(exclusionsPath, `${JSON.stringify(["target", "node_modules", ".pnpm-store"], null, 2)}\n`, {mode: 0o600, flag: "wx"});
    const captureDir = resolve(artifactDir, "candidate-capture");
    const captureResult = await runBundledHelper("scripts/capture-candidate.mjs", [
      "--case", caseId,
      "--workspace", workspace,
      "--source-repo", sourceRepo,
      "--exclusions-file", exclusionsPath,
      "--expected-agents-identity-file", overlayIdentityPath,
      "--trusted-dir", captureDir,
      "--expected-agents-sha256", args.get("expected-variant-sha256"),
    ], {env: await freshValidationStageEnvironment("capture"), allowFailure: true}, {
      extraProtectedRoots: [validationCargoHome, validationHome, validationCodexHome],
    });
    const captureReceiptPath = resolve(captureDir, "capture-receipt.json");
    if (captureResult.status !== 0 || !existsSync(captureReceiptPath)) fail(`candidate capture failed: ${captureResult.stderr}`);
    const captureReceipt = JSON.parse(await readFile(captureReceiptPath, "utf8"));
    if (!captureReceipt.trusted_raw_capture || !captureReceipt.round_trip?.equal) fail("candidate capture is not trusted");
    await writeStage("candidate_capture", {
      candidate_commit: captureReceipt.candidate_commit,
      candidate_tree: captureReceipt.candidate_tree,
      raw_tree_digest_sha256: captureReceipt.raw_tree_digest_sha256,
      trusted_validation_environment_sha256: sha256(
        await readFile(validationEnvironmentReceiptPath),
      ),
    }, [overlayIdentityPath, exclusionsPath, validationEnvironmentReceiptPath], [captureReceiptPath, resolve(captureDir, "candidate.patch"), resolve(captureDir, "raw-manifest.json")]);

    const validationWorkspace = resolve(artifactDir, "validation-workspace");
    const validationPreparationDir = resolve(artifactDir, "validation-preparation");
    const validationResult = await runBundledHelper("scripts/prepare-validation.mjs", [
      "--case", caseId,
      "--source-repo", sourceRepo,
      "--patch-file", resolve(captureDir, "candidate.patch"),
      "--capture-receipt", captureReceiptPath,
      "--workspace", validationWorkspace,
      "--trusted-dir", validationPreparationDir,
    ], {env: await freshValidationStageEnvironment("prepare"), allowFailure: true,
    }, {extraProtectedRoots: [captureDir, validationCargoHome, validationHome,
      validationCodexHome]});
    const validationReceiptPath = resolve(validationPreparationDir, "validation-preparation-receipt.json");
    if (validationResult.status !== 0 || !existsSync(validationReceiptPath)) {
      fail(`validation preparation failed: ${validationResult.stderr}`);
    }
    const validationReceipt = JSON.parse(await readFile(validationReceiptPath, "utf8"));
    await writeStage("validation_preparation", validationReceipt, [captureReceiptPath], [validationReceiptPath]);

    const coreTmp = await mkdtemp(resolve(validationEnvironmentRoot, "core-tmp-"));
    const coreTarget = resolve(validationEnvironmentRoot, "core-target");
    await mkdir(coreTarget, {mode: 0o700});
    const coreEnvironment = {
      ...validationEnvironment,
      TMPDIR: coreTmp,
      TMP: coreTmp,
      TEMP: coreTmp,
      CARGO_TARGET_DIR: coreTarget,
    };
    const coreAccess = candidateAccessProfile({
      protectedRoots: [
        controllerBenchmarkDir,
        attemptRegistryDir,
        sourceRepo,
        baseWorkspace,
        workspace,
        captureDir,
        stageDir,
        processDir,
        validationPreparationDir,
        resolve(artifactDir, "evaluator"),
      ],
      protectedPaths: helperProtectedPaths,
      restrictedRoots: [artifactDir, runRoot],
      allowedReadRoots: [
        validationWorkspace, validationEnvironmentRoot, validationCargoHome,
        validationHome, validationCodexHome, coreTmp, coreTarget, toolBin,
        validationEnvironment.RUSTUP_HOME, validationEnvironment.PNPM_HOME,
      ].filter(Boolean),
      allowedWriteRoots: [validationWorkspace, coreTmp, coreTarget],
      writeProtectedRoots: [
        validationCargoHome, validationHome, validationCodexHome, toolBin,
        validationEnvironment.RUSTUP_HOME, validationEnvironment.PNPM_HOME,
      ].filter(Boolean),
      baseProfile: DENY_NETWORK_PROFILE,
    });
    await verifyTrustedValidationGuard("before candidate core validation");
    await verifyControllerBundle("before candidate core validation");
    const core = await runCoreValidation(
      caseId,
      validationWorkspace,
      resolve(artifactDir, "core-validation"),
      coreEnvironment,
      constructionSmoke,
      common,
      controllerBenchmarkDir,
      formalAuthorization ? tools.find((tool) => tool.name === "bash")?.staged_path : "/bin/bash",
      coreAccess.profile,
    );
    await verifyTrustedValidationGuard("after candidate core validation");
    await writeStage("core_validation", {
      all_commands_passed: core.receipt.all_commands_passed,
      construction_smoke: constructionSmoke,
      trusted_validation_environment_sha256: sha256(
        await readFile(validationEnvironmentReceiptPath),
      ),
      candidate_environment_inherited: false,
      cargo_home_read_only: true,
    }, [validationReceiptPath, validationEnvironmentReceiptPath], [core.receiptPath]);

    // The adapter may only use a directory created after the agent and all
    // candidate core processes are extinct. The earlier per-run TMP remains a
    // read/write-denied input so candidate-controlled residue cannot influence
    // adapter observations.
    const adapterTmp = await mkdtemp(resolve(runRoot, "adapter-tmp-"));
    await chmod(adapterTmp, 0o700);
    if ((await readdir(adapterTmp)).length !== 0) fail("fresh adapter TMP is not empty");
    const adapterTmpInitialSha256 = sha256("[]\n");
    const controllerContextPath = resolve(artifactDir, "controller-evidence-context.json");
    const controllerContext = {
      schema: "tachiko-controller-evidence-context-v1",
      protocol_id: common.protocol_id,
      phase: common.phase,
      classification: common.classification,
      formal_result_eligible: common.formal_result_eligible,
      wave_id: common.wave_id,
      run_id: common.run_id,
      attempt_id: common.attempt_id,
      candidate_id: common.candidate_id,
      case_id: common.case_id,
      capture_receipt_sha256: sha256(await readFile(captureReceiptPath)),
      candidate_tree: captureReceipt.candidate_tree,
      raw_tree_digest_sha256: captureReceipt.raw_tree_digest_sha256,
      formal_authorization_sha256: common.formal_authorization?.sha256 ?? null,
      provider_auth_qualification_sha256:
        common.provider_auth_qualification_sha256,
      trusted_validation_environment_sha256: sha256(
        await readFile(validationEnvironmentReceiptPath),
      ),
      trusted_validation_cargo_manifest_sha256:
        validationCargoIdentity.manifest_sha256,
      adapter_forbidden_roots: [
        sourceRepo, artifactDir, workspace, baseWorkspace, controllerBenchmarkDir,
        home, codexHome, tmp, cargoHome,
        validationHome, validationCodexHome, validationCargoHome,
      ],
      adapter_write_forbidden_roots: [runRoot],
      adapter_write_allowed_roots: [adapterTmp],
      adapter_tmp_initial_sha256: adapterTmpInitialSha256,
    };
    const controllerContextBytes = canonicalBytes(controllerContext);
    await writeFile(controllerContextPath, controllerContextBytes, {mode: 0o400, flag: "wx"});
    const controllerContextSha256 = sha256(controllerContextBytes);
    const contextStage = await writeStage(
      "controller_evidence_context",
      {controller_context_sha256: controllerContextSha256,
        adapter_tmp_initial_sha256: adapterTmpInitialSha256},
      [captureReceiptPath, core.receiptPath,
        ...(common.formal_authorization ? [common.formal_authorization.path] : [])],
      [controllerContextPath],
    );
    let controllerIssuancePath = null;
    let controllerIssuanceSha256 = null;
    if (formalAuthorization) {
      controllerIssuancePath = resolve(artifactDir, "controller-context-issuance.json");
      const issuanceBody = {
        schema: "tachiko-controller-context-issuance-v1",
        protocol_id: common.protocol_id,
        phase: common.phase,
        classification: common.classification,
        formal_result_eligible: common.formal_result_eligible,
        wave_id: common.wave_id,
        run_id: common.run_id,
        attempt_id: common.attempt_id,
        candidate_id: common.candidate_id,
        case_id: common.case_id,
        artifact_dir: artifactDir,
        context: await fileIdentity(controllerContextPath),
        formal_authorization: authorizationIdentity,
        attempt_registry_entry: registryIdentity,
        attempt_ledger: await fileIdentity(ledgerPath),
        controller_bundle_manifest: await fileIdentity(infrastructureManifestPath),
        capture_receipt: await fileIdentity(captureReceiptPath),
        stage_receipts: [...stageReceiptIdentities],
        context_stage_receipt_sha256: sha256(contextStage.bytes),
        issued_at: new Date().toISOString(),
      };
      const issuance = formalControllerIssuanceRecord({
        body: issuanceBody,
        authorizationToken: formalAuthorization.authorization_token,
      });
      const issuanceBytes = canonicalBytes(issuance);
      await writeFile(controllerIssuancePath, issuanceBytes, {mode: 0o400, flag: "wx"});
      controllerIssuanceSha256 = sha256(issuanceBytes);
      await writeStage(
        "controller_context_issuance",
        {controller_issuance_sha256: controllerIssuanceSha256},
        [controllerContextPath, authorizationIdentity.path, registryEntryPath, ledgerPath,
          infrastructureManifestPath, captureReceiptPath],
        [controllerIssuancePath],
      );
    }

    const needsAdapter = productionCase.oracle_commands.some((entry) => entry.command_template.includes("<trusted-adapter-file>"));
    const adapterInputsReady = common.formal_result_eligible
      ? args.has("adapter-config") && args.has("adapter-integrity-receipt") &&
        args.has("expected-adapter-integrity-sha256") &&
        (caseId !== "TW-09" || args.has("adapter-probe-source"))
      : args.has("adapter-file") && args.has("expected-adapter-sha256");
    if (needsAdapter && !adapterInputsReady) {
      const pause = {
        schema: "tachiko-controller-adapter-pause-v1",
        ...common,
        disposition: "awaiting_trusted_adapter",
        launch_count: 1,
        resampling_performed: false,
        candidate_capture_receipt_sha256: sha256(await readFile(captureReceiptPath)),
        validation_receipt_sha256: sha256(await readFile(validationReceiptPath)),
        resume_rule: "resume this immutable attempt from production_oracles without relaunch",
        artifact_dir: artifactDir,
        validation_workspace: validationWorkspace,
        core_receipt_path: core.receiptPath,
        capture_receipt_path: captureReceiptPath,
        capture_dir: captureDir,
        validation_receipt_path: validationReceiptPath,
        validation_environment_receipt_path: validationEnvironmentReceiptPath,
        validation_environment_root: validationEnvironmentRoot,
        process_receipt_path: processReceiptPath,
        final_message_path: finalMessagePath,
        preflight_receipt_path: preflightReceiptPath,
        base_control_receipt_path: baseReceiptPath,
        registered_variant_path: registeredVariantPath,
        registered_variant_sha256: sha256(variantBytes),
        registered_task_path: registeredTaskPath,
        registered_task_sha256: sha256(frozenTaskBytes),
        controller_benchmark_dir: controllerBenchmarkDir,
        controller_bundle_sha256: infrastructure.sha256,
        source_repo: sourceRepo,
        run_root: runRoot,
        original_candidate_workspace: workspace,
        source_repo_identity: captureReceipt.source_repo,
        controller_context_path: controllerContextPath,
        controller_context_sha256: controllerContextSha256,
        controller_issuance_path: controllerIssuancePath,
        controller_issuance_sha256: controllerIssuanceSha256,
        frozen_at: registrationBody.registered_at,
        construction_smoke: constructionSmoke,
        bound_receipts: {
          preflight_sha256: sha256(await readFile(preflightReceiptPath)),
          base_control_sha256: sha256(await readFile(baseReceiptPath)),
          process_sha256: sha256(await readFile(processReceiptPath)),
          capture_sha256: sha256(await readFile(captureReceiptPath)),
          validation_sha256: sha256(await readFile(validationReceiptPath)),
          validation_environment_sha256: sha256(
            await readFile(validationEnvironmentReceiptPath),
          ),
          core_sha256: sha256(await readFile(core.receiptPath)),
        },
      };
      const pausePath = resolve(artifactDir, "awaiting-trusted-adapter.json");
      await writeFile(pausePath, canonicalBytes(pause), {mode: 0o600, flag: "wx"});
      await writeStage("awaiting_trusted_adapter", pause, [captureReceiptPath, validationReceiptPath], [pausePath]);
      console.log(JSON.stringify({artifact_dir: artifactDir, disposition: "awaiting_trusted_adapter"}));
      process.exitCode = 3;
      return;
    }

    const oracleDir = resolve(artifactDir, "production-oracles");
    let oracleReceiptPath;
    let adapterPackage = null;
    if (constructionSmoke) {
      await mkdir(oracleDir, {mode: 0o700});
      const oracleReceipt = {
        schema: "tachiko-controller-oracle-smoke-v1",
        ...common,
        construction_smoke: true,
        commands_executed: false,
        production_runner: await fileIdentity(resolve(controllerBenchmarkDir, "scripts/run-oracles.mjs")),
        case_command_count: productionCase.oracle_commands.length,
        case_assertion_count: productionCase.assertions.length,
        assessment_mode: productionCase.assertions.length === 0 ? "subjective_only_packet_gate" : "machine_and_or_subjective",
      };
      oracleReceiptPath = resolve(oracleDir, "oracle-run.json");
      await writeFile(oracleReceiptPath, canonicalBytes(oracleReceipt), {mode: 0o600, flag: "wx"});
    } else {
      const cargo = preflightReceipt.binaries.cargo;
      const rustc = preflightReceipt.binaries.rustc;
      const oracleArguments = [
        resolve(controllerBenchmarkDir, "scripts/run-oracles.mjs"),
        "--case", caseId,
        "--candidate-root", validationWorkspace,
        "--trusted-dir", oracleDir,
        "--expected-control-sha256", controls.sha256,
        "--trusted-shell", preflightReceipt.binaries.bash.path,
        "--expected-shell-sha256", preflightReceipt.binaries.bash.sha256,
        "--trusted-cargo", cargo.path,
        "--expected-cargo-sha256", cargo.sha256,
        "--trusted-rustc", rustc.path,
        "--expected-rustc-sha256", rustc.sha256,
        "--candidate-commit", validationReceipt.candidate_commit,
        "--trusted-validation-environment-receipt", validationEnvironmentReceiptPath,
        "--expected-validation-environment-sha256", sha256(
          await readFile(validationEnvironmentReceiptPath),
        ),
        "--controller-context", controllerContextPath,
        "--expected-controller-context-sha256", controllerContextSha256,
        ...(controllerIssuancePath ? [
          "--controller-issuance", controllerIssuancePath,
          "--expected-controller-issuance-sha256", controllerIssuanceSha256,
        ] : []),
        ...formalControllerTrustArguments(common),
        ...(common.formal_result_eligible ? ["--require-formal-context", "true"] : []),
      ];
      if (needsAdapter && common.formal_result_eligible) {
        await verifyTrustedValidationGuard("before formal adapter validation/build");
        adapterPackage = await validateBundledFormalAdapterPackage({
          adapterPath: args.get("adapter-file") ?? resolve(
            controllerBenchmarkDir,
            "evaluator/adapters/candidate-adapter.mjs",
          ),
          configPath: args.get("adapter-config"),
          integrityReceiptPath: args.get("adapter-integrity-receipt"),
          expectedIntegrityReceiptSha256: args.get("expected-adapter-integrity-sha256"),
          benchmarkRoot: controllerBenchmarkDir,
          forbiddenRoots: [sourceRepo, runRoot, artifactDir, workspace, validationWorkspace,
            controllerBenchmarkDir],
          context: controllerContext,
          candidateRoot: validationWorkspace,
          candidateManifestPath: resolve(captureDir, "raw-manifest.json"),
          probeSourcePath: args.get("adapter-probe-source"),
          buildRoot: resolve(artifactDir, "formal-tw09-probe-build"),
          cargoPath: cargo.path,
          cargoSha256: cargo.sha256,
          rustcPath: rustc.path,
          rustcSha256: rustc.sha256,
          environment: validationEnvironment,
        });
        await verifyTrustedValidationGuard("after formal adapter validation/build");
        if (adapterPackage.probe_build_receipt) {
          const buildStage = await writeStage("formal_adapter_build", {
            case_id: common.case_id,
            capture_receipt_sha256: controllerContext.capture_receipt_sha256,
            candidate_tree: controllerContext.candidate_tree,
            raw_tree_digest_sha256: controllerContext.raw_tree_digest_sha256,
            probe_sha256: adapterPackage.probe.sha256,
            probe_build_receipt_sha256: adapterPackage.probe_build_receipt.sha256,
            cargo_home_manifest_sha256:
              adapterPackage.probe_build_receipt.cargo_home_manifest_sha256,
            trusted_validation_environment_sha256: sha256(
              await readFile(validationEnvironmentReceiptPath),
            ),
            sealed_controller_builder: true,
          }, [
            adapterPackage.config.path,
            adapterPackage.integrity_receipt.path,
            adapterPackage.probe_source.path,
            adapterPackage.candidate_manifest.path,
          ], [adapterPackage.probe.path, adapterPackage.probe_build_receipt.path]);
          adapterPackage.adapter_build_stage_receipt = await fileIdentity(buildStage.path);
        }
        oracleArguments.push(...formalAdapterOracleArguments(adapterPackage));
      } else if (args.has("adapter-file")) {
        const adapter = await requireRegular(args.get("adapter-file"), "trusted adapter");
        const bytes = await readFile(adapter);
        if (!SHA256.test(args.get("expected-adapter-sha256") ?? "") ||
            sha256(bytes) !== args.get("expected-adapter-sha256")) fail("trusted adapter SHA-256 mismatch");
        oracleArguments.push("--adapter-file", adapter);
      }
      if (!adapterPackage && args.has("adapter-config")) {
        const adapterConfig = await requireRegular(args.get("adapter-config"), "trusted adapter config");
        const bytes = await readFile(adapterConfig);
        if (!SHA256.test(args.get("expected-adapter-config-sha256") ?? "") ||
            sha256(bytes) !== args.get("expected-adapter-config-sha256")) fail("trusted adapter config SHA-256 mismatch");
        oracleArguments.push("--adapter-config", adapterConfig);
      }
      const oracleResult = await runBundledHelper("scripts/run-oracles.mjs", oracleArguments.slice(1), {
        env: {...validationEnvironment, TMPDIR: adapterTmp, TMP: adapterTmp, TEMP: adapterTmp},
        allowFailure: true,
      }, {extraProtectedRoots: [captureDir, workspace, validationCargoHome,
        validationHome, validationCodexHome],
      extraProtectedPaths: [validationEnvironmentReceiptPath],
      nestedProcessSupervisor: true});
      oracleReceiptPath = resolve(oracleDir, "oracle-run.json");
      if (!existsSync(oracleReceiptPath)) fail(`production oracle runner failed without receipt: ${oracleResult.stderr}`);
    }
    const oracleReceipt = JSON.parse(await readFile(oracleReceiptPath, "utf8"));
    if (common.formal_result_eligible &&
        oracleReceipt.controller_issuance_sha256 !== controllerIssuanceSha256) {
      fail("production oracle evidence does not bind the controller issuance");
    }
    await writeStage(
      "production_oracles",
      {
        construction_smoke: constructionSmoke,
        overall_status: oracleReceipt.overall_status ?? "not_executed",
        controller_context_sha256: controllerContextSha256,
        controller_issuance_sha256: controllerIssuanceSha256,
        adapter_integrity_receipt_sha256: adapterPackage?.integrity_receipt.sha256 ?? null,
        adapter_probe_build_receipt_sha256:
          adapterPackage?.probe_build_receipt?.sha256 ?? null,
        adapter_build_stage_receipt_sha256:
          adapterPackage?.adapter_build_stage_receipt?.sha256 ?? null,
      },
      [
        core.receiptPath,
        controllerContextPath,
        validationEnvironmentReceiptPath,
        ...(controllerIssuancePath ? [controllerIssuancePath] : []),
        ...(adapterPackage ? [adapterPackage.scaffold.path, adapterPackage.scaffold_lock.path,
          adapterPackage.config.path, adapterPackage.probe.path,
          adapterPackage.integrity_receipt.path,
          ...(adapterPackage.probe_source ? [adapterPackage.probe_source.path] : []),
          ...(adapterPackage.candidate_manifest ? [adapterPackage.candidate_manifest.path] : []),
          ...(adapterPackage.probe_build_receipt
            ? [adapterPackage.probe_build_receipt.path] : []),
          ...(adapterPackage.adapter_build_stage_receipt
            ? [adapterPackage.adapter_build_stage_receipt.path] : [])] : []),
      ],
      [oracleReceiptPath],
    );

    const reviewInputDir = resolve(artifactDir, "review-input");
    await mkdir(reviewInputDir, {mode: 0o700});
    const reviewSources = [
      {path: "task.txt", role: "task", bytes: taskBytes},
      {path: "authority.json", role: "authority", bytes: await authorityPacket(caseId, controllerBenchmarkDir)},
      {path: "candidate-checkout.json", role: "candidate_checkout", bytes: await readFile(resolve(captureDir, "raw-manifest.json"))},
      {path: "candidate.diff", role: "candidate_diff", bytes: await readFile(resolve(captureDir, "candidate.patch"))},
      {path: "candidate-validation.json", role: "candidate_validation", bytes: canonicalBytes({core: core.receipt, oracles: oracleReceipt})},
      {path: "final-message.txt", role: "final_message", bytes: await readFile(finalMessagePath)},
      ...await changedCandidateReviewSources(validationWorkspace, captureReceipt),
    ];
    reviewSources.sort((left, right) => Buffer.from(left.path).compare(Buffer.from(right.path)));
    const reviewArtifacts = [];
    for (const source of reviewSources) {
      const destination = resolve(reviewInputDir, source.path);
      await mkdir(dirname(destination), {recursive: true, mode: 0o700});
      await writeFile(destination, source.bytes, {mode: 0o600, flag: "wx"});
      reviewArtifacts.push({path: source.path, roles: [source.role], bytes: source.bytes.length, sha256: sha256(source.bytes)});
    }
    const reviewInputManifestPath = resolve(artifactDir, "review-input-manifest.json");
    await writeFile(reviewInputManifestPath, canonicalBytes({schema: "tachiko-review-packet-input-v1", artifacts: reviewArtifacts}), {mode: 0o600, flag: "wx"});
    const reviewOutputDir = resolve(artifactDir, "review-output");
    const reviewTerminalPath = resolve(artifactDir, "review-terminal.json");
    const frozenAt = registrationBody.registered_at;
    const reviewResult = await runBundledHelper("scripts/build-review-packet.mjs", [
      "--case-id", caseId,
      "--candidate-id", common.candidate_id,
      "--input-root", reviewInputDir,
      "--input-manifest", reviewInputManifestPath,
      "--variant", registeredVariantPath,
      "--contract", resolve(controllerBenchmarkDir, "evaluator/contracts/review-packet-blinding-v1.json"),
      "--output-dir", reviewOutputDir,
      "--terminal-receipt", reviewTerminalPath,
      "--custodian-id", args.get("custodian-id") ?? "internal-custodian",
      "--custodian-eligible", "true",
      "--frozen-at", frozenAt,
      "--controller-context", controllerContextPath,
      "--expected-controller-context-sha256", controllerContextSha256,
      ...(controllerIssuancePath ? [
        "--controller-issuance", controllerIssuancePath,
        "--expected-controller-issuance-sha256", controllerIssuanceSha256,
      ] : []),
      ...formalControllerTrustArguments(common),
      ...(common.formal_result_eligible ? ["--require-formal-context", "true"] : []),
    ], {env: await freshValidationStageEnvironment("review-build"), allowFailure: true}, {
      extraProtectedRoots: [captureDir, validationWorkspace, validationCargoHome,
        validationHome, validationCodexHome],
    });
    const reviewReceiptPath = resolve(reviewOutputDir, "receipt.json");
    if (reviewResult.status !== 0 || !existsSync(reviewReceiptPath)) {
      fail(`review packet construction failed: ${reviewResult.stderr}`);
    }
    const reviewReceipt = JSON.parse(await readFile(reviewReceiptPath, "utf8"));
    if (!reviewReceipt.safe_to_release || (common.formal_result_eligible &&
        reviewReceipt.controller_issuance_sha256 !== controllerIssuanceSha256)) {
      fail("review packet is not safe to release or lacks the controller issuance binding");
    }
    const standaloneScanReceiptPath = resolve(artifactDir, "review-standalone-scan.json");
    const scanResult = await runBundledHelper("scripts/scan-review-packet.mjs", [
      "--packet-dir", resolve(reviewOutputDir, "packet"),
      "--contract", resolve(controllerBenchmarkDir, "evaluator/contracts/review-packet-blinding-v1.json"),
      "--variant", registeredVariantPath,
      "--receipt", standaloneScanReceiptPath,
      "--controller-context", controllerContextPath,
      "--expected-controller-context-sha256", controllerContextSha256,
      ...(controllerIssuancePath ? [
        "--controller-issuance", controllerIssuancePath,
        "--expected-controller-issuance-sha256", controllerIssuanceSha256,
      ] : []),
      ...formalControllerTrustArguments(common),
      ...(common.formal_result_eligible ? ["--require-formal-context", "true"] : []),
    ], {env: await freshValidationStageEnvironment("review-scan"), allowFailure: true}, {
      extraProtectedRoots: [captureDir, validationWorkspace, reviewInputDir,
        validationCargoHome, validationHome, validationCodexHome],
    });
    if (scanResult.status !== 0 || !existsSync(standaloneScanReceiptPath)) {
      fail(`standalone review packet scan failed: ${scanResult.stderr}`);
    }
    const standaloneScanReceipt = JSON.parse(await readFile(standaloneScanReceiptPath, "utf8"));
    if (!standaloneScanReceipt.safe_to_release ||
        standaloneScanReceipt.packet_tree_sha256 !== reviewReceipt.rendered_packet_sha256 ||
        standaloneScanReceipt.controller_context_sha256 !== controllerContextSha256 ||
        (common.formal_result_eligible &&
          standaloneScanReceipt.controller_issuance_sha256 !== controllerIssuanceSha256)) {
      fail("standalone review packet scan does not bind the built packet and controller context");
    }
    await writeStage("review_packet", {
      safe_to_release: true,
      semantic_scoring_performed: false,
      controller_context_sha256: controllerContextSha256,
      controller_issuance_sha256: controllerIssuanceSha256,
      standalone_scan_receipt_sha256: sha256(await readFile(standaloneScanReceiptPath)),
    }, [reviewInputManifestPath, controllerContextPath,
      ...(controllerIssuancePath ? [controllerIssuancePath] : [])], [
      reviewReceiptPath, reviewTerminalPath, standaloneScanReceiptPath,
    ]);

    const agentDisposition = processResult.timed_out
      ? "agent_timeout"
      : processResult.spawn_error || processResult.exit_code !== 0
        ? "agent_failed"
        : "awaiting_review";
    const resultSkeleton = {
      schema: "tachiko-controller-result-skeleton-v1",
      ...common,
      ...pendingResultState(common.formal_result_eligible),
      disposition: agentDisposition,
      no_resampling: true,
      launch_count: 1,
      candidate_commit: captureReceipt.candidate_commit,
      candidate_tree: captureReceipt.candidate_tree,
      raw_tree_digest_sha256: captureReceipt.raw_tree_digest_sha256,
      preflight_receipt_sha256: sha256(await readFile(preflightReceiptPath)),
      base_control_receipt_sha256: sha256(await readFile(baseReceiptPath)),
      process_receipt_sha256: sha256(await readFile(processReceiptPath)),
      capture_receipt_sha256: sha256(await readFile(captureReceiptPath)),
      validation_receipt_sha256: sha256(await readFile(validationReceiptPath)),
      core_receipt_sha256: sha256(await readFile(core.receiptPath)),
      oracle_receipt_sha256: sha256(await readFile(oracleReceiptPath)),
      review_receipt_sha256: sha256(await readFile(reviewReceiptPath)),
      review_scan_receipt_sha256: sha256(await readFile(standaloneScanReceiptPath)),
      controller_context_sha256: controllerContextSha256,
      controller_issuance_sha256: controllerIssuanceSha256,
      adapter_integrity_receipt_sha256: adapterPackage?.integrity_receipt.sha256 ?? null,
      adapter_probe_build_receipt_sha256: adapterPackage?.probe_build_receipt?.sha256 ?? null,
      adapter_build_stage_receipt_sha256:
        adapterPackage?.adapter_build_stage_receipt?.sha256 ?? null,
      scores_recorded: false,
      semantic_review_pending: true,
      limitations: [
        "provider-internal immutable deployment identity unavailable",
        "current-user host isolation rather than dedicated provider or OS accounts",
        "multi-reviewer scoring panel not completed by the controller",
      ],
    };
    const resultPath = resolve(artifactDir, "result-skeleton.json");
    await writeFile(resultPath, canonicalBytes(resultSkeleton), {mode: 0o600, flag: "wx"});
    await writeStage("result_skeleton", resultSkeleton, [reviewReceiptPath,
      ...(controllerIssuancePath ? [controllerIssuancePath] : [])], [resultPath]);
    await terminalize(agentDisposition, {
      launch_count: 1,
      process_exit_code: processResult.exit_code,
      process_signal: processResult.signal,
      timed_out: processResult.timed_out,
      controller_context_sha256: controllerContextSha256,
      controller_issuance_sha256: controllerIssuanceSha256,
      adapter_integrity_receipt_sha256: adapterPackage?.integrity_receipt.sha256 ?? null,
      adapter_probe_build_receipt_sha256: adapterPackage?.probe_build_receipt?.sha256 ?? null,
      adapter_build_stage_receipt_sha256:
        adapterPackage?.adapter_build_stage_receipt?.sha256 ?? null,
      result_skeleton_sha256: sha256(await readFile(resultPath)),
    });
    console.log(JSON.stringify({artifact_dir: artifactDir, disposition: agentDisposition, launch_count: 1}));
    if (agentDisposition !== "awaiting_review") process.exitCode = 1;
  } catch (error) {
    if (!terminalWritten) {
      try {
        await terminalize("infrastructure_failed", {
          launch_count: launchCount,
          error: error instanceof Error ? error.message : String(error),
        });
      } catch { /* preserve original failure */ }
    }
    throw error;
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch(async (error) => {
    if (committedFailureHandler) {
      try { await committedFailureHandler(error); } catch { /* preserve the original error */ }
    }
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
