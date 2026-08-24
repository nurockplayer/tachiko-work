#!/usr/bin/env node

import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { dirname, isAbsolute, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const benchmarkDir = resolve(scriptDir, "..");

function usage() {
  console.error(
    "usage: node prepare-validation.mjs --case TW-03 --source-repo /abs/repo " +
      "--patch-file /abs/candidate.patch --capture-receipt /abs/capture-receipt.json " +
      "--workspace /abs/validation-copy " +
      "--trusted-dir /abs/trusted-output",
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

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function isInside(candidate, parent) {
  const pathFromParent = relative(parent, candidate);
  return (
    pathFromParent === "" ||
    (!pathFromParent.startsWith("..") && !isAbsolute(pathFromParent))
  );
}

function run(executable, args, cwd, options = {}) {
  const result = spawnSync(executable, args, {
    cwd,
    encoding: "utf8",
    maxBuffer: 128 * 1024 * 1024,
    env: options.env ?? process.env,
  });
  if (!options.allowFailure && result.status !== 0) {
    fail(`${executable} ${args.join(" ")} failed: ${result.stderr || result.stdout}`);
  }
  return result;
}

function git(args, cwd, options = {}) {
  return run("rtk", ["proxy", "git", ...args], cwd, options);
}

const args = parseArgs(process.argv.slice(2));
for (const key of [
  "case",
  "source-repo",
  "patch-file",
  "capture-receipt",
  "workspace",
  "trusted-dir",
]) {
  if (!args.has(key)) usage();
}

const caseId = args.get("case");
const sourceRepo = await realpath(resolve(args.get("source-repo")));
const patchFile = await realpath(resolve(args.get("patch-file")));
const captureReceiptPath = await realpath(resolve(args.get("capture-receipt")));
const workspace = resolve(args.get("workspace"));
const trustedDir = resolve(args.get("trusted-dir"));
for (const candidate of [workspace, trustedDir]) {
  if (!isAbsolute(candidate) || existsSync(candidate)) {
    fail("workspace and trusted-dir must be absolute paths that do not exist");
  }
}
if (
  isInside(workspace, trustedDir) ||
  isInside(trustedDir, workspace) ||
  isInside(workspace, sourceRepo) ||
  isInside(sourceRepo, workspace)
) {
  fail("source-repo, workspace, and trusted-dir must be disjoint");
}

const manifest = JSON.parse(
  await readFile(resolve(benchmarkDir, "evaluator/cases.json"), "utf8"),
);
const entry = manifest.cases.find((candidate) => candidate.id === caseId);
if (!entry) fail(`unknown case ${caseId}`);
const patchBytes = await readFile(patchFile);
if (patchBytes.length === 0) fail("empty candidate patch is a hard failure, not a validation input");
const captureReceipt = JSON.parse(await readFile(captureReceiptPath, "utf8"));
if (
  captureReceipt.case_id !== caseId ||
  captureReceipt.historical_base_commit !== entry.historical_base_commit ||
  captureReceipt.diff_sha256 !== sha256(patchBytes) ||
  !/^[0-9a-f]{40}$/.test(captureReceipt.candidate_tree)
) {
  fail("capture receipt does not bind this case, base, patch, and candidate tree");
}

await mkdir(trustedDir, {recursive: false, mode: 0o700});
const bareRepo = resolve(trustedDir, "source.git");
const bundlePath = resolve(trustedDir, "base.bundle");
git(["clone", "--bare", "--no-local", sourceRepo, bareRepo], trustedDir);
git([`--git-dir=${bareRepo}`, "update-ref", "refs/heads/benchmark-base", entry.historical_base_commit], trustedDir);
git([`--git-dir=${bareRepo}`, "bundle", "create", bundlePath, "refs/heads/benchmark-base"], trustedDir);
git([`--git-dir=${bareRepo}`, "bundle", "verify", bundlePath], trustedDir);
await mkdir(dirname(workspace), {recursive: true});
git(["clone", "--branch", "benchmark-base", bundlePath, workspace], dirname(workspace));
git(["remote", "remove", "origin"], workspace);

const targetLookup = git(["cat-file", "-e", `${entry.ground_truth_commit}^{commit}`], workspace, {allowFailure: true});
if (targetLookup.status === 0) fail("ground-truth target leaked into validation workspace");
git(["apply", "--index", "--binary", patchFile], workspace);
const appliedTree = git(["write-tree"], workspace).stdout.trim();
if (appliedTree !== captureReceipt.candidate_tree) {
  fail(
    `captured candidate tree mismatch after apply: expected ` +
      `${captureReceipt.candidate_tree}, got ${appliedTree}`,
  );
}
const stagedFiles = git(["diff", "--cached", "--name-only", "HEAD"], workspace)
  .stdout.split("\n")
  .filter(Boolean);
if (stagedFiles.includes("AGENTS.md")) fail("candidate validation patch contains AGENTS.md");

const commitEnvironment = {
  ...process.env,
  GIT_AUTHOR_NAME: "Tachiko Benchmark Evaluator",
  GIT_AUTHOR_EMAIL: "benchmark.invalid@example.invalid",
  GIT_COMMITTER_NAME: "Tachiko Benchmark Evaluator",
  GIT_COMMITTER_EMAIL: "benchmark.invalid@example.invalid",
  GIT_AUTHOR_DATE: "2000-01-01T00:00:00Z",
  GIT_COMMITTER_DATE: "2000-01-01T00:00:00Z",
};
git(["commit", "--no-verify", "--no-gpg-sign", "-m", "candidate-under-evaluation"], workspace, {env: commitEnvironment});
const candidateCommit = git(["rev-parse", "HEAD"], workspace).stdout.trim();
const candidateTree = git(["show", "-s", "--format=%T", "HEAD"], workspace).stdout.trim();

const status = git(["status", "--porcelain"], workspace).stdout.trim();
if (status !== "" && status !== "ok") fail(`validation workspace is dirty: ${status}`);

const receipt = {
  protocol_id: manifest.protocol_id,
  case_id: caseId,
  historical_base_commit: entry.historical_base_commit,
  ground_truth_commit_absent_before_overlay: true,
  candidate_patch_sha256: sha256(patchBytes),
  capture_receipt_sha256: sha256(await readFile(captureReceiptPath)),
  captured_candidate_tree: captureReceipt.candidate_tree,
  capture_to_apply_tree_equal: true,
  candidate_commit: candidateCommit,
  candidate_tree: candidateTree,
  evaluator_oracle_present: false,
  clean_for_candidate_validation: true,
  created_at: new Date().toISOString(),
};
await writeFile(
  resolve(trustedDir, "validation-preparation-receipt.json"),
  `${JSON.stringify(receipt, null, 2)}\n`,
  {mode: 0o600},
);
console.log(JSON.stringify(receipt));
