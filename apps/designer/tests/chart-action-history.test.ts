// @vitest-environment happy-dom
// Steward acceptance for #302. Ported from d0b18b1; no history implementation is mocked.
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const renderReportChart = vi.hoisted(() => vi.fn((projection: { chart: unknown }) => {
  const canvas = document.createElement("canvas");
  // Observe the complete rendering-boundary configuration, not only its title.
  canvas.dataset.acceptanceChart = JSON.stringify(projection.chart);
  return canvas;
}));
vi.mock("../src/report-renderer.ts", () => ({
  renderReportChart,
  formatReportNumber: (value: number) => String(value),
}));

import { mountDesigner, type MountedDesigner } from "../src/designer-app.ts";
import type { DesignerProjectHost, SavedProjectSummary } from "../src/host/browser-project-host.ts";
import type { ReportChart } from "../src/report-model.ts";
import type { DesignerClient } from "../src/runtime/client.ts";
import type {
  BootstrapProjection, FieldBatchProjection, OpenedProjection,
  ProjectExport, PublicationProjection, TableProjection,
} from "../src/runtime/protocol.ts";

const collection = "items";
const bootstrap: BootstrapProjection = {
  title: "Chart history", revision: "resident/0", default_collection: collection,
  collections: [{ id: collection, key: collection, entity_count: 1 }],
};
function table(tracker: boolean): TableProjection {
  return {
    tracker_profile: tracker, revision: "resident/0",
    collection: { id: collection, key: collection, entity_count: 1 },
    columns: [
      { id: "name", key: "Name", field_type: "text" },
      { id: "value", key: "Value", field_type: "number" },
    ],
    rows: [{ id: "row-a", key: "row-a", fields: [
      { target: { entity: "row-a", field: "name" }, address: "row-a.name", stored: { kind: "text", value: "Alpha" }, formula: null, calculated: null, diagnostics: [], editable_scalar: "text" },
      { target: { entity: "row-a", field: "value" }, address: "row-a.value", stored: { kind: "number", value: 12 }, formula: null, calculated: null, diagnostics: [], editable_scalar: "number" },
    ] }],
  };
}
class Client implements DesignerClient {
  constructor(readonly tracker: boolean) {}
  readonly queryTable = vi.fn(async (): Promise<TableProjection> => table(this.tracker));
  // A presentation-only operation must never reach a semantic mutation endpoint.
  readonly trackerCommand = vi.fn(async (): Promise<PublicationProjection> => { throw new Error("unexpected semantic mutation"); });
  async bootstrap(): Promise<BootstrapProjection> { return structuredClone(bootstrap); }
  async openProject(): Promise<OpenedProjection> { return { bootstrap: structuredClone(bootstrap), table: table(this.tracker) }; }
  async exportProject(expectedRevision: string): Promise<ProjectExport> { return { revision: expectedRevision, bytes: new ArrayBuffer(8) }; }
  async closeProject(): Promise<void> {}
  async queryFields(revision: string): Promise<FieldBatchProjection> { return { revision, fields: [] }; }
  async editNumber(): Promise<PublicationProjection> { throw new Error("unexpected scalar mutation"); }
  async editText(): Promise<PublicationProjection> { throw new Error("unexpected scalar mutation"); }
  async editBoolean(): Promise<PublicationProjection> { throw new Error("unexpected scalar mutation"); }
  async editDate(): Promise<PublicationProjection> { throw new Error("unexpected scalar mutation"); }
  close(): void {}
}
class EmptyHost implements DesignerProjectHost {
  async list(): Promise<SavedProjectSummary[]> { return []; }
  async read(): Promise<ArrayBuffer> { throw new Error("no saved project"); }
  async publish(): Promise<void> {}
}

let app: MountedDesigner | undefined;
beforeEach(() => {
  vi.clearAllMocks();
  vi.stubGlobal("confirm", vi.fn(() => true));
  document.body.innerHTML = '<div id="app"></div>';
});
afterEach(() => {
  app?.destroy(); app = undefined;
  vi.unstubAllGlobals(); document.body.innerHTML = "";
});
async function setup(tracker = true): Promise<{ root: HTMLElement; client: Client }> {
  const root = document.querySelector<HTMLElement>("#app");
  if (!root) throw new Error("test root required");
  const client = new Client(tracker);
  app = mountDesigner(root, client, new EmptyHost());
  await app.ready;
  return { root, client };
}
function control(root: HTMLElement, name: RegExp): HTMLButtonElement {
  const button = [...root.querySelectorAll<HTMLButtonElement>("button")].find(item =>
    name.test(item.getAttribute("aria-label") ?? item.textContent));
  if (!button) throw new Error(`missing control ${String(name)}`);
  return button;
}
function input(root: HTMLElement, label: string): HTMLInputElement {
  const field = root.querySelector<HTMLInputElement>(`input[aria-label="${label}"]`);
  if (!field) throw new Error(`missing input ${label}`);
  return field;
}
function fill(root: HTMLElement, label: string, value: string): void {
  const field = input(root, label);
  field.value = value;
  field.dispatchEvent(new InputEvent("input", { bubbles: true }));
}
function select(root: HTMLElement, label: string, value: string): void {
  const field = root.querySelector<HTMLSelectElement>(`select[aria-label="${label}"]`);
  if (!field) throw new Error(`missing select ${label}`);
  field.value = value;
  field.dispatchEvent(new Event("change", { bubbles: true }));
}
function snapshot(root: HTMLElement): ReportChart {
  const encoded = root.querySelector<HTMLCanvasElement>("[data-report-host] canvas")?.dataset.acceptanceChart;
  if (!encoded) throw new Error("a rendered chart configuration is required");
  return JSON.parse(encoded) as ReportChart;
}
function description(root: HTMLElement, name: RegExp): string {
  const button = control(root, name);
  const related = (button.getAttribute("aria-describedby") ?? "").split(/\s+/)
    .map(id => document.getElementById(id)?.textContent ?? "").join(" ");
  return [button.textContent, button.getAttribute("aria-label"), button.getAttribute("aria-description"), related].join(" ");
}
async function createChart(root: HTMLElement, title = "Report"): Promise<void> {
  control(root, /^Create chart from selected source$/).click();
  fill(root, "Chart title", title);
  select(root, "Category field", "name");
  control(root, /^Apply chart$/).click();
  await vi.waitFor(() => { expect(root.querySelector(".report-card-title")?.textContent).toBe(title); });
  expect(root.querySelector(".report-data-table")?.textContent).toContain("Alpha");
  expect(root.querySelector(".report-data-table")?.textContent).toContain("12");
}
function assertSourceUnchanged(root: HTMLElement, client: Client): void {
  expect(root.querySelector('[data-testid="revision"]')?.textContent).toContain("resident/0");
  expect(client.trackerCommand).not.toHaveBeenCalled();
}

it("interleaves chart creation with earlier formatting and restores both in order", async () => {
  const { root, client } = await setup();
  control(root, /^Bold$/).click();
  expect(root.querySelector("[role=gridcell]")?.className).toContain("cell-bold");
  await createChart(root);
  const created = snapshot(root);
  expect(control(root, /^Undo\b/i).disabled).toBe(false);
  expect(root.textContent).not.toContain("undo/redo cleared");
  control(root, /^Undo\b/i).click();
  await vi.waitFor(() => { expect(root.querySelector(".report-card-title")).toBeNull(); });
  expect(root.querySelector("[role=gridcell]")?.className).toContain("cell-bold");
  control(root, /^Undo\b/i).click();
  expect(root.querySelector("[role=gridcell]")?.className).not.toContain("cell-bold");
  control(root, /^Redo\b/i).click();
  expect(root.querySelector("[role=gridcell]")?.className).toContain("cell-bold");
  control(root, /^Redo\b/i).click();
  await vi.waitFor(() => { expect(root.querySelector(".report-card-title")?.textContent).toBe("Report"); });
  expect(snapshot(root)).toEqual(created);
  assertSourceUnchanged(root, client);
});

it.each([true, false])("restores exact chart edit/delete configuration on Tracker profile %s", async tracker => {
  const { root, client } = await setup(tracker);
  await createChart(root);
  const created = snapshot(root);
  control(root, /^Edit chart$/).click();
  fill(root, "Chart title", "Edited chart");
  fill(root, "X axis label", "Items");
  fill(root, "Y axis label", "Amount");
  fill(root, "Series 1 label", "Measured");
  select(root, "Chart type", "line");
  input(root, "Show legend").click();
  control(root, /^Apply chart$/).click();
  await vi.waitFor(() => { expect(root.querySelector(".report-card-title")?.textContent).toBe("Edited chart"); });
  const edited = snapshot(root);
  expect(edited).toEqual({ ...created, title: "Edited chart", xLabel: "Items", yLabel: "Amount", series: [{ fieldId: "value", label: "Measured" }], kind: "line", legend: false });
  control(root, /^Undo\b/i).click();
  await vi.waitFor(() => { expect(snapshot(root)).toEqual(created); });
  control(root, /^Redo\b/i).click();
  await vi.waitFor(() => { expect(snapshot(root)).toEqual(edited); });
  control(root, /^Delete chart$/).click();
  expect(root.querySelector(".report-card-title")).toBeNull();
  control(root, /^Undo\b/i).click();
  await vi.waitFor(() => { expect(snapshot(root)).toEqual(edited); });
  control(root, /^Redo\b/i).click();
  await vi.waitFor(() => { expect(root.querySelector(".report-card-title")).toBeNull(); });
  assertSourceUnchanged(root, client);
  if (!tracker) expect(root.querySelector('[data-tracker="bold"]')).toBeNull();
});

it.each(["cancel", "reject", "unchanged"] as const)("%s chart draft preserves both nonempty history directions", async action => {
  const { root, client } = await setup();
  await createChart(root);
  const created = snapshot(root);
  control(root, /^Bold$/).click();
  control(root, /^Fill$/).click();
  control(root, /^Undo\b/i).click();
  expect(control(root, /^Undo\b/i).disabled).toBe(false);
  expect(control(root, /^Redo\b/i).disabled).toBe(false);
  const beforeStyle = root.querySelector("[role=gridcell]")?.getAttribute("style");
  const beforeClass = root.querySelector("[role=gridcell]")?.className;
  control(root, /^Edit chart$/).click();
  if (action === "cancel") {
    fill(root, "Chart title", "Unaccepted draft");
    control(root, /^Cancel$/).click();
  } else if (action === "reject") {
    fill(root, "Chart title", "");
    control(root, /^Apply chart$/).click();
    expect(root.querySelector(".report-editor")).not.toBeNull();
    control(root, /^Cancel$/).click();
  } else {
    control(root, /^Apply chart$/).click();
  }
  await vi.waitFor(() => { expect(root.querySelector(".report-editor")).toBeNull(); });
  expect(snapshot(root)).toEqual(created);
  expect(control(root, /^Undo\b/i).disabled).toBe(false);
  expect(control(root, /^Redo\b/i).disabled).toBe(false);
  control(root, /^Redo\b/i).click();
  control(root, /^Undo\b/i).click();
  expect(root.querySelector("[role=gridcell]")?.getAttribute("style")).toBe(beforeStyle);
  expect(root.querySelector("[role=gridcell]")?.className).toBe(beforeClass);
  control(root, /^Undo\b/i).click();
  expect(root.querySelector("[role=gridcell]")?.className).not.toContain("cell-bold");
  expect(snapshot(root)).toEqual(created);
  assertSourceUnchanged(root, client);
});

it("new chart work after Undo discards only the abandoned Redo branch", async () => {
  const { root, client } = await setup();
  control(root, /^Bold$/).click();
  control(root, /^Fill$/).click();
  control(root, /^Undo\b/i).click();
  const boldOnlyClass = root.querySelector("[role=gridcell]")?.className;
  const boldOnlyStyle = root.querySelector("[role=gridcell]")?.getAttribute("style");
  expect(control(root, /^Redo\b/i).disabled).toBe(false);
  await createChart(root);
  expect(control(root, /^Redo\b/i).disabled).toBe(true);
  expect(control(root, /^Undo\b/i).disabled).toBe(false);
  control(root, /^Undo\b/i).click();
  await vi.waitFor(() => { expect(root.querySelector(".report-card-title")).toBeNull(); });
  expect(root.querySelector("[role=gridcell]")?.className).toBe(boldOnlyClass);
  expect(root.querySelector("[role=gridcell]")?.getAttribute("style")).toBe(boldOnlyStyle);
  control(root, /^Undo\b/i).click();
  expect(root.querySelector("[role=gridcell]")?.className).not.toContain("cell-bold");
  control(root, /^Redo\b/i).click();
  control(root, /^Redo\b/i).click();
  await vi.waitFor(() => { expect(root.querySelector(".report-card-title")?.textContent).toBe("Report"); });
  expect(root.querySelector("[role=gridcell]")?.getAttribute("style")).toBe(boldOnlyStyle);
  expect(control(root, /^Redo\b/i).disabled).toBe(true);
  assertSourceUnchanged(root, client);
});

it("discloses the next chart action accessibly in each direction", async () => {
  const { root } = await setup();
  await createChart(root);
  expect(description(root, /^Undo\b/i)).toMatch(/chart/i);
  control(root, /^Undo\b/i).click();
  await vi.waitFor(() => { expect(root.querySelector(".report-card-title")).toBeNull(); });
  expect(description(root, /^Redo\b/i)).toMatch(/chart/i);
});
