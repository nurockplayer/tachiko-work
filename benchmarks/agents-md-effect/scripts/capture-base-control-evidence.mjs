#!/usr/bin/env node

// Construction-only evidence for the ordered, de-duplicated clean-base command
// union. This never launches Codex and can never create a formal result.

import {createHash} from "node:crypto";
import {existsSync} from "node:fs";
import {mkdir, realpath, readFile, writeFile} from "node:fs/promises";
import {spawnSync} from "node:child_process";
import {dirname, isAbsolute, resolve} from "node:path";
import {fileURLToPath} from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const benchmarkDir = resolve(scriptDir, "..");

function fail(message) {
  throw new Error(message);
}

function usage() {
  console.error(
    "usage: node capture-base-control-evidence.mjs --case TW-01 " +
      "--workspace /abs/repo --receipt /abs/receipt.json",
  );
  process.exit(2);
}

function parseArgs(argv) {
  const values = new Map();
  if (argv.length % 2 !== 0) usage();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined || values.has(key.slice(2))) {
      usage();
    }
    values.set(key.slice(2), value);
  }
  return values;
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function git(root, args, allowFailure = false) {
  const result = spawnSync("rtk", ["proxy", "git", ...args], {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (!allowFailure && result.status !== 0) {
    fail(result.stderr || result.stdout || `rtk git ${args.join(" ")} failed`);
  }
  return result;
}

const args = parseArgs(process.argv.slice(2));
for (const key of ["case", "workspace", "receipt"]) {
  if (!args.has(key)) usage();
}
const caseId = args.get("case");
if (!/^TW-0[1-9]$/.test(caseId)) usage();

const workspace = await realpath(resolve(args.get("workspace")));
const receiptPath = resolve(args.get("receipt"));
if (!isAbsolute(receiptPath) || existsSync(receiptPath)) {
  fail("receipt must be a new absolute path");
}

const [caseManifest, coreLock, environmentLock] = await Promise.all([
  readFile(resolve(benchmarkDir, "evaluator/cases.json"), "utf8").then(JSON.parse),
  readFile(resolve(benchmarkDir, "evaluator/core-score-lock.json"), "utf8").then(
    JSON.parse,
  ),
  readFile(resolve(benchmarkDir, "environment-lock.json"), "utf8").then(JSON.parse),
]);
const benchmarkCase = caseManifest.cases.find((entry) => entry.id === caseId);
const coreCase = coreLock.cases.find((entry) => entry.id === caseId);
if (!benchmarkCase || !coreCase) fail(`missing lock for ${caseId}`);

const head = git(workspace, ["rev-parse", "HEAD^{commit}"]).stdout.trim();
const tree = git(workspace, ["rev-parse", "HEAD^{tree}"]).stdout.trim();
if (
  head !== benchmarkCase.historical_base_commit ||
  tree !== benchmarkCase.historical_base_tree
) {
  fail(`${caseId} workspace is not its exact historical base/tree`);
}
if (git(workspace, ["status", "--porcelain=v1", "-z"]).stdout !== "") {
  fail(`${caseId} base workspace is not clean`);
}
const rootAgentsObjects = git(workspace, [
  "rev-list",
  "--objects",
  head,
  "--",
  "AGENTS.md",
]).stdout
  .split("\n")
  .filter((line) => line.endsWith(" AGENTS.md"));
if (rootAgentsObjects.length !== 0) {
  fail(`${caseId} base ancestry contains root AGENTS.md`);
}

const commands = [
  ...(benchmarkCase.validation?.base ?? []),
  ...coreCase.validation_checks.map((entry) => entry.command),
].filter((command, index, all) => all.indexOf(command) === index);
if (commands.length === 0) fail(`${caseId} has an empty base-control union`);

const commandReceipts = [];
let allPassed = true;
for (let index = 0; index < commands.length; index += 1) {
  const command = commands[index];
  const startedAt = new Date().toISOString();
  const started = process.hrtime.bigint();
  const result = spawnSync("/bin/zsh", ["-f", "-c", command], {
    cwd: workspace,
    encoding: "utf8",
    timeout: 1_800_000,
    maxBuffer: 128 * 1024 * 1024,
    env: {
      ...process.env,
      PATH: environmentLock.controlled_runner.path,
      LANG: "C.UTF-8",
      LC_ALL: "C.UTF-8",
      TZ: "UTC",
      CARGO_INCREMENTAL: "0",
      CARGO_NET_OFFLINE: "true",
      CARGO_TERM_COLOR: "never",
      RUSTUP_HOME: "/Users/tachikoma/.rustup",
    },
  });
  const durationSeconds =
    Number(process.hrtime.bigint() - started) / 1_000_000_000;
  const stdout = result.stdout ?? "";
  const stderr = result.stderr ?? "";
  const timedOut = result.error?.code === "ETIMEDOUT";
  const passed = result.status === 0 && !timedOut;
  allPassed &&= passed;
  commandReceipts.push({
    command_id: `base.${caseId.toLowerCase().replace("-", "")}.${index + 1}`,
    command,
    started_at: startedAt,
    duration_seconds: durationSeconds,
    deadline_seconds: 1800,
    exit_code: result.status,
    timed_out: timedOut,
    stdout_sha256: sha256(stdout),
    stderr_sha256: sha256(stderr),
    stdout,
    stderr,
  });
}

const receipt = {
  protocol_id: "tachiko-agents-effect-v1",
  classification: "construction_pilot_only",
  formal_result_eligible: false,
  case_id: caseId,
  mode: "historical_base_control_union",
  historical_base_commit: head,
  historical_base_tree: tree,
  ground_truth_commit: benchmarkCase.ground_truth_commit,
  clean_workspace_before_commands: true,
  root_agents_absent_from_base_ancestry: true,
  network_policy: "shell denied by construction host; Cargo additionally offline",
  cargo_net_offline: true,
  command_union_rule:
    "ordered deduplicated cases.validation.base then core-score validation commands",
  command_list_sha256: sha256(Buffer.from(`${JSON.stringify(commands)}\n`, "utf8")),
  commands: commandReceipts,
  all_commands_passed: allPassed,
  recorded_at: new Date().toISOString(),
  not_claimed: [
    "same-wave formal base-control receipt",
    "production controller or account isolation qualification",
    "Baseline A, Variant B, controlled A/B, or Ultra result",
  ],
};
await mkdir(dirname(receiptPath), {recursive: true});
await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, {mode: 0o600});
console.log(
  JSON.stringify({case_id: caseId, receipt: receiptPath, all_commands_passed: allPassed}),
);
if (!allPassed) process.exitCode = 1;
