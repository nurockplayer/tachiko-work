import { describe, expect, it, vi } from "vitest";

import { mountDesigner } from "../src/designer-app.ts";
import {
  DesignerRuntimeError,
  type DesignerClient,
} from "../src/runtime/client.ts";
import type {
  BootstrapProjection,
  FieldBatchProjection,
  FieldTarget,
  PublicationProjection,
  TableProjection,
} from "../src/runtime/protocol.ts";

const bootstrap: BootstrapProjection = {
  title: "Moonfall Balance",
  revision: "resident/0",
  default_collection: "weapons",
  collections: [
    { id: "economy", key: "economy", entity_count: 1 },
    { id: "weapons", key: "weapons", entity_count: 1 },
  ],
  control_field: { entity: "shop", field: "upgrade_cost" },
};

const table: TableProjection = {
  revision: "resident/0",
  collection: { id: "weapons", key: "weapons", entity_count: 1 },
  columns: [
    { id: "attack_interval", key: "attack_interval", field_type: "number" },
    { id: "damage", key: "damage", field_type: "number" },
    { id: "dps", key: "dps", field_type: "number" },
  ],
  rows: [
    {
      id: "iron_sword",
      key: "iron_sword",
      fields: [
        {
          target: { entity: "iron_sword", field: "attack_interval" },
          address: "iron_sword.attack_interval",
          stored: { kind: "number", value: 0.9 },
          formula: null,
          calculated: null,
          diagnostics: [],
          editable_number: true,
        },
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

class FakeClient implements DesignerClient {
  queryRequests: FieldTarget[][] = [];
  editRequests: Array<{
    expectedRevision: string;
    target: FieldTarget;
    input: string;
  }> = [];

  async bootstrap(): Promise<BootstrapProjection> {
    return bootstrap;
  }

  async queryTable(): Promise<TableProjection> {
    return structuredClone(table);
  }

  async queryFields(
    revision: string,
    fields: FieldTarget[],
  ): Promise<FieldBatchProjection> {
    this.queryRequests.push(structuredClone(fields));
    if (fields.length === 1 && fields[0]?.entity === "shop") {
      return {
        revision,
        fields: [
          {
            target: bootstrap.control_field,
            address: "shop.upgrade_cost",
            stored: null,
            formula: { source: "[tempered_blade.price]" },
            calculated: { status: "value", value: 200 },
            diagnostics: [],
            editable_number: false,
          },
        ],
      };
    }
    return {
      revision,
      fields: [
        { ...table.rows[0]!.fields[1]!, stored: { kind: "number", value: 45 } },
        {
          ...table.rows[0]!.fields[2]!,
          calculated: { status: "value", value: 50 },
        },
      ],
    };
  }

  async editNumber(
    expectedRevision: string,
    target: FieldTarget,
    input: string,
  ): Promise<PublicationProjection> {
    this.editRequests.push({
      expectedRevision,
      target: structuredClone(target),
      input,
    });
    return {
      base_revision: "resident/0",
      resulting_revision: "resident/1",
      entities: [],
      fields: [{ entity: "iron_sword", field: "damage" }],
      affected_calculations: [{ entity: "iron_sword", field: "dps" }],
    };
  }

  close(): void {}
}

class RejectingClient extends FakeClient {
  override async editNumber(): Promise<PublicationProjection> {
    throw new DesignerRuntimeError({
      code: "validation_failed",
      message: "Authoritative semantic validation rejected the candidate.",
      current_revision: "resident/0",
      diagnostics: [
        {
          code: "formula.division_by_zero",
          message: "DPS would divide by zero.",
          path: "entities.iron_sword.fields.dps",
        },
      ],
    });
  }
}

class StartupFailingClient extends FakeClient {
  override async bootstrap(): Promise<BootstrapProjection> {
    throw new Error("Designer runtime could not be loaded (404).");
  }
}

class RefreshFailingClient extends FakeClient {
  override async queryFields(
    revision: string,
    fields: FieldTarget[],
  ): Promise<FieldBatchProjection> {
    if (fields.length === 1 && fields[0]?.entity === "shop") {
      return super.queryFields(revision, fields);
    }
    throw new Error("Selective refresh is temporarily unavailable.");
  }
}

describe("Designer application seam", () => {
  it("renders the bounded table and selectively refreshes a derived result", async () => {
    document.body.innerHTML = '<div id="app"></div>';
    const root = document.querySelector<HTMLElement>("#app");
    if (root === null) throw new Error("test root is required");
    const client = new FakeClient();
    const app = mountDesigner(root, client);
    await app.ready;

    const damage = root.querySelector<HTMLInputElement>(
      'input[aria-label="Damage for Iron Sword"]',
    );
    expect(damage?.value).toBe("36");
    expect(root.querySelector('[data-field="iron_sword.dps"]')?.textContent).toContain(
      "40",
    );
    expect(root.querySelector('[data-testid="control-value"]')?.textContent).toBe(
      "200",
    );

    if (damage === null) throw new Error("damage input is required");
    damage.value = "45";
    const damageForm = damage.form;
    if (damageForm === null) throw new Error("damage form is required");
    damageForm.requestSubmit();

    await vi.waitFor(() => {
      expect(root.querySelector('[data-field="iron_sword.dps"]')?.textContent).toContain(
        "50",
      );
    });
    expect(client.queryRequests.at(-1)).toEqual([
      { entity: "iron_sword", field: "damage" },
      { entity: "iron_sword", field: "dps" },
    ]);
    expect(client.editRequests).toEqual([
      {
        expectedRevision: "resident/0",
        target: { entity: "iron_sword", field: "damage" },
        input: "45",
      },
    ]);
    expect(root.querySelector('[data-testid="control-value"]')?.textContent).toBe(
      "200",
    );
    expect(root.querySelector('[data-testid="revision"]')?.textContent).toContain(
      "resident/1",
    );
  });

  it("shows a structured rejection without advancing the visible canonical state", async () => {
    document.body.innerHTML = '<div id="app"></div>';
    const root = document.querySelector<HTMLElement>("#app");
    if (root === null) throw new Error("test root is required");
    const app = mountDesigner(root, new RejectingClient());
    await app.ready;

    const interval = root.querySelector<HTMLInputElement>(
      'input[aria-label="Attack Interval for Iron Sword"]',
    );
    if (interval === null) throw new Error("attack interval input is required");
    interval.value = "0";
    const intervalForm = interval.form;
    if (intervalForm === null) throw new Error("attack interval form is required");
    intervalForm.requestSubmit();

    await vi.waitFor(() => {
      expect(root.querySelector('[role="alert"]')?.textContent).toContain(
        "DPS would divide by zero.",
      );
    });
    expect(root.querySelector('[data-testid="revision"]')?.textContent).toContain(
      "resident/0",
    );
    expect(root.querySelector('[data-field="iron_sword.dps"]')?.textContent).toContain(
      "40",
    );
  });

  it("renders initialization failures instead of leaving the loading state visible", async () => {
    document.body.innerHTML = '<div id="app"></div>';
    const root = document.querySelector<HTMLElement>("#app");
    if (root === null) throw new Error("test root is required");
    const app = mountDesigner(root, new StartupFailingClient());

    await app.ready;

    expect(root.querySelector('[role="alert"]')?.textContent).toContain(
      "Designer runtime could not be loaded (404).",
    );
    expect(root.textContent).not.toContain("Starting the Rust workspace");
  });

  it("keeps stale edit controls disabled after a post-publication refresh failure", async () => {
    document.body.innerHTML = '<div id="app"></div>';
    const root = document.querySelector<HTMLElement>("#app");
    if (root === null) throw new Error("test root is required");
    const client = new RefreshFailingClient();
    const app = mountDesigner(root, client);
    await app.ready;

    const damage = root.querySelector<HTMLInputElement>(
      'input[aria-label="Damage for Iron Sword"]',
    );
    if (damage === null || damage.form === null) {
      throw new Error("damage edit form is required");
    }
    damage.value = "45";
    damage.form.requestSubmit();

    await vi.waitFor(() => {
      expect(root.querySelector('[role="alert"]')?.textContent).toContain(
        "Selective refresh is temporarily unavailable.",
      );
    });
    const staleDamage = root.querySelector<HTMLInputElement>(
      'input[aria-label="Damage for Iron Sword"]',
    );
    expect(staleDamage?.disabled).toBe(true);
    expect(staleDamage?.value).toBe("36");
    expect(root.querySelector('[data-testid="revision"]')?.textContent).toContain(
      "resident/1",
    );
    expect(client.editRequests).toHaveLength(1);
  });
});
