import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const PACKAGE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPOSITORY_ROOT = resolve(PACKAGE_ROOT, "../../../..");
const FIXTURE_ROOT = join(
  REPOSITORY_ROOT,
  "docs/research/fixtures/issue-163-pptx-provenance",
);
const TACHIKO_BIN = process.env.TACHIKO_BIN;
const DPS_FIELD = "89f0fd5e-dfc9-53bf-b008-85f78c403420";

function metricValue(result) {
  const group = result.projection.groups[0];
  const maximum = group.bucket.values.find(
    (value) => value.kind === "maximum" && value.field === DPS_FIELD,
  );
  assert(maximum, `DPS maximum is missing from ${result.kind} query result`);
  return maximum.outcome.value;
}

test(
  "fixture captures the existing Tachiko Analysis Query values and lineage",
  { skip: TACHIKO_BIN ? false : "set TACHIKO_BIN for authoritative capture verification" },
  async () => {
    const result = spawnSync(
      TACHIKO_BIN,
      [
        "analyze",
        "query",
        "examples/game-balance/game-balance.ro",
        "--schema",
        "6e594d33-70eb-5755-8b9f-f19b948d39ce",
        "--entity",
        "24ab8d17-bff2-53fc-9632-45617effe270",
        "--predicate",
        "fa616e90-705e-5fa8-b735-2a6e84d03354:gte:number:35",
        "--group-by",
        "866dded6-ba9f-542c-b328-bea19ee0f80f",
        "--result",
        "membership",
        "--result",
        "count",
        "--result",
        "min:fa616e90-705e-5fa8-b735-2a6e84d03354",
        "--result",
        `max:${DPS_FIELD}`,
        "--result",
        `observations:${DPS_FIELD}`,
        "--compare",
        "examples/game-balance/buffed-sword.ro",
      ],
      { cwd: REPOSITORY_ROOT, encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr || result.stdout);
    const query = JSON.parse(result.stdout);
    const base = JSON.parse(await readFile(join(FIXTURE_ROOT, "base.json"), "utf8"));
    const metricVariant = JSON.parse(
      await readFile(join(FIXTURE_ROOT, "variants/metric-change.json"), "utf8"),
    );
    const fixtureMetric = base.sources.find(
      (source) => source.id === `semantic:field:${DPS_FIELD}`,
    );
    const metricChange = metricVariant.changes.find(
      (change) => change.source_id === `semantic:field:${DPS_FIELD}`,
    );

    assert.equal(fixtureMetric.value, metricValue(query.first));
    assert.equal(metricChange.set.value, metricValue(query.second));
    assert.equal(base.analysis_lineage.source_revision, query.sources[0].source_revision);
    assert.equal(
      metricVariant.lineage_changes.source_revision,
      query.sources[1].source_revision,
    );
    assert.deepEqual(
      base.analysis_lineage.normalized_definition,
      query.normalized_definition,
    );
    assert.equal(
      base.analysis_lineage.formula_calculation_used,
      query.formula_calculation_used,
    );
  },
);
