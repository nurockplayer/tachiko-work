import { describe, expect, it } from "vitest";

import { createProjectionStore } from "../src/projection-store.ts";
import type {
  FieldBatchProjection,
  PublicationProjection,
  TableProjection,
} from "../src/runtime/protocol.ts";

const table: TableProjection = {
  revision: "resident/0",
  collection: { id: "weapons", key: "weapons", entity_count: 1 },
  columns: [
    { id: "damage", key: "damage", field_type: "number" },
    { id: "dps", key: "dps", field_type: "number" },
  ],
  rows: [
    {
      id: "iron_sword",
      key: "iron_sword",
      fields: [
        {
          target: { entity: "iron_sword", field: "damage" },
          address: "iron_sword.damage",
          stored: { kind: "number", value: 36 },
          formula: null,
          calculated: null,
          diagnostics: [],
          editable_number: true,
        },
        {
          target: { entity: "iron_sword", field: "dps" },
          address: "iron_sword.dps",
          stored: null,
          formula: {
            source: "([iron_sword.damage] / [iron_sword.attack_interval])",
          },
          calculated: { status: "value", value: 40 },
          diagnostics: [],
          editable_number: false,
        },
      ],
    },
  ],
};

const publication: PublicationProjection = {
  base_revision: "resident/0",
  resulting_revision: "resident/1",
  entities: [],
  fields: [{ entity: "iron_sword", field: "damage" }],
  affected_calculations: [{ entity: "iron_sword", field: "dps" }],
};

describe("revision-keyed projection store", () => {
  it("keeps opaque field-target tuples distinct when their dotted forms collide", () => {
    const collisionTable = structuredClone(table);
    const first = collisionTable.rows[0]!.fields[0]!;
    const second = collisionTable.rows[0]!.fields[1]!;
    first.target = { entity: "a.b", field: "c" };
    first.address = "first";
    second.target = { entity: "a", field: "b.c" };
    second.address = "second";
    const store = createProjectionStore(collisionTable, {
      target: { entity: "control", field: "value" },
      value: 200,
      revision: "resident/0",
    });

    const requested = store.beginPublication({
      base_revision: "resident/0",
      resulting_revision: "resident/1",
      entities: [],
      fields: [first.target, second.target],
      affected_calculations: [],
    });
    expect(requested).toEqual([first.target, second.target]);

    store.finishRefresh({
      revision: "resident/1",
      fields: [
        { ...first, stored: { kind: "number", value: 45 } },
        { ...second, calculated: { status: "value", value: 50 } },
      ],
    });
    expect(store.field("first")?.stored).toEqual({ kind: "number", value: 45 });
    expect(store.field("second")?.calculated).toEqual({
      status: "value",
      value: 50,
    });
  });

  it("refreshes only invalidated fields and carries an unrelated control forward", () => {
    const store = createProjectionStore(table, {
      target: { entity: "shop", field: "upgrade_cost" },
      value: 200,
      revision: "resident/0",
    });

    const requested = store.beginPublication(publication);

    expect(requested).toEqual([
      { entity: "iron_sword", field: "damage" },
      { entity: "iron_sword", field: "dps" },
    ]);
    expect(store.snapshot().currentness).toBe("refreshing");
    expect(store.snapshot().control).toEqual({
      target: { entity: "shop", field: "upgrade_cost" },
      value: 200,
      revision: "resident/1",
    });

    const refreshed: FieldBatchProjection = {
      revision: "resident/1",
      fields: [
        { ...table.rows[0]!.fields[0]!, stored: { kind: "number", value: 45 } },
        {
          ...table.rows[0]!.fields[1]!,
          calculated: { status: "value", value: 50 },
        },
      ],
    };
    store.finishRefresh(refreshed);

    expect(store.snapshot().currentness).toBe("current");
    expect(store.snapshot().table.revision).toBe("resident/1");
    expect(store.field("iron_sword.damage")?.stored).toEqual({
      kind: "number",
      value: 45,
    });
    expect(store.field("iron_sword.dps")?.calculated).toEqual({
      status: "value",
      value: 50,
    });
    expect(store.snapshot().control.value).toBe(200);
  });

  it("retains failed invalidations until a later refresh resolves every stale field", () => {
    const store = createProjectionStore(table, {
      target: { entity: "shop", field: "upgrade_cost" },
      value: 200,
      revision: "resident/0",
    });

    store.beginPublication(publication);
    store.failRefresh("temporary query failure");
    expect(store.snapshot().currentness).toBe("refresh_failed");

    const retryTargets = store.beginPublication({
      base_revision: "resident/1",
      resulting_revision: "resident/2",
      entities: [],
      fields: [],
      affected_calculations: [{ entity: "iron_sword", field: "dps" }],
    });
    expect(retryTargets).toEqual([
      { entity: "iron_sword", field: "damage" },
      { entity: "iron_sword", field: "dps" },
    ]);

    store.finishRefresh({
      revision: "resident/2",
      fields: [
        { ...table.rows[0]!.fields[0]!, stored: { kind: "number", value: 45 } },
        {
          ...table.rows[0]!.fields[1]!,
          calculated: { status: "value", value: 50 },
        },
      ],
    });
    expect(store.snapshot().currentness).toBe("current");
    expect(store.field("iron_sword.damage")?.stored).toEqual({
      kind: "number",
      value: 45,
    });
  });

  it("does not mark an invalidated unavailable control projection current", () => {
    const store = createProjectionStore(table, {
      target: { entity: "shop", field: "upgrade_cost" },
      value: 200,
      revision: "resident/0",
    });
    store.beginPublication({
      ...publication,
      affected_calculations: [
        { entity: "iron_sword", field: "dps" },
        { entity: "shop", field: "upgrade_cost" },
      ],
    });

    expect(() => {
      store.finishRefresh({
        revision: "resident/1",
        fields: [
          { ...table.rows[0]!.fields[0]!, stored: { kind: "number", value: 45 } },
          {
            ...table.rows[0]!.fields[1]!,
            calculated: { status: "value", value: 50 },
          },
          {
            target: { entity: "shop", field: "upgrade_cost" },
            address: "shop.upgrade_cost",
            stored: null,
            formula: { source: "[tempered_blade.price]" },
            calculated: { status: "unavailable" },
            diagnostics: [],
            editable_number: false,
          },
        ],
      });
    }).toThrow("invalidated control projection is unavailable");
    expect(store.snapshot().currentness).toBe("refreshing");
    expect(store.snapshot().control).toEqual({
      target: { entity: "shop", field: "upgrade_cost" },
      value: 200,
      revision: "resident/0",
    });
  });
});
