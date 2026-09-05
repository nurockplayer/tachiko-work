// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const renderReportChart = vi.hoisted(() => vi.fn(() => document.createElement("canvas")));
vi.mock("../src/report-renderer.ts", () => ({
  renderReportChart,
  formatReportNumber: (value: number) => String(value),
}));

import { mountDesigner, type MountedDesigner } from "../src/designer-app.ts";
import type { DesignerProjectHost } from "../src/host/browser-project-host.ts";
import type { DesignerClient } from "../src/runtime/client.ts";
import type {
  BootstrapProjection,
  FieldBatchProjection,
  OpenedProjection,
  ProjectExport,
  PublicationProjection,
  TableProjection,
} from "../src/runtime/protocol.ts";

const collection = "items";
const bootstrap: BootstrapProjection = {
  title: "Chart history",
  revision: "resident/0",
  default_collection: collection,
  collections: [{ id: collection, key: collection, entity_count: 1 }],
};

function table(revision = "resident/0"): TableProjection {
  return {
    tracker_profile: true,
    revision,
    collection: { id: collection, key: collection, entity_count: 1 },
    columns: [
      { id: "name", key: "Name", field_type: "text" },
      { id: "value", key: "Value", field_type: "number" },
    ],
    rows: [{
      id: "row-a",
      key: "row-a",
      fields: [
        {
          target: { entity: "row-a", field: "name" },
          address: "row-a.name",
          stored: { kind: "text", value: "Alpha" },
          formula: null,
          calculated: null,
          diagnostics: [],
          editable_scalar: "text",
        },
        {
          target: { entity: "row-a", field: "value" },
          address: "row-a.value",
          stored: { kind: "number", value: 12 },
          formula: null,
          calculated: null,
          diagnostics: [],
          editable_scalar: "number",
        },
      ],
    }],
  };
}

class Client implements DesignerClient {
  readonly queryTable = vi.fn(async (): Promise<TableProjection> => table());
  async bootstrap(): Promise<BootstrapProjection> { return structuredClone(bootstrap); }
  async openProject(): Promise<OpenedProjection> { return { bootstrap: structuredClone(bootstrap), table: table() }; }
  async exportProject(expectedRevision: string): Promise<ProjectExport> { return { revision: expectedRevision, bytes: new ArrayBuffer(8) }; }
  async closeProject(): Promise<void> {}
  async queryFields(revision: string): Promise<FieldBatchProjection> { return { revision, fields: [] }; }
  async editNumber(expectedRevision: string): Promise<PublicationProjection> { return publication(expectedRevision); }
  async editText(expectedRevision: string): Promise<PublicationProjection> { return publication(expectedRevision); }
  async editBoolean(expectedRevision: string): Promise<PublicationProjection> { return publication(expectedRevision); }
  async editDate(expectedRevision: string): Promise<PublicationProjection> { return publication(expectedRevision); }
  close(): void {}
}

function publication(revision: string): PublicationProjection {
  return { base_revision: revision, resulting_revision: revision, entities: [], fields: [], affected_calculations: [] };
}

class EmptyHost implements DesignerProjectHost {
  async list() { return []; }
  async read(): Promise<ArrayBuffer> { throw new Error("no saved project"); }
  async publish(): Promise<void> {}
}

let app: MountedDesigner | undefined;
beforeEach(() => {
  vi.stubGlobal("confirm", vi.fn(() => true));
  document.body.innerHTML = '<div id="app"></div>';
});
afterEach(() => {
  app?.destroy();
  app = undefined;
  vi.unstubAllGlobals();
  document.body.innerHTML = "";
});

async function setup(): Promise<HTMLElement> {
  const root = document.querySelector<HTMLElement>("#app");
  if (!root) throw new Error("test root required");
  app = mountDesigner(root, new Client(), new EmptyHost());
  await app.ready;
  return root;
}

function button(root: HTMLElement, selector: string): HTMLButtonElement {
  const control = root.querySelector<HTMLButtonElement>(selector);
  if (!control) throw new Error(`missing control ${selector}`);
  return control;
}

async function createChart(root: HTMLElement): Promise<void> {
  const create = root.querySelector<HTMLButtonElement>(".report-panel-actions button");
  if (!create) throw new Error("create chart control required");
  create.click();
  button(root, '.report-editor button[type="submit"]').click();
  await vi.waitFor(() => expect(root.querySelector(".report-card-title")?.textContent).toBe("Report"));
}

function undoDescription(root: HTMLElement): string {
  const control = button(root, '[data-tracker="undo"]');
  return control.getAttribute("aria-label") ?? control.textContent ?? "";
}

it("interleaves chart creation with earlier formatting without clearing history", async () => {
  const root = await setup();
  button(root, '[data-tracker="bold"]').click();
  expect(root.querySelector("[role=gridcell]")?.className).toContain("cell-bold");

  await createChart(root);
  expect(button(root, '[data-tracker="undo"]').disabled).toBe(false);
  expect(root.textContent).not.toContain("undo/redo cleared");
  expect(undoDescription(root).toLowerCase()).toContain("chart");

  button(root, '[data-tracker="undo"]').click();
  expect(root.querySelector(".report-card-title")).toBeNull();
  expect(root.querySelector("[role=gridcell]")?.className).toContain("cell-bold");

  button(root, '[data-tracker="undo"]').click();
  expect(root.querySelector("[role=gridcell]")?.className).not.toContain("cell-bold");

  button(root, '[data-tracker="redo"]').click();
  expect(root.querySelector("[role=gridcell]")?.className).toContain("cell-bold");
  button(root, '[data-tracker="redo"]').click();
  await vi.waitFor(() => expect(root.querySelector(".report-card-title")?.textContent).toBe("Report"));
});

it("undoes and redoes chart edit and delete as distinct presentation actions", async () => {
  const root = await setup();
  await createChart(root);

  const edit = [...root.querySelectorAll<HTMLButtonElement>("button")].find(control => control.textContent === "Edit chart");
  if (!edit) throw new Error("edit chart control required");
  edit.click();
  const title = root.querySelector<HTMLInputElement>('[aria-label="Chart title"]');
  if (!title) throw new Error("chart title input required");
  title.value = "Edited chart";
  title.dispatchEvent(new InputEvent("input", { bubbles: true }));
  button(root, '.report-editor button[type="submit"]').click();
  await vi.waitFor(() => expect(root.querySelector(".report-card-title")?.textContent).toBe("Edited chart"));

  button(root, '[data-tracker="undo"]').click();
  expect(root.querySelector(".report-card-title")?.textContent).toBe("Report");
  button(root, '[data-tracker="redo"]').click();
  expect(root.querySelector(".report-card-title")?.textContent).toBe("Edited chart");

  const remove = [...root.querySelectorAll<HTMLButtonElement>("button")].find(control => control.textContent === "Delete chart");
  if (!remove) throw new Error("delete chart control required");
  remove.click();
  expect(root.querySelector(".report-card-title")).toBeNull();

  button(root, '[data-tracker="undo"]').click();
  expect(root.querySelector(".report-card-title")?.textContent).toBe("Edited chart");
  button(root, '[data-tracker="redo"]').click();
  expect(root.querySelector(".report-card-title")).toBeNull();
});
