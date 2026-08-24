#!/usr/bin/env node

import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { copyFile, mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { dirname, isAbsolute, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const benchmarkDir = resolve(scriptDir, "..");

function usage() {
  console.error(
    "usage: node capture-candidate.mjs --case TW-01 --workspace /abs/workspace " +
      "--trusted-dir /abs/output --expected-agents-sha256 <sha256>",
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

function git(args, cwd, allowFailure = false) {
  const result = spawnSync("rtk", ["proxy", "git", ...args], {
    cwd,
    encoding: null,
    maxBuffer: 128 * 1024 * 1024,
  });
  if (!allowFailure && result.status !== 0) {
    fail(
      `rtk proxy git ${args.join(" ")} failed: ` +
        Buffer.from(result.stderr ?? result.stdout ?? []).toString("utf8"),
    );
  }
  return result;
}

const args = parseArgs(process.argv.slice(2));
for (const key of ["case", "workspace", "trusted-dir", "expected-agents-sha256"]) {
  if (!args.has(key)) usage();
}

const caseId = args.get("case");
const workspace = await realpath(resolve(args.get("workspace")));
const trustedDir = resolve(args.get("trusted-dir"));
const expectedAgentsSha256 = args.get("expected-agents-sha256");
if (!isAbsolute(trustedDir) || existsSync(trustedDir)) {
  fail("trusted-dir must be an absolute path that does not exist");
}
if (isInside(trustedDir, workspace) || isInside(workspace, trustedDir)) {
  fail("trusted-dir and workspace must be disjoint");
}
const manifest = JSON.parse(
  await readFile(resolve(benchmarkDir, "evaluator/cases.json"), "utf8"),
);
const entry = manifest.cases.find((candidate) => candidate.id === caseId);
if (!entry) fail(`unknown case ${caseId}`);

const agentsBytes = await readFile(resolve(workspace, "AGENTS.md"));
if (sha256(agentsBytes) !== expectedAgentsSha256) {
  fail("AGENTS.md overlay changed during the candidate run");
}
const head = Buffer.from(git(["rev-parse", "HEAD"], workspace).stdout)
  .toString("utf8")
  .trim();
const baseIsAncestor = git(
  ["merge-base", "--is-ancestor", entry.historical_base_commit, head],
  workspace,
  true,
);
if (baseIsAncestor.status !== 0) {
  fail("candidate history no longer descends from the fixed historical base");
}

await mkdir(trustedDir, { recursive: false, mode: 0o700 });
const indexPath = resolve(workspace, ".git/index");
const indexBackup = resolve(trustedDir, "index.before");
await copyFile(indexPath, indexBackup);

let patchBytes;
let changedFiles;
let numstat;
let candidateTree;
try {
  git(["add", "-A"], workspace);
  candidateTree = Buffer.from(git(["write-tree"], workspace).stdout)
    .toString("utf8")
    .trim();
  patchBytes = Buffer.from(
    git(
      [
        "diff",
        "--cached",
        "--binary",
        "--full-index",
        entry.historical_base_commit,
      ],
      workspace,
    ).stdout,
  );
  changedFiles = Buffer.from(
    git(
      ["diff", "--cached", "--name-only", "-z", entry.historical_base_commit],
      workspace,
    ).stdout,
  )
    .toString("utf8")
    .split("\0")
    .filter(Boolean);
  numstat = Buffer.from(
    git(["diff", "--cached", "--numstat", entry.historical_base_commit], workspace)
      .stdout,
  ).toString("utf8");
} finally {
  await copyFile(indexBackup, indexPath);
}

if (patchBytes.length > 0 && !patchBytes.subarray(0, 11).equals(Buffer.from("diff --git "))) {
  fail("captured non-empty candidate patch is not a raw applyable Git patch");
}

if (changedFiles.includes("AGENTS.md")) {
  fail("captured candidate patch contains AGENTS.md");
}
const totals = numstat
  .split("\n")
  .filter(Boolean)
  .reduce(
    (sum, line) => {
      const [insertions, deletions] = line.split("\t");
      if (/^\d+$/.test(insertions)) sum.insertions += Number(insertions);
      if (/^\d+$/.test(deletions)) sum.deletions += Number(deletions);
      return sum;
    },
    {insertions: 0, deletions: 0},
  );

const patchPath = resolve(trustedDir, "candidate.patch");
await writeFile(patchPath, patchBytes, {mode: 0o600});
const receipt = {
  protocol_id: manifest.protocol_id,
  case_id: caseId,
  historical_base_commit: entry.historical_base_commit,
  head_after: head,
  candidate_tree: candidateTree,
  commits_ahead: Number(
    Buffer.from(
      git(
        ["rev-list", "--count", `${entry.historical_base_commit}..${head}`],
        workspace,
      ).stdout,
    )
      .toString("utf8")
      .trim(),
  ),
  agents_sha256_after: expectedAgentsSha256,
  agents_unchanged: true,
  diff_sha256: sha256(patchBytes),
  diff_bytes: patchBytes.length,
  changed_files: changedFiles,
  insertions: totals.insertions,
  deletions: totals.deletions,
  empty_patch: patchBytes.length === 0,
  created_at: new Date().toISOString(),
};
await writeFile(
  resolve(trustedDir, "capture-receipt.json"),
  `${JSON.stringify(receipt, null, 2)}\n`,
  {mode: 0o600},
);
console.log(JSON.stringify(receipt));
