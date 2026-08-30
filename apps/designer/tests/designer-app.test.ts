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
  OpenedProjection,
  PublicationProjection,
  ProjectExport,
  TableProjection,
} from "../src/runtime/protocol.ts";
import type { DesignerProjectHost } from "../src/host/browser-project-host.ts";

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
    { id: "enabled", key: "enabled", field_type: "boolean" },
    { id: "name", key: "name", field_type: "text" },
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
          editable_scalar: "number",
        },
        {
          target: { entity: "iron_sword", field: "damage" },
          address: "iron_sword.damage",
          stored: { kind: "number", value: 36 },
          formula: null,
          calculated: null,
          diagnostics: [],
          editable_scalar: "number",
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
          editable_scalar: null,
        },
        {
          target: { entity: "iron_sword", field: "enabled" },
          address: "iron_sword.enabled",
          stored: { kind: "boolean", value: true },
          formula: null,
          calculated: null,
          diagnostics: [],
          editable_scalar: "boolean",
        },
        {
          target: { entity: "iron_sword", field: "name" },
          address: "iron_sword.name",
          stored: { kind: "text", value: "Iron Sword" },
          formula: null,
          calculated: null,
          diagnostics: [],
          editable_scalar: "text",
        },
      ],
    },
  ],
};

const openedProjection = (): OpenedProjection => ({
  bootstrap: structuredClone(bootstrap),
  table: structuredClone(table),
  control: {
    target: structuredClone(bootstrap.control_field),
    value: 200,
    revision: "resident/0",
  },
});

class FakeClient implements DesignerClient {
  queryRequests: FieldTarget[][] = [];
  editRequests: Array<{
    expectedRevision: string;
    target: FieldTarget;
    input: string;
  }> = [];
  textEditRequests: Array<{
    expectedRevision: string;
    target: FieldTarget;
    value: string;
  }> = [];
  booleanEditRequests: Array<{
    expectedRevision: string;
    target: FieldTarget;
    value: boolean;
  }> = [];

  async bootstrap(): Promise<BootstrapProjection> {
    return bootstrap;
  }

  async openProject(): Promise<OpenedProjection> {
    return openedProjection();
  }

  async exportProject(expectedRevision: string): Promise<ProjectExport> {
    return { revision: expectedRevision, bytes: new ArrayBuffer(8) };
  }

  async closeProject(): Promise<void> {}

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
            editable_scalar: null,
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

  async editText(
    expectedRevision: string,
    target: FieldTarget,
    value: string,
  ): Promise<PublicationProjection> {
    this.textEditRequests.push({
      expectedRevision,
      target: structuredClone(target),
      value,
    });
    return {
      base_revision: expectedRevision,
      resulting_revision: "resident/1",
      entities: [],
      fields: [structuredClone(target)],
      affected_calculations: [],
    };
  }

  async editBoolean(
    expectedRevision: string,
    target: FieldTarget,
    value: boolean,
  ): Promise<PublicationProjection> {
    this.booleanEditRequests.push({
      expectedRevision,
      target: structuredClone(target),
      value,
    });
    return {
      base_revision: expectedRevision,
      resulting_revision: "resident/2",
      entities: [],
      fields: [structuredClone(target)],
      affected_calculations: [],
    };
  }

  close(): void {}
}

class ScalarClient extends FakeClient {
  override async queryFields(
    revision: string,
    fields: FieldTarget[],
  ): Promise<FieldBatchProjection> {
    if (fields.length === 1 && fields[0]?.entity === "shop") {
      return super.queryFields(revision, fields);
    }
    this.queryRequests.push(structuredClone(fields));
    return {
      revision,
      fields: fields.map((target) => {
        const field = table.rows[0]!.fields.find(
          (candidate) =>
            candidate.target.entity === target.entity && candidate.target.field === target.field,
        );
        if (field === undefined) throw new Error(`Missing test field '${target.field}'.`);
        const refreshed = structuredClone(field);
        if (target.field === "name" && this.textEditRequests.length > 0) {
          refreshed.stored = { kind: "text", value: "Longsword\n+1" };
        }
        if (target.field === "enabled" && this.booleanEditRequests.length > 0) {
          refreshed.stored = { kind: "boolean", value: false };
        }
        return refreshed;
      }),
    };
  }
}

class FakeHost implements DesignerProjectHost {
  async list() {
    return [];
  }

  async read(): Promise<ArrayBuffer> {
    throw new Error("No saved project exists in this fixture.");
  }

  async publish(): Promise<void> {}
}

const host = new FakeHost();

class MemoryHost implements DesignerProjectHost {
  readonly projects = new Map<string, ArrayBuffer>();

  async list() {
    return [...this.projects].map(([name, bytes]) => ({
      name,
      byte_length: bytes.byteLength,
      saved_at: "2026-08-30T00:00:00.000Z",
    }));
  }

  async read(name: string): Promise<ArrayBuffer> {
    const bytes = this.projects.get(name);
    if (bytes === undefined) throw new Error("Project is missing.");
    return bytes.slice(0);
  }

  async publish(name: string, bytes: ArrayBuffer): Promise<void> {
    if (this.projects.has(name)) {
      throw new Error(`'${name}' already exists. Save As never overwrites a project.`);
    }
    this.projects.set(name, bytes.slice(0));
  }
}

class RejectingPublishHost extends MemoryHost {
  override async publish(): Promise<void> {
    throw new Error("Injected host commit failure.");
  }
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

class RejectingOpenClient extends FakeClient {
  override async openProject(): Promise<OpenedProjection> {
    throw new DesignerRuntimeError({
      code: "invalid_project",
      message: "Canonical project admission rejected corrupt bytes.",
      current_revision: "resident/0",
      diagnostics: [],
    });
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

class ControlRecoveryClient extends FakeClient {
  private refreshFailed = false;

  override async queryTable(): Promise<TableProjection> {
    const projected = structuredClone(table);
    if (this.editRequests.length > 0) {
      projected.revision = "resident/1";
      projected.rows[0]!.fields[1]!.stored = { kind: "number", value: 45 };
      projected.rows[0]!.fields[2]!.calculated = { status: "value", value: 50 };
    }
    return projected;
  }

  override async queryFields(
    revision: string,
    fields: FieldTarget[],
  ): Promise<FieldBatchProjection> {
    if (fields.some((field) => field.entity === "iron_sword")) {
      this.refreshFailed = true;
      throw new Error("Selective refresh is temporarily unavailable.");
    }
    const batch = await super.queryFields(revision, fields);
    if (this.refreshFailed && batch.fields[0]?.calculated?.status === "value") {
      batch.fields[0].calculated.value = 220;
    }
    return batch;
  }

  override async editNumber(
    expectedRevision: string,
    target: FieldTarget,
    input: string,
  ): Promise<PublicationProjection> {
    const publication = await super.editNumber(expectedRevision, target, input);
    publication.affected_calculations.push(bootstrap.control_field);
    return publication;
  }
}

describe("Designer application seam", () => {
  it("publishes Rust-authorized Text and Boolean controls against their visible revisions", async () => {
    document.body.innerHTML = '<div id="app"></div>';
    const root = document.querySelector<HTMLElement>("#app");
    if (root === null) throw new Error("test root is required");
    const client = new ScalarClient();
    const app = mountDesigner(root, client, host);
    await app.ready;

    const name = root.querySelector<HTMLTextAreaElement>(
      'textarea[aria-label="Name for Iron Sword"]',
    );
    if (name === null || name.form === null) throw new Error("text edit form is required");
    name.value = "Longsword\n+1";
    name.form.requestSubmit();
    await vi.waitFor(() => {
      expect(root.querySelector('[data-testid="revision"]')?.textContent).toContain(
        "resident/1",
      );
    });
    expect(client.textEditRequests).toEqual([
      {
        expectedRevision: "resident/0",
        target: { entity: "iron_sword", field: "name" },
        value: "Longsword\n+1",
      },
    ]);
    expect(
      root.querySelector<HTMLTextAreaElement>('textarea[aria-label="Name for Iron Sword"]')
        ?.value,
    ).toBe("Longsword\n+1");

    const enabled = root.querySelector<HTMLInputElement>(
      'input[aria-label="Enabled for Iron Sword"]',
    );
    if (enabled === null || enabled.form === null) {
      throw new Error("boolean edit form is required");
    }
    enabled.checked = false;
    enabled.form.requestSubmit();
    await vi.waitFor(() => {
      expect(root.querySelector('[data-testid="revision"]')?.textContent).toContain(
        "resident/2",
      );
    });
    expect(client.booleanEditRequests).toEqual([
      {
        expectedRevision: "resident/1",
        target: { entity: "iron_sword", field: "enabled" },
        value: false,
      },
    ]);
    expect(
      root.querySelector<HTMLInputElement>('input[aria-label="Enabled for Iron Sword"]')
        ?.checked,
    ).toBe(false);
    app.destroy();
  });

  it("preserves opaque edit targets across HTML parsing", async () => {
    const target = { entity: "entity\u0000id", field: "field\rid" };
    const opaqueTable = structuredClone(table);
    opaqueTable.columns[1]!.id = target.field;
    opaqueTable.rows[0]!.fields[1]!.target = target;

    class OpaqueTargetClient extends FakeClient {
      override async queryTable(): Promise<TableProjection> {
        return structuredClone(opaqueTable);
      }

      override async editNumber(
        expectedRevision: string,
        editedTarget: FieldTarget,
        input: string,
      ): Promise<PublicationProjection> {
        this.editRequests.push({
          expectedRevision,
          target: structuredClone(editedTarget),
          input,
        });
        return {
          base_revision: "resident/0",
          resulting_revision: "resident/1",
          entities: [],
          fields: [target],
          affected_calculations: [],
        };
      }

      override async queryFields(
        revision: string,
        fields: FieldTarget[],
      ): Promise<FieldBatchProjection> {
        if (fields.length === 1 && fields[0]?.entity === "shop") {
          return super.queryFields(revision, fields);
        }
        this.queryRequests.push(structuredClone(fields));
        return {
          revision,
          fields: [
            {
              ...opaqueTable.rows[0]!.fields[1]!,
              stored: { kind: "number", value: 45 },
            },
          ],
        };
      }
    }

    document.body.innerHTML = '<div id="app"></div>';
    const root = document.querySelector<HTMLElement>("#app");
    if (root === null) throw new Error("test root is required");
    const client = new OpaqueTargetClient();
    const app = mountDesigner(root, client, host);
    await app.ready;

    const damage = root.querySelector<HTMLInputElement>('input[value="36"]');
    if (damage === null) throw new Error("opaque target input is required");
    damage.value = "45";
    damage.form?.requestSubmit();

    await vi.waitFor(() => {
      expect(client.editRequests).toHaveLength(1);
    });
    expect(client.editRequests[0]?.target).toEqual(target);
  });

  it("renders the bounded table and selectively refreshes a derived result", async () => {
    document.body.innerHTML = '<div id="app"></div>';
    const root = document.querySelector<HTMLElement>("#app");
    if (root === null) throw new Error("test root is required");
    const client = new FakeClient();
    const app = mountDesigner(root, client, host);
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
    const app = mountDesigner(root, new RejectingClient(), host);
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
    const app = mountDesigner(root, new StartupFailingClient(), host);

    await app.ready;

    expect(root.querySelector('[role="alert"]')?.textContent).toContain(
      "Designer runtime could not be loaded (404).",
    );
    expect(root.textContent).not.toContain("Starting the Rust workspace");
  });

  it("keeps the current valid occurrence visible after failed Open admission", async () => {
    document.body.innerHTML = '<div id="app"></div>';
    const root = document.querySelector<HTMLElement>("#app");
    if (root === null) throw new Error("test root is required");
    const memoryHost = new MemoryHost();
    memoryHost.projects.set("corrupt.roproj", new ArrayBuffer(8));
    const confirm = vi.fn<Window["confirm"]>().mockReturnValue(true);
    vi.stubGlobal("confirm", confirm);
    const app = mountDesigner(root, new RejectingOpenClient(), memoryHost);
    await app.ready;

    root.querySelector<HTMLButtonElement>("[data-open-project]")?.click();
    await vi.waitFor(() => {
      expect(root.querySelector('[role="alert"]')?.textContent).toContain(
        "admission rejected",
      );
    });
    expect(root.getElementsByTagName("h1")[0]?.textContent).toBe("Moonfall Balance");
    expect(root.querySelector('[data-testid="revision"]')?.textContent).toContain(
      "resident/0",
    );
    expect(
      root.querySelector<HTMLInputElement>('input[aria-label="Damage for Iron Sword"]')
        ?.value,
    ).toBe("36");
    app.destroy();
    vi.unstubAllGlobals();
  });

  it("keeps stale edit controls disabled after a post-publication refresh failure", async () => {
    document.body.innerHTML = '<div id="app"></div>';
    const root = document.querySelector<HTMLElement>("#app");
    if (root === null) throw new Error("test root is required");
    const client = new RefreshFailingClient();
    const app = mountDesigner(root, client, host);
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
    expect(root.querySelector('[role="alert"]')?.textContent).toContain(
      "Edit published; refresh incomplete",
    );
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

  it("recovers a failed invalidated control through a fresh collection query", async () => {
    document.body.innerHTML = '<div id="app"></div>';
    const root = document.querySelector<HTMLElement>("#app");
    if (root === null) throw new Error("test root is required");
    const client = new ControlRecoveryClient();
    const app = mountDesigner(root, client, host);
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
      expect(root.querySelector('[data-currentness="refresh_failed"]')).not.toBeNull();
    });

    const collection = root.querySelector<HTMLSelectElement>(
      "[data-collection-select]",
    );
    if (collection === null) throw new Error("collection selector is required");
    collection.value = "economy";
    collection.dispatchEvent(new Event("change"));

    await vi.waitFor(() => {
      expect(root.querySelector('[data-currentness="current"]')).not.toBeNull();
    });
    expect(root.querySelector('[data-testid="control-value"]')?.textContent).toBe(
      "220",
    );
    expect(
      root.querySelector<HTMLInputElement>(
        'input[aria-label="Damage for Iron Sword"]',
      )?.disabled,
    ).toBe(false);
  });

  it("marks only confirmed host revisions durable and reopens after occurrence teardown", async () => {
    document.body.innerHTML = '<div id="app"></div>';
    const root = document.querySelector<HTMLElement>("#app");
    if (root === null) throw new Error("test root is required");
    const memoryHost = new MemoryHost();
    const prompt = vi.fn<Window["prompt"]>();
    vi.stubGlobal("prompt", prompt);
    const addEventListener = vi.spyOn(window, "addEventListener");
    const removeEventListener = vi.spyOn(window, "removeEventListener");
    prompt.mockReturnValueOnce("source.roproj");
    const app = mountDesigner(root, new FakeClient(), memoryHost);
    await app.ready;

    expect(root.querySelector('[data-testid="durability"]')?.textContent).toContain(
      "Unsaved changes",
    );
    expect(addEventListener).toHaveBeenCalledWith("beforeunload", expect.any(Function));
    root.querySelector<HTMLButtonElement>("[data-save-as]")?.click();
    await vi.waitFor(() => {
      expect(memoryHost.projects.has("source.roproj")).toBe(true);
    });
    expect(root.querySelector('[data-testid="durability"]')?.textContent).toContain(
      "Saved",
    );
    expect(removeEventListener).toHaveBeenCalledWith(
      "beforeunload",
      expect.any(Function),
    );

    const damage = root.querySelector<HTMLInputElement>(
      'input[aria-label="Damage for Iron Sword"]',
    );
    if (damage === null || damage.form === null) throw new Error("damage form is required");
    damage.value = "45";
    damage.form.requestSubmit();
    await vi.waitFor(() => {
      expect(root.querySelector('[data-testid="durability"]')?.textContent).toContain(
        "Unsaved changes",
      );
    });

    prompt.mockReturnValueOnce("source.roproj");
    root.querySelector<HTMLButtonElement>("[data-save-as]")?.click();
    await vi.waitFor(() => {
      expect(root.querySelector('[role="alert"]')?.textContent).toContain(
        "never overwrites",
      );
    });
    expect(root.querySelector('[data-testid="durability"]')?.textContent).toContain(
      "Unsaved changes",
    );

    prompt.mockReturnValueOnce("edited.roproj");
    root.querySelector<HTMLButtonElement>("[data-save-as]")?.click();
    await vi.waitFor(() => {
      expect(memoryHost.projects.has("edited.roproj")).toBe(true);
    });
    expect(root.querySelector('[data-testid="durability"]')?.textContent).toContain(
      "Saved",
    );

    root.querySelector<HTMLButtonElement>("[data-close-project]")?.click();
    await vi.waitFor(() => {
      expect(root.getElementsByTagName("h1")[0]?.textContent).toBe("No project open");
    });
    const saved = root.querySelector<HTMLSelectElement>("[data-saved-project-select]");
    if (saved === null) throw new Error("saved project picker is required");
    saved.value = "edited.roproj";
    saved.dispatchEvent(new Event("change"));
    root.querySelector<HTMLButtonElement>("[data-open-project]")?.click();
    await vi.waitFor(() => {
      expect(root.getElementsByTagName("h1")[0]?.textContent).toBe("Moonfall Balance");
    });
    expect(root.querySelector('[data-testid="durability"]')?.textContent).toContain(
      "Saved",
    );
    app.destroy();
    addEventListener.mockRestore();
    removeEventListener.mockRestore();
    vi.unstubAllGlobals();
  });

  it("requires confirmation before Open or Close discards a dirty occurrence", async () => {
    document.body.innerHTML = '<div id="app"></div>';
    const root = document.querySelector<HTMLElement>("#app");
    if (root === null) throw new Error("test root is required");
    const memoryHost = new MemoryHost();
    memoryHost.projects.set("saved.roproj", new ArrayBuffer(8));
    const client = new FakeClient();
    const openProject = vi.spyOn(client, "openProject");
    const closeProject = vi.spyOn(client, "closeProject");
    const confirm = vi.fn<Window["confirm"]>().mockReturnValue(false);
    vi.stubGlobal("confirm", confirm);
    const app = mountDesigner(root, client, memoryHost);
    await app.ready;

    root.querySelector<HTMLButtonElement>("[data-open-project]")?.click();
    root.querySelector<HTMLButtonElement>("[data-close-project]")?.click();
    const importInput = root.querySelector<HTMLInputElement>("[data-import-project]");
    if (importInput === null) throw new Error("project import input is required");
    Object.defineProperty(importInput, "value", {
      configurable: true,
      value: "saved.roproj",
      writable: true,
    });
    Object.defineProperty(importInput, "files", {
      configurable: true,
      value: [],
    });
    importInput.dispatchEvent(new Event("change"));

    expect(confirm).toHaveBeenCalledTimes(3);
    expect(openProject).not.toHaveBeenCalled();
    expect(closeProject).not.toHaveBeenCalled();
    expect(importInput.value).toBe("");
    confirm.mockReturnValue(true);
    importInput.value = "saved.roproj";
    importInput.dispatchEvent(new Event("change"));
    await vi.waitFor(() => {
      expect(importInput.value).toBe("");
    });
    expect(root.getElementsByTagName("h1")[0]?.textContent).toBe("Moonfall Balance");
    expect(root.querySelector('[data-testid="durability"]')?.textContent).toContain(
      "Unsaved changes",
    );
    app.destroy();
    vi.unstubAllGlobals();
  });

  it("keeps an absent destination and the current revision unsaved when host commit fails", async () => {
    document.body.innerHTML = '<div id="app"></div>';
    const root = document.querySelector<HTMLElement>("#app");
    if (root === null) throw new Error("test root is required");
    const rejectingHost = new RejectingPublishHost();
    const prompt = vi.fn<Window["prompt"]>().mockReturnValue("failed.roproj");
    vi.stubGlobal("prompt", prompt);
    const app = mountDesigner(root, new FakeClient(), rejectingHost);
    await app.ready;

    root.querySelector<HTMLButtonElement>("[data-save-as]")?.click();
    await vi.waitFor(() => {
      expect(root.querySelector('[role="alert"]')?.textContent).toContain(
        "Injected host commit failure",
      );
    });
    expect(rejectingHost.projects.has("failed.roproj")).toBe(false);
    expect(root.querySelector('[data-testid="durability"]')?.textContent).toContain(
      "Unsaved changes",
    );
    app.destroy();
    vi.unstubAllGlobals();
  });
});
