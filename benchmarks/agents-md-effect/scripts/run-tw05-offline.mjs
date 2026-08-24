#!/usr/bin/env node

import {createHash} from "node:crypto";
import {readFile, realpath, writeFile} from "node:fs/promises";
import {spawnSync} from "node:child_process";
import {isAbsolute, resolve} from "node:path";

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
if (!isAbsolute(args.get("candidate-root")) || !isAbsolute(args.get("output"))) {
  throw new Error("candidate-root and output must be absolute");
}
const candidateRoot = await realpath(resolve(args.get("candidate-root")));
const output = resolve(args.get("output"));
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
}

const executableCache = new Map();
const executions = [];
for (const command of commands) {
  let executable = executableCache.get(command.name);
  if (!executable) {
    executable = resolveExecutable(command.name, environment);
    executableCache.set(command.name, executable);
  }
  const result = spawnSync(executable, command.args, {
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
    stdout_sha256: sha256(result.stdout ?? ""),
    stderr_sha256: sha256(result.stderr ?? ""),
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
  executables,
  executions,
  pass: executions.every((entry) => entry.exit_code === 0 && entry.spawn_error === null),
};
await writeFile(output, `${JSON.stringify(receipt, null, 2)}\n`, {mode: 0o600});
console.log(JSON.stringify(receipt));
if (!receipt.pass) process.exitCode = 1;
