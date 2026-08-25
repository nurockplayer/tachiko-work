#!/usr/bin/env node

import {createHash} from "node:crypto";
import {lstat, mkdir, mkdtemp, readFile, realpath, rm, writeFile} from "node:fs/promises";
import {spawnSync} from "node:child_process";
import {dirname, isAbsolute, resolve} from "node:path";
import {tmpdir} from "node:os";
import {fileURLToPath} from "node:url";
import {
  contentSha256,
  deterministicPayload,
} from "./oracle-qualification-normalization.mjs";
import {runProcessGroupOnce} from "./process-group-supervisor.mjs";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const benchmarkDir = resolve(scriptDir, "..");
const runOracles = resolve(scriptDir, "run-oracles.mjs");
const runTw05Offline = resolve(scriptDir, "run-tw05-offline.mjs");
const materializeOracles = resolve(scriptDir, "materialize-oracles.mjs");
const sandboxExecutable = "/usr/bin/sandbox-exec";
const sandboxProfile = "(version 1)\n(allow default)\n(deny network*)\n";
const commandTimeoutMs = 1_800_000;
const terminationGraceMs = 10_000;
// The outer Node runner may execute up to 17 oracle commands and each exact
// Rust command has metadata, build, and direct-libtest stages. Its deadline
// must never preempt an inner 1,800s command plus the frozen 10s cleanup.
const oracleRunnerOuterTimeoutMs = 3 * 17 * (commandTimeoutMs + terminationGraceMs) + 60_000;
const tw05OfflineOuterTimeoutMs = 6 * (commandTimeoutMs + terminationGraceMs) + 70_000;
let expectedControlSha256;
let trustedCargo;
let trustedRustc;
let trustedShell;

function usage() {
  console.error(
    "usage: node qualify-oracles.mjs --source-repo /abs/repo --output /abs/oracles.json [--mode full|fixture-fast]",
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

function execute(executable, args, options = {}) {
  return spawnSync(executable, args, {
    encoding: "utf8",
    maxBuffer: 128 * 1024 * 1024,
    timeout: commandTimeoutMs,
    ...options,
  });
}

function supervisedExecutionReceipt(execution, timeout) {
  return {
    started_at: execution.started_at,
    completed_at: execution.completed_at,
    duration_seconds: execution.duration_seconds,
    deadline_seconds: timeout / 1000,
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

async function executeSupervised(executable, args, options = {}) {
  const timeout = options.timeout ?? commandTimeoutMs;
  const execution = await runProcessGroupOnce({
    executable,
    args,
    cwd: options.cwd,
    environment: options.env ?? process.env,
    input: options.input === undefined ? Buffer.alloc(0) : Buffer.from(options.input),
    timeoutMilliseconds: timeout,
    terminationGraceMilliseconds: terminationGraceMs,
  });
  let error;
  if (execution.spawn_error) error = Object.assign(new Error(execution.spawn_error), {code: "ESPAWN"});
  else if (execution.timed_out) error = Object.assign(new Error("command timed out"), {code: "ETIMEDOUT"});
  return {
    status: execution.exit_code,
    signal: execution.signal,
    error,
    stdout: execution.stdout.toString("utf8"),
    stderr: execution.stderr.toString("utf8"),
    process_supervision: supervisedExecutionReceipt(execution, timeout),
  };
}

function git(cwd, args, extraEnvironment = {}) {
  const result = execute("rtk", ["proxy", "git", ...args], {
    cwd,
    env: {
      ...process.env,
      GIT_CONFIG_NOSYSTEM: "1",
      GIT_CONFIG_GLOBAL: "/dev/null",
      GIT_ATTR_NOSYSTEM: "1",
      ...extraEnvironment,
    },
  });
  if (result.status !== 0) {
    fail(`rtk proxy git ${args.join(" ")} failed (${result.status}): ${result.stderr}`);
  }
  return result.stdout.trim();
}

function offlineEnvironment(extra = {}) {
  return {
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
    ...extra,
  };
}

function lastJsonLine(text) {
  for (const line of text.split(/\r?\n/).map((entry) => entry.trim()).filter(Boolean).reverse()) {
    try {
      return JSON.parse(line);
    } catch {
      // Qualification commands may print build output before their final JSON record.
    }
  }
  return null;
}

function compactCommand(entry) {
  return {
    id: entry.id,
    command_template: entry.command_template,
    command_template_sha256: entry.command_template_sha256,
    resolved_command_sha256: entry.resolved_command_sha256,
    execution_mode: entry.execution_mode ?? "shell",
    exit_code: entry.exit_code,
    signal: entry.signal,
    spawn_error: entry.spawn_error,
    process_supervision: entry.process_supervision,
    command_supervision: entry.command_supervision ?? null,
    stdout: {bytes: entry.stdout.bytes, sha256: entry.stdout.sha256},
    stderr: {bytes: entry.stderr.bytes, sha256: entry.stderr.sha256},
    ...(entry.rust_build ? {
      rust_build: entry.rust_build,
      toolchain: entry.toolchain,
      locked_files: entry.locked_files,
    } : {}),
  };
}

async function compactOracle(receipt, processStatus, trustedDir, adapterCase, runnerProcessSupervision) {
  let adapterExecution = null;
  if (adapterCase) {
    const adapterCommand = receipt.commands.find((entry) => entry.id.endsWith(".adapter"));
    const stdout = adapterCommand
      ? await readFile(resolve(trustedDir, adapterCommand.stdout.path), "utf8")
      : "";
    const observationBytes = await readFile(resolve(trustedDir, "observations.json")).catch(() => null);
    adapterExecution = {
      kind: "production_probe",
      command_exit_code: adapterCommand?.exit_code ?? null,
      observation: lastJsonLine(stdout),
      stdout: adapterCommand
        ? {bytes: adapterCommand.stdout.bytes, sha256: adapterCommand.stdout.sha256}
        : null,
      stderr: adapterCommand
        ? {bytes: adapterCommand.stderr.bytes, sha256: adapterCommand.stderr.sha256}
        : null,
      observation_artifact: observationBytes === null
        ? null
        : {bytes: observationBytes.length, sha256: sha256(observationBytes)},
      trusted_inputs: receipt.trusted_inputs
        .filter((entry) => ["adapter", "contract"].includes(entry.kind))
        .map(({kind, bytes, sha256: hash}) => ({kind, bytes, sha256: hash})),
    };
  }
  return {
    evidence: "executed",
    process_exit_code: processStatus,
    runner_process_supervision: runnerProcessSupervision,
    assessment_mode: receipt.assessment_mode,
    overall_status: receipt.overall_status,
    commands_pass: receipt.commands_pass,
    assertions_pass: receipt.assertions_pass,
    commands: receipt.commands.map(compactCommand),
    assertions: receipt.assertions,
    adapter_execution: adapterExecution,
  };
}

async function cloneAt(sourceRepo, commit, destination) {
  git(sourceRepo, ["clone", "--no-local", "--no-checkout", sourceRepo, destination]);
  git(destination, ["checkout", "--detach", commit]);
  const actualCommit = git(destination, ["rev-parse", "HEAD"]);
  if (actualCommit !== commit) fail(`checkout identity mismatch: expected ${commit}, got ${actualCommit}`);
  return {commit: actualCommit, tree: git(destination, ["rev-parse", "HEAD^{tree}"])};
}

async function materialize(caseId, sourceRepo, workspace, trustedDir) {
  const result = execute(process.execPath, [
    materializeOracles,
    "--case", caseId,
    "--source-repo", sourceRepo,
    "--validation-workspace", workspace,
    "--trusted-dir", trustedDir,
  ], {env: offlineEnvironment()});
  if (result.status !== 0) {
    fail(`${caseId} oracle materialization failed (${result.status}): ${result.stderr}`);
  }
  const receipt = lastJsonLine(result.stdout);
  if (!receipt) fail(`${caseId} materialization did not emit a receipt`);
  return {
    executed: true,
    source_commit: receipt.source_commit,
    oracle_lock_sha256: receipt.oracle_lock_sha256,
    files: receipt.materialized.map(({path, source_sha256, bytes, preexisting_sha256}) => ({
      path,
      source_sha256,
      bytes,
      preexisting_sha256,
    })),
    constructed_contracts: receipt.constructed_contracts,
  };
}

async function runCore(caseManifest, workspace) {
  const commands = [];
  for (const command of caseManifest.core_commands) {
    const result = await executeSupervised(sandboxExecutable, [
      "-p", sandboxProfile, trustedShell.path, "--noprofile", "--norc", "-c", command.command_template,
    ], {cwd: workspace, env: offlineEnvironment(), timeout: commandTimeoutMs});
    commands.push({
      id: command.id,
      command_template: command.command_template,
      command_template_sha256: sha256(command.command_template),
      resolved_command_sha256: sha256(command.command_template),
      exit_code: result.status,
      signal: result.signal,
      spawn_error: result.error?.message ?? null,
      process_supervision: result.process_supervision,
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
  return {
    evidence: "executed",
    commands,
    all_passed: commands.every((entry) => entry.exit_code === 0 && entry.spawn_error === null),
  };
}

async function runOracleCase({caseId, workspace, trustedDir, candidateCommit, adapterFile}) {
  const command = [
    runOracles,
    "--case", caseId,
    "--candidate-root", workspace,
    "--trusted-dir", trustedDir,
    "--expected-control-sha256", expectedControlSha256,
    "--candidate-commit", candidateCommit,
      "--trusted-shell", trustedShell.path,
      "--expected-shell-sha256", trustedShell.sha256,
      "--trusted-cargo", trustedCargo.path,
    "--expected-cargo-sha256", trustedCargo.sha256,
    "--trusted-rustc", trustedRustc.path,
    "--expected-rustc-sha256", trustedRustc.sha256,
  ];
  if (adapterFile) command.push("--adapter-file", adapterFile);
  const result = await executeSupervised(process.execPath, command, {
    env: offlineEnvironment(),
    timeout: oracleRunnerOuterTimeoutMs,
  });
  const receiptBytes = await readFile(resolve(trustedDir, "oracle-run.json"), "utf8").catch(() => {
    fail(
      `${caseId} production oracle exited before receipt (${result.status}): ` +
        `${result.stderr || result.stdout || result.error?.message}`,
    );
  });
  const receipt = JSON.parse(receiptBytes);
  return compactOracle(
    receipt,
    result.status,
    trustedDir,
    Boolean(adapterFile),
    result.process_supervision,
  );
}

const fixtureCommand = `#!/usr/bin/env node
import {readFile, writeFile} from "node:fs/promises";
const config = JSON.parse(await readFile(new URL("fixture.json", import.meta.url), "utf8"));
if (config.kind === "rust") {
  for (let index = 0; index < config.matches; index += 1) {
    console.log(JSON.stringify({type: "test", event: "started", name: config.name}));
    console.log(JSON.stringify({type: "test", event: "ok", name: config.name}));
  }
  console.log(JSON.stringify({type: "suite", event: "ok", passed: config.matches, failed: 0, ignored: 0}));
} else if (config.kind === "json") {
  console.log(JSON.stringify({assertions: {ready: config.value}}));
} else if (config.kind === "portable") {
  const outputIndex = process.argv.indexOf("--output") + 1;
  await writeFile(process.argv[outputIndex], JSON.stringify(config.observations));
} else if (config.kind !== "packet") {
  throw new Error("unknown fixture kind");
}
if (config.exit_code) process.exitCode = config.exit_code;
`;

async function writeFixtureManifest(familyRoot, manifest, lock) {
  const manifestBytes = `${JSON.stringify(manifest)}\n`;
  const lockBytes = `${JSON.stringify(lock)}\n`;
  const manifestPath = resolve(familyRoot, "manifest.json");
  const lockPath = resolve(familyRoot, "lock.json");
  await Promise.all([writeFile(manifestPath, manifestBytes), writeFile(lockPath, lockBytes)]);
  return {manifestBytes, lockBytes, manifestPath, lockPath};
}

async function executeSelectorFamily({root, id, command, selector, positiveConfig, negativeConfig}) {
  const familyRoot = resolve(root, id);
  const positive = resolve(familyRoot, "positive");
  const negative = resolve(familyRoot, "negative");
  await Promise.all([mkdir(positive, {recursive: true}), mkdir(negative, {recursive: true})]);
  const configBytes = [];
  for (const [directory, config] of [[positive, positiveConfig], [negative, negativeConfig]]) {
    const bytes = `${JSON.stringify(config)}\n`;
    configBytes.push(bytes);
    await Promise.all([
      writeFile(resolve(directory, "fixture-command.mjs"), fixtureCommand),
      writeFile(resolve(directory, "fixture.json"), bytes),
    ]);
    if (config.kind === "rust") {
      await Promise.all([mkdir(resolve(directory, "tests")), mkdir(resolve(directory, "src"))]);
      await Promise.all([
        writeFile(resolve(directory, "Cargo.toml"), `[package]
name = "fixture"
version = "0.0.0"
edition = "2024"

[lib]
name = "fixture_behavior"
path = "src/lib.rs"

[[test]]
name = "fixture"
path = "tests/fixture.rs"
`),
        writeFile(resolve(directory, "Cargo.lock"), `# This file is automatically @generated by Cargo.
version = 4

[[package]]
name = "fixture"
version = "0.0.0"
`),
        writeFile(
          resolve(directory, "tests/fixture.rs"),
          `#[test]\nfn ${config.name}() { assert!(fixture_behavior::behavior()); }\n`,
        ),
        writeFile(
          resolve(directory, "src/lib.rs"),
          `pub fn behavior() -> bool { ${config.matches === 1 ? "true" : "false"} }\n`,
        ),
      ]);
    }
  }
  const assertion = [{id: `${id}.assertion`, command_id: `${id}.command`, selector}];
  const manifest = {
    protocol_id: "tachiko-oracle-selector-qualification-v1",
    classification: "construction_pilot_only",
    formal_result_eligible: false,
    cases: [{
      id: "QF",
      oracle_commands: [{id: `${id}.command`, command_template: command, assertion_ids: [assertion[0].id]}],
      assertions: [{id: assertion[0].id, command_id: assertion[0].command_id, stage: "expectation_free_execution"}],
      subjective_groups: [],
    }],
  };
  const lock = {
    protocol_id: manifest.protocol_id,
    cases: [{
      id: "QF",
      files: positiveConfig.kind === "rust" ? [{
        path: "tests/fixture.rs",
        sha256: sha256(await readFile(resolve(positive, "tests/fixture.rs"))),
      }] : [],
      assertions: assertion,
    }],
  };
  const controls = await writeFixtureManifest(familyRoot, manifest, lock);
  const executeFixture = (candidate, trusted) => executeSupervised(process.execPath, [
    runOracles,
    "--case", "QF",
    "--candidate-root", candidate,
    "--trusted-dir", trusted,
    "--manifest", controls.manifestPath,
    "--oracle-lock", controls.lockPath,
    "--expected-manifest-sha256", sha256(controls.manifestBytes),
    "--expected-oracle-lock-sha256", sha256(controls.lockBytes),
    "--expected-control-sha256", expectedControlSha256,
    "--trusted-shell", trustedShell.path,
    "--expected-shell-sha256", trustedShell.sha256,
    "--trusted-cargo", trustedCargo.path,
    "--expected-cargo-sha256", trustedCargo.sha256,
    "--trusted-rustc", trustedRustc.path,
    "--expected-rustc-sha256", trustedRustc.sha256,
  ], {
    env: offlineEnvironment({PATH: `${candidate}:${process.env.PATH}`}),
    timeout: oracleRunnerOuterTimeoutMs,
  });
  const positiveTrusted = resolve(familyRoot, "positive-receipt");
  const negativeTrusted = resolve(familyRoot, "negative-receipt");
  const positiveResult = await executeFixture(positive, positiveTrusted);
  const negativeResult = await executeFixture(negative, negativeTrusted);
  if (positiveResult.status !== 0 || negativeResult.status !== 1) {
    fail(
      `${id} selector qualification failed: positive=${positiveResult.status}, ` +
        `negative=${negativeResult.status}; positive_stderr=${positiveResult.stderr}; ` +
        `negative_stderr=${negativeResult.stderr}`,
    );
  }
  const positiveReceipt = JSON.parse(await readFile(resolve(positiveTrusted, "oracle-run.json"), "utf8"));
  const negativeReceipt = JSON.parse(await readFile(resolve(negativeTrusted, "oracle-run.json"), "utf8"));
  return {
    id,
    evidence: "executed",
    fixture_sha256: sha256(`${fixtureCommand}${configBytes[0]}${configBytes[1]}`),
    positive: {
      accepted: positiveReceipt.assertions[0].pass,
      runner_status: positiveReceipt.overall_status,
      command_exit_code: positiveReceipt.commands[0].exit_code,
    },
    negative: {
      discriminated: !negativeReceipt.assertions[0].pass,
      runner_status: negativeReceipt.overall_status,
      command_exit_code: negativeReceipt.commands[0].exit_code,
    },
  };
}

async function qualifyPacketGate(root) {
  const familyRoot = resolve(root, "subjective-packet-gate");
  const candidate = resolve(familyRoot, "candidate");
  await mkdir(candidate, {recursive: true});
  await Promise.all([
    writeFile(resolve(candidate, "fixture-command.mjs"), fixtureCommand),
    writeFile(resolve(candidate, "fixture.json"), `${JSON.stringify({kind: "packet"})}\n`),
  ]);
  const manifest = {
    protocol_id: "tachiko-subjective-gate-qualification-v1",
    classification: "construction_pilot_only",
    formal_result_eligible: false,
    cases: [{
      id: "QF",
      oracle_commands: [{id: "packet.command", command_template: "node fixture-command.mjs", assertion_ids: []}],
      assertions: [],
      subjective_groups: [{id: "semantic", stage: "blinded_review_packet"}],
    }],
  };
  const lock = {protocol_id: manifest.protocol_id, cases: [{id: "QF", assertions: []}]};
  const controls = await writeFixtureManifest(familyRoot, manifest, lock);
  const trusted = resolve(familyRoot, "receipt");
  const result = await executeSupervised(process.execPath, [
    runOracles,
    "--case", "QF",
    "--candidate-root", candidate,
    "--trusted-dir", trusted,
    "--manifest", controls.manifestPath,
    "--oracle-lock", controls.lockPath,
    "--expected-manifest-sha256", sha256(controls.manifestBytes),
    "--expected-oracle-lock-sha256", sha256(controls.lockBytes),
    "--expected-control-sha256", expectedControlSha256,
    "--trusted-shell", trustedShell.path,
    "--expected-shell-sha256", trustedShell.sha256,
    "--trusted-cargo", trustedCargo.path,
    "--expected-cargo-sha256", trustedCargo.sha256,
    "--trusted-rustc", trustedRustc.path,
    "--expected-rustc-sha256", trustedRustc.sha256,
  ], {env: offlineEnvironment(), timeout: oracleRunnerOuterTimeoutMs});
  const receipt = JSON.parse(await readFile(resolve(trusted, "oracle-run.json"), "utf8"));
  if (result.status !== 0 || receipt.overall_status !== "packet_gate_ready") {
    fail("subjective packet-gate qualification failed");
  }
  return {
    id: "subjective_packet_gate",
    evidence: "executed",
    qualification: "deterministic_gate_only",
    runner_status: receipt.overall_status,
    semantic_discrimination_machine_qualified: false,
    semantic_discrimination_deferred_to: "deterministic_blinded_packet_fixtures",
  };
}

async function materializeCaseWorkspaces(caseEntry, root, sourceRepo) {
  const caseRoot = resolve(root, caseEntry.id);
  const targetWorkspace = resolve(caseRoot, "target");
  const negativeWorkspace = resolve(caseRoot, "negative");
  await mkdir(caseRoot, {recursive: true});
  const negativeIdentity = await cloneAt(sourceRepo, caseEntry.historical_base_commit, negativeWorkspace);
  if (negativeIdentity.tree !== caseEntry.historical_base_tree) {
    fail(`${caseEntry.id} historical base tree mismatch`);
  }

  let targetIdentity;
  let targetKind = "historical_target";
  if (caseEntry.id === "TW-09") {
    targetIdentity = await cloneAt(sourceRepo, caseEntry.historical_base_commit, targetWorkspace);
    const patchPath = resolve(benchmarkDir, "evaluator/construction-pilots/TW-09-rebased.patch");
    const patchBytes = await readFile(patchPath);
    if (sha256(patchBytes) !== "5bb0d435a779710434b04f8225741a533a2ac79335420d451bffc79aa6fd81cb") {
      fail("TW-09 trusted rebased patch hash mismatch");
    }
    git(targetWorkspace, ["apply", "--index", patchPath]);
    const pilotCommit = git(
      targetWorkspace,
      [
        "-c", "user.name=Tachiko Benchmark Construction",
        "-c", "user.email=benchmark.invalid@example.invalid",
        "commit", "--no-gpg-sign", "-m", "construction-only TW-09 rebased behavioral pilot",
      ],
      {
        GIT_AUTHOR_NAME: "Tachiko Benchmark Construction",
        GIT_AUTHOR_EMAIL: "benchmark.invalid@example.invalid",
        GIT_AUTHOR_DATE: "2000-01-01T00:00:00Z",
        GIT_COMMITTER_NAME: "Tachiko Benchmark Construction",
        GIT_COMMITTER_EMAIL: "benchmark.invalid@example.invalid",
        GIT_COMMITTER_DATE: "2000-01-01T00:00:00Z",
      },
    );
    void pilotCommit;
    targetIdentity = {
      commit: git(targetWorkspace, ["rev-parse", "HEAD"]),
      tree: git(targetWorkspace, ["rev-parse", "HEAD^{tree}"]),
      patch_sha256: sha256(patchBytes),
    };
    if (targetIdentity.commit !== "fdf1963c54254f62f03f46dc936d60baf178b0f8") {
      fail(`TW-09 deterministic pilot commit mismatch: ${targetIdentity.commit}`);
    }
    if (targetIdentity.tree !== "82854a472bd6aca1cab70b750fdcae864675ce5c") {
      fail(`TW-09 replay tree mismatch: ${targetIdentity.tree}`);
    }
    targetKind = "trusted_rebased_replay_positive";
  } else {
    targetIdentity = await cloneAt(sourceRepo, caseEntry.ground_truth_commit, targetWorkspace);
  }
  const [targetOverlay, negativeOverlay] = await Promise.all([
    materialize(caseEntry.id, sourceRepo, targetWorkspace, resolve(caseRoot, "target-overlay")),
    materialize(caseEntry.id, sourceRepo, negativeWorkspace, resolve(caseRoot, "negative-overlay")),
  ]);
  return {
    caseRoot,
    targetWorkspace,
    negativeWorkspace,
    materialization: {
      target: {kind: targetKind, ...targetIdentity, ...targetOverlay},
      negative: {kind: "historical_behavior_missing_base", ...negativeIdentity, ...negativeOverlay},
    },
  };
}

async function qualifyCase({caseEntry, caseManifest, root, sourceRepo}) {
  const workspaces = await materializeCaseWorkspaces(caseEntry, root, sourceRepo);
  const adapterFiles = {
    "TW-05": resolve(benchmarkDir, "evaluator/adapters/TW-05/historical-target-adapter.mjs"),
    "TW-09": resolve(benchmarkDir, "evaluator/adapters/TW-09/historical-target-adapter.mjs"),
  };
  const targetCore = await runCore(caseManifest, workspaces.targetWorkspace);
  const negativeCore = await runCore(caseManifest, workspaces.negativeWorkspace);
  let offlineHistoricalTarget;
  let offlineBehaviorMissingNegative;
  if (caseEntry.id === "TW-05") {
    offlineHistoricalTarget = await runTw05OfflineQualification(
      workspaces.targetWorkspace,
      resolve(workspaces.caseRoot, "target-offline.json"),
    );
    offlineBehaviorMissingNegative = await runTw05OfflineQualification(
      workspaces.negativeWorkspace,
      resolve(workspaces.caseRoot, "negative-offline.json"),
    );
    if (!offlineHistoricalTarget.pass || offlineBehaviorMissingNegative.pass) {
      fail("TW-05 offline target/negative qualification did not discriminate real execution");
    }
  }
  const targetOracle = await runOracleCase({
    caseId: caseEntry.id,
    workspace: workspaces.targetWorkspace,
    trustedDir: resolve(workspaces.caseRoot, "target-oracle"),
    candidateCommit: workspaces.materialization.target.commit,
    adapterFile: adapterFiles[caseEntry.id],
  });
  const negativeOracle = await runOracleCase({
    caseId: caseEntry.id,
    workspace: workspaces.negativeWorkspace,
    trustedDir: resolve(workspaces.caseRoot, "negative-oracle"),
    candidateCommit: caseEntry.historical_base_commit,
    adapterFile: adapterFiles[caseEntry.id],
  });
  const subjectiveOnly = caseManifest.assertions.length === 0 && caseManifest.subjective_groups.length > 0;
  const entry = {
    case_id: caseEntry.id,
    materialization: workspaces.materialization,
    target: {
      core: targetCore,
      oracle: targetOracle,
      accepted: subjectiveOnly ? null : targetOracle.overall_status === "passed",
      adapter_execution: targetOracle.adapter_execution,
    },
    negative: {
      core: negativeCore,
      oracle: negativeOracle,
      discriminated: subjectiveOnly ? null : negativeOracle.assertions.some((assertion) => !assertion.pass),
      adapter_execution: negativeOracle.adapter_execution,
    },
    qualification: subjectiveOnly ? "packet_gate_only" : "machine_qualified",
    machine_semantic_discrimination_qualified: !subjectiveOnly,
    ...(caseEntry.id === "TW-05" ? {
      offline_historical_target: offlineHistoricalTarget,
      offline_behavior_missing_negative: offlineBehaviorMissingNegative,
    } : {}),
  };
  if (!targetCore.all_passed) fail(`${caseEntry.id} historical target core command failed`);
  if (subjectiveOnly) {
    if (targetOracle.overall_status !== "packet_gate_ready") {
      fail(`${caseEntry.id} target packet gate did not pass`);
    }
  } else if (caseEntry.id === "TW-05") {
    entry.target.expected_contract_miss = true;
    entry.target.calibration = "frozen target lacks expected-revision input and stale-mutation rejection";
    if (entry.target.accepted || !entry.negative.discriminated) {
      fail("TW-05 calibration did not preserve the required stale-revision miss");
    }
  } else if (!entry.target.accepted || !entry.negative.discriminated) {
    fail(`${caseEntry.id} did not accept the executed positive and discriminate the executed negative`);
  }
  return {entry, workspaces};
}

async function qualifyTw05Reference(root) {
  const candidate = resolve(benchmarkDir, "evaluator/adapters/TW-05/reference-runtime");
  const adapter = resolve(benchmarkDir, "evaluator/adapters/TW-05/reference-adapter.mjs");
  const oracle = await runOracleCase({
    caseId: "TW-05",
    workspace: candidate,
    trustedDir: resolve(root, "tw05-reference-oracle"),
    candidateCommit: "0000000000000000000000000000000000000000",
    adapterFile: adapter,
  });
  if (oracle.overall_status !== "passed") fail("TW-05 controlled reference-positive failed");
  return {
    accepted: true,
    label: "controlled_reference_positive_for_missing_historical_contract_behavior",
    oracle,
  };
}

async function runTw05OfflineQualification(workspace, output) {
  const result = await executeSupervised(process.execPath, [
    runTw05Offline,
    "--candidate-root", workspace,
    "--output", output,
  ], {env: offlineEnvironment(), timeout: tw05OfflineOuterTimeoutMs});
  const receipt = JSON.parse(await readFile(output, "utf8"));
  return {
    evidence: "executed",
    process_exit_code: result.status,
    process_supervision: result.process_supervision,
    pass: receipt.pass,
    offline: receipt.offline,
    package_manager_dependency: receipt.package_manager_dependency,
    network_enforcement: {
      mode: receipt.network_enforcement.mode,
      sandbox_executable_sha256: receipt.network_enforcement.sandbox_executable_sha256,
      profile_sha256: receipt.network_enforcement.profile_sha256,
      probe_script_sha256: receipt.network_enforcement.probe_script_sha256,
      probe_denied: receipt.network_enforcement.probe_denied,
    },
    executables: receipt.executables.map(({name, sha256: hash}) => ({name, sha256: hash})),
    executions: receipt.executions.map(({purpose, name, args, exit_code, signal, spawn_error, stdout, stderr}) => ({
      purpose,
      name,
      args,
      exit_code,
      signal,
      spawn_error,
      stdout,
      stderr,
    })),
  };
}

function syntheticNormalizationCase(runRoot) {
  const runSpecificSha256 = sha256(runRoot);
  const outputEvidence = {
    bytes: Buffer.byteLength(runRoot),
    sha256: runSpecificSha256,
  };
  const command = {
    id: "fixture.rust-command",
    command_template: "cargo test -p fixture --test fixture --locked locked_name -- --exact",
    command_template_sha256: sha256(
      "cargo test -p fixture --test fixture --locked locked_name -- --exact",
    ),
    resolved_command: `${runRoot}/rust-target/fixture --exact`,
    resolved_command_sha256: runSpecificSha256,
    execution_mode: "trusted_cargo_direct_libtest",
    exit_code: 0,
    signal: null,
    spawn_error: null,
    stdout: outputEvidence,
    stderr: outputEvidence,
    toolchain: {cargo: trustedCargo, rustc: trustedRustc},
    rust_build: {
      stdout: outputEvidence,
      stderr: outputEvidence,
      package: {
        name: "fixture",
        manifest_sha256: "1".repeat(64),
        target_name: "fixture",
        target_source_sha256: "2".repeat(64),
      },
      artifact: {
        path: `${runRoot}/rust-target/fixture`,
        message_sha256: runSpecificSha256,
        executable_sha256: runSpecificSha256,
      },
    },
  };
  const core = {evidence: "executed", all_passed: true, commands: [command]};
  const oracle = {
    evidence: "executed",
    process_exit_code: 0,
    assessment_mode: "machine_only",
    overall_status: "passed",
    commands_pass: true,
    assertions_pass: true,
    commands: [command],
    assertions: [{
      id: "fixture.rust-assertion",
      command_id: command.id,
      selector_kind: "rust_test_exact",
      pass: true,
      reasons: [],
      evidence_mode: "trusted_cargo_direct_libtest_json_v0.1",
      matching_tests: 1,
      matching_test_outcomes: ["ok"],
      required_matching_tests: 1,
      suite_summary: {
        type: "suite",
        event: "ok",
        passed: 1,
        failed: 0,
        ignored: 0,
        exec_time: runRoot,
      },
      normalized_events_sha256: "3".repeat(64),
      normalized_suite_sha256: "4".repeat(64),
    }],
    adapter_execution: {
      kind: "production_probe",
      command_exit_code: 0,
      observation: {
        contract_id: "tachiko-tw09-stable-diagnostic-facts-v1",
        stable_facts_match: true,
        cargo_stdout_sha256: runSpecificSha256,
        cargo_stderr_sha256: runSpecificSha256,
        build_chatter_sha256: runSpecificSha256,
        run_root: runRoot,
      },
      stdout: outputEvidence,
      stderr: outputEvidence,
      observation_artifact: outputEvidence,
      trusted_inputs: [{kind: "adapter", bytes: 1, sha256: "5".repeat(64)}],
    },
  };
  const offline = {
    evidence: "executed",
    process_exit_code: 0,
    pass: true,
    offline: true,
    package_manager_dependency: false,
    network_enforcement: {mode: "fixture-deny-network", probe_denied: true},
    executables: [{name: "cargo", sha256: trustedCargo.sha256}],
    executions: [{
      purpose: "fixture",
      name: "cargo",
      args: ["test", "--locked"],
      exit_code: 0,
      signal: null,
      spawn_error: null,
      stdout: outputEvidence,
      stderr: outputEvidence,
    }],
  };
  return {
    case_id: "QF-TW09-NORMALIZATION",
    materialization: {
      target: {kind: "synthetic_normalization_fixture", commit: "6".repeat(40)},
      negative: {kind: "synthetic_normalization_fixture", commit: "7".repeat(40)},
    },
    qualification: "normalization_pipeline_fixture",
    machine_semantic_discrimination_qualified: true,
    target: {accepted: true, core, oracle},
    negative: {discriminated: true, core, oracle},
    offline_historical_target: offline,
    offline_behavior_missing_negative: {...offline, pass: false, process_exit_code: 1},
  };
}

const args = parseArgs(process.argv.slice(2));
for (const key of ["source-repo", "output"]) {
  if (!args.has(key)) usage();
}
if (!isAbsolute(args.get("source-repo")) || !isAbsolute(args.get("output"))) {
  fail("source-repo and output must be absolute");
}
const mode = args.get("mode") ?? "full";
if (!["full", "fixture-fast"].includes(mode)) fail("mode must be full or fixture-fast");
const sourceRepo = await realpath(resolve(args.get("source-repo")));
const output = resolve(args.get("output"));
const environmentLock = JSON.parse(
  await readFile(resolve(benchmarkDir, "environment-lock.json"), "utf8"),
);
const lockedBash = environmentLock.toolchain?.bash;
if (!lockedBash || !isAbsolute(lockedBash.path) ||
    !/^[0-9a-f]{64}$/.test(lockedBash.binary_sha256 ?? "") ||
    typeof lockedBash.version !== "string") {
  fail("environment lock does not contain a complete Bash identity");
}
const trustedShellPath = await realpath(lockedBash.path);
const trustedShellMetadata = await lstat(trustedShellPath);
if (trustedShellMetadata.isSymbolicLink() || !trustedShellMetadata.isFile() ||
    (trustedShellMetadata.mode & 0o111) === 0) {
  fail("trusted Bash must be an executable non-symlink regular file");
}
const trustedShellBytes = await readFile(trustedShellPath);
const trustedShellVersion = execute(lockedBash.path, ["--version"]);
if (sha256(trustedShellBytes) !== lockedBash.binary_sha256 ||
    trustedShellVersion.status !== 0 ||
    !trustedShellVersion.stdout.startsWith(`GNU bash, version ${lockedBash.version.slice("GNU bash ".length)}`)) {
  fail("trusted Bash differs from the environment lock");
}
trustedShell = {
  path: trustedShellPath,
  locked_path: lockedBash.path,
  bytes: trustedShellBytes.length,
  sha256: sha256(trustedShellBytes),
  version: lockedBash.version,
};
const rustupCargo = execute("rustup", ["which", "cargo"]);
if (rustupCargo.status !== 0 || !rustupCargo.stdout.trim()) fail("trusted Cargo is unavailable");
const trustedCargoPath = await realpath(rustupCargo.stdout.trim());
const trustedCargoMetadata = await lstat(trustedCargoPath);
if (trustedCargoMetadata.isSymbolicLink() || !trustedCargoMetadata.isFile()) {
  fail("trusted Cargo must be a non-symlink regular file");
}
const trustedCargoBytes = await readFile(trustedCargoPath);
trustedCargo = {
  path: trustedCargoPath,
  bytes: trustedCargoBytes.length,
  sha256: sha256(trustedCargoBytes),
};
const rustupRustc = execute("rustup", ["which", "rustc"]);
if (rustupRustc.status !== 0 || !rustupRustc.stdout.trim()) fail("trusted rustc is unavailable");
const trustedRustcPath = await realpath(rustupRustc.stdout.trim());
const trustedRustcMetadata = await lstat(trustedRustcPath);
if (trustedRustcMetadata.isSymbolicLink() || !trustedRustcMetadata.isFile()) {
  fail("trusted rustc must be a non-symlink regular file");
}
if (trustedRustcPath !== resolve(dirname(trustedCargoPath), "rustc")) {
  fail("trusted Cargo and rustc must share one toolchain directory");
}
const trustedRustcBytes = await readFile(trustedRustcPath);
trustedRustc = {
  path: trustedRustcPath,
  bytes: trustedRustcBytes.length,
  sha256: sha256(trustedRustcBytes),
};
const controlArtifacts = [
  "environment-lock.json",
  "evaluator/cases.json",
  "evaluator/oracle-lock.json",
  "evaluator/core-score-lock.json",
  "evaluator/authority-lock.json",
  "evaluator/production-oracles.json",
];
const controlObservations = [];
for (const path of controlArtifacts) {
  const bytes = await readFile(resolve(benchmarkDir, path));
  controlObservations.push({path, bytes: bytes.length, sha256: sha256(bytes)});
}
expectedControlSha256 = sha256(`${JSON.stringify(controlObservations)}\n`);
const qualificationRoot = await realpath(
  await mkdtemp(resolve(tmpdir(), "tachiko-oracle-qualification-")),
);

try {
  const sandboxBytes = await readFile(sandboxExecutable).catch(() => {
    fail("/usr/bin/sandbox-exec is required for construction qualification");
  });
  const probeResult = await executeSupervised(sandboxExecutable, [
    "-p", sandboxProfile, process.execPath, resolve(scriptDir, "probe-network-denial.mjs"),
  ], {env: offlineEnvironment(), timeout: 10_000});
  if (probeResult.status !== 0 || !/^network-denied:(?:EPERM|EACCES)\s*$/.test(probeResult.stdout)) {
    fail(`qualification network-denial probe failed: ${probeResult.stderr || probeResult.stdout}`);
  }

  const [casesBytes, manifestBytes, lockBytes] = await Promise.all([
    readFile(resolve(benchmarkDir, "evaluator/cases.json")),
    readFile(resolve(benchmarkDir, "evaluator/production-oracles.json")),
    readFile(resolve(benchmarkDir, "evaluator/oracle-lock.json")),
  ]);
  const casesDocument = JSON.parse(casesBytes.toString("utf8"));
  const manifest = JSON.parse(manifestBytes.toString("utf8"));
  const expectedRecord = {index: 3, class: 0, bits: "good", auxiliary: "9"};
  const families = [];
  families.push(await executeSelectorFamily({
    root: qualificationRoot,
    id: "rust_test_exact",
    command: "cargo test -p fixture --test fixture --locked locked_name -- --exact",
    selector: {kind: "rust_test_exact", test_name: "locked_name", required_matching_tests: 1},
    positiveConfig: {kind: "rust", name: "locked_name", matches: 1},
    negativeConfig: {kind: "rust", name: "locked_name", matches: 0},
  }));
  families.push(await executeSelectorFamily({
    root: qualificationRoot,
    id: "json_pointer",
    command: "node fixture-command.mjs",
    selector: {kind: "json_pointer", json_pointer: "/assertions/ready", expected: true},
    positiveConfig: {kind: "json", value: true},
    negativeConfig: {kind: "json", value: false},
  }));
  families.push(await executeSelectorFamily({
    root: qualificationRoot,
    id: "portable_record_set",
    command: "node fixture-command.mjs --output <trusted-portable-observations-file>",
    selector: {
      kind: "portable_record_set",
      indexes: [3],
      expected_records: [expectedRecord],
      require_selected_native_wasm_equal: true,
      reject_class: 255,
    },
    positiveConfig: {
      kind: "portable",
      observations: {contract_id: "tachiko-portable-observations-v1", native: [expectedRecord], wasm: [expectedRecord]},
    },
    negativeConfig: {
      kind: "portable",
      observations: {
        contract_id: "tachiko-portable-observations-v1",
        native: [{...expectedRecord, bits: "bad"}],
        wasm: [expectedRecord],
      },
    },
  }));
  families.push(await qualifyPacketGate(qualificationRoot));

  const caseQualifications = [];
  if (mode === "full") {
    for (const caseEntry of casesDocument.cases) {
      console.error(`qualifying ${caseEntry.id}: materialize target/base and execute frozen mappings`);
      const caseManifest = manifest.cases.find((entry) => entry.id === caseEntry.id);
      if (!caseManifest) fail(`production manifest omits ${caseEntry.id}`);
      const qualified = await qualifyCase({
        caseEntry,
        caseManifest,
        root: resolve(qualificationRoot, "cases"),
        sourceRepo,
      });
      caseQualifications.push(qualified.entry);
      await rm(qualified.workspaces.caseRoot, {recursive: true, force: true});
    }

    const tw05 = caseQualifications.find((entry) => entry.case_id === "TW-05");
    tw05.reference_positive = await qualifyTw05Reference(qualificationRoot);
    const tw09 = caseQualifications.find((entry) => entry.case_id === "TW-09");
    families.push({
      id: "tw05_normalized_contract",
      evidence: "executed",
      positive: {accepted: tw05.reference_positive.accepted, source: "controlled_reference_runtime"},
      negative: {discriminated: tw05.negative.discriminated, source: "historical_behavior_missing_base"},
      historical_target_calibration: {
        accepted: tw05.target.accepted,
        expected_contract_miss: tw05.target.expected_contract_miss,
      },
    });
    families.push({
      id: "tw05_offline_direct_execution",
      evidence: "executed",
      positive: {accepted: tw05.offline_historical_target.pass},
      negative: {discriminated: !tw05.offline_behavior_missing_negative.pass},
    });
    families.push({
      id: "tw09_normalized_contract",
      evidence: "executed",
      positive: {accepted: tw09.target.accepted, source: "trusted_rebased_replay_positive"},
      negative: {discriminated: tw09.negative.discriminated, source: "historical_behavior_missing_base"},
    });
  } else {
    caseQualifications.push(syntheticNormalizationCase(qualificationRoot));
  }

  const controlPaths = [
    "evaluator/cases.json",
    "evaluator/oracle-lock.json",
    "evaluator/production-oracles.json",
    "evaluator/contracts/TW-05-resident-parity.json",
    "evaluator/contracts/TW-09-stable-diagnostic-facts.json",
    "evaluator/adapters/candidate-adapter.mjs",
    "evaluator/adapters/candidate-adapter-config.schema.json",
    "evaluator/adapters/README.md",
    "evaluator/adapters/TW-05/historical-target-adapter.mjs",
    "evaluator/adapters/TW-05/reference-adapter.mjs",
    "evaluator/adapters/TW-05/reference-runtime/Cargo.lock",
    "evaluator/adapters/TW-05/reference-runtime/Cargo.toml",
    "evaluator/adapters/TW-05/reference-runtime/src/lib.rs",
    "evaluator/adapters/TW-05/reference-runtime/src/main.rs",
    "evaluator/adapters/TW-05/reference-runtime/worker.mjs",
    "evaluator/adapters/TW-09/historical-target-adapter.mjs",
    "evaluator/adapters/TW-09/historical-target-probe.rs",
    "evaluator/construction-pilots/TW-09-rebased.patch",
    "scripts/materialize-oracles.mjs",
    "scripts/oracle-qualification-normalization.mjs",
    "scripts/process-group-supervisor.mjs",
    "scripts/qualify-oracles.mjs",
    "scripts/run-oracles.mjs",
    "scripts/run-tw05-offline.mjs",
    "scripts/probe-network-denial.mjs",
    "scripts/validate-tw05-observations.mjs",
    "scripts/validate-tw09-stable-facts.mjs",
    "scripts/verify-benchmark.mjs",
    "scripts/verify-oracle-qualification.mjs",
  ];
  const controls = [];
  for (const path of controlPaths) {
    const bytes = await readFile(resolve(benchmarkDir, path));
    controls.push({path, bytes: bytes.length, sha256: sha256(bytes)});
  }
  const runReceipt = {
    schema: "tachiko-oracle-qualification-run-v3",
    protocol_id: casesDocument.protocol_id,
    classification: "construction_pilot_only",
    formal_result_eligible: false,
    execution_standard: "practical_internal_v1",
    mode,
    no_codex_launched: true,
    trusted_cargo: trustedCargo,
    trusted_rustc: trustedRustc,
    trusted_shell: trustedShell,
    expected_control_sha256: expectedControlSha256,
    controls,
    frozen_manifest_sha256: sha256(manifestBytes),
    frozen_oracle_lock_sha256: sha256(lockBytes),
    network_enforcement: {
      mode: "darwin_sandbox_deny_network",
      sandbox_executable_sha256: sha256(sandboxBytes),
      profile_sha256: sha256(sandboxProfile),
      probe_script_sha256: sha256(await readFile(resolve(scriptDir, "probe-network-denial.mjs"))),
      probe_denied: true,
      process_supervision: probeResult.process_supervision,
    },
    families,
    cases: caseQualifications,
    limitations: [
      "TW-05 frozen historical target 16289f8 executes successfully offline but misses the frozen stale-revision contract; a clearly labeled controlled runtime is the positive for that behavior.",
      "TW-01, TW-02, and TW-06 deterministic execution gates are qualified; their subjective semantic discrimination is deferred to deterministic blinded packet fixtures and is not claimed as machine-qualified here.",
      "Candidate implementations whose public names differ from the frozen adapter contract require a content-addressed trusted adapter configured during the same attempt; the attempt pauses without resampling.",
      "Provider-internal immutable deployment identity and additional independent reviewer panels remain outside this construction qualification.",
    ],
  };
  const payload = deterministicPayload(runReceipt);
  const receipt = {
    payload_sha256: contentSha256(payload),
    payload,
    run_receipt_sha256: contentSha256(runReceipt),
    run_receipt: runReceipt,
    evidence_commitment_sha256: payload.evidence_commitment_sha256,
  };
  await mkdir(dirname(output), {recursive: true});
  await writeFile(output, `${JSON.stringify(receipt, null, 2)}\n`, {mode: 0o600});
  console.log(JSON.stringify({output, payload_sha256: receipt.payload_sha256}));
} finally {
  await rm(qualificationRoot, {recursive: true, force: true});
}
