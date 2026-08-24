#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { constants } from "node:fs";
import {
  access,
  lstat,
  mkdir,
  readFile,
  readdir,
  realpath,
  statfs,
  writeFile,
} from "node:fs/promises";
import { dirname, isAbsolute, relative, resolve } from "node:path";

const CONTROL_ARTIFACTS = [
  "environment-lock.json",
  "evaluator/cases.json",
  "evaluator/oracle-lock.json",
  "evaluator/core-score-lock.json",
  "evaluator/authority-lock.json",
  "evaluator/production-oracles.json",
];
const INSTRUCTION_FILES = new Set(["AGENTS.md", "CLAUDE.md", "GEMINI.md"]);
const RUN_ROOT_LABEL = /(?:benchmark|protocol|baseline|variant|control|oracle|case|tw-0[1-9]|arm[-_.]?[ab])/i;

function usage() {
  console.error(
    "usage: node preflight-run.mjs --workspace /abs/workspace --home /abs/home " +
      "--codex-home /abs/codex-home --artifact-dir /abs/artifacts --receipt /abs/receipt.json",
  );
  process.exit(2);
}

function fail(message) {
  throw new Error(message);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function parseArgs(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined || values.has(key)) usage();
    values.set(key, value);
  }
  return values;
}

function relativePath(parent, child) {
  const pathFromParent = relative(parent, child);
  return pathFromParent === "" || (!pathFromParent.startsWith("..") && !isAbsolute(pathFromParent));
}

async function filesystemIdentity(path) {
  const info = await lstat(path);
  return {
    path,
    device: Number(info.dev),
    inode: Number(info.ino),
    type: info.isDirectory() ? "directory" : info.isFile() ? "file" : "other",
    owner: Number(info.uid),
    group: Number(info.gid),
    mode: info.mode & 0o7777,
  };
}

async function scanTree(root) {
  const entries = [];
  async function walk(directory) {
    const children = await readdir(directory);
    children.sort();
    for (const child of children) {
      const absolute = resolve(directory, child);
      const info = await lstat(absolute);
      const path = relative(root, absolute);
      const type = info.isDirectory()
        ? "directory"
        : info.isFile()
          ? "file"
          : info.isSymbolicLink()
            ? "symlink"
            : "other";
      entries.push({ path, type });
      if (info.isDirectory()) await walk(absolute);
    }
  }
  await walk(root);
  return entries;
}

async function scanWorkspaceInstructions(workspace) {
  const exposures = [];
  let directory = workspace;
  let isWorkspace = true;
  while (true) {
    for (const name of [...INSTRUCTION_FILES].sort()) {
      const candidate = resolve(directory, name);
      try {
        const info = await lstat(candidate);
        exposures.push({ path: candidate, name, type: info.isFile() ? "file" : "other" });
        if (!isWorkspace || name !== "AGENTS.md") {
          fail(`workspace ancestor instruction exposure: ${candidate}`);
        }
      } catch (error) {
        if (error?.code !== "ENOENT") throw error;
      }
    }
    const parent = dirname(directory);
    if (parent === directory) break;
    directory = parent;
    isWorkspace = false;
  }
  return exposures;
}

async function resolveExecutable(name) {
  for (const component of (process.env.PATH ?? "").split(":")) {
    if (!component) continue;
    const candidate = resolve(component, name);
    try {
      await access(candidate, constants.X_OK);
      return realpath(candidate);
    } catch (error) {
      if (error?.code !== "ENOENT" && error?.code !== "EACCES") throw error;
    }
  }
  fail(`required executable is unavailable: ${name}`);
}

async function observeExecutable(name, path, argumentsForVersion) {
  const resolved = await realpath(path);
  const bytes = await readFile(resolved);
  const result = spawnSync(resolved, argumentsForVersion, {
    encoding: "utf8",
    env: process.env,
  });
  if (result.error || result.status !== 0) {
    fail(`required executable probe failed: ${name}`);
  }
  return {
    path: resolved,
    sha256: sha256(bytes),
    version: result.stdout.trim(),
  };
}

async function controlObservations(artifactDir) {
  const artifacts = [];
  const parsed = new Map();
  for (const path of CONTROL_ARTIFACTS) {
    const absolute = resolve(artifactDir, path);
    if (!relativePath(artifactDir, absolute)) fail(`control artifact escapes artifact directory: ${path}`);
    const info = await lstat(absolute);
    if (!info.isFile()) fail(`control artifact is not a regular file: ${path}`);
    const bytes = await readFile(absolute);
    try {
      parsed.set(path, JSON.parse(bytes.toString("utf8")));
    } catch {
      fail(`control artifact is not valid JSON: ${path}`);
    }
    artifacts.push({ path, bytes: bytes.length, sha256: sha256(bytes) });
  }
  const lock = parsed.get("environment-lock.json");
  const productionOracles = parsed.get("evaluator/production-oracles.json");
  if (!lock?.protocol_id || lock.protocol_id !== productionOracles?.protocol_id) {
    fail("environment lock and production oracle manifest disagree on protocol_id");
  }
  const controlBytes = Buffer.from(`${JSON.stringify(artifacts)}\n`, "utf8");
  return {
    protocol_id: lock.protocol_id,
    artifacts,
    sha256: sha256(controlBytes),
  };
}

async function writeReceipt(path, receipt) {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(receipt, null, 2)}\n`, {
    encoding: "utf8",
    mode: 0o600,
    flag: "wx",
  });
}

let receiptPath;
try {
  const args = parseArgs(process.argv.slice(2));
  for (const key of ["--workspace", "--home", "--codex-home", "--artifact-dir", "--receipt"]) {
    if (!args.has(key)) usage();
    if (!isAbsolute(args.get(key))) fail(`${key} must be an absolute path`);
  }

  receiptPath = resolve(args.get("--receipt"));
  const [workspace, home, codexHome, artifactDir] = await Promise.all([
    realpath(args.get("--workspace")),
    realpath(args.get("--home")),
    realpath(args.get("--codex-home")),
    realpath(args.get("--artifact-dir")),
  ]);
  const runRoot = dirname(workspace);
  if (dirname(home) !== runRoot || dirname(codexHome) !== runRoot) {
    fail("workspace, HOME, and CODEX_HOME must be direct children of one run root");
  }
  if (RUN_ROOT_LABEL.test(runRoot)) fail("run root must use an opaque neutral name");
  if (relativePath(runRoot, artifactDir) || relativePath(runRoot, receiptPath)) {
    fail("controller artifacts and receipts must be outside the agent run root");
  }
  const [environmentHome, environmentCodexHome] = await Promise.all([
    process.env.HOME ? realpath(process.env.HOME) : Promise.resolve(undefined),
    process.env.CODEX_HOME ? realpath(process.env.CODEX_HOME) : Promise.resolve(undefined),
  ]);
  if (environmentHome !== home) fail("HOME must equal the supplied neutral HOME");
  if (environmentCodexHome !== codexHome) {
    fail("CODEX_HOME must equal the supplied neutral CODEX_HOME");
  }

  const [workspaceIdentity, homeIdentity, codexHomeIdentity, homeEntries, codexHomeEntries] =
    await Promise.all([
      filesystemIdentity(workspace),
      filesystemIdentity(home),
      filesystemIdentity(codexHome),
      scanTree(home),
      scanTree(codexHome),
    ]);
  for (const [label, identity] of [
    ["workspace", workspaceIdentity],
    ["HOME", homeIdentity],
    ["CODEX_HOME", codexHomeIdentity],
  ]) {
    if (identity.type !== "directory") fail(`${label} must be a real directory`);
  }
  if (homeEntries.length !== 0) fail("neutral HOME must be empty");
  if (codexHomeEntries.length !== 0) fail("neutral CODEX_HOME must be empty");

  const [instructions, controls, bashPath, gitPath, filesystem] = await Promise.all([
    scanWorkspaceInstructions(workspace),
    controlObservations(artifactDir),
    resolveExecutable("bash"),
    resolveExecutable("git"),
    statfs(workspace),
  ]);
  const expectedControlSha256 = process.env.PREFLIGHT_CONTROL_SHA256;
  if (expectedControlSha256 !== undefined && expectedControlSha256 !== controls.sha256) {
    fail(`control SHA-256 mismatch: expected ${expectedControlSha256}, got ${controls.sha256}`);
  }
  const [node, bash, git] = await Promise.all([
    observeExecutable("node", process.execPath, ["--version"]),
    observeExecutable("bash", bashPath, ["--version"]),
    observeExecutable("git", gitPath, ["--version"]),
  ]);
  const receipt = {
    protocol_id: controls.protocol_id,
    valid: true,
    paths: { run_root: runRoot, workspace, home, codex_home: codexHome, artifact_dir: artifactDir },
    environment: {
      HOME: process.env.HOME,
      CODEX_HOME: process.env.CODEX_HOME,
      PATH: process.env.PATH ?? "",
      LANG: process.env.LANG ?? null,
      LC_ALL: process.env.LC_ALL ?? null,
      TZ: process.env.TZ ?? null,
      PREFLIGHT_CONTROL_SHA256: expectedControlSha256 ?? null,
    },
    filesystem: { workspace: workspaceIdentity, home: homeIdentity, codex_home: codexHomeIdentity },
    scans: {
      workspace_instructions: instructions,
      home: { entries: homeEntries },
      codex_home: { entries: codexHomeEntries },
    },
    binaries: { node, bash, git },
    free_space: { bytes: Number(filesystem.bavail * filesystem.bsize) },
    controls,
  };
  await writeReceipt(receiptPath, receipt);
  console.log(JSON.stringify(receipt));
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  if (receiptPath) {
    try {
      await writeReceipt(receiptPath, { valid: false, error: message });
    } catch {
      // The caller receives the original preflight failure even when its receipt path is unusable.
    }
  }
  console.error(message);
  process.exitCode = 1;
}
