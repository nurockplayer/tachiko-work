#!/usr/bin/env node

import {createHash} from "node:crypto";
import {lstat, readFile, realpath, writeFile} from "node:fs/promises";
import {spawnSync} from "node:child_process";
import {isAbsolute, relative, resolve} from "node:path";
import {fileURLToPath} from "node:url";

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function inside(candidate, parent) {
  const path = relative(parent, candidate);
  return path === "" || (!path.startsWith("..") && !isAbsolute(path));
}

function exactKeys(value, expected, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  if (JSON.stringify(Object.keys(value).sort()) !== JSON.stringify([...expected].sort())) {
    throw new Error(`${label} keys mismatch`);
  }
}

const values = new Map();
for (let index = 2; index < process.argv.length; index += 2) {
  const key = process.argv[index];
  const value = process.argv[index + 1];
  if (!key?.startsWith("--") || value === undefined || values.has(key)) {
    throw new Error("invalid candidate adapter arguments");
  }
  values.set(key, value);
}
for (const key of ["--candidate-root", "--contract", "--output", "--config"]) {
  if (!values.has(key)) throw new Error(`missing ${key}`);
}

const candidateRoot = await realpath(resolve(values.get("--candidate-root")));
const configInput = resolve(values.get("--config"));
const configMetadata = await lstat(configInput);
if (configMetadata.isSymbolicLink() || !configMetadata.isFile()) {
  throw new Error("adapter config must be a non-symlink regular file");
}
const configPath = await realpath(configInput);
if (inside(configPath, candidateRoot)) throw new Error("adapter config is candidate-controlled");
const configBytes = await readFile(configPath);
if (!configBytes.toString("utf8").endsWith("\n")) throw new Error("adapter config needs final newline");
const config = JSON.parse(configBytes.toString("utf8"));
exactKeys(config, ["schema", "case_id", "probe"], "config");
exactKeys(config.probe, ["executable", "sha256", "arguments"], "probe");
if (
  config.schema !== "tachiko-candidate-adapter-v1" ||
  !["TW-05", "TW-09"].includes(config.case_id) ||
  !isAbsolute(config.probe.executable) ||
  !/^[0-9a-f]{64}$/.test(config.probe.sha256) ||
  !Array.isArray(config.probe.arguments) ||
  config.probe.arguments.length === 0 ||
  !config.probe.arguments.every((entry) => typeof entry === "string")
) {
  throw new Error("adapter config contract mismatch");
}
const probeMetadata = await lstat(config.probe.executable);
if (probeMetadata.isSymbolicLink() || !probeMetadata.isFile()) {
  throw new Error("adapter probe must be a non-symlink regular file");
}
const probePath = await realpath(config.probe.executable);
if (inside(probePath, candidateRoot)) throw new Error("adapter probe is candidate-controlled");
const probeBytes = await readFile(probePath);
if (sha256(probeBytes) !== config.probe.sha256) throw new Error("adapter probe SHA-256 mismatch");

const replacements = {
  "<candidate-root>": candidateRoot,
  "<contract>": resolve(values.get("--contract")),
};
const probeArguments = config.probe.arguments.map((argument) => {
  let value = argument;
  for (const [token, replacement] of Object.entries(replacements)) {
    value = value.replaceAll(token, replacement);
  }
  if (/<[^>]+>/.test(value)) throw new Error(`unresolved adapter probe token: ${value}`);
  return value;
});
const profile = "(version 1)\n(allow default)\n(deny network*)\n";
const result = spawnSync(
  "/usr/bin/sandbox-exec",
  ["-p", profile, probePath, ...probeArguments],
  {
    cwd: candidateRoot,
    encoding: "utf8",
    maxBuffer: 128 * 1024 * 1024,
    timeout: 1_800_000,
    env: {...process.env, CARGO_NET_OFFLINE: "true"},
  },
);
if (result.status !== 0) throw new Error(`candidate-specific probe failed: ${result.stderr}`);
const payload = JSON.parse(result.stdout.trim().split(/\r?\n/).at(-1));
const contractBytes = await readFile(resolve(values.get("--contract")));
const adapterBytes = await readFile(fileURLToPath(import.meta.url));
const envelope = config.case_id === "TW-05"
  ? {
      contract_sha256: sha256(contractBytes),
      adapter: {sha256: sha256(adapterBytes), behavior_implemented_by_adapter: false},
      native: payload.native,
      wasm: payload.wasm,
    }
  : {
      contract_sha256: sha256(contractBytes),
      adapter: {sha256: sha256(adapterBytes), behavior_implemented_by_adapter: false},
      observations: payload.observations,
    };
await writeFile(resolve(values.get("--output")), `${JSON.stringify(envelope)}\n`, {mode: 0o600});
console.log(JSON.stringify({
  scaffold: "tachiko-candidate-adapter-v1",
  case_id: config.case_id,
  config_sha256: sha256(configBytes),
  probe_sha256: sha256(probeBytes),
  probe_stdout_sha256: sha256(result.stdout),
  probe_stderr_sha256: sha256(result.stderr),
}));
