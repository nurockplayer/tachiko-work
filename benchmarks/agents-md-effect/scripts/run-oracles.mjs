#!/usr/bin/env node

import {createHash} from "node:crypto";
import {existsSync} from "node:fs";
import {lstat, mkdir, mkdtemp, readdir, readFile, readlink, realpath, rm, writeFile} from "node:fs/promises";
import {dirname, isAbsolute, relative, resolve} from "node:path";
import {tmpdir} from "node:os";
import {fileURLToPath} from "node:url";
import {
  materializeFormalAdapterEnvelope,
  validateFormalAdapterPackage,
} from "./adapter-integrity.mjs";
import {loadControllerContext} from "./controller-context.mjs";
import {
  DENY_NETWORK_PROFILE,
  denyReadProfile,
  probeNetworkSandbox,
  runNetworkSandboxed,
  supervisedWriteProtection,
} from "./network-sandbox.mjs";

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
let commandWriteProtection = supervisedWriteProtection();

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
      "[--adapter-config /abs/config.json --adapter-probe-source /abs/probe.rs " +
      "--candidate-raw-manifest /abs/raw-manifest.json " +
      "--adapter-probe-file /abs/controller-built-probe " +
      "--expected-adapter-probe-sha256 <sha256> " +
      "--adapter-probe-build-receipt /abs/build-receipt.json " +
      "--expected-adapter-probe-build-receipt-sha256 <sha256> " +
      "--adapter-build-stage-receipt /abs/stage-receipt.json " +
      "--expected-adapter-build-stage-receipt-sha256 <sha256> " +
      "--adapter-integrity-receipt /abs/review.json " +
      "--expected-adapter-integrity-sha256 <sha256>] " +
      "[--controller-context /abs/context.json --expected-controller-context-sha256 <sha256>] " +
      "[--controller-issuance /abs/issuance.json " +
      "--expected-controller-issuance-sha256 <sha256>] " +
      "[--formal-authorization /abs/authorization.json " +
      "--expected-formal-authorization-sha256 <sha256> " +
      "--attempt-registry-entry /abs/slot.json " +
      "--expected-attempt-registry-entry-sha256 <sha256>] " +
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
    process_containment: execution.process_containment,
    network_sandbox: execution.network_sandbox,
  };
}

async function supervisedCommand(executable, commandArgs, {cwd, env, timeout, profile}) {
  const effectiveProfile = `${profile ?? DENY_NETWORK_PROFILE}` +
    commandWriteProtection.profile_suffix;
  const execution = await runNetworkSandboxed({
    executable,
    args: commandArgs,
    cwd,
    environment: env,
    timeoutMilliseconds: timeout,
    terminationGraceMilliseconds: 10_000,
    profile: effectiveProfile,
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

async function parseSupervisedWriteProtection(value, required) {
  if (value === undefined) {
    if (required) fail("formal oracle execution requires controller supervised write protection");
    return supervisedWriteProtection();
  }
  let parsed;
  try { parsed = JSON.parse(value); } catch { fail("supervised write protection is not valid JSON"); }
  const keys = Object.keys(parsed ?? {}).sort();
  if (JSON.stringify(keys) !== JSON.stringify([
    "protected_paths", "protected_roots", "schema",
  ])) {
    fail("supervised write protection has an invalid schema");
  }
  if (parsed.schema !== "tachiko-supervised-write-protection-v1" ||
      !Array.isArray(parsed.protected_roots) || !Array.isArray(parsed.protected_paths)) {
    fail("supervised write protection has an invalid schema");
  }
  for (const [label, entries] of [
    ["protected_roots", parsed.protected_roots],
    ["protected_paths", parsed.protected_paths],
  ]) {
    if (entries.some((path) => typeof path !== "string" || !isAbsolute(path) || resolve(path) !== path) ||
        new Set(entries).size !== entries.length ||
        JSON.stringify(entries) !== JSON.stringify([...entries].sort())) {
      fail(`supervised write protection ${label} must be unique sorted absolute paths`);
    }
  }
  const canonicalRoots = [];
  const canonicalPaths = [];
  for (const path of parsed.protected_roots) {
    const metadata = await lstat(path);
    if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
      fail("supervised write protection root must be a non-symlink directory");
    }
    canonicalRoots.push(await realpath(path));
  }
  for (const path of parsed.protected_paths) {
    const metadata = await lstat(path);
    if (!metadata.isFile() || metadata.isSymbolicLink()) {
      fail("supervised write protection path must be a non-symlink regular file");
    }
    canonicalPaths.push(await realpath(path));
  }
  const normalized = supervisedWriteProtection({
    protectedRoots: canonicalRoots,
    protectedPaths: canonicalPaths,
  });
  if (required && !normalized.active) {
    fail("formal oracle supervised write protection must not be empty");
  }
  return normalized;
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

async function candidateTreeIdentity(root) {
  const entries = [];
  async function walk(directory, prefix = "") {
    const children = await readdir(directory, {withFileTypes: true});
    children.sort((left, right) => Buffer.from(left.name).compare(Buffer.from(right.name)));
    for (const child of children) {
      const relativePath = prefix ? `${prefix}/${child.name}` : child.name;
      const path = resolve(directory, child.name);
      const metadata = await lstat(path);
      const mode = metadata.mode & 0o7777;
      if (metadata.isDirectory()) {
        entries.push({path: relativePath, type: "directory", mode});
        await walk(path, relativePath);
      } else if (metadata.isFile()) {
        const bytes = await readFile(path);
        entries.push({path: relativePath, type: "file", mode, bytes: bytes.length, sha256: sha256(bytes)});
      } else if (metadata.isSymbolicLink()) {
        entries.push({path: relativePath, type: "symlink", mode, target: await readlink(path)});
      } else {
        fail(`unsupported candidate input type: ${relativePath}`);
      }
    }
  }
  await walk(root);
  return {entries: entries.length, sha256: sha256(`${JSON.stringify(entries)}\n`)};
}

async function contentTreeIdentity(root) {
  const canonicalRoot = await realpath(root);
  const entries = [];
  let fileBytes = 0;
  async function walk(directory, prefix = "") {
    const children = await readdir(directory, {withFileTypes: true});
    children.sort((left, right) => Buffer.from(left.name).compare(Buffer.from(right.name)));
    for (const child of children) {
      const relativePath = prefix ? `${prefix}/${child.name}` : child.name;
      const path = resolve(directory, child.name);
      const metadata = await lstat(path);
      if (metadata.isDirectory()) {
        entries.push({path: relativePath, type: "directory"});
        await walk(path, relativePath);
      } else if (metadata.isFile()) {
        const bytes = await readFile(path);
        entries.push({path: relativePath, type: "file", bytes: bytes.length,
          sha256: sha256(bytes)});
        fileBytes += bytes.length;
      } else if (metadata.isSymbolicLink()) {
        entries.push({path: relativePath, type: "symlink", target: await readlink(path)});
      } else fail(`unsupported trusted content tree entry: ${relativePath}`);
    }
  }
  await walk(canonicalRoot);
  return {
    path: canonicalRoot,
    digest_kind: "paths-types-content-v1",
    entries: entries.length,
    file_bytes: fileBytes,
    manifest_sha256: sha256(`${JSON.stringify(entries)}\n`),
  };
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

function rustEnvironment(candidateRoot, targetDirectory, rustcInput, baseEnvironment, commandTmp) {
  const forbidden = Object.keys(baseEnvironment).filter((key) =>
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
    ...baseEnvironment,
    CARGO_TARGET_DIR: targetDirectory,
    RUSTC: rustcInput.path,
    RUSTC_BOOTSTRAP: "1",
    TMPDIR: commandTmp,
    TMP: commandTmp,
    TEMP: commandTmp,
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
  baseEnvironment,
  commandTmp,
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
  const environment = rustEnvironment(
    candidateRoot,
    targetDirectory,
    rustcInput,
    baseEnvironment,
    commandTmp,
  );
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
const evidenceContext = await loadControllerContext({
  path: args.get("controller-context"),
  expectedSha256: args.get("expected-controller-context-sha256"),
  issuancePath: args.get("controller-issuance"),
  expectedIssuanceSha256: args.get("expected-controller-issuance-sha256"),
  authorizationPath: args.get("formal-authorization"),
  expectedAuthorizationSha256: args.get("expected-formal-authorization-sha256"),
  registryPath: args.get("attempt-registry-entry"),
  expectedRegistrySha256: args.get("expected-attempt-registry-entry-sha256"),
  required: args.get("require-formal-context") === "true",
});
if (args.has("require-formal-context") && args.get("require-formal-context") !== "true") {
  fail("require-formal-context only accepts true");
}
if (evidenceContext.context && evidenceContext.context.case_id !== caseId) {
  fail("controller context case binding mismatch");
}
commandWriteProtection = await parseSupervisedWriteProtection(
  args.get("supervised-write-protection-json"),
  evidenceContext.formal_result_eligible,
);
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
let trustedValidationEnvironmentInput = null;
let trustedValidationEnvironment = null;
let trustedValidationCargo = null;
if (evidenceContext.formal_result_eligible) {
  if (!args.has("trusted-validation-environment-receipt") ||
      !/^[0-9a-f]{64}$/.test(args.get("expected-validation-environment-sha256") ?? "")) {
    fail("formal oracle execution requires a trusted validation environment receipt/hash");
  }
  trustedValidationEnvironmentInput = await trustedRegularFile(
    args.get("trusted-validation-environment-receipt"),
    "trusted validation environment receipt",
    candidateRoot,
  );
  if (trustedValidationEnvironmentInput.sha256 !==
        args.get("expected-validation-environment-sha256") ||
      trustedValidationEnvironmentInput.sha256 !==
        evidenceContext.context.trusted_validation_environment_sha256) {
    fail("trusted validation environment receipt SHA-256 mismatch");
  }
  trustedValidationEnvironment = JSON.parse((await readFile(
    trustedValidationEnvironmentInput.path,
  )).toString("utf8"));
  if (trustedValidationEnvironment.schema !==
        "tachiko-controller-trusted-validation-environment-v1" ||
      trustedValidationEnvironment.created_after_agent_extinction !== true ||
      trustedValidationEnvironment.agent_environment_inherited !== false ||
      trustedValidationEnvironment.cargo_home_read_only !== true) {
    fail("trusted validation environment receipt contract mismatch");
  }
  for (const key of ["phase", "wave_id", "run_id", "attempt_id", "candidate_id", "case_id"]) {
    if (trustedValidationEnvironment[key] !== evidenceContext.context[key]) {
      fail(`trusted validation environment ${key} mismatch`);
    }
  }
  const registeredEnvironment = trustedValidationEnvironment.environment;
  for (const key of [
    "HOME", "CODEX_HOME", "CARGO_HOME", "PATH", "RUSTUP_HOME", "PNPM_HOME",
    "LANG", "LC_ALL", "TZ", "CARGO_INCREMENTAL", "CARGO_NET_OFFLINE",
    "CARGO_TERM_COLOR", "GIT_CONFIG_NOSYSTEM", "GIT_CONFIG_GLOBAL",
    "GIT_ATTR_NOSYSTEM", "GIT_ALTERNATE_OBJECT_DIRECTORIES",
  ]) {
    if (process.env[key] !== registeredEnvironment[key]) {
      fail(`formal oracle environment ${key} differs from trusted validation`);
    }
  }
  const dangerous = Object.keys(process.env).filter((key) =>
    /^(?:RUSTC_WRAPPER|RUSTC_WORKSPACE_WRAPPER|RUSTDOC|RUSTDOCFLAGS|CARGO_BUILD_|CARGO_TARGET_)/
      .test(key));
  if (dangerous.length > 0) {
    fail(`formal oracle environment contains override keys: ${dangerous.sort().join(", ")}`);
  }
  for (const key of ["HOME", "CODEX_HOME", "CARGO_HOME"]) {
    const path = await realpath(registeredEnvironment[key]);
    const metadata = await lstat(path);
    if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
      fail(`trusted validation ${key} must be a non-symlink directory`);
    }
  }
  if ((await readdir(registeredEnvironment.HOME)).length !== 0 ||
      (await readdir(registeredEnvironment.CODEX_HOME)).length !== 0) {
    fail("trusted validation HOME/CODEX_HOME is not empty");
  }
  trustedValidationCargo = await contentTreeIdentity(registeredEnvironment.CARGO_HOME);
  for (const key of ["digest_kind", "entries", "file_bytes", "manifest_sha256"]) {
    if (trustedValidationCargo[key] !== trustedValidationEnvironment.cargo_home[key]) {
      fail(`trusted validation Cargo home ${key} mismatch`);
    }
  }
  if (trustedValidationCargo.manifest_sha256 !==
      evidenceContext.context.trusted_validation_cargo_manifest_sha256) {
    fail("controller context trusted validation Cargo home mismatch");
  }
  for (const requiredRoot of [
    registeredEnvironment.HOME,
    registeredEnvironment.CODEX_HOME,
    registeredEnvironment.CARGO_HOME,
  ]) {
    const canonical = await realpath(requiredRoot);
    if (!commandWriteProtection.protected_roots.includes(canonical)) {
      fail("formal oracle write protection omits a trusted validation root");
    }
  }
}
const networkEnforcement = await probeNetworkSandbox({
  nodeExecutable: process.execPath,
  profile: `${DENY_NETWORK_PROFILE}${commandWriteProtection.profile_suffix}`,
});

if (args.has("benchmark-dir")) fail("benchmark-dir override is not permitted");
if (evidenceContext.formal_result_eligible && (args.has("manifest") || args.has("oracle-lock"))) {
  fail("formal oracle execution may not override frozen manifests or locks");
}
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
const tw09BuilderRequested = caseId === "TW-09" && evidenceContext.formal_result_eligible &&
  manifestCase.oracle_commands.some((entry) =>
    entry.command_template.includes("<trusted-adapter-file>"));
let cargoInput;
let rustcInput;
if (rustAssertionRequested || tw09BuilderRequested) {
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

await mkdir(trustedDir, {mode: 0o700});
const outputPaths = {
  portable: resolve(trustedDir, "portable-observations.json"),
  metadata: resolve(trustedDir, "metadata-observations.json"),
  observations: resolve(trustedDir, "observations.json"),
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
let formalAdapterPackage;
let contractFile;
const trustedInputs = [
  {kind: "manifest", ...manifestInput},
  {kind: "oracle_lock", ...lockInput},
  {kind: "trusted_shell", ...trustedShellInput},
  ...frozenControlInputs.map(({control_path: controlPath, path, bytes, sha256: hash}) => ({
    kind: "frozen_control",
    control_path: controlPath,
    path,
    bytes,
    sha256: hash,
  })),
  ...(cargoInput ? [
    {kind: "trusted_cargo", ...cargoInput},
    {kind: "trusted_rustc", ...rustcInput},
  ] : []),
  ...(trustedValidationEnvironmentInput ? [{
    kind: "trusted_validation_environment",
    ...trustedValidationEnvironmentInput,
  }] : []),
];
if (adapterRequested) {
  if (!args.has("adapter-file")) fail(`${caseId} requires --adapter-file`);
  if (evidenceContext.formal_result_eligible) {
    formalAdapterPackage = await validateFormalAdapterPackage({
      adapterPath: args.get("adapter-file"),
      configPath: args.get("adapter-config"),
      integrityReceiptPath: args.get("adapter-integrity-receipt"),
      expectedIntegrityReceiptSha256: args.get("expected-adapter-integrity-sha256"),
      benchmarkRoot: benchmarkDir,
      forbiddenRoots: [
        candidateRoot,
        benchmarkDir,
        ...(evidenceContext.context.adapter_forbidden_roots ?? []),
      ],
      context: evidenceContext.context,
      candidateRoot,
      candidateManifestPath: args.get("candidate-raw-manifest"),
      probeSourcePath: args.get("adapter-probe-source"),
      controllerContextPath: evidenceContext.context_path,
      builtProbePath: args.get("adapter-probe-file"),
      expectedBuiltProbeSha256: args.get("expected-adapter-probe-sha256"),
      probeBuildReceiptPath: args.get("adapter-probe-build-receipt"),
      expectedProbeBuildReceiptSha256:
        args.get("expected-adapter-probe-build-receipt-sha256"),
      adapterBuildStageReceiptPath: args.get("adapter-build-stage-receipt"),
      expectedAdapterBuildStageReceiptSha256:
        args.get("expected-adapter-build-stage-receipt-sha256"),
      cargoPath: cargoInput?.path,
      cargoSha256: cargoInput?.sha256,
      rustcPath: rustcInput?.path,
      rustcSha256: rustcInput?.sha256,
      environment: process.env,
    });
    adapterFile = formalAdapterPackage.scaffold.path;
    adapterConfig = formalAdapterPackage.config.path;
    for (const [kind, input] of Object.entries(formalAdapterPackage)) {
      if (kind !== "approval") trustedInputs.push({kind: `formal_adapter_${kind}`, ...input});
    }
  } else {
    const trusted = await trustedRegularFile(args.get("adapter-file"), "adapter-file", candidateRoot);
    trustedInputs.push({kind: "adapter", ...trusted});
    adapterFile = trusted.path;
  }
}
if (args.has("adapter-config") && !evidenceContext.formal_result_eligible) {
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

async function verifyTrustedInputsUnchanged(boundary) {
  const identities = [];
  for (const input of trustedInputs) {
    if (!input.path) fail(`trusted input ${input.kind} lacks an absolute path binding`);
    const current = await trustedRegularFile(input.path, `trusted input ${input.kind}`, candidateRoot);
    if (current.bytes !== input.bytes || current.sha256 !== input.sha256) {
      fail(`trusted input ${input.kind} changed ${boundary}`);
    }
    identities.push({kind: input.kind, path: current.path, bytes: current.bytes, sha256: current.sha256});
  }
  if (trustedValidationEnvironment) {
    const cargo = await contentTreeIdentity(
      trustedValidationEnvironment.environment.CARGO_HOME,
    );
    if (cargo.manifest_sha256 !== trustedValidationCargo.manifest_sha256 ||
        cargo.entries !== trustedValidationCargo.entries ||
        cargo.file_bytes !== trustedValidationCargo.file_bytes) {
      fail(`trusted validation Cargo home changed ${boundary}`);
    }
    identities.push({
      kind: "trusted_validation_cargo_home",
      path: cargo.path,
      entries: cargo.entries,
      file_bytes: cargo.file_bytes,
      sha256: cargo.manifest_sha256,
    });
  }
  return sha256(`${JSON.stringify(identities)}\n`);
}

const commands = [];
const assertions = [];
const trustedInputBoundaryChecks = [];
for (const [commandIndex, command] of manifestCase.oracle_commands.entries()) {
  const trustedInputsBeforeSha256 = await verifyTrustedInputsUnchanged(`before ${command.id}`);
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
  let commandSandboxProfile;
  let adapterTemporaryRoot = null;
  const adapterExecutionCommand = command.command_template.trimStart()
    .startsWith("node <trusted-adapter-file> ");
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
    if (adapterConfig && adapterExecutionCommand) {
      resolvedCommand += ` --config ${shellQuote(adapterConfig)}`;
    }
    if (formalAdapterPackage && adapterExecutionCommand) {
      if (formalAdapterPackage.probe_source) {
        resolvedCommand += ` --probe-file ${shellQuote(formalAdapterPackage.probe.path)}`;
      }
      const deniedReadRootInputs = [
        benchmarkDir,
        "/Users",
        tmpdir(),
        dirname(formalAdapterPackage.config.path),
        dirname(formalAdapterPackage.probe.path),
        dirname(formalAdapterPackage.integrity_receipt.path),
        ...(evidenceContext.context.adapter_forbidden_roots ?? []),
      ]
        .filter((root, index, all) => all.indexOf(root) === index);
      const deniedWriteRootInputs = evidenceContext.context.adapter_write_forbidden_roots;
      const allowedWriteRootInputs = evidenceContext.context.adapter_write_allowed_roots;
      if (!Array.isArray(deniedWriteRootInputs) || deniedWriteRootInputs.length === 0 ||
          !Array.isArray(allowedWriteRootInputs) || allowedWriteRootInputs.length !== 1) {
        fail("formal adapter context requires write confinement with one trusted temporary root");
      }
      const deniedReadRoots = [];
      for (const root of deniedReadRootInputs) deniedReadRoots.push(await realpath(resolve(root)));
      const deniedWriteRoots = [];
      for (const root of deniedWriteRootInputs) deniedWriteRoots.push(await realpath(resolve(root)));
      const allowedWriteRoots = [];
      for (const root of allowedWriteRootInputs) {
        const canonical = await realpath(resolve(root));
        if (!deniedWriteRoots.some((denied) => inside(canonical, denied))) {
          fail("formal adapter temporary write root is not nested in a denied write root");
        }
        allowedWriteRoots.push(canonical);
      }
      for (let index = deniedReadRoots.length - 1; index >= 0; index -= 1) {
        if (allowedWriteRoots.includes(deniedReadRoots[index])) deniedReadRoots.splice(index, 1);
      }
      if (evidenceContext.context.adapter_tmp_initial_sha256 !== sha256("[]\n") ||
          (await readdir(allowedWriteRoots[0])).length !== 0) {
        fail("formal adapter temporary root is not freshly empty");
      }
      adapterTemporaryRoot = {
        path: allowedWriteRoots[0],
        initial_entries: 0,
        initial_sha256: sha256("[]\n"),
        created_after_candidate_core_extinction: true,
      };
      for (const root of deniedReadRoots) {
        resolvedCommand += ` --deny-read-root ${shellQuote(root)}`;
      }
      for (const root of deniedWriteRoots) {
        resolvedCommand += ` --deny-write-root ${shellQuote(root)}`;
      }
      for (const root of allowedWriteRoots) {
        resolvedCommand += ` --allow-write-root ${shellQuote(root)}`;
      }
      const deniedWritePaths = [
        adapterFile,
        formalAdapterPackage.config.path,
        formalAdapterPackage.probe.path,
        formalAdapterPackage.integrity_receipt.path,
        ...(formalAdapterPackage.probe_build_receipt
          ? [formalAdapterPackage.probe_build_receipt.path] : []),
        formalAdapterPackage.scaffold_lock.path,
        trustedShellInput.path,
        process.execPath,
        ...(cargoInput ? [cargoInput.path, rustcInput.path] : []),
      ].filter((path, index, all) => all.indexOf(path) === index);
      for (const path of deniedWritePaths) {
        resolvedCommand += ` --deny-write-path ${shellQuote(path)}`;
      }
      commandSandboxProfile = denyReadProfile(deniedReadRoots, {
        denyUnlistedReads: true,
        denyUnlistedWrites: true,
        allowReadPaths: [
          adapterFile,
          formalAdapterPackage.config.path,
          formalAdapterPackage.probe.path,
          process.execPath,
        ],
        allowReadRoots: [candidateRoot],
        denyWriteRoots: [candidateRoot, ...deniedWriteRoots],
        denyWritePaths: deniedWritePaths,
        allowWriteRoots: allowedWriteRoots,
      });
      resolvedCommand +=
        ` --expected-sandbox-profile-sha256 ${sha256(commandSandboxProfile)}` +
        ` --contract-sha256 ${sha256(await readFile(contractFile))}`;
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
  let adapterMaterialization = null;
  let candidateInputImmutability = null;
  const candidateInputBefore = formalAdapterPackage && adapterExecutionCommand
    ? await candidateTreeIdentity(candidateRoot)
    : null;
  const commandTemporaryRoot = adapterTemporaryRoot?.path ??
    await mkdtemp(resolve(trustedDir, `.command-${String(commandIndex).padStart(2, "0")}-tmp-`));
  if ((await readdir(commandTemporaryRoot)).length !== 0) {
    fail(`oracle command ${command.id} TMP is not freshly empty`);
  }
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
      trustedDir,
      timeout: ORACLE_COMMAND_TIMEOUT_MS,
      baseEnvironment: process.env,
      commandTmp: commandTemporaryRoot,
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
    const commandEnvironment = {
      ...process.env,
      TMPDIR: commandTemporaryRoot,
      TMP: commandTemporaryRoot,
      TEMP: commandTemporaryRoot,
    };
    result = await supervisedCommand(
      trustedShellInput.path,
      ["--noprofile", "--norc", "-c", resolvedCommand],
      {
      cwd: candidateRoot,
      env: commandEnvironment,
      timeout: ORACLE_COMMAND_TIMEOUT_MS,
      profile: commandSandboxProfile,
      },
    );
    executionReceipt = {process_supervision: result.process_supervision};
    if (candidateInputBefore) {
      const candidateInputAfter = await candidateTreeIdentity(candidateRoot);
      candidateInputImmutability = {
        entries: candidateInputAfter.entries,
        before_sha256: candidateInputBefore.sha256,
        after_sha256: candidateInputAfter.sha256,
        unchanged: candidateInputBefore.sha256 === candidateInputAfter.sha256,
        checked_after_process_group_extinction:
          result.process_supervision.process_group_extinct_before_capture,
      };
      if (!candidateInputImmutability.unchanged ||
          !candidateInputImmutability.checked_after_process_group_extinction) {
        fail("formal adapter changed reconstructed candidate inputs");
      }
    }
    if (
      formalAdapterPackage &&
      adapterExecutionCommand &&
      result.status === 0 && !result.error
    ) {
      adapterMaterialization = await materializeFormalAdapterEnvelope({
        stdout: result.stdout,
        outputPath: outputPaths.observations,
        caseId,
        contractSha256: sha256(await readFile(contractFile)),
        sandboxProfileSha256: sha256(commandSandboxProfile),
        processGroupExtinct:
          result.process_supervision.process_group_extinct_before_capture,
        adapterPackage: formalAdapterPackage,
      });
    }
  }
  const commandTemporaryFinal = await candidateTreeIdentity(commandTemporaryRoot);
  await rm(commandTemporaryRoot, {recursive: true, force: false});
  executionReceipt.temporary_root = {
    path: commandTemporaryRoot,
    initial_entries: 0,
    initial_sha256: sha256("[]\n"),
    final_entries: commandTemporaryFinal.entries,
    final_sha256: commandTemporaryFinal.sha256,
    inspected_after_process_group_extinction:
      result.process_supervision.process_group_extinct_before_capture,
    removed_before_next_command: !existsSync(commandTemporaryRoot),
    reused_from_prior_command: false,
  };
  const trustedInputsAfterSha256 = await verifyTrustedInputsUnchanged(`after ${command.id}`);
  trustedInputBoundaryChecks.push({
    command_id: command.id,
    before_sha256: trustedInputsBeforeSha256,
    after_sha256: trustedInputsAfterSha256,
    unchanged: trustedInputsBeforeSha256 === trustedInputsAfterSha256,
  });
  const stdout = result.stdout ?? "";
  const stderr = result.stderr ?? "";
  const stdoutPath = resolve(trustedDir, `${logPrefix}.stdout`);
  const stderrPath = resolve(trustedDir, `${logPrefix}.stderr`);
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
    trusted_adapter_materialization: adapterMaterialization,
    adapter_temporary_root: adapterTemporaryRoot,
    candidate_input_immutability: candidateInputImmutability,
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
  classification: evidenceContext.classification,
  formal_result_eligible: evidenceContext.formal_result_eligible,
  controller_context_sha256: evidenceContext.context_sha256,
  controller_issuance_sha256: evidenceContext.issuance_sha256,
  formal_authorization_sha256: evidenceContext.authorization_sha256,
  attempt_registry_entry_sha256: evidenceContext.registry_sha256,
  network_enforcement: networkEnforcement,
  supervised_write_protection: {
    schema: commandWriteProtection.schema,
    active: commandWriteProtection.active,
    protected_roots: commandWriteProtection.protected_roots,
    protected_paths: commandWriteProtection.protected_paths,
    profile_suffix_sha256: commandWriteProtection.profile_suffix_sha256,
  },
  manifest_sha256: sha256(manifestBytes),
  oracle_lock_sha256: sha256(lockBytes),
  expected_control_sha256: args.get("expected-control-sha256"),
  trusted_inputs: trustedInputs,
  trusted_inputs_postchecked_unchanged: trustedInputBoundaryChecks.every((entry) => entry.unchanged),
  trusted_input_boundary_checks: trustedInputBoundaryChecks,
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
  resolve(trustedDir, "oracle-run.json"),
  `${JSON.stringify(receipt, null, 2)}\n`,
  {mode: 0o600},
);
console.log(JSON.stringify(receipt));
if (!(commandsPass && assertionsPass)) process.exitCode = 1;
