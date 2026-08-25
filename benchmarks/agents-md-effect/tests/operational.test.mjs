import assert from "node:assert/strict";
import test from "node:test";
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import {
  chmod,
  copyFile,
  cp,
  lstat,
  mkdir,
  mkdtemp,
  readdir,
  readlink,
  readFile,
  realpath,
  rename,
  rm,
  stat,
  symlink,
  unlink,
  writeFile,
} from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { basename, dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath, pathToFileURL } from "node:url";

const testDir = dirname(fileURLToPath(import.meta.url));
const benchmarkDir = resolve(testDir, "..");
const preflightScript = resolve(benchmarkDir, "scripts/preflight-run.mjs");
const captureCandidateScript = resolve(benchmarkDir, "scripts/capture-candidate.mjs");
const prepareValidationScript = resolve(benchmarkDir, "scripts/prepare-validation.mjs");
const runOraclesScript = resolve(benchmarkDir, "scripts/run-oracles.mjs");
const runTw05OfflineScript = resolve(benchmarkDir, "scripts/run-tw05-offline.mjs");
const qualifyOraclesScript = resolve(benchmarkDir, "scripts/qualify-oracles.mjs");
const verifyBenchmarkScript = resolve(benchmarkDir, "scripts/verify-benchmark.mjs");
const verifyOracleQualificationScript = resolve(
  benchmarkDir,
  "scripts/verify-oracle-qualification.mjs",
);
const buildReviewPacketScript = resolve(benchmarkDir, "scripts/build-review-packet.mjs");
const scanReviewPacketScript = resolve(benchmarkDir, "scripts/scan-review-packet.mjs");
const runControllerScript = resolve(benchmarkDir, "scripts/run-controller.mjs");
const captureBaseControlScript = resolve(
  benchmarkDir,
  "scripts/capture-base-control-evidence.mjs",
);
const networkProbeScript = resolve(benchmarkDir, "scripts/probe-network-denial.mjs");
const processGroupSupervisorScript = resolve(
  benchmarkDir,
  "scripts/process-group-supervisor.mjs",
);
const repositoryRoot = resolve(benchmarkDir, "../..");
const trustedCargoPath = spawnSync("rustup", ["which", "cargo"], {encoding: "utf8"}).stdout.trim();
const trustedCargoSha256 = sha256(readFileSync(trustedCargoPath));
const trustedRustcPath = spawnSync("rustup", ["which", "rustc"], {encoding: "utf8"}).stdout.trim();
const trustedRustcSha256 = sha256(readFileSync(trustedRustcPath));
const trustedShellPath = "/bin/bash";
const trustedShellSha256 = sha256(readFileSync(trustedShellPath));
const CONTROL_ARTIFACTS = [
  "environment-lock.json",
  "evaluator/cases.json",
  "evaluator/oracle-lock.json",
  "evaluator/core-score-lock.json",
  "evaluator/authority-lock.json",
  "evaluator/production-oracles.json",
];

async function readJson(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

async function writeOverlayIdentity(workspace, identityFile) {
  const overlayPath = resolve(workspace, "AGENTS.md");
  const [metadata, overlayBytes] = await Promise.all([
    lstat(overlayPath, { bigint: true }),
    readFile(overlayPath),
  ]);
  const identity = {
    schema: "tachiko-agents-overlay-identity-v1",
    path: "AGENTS.md",
    type: "regular",
    device: metadata.dev.toString(),
    inode: metadata.ino.toString(),
    uid: metadata.uid.toString(),
    gid: metadata.gid.toString(),
    mode: Number(metadata.mode & 0o7777n),
    bytes: overlayBytes.length,
    sha256: sha256(overlayBytes),
  };
  const identityBytes = Buffer.from(`${JSON.stringify(identity, null, 2)}\n`, "utf8");
  await writeFile(identityFile, identityBytes);
  return { identity, identityBytes };
}

function git(args, cwd, options = {}) {
  return spawnSync("rtk", ["proxy", "git", ...args], {
    cwd,
    encoding: options.encoding ?? "utf8",
    env: options.env ?? process.env,
    input: options.input,
    maxBuffer: 128 * 1024 * 1024,
  });
}

async function controlSha256(artifactDir) {
  const artifacts = [];
  for (const path of CONTROL_ARTIFACTS) {
    const bytes = await readFile(resolve(artifactDir, path));
    artifacts.push({ path, bytes: bytes.length, sha256: sha256(bytes) });
  }
  return sha256(Buffer.from(`${JSON.stringify(artifacts)}\n`, "utf8"));
}

function exactlyOnce(values, label) {
  assert.equal(new Set(values).size, values.length, `${label} must be unique`);
}

test("shared validator supervisor extinguishes a surviving descendant on one deadline", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-validator-supervisor-"));
  const leader = resolve(fixtureRoot, "leader.mjs");
  const descendantPidPath = resolve(fixtureRoot, "descendant.pid");
  try {
    await writeFile(leader, `
      import {spawn} from "node:child_process";
      import {writeFileSync} from "node:fs";
      const child = spawn(process.execPath, ["-e", "process.on('SIGTERM',()=>{});setInterval(()=>{},1000)"], {
        detached: false,
        stdio: "ignore",
      });
      writeFileSync(process.argv[2], String(child.pid));
      process.on("SIGTERM", () => process.exit(0));
      setInterval(() => {}, 1000);
    `);
    const {runProcessGroupOnce} = await import(pathToFileURL(processGroupSupervisorScript));
    const result = await runProcessGroupOnce({
      executable: process.execPath,
      args: [leader, descendantPidPath],
      cwd: fixtureRoot,
      environment: process.env,
      timeoutMilliseconds: 100,
      terminationGraceMilliseconds: 250,
    });
    const descendantPid = Number(await readFile(descendantPidPath, "utf8"));
    assert.equal(result.timed_out, true);
    assert.equal(result.termination_grace_intervals, 1);
    assert.equal(result.termination_deadline_reused_for_cleanup, true);
    assert.equal(result.termination_signal_sent, true);
    assert.equal(result.kill_signal_sent, true);
    assert.deepEqual(result.signal_actions.map((entry) => entry.signal), ["SIGTERM", "SIGKILL"]);
    assert.ok(result.signal_actions.every((entry) => !Number.isNaN(Date.parse(entry.sent_at))));
    assert.equal(result.descendant_cleanup_required, true);
    assert.equal(result.process_group_extinct_before_capture, true);
    assert.throws(() => process.kill(descendantPid, 0), /ESRCH/);
  } finally {
    await rm(fixtureRoot, {recursive: true, force: true});
  }
});

test("same-wave base command execution denies a real socket attempt in the kernel sandbox", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-base-network-"));
  const workspace = resolve(fixtureRoot, "base-workspace");
  try {
    const frozenCase = (await readJson(resolve(benchmarkDir, "evaluator/cases.json"))).cases
      .find((entry) => entry.id === "TW-01");
    const {prepareBaseWorkspace} = await import(pathToFileURL(runControllerScript));
    await prepareBaseWorkspace(
      await realpath(repositoryRoot), frozenCase.historical_base_commit,
      frozenCase.historical_base_tree, frozenCase.ground_truth_commit, workspace,
      resolve(fixtureRoot, "base-preparation"), process.env,
    );
    const binDir = resolve(fixtureRoot, "bin");
    await mkdir(binDir);
    await writeFile(
      resolve(binDir, "bash"),
      `#!/bin/bash\n${JSON.stringify(process.execPath)} ${JSON.stringify(networkProbeScript)}\n` +
        `exec /bin/bash "$@"\n`,
      {mode: 0o755},
    );
    const fakeBash = resolve(binDir, "bash");
    const fakeBashBytes = await readFile(fakeBash);
    const environmentReceipt = resolve(fixtureRoot, "environment.json");
    await writeFile(environmentReceipt, `${JSON.stringify({
      case_id: "TW-01", control_sha256: "a".repeat(64),
      wave_id: "1".repeat(32), run_id: "2".repeat(32), attempt_id: "3".repeat(32),
      candidate_id: "4".repeat(32), environment_identity_sha256: "b".repeat(64),
      tools: [{name: "bash", path: fakeBash, bytes: fakeBashBytes.length,
        sha256: sha256(fakeBashBytes)}],
    })}\n`);
    const receiptPath = resolve(fixtureRoot, "base-receipt.json");
    const result = spawnSync(process.execPath, [
      captureBaseControlScript, "--case", "TW-01", "--workspace", workspace,
      "--receipt", receiptPath, "--log-dir", resolve(fixtureRoot, "logs"),
      "--controller-bound", "true", "--phase", "construction_pilot_only",
      "--wave-id", "1".repeat(32), "--run-id", "2".repeat(32),
      "--attempt-id", "3".repeat(32), "--candidate-id", "4".repeat(32),
      "--control-sha256", "a".repeat(64), "--environment-receipt", environmentReceipt,
      "--trusted-shell", fakeBash, "--expected-shell-sha256", sha256(fakeBashBytes),
    ], {
      encoding: "utf8", timeout: 120_000, maxBuffer: 128 * 1024 * 1024,
      env: {...process.env, PATH: `${binDir}:${process.env.PATH}`},
    });
    const receipt = await readJson(receiptPath);
    assert.equal(result.status, 1, `${result.stderr}\n${result.stdout}\n${JSON.stringify(receipt)}`);
    assert.equal(receipt.classification, "same_wave_construction_control");
    assert.equal(receipt.network_enforcement.probe_denied, true);
    assert.equal(receipt.commands[0].network_sandbox.mode, "darwin_sandbox_deny_network");
    assert.match(
      await readFile(receipt.commands[0].stdout.path, "utf8"),
      /network-denied:(?:EPERM|EACCES)/,
    );
  } finally {
    await rm(fixtureRoot, {recursive: true, force: true});
  }
});

test("candidate core validation denies a real socket attempt in the kernel sandbox", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-core-network-"));
  try {
    const controllerRoot = resolve(fixtureRoot, "controller");
    const candidateRoot = resolve(fixtureRoot, "candidate");
    await Promise.all([mkdir(resolve(controllerRoot, "evaluator"), {recursive: true}), mkdir(candidateRoot)]);
    await writeFile(resolve(controllerRoot, "evaluator/core-score-lock.json"), `${JSON.stringify({
      cases: [{id: "TW-XX", validation_checks: [{
        id: "core.network", command: `${JSON.stringify(process.execPath)} ${JSON.stringify(networkProbeScript)}`,
      }]}],
    })}\n`);
    const {runCoreValidation} = await import(pathToFileURL(runControllerScript));
    const result = await runCoreValidation(
      "TW-XX", candidateRoot, resolve(fixtureRoot, "output"), process.env,
      false, {}, controllerRoot, "/bin/bash",
    );
    assert.equal(result.receipt.all_commands_passed, true);
    assert.equal(result.receipt.network_enforcement.probe_denied, true);
    assert.equal(result.receipt.commands[0].network_sandbox.mode, "darwin_sandbox_deny_network");
    assert.match(await readFile(result.receipt.commands[0].stdout.path, "utf8"), /network-denied/);
  } finally {
    await rm(fixtureRoot, {recursive: true, force: true});
  }
});

function ids(entries) {
  return entries.map((entry) => entry.id).sort();
}

function assertNoFrozenScoringCopies(value, path = "production-oracles.json") {
  if (!value || typeof value !== "object") return;
  assert.equal(Object.hasOwn(value, "points"), false, `${path} must not copy points`);
  assert.equal(Object.hasOwn(value, "selector"), false, `${path} must not copy selectors`);
  for (const [key, child] of Object.entries(value)) {
    assertNoFrozenScoringCopies(child, `${path}.${key}`);
  }
}

test("production oracle manifest covers every frozen operational input exactly once", async () => {
  const [cases, oracleLock, coreScoreLock, productionOracles] = await Promise.all([
    readJson(resolve(benchmarkDir, "evaluator/cases.json")),
    readJson(resolve(benchmarkDir, "evaluator/oracle-lock.json")),
    readJson(resolve(benchmarkDir, "evaluator/core-score-lock.json")),
    readJson(resolve(benchmarkDir, "evaluator/production-oracles.json")),
  ]);

  assert.equal(productionOracles.protocol_id, cases.protocol_id);
  assert.equal(productionOracles.classification, "construction_pilot_only");
  assert.equal(productionOracles.formal_result_eligible, false);
  assert.equal(productionOracles.execution_standard, "practical_internal_v1");
  assert.equal(
    productionOracles.qualification_requirement,
    "construction_pilot_only_qualification_required",
  );
  assert.equal(
    productionOracles.node_test_entry_point,
    "node --test benchmarks/agents-md-effect/tests/operational.test.mjs",
  );
  assert.equal(productionOracles.cases.length, 9);
  exactlyOnce(ids(productionOracles.cases), "production case IDs");
  assert.deepEqual(ids(productionOracles.cases), ids(cases.cases));
  assertNoFrozenScoringCopies(productionOracles);

  for (const caseEntry of cases.cases) {
    const oracleCase = oracleLock.cases.find((entry) => entry.id === caseEntry.id);
    const coreScoreCase = coreScoreLock.cases.find((entry) => entry.id === caseEntry.id);
    const productionCase = productionOracles.cases.find((entry) => entry.id === caseEntry.id);

    assert.ok(oracleCase, `${caseEntry.id} must have an oracle lock entry`);
    assert.ok(coreScoreCase, `${caseEntry.id} must have a core score entry`);
    assert.ok(productionCase, `${caseEntry.id} must have a production entry`);

    assert.deepEqual(
      ids(productionCase.core_commands),
      ids(coreScoreCase.validation_checks),
      `${caseEntry.id} core commands must map exactly once`,
    );
    assert.ok(
      productionCase.core_commands.every(
        (entry) => entry.stage === "candidate_core_validation",
      ),
      `${caseEntry.id} core commands must execute in candidate validation`,
    );
    for (const command of productionCase.core_commands) {
      const lockedCommand = coreScoreCase.validation_checks.find(
        (entry) => entry.id === command.id,
      );
      assert.equal(command.command_template, lockedCommand.command);
    }

    assert.deepEqual(
      ids(productionCase.oracle_commands),
      ids(oracleCase.command_specs),
      `${caseEntry.id} oracle commands must map exactly once`,
    );
    for (const command of productionCase.oracle_commands) {
      const lockedCommand = oracleCase.command_specs.find(
        (entry) => entry.id === command.id,
      );
      assert.equal(command.command_template, lockedCommand.run);
      assert.equal(command.stage, "isolated_oracle_pipeline");
      assert.deepEqual(
        command.stages,
        [
          "candidate_artifact_build",
          "trusted_probe_build",
          "expectation_free_execution",
        ],
        `${caseEntry.id} ${command.id} must use the isolated production stages`,
      );
      assert.deepEqual(
        [...command.assertion_ids].sort(),
        oracleCase.assertions
          .filter((assertion) => assertion.command_id === command.id)
          .map((assertion) => assertion.id)
          .sort(),
        `${caseEntry.id} ${command.id} must identify every assertion it feeds`,
      );
    }

    assert.deepEqual(
      ids(productionCase.assertions),
      ids(oracleCase.assertions),
      `${caseEntry.id} assertions must map exactly once`,
    );
    for (const assertion of oracleCase.assertions) {
      const mapping = productionCase.assertions.find((entry) => entry.id === assertion.id);
      assert.equal(mapping.command_id, assertion.command_id);
      assert.equal(mapping.stage, "expectation_free_execution");
    }

    const subjectiveGroupIds = caseEntry.validation.machine_contract_groups
      .filter((group) => group.assessment === "blinded_semantic_review")
      .map((group) => group.id)
      .sort();
    assert.deepEqual(
      ids(productionCase.subjective_groups),
      subjectiveGroupIds,
      `${caseEntry.id} subjective groups must map exactly once`,
    );
    assert.ok(
      productionCase.subjective_groups.every(
        (entry) => entry.stage === "blinded_review_packet",
      ),
      `${caseEntry.id} subjective groups must enter blinded review`,
    );
  }
});

async function createPreflightFixture(
  { runRoot = "r-0123456789abcdef0123456789abcdef", parentName } = {},
) {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-preflight-"));
  const root = resolve(fixtureRoot, parentName ?? "", runRoot);
  const workspace = resolve(root, "workspace");
  const home = resolve(root, "home");
  const codexHome = resolve(root, "codex-home");
  const tmpDir = resolve(root, "tmp");
  const cargoHome = resolve(root, "cargo-home");
  const artifactDir = resolve(fixtureRoot, "controls");
  await Promise.all([
    mkdir(workspace, { recursive: true }),
    mkdir(home, { recursive: true }),
    mkdir(codexHome, { recursive: true }),
    mkdir(tmpDir, { recursive: true }),
    mkdir(cargoHome, { recursive: true }),
    cp(benchmarkDir, artifactDir, { recursive: true }),
  ]);
  const agents = resolve(workspace, "AGENTS.md");
  await writeFile(agents, "workspace instruction\n");
  await mkdir(resolve(artifactDir, "receipts"));
  return {
    fixtureRoot,
    workspace,
    home,
    codexHome,
    tmpDir,
    cargoHome,
    artifactDir,
    receipt: resolve(artifactDir, "receipts", "receipt.json"),
    expectedAgentsSha256: sha256(await readFile(agents)),
    expectedControlSha256: await controlSha256(artifactDir),
  };
}

function runPreflight(
  fixture,
  {
    environment = {},
    includeExpectedAgents = true,
    includeExpectedControl = true,
    receipt,
    nodeExecutable = process.execPath,
  } = {},
) {
  const argumentsForPreflight = [
    preflightScript,
    "--workspace",
    fixture.workspace,
    "--home",
    fixture.home,
    "--codex-home",
    fixture.codexHome,
    "--artifact-dir",
    fixture.artifactDir,
    "--receipt",
    receipt ?? fixture.receipt,
  ];
  if (includeExpectedAgents) {
    argumentsForPreflight.push("--expected-agents-sha256", fixture.expectedAgentsSha256);
  }
  if (includeExpectedControl) {
    argumentsForPreflight.push("--expected-control-sha256", fixture.expectedControlSha256);
  }
  const childEnvironment = {
    ...process.env,
    HOME: fixture.home,
    CODEX_HOME: fixture.codexHome,
    TMPDIR: fixture.tmpDir,
    LANG: "C.UTF-8",
    LC_ALL: "C.UTF-8",
    TZ: "UTC",
    CARGO_INCREMENTAL: "0",
    CARGO_NET_OFFLINE: "true",
    CARGO_HOME: fixture.cargoHome,
    RUSTUP_HOME: process.env.RUSTUP_HOME ?? resolve(process.env.HOME, ".rustup"),
    PNPM_HOME: process.env.PNPM_HOME ?? dirname(process.execPath),
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_CONFIG_GLOBAL: "/dev/null",
    GIT_ATTR_NOSYSTEM: "1",
    ...environment,
  };
  delete childEnvironment.CARGO_TARGET_DIR;
  return spawnSync(
    nodeExecutable,
    argumentsForPreflight,
    {
      encoding: "utf8",
      env: childEnvironment,
    },
  );
}

function executablePath(name) {
  const result = spawnSync("/usr/bin/which", [name], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  return result.stdout.trim();
}

async function installEmptyWasmTargetFixture(fixture) {
  const toolDir = resolve(fixture.fixtureRoot, "tool-bin");
  const emptyTarget = resolve(fixture.fixtureRoot, "empty-wasm-target");
  const actualRustup = executablePath("rustup");
  const actualRustc = spawnSync(actualRustup, ["which", "rustc"], { encoding: "utf8" });
  assert.equal(actualRustc.status, 0, actualRustc.stderr);
  const fakeRustc = resolve(toolDir, "rustc");
  const fakeRustup = resolve(toolDir, "rustup");
  await Promise.all([mkdir(toolDir), mkdir(emptyTarget)]);
  await writeFile(
    fakeRustc,
    `#!/bin/sh\nif [ "$1" = "--print" ] && [ "$2" = "target-libdir" ] && ` +
      `[ "$3" = "--target" ] && [ "$4" = "wasm32-unknown-unknown" ]; then\n` +
      `  printf '%s\\n' ${JSON.stringify(emptyTarget)}\n  exit 0\nfi\n` +
      `exec ${JSON.stringify(actualRustc.stdout.trim())} "$@"\n`,
    { mode: 0o755 },
  );
  await writeFile(
    fakeRustup,
    `#!/bin/sh\nif [ "$1" = "which" ] && [ "$2" = "rustc" ]; then\n` +
      `  printf '%s\\n' ${JSON.stringify(fakeRustc)}\n  exit 0\nfi\n` +
      `exec ${JSON.stringify(actualRustup)} "$@"\n`,
    { mode: 0o755 },
  );
  return { PATH: `${toolDir}:${process.env.PATH}` };
}

async function withPreflightFixture(options, body) {
  const fixture = await createPreflightFixture(options);
  try {
    await body(fixture);
  } finally {
    await rm(fixture.fixtureRoot, { recursive: true, force: true });
  }
}

test("preflight accepts an empty neutral HOME and CODEX_HOME and records real observations", async () => {
  await withPreflightFixture({}, async (fixture) => {
    const result = runPreflight(fixture);
    assert.equal(result.status, 0, `${result.stderr}\n${result.stdout}`);

    const receipt = await readJson(fixture.receipt);
    assert.equal(receipt.valid, true);
    assert.equal(receipt.paths.workspace, await realpath(fixture.workspace));
    assert.deepEqual(receipt.scans.home.entries, []);
    assert.deepEqual(receipt.scans.codex_home.entries, []);
    assert.ok(receipt.binaries.node.sha256);
    for (const name of [
      "node",
      "bash",
      "git",
      "rtk",
      "rustup",
      "cargo",
      "rustc",
      "rustfmt",
      "clippy",
    ]) {
      assert.match(receipt.binaries[name].path, /^\//);
      assert.match(receipt.binaries[name].sha256, /^[0-9a-f]{64}$/);
      assert.ok(receipt.binaries[name].bytes > 0);
      assert.notEqual(receipt.binaries[name].version, "");
    }
    assert.equal(receipt.rust_target.target, "wasm32-unknown-unknown");
    assert.match(receipt.rust_target.sha256, /^[0-9a-f]{64}$/);
    assert.ok(receipt.rust_target.file_bytes > 0);
    assert.ok(receipt.rust_target.regular_files > 0);
    assert.deepEqual(receipt.environment, {
      HOME: fixture.home,
      CODEX_HOME: fixture.codexHome,
      TMPDIR: fixture.tmpDir,
      PATH: process.env.PATH,
      LANG: "C.UTF-8",
      LC_ALL: "C.UTF-8",
      TZ: "UTC",
      CARGO_INCREMENTAL: "0",
      CARGO_NET_OFFLINE: "true",
      CARGO_HOME: fixture.cargoHome,
      RUSTUP_HOME: process.env.RUSTUP_HOME ?? resolve(process.env.HOME, ".rustup"),
      PNPM_HOME: process.env.PNPM_HOME ?? dirname(process.execPath),
      GIT_CONFIG_NOSYSTEM: "1",
      GIT_CONFIG_GLOBAL: "/dev/null",
      GIT_ATTR_NOSYSTEM: "1",
      expected_agents_sha256: fixture.expectedAgentsSha256,
      expected_control_sha256: fixture.expectedControlSha256,
    });
    assert.ok(receipt.free_space.bytes > 0);
    assert.ok(receipt.controls.artifacts.some((entry) => entry.path === "environment-lock.json"));
    assert.ok(
      receipt.controls.artifacts.some(
        (entry) => entry.path === "evaluator/production-oracles.json",
      ),
    );
  });
});

test("preflight rejects an instruction file in a workspace ancestor", async () => {
  await withPreflightFixture({}, async (fixture) => {
    await writeFile(resolve(dirname(fixture.workspace), "AGENTS.md"), "leaked instruction\n");
    const result = runPreflight(fixture);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /workspace ancestor instruction exposure/i);
  });
});

test("preflight rejects a root AGENTS.md whose bytes differ from the controller hash", async () => {
  await withPreflightFixture({}, async (fixture) => {
    await writeFile(resolve(fixture.workspace, "AGENTS.md"), "altered instruction\n");
    const result = runPreflight(fixture);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /AGENTS.md SHA-256 mismatch/i);
  });
});

test("preflight rejects a symlinked root AGENTS.md", async () => {
  await withPreflightFixture({}, async (fixture) => {
    const agents = resolve(fixture.workspace, "AGENTS.md");
    const replacement = resolve(fixture.fixtureRoot, "replacement-agents.md");
    await writeFile(replacement, "workspace instruction\n");
    await unlink(agents);
    await symlink(replacement, agents);
    const result = runPreflight(fixture);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /root AGENTS.md must be a regular non-symlink file/i);
  });
});

test("preflight rejects nested instruction files", async () => {
  await withPreflightFixture({}, async (fixture) => {
    const nested = resolve(fixture.workspace, "nested");
    await mkdir(nested);
    await Promise.all(
      ["AGENTS.md", "CLAUDE.md", "GEMINI.md"].map((name) =>
        writeFile(resolve(nested, name), "leaked instruction\n"),
      ),
    );
    const result = runPreflight(fixture);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /nested workspace instruction exposure/i);
  });
});

test("preflight rejects a workspace symlinked directory that hides instructions", async () => {
  await withPreflightFixture({}, async (fixture) => {
    const outside = resolve(fixture.fixtureRoot, "outside-instructions");
    await mkdir(outside);
    await writeFile(resolve(outside, "AGENTS.md"), "hidden instruction\n");
    await symlink(outside, resolve(fixture.workspace, "benign-directory"));
    const result = runPreflight(fixture);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /workspace contains a symlinked directory/i);
  });
});

test("preflight rejects skills exposed through neutral HOME", async () => {
  await withPreflightFixture({}, async (fixture) => {
    await mkdir(resolve(fixture.home, ".codex", "skills"), { recursive: true });
    const result = runPreflight(fixture);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /neutral HOME must be empty/i);
  });
});

test("preflight rejects unexpected neutral HOME content", async () => {
  await withPreflightFixture({}, async (fixture) => {
    await writeFile(resolve(fixture.home, ".profile"), "unexpected\n");
    const result = runPreflight(fixture);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /neutral HOME must be empty/i);
  });
});

test("preflight rejects semantic-label bypasses in the run-root basename", async () => {
  await withPreflightFixture({ runRoot: "variantb" }, async (fixture) => {
    const result = runPreflight(fixture);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /run root must use the opaque r-<32-lowercase-hex> name grammar/i);
  });
});

test("preflight rejects an empty WASM target artifact directory", async () => {
  await withPreflightFixture({}, async (fixture) => {
    const result = runPreflight(fixture, {
      environment: await installEmptyWasmTargetFixture(fixture),
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /wasm32-unknown-unknown target has no regular artifacts/i);
  });
});

test("preflight rejects a controlled environment mismatch", async () => {
  await withPreflightFixture({}, async (fixture) => {
    const result = runPreflight(fixture, { environment: { CARGO_NET_OFFLINE: "false" } });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /CARGO_NET_OFFLINE must equal true/i);
  });
});

test("preflight ignores semantic-looking names in run-root ancestors", async () => {
  await withPreflightFixture({ parentName: "benchmark-archive" }, async (fixture) => {
    const result = runPreflight(fixture);
    assert.equal(result.status, 0, result.stderr);
  });
});

test("preflight requires the controller control digest", async () => {
  await withPreflightFixture({}, async (fixture) => {
    const result = runPreflight(fixture, { includeExpectedControl: false });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /--expected-control-sha256 is required/i);
  });
});

test("preflight fails closed when a registered control digest changes", async () => {
  await withPreflightFixture({}, async (fixture) => {
    const environmentLock = resolve(fixture.artifactDir, "environment-lock.json");
    await writeFile(environmentLock, `${await readFile(environmentLock, "utf8")} `);
    const result = runPreflight(fixture, {
      receipt: resolve(fixture.artifactDir, "receipts", "mutated-receipt.json"),
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /control SHA-256 mismatch/i);
  });
});

test("preflight rejects a receipt path that escapes trusted artifacts through a symlink", async () => {
  await withPreflightFixture({}, async (fixture) => {
    const outside = resolve(fixture.fixtureRoot, "outside");
    const escapedParent = resolve(fixture.artifactDir, "receipts", "escape");
    await mkdir(outside);
    await symlink(outside, escapedParent);
    const result = runPreflight(fixture, {
      receipt: resolve(escapedParent, "receipt.json"),
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /receipt must remain inside the trusted artifact directory/i);
  });
});

test("trusted capture preserves adversarial raw workspace state byte-for-byte", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-raw-capture-"));
  const sourceRepo = await realpath(resolve(benchmarkDir, "../.."));
  const workspace = resolve(fixtureRoot, "candidate-workspace");
  const captureDir = resolve(fixtureRoot, "capture");
  const validationWorkspace = resolve(fixtureRoot, "validation-workspace");
  const validationDir = resolve(fixtureRoot, "validation");
  const exclusionsFile = resolve(fixtureRoot, "capture-exclusions.json");
  const overlayIdentityFile = resolve(fixtureRoot, "overlay-identity.json");
  const cases = await readJson(resolve(benchmarkDir, "evaluator/cases.json"));
  const caseEntry = cases.cases.find((entry) => entry.id === "TW-01");

  try {
    let result = git(["clone", "--no-local", sourceRepo, workspace], fixtureRoot);
    assert.equal(result.status, 0, result.stderr);
    result = git(["checkout", "--detach", caseEntry.historical_base_commit], workspace);
    assert.equal(result.status, 0, result.stderr);

    const overlayBytes = Buffer.from("trusted task overlay\n", "utf8");
    const assumeBytes = Buffer.from("assume-unchanged raw bytes\n", "utf8");
    const skipBytes = Buffer.from("skip-worktree raw bytes\n", "utf8");
    const ignoredBytes = Buffer.from([0x69, 0x67, 0x6e, 0x6f, 0x72, 0x65, 0x64, 0x00, 0xff]);
    const untrackedBytes = Buffer.from("ordinary untracked bytes\n", "utf8");
    const stagedBytes = Buffer.from("staged raw bytes\n", "utf8");
    const hostileBytes = Buffer.from("raw hostile-filter bytes\r\n", "utf8");
    const hostileEolBytes = Buffer.from("must remain LF\n", "utf8");
    const binaryBytes = Buffer.from([0x00, 0xff, 0x80, 0x0a, 0x0d, 0x41]);
    const symlinkTarget = "../raw-target-without-resolution";
    const filterMarker = resolve(fixtureRoot, "clean-filter-ran");
    const hookMarker = resolve(fixtureRoot, "candidate-hook-ran");
    const filterScript = resolve(workspace, ".git", "evil-clean.sh");
    const hookScript = resolve(workspace, ".git", "hooks", "pre-commit");

    await Promise.all([
      writeFile(resolve(workspace, "AGENTS.md"), overlayBytes),
      writeFile(resolve(workspace, "README.md"), assumeBytes),
      writeFile(resolve(workspace, "CONTRIBUTING.md"), skipBytes),
      writeFile(resolve(workspace, "ignored-only.bin"), ignoredBytes),
      writeFile(resolve(workspace, "ordinary-untracked.txt"), untrackedBytes),
      writeFile(resolve(workspace, "staged-change.txt"), stagedBytes),
      writeFile(
        resolve(workspace, ".gitattributes"),
        "*.hostile filter=evil\n*.crlf text eol=crlf\n",
      ),
      writeFile(resolve(workspace, "payload.hostile"), hostileBytes),
      writeFile(resolve(workspace, "line-endings.crlf"), hostileEolBytes),
      writeFile(resolve(workspace, "binary.dat"), binaryBytes),
      writeFile(exclusionsFile, '["target/"]\n'),
      mkdir(resolve(workspace, "target")),
    ]);
    const overlayIdentity = await writeOverlayIdentity(workspace, overlayIdentityFile);
    await Promise.all([
      writeFile(resolve(workspace, "target", "excluded.cache"), "excluded\n"),
      symlink(symlinkTarget, resolve(workspace, "raw-link")),
      chmod(resolve(workspace, "LICENSE-MIT"), 0o755),
      writeFile(
        filterScript,
        `#!/bin/sh\nprintf invoked > ${JSON.stringify(filterMarker)}\nsed 's/raw/FILTERED/g'\n`,
        { mode: 0o755 },
      ),
      writeFile(
        hookScript,
        `#!/bin/sh\nprintf invoked > ${JSON.stringify(hookMarker)}\n`,
        { mode: 0o755 },
      ),
    ]);
    result = git(["add", "staged-change.txt"], workspace);
    assert.equal(result.status, 0, result.stderr);
    for (const command of [
      ["update-index", "--assume-unchanged", "README.md"],
      ["update-index", "--skip-worktree", "CONTRIBUTING.md"],
      ["config", "filter.evil.clean", filterScript],
      ["config", "filter.evil.required", "true"],
    ]) {
      result = git(command, workspace);
      assert.equal(result.status, 0, result.stderr);
    }
    await writeFile(resolve(workspace, ".git", "info", "exclude"), "ignored-only.bin\n");

    const legacyIndex = resolve(fixtureRoot, "legacy.index");
    await copyFile(resolve(workspace, ".git", "index"), legacyIndex);
    const legacyEnvironment = { ...process.env, GIT_INDEX_FILE: legacyIndex };
    result = git(["add", "-A"], workspace, { env: legacyEnvironment });
    assert.equal(result.status, 0, result.stderr);
    assert.equal((await readFile(filterMarker, "utf8")).trim(), "invoked");
    result = git(["ls-files", "--error-unmatch", "ignored-only.bin"], workspace, {
      env: legacyEnvironment,
    });
    assert.notEqual(result.status, 0, "the legacy index capture must miss ignored files");
    result = git(["write-tree"], workspace, { env: legacyEnvironment });
    assert.equal(result.status, 0, result.stderr);
    const legacyTree = result.stdout.trim();
    result = git(["show", `${legacyTree}:payload.hostile`], workspace, {
      env: legacyEnvironment,
      encoding: null,
    });
    assert.equal(result.status, 0, Buffer.from(result.stderr ?? []).toString("utf8"));
    assert.deepEqual(Buffer.from(result.stdout), Buffer.from("FILTERED hostile-filter bytes\r\n"));
    await rm(filterMarker);

    result = spawnSync(
      process.execPath,
      [
        captureCandidateScript,
        "--case",
        "TW-01",
        "--workspace",
        workspace,
        "--source-repo",
        sourceRepo,
        "--exclusions-file",
        exclusionsFile,
        "--expected-agents-identity-file",
        overlayIdentityFile,
        "--trusted-dir",
        captureDir,
        "--expected-agents-sha256",
        sha256(overlayBytes),
      ],
      { encoding: "utf8", maxBuffer: 128 * 1024 * 1024 },
    );
    assert.equal(result.status, 0, result.stderr);
    assert.equal(existsSync(filterMarker), false, "capture must not run candidate clean filters");
    assert.equal(existsSync(hookMarker), false, "capture must not run candidate hooks");

    const captureReceiptPath = resolve(captureDir, "capture-receipt.json");
    const captureReceipt = await readJson(captureReceiptPath);
    assert.equal(captureReceipt.trusted_raw_capture, true);
    assert.equal(captureReceipt.source_repo.path, sourceRepo);
    assert.equal(captureReceipt.overlay.type, "regular");
    assert.equal(captureReceipt.overlay.sha256, sha256(overlayBytes));
    assert.equal(captureReceipt.overlay_identity_equal, true);
    assert.deepEqual(captureReceipt.overlay_pre_run.expected, overlayIdentity.identity);
    assert.equal(
      captureReceipt.overlay_pre_run.file_sha256,
      sha256(overlayIdentity.identityBytes),
    );
    assert.equal(captureReceipt.exclusions.file_sha256, sha256(Buffer.from('["target/"]\n')));
    assert.deepEqual(captureReceipt.exclusions.paths, ["target"]);
    assert.match(captureReceipt.raw_tree_digest_sha256, /^[0-9a-f]{64}$/);
    assert.equal(captureReceipt.round_trip.equal, true);
    assert.equal(
      captureReceipt.round_trip.digest_sha256,
      captureReceipt.raw_tree_digest_sha256,
    );
    assert.match(captureReceipt.trusted_index.sha256, /^[0-9a-f]{64}$/);
    assert.match(captureReceipt.candidate_commit, /^[0-9a-f]{40}$/);
    assert.match(captureReceipt.candidate_tree, /^[0-9a-f]{40}$/);
    for (const path of [
      "README.md",
      "CONTRIBUTING.md",
      "ignored-only.bin",
      "ordinary-untracked.txt",
      "staged-change.txt",
      ".gitattributes",
      "payload.hostile",
      "line-endings.crlf",
      "binary.dat",
      "raw-link",
      "LICENSE-MIT",
    ]) {
      assert.ok(captureReceipt.changed_files.includes(path), `${path} must be captured`);
    }
    assert.equal(captureReceipt.changed_files.includes("AGENTS.md"), false);
    assert.equal(captureReceipt.changed_files.some((path) => path.startsWith("target/")), false);

    result = spawnSync(
      process.execPath,
      [
        prepareValidationScript,
        "--case",
        "TW-01",
        "--source-repo",
        sourceRepo,
        "--patch-file",
        resolve(captureDir, "candidate.patch"),
        "--capture-receipt",
        captureReceiptPath,
        "--workspace",
        validationWorkspace,
        "--trusted-dir",
        validationDir,
      ],
      { encoding: "utf8", maxBuffer: 128 * 1024 * 1024 },
    );
    assert.equal(result.status, 0, result.stderr);
    assert.equal(
      existsSync(filterMarker),
      false,
      "validation preparation must not run candidate clean filters",
    );
    assert.equal(
      existsSync(hookMarker),
      false,
      "validation preparation must not run candidate hooks",
    );

    assert.deepEqual(await readFile(resolve(validationWorkspace, "README.md")), assumeBytes);
    assert.deepEqual(
      await readFile(resolve(validationWorkspace, "CONTRIBUTING.md")),
      skipBytes,
    );
    assert.deepEqual(
      await readFile(resolve(validationWorkspace, "ignored-only.bin")),
      ignoredBytes,
    );
    assert.deepEqual(
      await readFile(resolve(validationWorkspace, "ordinary-untracked.txt")),
      untrackedBytes,
    );
    assert.deepEqual(
      await readFile(resolve(validationWorkspace, "staged-change.txt")),
      stagedBytes,
    );
    assert.deepEqual(await readFile(resolve(validationWorkspace, "payload.hostile")), hostileBytes);
    assert.deepEqual(
      await readFile(resolve(validationWorkspace, "line-endings.crlf")),
      hostileEolBytes,
    );
    assert.deepEqual(await readFile(resolve(validationWorkspace, "binary.dat")), binaryBytes);
    assert.equal(await readlink(resolve(validationWorkspace, "raw-link")), symlinkTarget);
    assert.notEqual((await lstat(resolve(validationWorkspace, "LICENSE-MIT"))).mode & 0o111, 0);
    assert.equal(existsSync(resolve(validationWorkspace, "AGENTS.md")), false);
    assert.equal(existsSync(resolve(validationWorkspace, "target")), false);

    const validationReceipt = await readJson(
      resolve(validationDir, "validation-preparation-receipt.json"),
    );
    assert.equal(validationReceipt.capture_receipt_verified, true);
    assert.equal(validationReceipt.capture_to_apply_tree_equal, true);
    assert.equal(
      validationReceipt.raw_tree_digest_sha256,
      captureReceipt.raw_tree_digest_sha256,
    );

    const captureRedirect = resolve(fixtureRoot, "capture-redirect");
    await symlink(captureDir, captureRedirect);
    result = spawnSync(
      process.execPath,
      [
        prepareValidationScript,
        "--case",
        "TW-01",
        "--source-repo",
        sourceRepo,
        "--patch-file",
        resolve(captureDir, "candidate.patch"),
        "--capture-receipt",
        captureReceiptPath,
        "--workspace",
        resolve(captureRedirect, "redirected-validation"),
        "--trusted-dir",
        resolve(fixtureRoot, "redirected-validation-trusted"),
      ],
      { encoding: "utf8", maxBuffer: 128 * 1024 * 1024 },
    );
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /capture artifacts|disjoint/i);

    result = spawnSync(
      process.execPath,
      [
        prepareValidationScript,
        "--case",
        "TW-01",
        "--source-repo",
        sourceRepo,
        "--patch-file",
        resolve(captureDir, "candidate.patch"),
        "--capture-receipt",
        captureReceiptPath,
        "--workspace",
        resolve(fixtureRoot, "unused-validation-workspace"),
        "--trusted-dir",
        resolve(captureRedirect, "redirected-trusted-output"),
      ],
      { encoding: "utf8", maxBuffer: 128 * 1024 * 1024 },
    );
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /capture artifacts|disjoint/i);
  } finally {
    await rm(fixtureRoot, { recursive: true, force: true });
  }
});

test("trusted capture rejects unsupported filesystem nodes", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-raw-node-"));
  const sourceRepo = await realpath(resolve(benchmarkDir, "../.."));
  const workspace = resolve(fixtureRoot, "workspace");
  const exclusionsFile = resolve(fixtureRoot, "exclusions.json");
  const overlayIdentityFile = resolve(fixtureRoot, "overlay-identity.json");
  const overlayBytes = Buffer.from("trusted task overlay\n", "utf8");
  try {
    await mkdir(workspace);
    await Promise.all([
      writeFile(resolve(workspace, "AGENTS.md"), overlayBytes),
      writeFile(exclusionsFile, "[]\n"),
    ]);
    await writeOverlayIdentity(workspace, overlayIdentityFile);
    const fifo = spawnSync(executablePath("mkfifo"), [resolve(workspace, "candidate.fifo")], {
      encoding: "utf8",
    });
    assert.equal(fifo.status, 0, fifo.stderr);
    const result = spawnSync(
      process.execPath,
      [
        captureCandidateScript,
        "--case",
        "TW-01",
        "--workspace",
        workspace,
        "--source-repo",
        sourceRepo,
        "--exclusions-file",
        exclusionsFile,
        "--expected-agents-identity-file",
        overlayIdentityFile,
        "--trusted-dir",
        resolve(fixtureRoot, "capture"),
        "--expected-agents-sha256",
        sha256(overlayBytes),
      ],
      { encoding: "utf8" },
    );
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /unsupported filesystem node/i);
  } finally {
    await rm(fixtureRoot, { recursive: true, force: true });
  }
});

test("trusted capture rejects escaping exclusion paths", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-raw-exclusion-"));
  const sourceRepo = await realpath(resolve(benchmarkDir, "../.."));
  const workspace = resolve(fixtureRoot, "workspace");
  const exclusionsFile = resolve(fixtureRoot, "exclusions.json");
  const overlayIdentityFile = resolve(fixtureRoot, "overlay-identity.json");
  const overlayBytes = Buffer.from("trusted task overlay\n", "utf8");
  try {
    await mkdir(workspace);
    await Promise.all([
      writeFile(resolve(workspace, "AGENTS.md"), overlayBytes),
      writeFile(exclusionsFile, '["../escape"]\n'),
    ]);
    await writeOverlayIdentity(workspace, overlayIdentityFile);
    const result = spawnSync(
      process.execPath,
      [
        captureCandidateScript,
        "--case",
        "TW-01",
        "--workspace",
        workspace,
        "--source-repo",
        sourceRepo,
        "--exclusions-file",
        exclusionsFile,
        "--expected-agents-identity-file",
        overlayIdentityFile,
        "--trusted-dir",
        resolve(fixtureRoot, "capture"),
        "--expected-agents-sha256",
        sha256(overlayBytes),
      ],
      { encoding: "utf8" },
    );
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /invalid non-normalized exclusion path/i);
  } finally {
    await rm(fixtureRoot, { recursive: true, force: true });
  }
});

test("trusted capture rejects unsafe exclusion spellings consistently", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-invalid-exclusions-"));
  const sourceRepo = await realpath(resolve(benchmarkDir, "../.."));
  const workspace = resolve(fixtureRoot, "workspace");
  const exclusionsFile = resolve(fixtureRoot, "exclusions.json");
  const overlayIdentityFile = resolve(fixtureRoot, "overlay-identity.json");
  const overlayBytes = Buffer.from("trusted task overlay\n", "utf8");
  const invalidLists = [
    [""],
    ["/"],
    ["."],
    ["/absolute"],
    ["target/../escape"],
    ["target\\ambiguous"],
    ["target/", "./target"],
  ];
  try {
    await mkdir(workspace);
    await writeFile(resolve(workspace, "AGENTS.md"), overlayBytes);
    await writeOverlayIdentity(workspace, overlayIdentityFile);
    for (const [index, exclusions] of invalidLists.entries()) {
      await writeFile(exclusionsFile, `${JSON.stringify(exclusions)}\n`);
      const result = spawnSync(
        process.execPath,
        [
          captureCandidateScript,
          "--case",
          "TW-01",
          "--workspace",
          workspace,
          "--source-repo",
          sourceRepo,
          "--exclusions-file",
          exclusionsFile,
          "--expected-agents-identity-file",
          overlayIdentityFile,
          "--trusted-dir",
          resolve(fixtureRoot, `capture-${index}`),
          "--expected-agents-sha256",
          sha256(overlayBytes),
        ],
        { encoding: "utf8" },
      );
      assert.notEqual(result.status, 0, JSON.stringify(exclusions));
      assert.match(result.stderr, /invalid non-normalized|duplicate capture exclusion/i);
    }
  } finally {
    await rm(fixtureRoot, { recursive: true, force: true });
  }
});

test("trusted capture rejects a same-byte root overlay inode replacement", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-overlay-identity-"));
  const sourceRepo = await realpath(resolve(benchmarkDir, "../.."));
  const workspace = resolve(fixtureRoot, "workspace");
  const exclusionsFile = resolve(fixtureRoot, "exclusions.json");
  const overlayIdentityFile = resolve(fixtureRoot, "overlay-identity.json");
  const overlayBytes = Buffer.from("trusted task overlay\n", "utf8");
  try {
    await mkdir(workspace);
    await Promise.all([
      writeFile(resolve(workspace, "AGENTS.md"), overlayBytes),
      writeFile(exclusionsFile, "[]\n"),
    ]);
    const preRun = await writeOverlayIdentity(workspace, overlayIdentityFile);
    await rename(resolve(workspace, "AGENTS.md"), resolve(workspace, "AGENTS.before"));
    await writeFile(resolve(workspace, "AGENTS.md"), overlayBytes);
    const replacement = await lstat(resolve(workspace, "AGENTS.md"), { bigint: true });
    assert.notEqual(replacement.ino.toString(), preRun.identity.inode);

    const result = spawnSync(
      process.execPath,
      [
        captureCandidateScript,
        "--case",
        "TW-01",
        "--workspace",
        workspace,
        "--source-repo",
        sourceRepo,
        "--exclusions-file",
        exclusionsFile,
        "--expected-agents-identity-file",
        overlayIdentityFile,
        "--trusted-dir",
        resolve(fixtureRoot, "capture"),
        "--expected-agents-sha256",
        sha256(overlayBytes),
      ],
      { encoding: "utf8" },
    );
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /AGENTS\.md overlay identity changed/i);
  } finally {
    await rm(fixtureRoot, { recursive: true, force: true });
  }
});

test("trusted capture rejects a trusted output redirected into the workspace", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-capture-redirect-"));
  const sourceRepo = await realpath(resolve(benchmarkDir, "../.."));
  const workspace = resolve(fixtureRoot, "workspace");
  const exclusionsFile = resolve(fixtureRoot, "exclusions.json");
  const overlayIdentityFile = resolve(fixtureRoot, "overlay-identity.json");
  const redirectedParent = resolve(fixtureRoot, "redirected-parent");
  const overlayBytes = Buffer.from("trusted task overlay\n", "utf8");
  try {
    await mkdir(workspace);
    await Promise.all([
      writeFile(resolve(workspace, "AGENTS.md"), overlayBytes),
      writeFile(exclusionsFile, "[]\n"),
    ]);
    await writeOverlayIdentity(workspace, overlayIdentityFile);
    await symlink(workspace, redirectedParent);
    const result = spawnSync(
      process.execPath,
      [
        captureCandidateScript,
        "--case",
        "TW-01",
        "--workspace",
        workspace,
        "--source-repo",
        sourceRepo,
        "--exclusions-file",
        exclusionsFile,
        "--expected-agents-identity-file",
        overlayIdentityFile,
        "--trusted-dir",
        resolve(redirectedParent, "capture"),
        "--expected-agents-sha256",
        sha256(overlayBytes),
      ],
      { encoding: "utf8", maxBuffer: 128 * 1024 * 1024 },
    );
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /trusted-dir and workspace must be disjoint/i);
  } finally {
    await rm(fixtureRoot, { recursive: true, force: true });
  }
});

async function createOracleRunnerFixture({command, selector, subjectiveGroups = [], caseId = "TW-XX"}) {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-oracle-runner-"));
  const candidateRoot = resolve(fixtureRoot, "candidate");
  const trustedDir = resolve(fixtureRoot, "trusted");
  const manifestPath = resolve(fixtureRoot, "manifest.json");
  const oracleLockPath = resolve(fixtureRoot, "oracle-lock.json");
  await mkdir(candidateRoot);
  const assertion = selector === null ? [] : [{
    id: "fixture.assertion",
    command_id: "fixture.command",
    selector,
  }];
  await Promise.all([
    writeFile(
      manifestPath,
      `${JSON.stringify({
        protocol_id: "fixture-v1",
        classification: "construction_pilot_only",
        formal_result_eligible: false,
        cases: [{
          id: caseId,
          oracle_commands: [{
            id: "fixture.command",
            command_template: command,
            assertion_ids: assertion.map((entry) => entry.id),
          }],
          assertions: assertion.map(({id, command_id}) => ({
            id,
            command_id,
            stage: "expectation_free_execution",
          })),
          subjective_groups: subjectiveGroups,
        }],
      }, null, 2)}\n`,
    ),
    writeFile(
      oracleLockPath,
      `${JSON.stringify({
        protocol_id: "fixture-v1",
        cases: [{id: caseId, files: [], assertions: assertion}],
      }, null, 2)}\n`,
    ),
  ]);
  return {fixtureRoot, candidateRoot, trustedDir, manifestPath, oracleLockPath, caseId};
}

function runOracleFixture(
  fixture,
  extra = [],
  environment = {},
  {
    rustcPath = trustedRustcPath,
    rustcSha256 = trustedRustcSha256,
    includeRustc = true,
    shellPath = trustedShellPath,
    shellSha256 = trustedShellSha256,
  } = {},
) {
  return spawnSync(
    process.execPath,
    [
      runOraclesScript,
      "--case",
      fixture.caseId,
      "--candidate-root",
      fixture.candidateRoot,
      "--trusted-dir",
      fixture.trustedDir,
      "--manifest",
      fixture.manifestPath,
      "--oracle-lock",
      fixture.oracleLockPath,
      "--expected-manifest-sha256",
      sha256(readFileSync(fixture.manifestPath)),
      "--expected-oracle-lock-sha256",
      sha256(readFileSync(fixture.oracleLockPath)),
      "--expected-control-sha256",
      "0".repeat(64),
      "--trusted-shell",
      shellPath,
      "--expected-shell-sha256",
      shellSha256,
      "--trusted-cargo",
      trustedCargoPath,
      "--expected-cargo-sha256",
      trustedCargoSha256,
      ...(includeRustc ? [
        "--trusted-rustc",
        rustcPath,
        "--expected-rustc-sha256",
        rustcSha256,
      ] : []),
      ...extra,
    ],
    {encoding: "utf8", env: {...process.env, ...environment}},
  );
}

test("oracle runner binds and supervises its trusted shell", async () => {
  const timeoutOverrideFixture = await createOracleRunnerFixture({command: "true", selector: null});
  try {
    const rejected = runOracleFixture(timeoutOverrideFixture, ["--timeout-ms", "1"]);
    assert.notEqual(rejected.status, 0);
    assert.match(rejected.stderr, /timeout-ms.*not permitted|exact.*1800/i);
  } finally {
    await rm(timeoutOverrideFixture.fixtureRoot, {recursive: true, force: true});
  }

  const wrongHashFixture = await createOracleRunnerFixture({command: "true", selector: null});
  try {
    const rejected = runOracleFixture(wrongHashFixture, [], {}, {shellSha256: "f".repeat(64)});
    assert.notEqual(rejected.status, 0);
    assert.match(rejected.stderr, /trusted shell SHA-256 mismatch/i);
  } finally {
    await rm(wrongHashFixture.fixtureRoot, {recursive: true, force: true});
  }

  const fixture = await createOracleRunnerFixture({command: "true", selector: null});
  try {
    const result = runOracleFixture(fixture);
    assert.equal(result.status, 0, result.stderr);
    const receipt = await readJson(resolve(fixture.trustedDir, "oracle-run.json"));
    const shell = receipt.trusted_inputs.find((entry) => entry.kind === "trusted_shell");
    assert.equal(shell.sha256, trustedShellSha256);
    assert.equal(receipt.commands[0].process_supervision.process_group_created, true);
    assert.equal(receipt.commands[0].process_supervision.deadline_seconds, 1800);
    assert.equal(receipt.commands[0].process_supervision.termination_grace_seconds, 10);
    assert.equal(receipt.commands[0].process_supervision.process_group_extinct_before_capture, true);
    assert.equal(receipt.trusted_inputs_postchecked_unchanged, true);
    assert.deepEqual(
      receipt.trusted_input_boundary_checks.map((entry) => entry.unchanged),
      [true],
    );
  } finally {
    await rm(fixture.fixtureRoot, {recursive: true, force: true});
  }
});

test("oracle runner requires an explicitly bound sibling Rust compiler", async () => {
  const fixture = await createOracleRunnerFixture({
    command: "cargo test -p fixture --test fixture --locked locked_name -- --exact",
    selector: {kind: "rust_test_exact", test_name: "locked_name", required_matching_tests: 1},
  });
  const fakeRustc = resolve(fixture.fixtureRoot, "trusted-rustc");
  try {
    await writeFile(fakeRustc, "#!/bin/sh\nexit 0\n", {mode: 0o755});
    const missing = runOracleFixture(fixture, [], {}, {includeRustc: false});
    assert.notEqual(missing.status, 0);
    assert.match(missing.stderr, /require.*trusted-rustc.*expected-rustc-sha256/i);

    const mismatched = runOracleFixture(fixture, [], {}, {
      rustcPath: fakeRustc,
      rustcSha256: sha256(await readFile(fakeRustc)),
    });
    assert.notEqual(mismatched.status, 0);
    assert.match(mismatched.stderr, /same trusted toolchain directory/i);
  } finally {
    await rm(fixture.fixtureRoot, {recursive: true, force: true});
  }
});

async function writeRustOracleCrate(candidateRoot, {testSource, harnessFalse = false}) {
  await mkdir(resolve(candidateRoot, "tests"), {recursive: true});
  await Promise.all([
    writeFile(resolve(candidateRoot, "Cargo.toml"), `[package]
name = "fixture"
version = "0.0.0"
edition = "2024"

[[test]]
name = "fixture"
path = "tests/fixture.rs"
${harnessFalse ? "harness = false\n" : ""}`),
    writeFile(resolve(candidateRoot, "Cargo.lock"), `# This file is automatically @generated by Cargo.
version = 4

[[package]]
name = "fixture"
version = "0.0.0"
`),
    writeFile(resolve(candidateRoot, "tests/fixture.rs"), testSource),
  ]);
}

async function bindRustFixtureFiles(fixture, paths = ["tests/fixture.rs"]) {
  const lock = await readJson(fixture.oracleLockPath);
  lock.cases[0].files = [];
  for (const path of paths) {
    lock.cases[0].files.push({
      path,
      sha256: sha256(await readFile(resolve(fixture.candidateRoot, path))),
    });
  }
  await writeFile(fixture.oracleLockPath, `${JSON.stringify(lock, null, 2)}\n`);
}

test("oracle runner builds with trusted Cargo and executes one real libtest binary", async () => {
  const fixture = await createOracleRunnerFixture({
    command: "cargo test -p fixture --test fixture --locked locked_name -- --exact",
    selector: {
      kind: "rust_test_exact",
      test_name: "locked_name",
      required_matching_tests: 1,
    },
  });
  try {
    await writeRustOracleCrate(fixture.candidateRoot, {
      testSource: "#[test]\nfn locked_name() {}\n",
    });
    await bindRustFixtureFiles(fixture);
    const fakeToolMarker = resolve(fixture.fixtureRoot, "fake-tool-used");
    for (const executable of ["cargo", "rustc"]) {
      await writeFile(
        resolve(fixture.candidateRoot, executable),
        `#!/bin/sh\nprintf '${executable}' > '${fakeToolMarker}'\nexit 99\n`,
        {mode: 0o755},
      );
    }
    const result = runOracleFixture(
      fixture,
      [],
      {PATH: `${fixture.candidateRoot}:${process.env.PATH}`},
    );
    assert.equal(result.status, 0, `${result.stderr}\n${result.stdout}`);
    assert.equal(existsSync(fakeToolMarker), false);
    const receipt = await readJson(resolve(fixture.trustedDir, "oracle-run.json"));
    assert.equal(receipt.assertions[0].matching_tests, 1);
    assert.equal(receipt.assertions[0].pass, true);
    assert.equal(receipt.assertions[0].evidence_mode, "trusted_cargo_direct_libtest_json_v0.1");
    assert.equal(receipt.commands[0].execution_mode, "trusted_cargo_direct_libtest");
    assert.equal(receipt.commands[0].toolchain.cargo.path, trustedCargoPath);
    assert.equal(receipt.commands[0].toolchain.cargo.sha256, trustedCargoSha256);
    assert.equal(receipt.commands[0].toolchain.rustc.path, trustedRustcPath);
    assert.equal(receipt.commands[0].toolchain.rustc.sha256, trustedRustcSha256);
    for (const supervision of [
      receipt.commands[0].rust_build.metadata_process_supervision,
      receipt.commands[0].rust_build.build_process_supervision,
      receipt.commands[0].process_supervision,
    ]) {
      assert.equal(supervision.process_group_created, true);
      assert.equal(supervision.termination_grace_seconds, 10);
      assert.equal(supervision.process_group_extinct_before_capture, true);
    }
    const commandSupervision = receipt.commands[0].command_supervision;
    assert.equal(commandSupervision.deadline_seconds, 1800);
    assert.equal(commandSupervision.stage_processes.length, 3);
    assert.ok(
      commandSupervision.stage_processes[1].deadline_milliseconds <
        commandSupervision.stage_processes[0].deadline_milliseconds,
    );
    assert.ok(
      commandSupervision.stage_processes[2].deadline_milliseconds <
        commandSupervision.stage_processes[1].deadline_milliseconds,
    );
    assert.equal(commandSupervision.all_process_groups_extinct_before_capture, true);
    assert.match(receipt.commands[0].rust_build.artifact.executable_sha256, /^[0-9a-f]{64}$/);
    assert.match(receipt.commands[0].rust_build.artifact.message_sha256, /^[0-9a-f]{64}$/);
    assert.ok(receipt.commands[0].rust_build.stdout.bytes > 0);
    assert.match(receipt.assertions[0].normalized_events_sha256, /^[0-9a-f]{64}$/);
    assert.match(receipt.assertions[0].normalized_suite_sha256, /^[0-9a-f]{64}$/);
  } finally {
    await rm(fixture.fixtureRoot, {recursive: true, force: true});
  }
});

test("oracle runner rejects missing and ignored real libtest matches", async () => {
  for (const [testSource, expectedMatches] of [
    ["#[test]\nfn another_name() {}\n", 0],
    ["#[test]\n#[ignore]\nfn locked_name() {}\n", 1],
  ]) {
    const fixture = await createOracleRunnerFixture({
      command: "cargo test -p fixture --test fixture --locked locked_name -- --exact",
      selector: {
        kind: "rust_test_exact",
        test_name: "locked_name",
        required_matching_tests: 1,
      },
    });
    try {
      await writeRustOracleCrate(fixture.candidateRoot, {testSource});
      await bindRustFixtureFiles(fixture);
      const result = runOracleFixture(fixture);
      assert.equal(result.status, 1, result.stderr);
      const receipt = await readJson(resolve(fixture.trustedDir, "oracle-run.json"));
      assert.equal(receipt.assertions[0].matching_tests, expectedMatches);
      assert.equal(receipt.assertions[0].pass, false);
    } finally {
      await rm(fixture.fixtureRoot, {recursive: true, force: true});
    }
  }
});

test("oracle runner records authenticated Cargo metadata failure as a failed exact assertion", async () => {
  const fixture = await createOracleRunnerFixture({
    command: "cargo test -p fixture --test fixture --locked locked_name -- --exact",
    selector: {kind: "rust_test_exact", test_name: "locked_name", required_matching_tests: 1},
  });
  try {
    const result = runOracleFixture(fixture);
    assert.equal(result.status, 1, result.stderr);
    const receipt = await readJson(resolve(fixture.trustedDir, "oracle-run.json"));
    assert.equal(receipt.commands[0].execution_mode, "trusted_cargo_metadata_failed");
    assert.notEqual(receipt.commands[0].exit_code, 0);
    assert.ok(receipt.commands[0].stderr.bytes > 0);
    assert.match(receipt.commands[0].stderr.sha256, /^[0-9a-f]{64}$/);
    assert.equal(receipt.commands[0].rust_build.package, null);
    assert.equal(receipt.commands[0].rust_build.artifact, null);
    assert.equal(receipt.assertions[0].matching_tests, 0);
    assert.equal(receipt.assertions[0].pass, false);
    assert.equal(receipt.assertions[0].evidence_mode, "trusted_cargo_preflight_failure");
  } finally {
    await rm(fixture.fixtureRoot, {recursive: true, force: true});
  }
});

test("oracle runner records authenticated Cargo build failure as a failed exact assertion", async () => {
  const fixture = await createOracleRunnerFixture({
    command: "cargo test -p fixture --test fixture --locked locked_name -- --exact",
    selector: {kind: "rust_test_exact", test_name: "locked_name", required_matching_tests: 1},
  });
  try {
    await writeRustOracleCrate(fixture.candidateRoot, {
      testSource: "#[test]\nfn locked_name() { this_will_not_compile }\n",
    });
    await bindRustFixtureFiles(fixture);
    const result = runOracleFixture(fixture);
    assert.equal(result.status, 1, result.stderr);
    const receipt = await readJson(resolve(fixture.trustedDir, "oracle-run.json"));
    assert.equal(receipt.commands[0].execution_mode, "trusted_cargo_build_failed");
    assert.notEqual(receipt.commands[0].exit_code, 0);
    assert.ok(receipt.commands[0].rust_build.package);
    assert.equal(receipt.commands[0].rust_build.artifact, null);
    assert.ok(receipt.commands[0].rust_build.stderr.bytes > 0);
    assert.equal(receipt.assertions[0].pass, false);
    assert.equal(receipt.assertions[0].evidence_mode, "trusted_cargo_preflight_failure");
  } finally {
    await rm(fixture.fixtureRoot, {recursive: true, force: true});
  }
});

test("oracle runner rejects harness=false, candidate Cargo config, and runner overrides", async () => {
  const fixture = await createOracleRunnerFixture({
    command: "cargo test -p fixture --test fixture --locked locked_name -- --exact",
    selector: {
      kind: "rust_test_exact",
      test_name: "locked_name",
      required_matching_tests: 1,
    },
  });
  try {
    await writeRustOracleCrate(fixture.candidateRoot, {
      harnessFalse: true,
      testSource: "fn main() { println!(\"{\\\"type\\\":\\\"test\\\",\\\"event\\\":\\\"ok\\\",\\\"name\\\":\\\"locked_name\\\"}\"); }\n",
    });
    await bindRustFixtureFiles(fixture);
    const result = runOracleFixture(fixture);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /harness\s*=\s*false/i);
  } finally {
    await rm(fixture.fixtureRoot, {recursive: true, force: true});
  }

  const configured = await createOracleRunnerFixture({
    command: "cargo test -p fixture --test fixture --locked locked_name -- --exact",
    selector: {kind: "rust_test_exact", test_name: "locked_name", required_matching_tests: 1},
  });
  try {
    await writeRustOracleCrate(configured.candidateRoot, {
      testSource: "#[test]\nfn locked_name() {}\n",
    });
    await bindRustFixtureFiles(configured);
    await mkdir(resolve(configured.candidateRoot, ".cargo"));
    await writeFile(resolve(configured.candidateRoot, ".cargo/config.toml"), "[target.'cfg(all())']\nrunner = 'false'\n");
    const result = runOracleFixture(configured);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /candidate Cargo config/i);
  } finally {
    await rm(configured.fixtureRoot, {recursive: true, force: true});
  }

  const overridden = await createOracleRunnerFixture({
    command: "cargo test -p fixture --test fixture --locked locked_name -- --exact",
    selector: {kind: "rust_test_exact", test_name: "locked_name", required_matching_tests: 1},
  });
  try {
    await writeRustOracleCrate(overridden.candidateRoot, {
      testSource: "#[test]\nfn locked_name() {}\n",
    });
    await bindRustFixtureFiles(overridden);
    const result = runOracleFixture(overridden, [], {
      CARGO_TARGET_AARCH64_APPLE_DARWIN_RUNNER: "false",
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /runner environment override/i);
  } finally {
    await rm(overridden.fixtureRoot, {recursive: true, force: true});
  }
});

test("oracle runner rejects a redirected locked Rust target and compact harness=false", async () => {
  const redirected = await createOracleRunnerFixture({
    command: "cargo test -p fixture --test fixture --locked locked_name -- --exact",
    selector: {kind: "rust_test_exact", test_name: "locked_name", required_matching_tests: 1},
  });
  try {
    await writeRustOracleCrate(redirected.candidateRoot, {
      testSource: "#[test]\nfn locked_name() {}\n",
    });
    await bindRustFixtureFiles(redirected);
    await writeFile(
      resolve(redirected.candidateRoot, "tests/redirected.rs"),
      "#[test]\nfn locked_name() {}\n",
    );
    await writeFile(resolve(redirected.candidateRoot, "Cargo.toml"), `[package]
name = "fixture"
version = "0.0.0"
edition = "2024"

[[test]]
name = "fixture"
path = "tests/redirected.rs"
`);
    const result = runOracleFixture(redirected);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /test source.*frozen locked.*tests\/fixture\.rs/i);
  } finally {
    await rm(redirected.fixtureRoot, {recursive: true, force: true});
  }

  const harness = await createOracleRunnerFixture({
    command: "cargo test -p fixture --test fixture --locked locked_name -- --exact",
    selector: {kind: "rust_test_exact", test_name: "locked_name", required_matching_tests: 1},
  });
  try {
    await writeRustOracleCrate(harness.candidateRoot, {
      testSource: "fn main() {}\n",
    });
    await bindRustFixtureFiles(harness);
    const manifest = await readFile(resolve(harness.candidateRoot, "Cargo.toml"), "utf8");
    await writeFile(
      resolve(harness.candidateRoot, "Cargo.toml"),
      `${manifest}harness=false#comment\n`,
    );
    const result = runOracleFixture(harness);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /harness\s*=\s*false/i);
  } finally {
    await rm(harness.fixtureRoot, {recursive: true, force: true});
  }
});

test("oracle runner binds every locked Rust file before and after exact execution", async () => {
  const mismatched = await createOracleRunnerFixture({
    command: "cargo test -p fixture --test fixture --locked locked_name -- --exact",
    selector: {kind: "rust_test_exact", test_name: "locked_name", required_matching_tests: 1},
  });
  try {
    await writeRustOracleCrate(mismatched.candidateRoot, {
      testSource: "#[test]\nfn locked_name() {}\n",
    });
    await bindRustFixtureFiles(mismatched);
    await writeFile(
      resolve(mismatched.candidateRoot, "tests/fixture.rs"),
      "#[test]\nfn locked_name() { assert!(true); }\n",
    );
    const result = runOracleFixture(mismatched);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /locked file SHA-256 mismatch.*tests\/fixture\.rs/i);
  } finally {
    await rm(mismatched.fixtureRoot, {recursive: true, force: true});
  }

  const mutated = await createOracleRunnerFixture({
    command: "cargo test -p fixture --test fixture --locked locked_name -- --exact",
    selector: {kind: "rust_test_exact", test_name: "locked_name", required_matching_tests: 1},
  });
  try {
    await writeRustOracleCrate(mutated.candidateRoot, {
      testSource: `#[test]
fn locked_name() { std::fs::write("locked.txt", "changed\\n").unwrap(); }
`,
    });
    await writeFile(resolve(mutated.candidateRoot, "locked.txt"), "original\n");
    await bindRustFixtureFiles(mutated, ["tests/fixture.rs", "locked.txt"]);
    const result = runOracleFixture(mutated);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /locked file SHA-256 mismatch.*locked\.txt/i);
  } finally {
    await rm(mutated.fixtureRoot, {recursive: true, force: true});
  }
});

test("oracle runner records nonzero commands and JSON-pointer mismatches fail closed", async () => {
  for (const [command, expectedReason] of [
    ["printf '{\"assertions\":{\"ready\":true}}\\n'; exit 7", /command exited 7/i],
    ["printf '{\"assertions\":{\"ready\":false}}\\n'", /JSON pointer value mismatch/i],
  ]) {
    const fixture = await createOracleRunnerFixture({
      command,
      selector: {
        kind: "json_pointer",
        json_pointer: "/assertions/ready",
        expected: true,
      },
    });
    try {
      const result = runOracleFixture(fixture);
      assert.equal(result.status, 1, result.stderr);
      const receipt = await readJson(resolve(fixture.trustedDir, "oracle-run.json"));
      assert.equal(receipt.commands[0].exit_code, command.includes("exit 7") ? 7 : 0);
      assert.match(receipt.commands[0].command_template_sha256, /^[0-9a-f]{64}$/);
      assert.match(receipt.commands[0].resolved_command_sha256, /^[0-9a-f]{64}$/);
      assert.match(receipt.commands[0].stdout.sha256, /^[0-9a-f]{64}$/);
      assert.equal(typeof receipt.commands[0].stdout.bytes, "number");
      assert.equal(receipt.assertions[0].pass, false);
      assert.match(receipt.assertions[0].reasons.join(" "), expectedReason);
      if (!command.includes("exit 7")) {
        assert.equal(receipt.assertions[0].actual, false);
        assert.match(receipt.assertions[0].actual_canonical_sha256, /^[0-9a-f]{64}$/);
      }
    } finally {
      await rm(fixture.fixtureRoot, {recursive: true, force: true});
    }
  }
});

test("oracle runner selects locked portable records and rejects a mismatch", async () => {
  const fixture = await createOracleRunnerFixture({
    command: "printf '{\"contract_id\":\"tachiko-portable-observations-v1\",\"native\":[{\"index\":3,\"class\":0,\"bits\":\"bad\",\"auxiliary\":\"9\"}],\"wasm\":[{\"index\":3,\"class\":0,\"bits\":\"good\",\"auxiliary\":\"9\"}]}\\n' > <trusted-portable-observations-file>",
    selector: {
      kind: "portable_record_set",
      indexes: [3],
      expected_records: [{index: 3, class: 0, bits: "good", auxiliary: "9"}],
      require_selected_native_wasm_equal: true,
      reject_class: 255,
    },
  });
  try {
    const result = runOracleFixture(fixture);
    assert.equal(result.status, 1, result.stderr);
    const receipt = await readJson(resolve(fixture.trustedDir, "oracle-run.json"));
    assert.equal(receipt.assertions[0].pass, false);
    assert.deepEqual(receipt.assertions[0].selected_native, [
      {index: 3, class: 0, bits: "bad", auxiliary: "9"},
    ]);
    assert.match(receipt.assertions[0].selected_native_sha256, /^[0-9a-f]{64}$/);
    assert.match(receipt.assertions[0].selected_wasm_sha256, /^[0-9a-f]{64}$/);
    assert.match(receipt.assertions[0].reasons.join(" "), /differ/i);
  } finally {
    await rm(fixture.fixtureRoot, {recursive: true, force: true});
  }
});

test("oracle runner classifies subjective-only cases as packet gates", async () => {
  const fixture = await createOracleRunnerFixture({
    command: "true",
    selector: null,
    subjectiveGroups: [{id: "semantic", stage: "blinded_review_packet"}],
  });
  try {
    const result = runOracleFixture(fixture);
    assert.equal(result.status, 0, result.stderr);
    const receipt = await readJson(resolve(fixture.trustedDir, "oracle-run.json"));
    assert.equal(receipt.assessment_mode, "subjective_only_packet_gate");
    assert.equal(receipt.overall_status, "packet_gate_ready");
    assert.equal(receipt.machine_score_claimed, false);
  } finally {
    await rm(fixture.fixtureRoot, {recursive: true, force: true});
  }
});

test("production oracle runner rejects missing or tampered required controller context", async () => {
  const missingFixture = await createOracleRunnerFixture({command: "true", selector: null});
  try {
    const missing = runOracleFixture(missingFixture, ["--require-formal-context", "true"]);
    assert.notEqual(missing.status, 0);
    assert.match(missing.stderr, /controller context is required/i);
  } finally {
    await rm(missingFixture.fixtureRoot, {recursive: true, force: true});
  }
  const tamperedFixture = await createOracleRunnerFixture({command: "true", selector: null});
  try {
    const context = await writeFormalControllerContext(resolve(tamperedFixture.fixtureRoot, "context.json"));
    const tampered = runOracleFixture(tamperedFixture, [
      "--controller-context", context.path,
      "--expected-controller-context-sha256", "f".repeat(64),
      "--require-formal-context", "true",
    ]);
    assert.notEqual(tampered.status, 0);
    assert.match(tampered.stderr, /controller context SHA-256 mismatch/i);
  } finally {
    await rm(tamperedFixture.fixtureRoot, {recursive: true, force: true});
  }
});

test("formal oracle adapter output is materialized only after sandboxed group extinction", async () => {
  const fixture = await createOracleRunnerFixture({
    caseId: "TW-05",
    command: "node <trusted-adapter-file> --candidate-root <validation-workspace> " +
      "--contract <trusted-contract-file> --output <trusted-observations-file>",
    selector: null,
  });
  const externalRoot = await mkdtemp(join(tmpdir(), "tachiko-formal-oracle-adapter-"));
  const runRoot = resolve(fixture.fixtureRoot, "neutral-run");
  const runTmp = resolve(runRoot, "adapter-tmp-fixture");
  try {
    await mkdir(runTmp, {recursive: true});
    const scaffold = resolve(benchmarkDir, "evaluator/adapters/candidate-adapter.mjs");
    const probe = resolve(externalRoot, "probe.mjs");
    const config = resolve(externalRoot, "config.json");
    const integrity = resolve(externalRoot, "integrity.json");
    const observations = [
      {step: "open", revision: 0},
      {step: "overview", entity_count: 2, formula_count: 2},
      {step: "calculate", first_product: 2, second_product: 4},
      {step: "set_first_base", revision: 1, first_product: 22},
      {step: "stale_set_first_base", typed_stale_revision_error: true,
        actual_revision: 1, state_unchanged: true},
      {step: "snapshot", revision: 1, first_base: 11, first_product: 22},
    ];
    const behavior = {
      native: {execution: "native_process", observations},
      wasm: {execution: "real_wasm32", worker_boundary: "typescript_worker", observations},
    };
    await writeFile(resolve(fixture.candidateRoot, "adapter-behavior.json"),
      `${JSON.stringify(behavior)}\n`, {mode: 0o400});
    await writeFile(
      probe,
      "#!/usr/bin/env node\nimport {readFileSync,writeFileSync} from 'node:fs';\n" +
        "import {resolve} from 'node:path';\n" +
        "writeFileSync(resolve(process.env.TMPDIR,'adapter.tmp'),'temporary\\n');\n" +
        "console.log(readFileSync(resolve(process.argv[2],'adapter-behavior.json'),'utf8').trim());\n",
      {mode: 0o755},
    );
    const configBytes = Buffer.from(`${JSON.stringify({
      schema: "tachiko-candidate-adapter-v1", case_id: "TW-05",
      probe: {executable: probe, sha256: sha256(await readFile(probe)),
        arguments: ["<candidate-root>"]},
    })}\n`);
    await writeFile(config, configBytes);
    const context = await writeFormalControllerContext(resolve(fixture.fixtureRoot, "context.json"), {
      protocol_id: "tachiko-agents-effect-v1",
      case_id: "TW-05",
      adapter_forbidden_roots: [await realpath(fixture.fixtureRoot)],
      adapter_write_forbidden_roots: [await realpath(runRoot)],
      adapter_write_allowed_roots: [await realpath(runTmp)],
      adapter_tmp_initial_sha256: sha256("[]\n"),
    });
    const integrityBytes = Buffer.from(`${JSON.stringify({
      schema: "tachiko-adapter-integrity-review-v1",
      protocol_id: context.context.protocol_id,
      phase: context.context.phase,
      wave_id: context.context.wave_id,
      run_id: context.context.run_id,
      attempt_id: context.context.attempt_id,
      candidate_id: context.context.candidate_id,
      case_id: context.context.case_id,
      capture_receipt_sha256: context.context.capture_receipt_sha256,
      scaffold_sha256: sha256(await readFile(scaffold)),
      config_sha256: sha256(configBytes),
      probe_sha256: sha256(await readFile(probe)),
      reviewer_id: "construction-fixture-reviewer", reviewer_eligible: true,
      reviewer_independent: true, no_expected_values: true,
      no_behavior_implementation: true, actual_candidate_exercise: true, approved: true,
    })}\n`);
    await writeFile(integrity, integrityBytes, {mode: 0o400});
    const controlPaths = [
      "environment-lock.json", "evaluator/cases.json", "evaluator/oracle-lock.json",
      "evaluator/core-score-lock.json", "evaluator/authority-lock.json",
      "evaluator/production-oracles.json",
    ];
    const controlIdentities = [];
    for (const path of controlPaths) {
      const bytes = await readFile(resolve(benchmarkDir, path));
      controlIdentities.push({path, bytes: bytes.length, sha256: sha256(bytes)});
    }
    const expectedControlSha256 = sha256(`${JSON.stringify(controlIdentities)}\n`);
    const result = spawnSync(process.execPath, [
      runOraclesScript,
      "--case", "TW-05",
      "--candidate-root", fixture.candidateRoot,
      "--trusted-dir", fixture.trustedDir,
      "--expected-control-sha256", expectedControlSha256,
      "--trusted-shell", trustedShellPath,
      "--expected-shell-sha256", trustedShellSha256,
      "--adapter-file", scaffold,
      "--adapter-config", config,
      "--adapter-integrity-receipt", integrity,
      "--expected-adapter-integrity-sha256", sha256(integrityBytes),
      "--controller-context", context.path,
      "--expected-controller-context-sha256", context.sha256,
      "--require-formal-context", "true",
    ], {encoding: "utf8", env: {...process.env, TMPDIR: runTmp}});
    assert.equal(result.status, 0, result.stderr || result.stdout);
    const receipt = await readJson(resolve(fixture.trustedDir, "oracle-run.json"));
    const materialization = receipt.commands[0].trusted_adapter_materialization;
    assert.equal(materialization.materialized_after_process_group_extinction, true);
    assert.equal(receipt.commands[0].process_supervision.process_group_extinct_before_capture, true);
    assert.equal(receipt.commands[0].candidate_input_immutability.unchanged, true);
    assert.equal(receipt.commands[0].adapter_temporary_root.initial_entries, 0);
    assert.equal(receipt.commands[0].adapter_temporary_root.path, await realpath(runTmp));
    assert.match(receipt.commands[0].candidate_input_immutability.before_sha256, /^[0-9a-f]{64}$/);
    assert.equal(
      receipt.commands[0].candidate_input_immutability.before_sha256,
      receipt.commands[0].candidate_input_immutability.after_sha256,
    );
    assert.equal(materialization.source_stdout_sha256, receipt.commands[0].stdout.sha256);
    assert.equal(await readFile(resolve(runTmp, "adapter.tmp"), "utf8"), "temporary\n");
    assert.equal(
      materialization.sha256,
      sha256(await readFile(resolve(fixture.trustedDir, "observations.json"))),
    );
  } finally {
    await rm(fixture.fixtureRoot, {recursive: true, force: true});
    await rm(externalRoot, {recursive: true, force: true});
  }
});

test("production oracle commands deny a real socket attempt in the kernel sandbox", async () => {
  const fixture = await createOracleRunnerFixture({
    command: `${JSON.stringify(process.execPath)} ${JSON.stringify(networkProbeScript)}`,
    selector: null,
    subjectiveGroups: [{id: "semantic", stage: "blinded_review_packet"}],
  });
  try {
    const result = runOracleFixture(fixture);
    assert.equal(result.status, 0, result.stderr);
    const receipt = await readJson(resolve(fixture.trustedDir, "oracle-run.json"));
    assert.equal(receipt.network_enforcement.probe_denied, true);
    assert.equal(receipt.commands[0].process_supervision.network_sandbox.mode,
      "darwin_sandbox_deny_network");
    assert.match(
      await readFile(resolve(fixture.trustedDir, receipt.commands[0].stdout.path), "utf8"),
      /network-denied:(?:EPERM|EACCES)/,
    );
  } finally {
    await rm(fixture.fixtureRoot, {recursive: true, force: true});
  }
});

test("oracle runner rejects a candidate-controlled trusted adapter", async () => {
  const fixture = await createOracleRunnerFixture({
    command: "node <trusted-adapter-file>",
    selector: null,
  });
  try {
    const adapter = resolve(fixture.candidateRoot, "adapter.mjs");
    await writeFile(adapter, "process.exit(0);\n");
    const result = runOracleFixture(fixture, ["--adapter-file", adapter]);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /adapter-file.*candidate-root.*disjoint/i);
  } finally {
    await rm(fixture.fixtureRoot, {recursive: true, force: true});
  }
});

test("oracle runner rejects candidate-controlled control files and duplicate command IDs", async () => {
  const candidateControls = await createOracleRunnerFixture({
    command: "true",
    selector: null,
  });
  try {
    const manifest = resolve(candidateControls.candidateRoot, "manifest.json");
    const lock = resolve(candidateControls.candidateRoot, "lock.json");
    await Promise.all([
      copyFile(candidateControls.manifestPath, manifest),
      copyFile(candidateControls.oracleLockPath, lock),
    ]);
    candidateControls.manifestPath = manifest;
    candidateControls.oracleLockPath = lock;
    const result = runOracleFixture(candidateControls);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /(?:manifest|oracle-lock).*candidate-root.*disjoint/i);
  } finally {
    await rm(candidateControls.fixtureRoot, {recursive: true, force: true});
  }

  const duplicate = await createOracleRunnerFixture({command: "true", selector: null});
  try {
    const manifest = await readJson(duplicate.manifestPath);
    manifest.cases[0].oracle_commands.push({...manifest.cases[0].oracle_commands[0]});
    await writeFile(duplicate.manifestPath, `${JSON.stringify(manifest)}\n`);
    const result = runOracleFixture(duplicate);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /duplicate command ID/i);
  } finally {
    await rm(duplicate.fixtureRoot, {recursive: true, force: true});
  }
});

test("production oracle runner rejects an unbound control digest", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-oracle-control-digest-"));
  const candidateRoot = resolve(fixtureRoot, "candidate");
  const trustedDir = resolve(fixtureRoot, "trusted");
  await mkdir(candidateRoot);
  try {
    const result = spawnSync(process.execPath, [
      runOraclesScript,
      "--case", "TW-01",
      "--candidate-root", candidateRoot,
      "--trusted-dir", trustedDir,
      "--expected-control-sha256", "0".repeat(64),
      "--trusted-shell", trustedShellPath,
      "--expected-shell-sha256", trustedShellSha256,
    ], {encoding: "utf8"});
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /control SHA-256 mismatch/i);
    assert.equal(existsSync(trustedDir), false);
  } finally {
    await rm(fixtureRoot, {recursive: true, force: true});
  }
});

test("candidate adapter scaffold fails closed on an untrusted probe hash", async () => {
  const fixture = await createOracleRunnerFixture({
    command: "node <trusted-adapter-file> --candidate-root <validation-workspace> --contract <trusted-contract-file> --output <trusted-observations-file>",
    selector: null,
  });
  const adapter = resolve(benchmarkDir, "evaluator/adapters/candidate-adapter.mjs");
  const contract = resolve(fixture.fixtureRoot, "contract.json");
  const probe = resolve(fixture.fixtureRoot, "probe.mjs");
  const config = resolve(fixture.fixtureRoot, "adapter-config.json");
  try {
    await writeFile(contract, "{}\n");
    await writeFile(probe, "#!/usr/bin/env node\nconsole.log('{}');\n", {mode: 0o755});
    await writeFile(config, `${JSON.stringify({
      schema: "tachiko-candidate-adapter-v1",
      case_id: "TW-05",
      probe: {
        executable: probe,
        sha256: "f".repeat(64),
        arguments: ["<candidate-root>"],
      },
    })}\n`);
    const result = runOracleFixture(fixture, [
      "--adapter-file", adapter,
      "--adapter-config", config,
      "--contract-file", contract,
    ]);
    assert.equal(result.status, 1, result.stderr);
    const receipt = await readJson(resolve(fixture.trustedDir, "oracle-run.json"));
    assert.equal(receipt.commands[0].exit_code, 1);
    const adapterStderr = await readFile(
      resolve(fixture.trustedDir, receipt.commands[0].stderr.path),
      "utf8",
    );
    assert.match(adapterStderr, /probe SHA-256 mismatch/i);
  } finally {
    await rm(fixture.fixtureRoot, {recursive: true, force: true});
  }
});

test("TW-05 offline runner never invokes a package manager", async () => {
  const fixtureRoot = await realpath(await mkdtemp(join(tmpdir(), "tachiko-tw05-offline-")));
  const binDir = resolve(fixtureRoot, "bin");
  const candidateRoot = resolve(fixtureRoot, "candidate");
  const output = resolve(fixtureRoot, "receipt.json");
  const marker = resolve(fixtureRoot, "package-manager-invoked");
  try {
    await Promise.all([mkdir(binDir), mkdir(candidateRoot)]);
    for (const executable of ["cargo", "node", "rustup"]) {
      const body = executable === "rustup"
        ? "#!/bin/sh\n[ \"$1 $2 $3\" = \"target list --installed\" ] && printf 'wasm32-unknown-unknown\\n'\n"
        : "#!/bin/sh\nexit 0\n";
      await writeFile(resolve(binDir, executable), body, {mode: 0o755});
    }
    for (const executable of ["npm", "pnpm", "yarn"]) {
      await writeFile(
        resolve(binDir, executable),
        `#!/bin/sh\nprintf invoked > '${marker}'\nexit 99\n`,
        {mode: 0o755},
      );
    }
    const result = spawnSync(
      executablePath("node"),
      [
        runTw05OfflineScript,
        "--candidate-root",
        candidateRoot,
        "--output",
        output,
        "--cargo-command",
        "cargo test --locked",
        "--node-test-file",
        "worker.test.mjs",
        "--node-benchmark-file",
        "bench.mjs",
      ],
      {encoding: "utf8", env: {...process.env, PATH: `${binDir}:/usr/bin:/bin`}},
    );
    assert.equal(result.status, 0, `${result.stderr}\n${result.stdout}`);
    assert.equal(existsSync(marker), false);
    const receipt = await readJson(output);
    assert.equal(receipt.offline, true);
    assert.equal(receipt.network_enforcement.mode, "darwin_sandbox_deny_network");
    assert.equal(receipt.network_enforcement.probe_denied, true);
    assert.match(receipt.network_enforcement.profile_sha256, /^[0-9a-f]{64}$/);
    assert.deepEqual(receipt.executables.map((entry) => entry.name), ["cargo", "node", "node"]);
    assert.ok(receipt.executions.every((entry) => entry.exit_code === 0));
    for (const execution of receipt.executions) {
      assert.equal(typeof execution.stdout.bytes, "number");
      assert.match(execution.stdout.sha256, /^[0-9a-f]{64}$/);
      assert.equal(typeof execution.stderr.bytes, "number");
      assert.match(execution.stderr.sha256, /^[0-9a-f]{64}$/);
    }
  } finally {
    await rm(fixtureRoot, {recursive: true, force: true});
  }
});

test("TW-05 offline runner reserves a trusted disjoint output", async () => {
  const fixtureRoot = await realpath(await mkdtemp(join(tmpdir(), "tachiko-tw05-output-")));
  const candidateRoot = resolve(fixtureRoot, "candidate");
  const outside = resolve(fixtureRoot, "outside");
  await Promise.all([mkdir(candidateRoot), mkdir(outside)]);
  const baseArguments = [
    runTw05OfflineScript,
    "--candidate-root", candidateRoot,
    "--cargo-command", "cargo test --locked",
    "--node-test-file", "worker.test.mjs",
    "--node-benchmark-file", "bench.mjs",
  ];
  try {
    for (const [output, expected] of [
      ["relative.json", /output must be absolute/i],
      [resolve(candidateRoot, "receipt.json"), /output.*candidate-root.*disjoint/i],
    ]) {
      const result = spawnSync(process.execPath, [...baseArguments, "--output", output], {
        encoding: "utf8",
      });
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, expected);
    }

    const existing = resolve(outside, "existing.json");
    await writeFile(existing, "preserve\n");
    const existingResult = spawnSync(
      process.execPath,
      [...baseArguments, "--output", existing],
      {encoding: "utf8"},
    );
    assert.notEqual(existingResult.status, 0);
    assert.match(existingResult.stderr, /output.*must not already exist/i);
    assert.equal(await readFile(existing, "utf8"), "preserve\n");

    const symlinkLeaf = resolve(outside, "symlink-leaf.json");
    await symlink(existing, symlinkLeaf);
    const symlinkLeafResult = spawnSync(
      process.execPath,
      [...baseArguments, "--output", symlinkLeaf],
      {encoding: "utf8"},
    );
    assert.notEqual(symlinkLeafResult.status, 0);
    assert.match(symlinkLeafResult.stderr, /including symlink or special files/i);

    const redirected = resolve(fixtureRoot, "redirected");
    await symlink(outside, redirected, "dir");
    const redirectedResult = spawnSync(
      process.execPath,
      [...baseArguments, "--output", resolve(redirected, "receipt.json")],
      {encoding: "utf8"},
    );
    assert.notEqual(redirectedResult.status, 0);
    assert.match(redirectedResult.stderr, /output parent.*symlink/i);

    const redirectedExisting = resolve(outside, "existing-parent");
    await mkdir(redirectedExisting);
    const nestedRedirect = resolve(fixtureRoot, "nested-redirect");
    await symlink(outside, nestedRedirect, "dir");
    const nestedRedirectResult = spawnSync(
      process.execPath,
      [...baseArguments, "--output", resolve(nestedRedirect, "existing-parent", "receipt.json")],
      {encoding: "utf8"},
    );
    assert.notEqual(nestedRedirectResult.status, 0);
    assert.match(nestedRedirectResult.stderr, /output parent.*symlink/i);
  } finally {
    await rm(fixtureRoot, {recursive: true, force: true});
  }
});

test("TW-05 offline runner removes its reservation on setup failure", async () => {
  const fixtureRoot = await realpath(await mkdtemp(join(tmpdir(), "tachiko-tw05-setup-fail-")));
  const candidateRoot = resolve(fixtureRoot, "candidate");
  const output = resolve(fixtureRoot, "failure-receipt.json");
  try {
    await mkdir(candidateRoot);
    const result = spawnSync(process.execPath, [
      runTw05OfflineScript,
      "--candidate-root", candidateRoot,
      "--output", output,
      "--cargo-command", "not-cargo test --locked",
    ], {encoding: "utf8"});
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /cargo-command must be a direct cargo invocation/i);
    assert.equal(existsSync(output), false);
  } finally {
    await rm(fixtureRoot, {recursive: true, force: true});
  }
});

test("oracle qualification summary regenerates byte-for-byte across controlled fixture runs", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-oracle-summary-"));
  try {
    const receipts = [];
    for (const name of ["first.json", "second.json"]) {
      const output = resolve(fixtureRoot, name);
      const result = spawnSync(process.execPath, [
        qualifyOraclesScript,
        "--source-repo", resolve(benchmarkDir, "../.."),
        "--output", output,
        "--mode", "fixture-fast",
      ], {encoding: "utf8", maxBuffer: 128 * 1024 * 1024});
      assert.equal(result.status, 0, `${result.stderr}\n${result.stdout}`);
      receipts.push(await readJson(output));
    }
    for (const receipt of receipts) {
      assert.equal(receipt.payload_sha256, sha256(`${JSON.stringify(receipt.payload)}\n`));
      assert.equal(
        receipt.run_receipt_sha256,
        sha256(`${JSON.stringify(receipt.run_receipt)}\n`),
      );
      assert.equal(receipt.payload.schema, "tachiko-oracle-qualification-summary-v3");
      assert.equal(receipt.run_receipt.schema, "tachiko-oracle-qualification-run-v3");
      assert.deepEqual(receipt.payload.trusted_shell, {
        bytes: receipt.run_receipt.trusted_shell.bytes,
        sha256: receipt.run_receipt.trusted_shell.sha256,
        version: receipt.run_receipt.trusted_shell.version,
      });
      assert.equal(receipt.payload.mode, "fixture-fast");
      assert.equal(receipt.run_receipt.mode, "fixture-fast");
      assert.ok(receipt.run_receipt.cases.length > 0);
      assert.ok(receipt.payload.cases.length > 0);
      assert.equal(
        receipt.evidence_commitment_sha256,
        receipt.payload.evidence_commitment_sha256,
      );
    }
    const firstFixture = receipts[0].run_receipt.cases[0];
    const secondFixture = receipts[1].run_receipt.cases[0];
    assert.notEqual(
      firstFixture.target.core.commands[0].resolved_command_sha256,
      secondFixture.target.core.commands[0].resolved_command_sha256,
    );
    assert.notEqual(
      firstFixture.target.oracle.assertions[0].suite_summary.exec_time,
      secondFixture.target.oracle.assertions[0].suite_summary.exec_time,
    );
    assert.notEqual(
      firstFixture.target.oracle.adapter_execution.observation.cargo_stdout_sha256,
      secondFixture.target.oracle.adapter_execution.observation.cargo_stdout_sha256,
    );
    assert.notEqual(
      firstFixture.offline_historical_target.executions[0].stdout.sha256,
      secondFixture.offline_historical_target.executions[0].stdout.sha256,
    );
    assert.equal(JSON.stringify(receipts[0].payload), JSON.stringify(receipts[1].payload));
    assert.equal(receipts[0].payload_sha256, receipts[1].payload_sha256);
    assert.doesNotMatch(
      JSON.stringify(receipts[0].payload),
      /\/var\/folders|exec_time|duration_ms/,
    );
  } finally {
    await rm(fixtureRoot, {recursive: true, force: true});
  }
});

test("qualification verifier rejects independently tampered summary or run evidence", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-oracle-verify-"));
  try {
    const originalPath = resolve(fixtureRoot, "original.json");
    const generated = spawnSync(process.execPath, [
      qualifyOraclesScript,
      "--source-repo", resolve(benchmarkDir, "../.."),
      "--output", originalPath,
      "--mode", "fixture-fast",
    ], {encoding: "utf8", maxBuffer: 128 * 1024 * 1024});
    assert.equal(generated.status, 0, `${generated.stderr}\n${generated.stdout}`);
    const original = await readJson(originalPath);
    const accepted = spawnSync(process.execPath, [
      verifyOracleQualificationScript,
      "--receipt", originalPath,
    ], {encoding: "utf8"});
    assert.equal(accepted.status, 0, accepted.stderr);

    const tamperedPayload = structuredClone(original);
    tamperedPayload.payload.cases[0].case_id = "TAMPERED-SUMMARY";
    tamperedPayload.payload_sha256 = sha256(`${JSON.stringify(tamperedPayload.payload)}\n`);
    const tamperedPayloadPath = resolve(fixtureRoot, "tampered-payload.json");
    await writeFile(tamperedPayloadPath, `${JSON.stringify(tamperedPayload)}\n`);
    const payloadResult = spawnSync(process.execPath, [
      verifyOracleQualificationScript,
      "--receipt", tamperedPayloadPath,
    ], {encoding: "utf8"});
    assert.notEqual(payloadResult.status, 0);
    assert.match(payloadResult.stderr, /deterministic summary does not match run evidence/i);

    const tamperedRun = structuredClone(original);
    tamperedRun.run_receipt.cases[0].case_id = "TAMPERED-RUN";
    tamperedRun.run_receipt_sha256 = sha256(`${JSON.stringify(tamperedRun.run_receipt)}\n`);
    const tamperedRunPath = resolve(fixtureRoot, "tampered-run.json");
    await writeFile(tamperedRunPath, `${JSON.stringify(tamperedRun)}\n`);
    const runResult = spawnSync(process.execPath, [
      verifyOracleQualificationScript,
      "--receipt", tamperedRunPath,
    ], {encoding: "utf8"});
    assert.notEqual(runResult.status, 0);
    assert.match(runResult.stderr, /deterministic summary does not match run evidence/i);
  } finally {
    await rm(fixtureRoot, {recursive: true, force: true});
  }
});

test("benchmark verifier authenticates the checked oracle qualification receipt", () => {
  const result = spawnSync(process.execPath, [verifyBenchmarkScript], {
    encoding: "utf8",
    maxBuffer: 128 * 1024 * 1024,
  });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /oracle_qualification_payload_sha256=[0-9a-f]{64}/);
  assert.match(result.stdout, /oracle_qualification_evidence_commitment_sha256=[0-9a-f]{64}/);
});

test("oracle qualification records executed target/base evidence for all frozen mappings", async () => {
  const [receipt, manifest] = await Promise.all([
    readJson(resolve(benchmarkDir, "evaluator/qualifications/oracles.json")),
    readJson(resolve(benchmarkDir, "evaluator/production-oracles.json")),
  ]);
  assert.equal(receipt.payload_sha256, sha256(`${JSON.stringify(receipt.payload)}\n`));
  assert.equal(receipt.run_receipt_sha256, sha256(`${JSON.stringify(receipt.run_receipt)}\n`));
  assert.equal(
    receipt.evidence_commitment_sha256,
    receipt.payload.evidence_commitment_sha256,
  );
  assert.equal(receipt.payload.schema, "tachiko-oracle-qualification-summary-v3");
  assert.equal(receipt.run_receipt.schema, "tachiko-oracle-qualification-run-v3");
  assert.equal(receipt.payload.no_codex_launched, true);
  assert.equal(receipt.run_receipt.cases.length, 9);
  assert.deepEqual(receipt.run_receipt.cases.map((entry) => entry.case_id), [
    "TW-01", "TW-02", "TW-03", "TW-04", "TW-05", "TW-06", "TW-07", "TW-08", "TW-09",
  ]);

  for (const caseEntry of receipt.run_receipt.cases) {
    const frozen = manifest.cases.find((entry) => entry.id === caseEntry.case_id);
    assert.ok(caseEntry.materialization.target.executed);
    assert.ok(caseEntry.materialization.negative.executed);
    assert.ok(caseEntry.materialization.target.files.every((entry) =>
      /^[0-9a-f]{64}$/.test(entry.source_sha256)));
    assert.deepEqual(
      caseEntry.target.core.commands.map((entry) => entry.id),
      frozen.core_commands.map((entry) => entry.id),
    );
    assert.deepEqual(
      caseEntry.target.oracle.commands.map((entry) => entry.id),
      frozen.oracle_commands.map((entry) => entry.id),
    );
    assert.deepEqual(
      caseEntry.negative.core.commands.map((entry) => entry.id),
      frozen.core_commands.map((entry) => entry.id),
    );
    assert.deepEqual(
      caseEntry.negative.oracle.commands.map((entry) => entry.id),
      frozen.oracle_commands.map((entry) => entry.id),
    );
    assert.equal(caseEntry.target.core.evidence, "executed");
    assert.equal(caseEntry.target.oracle.evidence, "executed");
    assert.equal(caseEntry.negative.core.evidence, "executed");
    assert.equal(caseEntry.negative.oracle.evidence, "executed");
    for (const execution of [
      ...caseEntry.target.core.commands,
      ...caseEntry.target.oracle.commands,
      ...caseEntry.negative.core.commands,
      ...caseEntry.negative.oracle.commands,
    ]) {
      assert.match(execution.command_template_sha256, /^[0-9a-f]{64}$/);
      assert.match(execution.resolved_command_sha256, /^[0-9a-f]{64}$/);
      assert.equal(typeof execution.stdout.bytes, "number");
      assert.match(execution.stdout.sha256, /^[0-9a-f]{64}$/);
      assert.equal(typeof execution.stderr.bytes, "number");
      assert.match(execution.stderr.sha256, /^[0-9a-f]{64}$/);
      assert.equal(
        execution.command_supervision?.deadline_seconds ??
          execution.process_supervision.deadline_seconds,
        1800,
      );
      assert.equal(execution.process_supervision.termination_grace_seconds, 10);
      assert.equal(execution.process_supervision.process_group_extinct_before_capture, true);
      if (execution.execution_mode === "trusted_cargo_direct_libtest") {
        assert.deepEqual(execution.locked_files.before, execution.locked_files.after);
        assert.ok(execution.locked_files.before.length > 0);
        assert.ok(execution.locked_files.before.every((entry) =>
          /^[0-9a-f]{64}$/.test(entry.sha256)));
        assert.match(execution.toolchain.rustc.sha256, /^[0-9a-f]{64}$/);
      }
    }
    for (const oracle of [caseEntry.target.oracle, caseEntry.negative.oracle]) {
      assert.ok(oracle.runner_process_supervision.deadline_seconds > 1800);
      assert.equal(oracle.runner_process_supervision.termination_grace_seconds, 10);
      assert.equal(oracle.runner_process_supervision.process_group_extinct_before_capture, true);
    }
    for (const assertion of [
      ...caseEntry.target.oracle.assertions,
      ...caseEntry.negative.oracle.assertions,
    ]) {
      if (assertion.selector_kind === "rust_test_exact") {
        assert.match(assertion.normalized_events_sha256, /^[0-9a-f]{64}$/);
        assert.match(assertion.normalized_suite_sha256, /^[0-9a-f]{64}$/);
      } else if (assertion.selector_kind === "json_pointer" && assertion.found) {
        assert.match(assertion.actual_canonical_sha256, /^[0-9a-f]{64}$/);
      } else if (assertion.selector_kind === "portable_record_set") {
        assert.match(assertion.selected_native_sha256, /^[0-9a-f]{64}$/);
        assert.match(assertion.selected_wasm_sha256, /^[0-9a-f]{64}$/);
      }
    }
  }

  for (const id of ["TW-01", "TW-02", "TW-06"]) {
    const entry = receipt.run_receipt.cases.find((candidate) => candidate.case_id === id);
    assert.equal(entry.qualification, "packet_gate_only");
    assert.equal(entry.machine_semantic_discrimination_qualified, false);
  }
  for (const id of ["TW-03", "TW-04", "TW-07", "TW-08", "TW-09"]) {
    const entry = receipt.run_receipt.cases.find((candidate) => candidate.case_id === id);
    assert.equal(entry.target.accepted, true);
    assert.equal(entry.negative.discriminated, true);
  }
  const tw05 = receipt.run_receipt.cases.find((entry) => entry.case_id === "TW-05");
  for (const offline of [tw05.offline_historical_target, tw05.offline_behavior_missing_negative]) {
    assert.ok(offline.process_supervision.deadline_seconds > 1800);
    assert.equal(offline.process_supervision.termination_grace_seconds, 10);
    assert.equal(offline.process_supervision.process_group_extinct_before_capture, true);
  }
  assert.equal(tw05.target.accepted, false);
  assert.equal(tw05.target.expected_contract_miss, true);
  assert.equal(tw05.reference_positive.accepted, true);
  assert.equal(tw05.negative.discriminated, true);
  assert.deepEqual(
    tw05.target.oracle.assertions.filter((entry) => !entry.pass).map((entry) => entry.id),
    ["tw-05.native-step-4", "tw-05.native-step-5"],
  );
  assert.equal(tw05.target.adapter_execution.command_exit_code, 0);
  assert.match(tw05.target.adapter_execution.stdout.sha256, /^[0-9a-f]{64}$/);
  assert.match(tw05.target.adapter_execution.stderr.sha256, /^[0-9a-f]{64}$/);
  assert.match(tw05.target.adapter_execution.observation_artifact.sha256, /^[0-9a-f]{64}$/);
  assert.equal(tw05.offline_historical_target.network_enforcement.probe_denied, true);
  assert.equal(tw05.offline_historical_target.package_manager_dependency, false);
  for (const offline of [
    tw05.offline_historical_target,
    tw05.offline_behavior_missing_negative,
  ]) {
    for (const execution of offline.executions) {
      assert.equal(typeof execution.stdout.bytes, "number");
      assert.match(execution.stdout.sha256, /^[0-9a-f]{64}$/);
      assert.equal(typeof execution.stderr.bytes, "number");
      assert.match(execution.stderr.sha256, /^[0-9a-f]{64}$/);
    }
  }

  const tw09 = receipt.run_receipt.cases.find((entry) => entry.case_id === "TW-09");
  assert.equal(tw09.materialization.target.kind, "trusted_rebased_replay_positive");
  assert.equal(tw09.materialization.target.tree, "82854a472bd6aca1cab70b750fdcae864675ce5c");
  assert.equal(tw09.target.adapter_execution.kind, "production_probe");
  assert.ok(receipt.run_receipt.families.every((entry) => entry.evidence === "executed"));
  assert.equal(receipt.payload.cases.length, 9);
  assert.doesNotMatch(
    JSON.stringify(receipt.payload),
    /\/var\/folders|exec_time|duration_ms/,
  );
});

async function reviewTreeManifest(root) {
  const entries = [];
  async function walk(directory, prefix = "") {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
      if (entry.isDirectory()) {
        await walk(resolve(directory, entry.name), relative);
      } else {
        assert.equal(entry.isFile(), true, `${relative} must be a regular file`);
        const bytes = await readFile(resolve(directory, entry.name));
        entries.push({ path: relative, bytes: bytes.length, sha256: sha256(bytes) });
      }
    }
  }
  await walk(root);
  return entries.sort((left, right) => Buffer.from(left.path).compare(Buffer.from(right.path)));
}

async function writeReviewInputManifest(inputRoot, manifestPath, roleByPath) {
  const artifacts = [];
  for (const [path, role] of Object.entries(roleByPath)) {
    const bytes = await readFile(resolve(inputRoot, path));
    artifacts.push({
      path,
      roles: Array.isArray(role) ? role : [role],
      bytes: bytes.length,
      sha256: sha256(bytes),
    });
  }
  artifacts.sort((left, right) => Buffer.from(left.path).compare(Buffer.from(right.path)));
  await writeFile(
    manifestPath,
    `${JSON.stringify({schema: "tachiko-review-packet-input-v1", artifacts}, null, 2)}\n`,
  );
}

async function writeFormalControllerContext(path, overrides = {}) {
  const context = {
    schema: "tachiko-controller-evidence-context-v1",
    protocol_id: "tachiko-agents-effect-v1",
    phase: "baseline_a",
    classification: "formal_authorized_attempt",
    formal_result_eligible: true,
    wave_id: "1".repeat(32), run_id: "2".repeat(32), attempt_id: "3".repeat(32),
    candidate_id: "0123456789abcdef0123456789abcdef", case_id: "TW-05",
    capture_receipt_sha256: "4".repeat(64), formal_authorization_sha256: "5".repeat(64),
    adapter_forbidden_roots: [],
    ...overrides,
  };
  const bytes = Buffer.from(`${JSON.stringify(context, null, 2)}\n`);
  await writeFile(path, bytes);
  return {path, sha256: sha256(bytes), context, bytes};
}

function runReviewPacketBuilder({
  inputRoot, inputManifest, outputDir, terminalReceipt, variants, controllerContext,
  requireFormalContext = false,
}) {
  const argumentsForBuilder = [
    buildReviewPacketScript,
    "--case-id", "TW-05",
    "--candidate-id", "0123456789abcdef0123456789abcdef",
    "--input-root", inputRoot,
    "--input-manifest", inputManifest,
    "--contract", resolve(benchmarkDir, "evaluator/contracts/review-packet-blinding-v1.json"),
    "--output-dir", outputDir,
    "--terminal-receipt", terminalReceipt,
    "--custodian-id", "internal-custodian-01",
    "--custodian-eligible", "true",
    "--frozen-at", "2026-08-25T00:00:00.000Z",
  ];
  if (controllerContext) argumentsForBuilder.push(
    "--controller-context", controllerContext.path,
    "--expected-controller-context-sha256", controllerContext.sha256,
  );
  if (requireFormalContext) argumentsForBuilder.push("--require-formal-context", "true");
  for (const variant of variants) argumentsForBuilder.push("--variant", variant);
  return spawnSync(process.execPath, argumentsForBuilder, { encoding: "utf8" });
}

async function writeStandaloneReviewPacket(packetDir, variantPath, overrides = {}) {
  const roles = [
    "task",
    "authority",
    "candidate_checkout",
    "candidate_diff",
    "candidate_validation",
    "final_message",
  ];
  const artifacts = [];
  for (const role of roles) {
    const path = `${role}.txt`;
    const value = overrides[role] ?? `${role} ordinary review evidence\n`;
    const bytes = Buffer.isBuffer(value) ? value : Buffer.from(value);
    await writeFile(resolve(packetDir, path), bytes);
    artifacts.push({
      display_path: path,
      path_redacted: false,
      review_role: role,
      original_path_sha256: sha256(Buffer.from(path)),
      pre_render_bytes: bytes.length,
      pre_render_sha256: sha256(bytes),
      rendered_bytes: bytes.length,
      rendered_sha256: sha256(bytes),
    });
  }
  artifacts.sort((left, right) =>
    Buffer.from(left.display_path).compare(Buffer.from(right.display_path)),
  );
  const contractPath = resolve(
    benchmarkDir,
    "evaluator/contracts/review-packet-blinding-v1.json",
  );
  const contractBytes = await readFile(contractPath);
  const contract = JSON.parse(contractBytes.toString("utf8"));
  const variantBytes = await readFile(variantPath);
  const identities = [{bytes: variantBytes.length, sha256: sha256(variantBytes)}];
  const manifest = {
    schema: "tachiko-review-packet-public-manifest-v1",
    protocol_id: "tachiko-agents-effect-v1",
    case_id: "TW-05",
    candidate_id: "0123456789abcdef0123456789abcdef",
    frozen_at: "2026-08-25T00:00:00.000Z",
    contract_sha256: sha256(contractBytes),
    rule_set_commitment_sha256: sha256(
      Buffer.from(`${JSON.stringify(contract.machine_match_rules, null, 2)}\n`),
    ),
    variant_set_commitment_sha256: sha256(
      Buffer.from(`${JSON.stringify(identities, null, 2)}\n`),
    ),
    input_manifest_sha256: sha256(Buffer.from("trusted input manifest fixture")),
    artifacts,
  };
  await writeFile(
    resolve(packetDir, "packet-manifest.json"),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );
}

test("formal review builder and standalone scanner require the same immutable controller context", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-formal-review-context-"));
  try {
    const inputRoot = resolve(fixtureRoot, "input");
    const inputManifest = resolve(fixtureRoot, "input-manifest.json");
    const variant = resolve(fixtureRoot, "variant");
    const context = await writeFormalControllerContext(resolve(fixtureRoot, "context.json"));
    await mkdir(inputRoot);
    const roleByPath = {};
    for (const role of [
      "task", "authority", "candidate_checkout", "candidate_diff",
      "candidate_validation", "final_message",
    ]) {
      const path = `${role}.txt`;
      roleByPath[path] = role;
      await writeFile(resolve(inputRoot, path), `${role} evidence\n`);
    }
    await writeFile(variant, "sealed formal variant text\n");
    await writeReviewInputManifest(inputRoot, inputManifest, roleByPath);

    const missing = runReviewPacketBuilder({
      inputRoot, inputManifest, outputDir: resolve(fixtureRoot, "missing-output"),
      terminalReceipt: resolve(fixtureRoot, "missing-terminal.json"), variants: [variant],
      requireFormalContext: true,
    });
    assert.notEqual(missing.status, 0);
    assert.match(missing.stderr, /controller context is required/i);

    const tampered = runReviewPacketBuilder({
      inputRoot, inputManifest, outputDir: resolve(fixtureRoot, "tampered-output"),
      terminalReceipt: resolve(fixtureRoot, "tampered-terminal.json"), variants: [variant],
      controllerContext: {...context, sha256: "f".repeat(64)}, requireFormalContext: true,
    });
    assert.notEqual(tampered.status, 0);
    assert.match(tampered.stderr, /controller context SHA-256 mismatch/i);

    const outputDir = resolve(fixtureRoot, "formal-output");
    const terminalPath = resolve(fixtureRoot, "formal-terminal.json");
    const built = runReviewPacketBuilder({
      inputRoot, inputManifest, outputDir, terminalReceipt: terminalPath, variants: [variant],
      controllerContext: context, requireFormalContext: true,
    });
    assert.equal(built.status, 0, built.stderr);
    const buildReceipt = await readJson(resolve(outputDir, "receipt.json"));
    const terminal = await readJson(terminalPath);
    for (const receipt of [buildReceipt, terminal]) {
      assert.equal(receipt.classification, "formal_authorized_attempt");
      assert.equal(receipt.formal_result_eligible, true);
      assert.equal(receipt.controller_context_sha256, context.sha256);
      assert.equal(receipt.attempt_id, context.context.attempt_id);
    }

    const scanReceiptPath = resolve(fixtureRoot, "formal-scan.json");
    const scanned = spawnSync(process.execPath, [
      scanReviewPacketScript, "--packet-dir", resolve(outputDir, "packet"),
      "--contract", resolve(benchmarkDir, "evaluator/contracts/review-packet-blinding-v1.json"),
      "--variant", variant, "--receipt", scanReceiptPath,
      "--controller-context", context.path,
      "--expected-controller-context-sha256", context.sha256,
      "--require-formal-context", "true",
    ], {encoding: "utf8"});
    assert.equal(scanned.status, 0, scanned.stderr);
    const scanReceipt = await readJson(scanReceiptPath);
    assert.equal(scanReceipt.classification, "formal_authorized_attempt");
    assert.equal(scanReceipt.formal_result_eligible, true);
    assert.equal(scanReceipt.controller_context_sha256, context.sha256);
  } finally {
    await rm(fixtureRoot, {recursive: true, force: true});
  }
});

test("review packet deterministically applies frozen R1-R4 while preserving domain text", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-review-packet-"));
  try {
    const inputRoot = resolve(fixtureRoot, "captured-review-input");
    const variantA = resolve(fixtureRoot, "sealed-a");
    const variantB = resolve(fixtureRoot, "sealed-b");
    const firstOutput = resolve(fixtureRoot, "packet-one");
    const secondOutput = resolve(fixtureRoot, "packet-two");
    const copied =
      "Preserve the repository declared runtime package manager lockfile formatter linter build system and dependency workflow exactly today.";
    const nearSource = "alpha beta gamma delta epsilon zeta theta omega";
    await mkdir(resolve(inputRoot, "notes"), { recursive: true });
    await writeFile(variantA, `${copied}\n${nearSource}\n`);
    await writeFile(variantB, "Keep every accepted project boundary explicit and independently reviewable.\n");
    await writeFile(resolve(inputRoot, "notes", "exact.txt"), `${copied}\n`);
    await writeFile(
      resolve(inputRoot, "notes", "case-whitespace.txt"),
      "PRESERVE  THE REPOSITORY DECLARED RUNTIME PACKAGE MANAGER LOCKFILE FORMATTER LINTER BUILD SYSTEM AND DEPENDENCY WORKFLOW EXACTLY TODAY.\n",
    );
    await writeFile(
      resolve(inputRoot, "notes", "near-copy.txt"),
      "alpha beta gamma delta epsilxn zeta theta omega\n",
    );
    await writeFile(
      resolve(inputRoot, "notes", "content-ref.txt"),
      "This line explicitly mentions a DEVELOPER INSTRUCTION during review.\n",
    );
    await writeFile(
      resolve(inputRoot, "notes", "ordinary-TW-05.txt"),
      "TW-05 resident runtime parity evidence remains unchanged.\n",
    );
    await writeFile(
      resolve(inputRoot, "notes", "AGENTS.md-copy.txt"),
      "The final message explicitly names Baseline A and must be rendered.\n",
    );
    await writeFile(
      resolve(inputRoot, "notes", "bom.txt"),
      Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), Buffer.from("ordinary BOM text\n")]),
    );
    const inputManifest = resolve(fixtureRoot, "trusted-input-manifest.json");
    await writeReviewInputManifest(inputRoot, inputManifest, {
      "notes/exact.txt": "task",
      "notes/case-whitespace.txt": "authority",
      "notes/near-copy.txt": "candidate_checkout",
      "notes/bom.txt": "candidate_checkout",
      "notes/content-ref.txt": "candidate_diff",
      "notes/ordinary-TW-05.txt": "candidate_validation",
      "notes/AGENTS.md-copy.txt": "final_message",
    });

    const first = runReviewPacketBuilder({
      inputRoot,
      inputManifest,
      outputDir: firstOutput,
      terminalReceipt: resolve(fixtureRoot, "terminal-one.json"),
      variants: [variantA, variantB],
    });
    assert.equal(first.status, 0, first.stderr);
    const second = runReviewPacketBuilder({
      inputRoot,
      inputManifest,
      outputDir: secondOutput,
      terminalReceipt: resolve(fixtureRoot, "terminal-two.json"),
      variants: [variantB, variantA],
    });
    assert.equal(second.status, 0, second.stderr);

    assert.deepEqual(
      await reviewTreeManifest(firstOutput),
      await reviewTreeManifest(secondOutput),
      "same frozen inputs must regenerate byte-identical packet and receipts",
    );

    const receipt = await readJson(resolve(firstOutput, "receipt.json"));
    assert.equal(receipt.schema, "tachiko-review-packet-receipt-v1");
    assert.equal(receipt.classification, "construction_pilot_only");
    assert.equal(receipt.formal_result_eligible, false);
    assert.equal(receipt.safe_to_release, true);
    assert.equal(receipt.semantic_scoring_performed, false);
    assert.equal(receipt.post_render_scan.match_count, 0);
    assert.equal(receipt.post_render_scan.safe_to_release, true);
    assert.match(receipt.contract.sha256, /^[0-9a-f]{64}$/);
    assert.match(receipt.variant_set.commitment_sha256, /^[0-9a-f]{64}$/);
    assert.equal(receipt.variant_set.count, 2);
    assert.match(receipt.private_match_map_sha256, /^[0-9a-f]{64}$/);
    assert.match(receipt.rendered_packet_sha256, /^[0-9a-f]{64}$/);
    assert.match(receipt.input_manifest_sha256, /^[0-9a-f]{64}$/);
    assert.notEqual(
      receipt.final_message.raw_sha256,
      receipt.final_message.redacted_sha256,
    );
    assert.ok(receipt.match_counts_by_rule.R1 >= 2);
    assert.ok(receipt.match_counts_by_rule.R2 >= 1);
    assert.ok(receipt.match_counts_by_rule.R3 >= 1);
    assert.ok(receipt.match_counts_by_rule.R4 >= 1);

    const placeholder = "[instruction-reference redacted]\n";
    for (const path of ["exact.txt", "case-whitespace.txt", "near-copy.txt", "content-ref.txt"]) {
      assert.equal(
        await readFile(resolve(firstOutput, "packet", "notes", path), "utf8"),
        placeholder,
      );
    }
    assert.equal(
      await readFile(resolve(firstOutput, "packet", "notes", "ordinary-TW-05.txt"), "utf8"),
      "TW-05 resident runtime parity evidence remains unchanged.\n",
    );
    const sensitiveRelativePath = "notes/AGENTS.md-copy.txt";
    const redactedPath = `redacted-path-${sha256(Buffer.from(sensitiveRelativePath))}`;
    assert.equal(
      await readFile(resolve(firstOutput, "packet", redactedPath), "utf8"),
      placeholder,
    );
    assert.deepEqual(
      await readFile(resolve(firstOutput, "packet", "notes", "bom.txt")),
      Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), Buffer.from("ordinary BOM text\n")]),
    );

    const privateMap = await readJson(resolve(firstOutput, "private-match-map.json"));
    assert.ok(privateMap.events.length >= 5);
    for (const event of privateMap.events) {
      assert.deepEqual(Object.keys(event).sort(), [
        "line_number",
        "opaque_path_alias",
        "original_artifact_sha256",
        "post_sha256",
        "pre_sha256",
        "rule_id",
      ]);
      assert.doesNotMatch(JSON.stringify(event), /Preserve|AGENTS\.md|Baseline A/);
    }
    const publicManifest = await readJson(
      resolve(firstOutput, "packet", "packet-manifest.json"),
    );
    assert.equal(publicManifest.artifacts.length, 7);
    assert.equal(publicManifest.variant_set_commitment_sha256, receipt.variant_set.commitment_sha256);
    assert.equal(publicManifest.contract_sha256, receipt.contract.sha256);
    assert.equal(JSON.stringify(publicManifest).includes("match_counts"), false);
    const displayedPaths = publicManifest.artifacts.map((entry) => entry.display_path);
    assert.deepEqual(
      displayedPaths,
      [...displayedPaths].sort((left, right) => Buffer.from(left).compare(Buffer.from(right))),
    );
  } finally {
    await rm(fixtureRoot, { recursive: true, force: true });
  }
});

test("review packet fails closed for binary, symlinked, or overlapping production paths", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-review-reject-"));
  try {
    const inputRoot = resolve(fixtureRoot, "input");
    const variant = resolve(fixtureRoot, "sealed-variant");
    await mkdir(inputRoot);
    await writeFile(variant, "one two three four five six seven eight long variant instruction line\n");
    await writeFile(resolve(inputRoot, "binary.dat"), Buffer.from([0xff, 0xfe, 0x00, 0x01]));
    for (const path of ["task.txt", "authority.txt", "checkout.txt", "diff.txt", "validation.txt"]) {
      await writeFile(resolve(inputRoot, path), `${path} ordinary evidence\n`);
    }
    const binaryManifest = resolve(fixtureRoot, "binary-manifest.json");
    await writeReviewInputManifest(inputRoot, binaryManifest, {
      "authority.txt": "authority",
      "binary.dat": "final_message",
      "checkout.txt": "candidate_checkout",
      "diff.txt": "candidate_diff",
      "task.txt": "task",
      "validation.txt": "candidate_validation",
    });
    let result = runReviewPacketBuilder({
      inputRoot,
      inputManifest: binaryManifest,
      outputDir: resolve(fixtureRoot, "binary-output"),
      terminalReceipt: resolve(fixtureRoot, "binary-terminal.json"),
      variants: [variant],
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /strict UTF-8|binary/i);
    let terminal = await readJson(resolve(fixtureRoot, "binary-terminal.json"));
    assert.equal(terminal.terminal_classification, "invalid_discarded");
    assert.equal(terminal.safe_to_release, false);

    await unlink(resolve(inputRoot, "binary.dat"));
    for (const path of ["task.txt", "authority.txt", "checkout.txt", "diff.txt", "validation.txt"]) {
      await unlink(resolve(inputRoot, path));
    }
    await writeFile(resolve(fixtureRoot, "outside.txt"), "ordinary\n");
    await symlink(resolve(fixtureRoot, "outside.txt"), resolve(inputRoot, "linked.txt"));
    const linkedInputManifest = resolve(fixtureRoot, "linked-input-manifest.json");
    await writeFile(
      linkedInputManifest,
      `${JSON.stringify({
        schema: "tachiko-review-packet-input-v1",
        artifacts: [{
          path: "linked.txt",
          roles: [
            "task",
            "authority",
            "candidate_checkout",
            "candidate_diff",
            "candidate_validation",
            "final_message",
          ],
          bytes: 9,
          sha256: sha256(Buffer.from("ordinary\n")),
        }],
      }, null, 2)}\n`,
    );
    result = runReviewPacketBuilder({
      inputRoot,
      inputManifest: linkedInputManifest,
      outputDir: resolve(fixtureRoot, "symlink-output"),
      terminalReceipt: resolve(fixtureRoot, "symlink-terminal.json"),
      variants: [variant],
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /symlink|regular file/i);

    await unlink(resolve(inputRoot, "linked.txt"));
    await writeFile(resolve(inputRoot, "ordinary.txt"), "ordinary\n");
    const ordinaryManifest = resolve(fixtureRoot, "ordinary-manifest.json");
    await writeReviewInputManifest(inputRoot, ordinaryManifest, {
      "ordinary.txt": [
        "task",
        "authority",
        "candidate_checkout",
        "candidate_diff",
        "candidate_validation",
        "final_message",
      ],
    });
    result = runReviewPacketBuilder({
      inputRoot,
      inputManifest: ordinaryManifest,
      outputDir: resolve(inputRoot, "nested-output"),
      terminalReceipt: resolve(fixtureRoot, "overlap-terminal.json"),
      variants: [variant],
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /disjoint|overlap/i);

    const variantLink = resolve(fixtureRoot, "variant-link");
    await symlink(variant, variantLink);
    result = runReviewPacketBuilder({
      inputRoot,
      inputManifest: ordinaryManifest,
      outputDir: resolve(fixtureRoot, "linked-variant-output"),
      terminalReceipt: resolve(fixtureRoot, "linked-variant-terminal.json"),
      variants: [variantLink],
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /symlink/i);

    const incompleteManifest = resolve(fixtureRoot, "incomplete-manifest.json");
    await writeReviewInputManifest(inputRoot, incompleteManifest, {
      "ordinary.txt": "task",
    });
    result = runReviewPacketBuilder({
      inputRoot,
      inputManifest: incompleteManifest,
      outputDir: resolve(fixtureRoot, "incomplete-output"),
      terminalReceipt: resolve(fixtureRoot, "incomplete-terminal.json"),
      variants: [variant],
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /required review artifact role/i);
    terminal = await readJson(resolve(fixtureRoot, "incomplete-terminal.json"));
    assert.equal(terminal.terminal_classification, "invalid_discarded");

    const conflatedManifest = resolve(fixtureRoot, "conflated-manifest.json");
    await writeReviewInputManifest(inputRoot, conflatedManifest, {
      "ordinary.txt": [
        "task",
        "authority",
        "candidate_checkout",
        "candidate_diff",
        "candidate_validation",
        "final_message",
      ],
    });
    result = runReviewPacketBuilder({
      inputRoot,
      inputManifest: conflatedManifest,
      outputDir: resolve(fixtureRoot, "conflated-output"),
      terminalReceipt: resolve(fixtureRoot, "conflated-terminal.json"),
      variants: [variant],
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /exactly one review artifact role/i);

    const forbiddenTerminal = resolve(inputRoot, "forbidden-terminal.json");
    result = runReviewPacketBuilder({
      inputRoot,
      inputManifest: ordinaryManifest,
      outputDir: resolve(fixtureRoot, "forbidden-terminal-output"),
      terminalReceipt: forbiddenTerminal,
      variants: [variant],
    });
    assert.notEqual(result.status, 0);
    assert.equal(existsSync(forbiddenTerminal), false);
  } finally {
    await rm(fixtureRoot, { recursive: true, force: true });
  }
});

test("review packet scanner rejects residual matches and qualifies ordinary subjective evidence", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-review-scan-"));
  try {
    const variant = resolve(fixtureRoot, "sealed-variant");
    const incompletePacket = resolve(fixtureRoot, "incomplete-packet");
    const unsafePacket = resolve(fixtureRoot, "unsafe-packet");
    const safePacket = resolve(fixtureRoot, "safe-packet");
    await writeFile(
      variant,
      "Preserve the repository runtime manager lockfile formatter linter build workflow exactly.\n",
    );
    await Promise.all([mkdir(incompletePacket), mkdir(unsafePacket), mkdir(safePacket)]);
    await writeFile(resolve(incompletePacket, "review.txt"), "ordinary safe text\n");
    await writeStandaloneReviewPacket(unsafePacket, variant, {
      task: "Baseline A must never remain visible.\n",
    });
    await writeStandaloneReviewPacket(safePacket, variant, {
      task: "The candidate explains runtime ownership and validates the requested behavior.\n",
    });
    const common = [
      "--contract", resolve(benchmarkDir, "evaluator/contracts/review-packet-blinding-v1.json"),
      "--variant", variant,
    ];
    const incompleteReceipt = resolve(fixtureRoot, "incomplete-scan.json");
    let result = spawnSync(
      process.execPath,
      [
        scanReviewPacketScript,
        "--packet-dir", incompletePacket,
        "--receipt", incompleteReceipt,
        ...common,
      ],
      { encoding: "utf8" },
    );
    assert.notEqual(result.status, 0);
    const incomplete = await readJson(incompleteReceipt);
    assert.equal(incomplete.safe_to_release, false);
    assert.equal(incomplete.terminal_classification, "invalid_discarded");

    const unsafeReceipt = resolve(fixtureRoot, "unsafe-scan.json");
    result = spawnSync(
      process.execPath,
      [scanReviewPacketScript, "--packet-dir", unsafePacket, "--receipt", unsafeReceipt, ...common],
      { encoding: "utf8" },
    );
    assert.notEqual(result.status, 0);
    const rejected = await readJson(unsafeReceipt);
    assert.equal(rejected.safe_to_release, false);
    assert.equal(rejected.terminal_classification, "invalid_discarded");
    assert.ok(rejected.match_count > 0);

    const safeReceipt = resolve(fixtureRoot, "safe-scan.json");
    result = spawnSync(
      process.execPath,
      [scanReviewPacketScript, "--packet-dir", safePacket, "--receipt", safeReceipt, ...common],
      { encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr);
    const accepted = await readJson(safeReceipt);
    assert.equal(accepted.safe_to_release, true);
    assert.equal(accepted.match_count, 0);
    assert.equal(accepted.semantic_scoring_performed, false);
    assert.equal(accepted.qualification, "subjective_packet_transport_only");

    const binaryPacket = resolve(fixtureRoot, "binary-packet");
    const binaryScanReceipt = resolve(fixtureRoot, "binary-scan.json");
    await mkdir(binaryPacket);
    await writeStandaloneReviewPacket(binaryPacket, variant, {
      final_message: Buffer.from([0xff, 0x00]),
    });
    result = spawnSync(
      process.execPath,
      [scanReviewPacketScript, "--packet-dir", binaryPacket, "--receipt", binaryScanReceipt, ...common],
      { encoding: "utf8" },
    );
    assert.notEqual(result.status, 0);
    const binaryRejected = await readJson(binaryScanReceipt);
    assert.equal(binaryRejected.safe_to_release, false);
    assert.equal(binaryRejected.terminal_classification, "invalid_discarded");
    assert.equal(binaryRejected.failure, "scan_failed");

    const mutatedContract = resolve(fixtureRoot, "mutated-contract.json");
    const contract = await readJson(
      resolve(benchmarkDir, "evaluator/contracts/review-packet-blinding-v1.json"),
    );
    contract.scope = `${contract.scope} changed`;
    await writeFile(mutatedContract, `${JSON.stringify(contract, null, 2)}\n`);
    result = spawnSync(
      process.execPath,
      [
        scanReviewPacketScript,
        "--packet-dir", safePacket,
        "--receipt", resolve(fixtureRoot, "mutated-contract-scan.json"),
        "--contract", mutatedContract,
        "--variant", variant,
      ],
      { encoding: "utf8" },
    );
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /frozen.*sha-256|hash/i);

    const linkedVariant = resolve(fixtureRoot, "linked-variant");
    await symlink(variant, linkedVariant);
    result = spawnSync(
      process.execPath,
      [
        scanReviewPacketScript,
        "--packet-dir", safePacket,
        "--receipt", resolve(fixtureRoot, "linked-scan.json"),
        "--contract", resolve(benchmarkDir, "evaluator/contracts/review-packet-blinding-v1.json"),
        "--variant", linkedVariant,
      ],
      { encoding: "utf8" },
    );
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /symlink/i);
  } finally {
    await rm(fixtureRoot, { recursive: true, force: true });
  }
});

async function controllerStageReceipts(artifactDir) {
  const receiptDir = resolve(artifactDir, "stage-receipts");
  const names = (await readdir(receiptDir)).sort();
  const receipts = [];
  let prior = null;
  for (const name of names) {
    const bytes = await readFile(resolve(receiptDir, name));
    const receipt = JSON.parse(bytes.toString("utf8"));
    assert.equal(receipt.stage_order, receipts.length);
    assert.equal(receipt.prior_receipt_sha256, prior);
    prior = sha256(bytes);
    receipts.push(receipt);
  }
  return receipts;
}

async function createControllerSmokeFixture(mode, caseId = "TW-01") {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-controller-"));
  const runRoot = resolve(fixtureRoot, "r-0123456789abcdef0123456789abcdef");
  const artifactDir = resolve(fixtureRoot, "trusted-controller-artifacts");
  const fakeAgent = resolve(fixtureRoot, "fake-agent.mjs");
  const agentArgs = resolve(fixtureRoot, "agent-args.json");
  const attemptRegistryDir = resolve(fixtureRoot, "attempt-registry");
  const variantFile = resolve(repositoryRoot, "AGENTS.md");
  const variantBytes = await readFile(variantFile);
  await mkdir(attemptRegistryDir);
  await writeFile(
    fakeAgent,
    `#!/usr/bin/env node\n` +
      `import {appendFileSync, writeFileSync} from "node:fs";\n` +
      `import {spawn} from "node:child_process";\n` +
      `const mode = process.argv[2];\n` +
      `const launchPath = mode === "noop" || mode === "timeout_no_changes" ` +
      `? process.argv[3] : ".fake-agent-launches";\n` +
      `appendFileSync(launchPath, "1\\n");\n` +
      `if (mode !== "noop" && mode !== "timeout_no_changes") ` +
      `writeFileSync("controller-smoke.txt", "candidate mutation\\n");\n` +
      `if (mode === "descendant" || mode === "timeout_descendant") {\n` +
      `  const childCode = mode === "timeout_descendant" ` +
      `? "process.on('SIGTERM',()=>{}); setInterval(() => {}, 1000)" ` +
      `: "setInterval(() => {}, 1000)";\n` +
      `  const child = spawn(process.execPath, ["-e", childCode], {stdio:"ignore"});\n` +
      `  child.unref();\n` +
      `  writeFileSync(".descendant-pid", String(child.pid));\n` +
      `}\n` +
      `console.log(JSON.stringify({type:"item.completed",item:{type:"agent_message",text:"construction smoke complete"}}));\n` +
      `if (mode === "failure") process.exit(7);\n` +
      `if (mode === "timeout" || mode === "timeout_descendant" || mode === "timeout_no_changes") ` +
      `setInterval(() => {}, 1000);\n`,
    {mode: 0o755},
  );
  await writeFile(agentArgs, `${JSON.stringify([mode, resolve(fixtureRoot, "no-change-launches")])}\n`);
  return {
    fixtureRoot,
    runRoot,
    artifactDir,
    arguments: [
      runControllerScript,
      "--case", caseId,
      "--source-repo", repositoryRoot,
      "--variant-file", variantFile,
      "--expected-variant-sha256", sha256(variantBytes),
      "--phase", "construction_pilot_only",
      "--run-root", runRoot,
      "--artifact-dir", artifactDir,
      "--attempt-registry-dir", attemptRegistryDir,
      "--agent-executable", fakeAgent,
      "--agent-args-file", agentArgs,
      "--timeout-seconds", mode.startsWith("timeout") ? "1" : "10",
      "--wave-id", "11111111111111111111111111111111",
      "--run-id", "22222222222222222222222222222222",
      "--attempt-id", "33333333333333333333333333333333",
      "--candidate-id", "44444444444444444444444444444444",
      "--construction-smoke", "true",
    ],
  };
}

for (const [mode, expectedDisposition, expectedStatus] of [
  ["noop", "awaiting_review", 0],
  ["timeout_no_changes", "agent_timeout", 1],
]) {
  test(`controller scores ${mode} as a task outcome with explicit empty-patch evidence`, async () => {
    const fixture = await createControllerSmokeFixture(mode);
    try {
      const result = spawnSync(process.execPath, fixture.arguments, {
        encoding: "utf8", timeout: 120_000, maxBuffer: 128 * 1024 * 1024,
      });
      assert.equal(result.status, expectedStatus, result.stderr);
      const validation = await readJson(resolve(
        fixture.artifactDir,
        "validation-preparation/validation-preparation-receipt.json",
      ));
      assert.equal(validation.empty_patch, true);
      assert.equal(validation.candidate_patch_bytes, 0);
      assert.equal(existsSync(resolve(fixture.artifactDir, "result-skeleton.json")), true);
      const terminal = await readJson(resolve(fixture.artifactDir, "terminal.json"));
      assert.equal(terminal.disposition, expectedDisposition);
      assert.equal(terminal.launch_count, 1);
    } finally {
      await rm(fixture.fixtureRoot, {recursive: true, force: true});
    }
  });
}

test("validation preparation directly reconstructs an empty trusted patch", async () => {
  const fixture = await createControllerSmokeFixture("noop");
  try {
    const controller = spawnSync(process.execPath, fixture.arguments, {
      encoding: "utf8", timeout: 120_000, maxBuffer: 128 * 1024 * 1024,
    });
    assert.equal(controller.status, 0, controller.stderr);
    const captureDir = resolve(fixture.artifactDir, "candidate-capture");
    const directWorkspace = resolve(fixture.fixtureRoot, "direct-validation-workspace");
    const directTrusted = resolve(fixture.fixtureRoot, "direct-validation-trusted");
    const result = spawnSync(process.execPath, [
      prepareValidationScript, "--case", "TW-01", "--source-repo", repositoryRoot,
      "--patch-file", resolve(captureDir, "candidate.patch"),
      "--capture-receipt", resolve(captureDir, "capture-receipt.json"),
      "--workspace", directWorkspace, "--trusted-dir", directTrusted,
    ], {encoding: "utf8", timeout: 120_000, maxBuffer: 128 * 1024 * 1024});
    assert.equal(result.status, 0, result.stderr);
    const receipt = await readJson(resolve(directTrusted, "validation-preparation-receipt.json"));
    assert.equal(receipt.empty_patch, true);
    assert.equal(receipt.candidate_patch_bytes, 0);
    assert.equal(receipt.capture_to_apply_tree_equal, true);
    const frozenCase = (await readJson(resolve(benchmarkDir, "evaluator/cases.json"))).cases
      .find((entry) => entry.id === "TW-01");
    assert.equal(receipt.candidate_tree, frozenCase.historical_base_tree);
  } finally {
    await rm(fixture.fixtureRoot, {recursive: true, force: true});
  }
});

test("one-shot controller orders same-wave controls, captures one successful launch, and chains every stage", async () => {
  const fixture = await createControllerSmokeFixture("success");
  try {
    const result = spawnSync(process.execPath, fixture.arguments, {
      encoding: "utf8",
      timeout: 120_000,
      maxBuffer: 128 * 1024 * 1024,
    });
    assert.equal(result.status, 0, result.stderr);
    const receipts = await controllerStageReceipts(fixture.artifactDir);
    const stages = receipts.map((entry) => entry.stage);
    assert.ok(stages.indexOf("same_wave_base_control") < stages.indexOf("agent_launch"));
    for (const receipt of receipts) {
      assert.equal(receipt.wave_id, "11111111111111111111111111111111");
      assert.equal(receipt.run_id, "22222222222222222222222222222222");
      assert.equal(receipt.attempt_id, "33333333333333333333333333333333");
      assert.equal(receipt.candidate_id, "44444444444444444444444444444444");
      assert.equal(receipt.case_id, "TW-01");
      assert.match(receipt.control_sha256, /^[0-9a-f]{64}$/);
      assert.match(receipt.environment_identity_sha256, /^[0-9a-f]{64}$/);
    }
    for (const required of [
      "candidate_preflight",
      "agent_launch",
      "agent_process",
      "overlay_identity_postcheck",
      "candidate_capture",
      "validation_preparation",
      "core_validation",
      "production_oracles",
      "review_packet",
      "result_skeleton",
    ]) {
      assert.ok(stages.includes(required), `missing ${required}`);
    }
    assert.equal(
      await readFile(resolve(fixture.runRoot, "workspace", ".fake-agent-launches"), "utf8"),
      "1\n",
    );
    const ledgerLines = (await readFile(resolve(fixture.artifactDir, "attempt-ledger.jsonl"), "utf8"))
      .trim().split("\n").map(JSON.parse);
    assert.equal(ledgerLines.length, 2);
    assert.equal(ledgerLines[0].disposition, "registered");
    assert.equal(ledgerLines[1].disposition, "awaiting_review");
    assert.equal(ledgerLines[1].previous_attempt_entry_sha256, ledgerLines[0].entry_sha256);
    const base = await readJson(resolve(fixture.artifactDir, "base-control-receipt.json"));
    assert.equal(base.classification, "same_wave_construction_control");
    assert.equal(base.wave_id, ledgerLines[0].wave_id);
    assert.equal(base.attempt_id, ledgerLines[0].attempt_id);
    assert.equal(base.raw_logs_embedded, false);
    const environmentReceipt = await readJson(resolve(fixture.artifactDir, "environment-receipt.json"));
    const registeredBash = environmentReceipt.tools.find((tool) => tool.name === "bash");
    assert.equal(base.trusted_shell.sha256, registeredBash.sha256);
    assert.deepEqual(base.trusted_shell.arguments_prefix, ["--noprofile", "--norc", "-c"]);
    assert.equal(base.trusted_shell.qualification_executed, true);
    assert.equal(existsSync(resolve(fixture.artifactDir, "process", "stdout.raw")), true);
    assert.equal(existsSync(resolve(fixture.artifactDir, "process", "stderr.raw")), true);
    assert.equal(
      await readFile(resolve(fixture.artifactDir, "process", "final-message.txt"), "utf8"),
      "construction smoke complete\n",
    );
    const evidenceContext = await readJson(
      resolve(fixture.artifactDir, "controller-evidence-context.json"),
    );
    const canonicalRunRoot = await realpath(fixture.runRoot);
    assert.equal(evidenceContext.adapter_forbidden_roots.includes(canonicalRunRoot), false);
    assert.ok(evidenceContext.adapter_forbidden_roots.includes(resolve(canonicalRunRoot, "workspace")));
    assert.ok(evidenceContext.adapter_forbidden_roots.includes(resolve(canonicalRunRoot, "control")));
    assert.ok(evidenceContext.adapter_forbidden_roots.includes(resolve(canonicalRunRoot, "tmp")));
    assert.deepEqual(evidenceContext.adapter_write_forbidden_roots, [canonicalRunRoot]);
    assert.equal(evidenceContext.adapter_write_allowed_roots.length, 1);
    assert.match(evidenceContext.adapter_write_allowed_roots[0], /\/adapter-tmp-[^/]+$/);
    assert.deepEqual(await readdir(evidenceContext.adapter_write_allowed_roots[0]), []);
    assert.equal(evidenceContext.adapter_tmp_initial_sha256, sha256("[]\n"));
    const duplicateRunRoot = resolve(fixture.fixtureRoot, "r-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    const duplicateArtifactDir = resolve(fixture.fixtureRoot, "duplicate-artifacts");
    const duplicateArguments = [...fixture.arguments];
    duplicateArguments[duplicateArguments.indexOf(fixture.runRoot)] = duplicateRunRoot;
    duplicateArguments[duplicateArguments.indexOf(fixture.artifactDir)] = duplicateArtifactDir;
    duplicateArguments[duplicateArguments.indexOf("22222222222222222222222222222222")] =
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    duplicateArguments[duplicateArguments.indexOf("33333333333333333333333333333333")] =
      "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    duplicateArguments[duplicateArguments.indexOf("44444444444444444444444444444444")] =
      "cccccccccccccccccccccccccccccccc";
    const duplicate = spawnSync(process.execPath, duplicateArguments, {encoding: "utf8"});
    assert.notEqual(duplicate.status, 0);
    assert.match(duplicate.stderr, /wave.*case.*phase|slot.*registered|resampling denied/i);
    assert.equal(existsSync(duplicateRunRoot), false);
    assert.equal(existsSync(duplicateArtifactDir), false);
  } finally {
    await rm(fixture.fixtureRoot, {recursive: true, force: true});
  }
});

test("controller records an external terminal outcome when setup fails after atomic slot registration", async () => {
  const fixture = await createControllerSmokeFixture("success");
  try {
    const missingTemplate = resolve(fixture.fixtureRoot, "missing-cargo-home-template");
    const result = spawnSync(
      process.execPath,
      [...fixture.arguments, "--cargo-home-template", missingTemplate],
      {encoding: "utf8", timeout: 120_000, maxBuffer: 128 * 1024 * 1024},
    );
    assert.notEqual(result.status, 0);
    const registryDir = fixture.arguments[fixture.arguments.indexOf("--attempt-registry-dir") + 1];
    const terminalNames = (await readdir(registryDir)).filter((name) => name.endsWith(".terminal.json"));
    assert.equal(terminalNames.length, 1);
    const terminal = await readJson(resolve(registryDir, terminalNames[0]));
    assert.equal(terminal.disposition, "infrastructure_failed");
    assert.equal(terminal.launch_count, 0);
    assert.equal(terminal.resampling_performed, false);
    assert.match(terminal.attempt_registry_entry_sha256, /^[0-9a-f]{64}$/);
  } finally {
    await rm(fixture.fixtureRoot, {recursive: true, force: true});
  }
});

test("terminal ledger append remains the single commit when marker persistence fails", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-terminal-commit-"));
  try {
    const ledgerPath = resolve(fixtureRoot, "attempt-ledger.jsonl");
    const markerPath = resolve(fixtureRoot, "terminal.json");
    await writeFile(ledgerPath, '{"disposition":"registered"}\n');
    const {commitTerminalEntry} = await import(pathToFileURL(runControllerScript));
    let committed = false;
    const outcome = await commitTerminalEntry({
      ledgerPath,
      markerPath,
      terminal: {disposition: "infrastructure_failed", entry_sha256: "a".repeat(64)},
      onCommitted() { committed = true; },
      markerWriter: async () => { throw new Error("injected marker failure"); },
    });
    const lines = (await readFile(ledgerPath, "utf8")).trim().split("\n").map(JSON.parse);
    assert.equal(lines.length, 2);
    assert.equal(lines[1].disposition, "infrastructure_failed");
    assert.equal(committed, true);
    assert.equal(outcome.marker_written, false);
    assert.match(outcome.marker_error, /injected marker failure/);
  } finally {
    await rm(fixtureRoot, {recursive: true, force: true});
  }
});

test("formal runtime staging locks the catalog semantics and every staged binary identity", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-formal-runtime-"));
  try {
    const {
      comparePreflightToolIdentities,
      inspectFormalCargoHome,
      inspectFormalRuntime,
      stageFormalModelCatalog,
      stageToolBin,
      verifyStagedRuntimeArtifacts,
    } =
      await import(pathToFileURL(runControllerScript));
    const lock = await readJson(resolve(benchmarkDir, "environment-lock.json"));
    const rustupHomeTemplate = await realpath(
      process.env.RUSTUP_HOME ?? resolve(process.env.HOME, ".rustup"),
    );
    const inspected = await inspectFormalRuntime(lock, rustupHomeTemplate);
    for (const name of [
      "bash", "cargo", "cargo-clippy", "codex-code-mode-host", "git", "node", "pnpm",
      "rtk", "rustc", "rustfmt", "rustup",
    ]) {
      assert.ok(inspected.tools.some((tool) => tool.name === name), `missing formal runtime ${name}`);
    }
    assert.match(inspected.identity_sha256, /^[0-9a-f]{64}$/);
    const stagedTools = await stageToolBin(resolve(fixtureRoot, "bin"), inspected);
    assert.equal(stagedTools.identity_sha256, inspected.identity_sha256);
    for (const tool of stagedTools.tools) {
      assert.equal(sha256(await readFile(tool.staged_path)), tool.sha256);
    }
    const stagedPreflight = await verifyStagedRuntimeArtifacts(stagedTools, lock);
    assert.equal(stagedPreflight.all_staged_artifacts_verified, true);
    assert.equal(stagedPreflight.rustup_home_path, stagedTools.rustup_home_path);
    const cargoHome = await inspectFormalCargoHome(lock.offline_dependency_cache.template_path, lock);
    assert.equal(cargoHome.manifest_sha256, lock.offline_dependency_cache.tree_sha256);
    const preflightFixture = await createPreflightFixture();
    try {
      const stagedNode = stagedTools.tools.find((tool) => tool.name === "node").staged_path;
      const preflightResult = runPreflight(preflightFixture, {
        nodeExecutable: stagedNode,
        environment: {
          PATH: `${resolve(fixtureRoot, "bin")}:/usr/bin:/bin:/usr/sbin:/sbin`,
          RUSTUP_HOME: stagedTools.rustup_home_path,
        },
      });
      assert.equal(preflightResult.status, 0, preflightResult.stderr);
      const preflightReceipt = await readJson(preflightFixture.receipt);
      const comparison = await comparePreflightToolIdentities(stagedTools.tools, preflightReceipt, true);
      assert.equal(comparison.all_required_matched, true);
      assert.deepEqual(
        comparison.compared_names,
        ["bash", "cargo", "clippy", "git", "node", "rtk", "rustc", "rustfmt", "rustup"],
      );
    } finally {
      await rm(preflightFixture.fixtureRoot, {recursive: true, force: true});
    }

    const catalog = {
      models: [{slug: "fixture-model", base_instructions: "fixture instructions"}],
    };
    const rawBytes = Buffer.from(`${JSON.stringify(catalog, null, 2)}\n`);
    const canonicalJson = (value) => {
      if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
      if (value && typeof value === "object") {
        return `{${Object.keys(value).sort().map((key) =>
          `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(",")}}`;
      }
      return JSON.stringify(value);
    };
    const source = resolve(fixtureRoot, "trusted-catalog.json");
    const destination = resolve(fixtureRoot, "runtime", "model-catalog.json");
    await writeFile(source, rawBytes);
    const catalogLock = {
      bytes: rawBytes.length,
      raw_sha256: sha256(rawBytes),
      canonical_catalog_sha256: sha256(`${canonicalJson(catalog)}\n`),
      model_record_sha256: sha256(`${canonicalJson(catalog.models[0])}\n`),
      base_instructions_sha256: sha256("fixture instructions\n"),
    };
    const staged = await stageFormalModelCatalog({
      sourcePath: source,
      destinationPath: destination,
      catalogLock,
      modelId: "fixture-model",
    });
    assert.equal(staged.sha256, catalogLock.raw_sha256);
    assert.equal((await stat(destination)).mode & 0o222, 0);
    await writeFile(source, Buffer.concat([rawBytes, Buffer.from(" ")]));
    await assert.rejects(
      stageFormalModelCatalog({
        sourcePath: source,
        destinationPath: resolve(fixtureRoot, "runtime", "tampered.json"),
        catalogLock,
        modelId: "fixture-model",
      }),
      /catalog.*lock|SHA-256|bytes/i,
    );
  } finally {
    await rm(fixtureRoot, {recursive: true, force: true});
  }
});

test("timeout cleanup reuses one TERM deadline when a surviving descendant ignores TERM", async () => {
  const fixture = await createControllerSmokeFixture("timeout_descendant");
  try {
    const result = spawnSync(process.execPath, fixture.arguments, {
      encoding: "utf8",
      timeout: 120_000,
      maxBuffer: 128 * 1024 * 1024,
    });
    assert.notEqual(result.status, 0);
    const receipt = await readJson(resolve(fixture.artifactDir, "process", "receipt.json"));
    assert.equal(receipt.timed_out, true);
    assert.equal(receipt.deadline_seconds, 1);
    assert.equal(receipt.descendant_cleanup_required, true);
    assert.equal(receipt.termination_grace_seconds, 0.25);
    assert.equal(receipt.termination_grace_intervals, 1);
    assert.equal(receipt.termination_deadline_reused_for_cleanup, true);
    assert.equal(receipt.termination_signal_sent, true);
    assert.equal(receipt.kill_signal_sent, true);
    assert.deepEqual(receipt.signal_actions.map((entry) => entry.signal), ["SIGTERM", "SIGKILL"]);
    const descendantPid = Number(await readFile(
      resolve(fixture.runRoot, "workspace", ".descendant-pid"),
      "utf8",
    ));
    assert.throws(() => process.kill(descendantPid, 0), /ESRCH|no such process/i);
  } finally {
    await rm(fixture.fixtureRoot, {recursive: true, force: true});
  }
});

for (const [mode, expectedDisposition] of [
  ["failure", "agent_failed"],
  ["timeout", "agent_timeout"],
]) {
  test(`one-shot controller terminalizes ${mode} after exactly one launch without resampling`, async () => {
    const fixture = await createControllerSmokeFixture(mode);
    try {
      const result = spawnSync(process.execPath, fixture.arguments, {
        encoding: "utf8",
        timeout: 120_000,
        maxBuffer: 128 * 1024 * 1024,
      });
      assert.notEqual(result.status, 0);
      assert.equal(
        await readFile(resolve(fixture.runRoot, "workspace", ".fake-agent-launches"), "utf8"),
        "1\n",
      );
      const ledger = (await readFile(resolve(fixture.artifactDir, "attempt-ledger.jsonl"), "utf8"))
        .trim().split("\n").map(JSON.parse);
      assert.equal(ledger.length, 2);
      assert.equal(ledger[1].disposition, expectedDisposition);
      assert.equal(ledger.filter((entry) => entry.disposition !== "registered").length, 1);
      const processReceipt = await readJson(resolve(fixture.artifactDir, "process", "receipt.json"));
      assert.equal(processReceipt.spawn_count, 1);
      assert.equal(processReceipt.resampling_performed, false);
      assert.equal(processReceipt.timed_out, mode === "timeout");
      assert.equal(processReceipt.termination_grace_seconds, 0.25);
    } finally {
      await rm(fixture.fixtureRoot, {recursive: true, force: true});
    }
  });
}

test("one-shot controller records spawn failure without signaling an undefined process group", async () => {
  const fixture = await createControllerSmokeFixture("spawn_error");
  try {
    const executable = fixture.arguments[fixture.arguments.indexOf("--agent-executable") + 1];
    await writeFile(executable, "#!/definitely/missing/interpreter\n", {mode: 0o755});
    const result = spawnSync(process.execPath, fixture.arguments, {
      encoding: "utf8",
      timeout: 120_000,
      maxBuffer: 128 * 1024 * 1024,
    });
    assert.notEqual(result.status, 0);
    const processReceipt = await readJson(resolve(fixture.artifactDir, "process", "receipt.json"));
    assert.match(processReceipt.spawn_error, /ENOENT|missing|spawn/i);
    assert.equal(processReceipt.process_group_created, false);
    assert.equal(processReceipt.spawn_count, 1);
    const ledger = (await readFile(resolve(fixture.artifactDir, "attempt-ledger.jsonl"), "utf8"))
      .trim().split("\n").map(JSON.parse);
    assert.equal(ledger.length, 2);
    assert.equal(ledger[1].disposition, "agent_failed", JSON.stringify(ledger[1].detail));
  } finally {
    await rm(fixture.fixtureRoot, {recursive: true, force: true});
  }
});

test("formal controller phase rejects without external authorization before preparation or launch", async () => {
  const fixture = await createControllerSmokeFixture("success");
  try {
    const formalArguments = [...fixture.arguments];
    formalArguments[formalArguments.indexOf("construction_pilot_only")] = "baseline_a";
    let result = spawnSync(process.execPath, formalArguments, {encoding: "utf8"});
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /external formal authorization/i);
    assert.equal(existsSync(fixture.runRoot), false);
    assert.equal(existsSync(fixture.artifactDir), false);
  } finally {
    await rm(fixture.fixtureRoot, {recursive: true, force: true});
  }
});

test("formal authorization commitments bind every effective local runtime identity", async () => {
  const {requireFormalAuthorizationCommitments, requireFormalFreeSpace, requireFormalTiming} =
    await import(pathToFileURL(runControllerScript));
  const commitments = {
    agent_executable_sha256: "1".repeat(64),
    agent_args_sha256: "2".repeat(64),
    variant_sha256: "3".repeat(64),
    model_catalog_sha256: "4".repeat(64),
    code_mode_host_sha256: "5".repeat(64),
    formal_runtime_identity_sha256: "6".repeat(64),
    effective_agent_args_sha256: "7".repeat(64),
    timeout_seconds: 5400,
    termination_grace_seconds: 10,
    rustup_home_template_sha256: "8".repeat(64),
    cargo_home_template_sha256: "9".repeat(64),
    pnpm_home_template_sha256: "a".repeat(64),
    sandbox_executable_sha256: "b".repeat(64),
  };
  assert.doesNotThrow(() => requireFormalAuthorizationCommitments({...commitments}, commitments));
  for (const field of Object.keys(commitments)) {
    const changed = {...commitments, [field]: "f".repeat(64)};
    assert.throws(
      () => requireFormalAuthorizationCommitments(changed, commitments),
      new RegExp(field),
    );
  }
  const tw01 = (await readJson(resolve(benchmarkDir, "evaluator/cases.json"))).cases
    .find((entry) => entry.id === "TW-01");
  assert.doesNotThrow(() => requireFormalTiming(tw01, 5400, commitments));
  assert.throws(() => requireFormalTiming(tw01, 5399, commitments), /exact frozen case time limit/i);
  assert.throws(
    () => requireFormalTiming(tw01, 5400, {...commitments, termination_grace_seconds: 9}),
    /termination_grace_seconds/i,
  );
  const lock = await readJson(resolve(benchmarkDir, "environment-lock.json"));
  assert.deepEqual(
    requireFormalFreeSpace(
      {free_space: {bytes: lock.controlled_runner.minimum_free_bytes_before_each_run}},
      lock,
      true,
    ),
    {
      required_minimum_bytes: lock.controlled_runner.minimum_free_bytes_before_each_run,
      observed_bytes: lock.controlled_runner.minimum_free_bytes_before_each_run,
      enforced: true,
    },
  );
  assert.throws(
    () => requireFormalFreeSpace({free_space: {bytes: 1}}, lock, true),
    /insufficient free space/i,
  );
  assert.equal(requireFormalFreeSpace({free_space: {bytes: 1}}, lock, false).enforced, false);
});

test("formal controller propagates one sealed adapter package to non-smoke oracles", async () => {
  const {formalAdapterOracleArguments} = await import(pathToFileURL(runControllerScript));
  const packageIdentity = {
    scaffold: {path: "/external/sealed-adapter.mjs"},
    config: {path: "/external/config.json"},
    integrity_receipt: {path: "/external/integrity.json", sha256: "a".repeat(64)},
  };
  const argumentsForOracle = formalAdapterOracleArguments(packageIdentity);
  assert.deepEqual(argumentsForOracle, [
    "--adapter-file", packageIdentity.scaffold.path,
    "--adapter-config", packageIdentity.config.path,
    "--adapter-integrity-receipt", packageIdentity.integrity_receipt.path,
    "--expected-adapter-integrity-sha256", packageIdentity.integrity_receipt.sha256,
  ]);
  assert.equal(argumentsForOracle.filter((entry) => entry === "--adapter-config").length, 1);
});

test("authorized formal result skeleton remains eligible while score freeze is pending", async () => {
  const {pendingResultState} = await import(pathToFileURL(runControllerScript));
  assert.deepEqual(pendingResultState(true), {
    formal_result_eligible: true,
    formal_attempt_authorized: true,
    result_state: "awaiting_score_freeze",
  });
  assert.deepEqual(pendingResultState(false), {
    formal_result_eligible: false,
    formal_attempt_authorized: false,
    result_state: "awaiting_score_freeze",
  });
});

test("controller waits for descendant process-group extinction before candidate capture", async () => {
  const fixture = await createControllerSmokeFixture("descendant");
  try {
    const result = spawnSync(process.execPath, fixture.arguments, {
      encoding: "utf8",
      timeout: 120_000,
      maxBuffer: 128 * 1024 * 1024,
    });
    assert.equal(result.status, 0, `${result.stderr}\n${result.stdout}`);
    const descendantPid = Number(await readFile(resolve(fixture.runRoot, "workspace", ".descendant-pid"), "utf8"));
    assert.throws(() => process.kill(descendantPid, 0), /ESRCH|no such process/i);
    const receipt = await readJson(resolve(fixture.artifactDir, "process", "receipt.json"));
    assert.equal(receipt.process_group_extinct_before_capture, true);
    assert.equal(receipt.descendant_cleanup_required, true);
    assert.equal(receipt.spawn_count, 1);
  } finally {
    await rm(fixture.fixtureRoot, {recursive: true, force: true});
  }
});

test("trusted adapter resumes the same captured attempt without launching the agent again", async () => {
  const fixture = await createControllerSmokeFixture("success", "TW-05");
  try {
    let result = spawnSync(process.execPath, fixture.arguments, {
      encoding: "utf8",
      timeout: 120_000,
      maxBuffer: 128 * 1024 * 1024,
    });
    assert.equal(result.status, 3, result.stderr);
    assert.equal(existsSync(resolve(fixture.artifactDir, "terminal.json")), false);
    assert.equal(
      await readFile(resolve(fixture.runRoot, "workspace", ".fake-agent-launches"), "utf8"),
      "1\n",
    );
    const adapter = resolve(fixture.fixtureRoot, "trusted-adapter.mjs");
    const adapterBytes = Buffer.from("export default function adapter() { return {}; }\n");
    await writeFile(adapter, adapterBytes, {mode: 0o600});
    const patchPath = resolve(fixture.artifactDir, "candidate-capture", "candidate.patch");
    const capturedPatchBytes = await readFile(patchPath);
    await writeFile(patchPath, Buffer.concat([capturedPatchBytes, Buffer.from("tamper\n")]));
    let rejected = spawnSync(process.execPath, [
      runControllerScript,
      "--resume-artifact-dir", fixture.artifactDir,
      "--adapter-file", adapter,
      "--expected-adapter-sha256", sha256(adapterBytes),
    ], {encoding: "utf8"});
    assert.notEqual(rejected.status, 0);
    assert.match(rejected.stderr, /stage|candidate|patch|capture|changed/i);
    assert.equal(existsSync(resolve(fixture.artifactDir, "terminal.json")), false);
    await writeFile(patchPath, capturedPatchBytes);
    const pausePath = resolve(fixture.artifactDir, "awaiting-trusted-adapter.json");
    const pauseBytes = await readFile(pausePath);
    const tamperedPause = JSON.parse(pauseBytes.toString("utf8"));
    tamperedPause.validation_workspace = resolve(fixture.fixtureRoot, "redirected-validation");
    await writeFile(pausePath, `${JSON.stringify(tamperedPause, null, 2)}\n`);
    rejected = spawnSync(process.execPath, [
      runControllerScript,
      "--resume-artifact-dir", fixture.artifactDir,
      "--adapter-file", adapter,
      "--expected-adapter-sha256", sha256(adapterBytes),
    ], {encoding: "utf8"});
    assert.notEqual(rejected.status, 0);
    assert.match(rejected.stderr, /validation workspace|redirected-validation|final stage|pause receipt/i);
    assert.equal(existsSync(resolve(fixture.artifactDir, "terminal.json")), false);
    await writeFile(pausePath, pauseBytes);
    result = spawnSync(process.execPath, [
      runControllerScript,
      "--resume-artifact-dir", fixture.artifactDir,
      "--adapter-file", adapter,
      "--expected-adapter-sha256", sha256(adapterBytes),
    ], {
      encoding: "utf8",
      timeout: 120_000,
      maxBuffer: 128 * 1024 * 1024,
    });
    assert.equal(result.status, 0, result.stderr);
    assert.equal(
      await readFile(resolve(fixture.runRoot, "workspace", ".fake-agent-launches"), "utf8"),
      "1\n",
    );
    const ledger = (await readFile(resolve(fixture.artifactDir, "attempt-ledger.jsonl"), "utf8"))
      .trim().split("\n").map(JSON.parse);
    assert.equal(ledger.length, 2);
    assert.equal(ledger[1].disposition, "awaiting_review");
    assert.equal(ledger[1].detail.resumed_same_attempt, true);
    assert.equal(ledger[1].launch_count, 1);
    const receipts = await controllerStageReceipts(fixture.artifactDir);
    const stages = receipts.map((entry) => entry.stage);
    assert.ok(stages.includes("awaiting_trusted_adapter"));
    assert.ok(stages.indexOf("awaiting_trusted_adapter") < stages.indexOf("production_oracles"));
  } finally {
    await rm(fixture.fixtureRoot, {recursive: true, force: true});
  }
});

test("formal adapter validation accepts only the sealed qualified scaffold", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-formal-adapter-"));
  try {
    const arbitraryAdapter = resolve(fixtureRoot, "arbitrary-adapter.mjs");
    await writeFile(arbitraryAdapter, "console.log('{}');\n");
    const {validateFormalAdapterPackage} = await import(pathToFileURL(runControllerScript));
    await assert.rejects(
      validateFormalAdapterPackage({
        adapterPath: arbitraryAdapter,
        benchmarkRoot: benchmarkDir,
        forbiddenRoots: [repositoryRoot, fixtureRoot],
      }),
      /sealed qualified candidate adapter scaffold/i,
    );
  } finally {
    await rm(fixtureRoot, {recursive: true, force: true});
  }
});

test("formal adapter config and probe cannot originate in candidate or controller roots", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-formal-adapter-roots-"));
  const candidateRoot = resolve(fixtureRoot, "candidate");
  try {
    await mkdir(candidateRoot);
    const probe = resolve(candidateRoot, "probe.mjs");
    const config = resolve(candidateRoot, "adapter-config.json");
    await writeFile(probe, "#!/usr/bin/env node\nconsole.log('{}');\n", {mode: 0o755});
    await writeFile(config, `${JSON.stringify({
      schema: "tachiko-candidate-adapter-v1",
      case_id: "TW-05",
      probe: {executable: probe, sha256: sha256(await readFile(probe)), arguments: ["<candidate-root>"]},
    })}\n`);
    const {validateFormalAdapterPackage} = await import(pathToFileURL(runControllerScript));
    await assert.rejects(
      validateFormalAdapterPackage({
        adapterPath: resolve(benchmarkDir, "evaluator/adapters/candidate-adapter.mjs"),
        configPath: config,
        benchmarkRoot: benchmarkDir,
        forbiddenRoots: [candidateRoot, benchmarkDir],
        context: {case_id: "TW-05"},
      }),
      /adapter config.*forbidden|adapter config.*disjoint/i,
    );
  } finally {
    await rm(fixtureRoot, {recursive: true, force: true});
  }
});

test("formal adapter validation requires an external hash-bound integrity approval", async () => {
  const externalRoot = await mkdtemp(join(tmpdir(), "tachiko-formal-adapter-external-"));
  try {
    const probe = resolve(externalRoot, "probe.mjs");
    const config = resolve(externalRoot, "adapter-config.json");
    await writeFile(probe, "#!/usr/bin/env node\nconsole.log('{}');\n", {mode: 0o755});
    await writeFile(config, `${JSON.stringify({
      schema: "tachiko-candidate-adapter-v1",
      case_id: "TW-05",
      probe: {executable: probe, sha256: sha256(await readFile(probe)), arguments: ["<candidate-root>"]},
    })}\n`);
    const {validateFormalAdapterPackage} = await import(pathToFileURL(runControllerScript));
    await assert.rejects(
      validateFormalAdapterPackage({
        adapterPath: resolve(benchmarkDir, "evaluator/adapters/candidate-adapter.mjs"),
        configPath: config,
        benchmarkRoot: benchmarkDir,
        forbiddenRoots: [repositoryRoot],
        context: {
          protocol_id: "tachiko-agents-effect-v1",
          phase: "baseline_a",
          wave_id: "1".repeat(32), run_id: "2".repeat(32), attempt_id: "3".repeat(32),
          candidate_id: "4".repeat(32), case_id: "TW-05",
          capture_receipt_sha256: "5".repeat(64),
        },
      }),
      /adapter integrity receipt is required/i,
    );
  } finally {
    await rm(externalRoot, {recursive: true, force: true});
  }
});

test("formal adapter integrity approval is bound to the capture and exact adapter package", async () => {
  const externalRoot = await mkdtemp(join(tmpdir(), "tachiko-formal-adapter-approved-"));
  try {
    const scaffold = resolve(benchmarkDir, "evaluator/adapters/candidate-adapter.mjs");
    const probe = resolve(externalRoot, "probe.mjs");
    const config = resolve(externalRoot, "adapter-config.json");
    const integrity = resolve(externalRoot, "adapter-integrity.json");
    await writeFile(probe, "#!/usr/bin/env node\nconsole.log('{}');\n", {mode: 0o755});
    const configBytes = Buffer.from(`${JSON.stringify({
      schema: "tachiko-candidate-adapter-v1",
      case_id: "TW-05",
      probe: {executable: probe, sha256: sha256(await readFile(probe)), arguments: ["<candidate-root>"]},
    })}\n`);
    await writeFile(config, configBytes);
    const context = {
      protocol_id: "tachiko-agents-effect-v1", phase: "baseline_a",
      wave_id: "1".repeat(32), run_id: "2".repeat(32), attempt_id: "3".repeat(32),
      candidate_id: "4".repeat(32), case_id: "TW-05",
      capture_receipt_sha256: "5".repeat(64),
    };
    const integrityBytes = Buffer.from(`${JSON.stringify({
      schema: "tachiko-adapter-integrity-review-v1", ...context,
      scaffold_sha256: sha256(await readFile(scaffold)),
      config_sha256: sha256(configBytes), probe_sha256: sha256(await readFile(probe)),
      reviewer_id: "independent-reviewer", reviewer_eligible: true,
      reviewer_independent: true, no_expected_values: true,
      no_behavior_implementation: true, actual_candidate_exercise: true, approved: true,
    })}\n`);
    await writeFile(integrity, integrityBytes);
    const {validateFormalAdapterPackage} = await import(pathToFileURL(runControllerScript));
    const validated = await validateFormalAdapterPackage({
      adapterPath: scaffold, configPath: config, integrityReceiptPath: integrity,
      expectedIntegrityReceiptSha256: sha256(integrityBytes), benchmarkRoot: benchmarkDir,
      forbiddenRoots: [repositoryRoot], context,
    });
    assert.equal(validated.integrity_receipt.sha256, sha256(integrityBytes));
    await assert.rejects(
      validateFormalAdapterPackage({
        adapterPath: scaffold, configPath: config, integrityReceiptPath: integrity,
        expectedIntegrityReceiptSha256: sha256(integrityBytes), benchmarkRoot: benchmarkDir,
        forbiddenRoots: [repositoryRoot],
        context: {...context, capture_receipt_sha256: "6".repeat(64)},
      }),
      /capture_receipt_sha256 mismatch/i,
    );
  } finally {
    await rm(externalRoot, {recursive: true, force: true});
  }
});

test("formal adapter probes cannot receive contract or expected-value path tokens", async () => {
  const externalRoot = await mkdtemp(join(tmpdir(), "tachiko-formal-adapter-token-"));
  try {
    const probe = resolve(externalRoot, "probe.mjs");
    const config = resolve(externalRoot, "adapter-config.json");
    await writeFile(probe, "#!/usr/bin/env node\nconsole.log('{}');\n", {mode: 0o755});
    await writeFile(config, `${JSON.stringify({
      schema: "tachiko-candidate-adapter-v1", case_id: "TW-05",
      probe: {executable: probe, sha256: sha256(await readFile(probe)), arguments: ["<contract>"]},
    })}\n`);
    const {validateFormalAdapterPackage} = await import(pathToFileURL(runControllerScript));
    await assert.rejects(
      validateFormalAdapterPackage({
        adapterPath: resolve(benchmarkDir, "evaluator/adapters/candidate-adapter.mjs"),
        configPath: config, benchmarkRoot: benchmarkDir, forbiddenRoots: [repositoryRoot],
      }),
      /contract|expected-value|probe token/i,
    );
  } finally {
    await rm(externalRoot, {recursive: true, force: true});
  }
});

test("sealed candidate adapter never expands the trusted contract into probe arguments", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-adapter-contract-token-"));
  const candidateRoot = resolve(fixtureRoot, "candidate");
  try {
    await mkdir(candidateRoot);
    const probe = resolve(fixtureRoot, "probe.mjs");
    const config = resolve(fixtureRoot, "config.json");
    const contract = resolve(fixtureRoot, "contract.json");
    const output = resolve(fixtureRoot, "output.json");
    await writeFile(probe, "#!/usr/bin/env node\nconsole.log('{}');\n", {mode: 0o755});
    await writeFile(contract, "{}\n");
    await writeFile(config, `${JSON.stringify({
      schema: "tachiko-candidate-adapter-v1", case_id: "TW-05",
      probe: {executable: probe, sha256: sha256(await readFile(probe)), arguments: ["<contract>"]},
    })}\n`);
    const result = spawnSync(process.execPath, [
      resolve(benchmarkDir, "evaluator/adapters/candidate-adapter.mjs"),
      "--candidate-root", candidateRoot, "--contract", contract,
      "--output", output, "--config", config,
    ], {encoding: "utf8"});
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /contract.*probe|forbidden.*token|unresolved.*contract/i);
    assert.equal(existsSync(output), false);
  } finally {
    await rm(fixtureRoot, {recursive: true, force: true});
  }
});

test("sealed candidate adapter inherits one outer combined sandbox without nesting", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-adapter-outer-sandbox-"));
  const runRoot = resolve(fixtureRoot, "neutral-run");
  const stagedBin = resolve(runRoot, "tool-bin");
  const stagedNode = resolve(stagedBin, "node");
  const priorTmp = resolve(runRoot, "tmp");
  const runTmp = resolve(runRoot, "adapter-tmp-fixture");
  const protectedHome = resolve(runRoot, "home");
  const artifactRoot = resolve(fixtureRoot, "controller-artifacts");
  const candidateRoot = resolve(artifactRoot, "validation-workspace");
  const expectedRoot = resolve(artifactRoot, "controller-expected-values");
  const trustedRoot = resolve(artifactRoot, "trusted-output");
  const externalRoot = resolve(fixtureRoot, "external-adapter");
  try {
    await Promise.all([
      mkdir(candidateRoot, {recursive: true}), mkdir(expectedRoot, {recursive: true}),
      mkdir(trustedRoot, {recursive: true}), mkdir(externalRoot, {recursive: true}),
      mkdir(stagedBin, {recursive: true}), mkdir(runTmp, {recursive: true}),
      mkdir(priorTmp, {recursive: true}), mkdir(protectedHome, {recursive: true}),
    ]);
    await copyFile(process.execPath, stagedNode);
    await chmod(stagedNode, 0o755);
    const probe = resolve(externalRoot, "probe.mjs");
    const config = resolve(externalRoot, "config.json");
    const contract = resolve(expectedRoot, "contract.json");
    const output = resolve(trustedRoot, "trusted-observations.json");
    await writeFile(resolve(candidateRoot, "behavior.txt"), "captured candidate behavior\n");
    const protectedPath = resolve(protectedHome, "sealed.txt");
    const tmpOutput = resolve(runTmp, "probe.tmp");
    const priorTmpSecret = resolve(priorTmp, "candidate-residue.txt");
    await writeFile(protectedPath, "sealed\n", {mode: 0o400});
    await writeFile(priorTmpSecret, "candidate-controlled residue\n");
    await writeFile(
      probe,
      "#!/usr/bin/env node\nimport {readFileSync,writeFileSync} from 'node:fs';\n" +
        "import {resolve} from 'node:path';\n" +
        "const behavior=readFileSync(resolve(process.argv[2],'behavior.txt'),'utf8').trim();\n" +
        "writeFileSync(process.argv[3],'allowed tmp output\\n');\n" +
        "let protected_write_denied=false;try{writeFileSync(process.argv[4],'tampered\\n')}" +
        "catch(e){protected_write_denied=e.code==='EPERM'||e.code==='EACCES'}\n" +
        `let prior_tmp_read_denied=false;try{readFileSync(${JSON.stringify(priorTmpSecret)})}` +
        "catch(e){prior_tmp_read_denied=e.code==='EPERM'||e.code==='EACCES'}\n" +
        "console.log(JSON.stringify({observations:{behavior,tmp_written:true," +
        "protected_write_denied,prior_tmp_read_denied}}));\n",
      {mode: 0o755},
    );
    await writeFile(contract, "{}\n");
    await writeFile(config, `${JSON.stringify({
      schema: "tachiko-candidate-adapter-v1", case_id: "TW-09",
      probe: {executable: probe, sha256: sha256(await readFile(probe)),
        arguments: ["<candidate-root>", tmpOutput, protectedPath]},
    })}\n`);
    const adapter = resolve(benchmarkDir, "evaluator/adapters/candidate-adapter.mjs");
    const {denyReadProfile} = await import(
      pathToFileURL(resolve(benchmarkDir, "scripts/network-sandbox.mjs"))
    );
    const artifactRootReal = await realpath(artifactRoot);
    const candidateRootReal = await realpath(candidateRoot);
    const benchmarkRootReal = await realpath(benchmarkDir);
    const configReal = await realpath(config);
    const probeReal = await realpath(probe);
    const profile = denyReadProfile(
      [artifactRootReal, benchmarkRootReal, await realpath(priorTmp)],
      {
        allowReadPaths: [adapter], allowReadRoots: [candidateRootReal],
        denyWriteRoots: [candidateRootReal, await realpath(runRoot)],
        denyWritePaths: [adapter, configReal, probeReal, stagedNode],
        allowWriteRoots: [await realpath(runTmp)],
      },
    );
    const result = spawnSync("/usr/bin/sandbox-exec", [
      "-p", profile, stagedNode,
      adapter,
      "--candidate-root", candidateRoot, "--contract", contract,
      "--output", output, "--config", config,
      "--deny-read-root", artifactRootReal, "--deny-read-root", benchmarkRootReal,
      "--deny-read-root", await realpath(priorTmp),
      "--deny-write-path", stagedNode,
      "--deny-write-root", await realpath(runRoot),
      "--allow-write-root", await realpath(runTmp),
      "--expected-sandbox-profile-sha256", sha256(profile),
      "--contract-sha256", sha256(await readFile(contract)),
    ], {encoding: "utf8", env: {...process.env, PATH: stagedBin}});
    assert.equal(result.status, 0, result.stderr);
    assert.equal(existsSync(output), false, "sandboxed adapter must not materialize trusted output");
    const adapterReceipt = JSON.parse(result.stdout.trim());
    assert.deepEqual(adapterReceipt.envelope.observations, {
      behavior: "captured candidate behavior",
      tmp_written: true,
      protected_write_denied: true,
      prior_tmp_read_denied: true,
    });
    assert.equal(await readFile(tmpOutput, "utf8"), "allowed tmp output\n");
    assert.equal(await readFile(protectedPath, "utf8"), "sealed\n");
    const {materializeFormalAdapterEnvelope} = await import(
      pathToFileURL(resolve(benchmarkDir, "scripts/adapter-integrity.mjs"))
    );
    const materialized = await materializeFormalAdapterEnvelope({
      stdout: result.stdout,
      outputPath: output,
      caseId: "TW-09",
      contractSha256: sha256(await readFile(contract)),
      sandboxProfileSha256: sha256(profile),
      processGroupExtinct: true,
      adapterPackage: {
        scaffold: {path: adapter, sha256: sha256(await readFile(adapter))},
        config: {path: configReal, sha256: sha256(await readFile(configReal))},
        probe: {path: probeReal, sha256: sha256(await readFile(probeReal))},
      },
    });
    assert.equal(materialized.materialized_after_process_group_extinction, true);
    assert.equal(materialized.sha256, sha256(await readFile(output)));
  } finally {
    await rm(fixtureRoot, {recursive: true, force: true});
  }
});

test("sealed candidate adapter kernel sandbox denies probe reads of expected-value roots", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-adapter-read-denial-"));
  const candidateRoot = resolve(fixtureRoot, "candidate");
  const expectedRoot = resolve(fixtureRoot, "expected-values");
  try {
    await Promise.all([mkdir(candidateRoot), mkdir(expectedRoot)]);
    const expected = resolve(expectedRoot, "contract-copy.json");
    const probe = resolve(fixtureRoot, "probe.mjs");
    const config = resolve(fixtureRoot, "config.json");
    const contract = resolve(fixtureRoot, "contract.json");
    const output = resolve(fixtureRoot, "output.json");
    await writeFile(expected, '{"expected":"secret"}\n');
    await writeFile(
      probe,
      `#!/usr/bin/env node\nimport {readFileSync} from "node:fs";\n` +
        `readFileSync(${JSON.stringify(expected)});\nconsole.log('{}');\n`,
      {mode: 0o755},
    );
    await writeFile(contract, "{}\n");
    await writeFile(config, `${JSON.stringify({
      schema: "tachiko-candidate-adapter-v1", case_id: "TW-09",
      probe: {executable: probe, sha256: sha256(await readFile(probe)), arguments: ["<candidate-root>"]},
    })}\n`);
    const adapter = resolve(benchmarkDir, "evaluator/adapters/candidate-adapter.mjs");
    const {denyReadProfile} = await import(
      pathToFileURL(resolve(benchmarkDir, "scripts/network-sandbox.mjs"))
    );
    const expectedRootReal = await realpath(expectedRoot);
    const candidateRootReal = await realpath(candidateRoot);
    const configReal = await realpath(config);
    const probeReal = await realpath(probe);
    const profile = denyReadProfile(
      [expectedRootReal],
      {
        allowReadPaths: [adapter], allowReadRoots: [candidateRootReal],
        denyWriteRoots: [candidateRootReal], denyWritePaths: [adapter, configReal, probeReal],
      },
    );
    const result = spawnSync("/usr/bin/sandbox-exec", [
      "-p", profile, process.execPath,
      adapter,
      "--candidate-root", candidateRoot, "--contract", contract,
      "--output", output, "--config", config, "--deny-read-root", expectedRootReal,
      "--expected-sandbox-profile-sha256", sha256(profile),
      "--contract-sha256", sha256(await readFile(contract)),
    ], {encoding: "utf8"});
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /operation not permitted|EPERM|candidate-specific probe failed/i);
    assert.equal(existsSync(output), false);
  } finally {
    await rm(fixtureRoot, {recursive: true, force: true});
  }
});

test("sealed candidate adapter kernel sandbox denies expected-root overwrite and metadata mutation", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-adapter-write-denial-"));
  const candidateRoot = resolve(fixtureRoot, "candidate");
  const expectedRoot = resolve(fixtureRoot, "expected-values");
  try {
    await Promise.all([mkdir(candidateRoot), mkdir(expectedRoot)]);
    const expected = resolve(expectedRoot, "contract.json");
    const probe = resolve(fixtureRoot, "probe.mjs");
    const config = resolve(fixtureRoot, "config.json");
    const output = resolve(fixtureRoot, "output.json");
    const original = Buffer.from('{"expected":"sealed"}\n');
    await writeFile(expected, original, {mode: 0o400});
    await writeFile(
      probe,
      "#!/usr/bin/env node\nimport {chmodSync,writeFileSync} from 'node:fs';\n" +
        `chmodSync(${JSON.stringify(expected)}, 0o600);\n` +
        `writeFileSync(${JSON.stringify(expected)}, 'tampered\\n');\n` +
        "console.log(JSON.stringify({observations:{ready:true}}));\n",
      {mode: 0o755},
    );
    const probeBytes = await readFile(probe);
    await writeFile(config, `${JSON.stringify({
      schema: "tachiko-candidate-adapter-v1", case_id: "TW-09",
      probe: {executable: probe, sha256: sha256(probeBytes), arguments: ["<candidate-root>"]},
    })}\n`);
    const adapter = resolve(benchmarkDir, "evaluator/adapters/candidate-adapter.mjs");
    const {denyReadProfile} = await import(
      pathToFileURL(resolve(benchmarkDir, "scripts/network-sandbox.mjs"))
    );
    const expectedRootReal = await realpath(expectedRoot);
    const candidateRootReal = await realpath(candidateRoot);
    const configReal = await realpath(config);
    const probeReal = await realpath(probe);
    const profile = denyReadProfile(
      [expectedRootReal],
      {
        allowReadPaths: [adapter], allowReadRoots: [candidateRootReal],
        denyWriteRoots: [candidateRootReal], denyWritePaths: [adapter, configReal, probeReal],
      },
    );
    const result = spawnSync("/usr/bin/sandbox-exec", [
      "-p", profile, process.execPath, adapter,
      "--candidate-root", candidateRoot, "--contract", expected,
      "--output", output, "--config", config, "--deny-read-root", expectedRootReal,
      "--expected-sandbox-profile-sha256", sha256(profile),
      "--contract-sha256", sha256(original),
    ], {encoding: "utf8"});
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /operation not permitted|EPERM|candidate-specific probe failed/i);
    assert.deepEqual(await readFile(expected), original);
    assert.equal((await stat(expected)).mode & 0o777, 0o400);
  } finally {
    await rm(fixtureRoot, {recursive: true, force: true});
  }
});

test("sealed candidate adapter kernel sandbox preserves reconstructed candidate bytes", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-adapter-candidate-write-denial-"));
  const candidateRoot = resolve(fixtureRoot, "candidate");
  const expectedRoot = resolve(fixtureRoot, "expected-values");
  try {
    await Promise.all([mkdir(candidateRoot), mkdir(expectedRoot)]);
    const behavior = resolve(candidateRoot, "behavior.txt");
    const contract = resolve(expectedRoot, "contract.json");
    const probe = resolve(fixtureRoot, "probe.mjs");
    const config = resolve(fixtureRoot, "config.json");
    const output = resolve(fixtureRoot, "output.json");
    const original = Buffer.from("captured candidate behavior\n");
    await writeFile(behavior, original, {mode: 0o400});
    await writeFile(contract, "{}\n");
    await writeFile(
      probe,
      "#!/usr/bin/env node\nimport {chmodSync,writeFileSync} from 'node:fs';\n" +
        `chmodSync(${JSON.stringify(behavior)}, 0o600);\n` +
        `writeFileSync(${JSON.stringify(behavior)}, 'patched behavior\\n');\n` +
        "console.log(JSON.stringify({observations:{ready:true}}));\n",
      {mode: 0o755},
    );
    await writeFile(config, `${JSON.stringify({
      schema: "tachiko-candidate-adapter-v1", case_id: "TW-09",
      probe: {executable: probe, sha256: sha256(await readFile(probe)), arguments: ["<candidate-root>"]},
    })}\n`);
    const adapter = resolve(benchmarkDir, "evaluator/adapters/candidate-adapter.mjs");
    const {denyReadProfile} = await import(
      pathToFileURL(resolve(benchmarkDir, "scripts/network-sandbox.mjs"))
    );
    const expectedRootReal = await realpath(expectedRoot);
    const candidateRootReal = await realpath(candidateRoot);
    const configReal = await realpath(config);
    const probeReal = await realpath(probe);
    const profile = denyReadProfile([expectedRootReal], {
      allowReadPaths: [adapter], allowReadRoots: [candidateRootReal],
      denyWriteRoots: [candidateRootReal], denyWritePaths: [adapter, configReal, probeReal],
    });
    const result = spawnSync("/usr/bin/sandbox-exec", [
      "-p", profile, process.execPath, adapter,
      "--candidate-root", candidateRoot, "--contract", contract,
      "--output", output, "--config", config, "--deny-read-root", expectedRootReal,
      "--expected-sandbox-profile-sha256", sha256(profile),
      "--contract-sha256", sha256(await readFile(contract)),
    ], {encoding: "utf8"});
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /operation not permitted|EPERM|candidate-specific probe failed/i);
    assert.deepEqual(await readFile(behavior), original);
    assert.equal((await stat(behavior)).mode & 0o777, 0o400);
  } finally {
    await rm(fixtureRoot, {recursive: true, force: true});
  }
});

test("combined adapter profile protects sibling controls while allowing nested candidate reads", async () => {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-adapter-sibling-controls-"));
  const artifactRoot = resolve(fixtureRoot, "artifacts");
  const candidateRoot = resolve(artifactRoot, "validation-workspace");
  const expectedRoot = resolve(artifactRoot, "controller-expected-values");
  const trustedRoot = resolve(artifactRoot, "trusted-output");
  const externalRoot = resolve(fixtureRoot, "external-adapter");
  try {
    await Promise.all([
      mkdir(candidateRoot, {recursive: true}), mkdir(expectedRoot, {recursive: true}),
      mkdir(trustedRoot, {recursive: true}), mkdir(externalRoot, {recursive: true}),
    ]);
    const behavior = resolve(candidateRoot, "behavior.txt");
    const expected = resolve(expectedRoot, "contract.json");
    const probe = resolve(externalRoot, "probe.mjs");
    const config = resolve(externalRoot, "config.json");
    const output = resolve(trustedRoot, "observations.json");
    const expectedBytes = Buffer.from('{"expected":"sealed"}\n');
    await writeFile(behavior, "captured behavior\n", {mode: 0o400});
    await writeFile(expected, expectedBytes, {mode: 0o400});
    await writeFile(
      probe,
      "#!/usr/bin/env node\nimport {chmodSync,readFileSync,writeFileSync} from 'node:fs';\n" +
        "import {resolve} from 'node:path';\n" +
        "const behavior=readFileSync(resolve(process.argv[2],'behavior.txt'),'utf8').trim();\n" +
        `let readDenied=false;try{readFileSync(${JSON.stringify(expected)})}catch(e){readDenied=e.code==='EPERM'||e.code==='EACCES'}\n` +
        `let writeDenied=false;try{chmodSync(${JSON.stringify(expected)},0o600);writeFileSync(${JSON.stringify(expected)},'tampered\\n')}catch(e){writeDenied=e.code==='EPERM'||e.code==='EACCES'}\n` +
        `let selfWriteDenied=false;try{chmodSync(${JSON.stringify(probe)},0o600);writeFileSync(${JSON.stringify(probe)},'tampered\\n')}catch(e){selfWriteDenied=e.code==='EPERM'||e.code==='EACCES'}\n` +
        "console.log(JSON.stringify({observations:{behavior,read_denied:readDenied," +
        "write_denied:writeDenied,self_write_denied:selfWriteDenied}}));\n",
      {mode: 0o755},
    );
    const probeBytes = await readFile(probe);
    await writeFile(config, `${JSON.stringify({
      schema: "tachiko-candidate-adapter-v1", case_id: "TW-09",
      probe: {executable: probe, sha256: sha256(probeBytes), arguments: ["<candidate-root>"]},
    })}\n`);
    const adapter = resolve(benchmarkDir, "evaluator/adapters/candidate-adapter.mjs");
    const {denyReadProfile} = await import(
      pathToFileURL(resolve(benchmarkDir, "scripts/network-sandbox.mjs"))
    );
    const artifactRootReal = await realpath(artifactRoot);
    const candidateRootReal = await realpath(candidateRoot);
    const configReal = await realpath(config);
    const probeReal = await realpath(probe);
    const profile = denyReadProfile([artifactRootReal], {
      allowReadPaths: [adapter], allowReadRoots: [candidateRootReal],
      denyWriteRoots: [candidateRootReal], denyWritePaths: [adapter, configReal, probeReal],
    });
    const result = spawnSync("/usr/bin/sandbox-exec", [
      "-p", profile, process.execPath, adapter,
      "--candidate-root", candidateRoot, "--contract", expected,
      "--output", output, "--config", config, "--deny-read-root", artifactRootReal,
      "--expected-sandbox-profile-sha256", sha256(profile),
      "--contract-sha256", sha256(expectedBytes),
    ], {encoding: "utf8"});
    assert.equal(result.status, 0, result.stderr);
    assert.equal(existsSync(output), false);
    const adapterReceipt = JSON.parse(result.stdout.trim());
    assert.deepEqual(adapterReceipt.envelope.observations, {
      behavior: "captured behavior", read_denied: true, write_denied: true,
      self_write_denied: true,
    });
    assert.deepEqual(await readFile(expected), expectedBytes);
    assert.equal((await stat(expected)).mode & 0o777, 0o400);
    assert.deepEqual(await readFile(probe), probeBytes);
    assert.equal((await stat(probe)).mode & 0o777, 0o755);
  } finally {
    await rm(fixtureRoot, {recursive: true, force: true});
  }
});
