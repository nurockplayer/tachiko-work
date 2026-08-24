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
  readlink,
  readFile,
  realpath,
  rename,
  rm,
  symlink,
  unlink,
  writeFile,
} from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

const testDir = dirname(fileURLToPath(import.meta.url));
const benchmarkDir = resolve(testDir, "..");
const preflightScript = resolve(benchmarkDir, "scripts/preflight-run.mjs");
const captureCandidateScript = resolve(benchmarkDir, "scripts/capture-candidate.mjs");
const prepareValidationScript = resolve(benchmarkDir, "scripts/prepare-validation.mjs");
const runOraclesScript = resolve(benchmarkDir, "scripts/run-oracles.mjs");
const runTw05OfflineScript = resolve(benchmarkDir, "scripts/run-tw05-offline.mjs");
const qualifyOraclesScript = resolve(benchmarkDir, "scripts/qualify-oracles.mjs");
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
  { environment = {}, includeExpectedAgents = true, includeExpectedControl = true, receipt } = {},
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
    process.execPath,
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
    assert.equal(result.status, 0, result.stderr);

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

async function createOracleRunnerFixture({command, selector, subjectiveGroups = []}) {
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
          id: "TW-XX",
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
        cases: [{id: "TW-XX", assertions: assertion}],
      }, null, 2)}\n`,
    ),
  ]);
  return {fixtureRoot, candidateRoot, trustedDir, manifestPath, oracleLockPath};
}

function runOracleFixture(fixture, extra = [], environment = {}) {
  return spawnSync(
    process.execPath,
    [
      runOraclesScript,
      "--case",
      "TW-XX",
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
      ...extra,
    ],
    {encoding: "utf8", env: {...process.env, ...environment}},
  );
}

test("oracle runner requires exactly one matching Rust test", async () => {
  for (const [events, expectedStatus, expectedMatches] of [
    [
      [
        {type: "test", event: "started", name: "locked_name"},
        {type: "test", event: "ok", name: "locked_name"},
        {type: "suite", event: "ok", passed: 1, failed: 0, ignored: 0},
      ],
      0,
      1,
    ],
    [[{type: "suite", event: "ok", passed: 0, failed: 0, ignored: 0}], 1, 0],
    [
      [
        {type: "test", event: "started", name: "locked_name"},
        {type: "test", event: "ok", name: "locked_name"},
        {type: "test", event: "started", name: "locked_name"},
        {type: "test", event: "ok", name: "locked_name"},
        {type: "suite", event: "ok", passed: 2, failed: 0, ignored: 0},
      ],
      1,
      2,
    ],
    [
      [
        {type: "test", event: "started", name: "locked_name"},
        {type: "test", event: "ignored", name: "locked_name"},
        {type: "suite", event: "ok", passed: 0, failed: 0, ignored: 1},
      ],
      1,
      1,
    ],
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
      const cargo = resolve(fixture.candidateRoot, "cargo");
      await writeFile(
        cargo,
        `#!/bin/sh\nprintf '%s\\n' '${events.map((entry) => JSON.stringify(entry)).join("' '")}'\n`,
        {mode: 0o755},
      );
      const result = runOracleFixture(
        fixture,
        [],
        {PATH: `${fixture.candidateRoot}:${process.env.PATH}`},
      );
      assert.equal(result.status, expectedStatus, result.stderr);
      const receipt = await readJson(resolve(fixture.trustedDir, "oracle-run.json"));
      assert.equal(receipt.assertions[0].matching_tests, expectedMatches);
      assert.equal(receipt.assertions[0].pass, expectedStatus === 0);
      assert.equal(receipt.assertions[0].evidence_mode, "libtest_json_v0.1_bootstrap");
    } finally {
      await rm(fixture.fixtureRoot, {recursive: true, force: true});
    }
  }
});

test("oracle runner rejects spoofed human-readable Rust test output", async () => {
  const fixture = await createOracleRunnerFixture({
    command: "cargo test -p fixture --test fixture --locked locked_name -- --exact",
    selector: {
      kind: "rust_test_exact",
      test_name: "locked_name",
      required_matching_tests: 1,
    },
  });
  try {
    await writeFile(
      resolve(fixture.candidateRoot, "cargo"),
      "#!/bin/sh\nprintf 'test locked_name ... ok\\n'\n",
      {mode: 0o755},
    );
    const result = runOracleFixture(
      fixture,
      [],
      {PATH: `${fixture.candidateRoot}:${process.env.PATH}`},
    );
    assert.equal(result.status, 1, result.stderr);
    const receipt = await readJson(resolve(fixture.trustedDir, "oracle-run.json"));
    assert.equal(receipt.assertions[0].pass, false);
    assert.match(receipt.assertions[0].reasons.join(" "), /libtest JSON/i);
  } finally {
    await rm(fixture.fixtureRoot, {recursive: true, force: true});
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
      assert.equal(receipt.assertions[0].pass, false);
      assert.match(receipt.assertions[0].reasons.join(" "), expectedReason);
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
    assert.match(result.stderr, /manifest.*candidate-root.*disjoint/i);
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
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-tw05-offline-"));
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
  } finally {
    await rm(fixtureRoot, {recursive: true, force: true});
  }
});

test("oracle qualification records executed target/base evidence for all frozen mappings", async () => {
  const [receipt, manifest] = await Promise.all([
    readJson(resolve(benchmarkDir, "evaluator/qualifications/oracles.json")),
    readJson(resolve(benchmarkDir, "evaluator/production-oracles.json")),
  ]);
  assert.equal(receipt.payload_sha256, sha256(`${JSON.stringify(receipt.payload)}\n`));
  assert.equal(receipt.payload.schema, "tachiko-oracle-qualification-v2");
  assert.equal(receipt.payload.no_codex_launched, true);
  assert.equal(receipt.payload.cases.length, 9);
  assert.deepEqual(receipt.payload.cases.map((entry) => entry.case_id), [
    "TW-01", "TW-02", "TW-03", "TW-04", "TW-05", "TW-06", "TW-07", "TW-08", "TW-09",
  ]);

  for (const caseEntry of receipt.payload.cases) {
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
  }

  for (const id of ["TW-01", "TW-02", "TW-06"]) {
    const entry = receipt.payload.cases.find((candidate) => candidate.case_id === id);
    assert.equal(entry.qualification, "packet_gate_only");
    assert.equal(entry.machine_semantic_discrimination_qualified, false);
  }
  for (const id of ["TW-03", "TW-04", "TW-07", "TW-08", "TW-09"]) {
    const entry = receipt.payload.cases.find((candidate) => candidate.case_id === id);
    assert.equal(entry.target.accepted, true);
    assert.equal(entry.negative.discriminated, true);
  }
  const tw05 = receipt.payload.cases.find((entry) => entry.case_id === "TW-05");
  assert.equal(tw05.target.accepted, false);
  assert.equal(tw05.target.expected_contract_miss, true);
  assert.equal(tw05.reference_positive.accepted, true);
  assert.equal(tw05.negative.discriminated, true);
  assert.deepEqual(
    tw05.target.oracle.assertions.filter((entry) => !entry.pass).map((entry) => entry.id),
    ["tw-05.native-step-4", "tw-05.native-step-5"],
  );
  assert.equal(tw05.target.adapter_execution.command_exit_code, 0);
  assert.equal(tw05.offline_historical_target.network_enforcement.probe_denied, true);
  assert.equal(tw05.offline_historical_target.package_manager_dependency, false);

  const tw09 = receipt.payload.cases.find((entry) => entry.case_id === "TW-09");
  assert.equal(tw09.materialization.target.kind, "trusted_rebased_replay_positive");
  assert.equal(tw09.materialization.target.tree, "82854a472bd6aca1cab70b750fdcae864675ce5c");
  assert.equal(tw09.target.adapter_execution.kind, "production_probe");
  assert.ok(receipt.payload.families.every((entry) => entry.evidence === "executed"));
});
