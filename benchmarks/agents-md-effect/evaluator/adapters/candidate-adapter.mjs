#!/usr/bin/env node

import {createHash} from "node:crypto";
import {lstat, readFile, realpath} from "node:fs/promises";
import {spawnSync} from "node:child_process";
import {isAbsolute, relative, resolve} from "node:path";
import {fileURLToPath} from "node:url";

const DENY_NETWORK_PROFILE = "(version 1)\n(allow default)\n(deny network*)\n";

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

function denyReadProfile(
  roots,
  allowReadPaths,
  allowReadRoots,
  denyWriteRoots,
  denyWritePaths,
  allowWriteRoots,
  allowWritePaths,
) {
  const quote = (value) => resolve(value).replaceAll("\\", "\\\\").replaceAll('"', '\\"');
  const denied = [...new Set(roots.map(quote))].sort();
  const allowed = [...new Set(allowReadPaths.map(quote))].sort();
  const readable = [...new Set(allowReadRoots.map(quote))].sort();
  const writeDenied = [...new Set([...roots, ...denyWriteRoots].map(quote))].sort();
  const writeProtected = [...new Set(denyWritePaths.map(quote))].sort();
  const writableRoots = [...new Set(allowWriteRoots.map(quote))].sort();
  const writable = [...new Set(allowWritePaths.map(quote))].sort();
  return `${DENY_NETWORK_PROFILE}${denied.map((root) =>
    `(deny file-read* (subpath "${root}"))\n`).join("")}${writeDenied.map((root) =>
    `(deny file-write* (subpath "${root}"))\n`).join("")}${writeProtected.map((path) =>
    `(deny file-write* (literal "${path}"))\n`).join("")}${[...new Set([...denied, ...writeDenied])].map((root) =>
    `(allow file-read-metadata (subpath "${root}"))\n`).join("")}${readable.map((root) =>
    `(allow file-read* (subpath "${root}"))\n`).join("")}${allowed.map((path) =>
    `(allow file-read* (literal "${path}"))\n`).join("")}${writableRoots.map((path) =>
    `(allow file-write* (subpath "${path}"))\n`).join("")}${writable.map((path) =>
    `(allow file-write* (literal "${path}"))\n`).join("")}`;
}

const values = new Map();
const denyReadArguments = [];
const denyWriteArguments = [];
const denyWriteRootArguments = [];
const allowWriteRootArguments = [];
for (let index = 2; index < process.argv.length; index += 2) {
  const key = process.argv[index];
  const value = process.argv[index + 1];
  if (!key?.startsWith("--") || value === undefined ||
      (values.has(key) && ![
        "--deny-read-root", "--deny-write-path", "--deny-write-root", "--allow-write-root",
      ].includes(key))) {
    throw new Error("invalid candidate adapter arguments");
  }
  if (key === "--deny-read-root") denyReadArguments.push(value);
  else if (key === "--deny-write-path") denyWriteArguments.push(value);
  else if (key === "--deny-write-root") denyWriteRootArguments.push(value);
  else if (key === "--allow-write-root") allowWriteRootArguments.push(value);
  else values.set(key, value);
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
const denyReadRoots = [];
for (const input of denyReadArguments) {
  if (!isAbsolute(input)) throw new Error("adapter deny-read roots must be absolute");
  const root = resolve(input);
  if (inside(root, candidateRoot)) {
    throw new Error("adapter deny-read roots must not be inside the candidate root");
  }
  denyReadRoots.push(root);
}
const adapterPath = fileURLToPath(import.meta.url);
const denyWritePaths = [adapterPath, configPath, probePath];
for (const input of denyWriteArguments) {
  if (!isAbsolute(input)) throw new Error("adapter deny-write paths must be absolute");
  denyWritePaths.push(resolve(input));
}
const denyWriteRoots = [];
for (const input of denyWriteRootArguments) {
  if (!isAbsolute(input)) throw new Error("adapter deny-write roots must be absolute");
  denyWriteRoots.push(resolve(input));
}
const allowWriteRoots = [];
for (const input of allowWriteRootArguments) {
  if (!isAbsolute(input)) throw new Error("adapter allow-write roots must be absolute");
  const root = resolve(input);
  if (!denyWriteRoots.some((denied) => inside(root, denied))) {
    throw new Error("adapter allow-write roots must be nested in a denied write root");
  }
  allowWriteRoots.push(root);
}
const profile = denyReadProfile(
  denyReadRoots,
  [adapterPath],
  [candidateRoot],
  [candidateRoot, ...denyWriteRoots],
  denyWritePaths,
  allowWriteRoots,
  [],
);
if (denyReadRoots.length > 0) {
  if (!/^[0-9a-f]{64}$/.test(values.get("--expected-sandbox-profile-sha256") ?? "") ||
      values.get("--expected-sandbox-profile-sha256") !== sha256(profile)) {
    throw new Error("outer adapter sandbox profile SHA-256 mismatch");
  }
  if (!/^[0-9a-f]{64}$/.test(values.get("--contract-sha256") ?? "")) {
    throw new Error("trusted contract SHA-256 is required under the outer sandbox");
  }
}

const replacements = {
  "<candidate-root>": candidateRoot,
};
const probeArguments = config.probe.arguments.map((argument) => {
  if (/<(?:contract|output|trusted-[^>]+)>/i.test(argument)) {
    throw new Error("trusted contract and expected-value probe tokens are forbidden");
  }
  let value = argument;
  for (const [token, replacement] of Object.entries(replacements)) {
    value = value.replaceAll(token, replacement);
  }
  if (/<[^>]+>/.test(value)) throw new Error(`unresolved adapter probe token: ${value}`);
  if (isAbsolute(value) &&
      denyReadRoots.some((root) => inside(resolve(value), root)) &&
      !inside(resolve(value), candidateRoot)) {
    throw new Error("adapter probe argument exposes a denied expected-value root");
  }
  return value;
});
const result = spawnSync(
  probePath,
  probeArguments,
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
const contractSha256 = values.has("--contract-sha256")
  ? values.get("--contract-sha256")
  : sha256(await readFile(resolve(values.get("--contract"))));
const adapterBytes = await readFile(adapterPath);
const envelope = config.case_id === "TW-05"
  ? {
      contract_sha256: contractSha256,
      adapter: {sha256: sha256(adapterBytes), behavior_implemented_by_adapter: false},
      native: payload.native,
      wasm: payload.wasm,
    }
  : {
      contract_sha256: contractSha256,
      adapter: {sha256: sha256(adapterBytes), behavior_implemented_by_adapter: false},
      observations: payload.observations,
    };
console.log(JSON.stringify({
  scaffold: "tachiko-candidate-adapter-v1",
  case_id: config.case_id,
  config_sha256: sha256(configBytes),
  probe_sha256: sha256(probeBytes),
  probe_stdout_sha256: sha256(result.stdout),
  probe_stderr_sha256: sha256(result.stderr),
  sandbox_profile_sha256: sha256(profile),
  denied_read_roots_sha256: sha256(`${JSON.stringify(denyReadRoots)}\n`),
  envelope,
}));
