#!/usr/bin/env node

import {createHash} from "node:crypto";
import {existsSync} from "node:fs";
import {lstat, mkdir, readFile, realpath, writeFile} from "node:fs/promises";
import {dirname, isAbsolute, relative, resolve} from "node:path";
import {fileURLToPath} from "node:url";
import {runProcessGroupOnce} from "./process-group-supervisor.mjs";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const defaultBenchmarkDir = resolve(scriptDir, "..");
const frozenManifestSha256 = "7cd25981b9edd28066530d2dc60f631b5d00edbe8b435e2c57264010b8799109";
const frozenOracleLockSha256 = "0fbd2091c19cacd6bd91dbed55bd2c056573ddfed879da0ff85830bf63ca9fbf";
const frozenControlArtifacts = [
  "environment-lock.json",
  "evaluator/cases.json",
  "evaluator/oracle-lock.json",
  "evaluator/core-score-lock.json",
  "evaluator/authority-lock.json",
  "evaluator/production-oracles.json",
];
const ORACLE_COMMAND_TIMEOUT_MS = 1_800_000;

function usage() {
  console.error(
    "usage: node run-oracles.mjs --case TW-03 --candidate-root /abs/validation " +
      "--trusted-dir /abs/trusted --expected-control-sha256 <sha256> " +
      "[--manifest /abs/production-oracles.json --expected-manifest-sha256 <sha256>] " +
      "[--oracle-lock /abs/oracle-lock.json --expected-oracle-lock-sha256 <sha256>] " +
      "[--trusted-cargo /abs/cargo --expected-cargo-sha256 <sha256>] " +
      "[--trusted-rustc /abs/rustc --expected-rustc-sha256 <sha256>] " +
      "--trusted-shell /abs/bash --expected-shell-sha256 <sha256> " +
      "[--adapter-file /abs/adapter.mjs] " +
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

function processSupervision(execution) {
  return {
    started_at: execution.started_at,
    completed_at: execution.completed_at,
    duration_seconds: execution.duration_seconds,
    deadline_seconds: execution.deadline_seconds,
    exit_code: execution.exit_code,
    signal: execution.signal,
    spawn_error: execution.spawn_error,
    timed_out: execution.timed_out,
    process_group_created: execution.process_group_created,
    termination_grace_seconds: execution.termination_grace_seconds,
    termination_grace_intervals: execution.termination_grace_intervals,
    termination_deadline_reused_for_cleanup: execution.termination_deadline_reused_for_cleanup,
    termination_signal_sent: execution.termination_signal_sent,
    kill_signal_sent: execution.kill_signal_sent,
    signal_actions: execution.signal_actions,
    descendant_cleanup_required: execution.descendant_cleanup_required,
    process_group_extinct_before_capture: execution.process_group_extinct_before_capture,
  };
}

async function supervisedCommand(executable, commandArgs, {cwd, env, timeout}) {
  const execution = await runProcessGroupOnce({
    executable,
    args: commandArgs,
    cwd,
    environment: env,
    timeoutMilliseconds: timeout,
    terminationGraceMilliseconds: 10_000,
  });
  execution.deadline_seconds = timeout / 1000;
  let error;
  if (execution.spawn_error) error = Object.assign(new Error(execution.spawn_error), {code: "ESPAWN"});
  else if (execution.timed_out) error = Object.assign(new Error("command timed out"), {code: "ETIMEDOUT"});
  return {
    status: execution.exit_code,
    signal: execution.signal,
    error,
    stdout: execution.stdout.toString("utf8"),
    stderr: execution.stderr.toString("utf8"),
    process_supervision: processSupervision(execution),
  };
}

function canonicalValue(value) {
  if (Array.isArray(value)) return value.map(canonicalValue);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value).sort().map((key) => [key, canonicalValue(value[key])]),
    );
  }
  return value;
}

function canonicalSha256(value) {
  return sha256(JSON.stringify(canonicalValue(value)));
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

async function bindLockedCandidateFiles(lockCase, candidateRoot) {
  if (!Array.isArray(lockCase.files)) fail("oracle-lock files must be an array");
  const seen = new Set();
  const observations = [];
  for (const entry of lockCase.files) {
    if (
      typeof entry.path !== "string" ||
      entry.path.length === 0 ||
      isAbsolute(entry.path) ||
      entry.path.includes("\\") ||
      entry.path.split("/").some((component) => component === "" || component === "." || component === "..")
    ) {
      fail(`invalid locked candidate file path: ${entry.path}`);
    }
    if (seen.has(entry.path)) fail(`duplicate locked candidate file path: ${entry.path}`);
    seen.add(entry.path);
    if (!/^[0-9a-f]{64}$/.test(entry.sha256 ?? "")) {
      fail(`invalid locked candidate file SHA-256: ${entry.path}`);
    }
    const requested = resolve(candidateRoot, entry.path);
    if (!inside(requested, candidateRoot) || requested === candidateRoot) {
      fail(`locked candidate file escapes candidate-root: ${entry.path}`);
    }
    const metadata = await lstat(requested);
    if (metadata.isSymbolicLink() || !metadata.isFile()) {
      fail(`locked candidate file must be a non-symlink regular file: ${entry.path}`);
    }
    const canonical = await realpath(requested);
    if (canonical !== requested) {
      fail(`locked candidate file path was redirected: ${entry.path}`);
    }
    const bytes = await readFile(canonical);
    const observedSha256 = sha256(bytes);
    if (observedSha256 !== entry.sha256) {
      fail(`locked file SHA-256 mismatch: ${entry.path}`);
    }
    observations.push({
      path: entry.path,
      canonical_path: canonical,
      bytes: bytes.length,
      sha256: observedSha256,
    });
  }
  return observations;
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

function parseRustTestCommand(command) {
  const match = command.match(
    /^cargo test -p ([a-z0-9-]+) --test ([A-Za-z0-9_-]+) --locked ([A-Za-z0-9_:.-]+) -- --exact$/,
  );
  if (!match) return null;
  return {packageName: match[1], targetName: match[2], testName: match[3]};
}

function rustEnvironment(candidateRoot, targetDirectory, rustcInput) {
  const forbidden = Object.keys(process.env).filter((key) =>
    key === "CARGO_BUILD_TARGET" ||
    key === "RUSTC" ||
    key === "RUSTDOC" ||
    key === "RUSTC_WRAPPER" ||
    key === "RUSTC_WORKSPACE_WRAPPER" ||
    /^CARGO_TARGET_.+_RUNNER$/.test(key));
  if (forbidden.length > 0) {
    fail(`runner environment override is not permitted: ${forbidden.sort().join(", ")}`);
  }
  for (const name of ["config", "config.toml"]) {
    if (existsSync(resolve(candidateRoot, ".cargo", name))) {
      fail(`candidate Cargo config is not permitted: .cargo/${name}`);
    }
  }
  return {
    ...process.env,
    CARGO_TARGET_DIR: targetDirectory,
    RUSTC: rustcInput.path,
    RUSTC_BOOTSTRAP: "1",
  };
}

function commandOutputEvidence(path, output) {
  return {path, bytes: Buffer.byteLength(output), sha256: sha256(output)};
}

async function executeTrustedRustTest({
  cargoInput,
  rustcInput,
  candidateRoot,
  commandSpec,
  lockedFiles,
  logPrefix,
  trustedDir,
  timeout,
}) {
  const commandStartedAt = new Date().toISOString();
  const commandStarted = process.hrtime.bigint();
  const commandDeadline = commandStarted + BigInt(timeout) * 1_000_000n;
  const stageProcesses = [];
  const remainingTimeout = () => {
    const remainingNanoseconds = commandDeadline - process.hrtime.bigint();
    return Math.max(1, Number(remainingNanoseconds / 1_000_000n));
  };
  const recordStage = (name, deadlineMilliseconds, result) => {
    stageProcesses.push({
      name,
      deadline_milliseconds: deadlineMilliseconds,
      process_supervision: result.process_supervision,
    });
  };
  const commandSupervision = () => ({
    started_at: commandStartedAt,
    completed_at: new Date().toISOString(),
    duration_seconds: Number(process.hrtime.bigint() - commandStarted) / 1_000_000_000,
    deadline_seconds: timeout / 1000,
    stage_processes: stageProcesses,
    all_process_groups_extinct_before_capture: stageProcesses.every(
      (entry) => entry.process_supervision.process_group_extinct_before_capture,
    ),
  });
  const targetDirectory = resolve(trustedDir, "rust-target");
  const environment = rustEnvironment(candidateRoot, targetDirectory, rustcInput);
  const metadataArgs = ["metadata", "--locked", "--format-version", "1", "--no-deps"];
  const metadataStdoutPath = `${logPrefix}.metadata.stdout`;
  const metadataStderrPath = `${logPrefix}.metadata.stderr`;
  const buildStdoutPath = `${logPrefix}.build.stdout`;
  const buildStderrPath = `${logPrefix}.build.stderr`;
  const metadataTimeout = remainingTimeout();
  const metadataResult = await supervisedCommand(cargoInput.path, metadataArgs, {
    cwd: candidateRoot,
    env: environment,
    timeout: metadataTimeout,
  });
  recordStage("cargo_metadata", metadataTimeout, metadataResult);
  if (metadataResult.status !== 0) {
    await Promise.all([
      writeFile(resolve(trustedDir, metadataStdoutPath), metadataResult.stdout ?? ""),
      writeFile(resolve(trustedDir, metadataStderrPath), metadataResult.stderr ?? ""),
    ]);
    return {
      result: metadataResult,
      resolvedCommand: `${cargoInput.path} ${metadataArgs.map(shellQuote).join(" ")}`,
      receipt: {
        execution_mode: "trusted_cargo_metadata_failed",
        command_supervision: commandSupervision(),
        process_supervision: metadataResult.process_supervision,
        toolchain: {cargo: cargoInput, rustc: rustcInput},
        rust_build: {
          metadata_process_supervision: metadataResult.process_supervision,
          metadata_command_sha256: sha256(JSON.stringify([cargoInput.path, ...metadataArgs])),
          metadata_stdout: commandOutputEvidence(metadataStdoutPath, metadataResult.stdout ?? ""),
          metadata_stderr: commandOutputEvidence(metadataStderrPath, metadataResult.stderr ?? ""),
          command_sha256: null,
          stdout: null,
          stderr: null,
          package: null,
          artifact: null,
        },
      },
    };
  }
  const metadata = JSON.parse(metadataResult.stdout);
  const packages = metadata.packages.filter((entry) => entry.name === commandSpec.packageName);
  if (packages.length !== 1) {
    fail(`trusted Cargo metadata package mismatch: ${commandSpec.packageName}`);
  }
  const packageEntry = packages[0];
  const targets = packageEntry.targets.filter((entry) => entry.name === commandSpec.targetName);
  if (targets.length !== 1) fail(`trusted Cargo metadata test target mismatch: ${commandSpec.targetName}`);
  const target = targets[0];
  if (
    JSON.stringify(target.kind) !== JSON.stringify(["test"]) ||
    JSON.stringify(target.crate_types) !== JSON.stringify(["bin"]) ||
    target.test !== true
  ) {
    fail(`custom test executable is not permitted: ${commandSpec.targetName}`);
  }
  const manifestPath = await realpath(packageEntry.manifest_path);
  const sourcePath = await realpath(target.src_path);
  if (!inside(manifestPath, candidateRoot) || !inside(sourcePath, candidateRoot)) {
    fail("Rust test manifest and source must be inside candidate-root");
  }
  for (const [path, label] of [[manifestPath, "package manifest"], [sourcePath, "test source"]]) {
    const metadata = await lstat(path);
    if (metadata.isSymbolicLink() || !metadata.isFile()) fail(`${label} must be a regular file`);
  }
  const expectedSourcePath = resolve(
    dirname(manifestPath),
    "tests",
    `${commandSpec.targetName}.rs`,
  );
  const matchingLockedSources = lockedFiles.filter(
    (entry) => entry.canonical_path === expectedSourcePath,
  );
  if (matchingLockedSources.length !== 1 || sourcePath !== expectedSourcePath) {
    fail(
      `test source must equal the unique frozen locked source ` +
        `${relative(candidateRoot, expectedSourcePath)}`,
    );
  }
  const manifestBytes = await readFile(manifestPath);
  if (/(?:^|\n)\s*["']?harness["']?\s*=\s*false(?=\s|#|$)/m.test(manifestBytes.toString("utf8"))) {
    fail("harness = false is not permitted for exact Rust oracle tests");
  }
  const packageReceipt = {
    id_sha256: sha256(packageEntry.id),
    name: packageEntry.name,
    manifest_sha256: sha256(manifestBytes),
    target_name: target.name,
    target_source_sha256: sha256(await readFile(sourcePath)),
  };

  const buildArgs = [
    "test", "-p", commandSpec.packageName,
    "--test", commandSpec.targetName,
    "--locked", "--no-run", "--message-format=json",
  ];
  const buildTimeout = remainingTimeout();
  const buildResult = await supervisedCommand(cargoInput.path, buildArgs, {
    cwd: candidateRoot,
    env: environment,
    timeout: buildTimeout,
  });
  recordStage("cargo_build", buildTimeout, buildResult);
  const artifactMessages = [];
  for (const line of (buildResult.stdout ?? "").split(/\r?\n/).filter(Boolean)) {
    try {
      const message = JSON.parse(line);
      if (
        message.reason === "compiler-artifact" &&
        message.package_id === packageEntry.id &&
        message.target?.name === commandSpec.targetName &&
        JSON.stringify(message.target?.kind) === JSON.stringify(["test"]) &&
        message.profile?.test === true &&
        typeof message.executable === "string"
      ) {
        artifactMessages.push({message, line});
      }
    } catch {
      // Cargo may emit non-JSON diagnostics on stderr, never as artifact identity.
    }
  }
  if (buildResult.status !== 0) {
    await Promise.all([
      writeFile(resolve(trustedDir, metadataStdoutPath), metadataResult.stdout ?? ""),
      writeFile(resolve(trustedDir, metadataStderrPath), metadataResult.stderr ?? ""),
      writeFile(resolve(trustedDir, buildStdoutPath), buildResult.stdout ?? ""),
      writeFile(resolve(trustedDir, buildStderrPath), buildResult.stderr ?? ""),
    ]);
    return {
      result: buildResult,
      resolvedCommand: `${cargoInput.path} ${buildArgs.map(shellQuote).join(" ")}`,
      receipt: {
        execution_mode: "trusted_cargo_build_failed",
        command_supervision: commandSupervision(),
        process_supervision: buildResult.process_supervision,
        toolchain: {cargo: cargoInput, rustc: rustcInput},
        rust_build: {
          metadata_process_supervision: metadataResult.process_supervision,
          metadata_command_sha256: sha256(JSON.stringify([cargoInput.path, ...metadataArgs])),
          metadata_stdout: commandOutputEvidence(metadataStdoutPath, metadataResult.stdout ?? ""),
          metadata_stderr: commandOutputEvidence(metadataStderrPath, metadataResult.stderr ?? ""),
          command_sha256: sha256(JSON.stringify([cargoInput.path, ...buildArgs])),
          build_process_supervision: buildResult.process_supervision,
          stdout: commandOutputEvidence(buildStdoutPath, buildResult.stdout ?? ""),
          stderr: commandOutputEvidence(buildStderrPath, buildResult.stderr ?? ""),
          package: packageReceipt,
          artifact: null,
        },
      },
    };
  }
  if (artifactMessages.length !== 1) {
    fail(
      `trusted Cargo --no-run did not emit exactly one test artifact for ` +
        `${commandSpec.targetName}: status=${buildResult.status}, artifacts=${artifactMessages.length}; ` +
        `stderr=${buildResult.stderr ?? ""}`,
    );
  }
  const artifactMessage = artifactMessages[0];
  const executableInput = resolve(artifactMessage.message.executable);
  const executableMetadata = await lstat(executableInput);
  if (executableMetadata.isSymbolicLink() || !executableMetadata.isFile()) {
    fail("Cargo test artifact must be a non-symlink regular file");
  }
  const executable = await realpath(executableInput);
  const canonicalTargetDirectory = await realpath(targetDirectory);
  if (!inside(executable, canonicalTargetDirectory)) fail("Cargo test artifact escaped trusted target dir");
  const executableBytes = await readFile(executable);
  const testArgs = [commandSpec.testName, "--exact", "-Z", "unstable-options", "--format", "json"];
  const testTimeout = remainingTimeout();
  const testResult = await supervisedCommand(executable, testArgs, {
    cwd: candidateRoot,
    env: environment,
    timeout: testTimeout,
  });
  recordStage("direct_libtest", testTimeout, testResult);
  await Promise.all([
    writeFile(resolve(trustedDir, metadataStdoutPath), metadataResult.stdout ?? ""),
    writeFile(resolve(trustedDir, metadataStderrPath), metadataResult.stderr ?? ""),
    writeFile(resolve(trustedDir, buildStdoutPath), buildResult.stdout ?? ""),
    writeFile(resolve(trustedDir, buildStderrPath), buildResult.stderr ?? ""),
  ]);
  return {
    result: testResult,
    resolvedCommand: `${executable} ${testArgs.map(shellQuote).join(" ")}`,
    receipt: {
      execution_mode: "trusted_cargo_direct_libtest",
      command_supervision: commandSupervision(),
      process_supervision: testResult.process_supervision,
      toolchain: {cargo: cargoInput, rustc: rustcInput},
      rust_build: {
        metadata_process_supervision: metadataResult.process_supervision,
        metadata_command_sha256: sha256(JSON.stringify([cargoInput.path, ...metadataArgs])),
        metadata_stdout: commandOutputEvidence(metadataStdoutPath, metadataResult.stdout ?? ""),
        metadata_stderr: commandOutputEvidence(metadataStderrPath, metadataResult.stderr ?? ""),
        command_sha256: sha256(JSON.stringify([cargoInput.path, ...buildArgs])),
        build_process_supervision: buildResult.process_supervision,
        stdout: commandOutputEvidence(buildStdoutPath, buildResult.stdout ?? ""),
        stderr: commandOutputEvidence(buildStderrPath, buildResult.stderr ?? ""),
        package: packageReceipt,
        artifact: {
          path: executable,
          executable_sha256: sha256(executableBytes),
          bytes: executableBytes.length,
          message_sha256: sha256(artifactMessage.line),
        },
      },
    },
  };
}

function libtestEvidence(output, testName) {
  const events = [];
  for (const line of output.split(/\r?\n/)) {
    try {
      const event = JSON.parse(line);
      if (event && typeof event === "object" && ["test", "suite"].includes(event.type)) {
        events.push(event);
      }
    } catch {
      // Cargo diagnostics are separate from the trusted libtest JSON events.
    }
  }
  const matching = events.filter((event) => event.type === "test" && event.name === testName);
  const started = matching.filter((event) => event.event === "started");
  const terminal = matching.filter((event) => ["ok", "failed", "ignored"].includes(event.event));
  const suites = events.filter((event) => event.type === "suite");
  const normalizedEvents = matching.map(({type, event, name}) => ({type, event, name}));
  const normalizedSuites = suites.map((event) => Object.fromEntries(
    ["type", "event", "passed", "failed", "ignored", "measured", "filtered_out"]
      .filter((key) => event[key] !== undefined)
      .map((key) => [key, event[key]]),
  ));
  return {started, terminal, suites, normalizedEvents, normalizedSuites};
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
if (args.has("timeout-ms")) fail("--timeout-ms is not permitted; oracle deadline is exactly 1800 seconds");
for (const key of [
  "case", "candidate-root", "trusted-dir", "expected-control-sha256",
  "trusted-shell", "expected-shell-sha256",
]) {
  if (!args.has(key)) usage();
}
if (!/^[0-9a-f]{64}$/.test(args.get("expected-control-sha256"))) {
  fail("expected-control-sha256 must be lowercase SHA-256");
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
if (!/^[0-9a-f]{64}$/.test(args.get("expected-shell-sha256"))) {
  fail("expected-shell-sha256 must be lowercase SHA-256");
}
const trustedShellInput = await trustedRegularFile(
  args.get("trusted-shell"),
  "trusted shell",
  candidateRoot,
);
const trustedShellMetadata = await lstat(trustedShellInput.path);
if ((trustedShellMetadata.mode & 0o111) === 0) fail("trusted shell must be executable");
if (trustedShellInput.sha256 !== args.get("expected-shell-sha256")) {
  fail("trusted shell SHA-256 mismatch");
}

if (args.has("benchmark-dir")) fail("benchmark-dir override is not permitted");
const benchmarkDir = defaultBenchmarkDir;
const manifestPath = resolve(
  args.get("manifest") ?? resolve(benchmarkDir, "evaluator/production-oracles.json"),
);
const oracleLockPath = resolve(
  args.get("oracle-lock") ?? resolve(benchmarkDir, "evaluator/oracle-lock.json"),
);
const [manifestInput, lockInput] = await Promise.all([
  trustedRegularFile(manifestPath, "manifest", candidateRoot),
  trustedRegularFile(oracleLockPath, "oracle-lock", candidateRoot),
]);
const [manifestBytes, lockBytes] = await Promise.all([
  readFile(manifestInput.path),
  readFile(lockInput.path),
]);
const expectedManifestSha256 = args.has("manifest")
  ? args.get("expected-manifest-sha256")
  : frozenManifestSha256;
const expectedOracleLockSha256 = args.has("oracle-lock")
  ? args.get("expected-oracle-lock-sha256")
  : frozenOracleLockSha256;
if (!/^[0-9a-f]{64}$/.test(expectedManifestSha256 ?? "")) {
  fail("manifest override requires expected-manifest-sha256");
}
if (!/^[0-9a-f]{64}$/.test(expectedOracleLockSha256 ?? "")) {
  fail("oracle-lock override requires expected-oracle-lock-sha256");
}
if (sha256(manifestBytes) !== expectedManifestSha256) fail("manifest SHA-256 mismatch");
if (sha256(lockBytes) !== expectedOracleLockSha256) fail("oracle-lock SHA-256 mismatch");
const usingFrozenControls = !args.has("manifest") && !args.has("oracle-lock");
const frozenControlInputs = [];
if (usingFrozenControls) {
  for (const path of frozenControlArtifacts) {
    const input = await trustedRegularFile(resolve(benchmarkDir, path), path, candidateRoot);
    frozenControlInputs.push({control_path: path, ...input});
  }
  const observedControlSha256 = sha256(`${JSON.stringify(
    frozenControlInputs.map(({control_path, bytes, sha256: hash}) => ({
      path: control_path,
      bytes,
      sha256: hash,
    })),
  )}\n`);
  if (observedControlSha256 !== args.get("expected-control-sha256")) {
    fail(
      `control SHA-256 mismatch: expected ${args.get("expected-control-sha256")}, ` +
        `got ${observedControlSha256}`,
    );
  }
}
const manifest = JSON.parse(manifestBytes.toString("utf8"));
const lock = JSON.parse(lockBytes.toString("utf8"));
if (manifest.protocol_id !== lock.protocol_id) fail("manifest/oracle protocol mismatch");
const manifestCase = manifest.cases?.find((entry) => entry.id === caseId);
const lockCase = lock.cases?.find((entry) => entry.id === caseId);
if (!manifestCase || !lockCase) fail(`unknown case ${caseId}`);
const lockedAssertions = new Map((lockCase.assertions ?? []).map((entry) => [entry.id, entry]));
const mappedAssertions = new Map((manifestCase.assertions ?? []).map((entry) => [entry.id, entry]));
if (lockedAssertions.size !== (lockCase.assertions ?? []).length) fail("duplicate locked assertion ID");
if (mappedAssertions.size !== (manifestCase.assertions ?? []).length) fail("duplicate mapped assertion ID");
if (mappedAssertions.size !== lockedAssertions.size) fail("manifest assertion coverage mismatch");
for (const [id, assertion] of lockedAssertions) {
  const mapping = mappedAssertions.get(id);
  if (!mapping || mapping.command_id !== assertion.command_id) fail(`assertion mapping mismatch: ${id}`);
}
const commandIds = manifestCase.oracle_commands.map((entry) => entry.id);
if (new Set(commandIds).size !== commandIds.length) fail("duplicate command ID");
const rustAssertionRequested = [...lockedAssertions.values()].some(
  (entry) => entry.selector?.kind === "rust_test_exact",
);
let cargoInput;
let rustcInput;
if (rustAssertionRequested) {
  if (
    !args.has("trusted-cargo") ||
    !args.has("expected-cargo-sha256") ||
    !args.has("trusted-rustc") ||
    !args.has("expected-rustc-sha256")
  ) {
    fail(
      "Rust exact tests require --trusted-cargo, --expected-cargo-sha256, " +
        "--trusted-rustc, and --expected-rustc-sha256",
    );
  }
  cargoInput = await trustedRegularFile(args.get("trusted-cargo"), "trusted-cargo", candidateRoot);
  if (cargoInput.sha256 !== args.get("expected-cargo-sha256")) {
    fail("trusted Cargo SHA-256 mismatch");
  }
  rustcInput = await trustedRegularFile(
    args.get("trusted-rustc"),
    "trusted Rust compiler",
    candidateRoot,
  );
  if (rustcInput.sha256 !== args.get("expected-rustc-sha256")) {
    fail("trusted Rust compiler SHA-256 mismatch");
  }
  if (rustcInput.path !== resolve(dirname(cargoInput.path), "rustc")) {
    fail("trusted Cargo and rustc must be siblings in the same trusted toolchain directory");
  }
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
let adapterConfig;
let contractFile;
const trustedInputs = [
  {kind: "manifest", ...manifestInput},
  {kind: "oracle_lock", ...lockInput},
  {kind: "trusted_shell", ...trustedShellInput},
  ...frozenControlInputs.map(({control_path: controlPath, bytes, sha256: hash}) => ({
    kind: "frozen_control",
    control_path: controlPath,
    bytes,
    sha256: hash,
  })),
  ...(cargoInput ? [
    {kind: "trusted_cargo", ...cargoInput},
    {kind: "trusted_rustc", ...rustcInput},
  ] : []),
];
if (adapterRequested) {
  if (!args.has("adapter-file")) fail(`${caseId} requires --adapter-file`);
  const trusted = await trustedRegularFile(args.get("adapter-file"), "adapter-file", candidateRoot);
  trustedInputs.push({kind: "adapter", ...trusted});
  adapterFile = trusted.path;
}
if (args.has("adapter-config")) {
  if (!adapterRequested) fail("adapter-config supplied for a case without an adapter");
  const trusted = await trustedRegularFile(
    args.get("adapter-config"),
    "adapter-config",
    candidateRoot,
  );
  trustedInputs.push({kind: "adapter_config", ...trusted});
  adapterConfig = trusted.path;
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
    if (
      adapterConfig &&
      command.command_template.includes("<trusted-observations-file>")
    ) {
      resolvedCommand += ` --config ${shellQuote(adapterConfig)}`;
    }
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

  const rustAssertions = (command.assertion_ids ?? [])
    .map((id) => lockedAssertions.get(id))
    .filter((entry) => entry?.selector?.kind === "rust_test_exact");
  if (rustAssertions.length > 0) {
    if (rustAssertions.length !== 1) {
      fail(`${command.id} Rust assertion is not a locked exact cargo test command`);
    }
  }
  const logPrefix = `${String(commandIndex).padStart(2, "0")}-${safeId(command.id)}`;
  let result;
  let executionReceipt = {};
  if (rustAssertions.length > 0) {
    const commandSpec = parseRustTestCommand(resolvedCommand);
    if (!commandSpec || commandSpec.testName !== rustAssertions[0].selector.test_name) {
      fail(`${command.id} Rust assertion is not a locked exact cargo test command`);
    }
    const lockedFilesBefore = await bindLockedCandidateFiles(lockCase, candidateRoot);
    const execution = await executeTrustedRustTest({
      cargoInput,
      rustcInput,
      candidateRoot,
      commandSpec,
      lockedFiles: lockedFilesBefore,
      logPrefix,
      trustedDir: trustedDirInput,
      timeout: ORACLE_COMMAND_TIMEOUT_MS,
    });
    result = execution.result;
    resolvedCommand = execution.resolvedCommand;
    executionReceipt = execution.receipt;
    const lockedFilesAfter = await bindLockedCandidateFiles(lockCase, candidateRoot);
    executionReceipt.locked_files = {
      before: lockedFilesBefore,
      after: lockedFilesAfter,
    };
  } else {
    result = await supervisedCommand(
      trustedShellInput.path,
      ["--noprofile", "--norc", "-c", resolvedCommand],
      {
      cwd: candidateRoot,
      env: process.env,
      timeout: ORACLE_COMMAND_TIMEOUT_MS,
      },
    );
    executionReceipt = {process_supervision: result.process_supervision};
  }
  const stdout = result.stdout ?? "";
  const stderr = result.stderr ?? "";
  const stdoutPath = resolve(trustedDirInput, `${logPrefix}.stdout`);
  const stderrPath = resolve(trustedDirInput, `${logPrefix}.stderr`);
  await Promise.all([writeFile(stdoutPath, stdout), writeFile(stderrPath, stderr)]);
  const commandReceipt = {
    id: command.id,
    command_template: command.command_template,
    command_template_sha256: sha256(command.command_template),
    resolved_command: resolvedCommand,
    resolved_command_sha256: sha256(resolvedCommand),
    exit_code: result.status,
    signal: result.signal,
    spawn_error: result.error?.message ?? null,
    stdout: {path: `${logPrefix}.stdout`, bytes: Buffer.byteLength(stdout), sha256: sha256(stdout)},
    stderr: {path: `${logPrefix}.stderr`, bytes: Buffer.byteLength(stderr), sha256: sha256(stderr)},
    ...executionReceipt,
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
      const evidence = libtestEvidence(stdout, locked.selector.test_name);
      const suite = evidence.suites.at(-1);
      detail = {
        evidence_mode: commandReceipt.execution_mode === "trusted_cargo_direct_libtest"
          ? "trusted_cargo_direct_libtest_json_v0.1"
          : "trusted_cargo_preflight_failure",
        matching_tests: evidence.started.length,
        matching_test_outcomes: evidence.terminal.map((entry) => entry.event),
        required_matching_tests: locked.selector.required_matching_tests,
        suite_summary: evidence.normalizedSuites.at(-1) ?? null,
        normalized_events_sha256: canonicalSha256(evidence.normalizedEvents),
        normalized_suite_sha256: canonicalSha256(evidence.normalizedSuites),
      };
      if (evidence.started.length !== locked.selector.required_matching_tests) {
        reasons.push(
          `matching Rust tests ${evidence.started.length}, required ${locked.selector.required_matching_tests}`,
        );
      }
      if (
        evidence.terminal.length !== locked.selector.required_matching_tests ||
        evidence.terminal.some((entry) => entry.event !== "ok")
      ) {
        reasons.push("matching Rust test lacks one passing libtest JSON event");
      }
      if (
        !suite ||
        suite.event !== "ok" ||
        suite.passed !== locked.selector.required_matching_tests ||
        suite.failed !== 0 ||
        suite.ignored !== 0
      ) {
        reasons.push("libtest JSON suite summary does not prove one passing test");
      }
    } else if (locked.selector.kind === "json_pointer") {
      const document = parseJsonStdout(stdout);
      const selected = jsonPointer(document, locked.selector.json_pointer);
      detail = {
        json_pointer: locked.selector.json_pointer,
        found: selected.found,
        actual: selected.value,
        actual_canonical_sha256: selected.found ? canonicalSha256(selected.value) : null,
      };
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
      detail.selected_native_sha256 = canonicalSha256(detail.selected_native ?? []);
      detail.selected_wasm_sha256 = canonicalSha256(detail.selected_wasm ?? []);
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
  expected_control_sha256: args.get("expected-control-sha256"),
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
