#!/usr/bin/env node

import {createHash} from "node:crypto";
import {mkdir, readFile, realpath, writeFile} from "node:fs/promises";
import {spawnSync} from "node:child_process";
import {dirname, resolve} from "node:path";
import {fileURLToPath} from "node:url";

function parseArgs(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined || values.has(key)) {
      throw new Error("invalid TW-09 adapter arguments");
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

const args = parseArgs(process.argv.slice(2));
const candidateRoot = await realpath(resolve(args.get("--candidate-root")));
const contractBytes = await readFile(resolve(args.get("--contract")));
const adapterPath = fileURLToPath(import.meta.url);
const adapterBytes = await readFile(adapterPath);
const probePath = resolve(dirname(adapterPath), "historical-target-probe.rs");
const probeBytes = await readFile(probePath);
const output = resolve(args.get("--output"));
const buildRoot = resolve(dirname(output), "tw09-probe-build");
await mkdir(buildRoot, {mode: 0o700});
const manifest = `[workspace]\n\n[package]\nname = "tachiko-tw09-probe"\nversion = "0.0.0"\nedition = "2024"\n\n[[bin]]\nname = "tachiko-tw09-probe"\npath = ${JSON.stringify(probePath)}\n\n[dependencies]\ntachiko-semantic-core = { path = ${JSON.stringify(resolve(candidateRoot, "crates/semantic-core"))} }\nserde_json = "1.0"\n`;
const manifestPath = resolve(buildRoot, "Cargo.toml");
await writeFile(manifestPath, manifest, {mode: 0o600});

const cargoLookup = spawnSync("/usr/bin/which", ["cargo"], {encoding: "utf8"});
if (cargoLookup.status !== 0) throw new Error("cargo unavailable for TW-09 probe");
const profile = "(version 1)\n(allow default)\n(deny network*)\n";
const cargoEnvironment = {
  ...process.env,
  CARGO_NET_OFFLINE: "true",
  CARGO_TARGET_DIR: resolve(buildRoot, "target"),
};
const lockResult = spawnSync(
  "/usr/bin/sandbox-exec",
  [
    "-p",
    profile,
    cargoLookup.stdout.trim(),
    "generate-lockfile",
    "--manifest-path",
    manifestPath,
    "--offline",
  ],
  {cwd: buildRoot, encoding: "utf8", env: cargoEnvironment, timeout: 1_800_000},
);
if (lockResult.status !== 0) {
  throw new Error(`TW-09 probe lock generation failed: ${lockResult.stderr || lockResult.stdout}`);
}
const result = spawnSync(
  "/usr/bin/sandbox-exec",
  [
    "-p",
    profile,
    cargoLookup.stdout.trim(),
    "run",
    "--manifest-path",
    manifestPath,
    "--locked",
  ],
  {
    cwd: buildRoot,
    encoding: "utf8",
    maxBuffer: 128 * 1024 * 1024,
    timeout: 1_800_000,
    env: cargoEnvironment,
  },
);
if (result.status !== 0) {
  throw new Error(`TW-09 production probe failed: ${result.stderr || result.stdout}`);
}
const lines = result.stdout.split(/\r?\n/).filter(Boolean);
const rawObservations = JSON.parse(lines.at(-1));
const observations = {
  machine_fact: rawObservations.machine_fact,
  presentation_invariance: {
    stable_facts_equal: rawObservations.presentation_invariance.stable_facts_equal,
    presentation_differs: rawObservations.presentation_invariance.presentation_differs,
  },
  renamed_duplicate: {
    stable_facts_equal: rawObservations.renamed_duplicate.stable_facts_equal,
    subjects: rawObservations.renamed_duplicate.subjects,
  },
  stable_order: {machine_codes: rawObservations.stable_order.machine_codes},
};
const envelope = {
  contract_sha256: sha256(contractBytes),
  adapter: {sha256: sha256(adapterBytes), behavior_implemented_by_adapter: false},
  observations,
};
await writeFile(output, `${JSON.stringify(envelope)}\n`, {mode: 0o600});
console.log(JSON.stringify({
  adapter: "TW-09-historical-target-v1",
  probe_sha256: sha256(probeBytes),
  generated_lock_sha256: sha256(await readFile(resolve(buildRoot, "Cargo.lock"))),
  cargo_stdout_sha256: sha256(result.stdout),
  cargo_stderr_sha256: sha256(result.stderr),
}));
