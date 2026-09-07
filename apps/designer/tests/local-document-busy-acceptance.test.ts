import { describe, expect, it, vi } from "vitest";

import { mountDesigner } from "../src/designer-app.ts";
import type { DesignerProjectHost } from "../src/host/browser-project-host.ts";
import type { DesignerClient } from "../src/runtime/client.ts";
import type {
  BootstrapProjection,
  FieldBatchProjection,
  FieldTarget,
  OpenedProjection,
  ProjectExport,
  PublicationProjection,
  TableProjection,
} from "../src/runtime/protocol.ts";

const bootstrap: BootstrapProjection = {
  title: "Moonfall Balance",
  revision: "resident/0",
  default_collection: "weapons",
  collections: [{ id: "weapons", key: "weapons", entity_count: 1 }],
};

const table = (revision = "resident/0", damage = 36): TableProjection => ({
  revision,
  collection: { id: "weapons", key: "weapons", entity_count: 1 },
  columns: [{ id: "damage", key: "damage", field_type: "number" }],
  rows: [{
    id: "iron_sword",
    key: "iron_sword",
    fields: [{
      target: { entity: "iron_sword", field: "damage" },
      address: "iron_sword.damage",
      stored: { kind: "number", value: damage },
      formula: null,
      calculated: null,
      diagnostics: [],
      editable_scalar: "number",
    }],
  }],
});

class BusyClient implements DesignerClient {
  editStarted = false;
  readonly localOpen = vi.fn(async (): Promise<OpenedProjection> => ({
    bootstrap: { ...bootstrap, title: "Other Project" },
    table: table("resident/0", 12),
  }));
  #finishEdit: ((publication: PublicationProjection) => void) | null = null;

  async bootstrap(): Promise<BootstrapProjection> {
    return structuredClone(bootstrap);
  }

  async openProject(): Promise<OpenedProjection> {
    return { bootstrap: structuredClone(bootstrap), table: table() };
  }

  async openLocalDocument(bytes: ArrayBuffer): Promise<OpenedProjection> {
    return this.localOpen(bytes);
  }

  async exportProject(expectedRevision: string): Promise<ProjectExport> {
    return { revision: expectedRevision, bytes: new ArrayBuffer(1) };
  }

  async closeProject(): Promise<void> {}

  async queryTable(): Promise<TableProjection> {
    return table();
  }

  async queryFields(
    expectedRevision: string,
    fields: FieldTarget[],
  ): Promise<FieldBatchProjection> {
    return {
      revision: expectedRevision,
      fields: fields.map((target) => ({
        ...table("resident/1", 45).rows[0]!.fields[0]!,
        target: structuredClone(target),
        address: `${target.entity}.${target.field}`,
      })),
    };
  }

  editNumber(): Promise<PublicationProjection> {
    this.editStarted = true;
    return new Promise((resolve) => {
      this.#finishEdit = resolve;
    });
  }

  finishEdit(): void {
    const finish = this.#finishEdit;
    if (finish === null) throw new Error("pending edit was not started");
    this.#finishEdit = null;
    finish({
      base_revision: "resident/0",
      resulting_revision: "resident/1",
      entities: [],
      fields: [{ entity: "iron_sword", field: "damage" }],
      affected_calculations: [],
    });
  }

  async editText(): Promise<PublicationProjection> {
    throw new Error("Text editing is outside this acceptance fixture.");
  }

  async editBoolean(): Promise<PublicationProjection> {
    throw new Error("Boolean editing is outside this acceptance fixture.");
  }

  async editDate(): Promise<PublicationProjection> {
    throw new Error("Date editing is outside this acceptance fixture.");
  }

  close(): void {}
}

const host: DesignerProjectHost = {
  async list() { return []; },
  async read() { throw new Error("No saved project exists in this fixture."); },
  async publish() {},
};

describe("local document busy acceptance", () => {
  it("rejects a distinct warm local launch visibly and non-destructively while Designer is busy", async () => {
    document.body.innerHTML = '<div id="app"></div>';
    const root = document.querySelector<HTMLElement>("#app");
    if (root === null) throw new Error("test root is required");
    const client = new BusyClient();
    const app = mountDesigner(root, client, host);
    await app.ready;

    const damage = root.querySelector<HTMLInputElement>(
      'input[aria-label="Damage for Iron Sword"]',
    );
    if (damage === null || damage.form === null) throw new Error("damage form is required");
    damage.value = "45";
    damage.form.requestSubmit();
    await vi.waitFor(() => expect(client.editStarted).toBe(true));

    await app.openLocalDocumentHandles([{
      kind: "file",
      name: "other.ro",
      getFile: async () => new File(["opaque local bytes"], "other.ro"),
    }]);

    expect(client.localOpen).not.toHaveBeenCalled();
    expect(root.querySelector('[role="alert"]')?.textContent).toMatch(
      /busy|in progress|try again/i,
    );
    expect(root.getElementsByTagName("h1")[0]?.textContent).toBe("Moonfall Balance");
    expect(root.querySelector('[data-testid="revision"]')?.textContent).toContain(
      "resident/0",
    );

    client.finishEdit();
    await vi.waitFor(() => {
      expect(root.querySelector('[data-testid="revision"]')?.textContent).toContain(
        "resident/1",
      );
    });
    expect(
      root.querySelector<HTMLInputElement>('input[aria-label="Damage for Iron Sword"]')
        ?.value,
    ).toBe("45");
    app.destroy();
  });
});
