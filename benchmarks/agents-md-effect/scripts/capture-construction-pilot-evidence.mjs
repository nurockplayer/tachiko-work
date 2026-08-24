#!/usr/bin/env node

import {createHash} from "node:crypto";
import {existsSync} from "node:fs";
import {mkdir, realpath, writeFile} from "node:fs/promises";
import {spawnSync} from "node:child_process";
import {dirname, isAbsolute, resolve} from "node:path";

function usage() {
  console.error(
    "usage: node capture-construction-pilot-evidence.mjs --case TW-08 " +
      "--mode historical_target --workspace /abs/repo --base <sha> " +
      "--ground-truth <sha> --receipt /abs/receipt.json --command '<command>' [...]",
  );
  process.exit(2);
}

function parseArgs(argv) {
  const values = new Map();
  const commands = [];
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined) usage();
    if (key === "--command") commands.push(value);
    else if (values.has(key.slice(2))) usage();
    else values.set(key.slice(2), value);
  }
  return {values, commands};
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function runGit(root, args, allowFailure = false) {
  const result = spawnSync("rtk", ["proxy", "git", ...args], {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (!allowFailure && result.status !== 0) {
    throw new Error(result.stderr || result.stdout);
  }
  return result;
}

const {values, commands} = parseArgs(process.argv.slice(2));
for (const key of ["case", "mode", "workspace", "base", "ground-truth", "receipt"]) {
  if (!values.has(key)) usage();
}
if (commands.length === 0) usage();
if (!/^TW-0[1-9]$/.test(values.get("case"))) usage();
if (!/^[0-9a-f]{40}$/.test(values.get("base"))) usage();
if (!/^[0-9a-f]{40}$/.test(values.get("ground-truth"))) usage();

const workspace = await realpath(resolve(values.get("workspace")));
const receiptPath = resolve(values.get("receipt"));
if (!isAbsolute(receiptPath) || existsSync(receiptPath)) {
  throw new Error("receipt must be a new absolute path");
}
const head = runGit(workspace, ["rev-parse", "HEAD^{commit}"]).stdout.trim();
const headTree = runGit(workspace, ["rev-parse", "HEAD^{tree}"]).stdout.trim();
const status = runGit(workspace, ["status", "--porcelain=v1", "-z"]).stdout;
if (status !== "") throw new Error("pilot workspace must be clean and committed");
const parent = runGit(workspace, ["rev-parse", "HEAD^1"]).stdout.trim();
if (parent !== values.get("base")) {
  throw new Error("pilot subject must be a direct child of the declared replay base");
}
const rootAgents = runGit(
  workspace,
  ["ls-tree", "-r", "--name-only", values.get("base"), "--", "AGENTS.md"],
).stdout.trim();
if (rootAgents !== "") throw new Error("pilot base unexpectedly tracks root AGENTS.md");

const commandReceipts = [];
let allPassed = true;
for (const command of commands) {
  const startedAt = new Date().toISOString();
  const started = process.hrtime.bigint();
  const result = spawnSync("/bin/zsh", ["-f", "-c", command], {
    cwd: workspace,
    encoding: "utf8",
    timeout: 1_800_000,
    maxBuffer: 128 * 1024 * 1024,
    env: {...process.env, CARGO_TERM_COLOR: "never", CARGO_INCREMENTAL: "0"},
  });
  const ended = process.hrtime.bigint();
  const stdout = result.stdout ?? "";
  const stderr = result.stderr ?? "";
  const exitCode = result.status;
  const timedOut = result.error?.code === "ETIMEDOUT";
  if (exitCode !== 0 || timedOut) allPassed = false;
  commandReceipts.push({
    command,
    started_at: startedAt,
    duration_seconds: Number(ended - started) / 1_000_000_000,
    exit_code: exitCode,
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
  case_id: values.get("case"),
  mode: values.get("mode"),
  historical_base_commit: values.get("base"),
  ground_truth_commit: values.get("ground-truth"),
  pilot_subject_commit: head,
  pilot_subject_tree: headTree,
  pilot_parent_is_replay_base: true,
  clean_workspace: true,
  root_agents_absent_from_base_tree: true,
  commands: commandReceipts,
  all_commands_passed: allPassed,
  recorded_at: new Date().toISOString(),
};
await mkdir(dirname(receiptPath), {recursive: true});
await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, {mode: 0o600});
console.log(JSON.stringify({receipt: receiptPath, all_commands_passed: allPassed}));
if (!allPassed) process.exitCode = 1;
