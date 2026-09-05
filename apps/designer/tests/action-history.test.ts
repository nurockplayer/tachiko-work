import { afterEach, expect, it, vi } from "vitest";
import { mountDesigner, type MountedDesigner } from "../src/designer-app.ts";
import type { DesignerProjectHost } from "../src/host/browser-project-host.ts";
import type { DesignerClient } from "../src/runtime/client.ts";
import type { TableProjection } from "../src/runtime/protocol.ts";

let mounted: MountedDesigner | undefined;
afterEach(() => {
  mounted?.destroy();
  mounted = undefined;
  document.body.replaceChildren();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

function trackerTable(revision = "resident/0"): TableProjection {
  return {
    tracker_profile: true,
    revision,
    collection: { id: "tracker", key: "tracker", entity_count: 1 },
    columns: [
      { id: "task", key: "task", field_type: "Text" },
      { id: "estimate", key: "estimate", field_type: "Number" },
      { id: "done", key: "done", field_type: "Boolean", dropdown_options: ["true", "false"] },
    ],
    rows: [{
      id: "item",
      key: "item",
      fields: [{
        target: { entity: "item", field: "task" },
        address: "item.task",
        stored: { kind: "text", value: "Accepted work" },
        formula: null,
        calculated: null,
        diagnostics: [],
        editable_scalar: "text",
      }],
    }],
  };
}

async function setup() {
  let table = trackerTable();
  const ordinary = trackerTable();
  delete ordinary.tracker_profile;
  ordinary.collection = { id: "ordinary", key: "ordinary", entity_count: 1 };
  ordinary.rows[0]!.fields[0]!.target = { entity: "item", field: "task" };

  const queryTable = vi.fn(async (collection: string) => structuredClone(
    collection === "ordinary" ? { ...ordinary, revision: table.revision } : table,
  ));
  const trackerCommand = vi.fn(async (request: { type: string }) => {
    const base = table.revision;
    const next = Number(base.split("/")[1] ?? "0") + 1;
    table = { ...table, revision: `resident/${String(next)}` };
    return {
      base_revision: base,
      resulting_revision: table.revision,
      entities: ["item"],
      fields: [],
      affected_calculations: [],
      request,
    };
  });
  const editText = vi.fn(async (revision: string) => ({
    base_revision: revision,
    resulting_revision: `resident/${String(Number(revision.split("/")[1] ?? "0") + 1)}`,
    entities: ["item"],
    fields: ["task"],
    affected_calculations: [],
  }));
  const client: DesignerClient = {
    bootstrap: async () => ({
      title: "Driver Tracker",
      revision: table.revision,
      default_collection: "tracker",
      collections: [table.collection, ordinary.collection],
    }),
    queryTable,
    trackerCommand,
    openProject: vi.fn(),
    exportProject: vi.fn(async revision => ({ revision, bytes: new ArrayBuffer(1) })),
    closeProject: vi.fn(),
    queryFields: vi.fn(async () => []),
    editNumber: vi.fn(),
    editText,
    editBoolean: vi.fn(),
    editDate: vi.fn(),
    close: vi.fn(),
  };
  const host: DesignerProjectHost = { list: async () => [], read: vi.fn(), publish: vi.fn() };
  const root = document.createElement("div");
  document.body.append(root);
  mounted = mountDesigner(root, client, host);
  await mounted.ready;
  return { root, client, queryTable, trackerCommand, editText };
}

function click(root: HTMLElement, selector: string): void {
  const button = root.querySelector<HTMLButtonElement>(selector);
  expect(button, selector).not.toBeNull();
  button?.click();
}

async function switchCollection(root: HTMLElement, collection: string): Promise<void> {
  const select = root.querySelector<HTMLSelectElement>("[data-collection-select]");
  if (!select) throw new Error("No collection selector");
  select.value = collection;
  select.dispatchEvent(new Event("change", { bubbles: true }));
  await vi.waitFor(() => expect(root.querySelector<HTMLSelectElement>("[data-collection-select]")?.disabled).toBe(false));
}

it("keeps prior Tracker actions undoable after a successful generic semantic edit", async () => {
  const { root, client, queryTable } = await setup();
  click(root, '[data-tracker="append"]');
  await vi.waitFor(() => expect(root.querySelector<HTMLButtonElement>('[data-tracker="undo"]')?.disabled).toBe(false));

  await switchCollection(root, "ordinary");
  client.queryFields = vi.fn(async () => [{
    target: { entity: "item", field: "task" },
    address: "item.task",
    stored: { kind: "text", value: "Accepted work" },
    formula: null,
    calculated: null,
    diagnostics: [],
    editable_scalar: "text",
  }]);
  queryTable.mockResolvedValueOnce({ ...trackerTable("resident/2"), collection: { id: "ordinary", key: "ordinary", entity_count: 1 } });
  root.querySelector<HTMLFormElement>('[data-edit-kind="text"]')?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
  await vi.waitFor(() => expect(root.querySelector('[data-testid="revision"]')?.textContent).toBe("resident/2"));

  queryTable.mockResolvedValueOnce(trackerTable("resident/2"));
  await switchCollection(root, "tracker");
  const undo = root.querySelector<HTMLButtonElement>('[data-tracker="undo"]');
  expect(undo?.disabled).toBe(false);
  expect(root.textContent).not.toContain("undo/redo cleared after an edit outside Tracker");
});

it("preserves chronological undo/redo across semantic and presentation actions", async () => {
  const { root } = await setup();
  click(root, '[data-tracker="bold"]');
  click(root, '[data-tracker="append"]');
  await vi.waitFor(() => expect(root.querySelector<HTMLButtonElement>('[data-tracker="undo"]')?.disabled).toBe(false));

  click(root, '[data-tracker="undo"]');
  await vi.waitFor(() => expect(root.querySelector<HTMLButtonElement>('[data-tracker="redo"]')?.disabled).toBe(false));
  expect(root.querySelector("[role=gridcell]")?.className).toContain("cell-bold");

  click(root, '[data-tracker="undo"]');
  expect(root.querySelector("[role=gridcell]")?.className).not.toContain("cell-bold");

  click(root, '[data-tracker="redo"]');
  expect(root.querySelector("[role=gridcell]")?.className).toContain("cell-bold");
  click(root, '[data-tracker="redo"]');
  await vi.waitFor(() => expect(root.querySelector<HTMLButtonElement>('[data-tracker="redo"]')?.disabled).toBe(true));
});

it("keeps the undo stack across Save but starts a fresh stack after reopening", async () => {
  const { root } = await setup();
  click(root, '[data-tracker="bold"]');
  expect(root.querySelector<HTMLButtonElement>('[data-tracker="undo"]')?.disabled).toBe(false);

  vi.stubGlobal("prompt", vi.fn(() => "action-history.roproj"));
  click(root, "[data-save-project]");
  await vi.waitFor(() => expect(root.textContent).toContain("Save As complete"));
  expect(root.querySelector<HTMLButtonElement>('[data-tracker="undo"]')?.disabled).toBe(false);
});
