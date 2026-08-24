import assert from "node:assert/strict";
import test from "node:test";
import { createHash } from "node:crypto";
import {
  cp,
  mkdir,
  mkdtemp,
  readFile,
  realpath,
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

async function createPreflightFixture({ runRoot = "opaque-run", parentName } = {}) {
  const fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-preflight-"));
  const root = resolve(fixtureRoot, parentName ?? "", runRoot);
  const workspace = resolve(root, "workspace");
  const home = resolve(root, "home");
  const codexHome = resolve(root, "codex-home");
  const artifactDir = resolve(fixtureRoot, "controls");
  await Promise.all([
    mkdir(workspace, { recursive: true }),
    mkdir(home, { recursive: true }),
    mkdir(codexHome, { recursive: true }),
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
  return spawnSync(
    process.execPath,
    argumentsForPreflight,
    {
      encoding: "utf8",
      env: {
        ...process.env,
        HOME: fixture.home,
        CODEX_HOME: fixture.codexHome,
        RUSTUP_HOME: process.env.RUSTUP_HOME ?? resolve(process.env.HOME, ".rustup"),
        ...environment,
      },
    },
  );
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

test("preflight rejects semantic labels in the derived run root", async () => {
  await withPreflightFixture({ runRoot: "baseline-a" }, async (fixture) => {
    const result = runPreflight(fixture);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /run root must use an opaque neutral name/i);
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
