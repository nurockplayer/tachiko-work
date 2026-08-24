#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { constants } from "node:fs";
import {
  access,
  lstat,
  readFile,
  readlink,
  readdir,
  realpath,
  stat,
  statfs,
  writeFile,
} from "node:fs/promises";
import { basename, dirname, isAbsolute, relative, resolve } from "node:path";

const CONTROL_ARTIFACTS = [
  "environment-lock.json",
  "evaluator/cases.json",
  "evaluator/oracle-lock.json",
  "evaluator/core-score-lock.json",
  "evaluator/authority-lock.json",
  "evaluator/production-oracles.json",
];
const INSTRUCTION_FILES = new Set(["AGENTS.md", "CLAUDE.md", "GEMINI.md"]);
const RUN_ROOT_NAME = /^r-[0-9a-f]{32}$/;

function usage() {
  console.error(
    "usage: node preflight-run.mjs --workspace /abs/workspace --home /abs/home " +
      "--codex-home /abs/codex-home --artifact-dir /abs/artifacts --receipt /abs/receipt.json " +
      "--expected-agents-sha256 <sha256> --expected-control-sha256 <sha256>",
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

async function scanWorkspaceInstructions(workspace, expectedAgentsSha256) {
  const rootAgents = resolve(workspace, "AGENTS.md");
  const rootInfo = await lstat(rootAgents).catch((error) => {
    if (error?.code === "ENOENT") fail("workspace root AGENTS.md is required");
    throw error;
  });
  if (!rootInfo.isFile() || rootInfo.isSymbolicLink()) {
    fail("root AGENTS.md must be a regular non-symlink file");
  }
  const rootBytes = await readFile(rootAgents);
  const rootSha256 = sha256(rootBytes);
  if (rootSha256 !== expectedAgentsSha256) {
    fail(`AGENTS.md SHA-256 mismatch: expected ${expectedAgentsSha256}, got ${rootSha256}`);
  }

  const exposures = [{ path: rootAgents, name: "AGENTS.md", type: "file", sha256: rootSha256 }];
  async function walk(directory) {
    const children = await readdir(directory);
    children.sort();
    for (const child of children) {
      const candidate = resolve(directory, child);
      const info = await lstat(candidate);
      if (INSTRUCTION_FILES.has(child) && candidate !== rootAgents) {
        fail(`nested workspace instruction exposure: ${candidate}`);
      }
      if (info.isSymbolicLink()) {
        const target = await stat(candidate).catch(() => {
          fail(`workspace contains an unresolved symlink: ${candidate}`);
        });
        if (target.isDirectory()) {
          fail(`workspace contains a symlinked directory: ${candidate}`);
        }
      }
      if (info.isDirectory()) await walk(candidate);
    }
  }
  await walk(workspace);

  for (let directory = dirname(workspace); ; directory = dirname(directory)) {
    for (const name of [...INSTRUCTION_FILES].sort()) {
      const candidate = resolve(directory, name);
      try {
        const info = await lstat(candidate);
        exposures.push({ path: candidate, name, type: info.isFile() ? "file" : "other" });
        fail(`workspace ancestor instruction exposure: ${candidate}`);
      } catch (error) {
        if (error?.code !== "ENOENT") throw error;
      }
    }
    if (dirname(directory) === directory) break;
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
    bytes: bytes.length,
    sha256: sha256(bytes),
    version: result.stdout.trim(),
  };
}

function command(executable, args) {
  const result = spawnSync(executable, args, { encoding: "utf8", env: process.env });
  if (result.error || result.status !== 0) {
    fail(`required executable probe failed: ${executable} ${args.join(" ")}`);
  }
  return result.stdout.trim();
}

async function rustupWhich(rustupPath, executable) {
  const path = command(rustupPath, ["which", executable]);
  if (!path) fail(`rustup could not locate required executable: ${executable}`);
  return realpath(path);
}

async function hashTree(root) {
  const entries = [];
  let fileBytes = 0;
  let regularFiles = 0;
  async function walk(directory) {
    const children = await readdir(directory);
    children.sort();
    for (const child of children) {
      const absolute = resolve(directory, child);
      const info = await lstat(absolute);
      const path = relative(root, absolute);
      if (info.isDirectory()) {
        entries.push({ path, type: "directory" });
        await walk(absolute);
      } else if (info.isFile()) {
        const bytes = await readFile(absolute);
        regularFiles += 1;
        fileBytes += bytes.length;
        entries.push({ path, type: "file", bytes: bytes.length, sha256: sha256(bytes) });
      } else if (info.isSymbolicLink()) {
        entries.push({ path, type: "symlink", target: await readlink(absolute) });
      } else {
        fail(`unsupported Rust target artifact: ${absolute}`);
      }
    }
  }
  await walk(root);
  return {
    path: root,
    entries: entries.length,
    regular_files: regularFiles,
    file_bytes: fileBytes,
    sha256: sha256(Buffer.from(`${JSON.stringify(entries)}\n`, "utf8")),
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
  await writeFile(path, `${JSON.stringify(receipt, null, 2)}\n`, {
    encoding: "utf8",
    mode: 0o600,
    flag: "wx",
  });
}

async function validateReceiptPath(receipt, artifactDir, runRoot) {
  const receiptName = basename(receipt);
  if (!receiptName || receiptName === "." || receiptName === "..") {
    fail("receipt must name a new file inside the trusted artifact directory");
  }
  const parent = await realpath(dirname(receipt));
  if (!relativePath(artifactDir, parent) || relativePath(runRoot, parent)) {
    fail("receipt must remain inside the trusted artifact directory and outside the agent run root");
  }
  return resolve(parent, receiptName);
}

async function resolveRunDirectory(name, runRoot) {
  const value = process.env[name];
  if (!value) fail(`${name} must be set`);
  const resolved = await realpath(value);
  const info = await lstat(resolved);
  if (!info.isDirectory()) fail(`${name} must name a directory`);
  if (!relativePath(runRoot, resolved)) fail(`${name} must remain within the opaque run root`);
  return resolved;
}

let receiptPath;
try {
  const args = parseArgs(process.argv.slice(2));
  for (const key of ["--workspace", "--home", "--codex-home", "--artifact-dir", "--receipt"]) {
    if (!args.has(key)) usage();
    if (!isAbsolute(args.get(key))) fail(`${key} must be an absolute path`);
  }
  for (const key of ["--expected-agents-sha256", "--expected-control-sha256"]) {
    if (!args.has(key)) fail(`${key} is required`);
  }
  const expectedAgentsSha256 = args.get("--expected-agents-sha256");
  const expectedControlSha256 = args.get("--expected-control-sha256");
  for (const [key, value] of [
    ["--expected-agents-sha256", expectedAgentsSha256],
    ["--expected-control-sha256", expectedControlSha256],
  ]) {
    if (!/^[0-9a-f]{64}$/.test(value)) fail(`${key} must be 64 lowercase hexadecimal characters`);
  }
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
  if (!RUN_ROOT_NAME.test(basename(runRoot))) {
    fail("run root must use the opaque r-<32-lowercase-hex> name grammar");
  }
  if (relativePath(runRoot, artifactDir)) fail("controller artifacts must be outside the agent run root");
  receiptPath = await validateReceiptPath(resolve(args.get("--receipt")), artifactDir, runRoot);
  if (!process.env.PATH) fail("PATH must be set");
  for (const [name, expected] of Object.entries({
    LANG: "C.UTF-8",
    LC_ALL: "C.UTF-8",
    TZ: "UTC",
    CARGO_INCREMENTAL: "0",
    CARGO_NET_OFFLINE: "true",
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_CONFIG_GLOBAL: "/dev/null",
    GIT_ATTR_NOSYSTEM: "1",
  })) {
    if (process.env[name] !== expected) fail(`${name} must equal ${expected}`);
  }
  for (const name of ["RUSTUP_HOME", "PNPM_HOME"]) {
    if (!process.env[name]) fail(`${name} must be set`);
  }
  if (process.env.CARGO_TARGET_DIR !== undefined) {
    fail("CARGO_TARGET_DIR must be absent so clone-local target directories are used");
  }
  const [environmentHome, environmentCodexHome, environmentTmpdir, environmentCargoHome] =
    await Promise.all([
      resolveRunDirectory("HOME", runRoot),
      resolveRunDirectory("CODEX_HOME", runRoot),
      resolveRunDirectory("TMPDIR", runRoot),
      resolveRunDirectory("CARGO_HOME", runRoot),
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

  const [instructions, controls, bashPath, gitPath, rtkPath, rustupPath, filesystem] = await Promise.all([
    scanWorkspaceInstructions(workspace, expectedAgentsSha256),
    controlObservations(artifactDir),
    resolveExecutable("bash"),
    resolveExecutable("git"),
    resolveExecutable("rtk"),
    resolveExecutable("rustup"),
    statfs(workspace),
  ]);
  if (expectedControlSha256 !== controls.sha256) {
    fail(`control SHA-256 mismatch: expected ${expectedControlSha256}, got ${controls.sha256}`);
  }
  const [cargoPath, rustcPath, rustfmtPath, clippyPath] = await Promise.all([
    rustupWhich(rustupPath, "cargo"),
    rustupWhich(rustupPath, "rustc"),
    rustupWhich(rustupPath, "rustfmt"),
    rustupWhich(rustupPath, "cargo-clippy"),
  ]);
  const [node, bash, git, rtk, rustup, cargo, rustc, rustfmt, clippy] = await Promise.all([
    observeExecutable("node", process.execPath, ["--version"]),
    observeExecutable("bash", bashPath, ["--version"]),
    observeExecutable("git", gitPath, ["--version"]),
    observeExecutable("rtk", rtkPath, ["--version"]),
    observeExecutable("rustup", rustupPath, ["--version"]),
    observeExecutable("cargo", cargoPath, ["--version"]),
    observeExecutable("rustc", rustcPath, ["--version"]),
    observeExecutable("rustfmt", rustfmtPath, ["--version"]),
    observeExecutable("clippy", clippyPath, ["--version"]),
  ]);
  const rustTargetPath = await realpath(
    command(rustc.path, ["--print", "target-libdir", "--target", "wasm32-unknown-unknown"]),
  );
  const rustTarget = { target: "wasm32-unknown-unknown", ...(await hashTree(rustTargetPath)) };
  if (rustTarget.regular_files === 0 || rustTarget.file_bytes === 0) {
    fail("wasm32-unknown-unknown target has no regular artifacts");
  }
  const receipt = {
    protocol_id: controls.protocol_id,
    valid: true,
    paths: { run_root: runRoot, workspace, home, codex_home: codexHome, artifact_dir: artifactDir },
    environment: {
      HOME: process.env.HOME,
      CODEX_HOME: process.env.CODEX_HOME,
      TMPDIR: process.env.TMPDIR,
      PATH: process.env.PATH ?? "",
      LANG: process.env.LANG ?? null,
      LC_ALL: process.env.LC_ALL ?? null,
      TZ: process.env.TZ ?? null,
      CARGO_INCREMENTAL: process.env.CARGO_INCREMENTAL,
      CARGO_NET_OFFLINE: process.env.CARGO_NET_OFFLINE,
      CARGO_HOME: process.env.CARGO_HOME,
      RUSTUP_HOME: process.env.RUSTUP_HOME,
      PNPM_HOME: process.env.PNPM_HOME,
      GIT_CONFIG_NOSYSTEM: process.env.GIT_CONFIG_NOSYSTEM,
      GIT_CONFIG_GLOBAL: process.env.GIT_CONFIG_GLOBAL,
      GIT_ATTR_NOSYSTEM: process.env.GIT_ATTR_NOSYSTEM,
      expected_agents_sha256: expectedAgentsSha256,
      expected_control_sha256: expectedControlSha256,
    },
    filesystem: {
      workspace: workspaceIdentity,
      home: homeIdentity,
      codex_home: codexHomeIdentity,
      tmpdir: await filesystemIdentity(environmentTmpdir),
      cargo_home: await filesystemIdentity(environmentCargoHome),
    },
    scans: {
      workspace_instructions: instructions,
      home: { entries: homeEntries },
      codex_home: { entries: codexHomeEntries },
    },
    binaries: { node, bash, git, rtk, rustup, cargo, rustc, rustfmt, clippy },
    rust_target: rustTarget,
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
