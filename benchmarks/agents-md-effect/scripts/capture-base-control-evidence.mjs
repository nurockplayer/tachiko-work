#!/usr/bin/env node

// Executes the frozen clean-base command union without exposing the candidate
// workspace or selected instruction bytes. Controller-bound invocations are
// same-wave evidence; standalone invocations remain construction-only.

import {spawnSync} from "node:child_process";
import {createHash} from "node:crypto";
import {existsSync} from "node:fs";
import {lstat, mkdir, readFile, realpath, writeFile} from "node:fs/promises";
import {basename, dirname, isAbsolute, relative, resolve} from "node:path";
import {fileURLToPath} from "node:url";
import {
  DENY_NETWORK_PROFILE,
  probeNetworkSandbox,
  runNetworkSandboxed,
  supervisedWriteProtection,
} from "./network-sandbox.mjs";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const benchmarkDir = resolve(scriptDir, "..");
const ID = /^[0-9a-f]{32}$/;
const SHA256 = /^[0-9a-f]{64}$/;

function fail(message) { throw new Error(message); }

function usage() {
  console.error(
    "usage: node capture-base-control-evidence.mjs --case TW-01 " +
      "--workspace /abs/repo --receipt /abs/receipt.json [--log-dir /abs/new-logs] " +
      "[--controller-bound true --phase construction_pilot_only --wave-id <32hex> " +
      "--run-id <32hex> --attempt-id <32hex> --candidate-id <32hex> " +
      "--control-sha256 <sha256> --environment-receipt /abs/receipt.json " +
      "--trusted-shell /abs/bash --expected-shell-sha256 <sha256> " +
      "--construction-smoke true]",
  );
  process.exit(2);
}

function parseArgs(argv) {
  if (argv.length % 2 !== 0) usage();
  const allowed = new Set([
    "case", "workspace", "receipt", "log-dir", "controller-bound", "phase",
    "wave-id", "run-id", "attempt-id", "candidate-id", "control-sha256",
    "environment-receipt", "construction-smoke",
    "trusted-shell", "expected-shell-sha256",
    "supervised-write-protection-json",
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

function sha256(value) { return createHash("sha256").update(value).digest("hex"); }

function isInside(candidate, parent) {
  const path = relative(parent, candidate);
  return path === "" || (!path.startsWith("..") && !isAbsolute(path));
}

async function prospective(path, label) {
  if (!isAbsolute(path)) fail(`${label} must be absolute`);
  if (existsSync(path)) fail(`${label} must not already exist`);
  const parent = await realpath(dirname(path));
  const info = await lstat(parent);
  if (!info.isDirectory() || info.isSymbolicLink()) fail(`${label} parent must be a real directory`);
  return resolve(parent, basename(path));
}

function isolatedGitEnvironment() {
  const environment = {...process.env};
  for (const key of Object.keys(environment)) {
    if (/^GIT_(?:DIR|WORK_TREE|INDEX_FILE|OBJECT_DIRECTORY|COMMON_DIR|CONFIG_PARAMETERS)$/.test(key) ||
        /^GIT_CONFIG_(?:KEY|VALUE)_\d+$/.test(key)) delete environment[key];
  }
  return {
    ...environment,
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_CONFIG_GLOBAL: "/dev/null",
    GIT_ATTR_NOSYSTEM: "1",
    GIT_ALTERNATE_OBJECT_DIRECTORIES: "",
  };
}

function git(root, args, allowFailure = false) {
  const result = spawnSync("rtk", [
    "proxy", "git", "-c", "core.hooksPath=/dev/null", "-c",
    "core.attributesFile=/dev/null", "-c", "core.autocrlf=false", ...args,
  ], {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
    env: isolatedGitEnvironment(),
  });
  if (!allowFailure && result.status !== 0) {
    fail(result.stderr || result.stdout || `rtk git ${args.join(" ")} failed`);
  }
  return result;
}

async function writeContentAddressed(logDir, stream, bytes) {
  const hash = sha256(bytes);
  const path = resolve(logDir, `${hash}.${stream}`);
  if (existsSync(path)) {
    if (!(await readFile(path)).equals(bytes)) fail("content-addressed log collision");
  } else {
    await writeFile(path, bytes, {mode: 0o600, flag: "wx"});
  }
  return {path, bytes: bytes.length, sha256: hash};
}

const args = parseArgs(process.argv.slice(2));
for (const key of ["case", "workspace", "receipt"]) if (!args.has(key)) usage();
const caseId = args.get("case");
if (!/^TW-0[1-9]$/.test(caseId)) usage();
const controllerBound = args.get("controller-bound") === "true";
const constructionSmoke = args.get("construction-smoke") === "true";
if (args.has("controller-bound") && !controllerBound) fail("--controller-bound only accepts true");
if (args.has("construction-smoke") && !constructionSmoke) fail("--construction-smoke only accepts true");

async function parseWriteProtection(value, required) {
  if (value === undefined) {
    if (required) fail("controller-bound base control requires supervised write protection");
    return supervisedWriteProtection();
  }
  let parsed;
  try { parsed = JSON.parse(value); } catch { fail("supervised write protection is not valid JSON"); }
  if (JSON.stringify(Object.keys(parsed ?? {}).sort()) !== JSON.stringify([
    "protected_paths", "protected_roots", "schema",
  ]) || parsed.schema !== "tachiko-supervised-write-protection-v1" ||
      !Array.isArray(parsed.protected_roots) || !Array.isArray(parsed.protected_paths)) {
    fail("supervised write protection has an invalid schema");
  }
  const canonicalRoots = [];
  const canonicalPaths = [];
  for (const [label, entries] of [
    ["protected_roots", parsed.protected_roots],
    ["protected_paths", parsed.protected_paths],
  ]) {
    if (entries.some((path) => typeof path !== "string" || !isAbsolute(path) || resolve(path) !== path) ||
        new Set(entries).size !== entries.length ||
        JSON.stringify(entries) !== JSON.stringify([...entries].sort())) {
      fail(`supervised write protection ${label} must be unique sorted absolute paths`);
    }
  }
  for (const path of parsed.protected_roots) {
    const metadata = await lstat(path);
    if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
      fail("supervised write protection root must be a non-symlink directory");
    }
    canonicalRoots.push(await realpath(path));
  }
  for (const path of parsed.protected_paths) {
    const metadata = await lstat(path);
    if (!metadata.isFile() || metadata.isSymbolicLink()) {
      fail("supervised write protection path must be a non-symlink regular file");
    }
    canonicalPaths.push(await realpath(path));
  }
  const normalized = supervisedWriteProtection({
    protectedRoots: canonicalRoots,
    protectedPaths: canonicalPaths,
  });
  if (required && !normalized.active) fail("controller-bound write protection must not be empty");
  return normalized;
}

const writeProtection = await parseWriteProtection(
  args.get("supervised-write-protection-json"),
  controllerBound,
);

const workspaceInput = resolve(args.get("workspace"));
if (!isAbsolute(args.get("workspace"))) fail("workspace must be absolute");
if (!isAbsolute(args.get("receipt"))) fail("receipt must be absolute");
if (args.has("log-dir") && !isAbsolute(args.get("log-dir"))) fail("log-dir must be absolute");
const workspaceInfo = await lstat(workspaceInput);
if (!workspaceInfo.isDirectory() || workspaceInfo.isSymbolicLink()) fail("workspace must be a real directory");
const workspace = await realpath(workspaceInput);
const receiptPath = await prospective(resolve(args.get("receipt")), "receipt");
const logDir = await prospective(
  resolve(args.get("log-dir") ?? resolve(dirname(receiptPath), "base-control-logs")),
  "log directory",
);
for (const path of [receiptPath, logDir]) {
  if (isInside(path, workspace) || isInside(workspace, path)) {
    fail("trusted evidence paths and base workspace must be disjoint");
  }
}

let binding = null;
let trustedShell = "/bin/zsh";
let trustedShellReceipt = null;
if (controllerBound) {
  for (const key of [
    "phase", "wave-id", "run-id", "attempt-id", "candidate-id", "control-sha256",
    "environment-receipt", "trusted-shell", "expected-shell-sha256",
  ]) if (!args.has(key)) usage();
  for (const key of ["wave-id", "run-id", "attempt-id", "candidate-id"]) {
    if (!ID.test(args.get(key))) fail(`${key} must be opaque lowercase 128-bit hex`);
  }
  if (!SHA256.test(args.get("control-sha256"))) fail("control-sha256 must be SHA-256");
  const phase = args.get("phase");
  if (!["construction_pilot_only", "baseline_a", "variant_b"].includes(phase)) fail("invalid phase");
  if (constructionSmoke && phase !== "construction_pilot_only") fail("construction smoke is forbidden in a formal phase");
  const environmentPathInput = resolve(args.get("environment-receipt"));
  if (!isAbsolute(args.get("environment-receipt"))) fail("environment-receipt must be absolute");
  const environmentInfo = await lstat(environmentPathInput);
  if (!environmentInfo.isFile() || environmentInfo.isSymbolicLink()) {
    fail("environment-receipt must be a trusted regular file");
  }
  const environmentPath = await realpath(environmentPathInput);
  if (isInside(environmentPath, workspace)) fail("environment receipt must be outside workspace");
  const environmentBytes = await readFile(environmentPath);
  const environmentReceipt = JSON.parse(environmentBytes.toString("utf8"));
  for (const [argumentKey, receiptKey] of [
    ["wave-id", "wave_id"], ["run-id", "run_id"], ["attempt-id", "attempt_id"],
    ["candidate-id", "candidate_id"],
  ]) {
    if (environmentReceipt[receiptKey] !== args.get(argumentKey)) fail(`environment receipt ${receiptKey} mismatch`);
  }
  if (environmentReceipt.case_id !== caseId || environmentReceipt.control_sha256 !== args.get("control-sha256")) {
    fail("environment receipt case/control mismatch");
  }
  if (!isAbsolute(args.get("trusted-shell")) ||
      !SHA256.test(args.get("expected-shell-sha256"))) {
    fail("controller-bound trusted shell path/hash are invalid");
  }
  const trustedShellInput = resolve(args.get("trusted-shell"));
  const trustedShellCanonical = await realpath(trustedShellInput);
  const trustedShellInfo = await lstat(trustedShellCanonical);
  if (!trustedShellInfo.isFile() || trustedShellInfo.isSymbolicLink() ||
      (trustedShellInfo.mode & 0o111) === 0) {
    fail("controller-bound trusted shell must resolve to an executable regular file");
  }
  const trustedShellBytes = await readFile(trustedShellCanonical);
  const trustedShellSha256 = sha256(trustedShellBytes);
  if (trustedShellSha256 !== args.get("expected-shell-sha256")) {
    fail("controller-bound trusted shell SHA-256 mismatch");
  }
  const registeredBash = environmentReceipt.tools?.find((tool) => tool.name === "bash");
  let registeredPathMatched = false;
  for (const path of [registeredBash?.path, registeredBash?.source_path, registeredBash?.staged_path]
    .filter(Boolean)) {
    if (await realpath(path) === trustedShellCanonical) registeredPathMatched = true;
  }
  if (!registeredBash || registeredBash.sha256 !== trustedShellSha256 || !registeredPathMatched) {
    fail("controller-bound trusted shell is not the environment-registered Bash");
  }
  trustedShell = trustedShellCanonical;
  const qualification = spawnSync(
    trustedShell,
    ["--noprofile", "--norc", "-c", "exit 0"],
    {cwd: workspace, encoding: "utf8", env: process.env},
  );
  if (qualification.error || qualification.status !== 0) {
    fail("controller-bound trusted shell qualification failed");
  }
  trustedShellReceipt = {
    path: trustedShell,
    bytes: trustedShellBytes.length,
    sha256: trustedShellSha256,
    arguments_prefix: ["--noprofile", "--norc", "-c"],
    qualification_executed: true,
  };
  binding = {
    phase,
    wave_id: args.get("wave-id"),
    run_id: args.get("run-id"),
    attempt_id: args.get("attempt-id"),
    candidate_id: args.get("candidate-id"),
    control_sha256: args.get("control-sha256"),
    environment_identity_sha256: environmentReceipt.environment_identity_sha256,
    environment_receipt: {path: environmentPath, bytes: environmentBytes.length, sha256: sha256(environmentBytes)},
  };
  if (!SHA256.test(binding.environment_identity_sha256 ?? "")) {
    fail("environment receipt lacks an identity commitment");
  }
}

const [caseManifest, coreLock] = await Promise.all([
  readFile(resolve(benchmarkDir, "evaluator/cases.json"), "utf8").then(JSON.parse),
  readFile(resolve(benchmarkDir, "evaluator/core-score-lock.json"), "utf8").then(JSON.parse),
]);
const benchmarkCase = caseManifest.cases.find((entry) => entry.id === caseId);
const coreCase = coreLock.cases.find((entry) => entry.id === caseId);
if (!benchmarkCase || !coreCase) fail(`missing lock for ${caseId}`);

const headBefore = git(workspace, ["rev-parse", "HEAD^{commit}"]).stdout.trim();
const treeBefore = git(workspace, ["rev-parse", "HEAD^{tree}"]).stdout.trim();
if (headBefore !== benchmarkCase.historical_base_commit || treeBefore !== benchmarkCase.historical_base_tree) {
  fail(`${caseId} workspace is not its exact historical base/tree`);
}
if (git(workspace, ["status", "--porcelain=v1", "-z"]).stdout !== "") fail(`${caseId} base workspace is not clean`);
if (existsSync(resolve(workspace, "AGENTS.md"))) fail("base control workspace exposes AGENTS.md");
const targetLookup = git(workspace, ["cat-file", "-e", `${benchmarkCase.ground_truth_commit}^{commit}`], true);
if (targetLookup.status === 0) fail("ground-truth commit leaked into base control workspace");
const rootAgentsObjects = git(workspace, ["rev-list", "--objects", headBefore, "--", "AGENTS.md"])
  .stdout.split("\n").filter((line) => line.endsWith(" AGENTS.md"));
if (rootAgentsObjects.length !== 0) fail(`${caseId} base ancestry contains root AGENTS.md`);

const commands = [
  ...(benchmarkCase.validation?.base ?? []),
  ...coreCase.validation_checks.map((entry) => entry.command),
].filter((command, index, all) => all.indexOf(command) === index);
if (commands.length === 0) fail(`${caseId} has an empty base-control union`);

const networkEnforcement = constructionSmoke
  ? {mode: "construction_smoke_not_executed", probe_denied: null}
  : await probeNetworkSandbox({
    nodeExecutable: process.execPath,
    profile: `${DENY_NETWORK_PROFILE}${writeProtection.profile_suffix}`,
  });

await mkdir(logDir, {mode: 0o700});
const commandReceipts = [];
let allPassed = true;
for (let index = 0; index < commands.length; index += 1) {
  const command = commands[index];
  if (constructionSmoke) {
    commandReceipts.push({
      command_id: `base.${caseId.toLowerCase().replace("-", "")}.${index + 1}`,
      command,
      execution: "construction_smoke_not_executed",
      exit_code: null,
      timed_out: false,
      stdout: await writeContentAddressed(logDir, "stdout", Buffer.alloc(0)),
      stderr: await writeContentAddressed(logDir, "stderr", Buffer.alloc(0)),
    });
    continue;
  }
  const shellArguments = controllerBound
    ? ["--noprofile", "--norc", "-c", command]
    : ["-f", "-c", command];
  const result = await runNetworkSandboxed({
    executable: trustedShell,
    args: shellArguments,
    cwd: workspace,
    environment: process.env,
    timeoutMilliseconds: 1_800_000,
    terminationGraceMilliseconds: 10_000,
    profile: `${DENY_NETWORK_PROFILE}${writeProtection.profile_suffix}`,
  });
  const stdout = result.stdout;
  const stderr = result.stderr;
  const passed = result.exit_code === 0 && !result.timed_out && !result.spawn_error &&
    result.process_group_extinct_before_capture;
  allPassed &&= passed;
  commandReceipts.push({
    command_id: `base.${caseId.toLowerCase().replace("-", "")}.${index + 1}`,
    command,
    execution: "canonical",
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
    process_group_extinct_before_integrity_check: result.process_group_extinct_before_capture,
    process_containment: result.process_containment,
    network_sandbox: result.network_sandbox,
    stdout: await writeContentAddressed(logDir, "stdout", stdout),
    stderr: await writeContentAddressed(logDir, "stderr", stderr),
  });
}

const headAfter = git(workspace, ["rev-parse", "HEAD^{commit}"]).stdout.trim();
const treeAfter = git(workspace, ["rev-parse", "HEAD^{tree}"]).stdout.trim();
const cleanAfter = git(workspace, ["status", "--porcelain=v1", "-z"]).stdout === "";
if (headAfter !== headBefore || treeAfter !== treeBefore || !cleanAfter) allPassed = false;

const phase = binding?.phase ?? "construction_pilot_only";
const receipt = {
  schema: "tachiko-same-wave-base-control-v1",
  protocol_id: caseManifest.protocol_id,
  classification: controllerBound
    ? phase === "construction_pilot_only" ? "same_wave_construction_control" : "same_wave_formal_control"
    : "construction_pilot_only",
  formal_result_eligible: controllerBound && phase !== "construction_pilot_only" && !constructionSmoke,
  ...(binding ?? {}),
  trusted_shell: trustedShellReceipt,
  case_id: caseId,
  mode: "historical_base_control_union",
  historical_base_commit: headBefore,
  historical_base_tree: treeBefore,
  ground_truth_commit_absent: true,
  clean_workspace_before_commands: true,
  clean_workspace_after_commands: cleanAfter,
  head_tree_unchanged: headBefore === headAfter && treeBefore === treeAfter,
  root_agents_absent_from_base_ancestry: true,
  root_agents_absent_from_workspace: true,
  candidate_instruction_bytes_exposed: false,
  network_policy: "kernel-enforced darwin sandbox deny-network plus Cargo offline",
  network_enforcement: networkEnforcement,
  supervised_write_protection: {
    schema: writeProtection.schema,
    active: writeProtection.active,
    protected_roots: writeProtection.protected_roots,
    protected_paths: writeProtection.protected_paths,
    profile_suffix_sha256: writeProtection.profile_suffix_sha256,
  },
  cargo_net_offline: process.env.CARGO_NET_OFFLINE === "true",
  environment: Object.fromEntries([
    "HOME", "CODEX_HOME", "TMPDIR", "PATH", "LANG", "LC_ALL", "TZ", "CARGO_INCREMENTAL",
    "CARGO_NET_OFFLINE", "CARGO_HOME", "RUSTUP_HOME", "PNPM_HOME", "GIT_CONFIG_NOSYSTEM",
    "GIT_CONFIG_GLOBAL", "GIT_ATTR_NOSYSTEM",
  ].map((key) => [key, process.env[key] ?? null])),
  command_union_rule: "ordered deduplicated cases.validation.base then core-score validation commands",
  command_list_sha256: sha256(Buffer.from(`${JSON.stringify(commands)}\n`, "utf8")),
  construction_smoke: constructionSmoke,
  commands_executed: !constructionSmoke,
  commands: commandReceipts,
  all_commands_passed: allPassed,
  raw_logs_embedded: false,
  log_directory: logDir,
  completed_at: new Date().toISOString(),
  not_claimed: controllerBound ? [] : [
    "same-wave formal base-control receipt",
    "Baseline A, Variant B, controlled A/B, or Ultra result",
  ],
};
await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, {mode: 0o600, flag: "wx"});
console.log(JSON.stringify({case_id: caseId, receipt: receiptPath, all_commands_passed: allPassed}));
if (!allPassed) process.exitCode = 1;
