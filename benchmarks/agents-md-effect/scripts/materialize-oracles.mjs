#!/usr/bin/env node

import { createHash, randomUUID } from "node:crypto";
import { constants, existsSync } from "node:fs";
import {
  lstat,
  mkdir,
  open,
  readFile,
  realpath,
  rename,
  unlink,
  writeFile,
} from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { dirname, isAbsolute, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const benchmarkDir = resolve(scriptDir, "..");

function usage() {
  console.error(
    "usage: node materialize-oracles.mjs --case TW-03 --source-repo /abs/repo " +
      "--validation-workspace /abs/candidate-copy --trusted-dir /abs/trusted-output",
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

function gitShow(repository, commit, sourcePath) {
  const result = spawnSync(
    "rtk",
    ["proxy", "git", "show", `${commit}:${sourcePath}`],
    {
    cwd: repository,
    encoding: null,
    maxBuffer: 32 * 1024 * 1024,
    },
  );
  if (result.status !== 0) {
    fail(
      `rtk proxy git show failed for ${commit}:${sourcePath}: ` +
        Buffer.from(result.stderr ?? []).toString("utf8"),
    );
  }
  return Buffer.from(result.stdout);
}

async function ensureSafeDirectory(root, targetDirectory) {
  if (!isInside(targetDirectory, root)) {
    fail(`oracle parent escapes validation workspace: ${targetDirectory}`);
  }
  const components = relative(root, targetDirectory).split("/").filter(Boolean);
  let cursor = root;
  for (const component of components) {
    cursor = resolve(cursor, component);
    try {
      const info = await lstat(cursor);
      if (info.isSymbolicLink() || !info.isDirectory()) {
        fail(`oracle parent is not a real directory: ${cursor}`);
      }
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
      await mkdir(cursor, {mode: 0o755});
      const info = await lstat(cursor);
      if (info.isSymbolicLink() || !info.isDirectory()) {
        fail(`oracle parent creation was redirected: ${cursor}`);
      }
    }
    const resolvedCursor = await realpath(cursor);
    if (!isInside(resolvedCursor, root)) {
      fail(`oracle parent resolves outside validation workspace: ${cursor}`);
    }
  }
}

async function safeAtomicWrite(root, destination, bytes) {
  const parent = dirname(destination);
  await ensureSafeDirectory(root, parent);
  let preexistingBytes = null;
  try {
    const info = await lstat(destination);
    if (info.isSymbolicLink() || !info.isFile()) {
      fail(`oracle destination is not a regular file: ${destination}`);
    }
    preexistingBytes = await readFile(destination);
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
  }

  const temporary = resolve(parent, `.oracle-${process.pid}-${randomUUID()}`);
  let handle;
  try {
    handle = await open(
      temporary,
      constants.O_WRONLY |
        constants.O_CREAT |
        constants.O_EXCL |
        (constants.O_NOFOLLOW ?? 0),
      0o644,
    );
    await handle.writeFile(bytes);
    await handle.sync();
    await handle.close();
    handle = undefined;
    await rename(temporary, destination);
  } catch (error) {
    await handle?.close().catch(() => {});
    await unlink(temporary).catch(() => {});
    throw error;
  }

  const destinationInfo = await lstat(destination);
  if (destinationInfo.isSymbolicLink() || !destinationInfo.isFile()) {
    fail(`oracle destination changed type after writing: ${destination}`);
  }
  return {preexistingBytes, destinationInfo};
}

const args = parseArgs(process.argv.slice(2));
for (const key of ["case", "source-repo", "validation-workspace", "trusted-dir"]) {
  if (!args.has(key)) usage();
}

const caseId = args.get("case");
const sourceRepo = await realpath(resolve(args.get("source-repo")));
const validationWorkspace = await realpath(resolve(args.get("validation-workspace")));
const trustedDir = resolve(args.get("trusted-dir"));

if (!isAbsolute(trustedDir)) fail("trusted-dir must be absolute");
if (existsSync(trustedDir)) fail("trusted-dir must not already exist");
if (isInside(validationWorkspace, sourceRepo) || isInside(sourceRepo, validationWorkspace)) {
  fail("source-repo and validation-workspace must be disjoint");
}
if (isInside(trustedDir, validationWorkspace) || isInside(validationWorkspace, trustedDir)) {
  fail("trusted-dir and validation-workspace must be disjoint");
}

const lockPath = resolve(benchmarkDir, "evaluator/oracle-lock.json");
const lockBytes = await readFile(lockPath);
const lock = JSON.parse(lockBytes.toString("utf8"));
const entry = lock.cases.find((candidate) => candidate.id === caseId);
if (!entry) fail(`unknown case ${caseId}`);

await mkdir(trustedDir, { recursive: false, mode: 0o700 });
const materialized = [];
const constructedContracts = [];

for (const file of entry.files) {
  const sourceBytes = gitShow(sourceRepo, entry.source_commit, file.path);
  const sourceSha256 = sha256(sourceBytes);
  if (sourceSha256 !== file.sha256) {
    fail(
      `${caseId} oracle hash mismatch for ${file.path}: ` +
        `expected ${file.sha256}, got ${sourceSha256}`,
    );
  }

  const destination = resolve(validationWorkspace, file.path);
  if (!isInside(destination, validationWorkspace)) {
    fail(`${caseId} oracle path escapes validation workspace: ${file.path}`);
  }
  const {preexistingBytes, destinationInfo} = await safeAtomicWrite(
    validationWorkspace,
    destination,
    sourceBytes,
  );
  if (destinationInfo.size !== sourceBytes.length) {
    fail(`${caseId} oracle size mismatch after writing ${file.path}`);
  }

  materialized.push({
    path: file.path,
    source_sha256: sourceSha256,
    bytes: sourceBytes.length,
    preexisting_sha256: preexistingBytes === null ? null : sha256(preexistingBytes),
  });
}

for (const contract of entry.constructed_contracts ?? []) {
  const contractBytes = await readFile(resolve(benchmarkDir, contract.path));
  const contractSha256 = sha256(contractBytes);
  if (contractSha256 !== contract.sha256) {
    fail(`${caseId} constructed contract hash mismatch for ${contract.path}`);
  }
  constructedContracts.push({
    id: contract.id,
    path: contract.path,
    sha256: contractSha256,
    bytes: contractBytes.length,
  });
}

const receipt = {
  protocol_id: lock.protocol_id,
  case_id: caseId,
  source_commit: entry.source_commit,
  oracle_lock_sha256: sha256(lockBytes),
  mode: entry.mode,
  materialized,
  constructed_contracts: constructedContracts,
  command_specs: entry.command_specs,
  assertions: entry.assertions,
  integrity_gates: entry.integrity_gates,
  unscored_gates: entry.unscored_gates,
  group_mappings: entry.group_mappings,
  created_at: new Date().toISOString(),
};

await writeFile(
  resolve(trustedDir, "oracle-receipt.json"),
  `${JSON.stringify(receipt, null, 2)}\n`,
  { mode: 0o600 },
);

console.log(JSON.stringify(receipt));
