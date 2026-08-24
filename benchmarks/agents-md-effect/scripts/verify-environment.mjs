#!/usr/bin/env node

import { arch } from "node:os";
import { createHash } from "node:crypto";
import { access, lstat, readFile, readdir, realpath, statfs } from "node:fs/promises";
import { constants } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const benchmarkDir = resolve(scriptDir, "..");
const repositoryDir = resolve(benchmarkDir, "../..");
const lock = JSON.parse(
  await readFile(resolve(benchmarkDir, "environment-lock.json"), "utf8"),
);

function fail(message) {
  throw new Error(message);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function command(executable, args = []) {
  const result = spawnSync(executable, args, {
    cwd: repositoryDir,
    encoding: "utf8",
    env: {...process.env, PATH: lock.controlled_runner.path},
  });
  if (result.status !== 0) {
    fail(`${executable} ${args.join(" ")} failed: ${result.stderr || result.stdout}`);
  }
  return result.stdout.trim();
}

async function verifyFileHash(filePath, expected, label) {
  const actual = sha256(await readFile(filePath));
  if (actual !== expected) fail(`${label} SHA-256 mismatch: ${actual}`);
}

for (const [name, value] of Object.entries({
  LANG: "C.UTF-8",
  LC_ALL: "C.UTF-8",
  TZ: "UTC",
  CARGO_INCREMENTAL: "0",
  CARGO_NET_OFFLINE: "true",
  RUSTUP_HOME: lock.controlled_runner.environment.RUSTUP_HOME,
  PNPM_HOME: lock.controlled_runner.environment.PNPM_HOME,
})) {
  if (process.env[name] !== value) {
    fail(`${name} must equal ${value}; found ${process.env[name] ?? "<unset>"}`);
  }
}
if (process.env.PATH !== lock.controlled_runner.path) {
  fail("PATH differs from the neutral controlled-runner path");
}
if (process.env.CARGO_TARGET_DIR !== undefined) {
  fail("CARGO_TARGET_DIR must be unset so repository smoke scripts use clone-local target/");
}
if (!process.env.CARGO_HOME) fail("CARGO_HOME must name a per-run sealed-cache clone");
if (!process.env.HOME) fail("HOME must name the per-run sealed empty home");
const cargoHome = await realpath(resolve(process.env.CARGO_HOME));
const runHome = await realpath(resolve(process.env.HOME));
if (runHome !== resolve(dirname(cargoHome), "home")) {
  fail("HOME must be the home sibling inside the same opaque run root as CARGO_HOME");
}
const homeStat = await lstat(runHome);
if (!homeStat.isDirectory() || (homeStat.mode & 0o222) !== 0) {
  fail("HOME must be a read-only directory");
}
if (homeStat.uid === Number(command("/usr/bin/id", ["-u"]))) {
  fail("HOME must be supervisor-owned, not agent-owned");
}
if ((await readdir(runHome)).length !== 0) fail("HOME must be empty before launch");
const sealedCargoTemplate = await realpath(
  lock.offline_dependency_cache.template_path,
);
if (cargoHome === sealedCargoTemplate) {
  fail("CARGO_HOME must be a copy-on-write clone, not the sealed template itself");
}
await access(cargoHome, constants.R_OK | constants.W_OK);
const cargoTree = JSON.parse(
  command(process.execPath, [resolve(scriptDir, "hash-tree.mjs"), cargoHome, "--content-only"]),
);
if (
  cargoTree.digest_kind !== lock.offline_dependency_cache.digest_kind ||
  cargoTree.manifest_sha256 !== lock.offline_dependency_cache.tree_sha256
) {
  fail("per-run CARGO_HOME content differs from the sealed dependency template");
}
if (arch() !== "arm64") fail(`expected arm64, found ${arch()}`);
if (command("/usr/bin/id", ["-un"]) !== lock.controlled_runner.required_os_account) {
  fail(`controlled run must execute as ${lock.controlled_runner.required_os_account}`);
}

const productVersion = command("/usr/bin/sw_vers", ["-productVersion"]);
const buildVersion = command("/usr/bin/sw_vers", ["-buildVersion"]);
const kernel = command("/usr/bin/uname", ["-r"]);
if (productVersion !== "15.7.4" || buildVersion !== "24G517" || kernel !== "24.6.0") {
  fail(`host mismatch: macOS ${productVersion} ${buildVersion}, kernel ${kernel}`);
}

if (Number(command("/usr/sbin/sysctl", ["-n", "hw.ncpu"])) !== 12) {
  fail("logical CPU allocation mismatch");
}
if (Number(command("/usr/sbin/sysctl", ["-n", "hw.memsize"])) !== 17179869184) {
  fail("memory allocation mismatch");
}

await verifyFileHash(
  lock.controlled_agent.codex_binary_path,
  lock.controlled_agent.codex_binary_sha256,
  "Codex",
);
await verifyFileHash(
  lock.controlled_agent.code_mode_host_path,
  lock.controlled_agent.code_mode_host_sha256,
  "Codex code-mode host",
);
await verifyFileHash(lock.toolchain.node.path, lock.toolchain.node.binary_sha256, "Node");
await verifyFileHash(lock.toolchain.pnpm.path, lock.toolchain.pnpm.binary_sha256, "pnpm");
await verifyFileHash(lock.toolchain.rtk.path, lock.toolchain.rtk.binary_sha256, "rtk");
await verifyFileHash(lock.toolchain.bash.path, lock.toolchain.bash.binary_sha256, "Bash");

if (command(lock.controlled_agent.codex_binary_path, ["--version"]) !== "codex-cli 0.149.0") {
  fail("Codex version mismatch");
}
if (command(lock.toolchain.node.path, ["--version"]) !== "v24.15.0") {
  fail("Node version mismatch");
}
if (command(lock.toolchain.pnpm.path, ["--version"]).split("\n").at(-1) !== "11.13.0") {
  fail("pnpm version mismatch");
}
if (command(lock.toolchain.rtk.path, ["--version"]) !== "rtk 0.34.3") {
  fail("rtk version mismatch");
}
if (!command(lock.toolchain.bash.path, ["--version"]).startsWith("GNU bash, version 5.3.15")) {
  fail("Bash version mismatch");
}
if (
  command("rustup", ["run", "1.97.1", "rustc", "--version"]) !==
  `rustc ${lock.toolchain.rust_primary.rustc}`
) {
  fail("primary Rust version mismatch");
}
if (
  command("rustup", ["run", "stable", "rustc", "--version"]) !==
  `rustc ${lock.toolchain.rust_primary.rustc}`
) {
  fail("stable Rust alias does not resolve to the locked primary toolchain");
}
if (
  command("rustup", ["run", "1.85.0", "rustc", "--version"]) !==
  `rustc ${lock.toolchain.rust_compatibility.rustc}`
) {
  fail("compatibility Rust version mismatch");
}
const installedTargets = command("rustup", ["target", "list", "--installed", "--toolchain", "1.97.1"])
  .split("\n")
  .filter(Boolean);
for (const target of lock.toolchain.rust_primary.targets) {
  if (!installedTargets.includes(target)) fail(`missing Rust target ${target}`);
}

const filesystem = await statfs(repositoryDir);
const freeBytes = Number(filesystem.bavail * filesystem.bsize);
if (freeBytes < lock.controlled_runner.minimum_free_bytes_before_each_run) {
  fail(
    `insufficient free space: ${freeBytes} bytes; need ` +
      `${lock.controlled_runner.minimum_free_bytes_before_each_run}`,
  );
}

console.log(
  JSON.stringify({
    protocol_id: lock.protocol_id,
    host_or_image_id: lock.controlled_runner.host_or_image_id,
    free_bytes: freeBytes,
    toolchain_verified: true,
    cargo_home_content_sha256: cargoTree.manifest_sha256,
    runner_account: lock.controlled_runner.required_os_account,
    environment_verified: true,
  }),
);
