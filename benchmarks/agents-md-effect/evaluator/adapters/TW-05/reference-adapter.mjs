#!/usr/bin/env node

import {createHash} from "node:crypto";
import {readFile, realpath, writeFile} from "node:fs/promises";
import {spawnSync} from "node:child_process";
import {resolve} from "node:path";
import {Worker} from "node:worker_threads";
import {fileURLToPath} from "node:url";

function parseArgs(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined || values.has(key)) {
      throw new Error("invalid TW-05 reference adapter arguments");
    }
    values.set(key, value);
  }
  for (const key of ["--candidate-root", "--contract", "--output"]) {
    if (!values.has(key)) throw new Error(`missing ${key}`);
  }
  return values;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function runWorker(workerPath, wasmPath) {
  return new Promise((resolveResult, reject) => {
    const worker = new Worker(workerPath, {workerData: {wasmPath}});
    worker.once("message", resolveResult);
    worker.once("error", reject);
    worker.once("exit", (code) => {
      if (code !== 0) reject(new Error(`reference Worker exited with ${code}`));
    });
  });
}

const args = parseArgs(process.argv.slice(2));
const candidateRoot = await realpath(resolve(args.get("--candidate-root")));
const contractBytes = await readFile(resolve(args.get("--contract")));
const adapterBytes = await readFile(fileURLToPath(import.meta.url));
const manifest = resolve(candidateRoot, "Cargo.toml");
const cargo = spawnSync("/usr/bin/which", ["cargo"], {encoding: "utf8"}).stdout.trim();
const environment = {...process.env, CARGO_NET_OFFLINE: "true"};
for (const buildArgs of [
  ["build", "--manifest-path", manifest, "--release", "--locked"],
  [
    "build",
    "--manifest-path",
    manifest,
    "--target",
    "wasm32-unknown-unknown",
    "--release",
    "--locked",
  ],
]) {
  const result = spawnSync(cargo, buildArgs, {
    cwd: candidateRoot,
    encoding: "utf8",
    env: environment,
    maxBuffer: 128 * 1024 * 1024,
    timeout: 1_800_000,
  });
  if (result.status !== 0) throw new Error(result.stderr || "reference build failed");
}
const native = spawnSync(
  resolve(candidateRoot, "target/release/tachiko-tw05-reference-native"),
  [],
  {cwd: candidateRoot, encoding: "utf8", env: environment},
);
if (native.status !== 0) throw new Error(native.stderr || "reference native execution failed");
const nativeObservations = JSON.parse(native.stdout.trim());
const wasmObservations = await runWorker(
  resolve(candidateRoot, "worker.mjs"),
  resolve(
    candidateRoot,
    "target/wasm32-unknown-unknown/release/tachiko_tw05_reference_runtime.wasm",
  ),
);
const envelope = {
  contract_sha256: sha256(contractBytes),
  adapter: {sha256: sha256(adapterBytes), behavior_implemented_by_adapter: false},
  native: {execution: "native_process", observations: nativeObservations},
  wasm: {
    execution: "real_wasm32",
    worker_boundary: "typescript_worker",
    observations: wasmObservations,
  },
};
await writeFile(resolve(args.get("--output")), `${JSON.stringify(envelope)}\n`, {mode: 0o600});
console.log(JSON.stringify({adapter: "TW-05-controlled-reference-v1"}));
