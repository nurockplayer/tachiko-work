#!/usr/bin/env node

import {createHash} from "node:crypto";
import {existsSync} from "node:fs";
import {lstat, mkdir, readFile, realpath, writeFile} from "node:fs/promises";
import {spawnSync} from "node:child_process";
import {dirname, isAbsolute, relative, resolve} from "node:path";
import {fileURLToPath} from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const defaultBenchmarkDir = resolve(scriptDir, "..");

function usage() {
  console.error(
    "usage: node run-oracles.mjs --case TW-03 --candidate-root /abs/validation " +
      "--trusted-dir /abs/trusted [--manifest /abs/production-oracles.json] " +
      "[--oracle-lock /abs/oracle-lock.json] [--adapter-file /abs/adapter.mjs] " +
      "[--contract-file /abs/contract.json] [--candidate-commit <sha>]",
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

function fail(message) {
  throw new Error(message);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function inside(candidate, parent) {
  const path = relative(parent, candidate);
  return path === "" || (!path.startsWith("..") && !isAbsolute(path));
}

async function prospectiveRealpath(path) {
  let cursor = resolve(path);
  const suffix = [];
  while (!existsSync(cursor)) {
    const parent = dirname(cursor);
    if (parent === cursor) fail(`no existing ancestor for ${path}`);
    suffix.unshift(relative(parent, cursor));
    cursor = parent;
  }
  let resolved = await realpath(cursor);
  for (const component of suffix) resolved = resolve(resolved, component);
  return resolved;
}

async function trustedRegularFile(path, label, candidateRoot) {
  const absolute = resolve(path);
  const metadata = await lstat(absolute);
  if (metadata.isSymbolicLink() || !metadata.isFile()) {
    fail(`${label} must be a non-symlink regular file`);
  }
  const canonical = await realpath(absolute);
  if (inside(canonical, candidateRoot)) {
    fail(`${label} and candidate-root must be disjoint`);
  }
  const bytes = await readFile(canonical);
  return {path: canonical, bytes: bytes.length, sha256: sha256(bytes)};
}

function shellQuote(value) {
  return `'${String(value).replaceAll("'", `'\\''`)}'`;
}

function replaceToken(command, token, value) {
  return command.replaceAll(`<${token}>`, shellQuote(value));
}

function safeId(value) {
  return value.replaceAll(/[^A-Za-z0-9_.-]/g, "_");
}

function exactRustOutcomes(output, testName) {
  const escaped = testName.replaceAll(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const expression = new RegExp(`^test ${escaped} \\.\\.\\. (ok|FAILED|ignored)$`, "gm");
  return [...output.matchAll(expression)].map((match) => match[1]);
}

function parseJsonStdout(stdout) {
  const lines = stdout.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
  for (let index = lines.length - 1; index >= 0; index -= 1) {
    try {
      return JSON.parse(lines[index]);
    } catch {
      // Commands may emit build chatter before their final JSON record.
    }
  }
  return undefined;
}

function jsonPointer(value, pointer) {
  if (pointer === "") return {found: true, value};
  if (!pointer.startsWith("/")) return {found: false};
  let cursor = value;
  for (const raw of pointer.slice(1).split("/")) {
    const component = raw.replaceAll("~1", "/").replaceAll("~0", "~");
    if (cursor === null || typeof cursor !== "object" || !Object.hasOwn(cursor, component)) {
      return {found: false};
    }
    cursor = cursor[component];
  }
  return {found: true, value: cursor};
}

function portableResult(observations, selector) {
  const reasons = [];
  if (
    observations?.contract_id !== "tachiko-portable-observations-v1" ||
    !Array.isArray(observations.native) ||
    !Array.isArray(observations.wasm)
  ) {
    return {pass: false, reasons: ["portable observation structure is invalid"]};
  }
  const indexes = new Set(selector.indexes);
  const native = observations.native.filter((record) => indexes.has(record.index));
  const wasm = observations.wasm.filter((record) => indexes.has(record.index));
  const expected = selector.expected_records;
  if (JSON.stringify(native) !== JSON.stringify(expected)) {
    reasons.push("native selected records differ from the lock");
  }
  if (JSON.stringify(wasm) !== JSON.stringify(expected)) {
    reasons.push("WASM selected records differ from the lock");
  }
  if (selector.require_selected_native_wasm_equal && JSON.stringify(native) !== JSON.stringify(wasm)) {
    reasons.push("selected native/WASM records differ");
  }
  if ([...native, ...wasm].some((record) => record.class === selector.reject_class)) {
    reasons.push(`selected record uses rejected class ${selector.reject_class}`);
  }
  return {pass: reasons.length === 0, reasons, selected_native: native, selected_wasm: wasm};
}

const args = parseArgs(process.argv.slice(2));
for (const key of ["case", "candidate-root", "trusted-dir"]) {
  if (!args.has(key)) usage();
}

const caseId = args.get("case");
const candidateRoot = await realpath(resolve(args.get("candidate-root")));
const trustedDirInput = resolve(args.get("trusted-dir"));
const trustedDir = await prospectiveRealpath(trustedDirInput);
if (!isAbsolute(args.get("candidate-root")) || !isAbsolute(args.get("trusted-dir"))) {
  fail("candidate-root and trusted-dir must be absolute");
}
if (existsSync(trustedDirInput)) fail("trusted-dir must not already exist");
if (inside(trustedDir, candidateRoot) || inside(candidateRoot, trustedDir)) {
  fail("trusted-dir and candidate-root must be disjoint");
}

const benchmarkDir = args.has("benchmark-dir")
  ? await realpath(resolve(args.get("benchmark-dir")))
  : defaultBenchmarkDir;
const manifestPath = resolve(
  args.get("manifest") ?? resolve(benchmarkDir, "evaluator/production-oracles.json"),
);
const oracleLockPath = resolve(
  args.get("oracle-lock") ?? resolve(benchmarkDir, "evaluator/oracle-lock.json"),
);
const [manifestBytes, lockBytes] = await Promise.all([
  readFile(manifestPath),
  readFile(oracleLockPath),
]);
const manifest = JSON.parse(manifestBytes.toString("utf8"));
const lock = JSON.parse(lockBytes.toString("utf8"));
if (manifest.protocol_id !== lock.protocol_id) fail("manifest/oracle protocol mismatch");
const manifestCase = manifest.cases?.find((entry) => entry.id === caseId);
const lockCase = lock.cases?.find((entry) => entry.id === caseId);
if (!manifestCase || !lockCase) fail(`unknown case ${caseId}`);
const lockedAssertions = new Map((lockCase.assertions ?? []).map((entry) => [entry.id, entry]));
const mappedAssertions = new Map((manifestCase.assertions ?? []).map((entry) => [entry.id, entry]));
if (lockedAssertions.size !== (lockCase.assertions ?? []).length) fail("duplicate locked assertion ID");
if (mappedAssertions.size !== lockedAssertions.size) fail("manifest assertion coverage mismatch");
for (const [id, assertion] of lockedAssertions) {
  const mapping = mappedAssertions.get(id);
  if (!mapping || mapping.command_id !== assertion.command_id) fail(`assertion mapping mismatch: ${id}`);
}

await mkdir(trustedDirInput, {mode: 0o700});
const outputPaths = {
  portable: resolve(trustedDirInput, "portable-observations.json"),
  metadata: resolve(trustedDirInput, "metadata-observations.json"),
  observations: resolve(trustedDirInput, "observations.json"),
};
const defaultContracts = {
  "TW-05": resolve(benchmarkDir, "evaluator/contracts/TW-05-resident-parity.json"),
  "TW-09": resolve(benchmarkDir, "evaluator/contracts/TW-09-stable-diagnostic-facts.json"),
};
const adapterRequested = manifestCase.oracle_commands.some((entry) =>
  entry.command_template.includes("<trusted-adapter-file>"));
const contractRequested = manifestCase.oracle_commands.some((entry) =>
  entry.command_template.includes("<trusted-contract-file>"));
let adapterFile;
let contractFile;
const trustedInputs = [];
if (adapterRequested) {
  if (!args.has("adapter-file")) fail(`${caseId} requires --adapter-file`);
  const trusted = await trustedRegularFile(args.get("adapter-file"), "adapter-file", candidateRoot);
  trustedInputs.push({kind: "adapter", ...trusted});
  adapterFile = trusted.path;
}
if (contractRequested) {
  const requestedContract = args.get("contract-file") ?? defaultContracts[caseId];
  if (!requestedContract) fail(`${caseId} requires --contract-file`);
  const trusted = await trustedRegularFile(requestedContract, "contract-file", candidateRoot);
  trustedInputs.push({kind: "contract", ...trusted});
  contractFile = trusted.path;
}

const commands = [];
const assertions = [];
for (const [commandIndex, command] of manifestCase.oracle_commands.entries()) {
  const expectedIds = lockedAssertions.size === 0
    ? []
    : [...lockedAssertions.values()]
      .filter((entry) => entry.command_id === command.id)
      .map((entry) => entry.id)
      .sort();
  if (JSON.stringify([...(command.assertion_ids ?? [])].sort()) !== JSON.stringify(expectedIds)) {
    fail(`command assertion coverage mismatch: ${command.id}`);
  }

  let resolvedCommand = command.command_template;
  const replacements = {
    benchmark: benchmarkDir,
    controller: benchmarkDir,
    "validation-workspace": candidateRoot,
    "trusted-portable-observations-file": outputPaths.portable,
    "trusted-metadata-observations-file": outputPaths.metadata,
    "trusted-observations-file": outputPaths.observations,
  };
  for (const [token, value] of Object.entries(replacements)) {
    resolvedCommand = replaceToken(resolvedCommand, token, value);
  }
  if (resolvedCommand.includes("<trusted-adapter-file>")) {
    resolvedCommand = replaceToken(resolvedCommand, "trusted-adapter-file", adapterFile);
  }
  if (resolvedCommand.includes("<trusted-contract-file>")) {
    resolvedCommand = replaceToken(resolvedCommand, "trusted-contract-file", contractFile);
  }
  if (resolvedCommand.includes("<candidate-commit>")) {
    if (!args.has("candidate-commit")) fail(`${command.id} requires --candidate-commit`);
    const candidateCommit = args.get("candidate-commit");
    if (!/^[0-9a-f]{40}$/.test(candidateCommit)) fail("candidate-commit must be a full SHA-1");
    resolvedCommand = replaceToken(resolvedCommand, "candidate-commit", candidateCommit);
  }
  if (/<[^>]+>/.test(resolvedCommand)) fail(`unresolved command placeholder: ${command.id}`);

  const result = spawnSync("/bin/bash", ["--noprofile", "--norc", "-c", resolvedCommand], {
    cwd: candidateRoot,
    encoding: "utf8",
    env: process.env,
    maxBuffer: 128 * 1024 * 1024,
    timeout: Number(args.get("timeout-ms") ?? 1_800_000),
  });
  const stdout = result.stdout ?? "";
  const stderr = result.stderr ?? "";
  const logPrefix = `${String(commandIndex).padStart(2, "0")}-${safeId(command.id)}`;
  const stdoutPath = resolve(trustedDirInput, `${logPrefix}.stdout`);
  const stderrPath = resolve(trustedDirInput, `${logPrefix}.stderr`);
  await Promise.all([writeFile(stdoutPath, stdout), writeFile(stderrPath, stderr)]);
  const commandReceipt = {
    id: command.id,
    command_template: command.command_template,
    resolved_command: resolvedCommand,
    resolved_command_sha256: sha256(resolvedCommand),
    exit_code: result.status,
    signal: result.signal,
    spawn_error: result.error?.message ?? null,
    stdout: {path: `${logPrefix}.stdout`, bytes: Buffer.byteLength(stdout), sha256: sha256(stdout)},
    stderr: {path: `${logPrefix}.stderr`, bytes: Buffer.byteLength(stderr), sha256: sha256(stderr)},
  };
  commands.push(commandReceipt);

  for (const assertionId of command.assertion_ids ?? []) {
    const locked = lockedAssertions.get(assertionId);
    if (!locked) fail(`unknown assertion ${assertionId}`);
    const reasons = [];
    let detail = {};
    if (result.error) reasons.push(`command spawn failed: ${result.error.message}`);
    if (result.status !== 0) reasons.push(`command exited ${result.status ?? "without status"}`);
    if (locked.selector.kind === "rust_test_exact") {
      const outcomes = exactRustOutcomes(`${stdout}\n${stderr}`, locked.selector.test_name);
      detail = {
        matching_tests: outcomes.length,
        matching_test_outcomes: outcomes,
        required_matching_tests: locked.selector.required_matching_tests,
      };
      if (outcomes.length !== locked.selector.required_matching_tests) {
        reasons.push(
          `matching Rust tests ${outcomes.length}, required ${locked.selector.required_matching_tests}`,
        );
      }
      if (outcomes.some((outcome) => outcome !== "ok")) {
        reasons.push("matching Rust test did not pass");
      }
    } else if (locked.selector.kind === "json_pointer") {
      const document = parseJsonStdout(stdout);
      const selected = jsonPointer(document, locked.selector.json_pointer);
      detail = {json_pointer: locked.selector.json_pointer, found: selected.found, actual: selected.value};
      if (!selected.found) reasons.push("JSON pointer is absent");
      else if (JSON.stringify(selected.value) !== JSON.stringify(locked.selector.expected)) {
        reasons.push("JSON pointer value mismatch");
      }
    } else if (locked.selector.kind === "portable_record_set") {
      let selected;
      try {
        selected = portableResult(
          JSON.parse(await readFile(outputPaths.portable, "utf8")),
          locked.selector,
        );
      } catch (error) {
        selected = {pass: false, reasons: [`portable observations unavailable: ${error.message}`]};
      }
      reasons.push(...selected.reasons);
      detail = Object.fromEntries(Object.entries(selected).filter(([key]) => key !== "pass" && key !== "reasons"));
    } else {
      fail(`unsupported selector kind ${locked.selector.kind}`);
    }
    assertions.push({
      id: assertionId,
      command_id: command.id,
      selector_kind: locked.selector.kind,
      pass: reasons.length === 0,
      reasons,
      ...detail,
    });
  }
}

const commandsPass = commands.every((entry) => entry.exit_code === 0 && entry.spawn_error === null);
const assertionsPass = assertions.every((entry) => entry.pass);
const subjectiveGroups = manifestCase.subjective_groups ?? [];
const subjectiveOnly = lockedAssertions.size === 0 && subjectiveGroups.length > 0;
const assessmentMode = subjectiveOnly
  ? "subjective_only_packet_gate"
  : subjectiveGroups.length > 0
    ? "machine_and_subjective"
    : "machine_only";
const overallStatus = subjectiveOnly
  ? commandsPass ? "packet_gate_ready" : "packet_gate_failed"
  : commandsPass && assertionsPass ? "passed" : "failed";
const receipt = {
  protocol_id: manifest.protocol_id,
  case_id: caseId,
  classification: "construction_pilot_only",
  formal_result_eligible: false,
  manifest_sha256: sha256(manifestBytes),
  oracle_lock_sha256: sha256(lockBytes),
  trusted_inputs: trustedInputs,
  candidate_root: candidateRoot,
  assessment_mode: assessmentMode,
  machine_score_claimed: false,
  subjective_groups: subjectiveGroups.map((entry) => entry.id),
  commands,
  assertions,
  commands_pass: commandsPass,
  assertions_pass: assertionsPass,
  overall_status: overallStatus,
};
await writeFile(
  resolve(trustedDirInput, "oracle-run.json"),
  `${JSON.stringify(receipt, null, 2)}\n`,
  {mode: 0o600},
);
console.log(JSON.stringify(receipt));
if (!(commandsPass && assertionsPass)) process.exitCode = 1;
