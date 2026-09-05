import { afterEach, expect, it, vi } from "vitest";
import { mountDesigner, type MountedDesigner } from "../src/designer-app.ts";
import type { DesignerProjectHost } from "../src/host/browser-project-host.ts";
import type { DesignerClient } from "../src/runtime/client.ts";
import type { PublicationProjection, TableProjection, TrackerCommand } from "../src/runtime/protocol.ts";

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
      fields: [
        {
          target: { entity: "item", field: "task" },
          address: "item.task",
          stored: { kind: "text", value: "Accepted work" },
          formula: null,
          calculated: null,
          diagnostics: [],
          editable_scalar: "text",
        },
        {
          target: { entity: "item", field: "estimate" },
          address: "item.estimate",
          stored: { kind: "number", value: 2 },
          formula: null,
          calculated: null,
          diagnostics: [],
          editable_scalar: "number",
        },
        {
          target: { entity: "item", field: "done" },
          address: "item.done",
          stored: { kind: "boolean", value: false },
          formula: null,
          calculated: null,
          diagnostics: [],
          editable_scalar: "boolean",
        },
      ],
    }],
  };
}

async function setup() {
  let table = trackerTable();
  const ordinary = structuredClone(table);
  delete ordinary.tracker_profile;
  ordinary.collection = { id: "ordinary", key: "ordinary", entity_count: 1 };

  const queryTable = vi.fn(async (collection: string) => structuredClone(
    collection === "ordinary" ? { ...ordinary, revision: table.revision } : table,
  ));
  const trackerCommand = vi.fn(async (request: TrackerCommand): Promise<PublicationProjection> => {
    const base = table.revision;
    const next = Number(base.split("/")[1] ?? "0") + 1;
    table = { ...table, revision: `resident/${String(next)}` };
    return {
      base_revision: base,
      resulting_revision: table.revision,
      entities: request.type === "append_row" ? ["item"] : [],
      fields: [],
      affected_calculations: [],
    };
  });
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
    queryFields: vi.fn(),
    editNumber: vi.fn(),
    editText: vi.fn(),
    editBoolean: vi.fn(),
    editDate: vi.fn(),
    close: vi.fn(),
  };
  const host: DesignerProjectHost = { list: async () => [], read: vi.fn(), publish: vi.fn() };
  const root = document.createElement("div");
  document.body.append(root);
  mounted = mountDesigner(root, client, host);
  await mounted.ready;
  return { root, client, queryTable, trackerCommand };
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

it("keeps generic semantic edits and earlier Tracker actions in one chronological undo stack", async () => {
  const { root, client, queryTable, trackerCommand } = await setup();
  click(root, '[data-tracker="append"]');
  await vi.waitFor(() => expect(root.querySelector<HTMLButtonElement>('[data-tracker="undo"]')?.disabled).toBe(false));

  await switchCollection(root, "ordinary");
  client.editText = vi.fn().mockResolvedValue({
    base_revision: "resident/1",
    resulting_revision: "resident/2",
    entities: [],
    fields: [{ entity: "item", field: "task" }],
    affected_calculations: [],
  });
  client.queryFields = vi.fn().mockRejectedValue(new Error("refresh failed"));
  root.querySelector<HTMLFormElement>('[data-edit-kind="text"]')?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
  await vi.waitFor(() => expect(root.textContent).toContain("Edit published; refresh incomplete"));

  queryTable.mockResolvedValueOnce(trackerTable("resident/2"));
  await switchCollection(root, "tracker");
  const undo = root.querySelector<HTMLButtonElement>('[data-tracker="undo"]');
  expect(undo?.disabled).toBe(false);
  expect(root.textContent).not.toContain("undo/redo cleared after an edit outside Tracker");

  click(root, '[data-tracker="undo"]');
  await vi.waitFor(() => expect(trackerCommand.mock.calls.at(-1)?.[0]).toEqual(expect.objectContaining({ type: "undo", expected_revision: "resident/2" })));
  expect(root.querySelector<HTMLButtonElement>('[data-tracker="undo"]')?.disabled).toBe(false);
  click(root, '[data-tracker="undo"]');
  await vi.waitFor(() => expect(trackerCommand.mock.calls.filter(([request]) => request.type === "undo")).toHaveLength(2));
});

it("preserves chronological undo/redo across Tracker semantic and presentation actions", async () => {
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

it("treats chart create as one undoable presentation action without discarding earlier formatting", async () => {
  const { root } = await setup();
  click(root, '[data-tracker="bold"]');
  expect(root.querySelector<HTMLButtonElement>('[data-tracker="undo"]')?.disabled).toBe(false);

  const create = root.querySelector<HTMLButtonElement>(".report-panel-actions button");
  if (!create) throw new Error("Create chart control is required");
  create.click();
  const apply = root.querySelector<HTMLButtonElement>('.report-editor button[type="submit"]');
  if (!apply) throw new Error("Apply chart control is required");
  apply.click();
  await vi.waitFor(() => expect(root.querySelector(".report-card-title")?.textContent).toBe("Report"));

  expect(root.querySelector<HTMLButtonElement>('[data-tracker="undo"]')?.disabled).toBe(false);
  expect(root.textContent).not.toContain("undo/redo cleared");
  click(root, '[data-tracker="undo"]');
  expect(root.querySelector(".report-card-title")).toBeNull();
  expect(root.querySelector("[role=gridcell]")?.className).toContain("cell-bold");

  click(root, '[data-tracker="redo"]');
  await vi.waitFor(() => expect(root.querySelector(".report-card-title")?.textContent).toBe("Report"));
  expect(root.querySelector("[role=gridcell]")?.className).toContain("cell-bold");
});

it("keeps the current-session undo stack across Save As", async () => {
  const { root } = await setup();
  click(root, '[data-tracker="bold"]');
  expect(root.querySelector<HTMLButtonElement>('[data-tracker="undo"]')?.disabled).toBe(false);

  vi.stubGlobal("prompt", vi.fn(() => "action-history.roproj"));
  click(root, "[data-save-project]");
  await vi.waitFor(() => expect(root.textContent).toContain("Save As complete"));
  expect(root.querySelector<HTMLButtonElement>('[data-tracker="undo"]')?.disabled).toBe(false);
});
