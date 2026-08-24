import assert from "node:assert/strict";
import test from "node:test";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const testDir = dirname(fileURLToPath(import.meta.url));
const benchmarkDir = resolve(testDir, "..");

async function readJson(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

function exactlyOnce(values, label) {
  assert.equal(new Set(values).size, values.length, `${label} must be unique`);
}

function ids(entries) {
  return entries.map((entry) => entry.id).sort();
}

test("production oracle manifest covers every frozen operational input exactly once", async () => {
  const [cases, oracleLock, coreScoreLock, productionOracles] = await Promise.all([
    readJson(resolve(benchmarkDir, "evaluator/cases.json")),
    readJson(resolve(benchmarkDir, "evaluator/oracle-lock.json")),
    readJson(resolve(benchmarkDir, "evaluator/core-score-lock.json")),
    readJson(resolve(benchmarkDir, "evaluator/production-oracles.json")),
  ]);

  assert.equal(productionOracles.protocol_id, cases.protocol_id);
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
      assert.equal(Object.hasOwn(mapping, "points"), false);
      assert.equal(Object.hasOwn(mapping, "selector"), false);
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
