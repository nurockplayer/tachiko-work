#!/usr/bin/env node

import {createHash} from "node:crypto";
import {constants, existsSync} from "node:fs";
import {lstat, mkdir, open, readFile, realpath, unlink} from "node:fs/promises";
import {spawnSync} from "node:child_process";
import {basename, dirname, isAbsolute, relative, resolve} from "node:path";

const sandboxExecutable = "/usr/bin/sandbox-exec";
const sandboxProfile = "(version 1)\n(allow default)\n(deny network*)\n";

function usage() {
  console.error(
    "usage: node run-tw05-offline.mjs --candidate-root /abs/repo --output /abs/receipt.json " +
      "[--cargo-command 'cargo test --locked'] [--node-test-file path] " +
      "[--node-benchmark-file path]",
  );
  process.exit(2);
}

function parseArgs(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined || values.has(key.slice(2))) usage();
    values.set(key.slice(2), value);
  }
  return values;
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function inside(candidate, parent) {
  const path = relative(parent, candidate);
  return path === "" || (!path.startsWith("..") && !isAbsolute(path));
}

async function reserveTrustedOutput(outputInput, candidateRoot) {
  if (!isAbsolute(outputInput)) throw new Error("output must be absolute");
  const requested = resolve(outputInput);
  try {
    await lstat(requested);
    throw new Error("output must not already exist (including symlink or special files)");
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  const parentComponents = [];
  for (let component = dirname(requested); dirname(component) !== component; component = dirname(component)) {
    parentComponents.unshift(component);
  }
  for (const component of parentComponents) {
    let metadata;
    try {
      metadata = await lstat(component);
    } catch (error) {
      if (error.code === "ENOENT") break;
      throw error;
    }
    if (metadata.isSymbolicLink()) throw new Error("output parent must not contain a symlink");
    if (!metadata.isDirectory()) throw new Error("output ancestor must be a directory");
  }
  let cursor = dirname(requested);
  const suffix = [];
  while (!existsSync(cursor)) {
    const parent = dirname(cursor);
    if (parent === cursor) throw new Error("output has no existing ancestor");
    suffix.unshift(basename(cursor));
    cursor = parent;
  }
  const ancestorMetadata = await lstat(cursor);
  if (ancestorMetadata.isSymbolicLink()) throw new Error("output parent must not be a symlink");
  if (!ancestorMetadata.isDirectory()) throw new Error("output ancestor must be a directory");
  let canonicalParent = await realpath(cursor);
  for (const component of suffix) {
    canonicalParent = resolve(canonicalParent, component);
    await mkdir(canonicalParent, {mode: 0o700});
    const metadata = await lstat(canonicalParent);
    if (metadata.isSymbolicLink() || !metadata.isDirectory()) {
      throw new Error("output parent creation was redirected");
    }
  }
  const output = resolve(canonicalParent, basename(requested));
  if (inside(output, candidateRoot) || inside(candidateRoot, output)) {
    throw new Error("output and candidate-root must be disjoint");
  }
  if (existsSync(output)) throw new Error("output must not already exist");
  const handle = await open(
    output,
    constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | (constants.O_NOFOLLOW ?? 0),
    0o600,
  );
  const metadata = await lstat(output);
  if (metadata.isSymbolicLink() || !metadata.isFile()) {
    await handle.close();
    throw new Error("reserved output is not a regular file");
  }
  return {handle, output};
}

function splitTrustedCommand(command) {
  const parts = command.trim().split(/\s+/).filter(Boolean);
  if (parts[0] !== "cargo" || parts.length < 2) {
    throw new Error("cargo-command must be a direct cargo invocation");
  }
  if (parts.some((part) => /[;&|`$<>]/.test(part))) {
    throw new Error("cargo-command contains shell syntax");
  }
  return parts;
}

function resolveExecutable(name, environment) {
  const found = spawnSync("/usr/bin/which", [name], {
    encoding: "utf8",
    env: environment,
  });
  if (found.status !== 0 || !found.stdout.trim()) {
    throw new Error(`required executable is unavailable: ${name}`);
  }
  return found.stdout.trim();
}

const args = parseArgs(process.argv.slice(2));
for (const key of ["candidate-root", "output"]) {
  if (!args.has(key)) usage();
}
if (!isAbsolute(args.get("candidate-root"))) throw new Error("candidate-root must be absolute");
const candidateRoot = await realpath(resolve(args.get("candidate-root")));
const {handle: outputHandle, output} = await reserveTrustedOutput(args.get("output"), candidateRoot);
let outputClosed = false;
let receiptWritten = false;
try {
const sandboxBytes = await readFile(sandboxExecutable).catch(() => {
  throw new Error("/usr/bin/sandbox-exec is required for kernel network denial");
});
const environment = {
  ...process.env,
  CARGO_NET_OFFLINE: "true",
  HTTP_PROXY: "http://127.0.0.1:9",
  HTTPS_PROXY: "http://127.0.0.1:9",
  ALL_PROXY: "http://127.0.0.1:9",
  NO_PROXY: "",
  http_proxy: "http://127.0.0.1:9",
  https_proxy: "http://127.0.0.1:9",
  all_proxy: "http://127.0.0.1:9",
  no_proxy: "",
};

const commands = [];
if (args.has("cargo-command")) {
  const [executable, ...commandArgs] = splitTrustedCommand(args.get("cargo-command"));
  commands.push({name: executable, args: commandArgs, purpose: "rust_tests"});
} else {
  const manifest = "spikes/issue-26-runtime/Cargo.toml";
  commands.push(
    {
      name: "cargo",
      args: ["build", "--manifest-path", manifest, "--bin", "native-driver", "--release", "--locked"],
      purpose: "native_build",
    },
    {
      name: "cargo",
      args: ["build", "--manifest-path", manifest, "--target", "wasm32-unknown-unknown", "--release", "--locked"],
      purpose: "wasm_build",
    },
    {
      name: "cargo",
      args: ["test", "--manifest-path", manifest, "--all-targets", "--locked"],
      purpose: "rust_tests",
    },
  );
}
if (args.has("node-test-file")) {
  commands.push({name: "node", args: ["--test", args.get("node-test-file")], purpose: "worker_tests"});
} else if (!args.has("cargo-command")) {
  commands.push({
    name: "node",
    args: [
      "--test",
      "spikes/issue-26-runtime/test/worker-runtime.test.ts",
      "spikes/issue-26-runtime/test/native-wasm-parity.test.ts",
    ],
    purpose: "worker_tests",
  });
}
if (args.has("node-benchmark-file")) {
  commands.push({name: "node", args: [args.get("node-benchmark-file")], purpose: "benchmark"});
} else if (!args.has("cargo-command")) {
  commands.push({
    name: "node",
    args: ["spikes/issue-26-runtime/benchmark/runtime-benchmark.ts"],
    purpose: "benchmark",
  });
  commands.push({
    name: "bash",
    args: ["scripts/issue-26-portability-audit.sh"],
    purpose: "portability_audit",
  });
}

const networkProbePath = resolve(
  new URL("probe-network-denial.mjs", import.meta.url).pathname,
);
const networkProbe = spawnSync(
  sandboxExecutable,
  ["-p", sandboxProfile, process.execPath, networkProbePath],
  {cwd: candidateRoot, encoding: "utf8", env: environment, timeout: 10_000},
);
if (networkProbe.status !== 0 || !/^network-denied:(?:EPERM|EACCES)\s*$/.test(networkProbe.stdout)) {
  throw new Error(
    `kernel network denial probe failed: ${networkProbe.stderr || networkProbe.stdout}`,
  );
}

const executableCache = new Map();
const executions = [];
for (const command of commands) {
  let executable = executableCache.get(command.name);
  if (!executable) {
    executable = resolveExecutable(command.name, environment);
    executableCache.set(command.name, executable);
  }
  const result = spawnSync(sandboxExecutable, ["-p", sandboxProfile, executable, ...command.args], {
    cwd: candidateRoot,
    encoding: "utf8",
    env: environment,
    maxBuffer: 128 * 1024 * 1024,
    timeout: Number(args.get("timeout-ms") ?? 1_800_000),
  });
  executions.push({
    purpose: command.purpose,
    name: command.name,
    args: command.args,
    exit_code: result.status,
    signal: result.signal,
    spawn_error: result.error?.message ?? null,
    stdout: {
      bytes: Buffer.byteLength(result.stdout ?? ""),
      sha256: sha256(result.stdout ?? ""),
    },
    stderr: {
      bytes: Buffer.byteLength(result.stderr ?? ""),
      sha256: sha256(result.stderr ?? ""),
    },
  });
}

const executables = [];
for (const command of commands) {
  const path = executableCache.get(command.name);
  executables.push({
    name: command.name,
    path,
    sha256: sha256(await readFile(path)),
  });
}
const receipt = {
  schema: "tachiko-tw05-offline-execution-v1",
  classification: "construction_pilot_only",
  formal_result_eligible: false,
  candidate_root: candidateRoot,
  offline: true,
  network_environment: {
    CARGO_NET_OFFLINE: environment.CARGO_NET_OFFLINE,
    HTTP_PROXY: environment.HTTP_PROXY,
    HTTPS_PROXY: environment.HTTPS_PROXY,
    ALL_PROXY: environment.ALL_PROXY,
    NO_PROXY: environment.NO_PROXY,
  },
  package_manager_dependency: false,
  network_enforcement: {
    mode: "darwin_sandbox_deny_network",
    sandbox_executable: sandboxExecutable,
    sandbox_executable_sha256: sha256(sandboxBytes),
    profile_sha256: sha256(sandboxProfile),
    profile: sandboxProfile,
    probe_executable: process.execPath,
    probe_executable_sha256: sha256(await readFile(process.execPath)),
    probe_script_sha256: sha256(await readFile(networkProbePath)),
    probe_denied: true,
    probe_stdout_sha256: sha256(networkProbe.stdout),
    probe_stderr_sha256: sha256(networkProbe.stderr),
  },
  executables,
  executions,
  pass: executions.every((entry) => entry.exit_code === 0 && entry.spawn_error === null),
};
await outputHandle.writeFile(`${JSON.stringify(receipt, null, 2)}\n`);
await outputHandle.sync();
receiptWritten = true;
await outputHandle.close();
outputClosed = true;
console.log(JSON.stringify(receipt));
if (!receipt.pass) process.exitCode = 1;
} catch (error) {
  throw error;
} finally {
  if (!outputClosed) await outputHandle.close().catch(() => {});
  if (!receiptWritten) await unlink(output).catch(() => {});
}
