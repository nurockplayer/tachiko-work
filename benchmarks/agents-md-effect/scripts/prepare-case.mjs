#!/usr/bin/env node

import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { mkdir, readFile, realpath, rm, stat, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { dirname, isAbsolute, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const benchmarkDir = resolve(scriptDir, "..");

function usage() {
  console.error(
    "usage: node prepare-case.mjs --case TW-01 --source-repo /abs/repo " +
      "--variant-file /abs/AGENTS.md --workspace /abs/workspace " +
      "--trusted-dir /abs/trusted-output --expected-variant-sha256 <sha256>",
  );
  process.exit(2);
}

function parseArgs(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined) usage();
    values.set(key.slice(2), value);
  }
  return values;
}

function fail(message) {
  throw new Error(message);
}

function run(command, args, cwd, allowFailure = false) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  if (!allowFailure && result.status !== 0) {
    fail(`${command} ${args.join(" ")} failed: ${result.stderr || result.stdout}`);
  }
  return result;
}

function git(args, cwd, allowFailure = false) {
  return run("rtk", ["proxy", "git", ...args], cwd, allowFailure);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function isInside(candidate, parent) {
  const path = relative(parent, candidate);
  return path === "" || (!path.startsWith("..") && !isAbsolute(path));
}

const args = parseArgs(process.argv.slice(2));
const required = [
  "case",
  "source-repo",
  "variant-file",
  "workspace",
  "trusted-dir",
  "expected-variant-sha256",
];
for (const key of required) if (!args.has(key)) usage();

const caseId = args.get("case");
const sourceRepo = await realpath(resolve(args.get("source-repo")));
const variantFile = await realpath(resolve(args.get("variant-file")));
const workspace = resolve(args.get("workspace"));
const trustedDir = resolve(args.get("trusted-dir"));
const expectedVariantSha256 = args.get("expected-variant-sha256");

if (!isAbsolute(workspace) || !isAbsolute(trustedDir)) {
  fail("workspace and trusted-dir must be absolute");
}
if (existsSync(workspace) || existsSync(trustedDir)) {
  fail("workspace and trusted-dir must not already exist");
}
if (isInside(workspace, trustedDir) || isInside(trustedDir, workspace)) {
  fail("workspace and trusted-dir must be disjoint");
}
if (isInside(workspace, sourceRepo) || isInside(sourceRepo, workspace)) {
  fail("workspace and source-repo must be disjoint");
}
if (/(?:benchmark|baseline|variant|tw-0[1-9]|arm[-_.]?[ab])/i.test(workspace)) {
  fail("agent workspace path must use an opaque neutral name");
}

const manifest = JSON.parse(
  await readFile(resolve(benchmarkDir, "evaluator/cases.json"), "utf8"),
);
const entry = manifest.cases.find((candidate) => candidate.id === caseId);
if (!entry) fail(`unknown case ${caseId}`);

const variantBytes = await readFile(variantFile);
const variantSha256 = sha256(variantBytes);
if (variantSha256 !== expectedVariantSha256) {
  fail(`variant SHA-256 mismatch: expected ${expectedVariantSha256}, got ${variantSha256}`);
}

await mkdir(trustedDir, { recursive: false, mode: 0o700 });
const bareRepo = resolve(trustedDir, "source.git");
const bundlePath = resolve(trustedDir, "base.bundle");

git(["clone", "--bare", "--no-local", sourceRepo, bareRepo], trustedDir);
git(
  [
    `--git-dir=${bareRepo}`,
    "update-ref",
    "refs/heads/work",
    entry.historical_base_commit,
  ],
  trustedDir,
);
git(
  [
    `--git-dir=${bareRepo}`,
    "bundle",
    "create",
    bundlePath,
    "refs/heads/work",
  ],
  trustedDir,
);
git([`--git-dir=${bareRepo}`, "bundle", "verify", bundlePath], trustedDir);

await mkdir(dirname(workspace), { recursive: true });
git(["clone", "--branch", "work", bundlePath, workspace], dirname(workspace));
git(["remote", "remove", "origin"], workspace);
git(["config", "core.logAllRefUpdates", "false"], workspace);
await rm(resolve(workspace, ".git/logs"), { recursive: true, force: true });
if (existsSync(resolve(workspace, ".git/logs"))) fail("agent workspace retains Git reflogs");
if (existsSync(resolve(workspace, ".git/objects/info/alternates"))) {
  fail("agent workspace contains a Git alternates pointer");
}
const remotes = git(["remote"], workspace).stdout.trim();
if (remotes !== "") fail("agent workspace retains a Git remote");

const head = git(["rev-parse", "HEAD"], workspace).stdout.trim();
const tree = git(["show", "-s", "--format=%T", "HEAD"], workspace).stdout.trim();
if (head !== entry.historical_base_commit) fail(`prepared HEAD mismatch for ${caseId}`);
if (tree !== entry.historical_base_tree) fail(`prepared tree mismatch for ${caseId}`);

const targetLookup = git(
  ["cat-file", "-e", `${entry.ground_truth_commit}^{commit}`],
  workspace,
  true,
);
if (targetLookup.status === 0) fail(`ground-truth target leaked into ${caseId} bundle`);

const unreachable = git(["fsck", "--no-reflogs", "--unreachable"], workspace);
if (unreachable.stdout.trim() !== "") fail(`${caseId} workspace contains unreachable objects`);

const historicalAgents = git(
  ["rev-list", "--objects", "HEAD", "--", "AGENTS.md"],
  workspace,
).stdout
  .split("\n")
  .some((line) => line.endsWith(" AGENTS.md"));
if (historicalAgents) {
  fail(`${caseId} base history contains a root AGENTS.md treatment-detection channel`);
}
const trackedAgents = git(["ls-files", "--error-unmatch", "AGENTS.md"], workspace, true);
if (trackedAgents.status === 0) fail(`${caseId} historical base tracks root AGENTS.md`);
const excludePath = resolve(workspace, ".git/info/exclude");
const exclude = await readFile(excludePath, "utf8");
const suffix = exclude.endsWith("\n") ? "" : "\n";
await writeFile(excludePath, `${exclude}${suffix}/AGENTS.md\n`, "utf8");
await writeFile(resolve(workspace, "AGENTS.md"), variantBytes, { mode: 0o644 });

const status = git(["status", "--porcelain"], workspace).stdout.trim();
if (status !== "" && status !== "ok") {
  fail(`prepared workspace is not status-clean: ${status}`);
}

const taskPath = resolve(benchmarkDir, entry.task_file);
const taskBytes = await readFile(taskPath);
const taskInfo = await stat(taskPath);
if (sha256(taskBytes) !== entry.task_sha256 || taskInfo.size !== entry.task_bytes) {
  fail(`${caseId} task lock mismatch during preparation`);
}

const receipt = {
  protocol_id: manifest.protocol_id,
  case_id: caseId,
  historical_base_commit: head,
  historical_base_tree: tree,
  ground_truth_commit_absent: true,
  unreachable_objects_absent: true,
  github_remote_absent: true,
  historical_root_agents_absent_from_base_history: true,
  agents_sha256: variantSha256,
  agents_bytes: variantBytes.length,
  task_sha256: entry.task_sha256,
  task_bytes: entry.task_bytes,
  workspace,
  created_at: new Date().toISOString(),
};
await writeFile(
  resolve(trustedDir, "preparation-receipt.json"),
  `${JSON.stringify(receipt, null, 2)}\n`,
  { mode: 0o600 },
);

console.log(JSON.stringify(receipt));
