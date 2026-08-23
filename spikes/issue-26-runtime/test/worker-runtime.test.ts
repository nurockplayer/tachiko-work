import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { RuntimeClient } from "../worker/runtime-client.ts";

const here = path.dirname(fileURLToPath(import.meta.url));
const wasmPath = path.resolve(
  here,
  "../target/wasm32-unknown-unknown/release/tachiko_issue_26_runtime_spike.wasm",
);

test("TypeScript client reaches workspace-engine through a resident Worker/WASM runtime", async () => {
  const client = await RuntimeClient.spawn(wasmPath);
  try {
    const opened = await client.openSynthetic(2);
    assert.equal(opened.revision, 0);

    const overview = await client.execute({ type: "overview" });
    assert.deepEqual(overview, {
      revision: 0,
      result: {
        type: "overview",
        schema_count: 1,
        entity_count: 2,
        formula_count: 2,
      },
    });

    const calculation = await client.execute({ type: "calculate" });
    assert.equal(calculation.revision, 0);
    assert.equal(calculation.result.type, "calculation");
    if (calculation.result.type !== "calculation") {
      throw new Error("expected calculation result");
    }
    assert.equal(calculation.result.calculated.length, 6);

    const mutation = await client.execute({
      type: "set_scalar",
      address: { entity: "entity_0000", field: "base" },
      input: "11",
    });
    assert.equal(mutation.revision, 1);
    assert.equal(mutation.result.type, "mutation");
    if (mutation.result.type !== "mutation") {
      throw new Error("expected mutation result");
    }
    assert.equal(mutation.result.change_count, 2);
    assert.equal(
      mutation.result.diff_text,
      "Synthetic Records Entity 0000\nbase: 1 -> 11\naffected computed: 2 -> 22\n",
    );
    assert.deepEqual(mutation.result.patches, [
      {
        field: {
          entity: "synthetic-entity-000000",
          field: "synthetic-base-field-id",
        },
        value: { type: "number", value: 11 },
      },
      {
        field: {
          entity: "synthetic-entity-000000",
          field: "synthetic-computed-field-id",
        },
        value: { type: "number", value: 22 },
      },
    ]);

    const textMutation = await client.execute({
      type: "set_scalar",
      address: { entity: "entity_0000", field: "label" },
      input: "Renamed",
    });
    assert.deepEqual(textMutation, {
      revision: 2,
      result: {
        type: "mutation",
        change_count: 1,
        diff_text:
          'Synthetic Records Entity 0000\nlabel: "Record 0" -> "Renamed"\n',
        patches: [
          {
            field: {
              entity: "synthetic-entity-000000",
              field: "synthetic-label-field-id",
            },
            value: { type: "text", value: "Renamed" },
          },
        ],
      },
    });

    const snapshot = await client.snapshot();
    assert.equal(snapshot.entities["synthetic-entity-000000"].key, "entity_0000");
  } finally {
    await client.close();
  }
});
