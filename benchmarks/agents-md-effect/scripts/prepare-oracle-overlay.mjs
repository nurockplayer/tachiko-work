#!/usr/bin/env node

import {createHash} from "node:crypto";
import {existsSync} from "node:fs";
import {mkdir, readFile, realpath, writeFile} from "node:fs/promises";
import {spawnSync} from "node:child_process";
import {dirname, isAbsolute, relative, resolve} from "node:path";
import {fileURLToPath} from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const benchmarkDir = resolve(scriptDir, "..");

function usage() {
  console.error(
    "usage: node prepare-oracle-overlay.mjs --case TW-03 --source-repo /abs/repo " +
      "--workspace /abs/validation-copy --candidate-receipt /abs/receipt.json " +
      "--trusted-dir /abs/trusted-oracle-output",
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
  "workspace",
  "candidate-receipt",
  "trusted-dir",
]) {
  if (!args.has(key)) usage();
}

const caseId = args.get("case");
const sourceRepo = await realpath(resolve(args.get("source-repo")));
const workspace = await realpath(resolve(args.get("workspace")));
const candidateReceiptPath = await realpath(resolve(args.get("candidate-receipt")));
const trustedDir = resolve(args.get("trusted-dir"));
if (!isAbsolute(trustedDir) || existsSync(trustedDir)) {
  fail("trusted-dir must be an absolute path that does not exist");
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

const candidateReceiptBytes = await readFile(candidateReceiptPath);
const candidateReceipt = JSON.parse(candidateReceiptBytes.toString("utf8"));
if (
  candidateReceipt.case_id !== caseId ||
  candidateReceipt.historical_base_commit !== entry.historical_base_commit ||
  candidateReceipt.evaluator_oracle_present !== false ||
  candidateReceipt.clean_for_candidate_validation !== true ||
  !/^[0-9a-f]{40}$/.test(candidateReceipt.candidate_commit) ||
  !/^[0-9a-f]{40}$/.test(candidateReceipt.candidate_tree)
) {
  fail("candidate receipt is not valid for this case and stage");
}

const head = git(["rev-parse", "HEAD"], workspace).stdout.trim();
const tree = git(["show", "-s", "--format=%T", "HEAD"], workspace).stdout.trim();
const parent = git(["rev-parse", "HEAD^"], workspace).stdout.trim();
const statusBefore = git(["status", "--porcelain"], workspace).stdout.trim();
if (
  head !== candidateReceipt.candidate_commit ||
  tree !== candidateReceipt.candidate_tree ||
  parent !== entry.historical_base_commit ||
  statusBefore !== ""
) {
  fail("validation workspace no longer matches the frozen clean candidate tree");
}
const targetLookup = git(
  ["cat-file", "-e", `${entry.ground_truth_commit}^{commit}`],
  workspace,
  {allowFailure: true},
);
if (targetLookup.status === 0) fail("ground-truth target leaked before oracle overlay");

await mkdir(trustedDir, {recursive: false, mode: 0o700});
const oracleReceiptDir = resolve(trustedDir, "oracle");
run(
  process.execPath,
  [
    resolve(scriptDir, "materialize-oracles.mjs"),
    "--case",
    caseId,
    "--source-repo",
    sourceRepo,
    "--validation-workspace",
    workspace,
    "--trusted-dir",
    oracleReceiptDir,
  ],
  benchmarkDir,
);

const commitEnvironment = {
  ...process.env,
  GIT_AUTHOR_NAME: "Tachiko Benchmark Evaluator",
  GIT_AUTHOR_EMAIL: "benchmark.invalid@example.invalid",
  GIT_COMMITTER_NAME: "Tachiko Benchmark Evaluator",
  GIT_COMMITTER_EMAIL: "benchmark.invalid@example.invalid",
  GIT_AUTHOR_DATE: "2000-01-01T00:00:01Z",
  GIT_COMMITTER_DATE: "2000-01-01T00:00:01Z",
};
git(["add", "-A"], workspace);
git(
  ["commit", "--allow-empty", "--no-verify", "--no-gpg-sign", "-m", "evaluator-oracle-overlay"],
  workspace,
  {env: commitEnvironment},
);

const evaluatorCommit = git(["rev-parse", "HEAD"], workspace).stdout.trim();
const evaluatorTree = git(["show", "-s", "--format=%T", "HEAD"], workspace).stdout.trim();
const evaluatorParent = git(["rev-parse", "HEAD^"], workspace).stdout.trim();
const statusAfter = git(["status", "--porcelain"], workspace).stdout.trim();
if (evaluatorParent !== head || statusAfter !== "") {
  fail("oracle overlay did not produce a clean child of the frozen candidate commit");
}

const receipt = {
  protocol_id: manifest.protocol_id,
  case_id: caseId,
  historical_base_commit: entry.historical_base_commit,
  candidate_preparation_receipt_sha256: sha256(candidateReceiptBytes),
  candidate_commit: head,
  candidate_tree: tree,
  evaluator_commit: evaluatorCommit,
  evaluator_tree: evaluatorTree,
  evaluator_parent_is_candidate: true,
  clean_for_oracle_validation: true,
  created_at: new Date().toISOString(),
};
await writeFile(
  resolve(trustedDir, "oracle-overlay-receipt.json"),
  `${JSON.stringify(receipt, null, 2)}\n`,
  {mode: 0o600},
);
console.log(JSON.stringify(receipt));
