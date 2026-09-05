// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const renderReportChart = vi.hoisted(() => vi.fn(() => document.createElement("canvas")));
vi.mock("../src/report-renderer.ts", () => ({
  renderReportChart,
  formatReportNumber: (value: number) => String(value),
}));

import { mountDesigner } from "../src/designer-app.ts";
import { emptyTrackerView } from "../src/tracker-model.ts";
import type { ReportChart } from "../src/report-model.ts";
import type { DesignerClient } from "../src/runtime/client.ts";
import type {
  BootstrapProjection,
  FieldBatchProjection,
  OpenedProjection,
  PublicationProjection,
  ProjectExport,
  TableProjection,
} from "../src/runtime/protocol.ts";
import type { DesignerProjectHost, SavedProjectSummary } from "../src/host/browser-project-host.ts";

const alpha = "alpha";
const beta = "beta";
const bootstrap: BootstrapProjection = {
  title: "Report App",
  revision: "resident/0",
  default_collection: alpha,
  collections: [
    { id: alpha, key: alpha, entity_count: 1 },
    { id: beta, key: beta, entity_count: 1 },
  ],
};

function table(collection: string, rowId: string, label: string, value: number): TableProjection {
  return {
    tracker_profile: false,
    revision: "resident/0",
    collection: { id: collection, key: collection, entity_count: 1 },
    columns: [
      { id: "name", key: "Name", field_type: "text" },
      { id: "value", key: "Value", field_type: "number" },
    ],
    rows: [{
      id: rowId,
      key: rowId,
      fields: [
        { target: { entity: rowId, field: "name" }, address: `${rowId}.name`, stored: { kind: "text", value: label }, formula: null, calculated: null, diagnostics: [], editable_scalar: "text" },
        { target: { entity: rowId, field: "value" }, address: `${rowId}.value`, stored: { kind: "number", value }, formula: null, calculated: null, diagnostics: [], editable_scalar: "number" },
      ],
    }],
  };
}

function opened(): OpenedProjection {
  return { bootstrap: structuredClone(bootstrap), table: table(alpha, "row-a", "Alpha row", 12) };
}

class AppClient implements DesignerClient {
  readonly queryTable = vi.fn(async (collection: string): Promise<TableProjection> => collection === beta ? table(beta, "row-b", "Beta row", 8) : table(alpha, "row-a", "Alpha row", 12));
  readonly inspectProject = vi.fn(async (): Promise<OpenedProjection> => opened());
  readonly openProject = vi.fn(async (): Promise<OpenedProjection> => opened());

  async bootstrap(): Promise<BootstrapProjection> { return structuredClone(bootstrap); }
  async exportProject(expectedRevision: string): Promise<ProjectExport> { return { revision: expectedRevision, bytes: new ArrayBuffer(8) }; }
  async closeProject(): Promise<void> {}
  async queryFields(revision: string): Promise<FieldBatchProjection> { return { revision, fields: [] }; }
  async editNumber(expectedRevision: string): Promise<PublicationProjection> { return publication(expectedRevision); }
  async editText(expectedRevision: string): Promise<PublicationProjection> { return publication(expectedRevision); }
  async editBoolean(expectedRevision: string): Promise<PublicationProjection> { return publication(expectedRevision); }
  async editDate(expectedRevision: string): Promise<PublicationProjection> { return publication(expectedRevision); }
  close(): void {}
}

class TrackerAppClient extends AppClient {
  override readonly queryTable = vi.fn(async (collection: string): Promise<TableProjection> => collection === beta ? trackerTable(beta, "row-b", "Beta row", 8) : trackerTable(alpha, "row-a", "Alpha row", 12));
  override readonly inspectProject = vi.fn(async (): Promise<OpenedProjection> => trackerOpened());
  override readonly openProject = vi.fn(async (): Promise<OpenedProjection> => trackerOpened());
}

function trackerTable(collection: string, rowId: string, label: string, value: number): TableProjection {
  return { ...table(collection, rowId, label, value), tracker_profile: true };
}

function trackerOpened(): OpenedProjection {
  return { bootstrap: structuredClone(bootstrap), table: trackerTable(alpha, "row-a", "Alpha row", 12) };
}

function publication(revision: string): PublicationProjection {
  return { base_revision: revision, resulting_revision: revision, entities: [], fields: [], affected_calculations: [] };
}

class SnapshotHost implements DesignerProjectHost {
  constructor(readonly presentation: string | undefined) {}
  readonly bytes = new ArrayBuffer(8);
  async list(): Promise<SavedProjectSummary[]> { return [{ name: "saved.roproj", byte_length: this.bytes.byteLength, saved_at: "2026-09-05T00:00:00.000Z" }]; }
  async read(): Promise<ArrayBuffer> { return this.bytes.slice(0); }
  async readSnapshot(): Promise<{ bytes: ArrayBuffer; presentation?: string }> {
    return { bytes: this.bytes.slice(0), ...(this.presentation === undefined ? {} : { presentation: this.presentation }) };
  }
  async publish(): Promise<void> {}
}

class EmptyHost extends SnapshotHost {
  constructor() { super(undefined); }
  override async list(): Promise<SavedProjectSummary[]> { return []; }
}

function chart(overrides: Partial<ReportChart> = {}): ReportChart {
  return {
    id: "00000000-0000-4000-8000-000000000260",
    collectionId: alpha,
    entityIds: ["row-a"],
    categoryFieldId: null,
    series: [{ fieldId: "value", label: "Value" }],
    kind: "column",
    title: "Saved chart",
    xLabel: "",
    yLabel: "",
    legend: true,
    ...overrides,
  };
}

function presentation(charts: unknown): string {
  return JSON.stringify({ ...emptyTrackerView(), charts });
}

function rootElement(): HTMLElement {
  document.body.innerHTML = '<div id="app"></div>';
  const root = document.querySelector<HTMLElement>("#app");
  if (root === null) throw new Error("test root is required");
  return root;
}

describe("mounted Designer report integration", () => {
  beforeEach(() => {
    vi.stubGlobal("confirm", vi.fn(() => true));
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    document.body.innerHTML = "";
  });

  it.each([
    ["foreign collection", chart({ collectionId: "foreign" })],
    ["unknown chart property", { ...chart(), unexpected: true }],
  ])("rejects %s before replacing the resident project", async (_label, invalidChart) => {
    const root = rootElement();
    const client = new AppClient();
    const app = mountDesigner(root, client, new SnapshotHost(presentation([invalidChart])));
    await app.ready;
    const beforeRevision = root.querySelector<HTMLElement>('[data-testid="revision"]')?.textContent;

    const open = root.querySelector<HTMLButtonElement>("[data-open-project]");
    if (open === null) throw new Error("open control is required");
    open.click();
    await vi.waitFor(() => { expect(root.querySelector('[role="alert"]')?.textContent).toContain("Project not opened"); });
    expect(client.inspectProject).toHaveBeenCalledOnce();
    expect(client.openProject).not.toHaveBeenCalled();
    expect(root.querySelector<HTMLElement>("h1")?.textContent).toBe("Report App");
    expect(root.querySelector<HTMLElement>('[data-testid="revision"]')?.textContent).toBe(beforeRevision);
    app.destroy();
  });

  it("keeps a chart draft and original source when Collection changes", async () => {
    const root = rootElement();
    const client = new AppClient();
    const app = mountDesigner(root, client, new EmptyHost());
    await app.ready;
    root.querySelector<HTMLButtonElement>(".report-panel-actions button")?.click();
    expect(root.querySelector("h3")?.textContent).toContain("Create report chart");
    const callsBefore = client.queryTable.mock.calls.length;

    const collection = root.querySelector<HTMLSelectElement>("[data-collection-select]");
    if (collection === null) throw new Error("collection selector is required");
    collection.value = beta;
    collection.dispatchEvent(new Event("change", { bubbles: true }));

    expect(client.queryTable).toHaveBeenCalledTimes(callsBefore);
    expect(root.querySelector<HTMLElement>('[role="alert"]')?.textContent).toContain("Apply or cancel the chart draft");
    expect(root.querySelector<HTMLSelectElement>("[data-collection-select]")?.value).toBe(alpha);
    expect(root.querySelector("h3")?.textContent).toContain("Create report chart");
    expect(root.querySelector<HTMLButtonElement>('.report-editor button[type="submit"]')?.textContent).toBe("Apply chart");
    expect([...root.querySelectorAll("button")].some(button => button.textContent === "Cancel")).toBe(true);
    app.destroy();
  });

  it("clears Tracker undo/redo after accepted chart create, edit, and delete", async () => {
    const root = rootElement();
    const app = mountDesigner(root, new TrackerAppClient(), new EmptyHost());
    await app.ready;

    const format = (): HTMLButtonElement => {
      const button = root.querySelector<HTMLButtonElement>('[data-tracker="bold"]');
      if (button === null) throw new Error("format control is required");
      return button;
    };
    const undo = (): HTMLButtonElement => {
      const button = root.querySelector<HTMLButtonElement>('[data-tracker="undo"]');
      if (button === null) throw new Error("undo control is required");
      return button;
    };
    const redo = (): HTMLButtonElement => {
      const button = root.querySelector<HTMLButtonElement>('[data-tracker="redo"]');
      if (button === null) throw new Error("redo control is required");
      return button;
    };
    format().click();
    expect(undo().disabled).toBe(false);
    undo().click();
    expect(redo().disabled).toBe(false);
    redo().click();
    expect(undo().disabled).toBe(false);

    const create = root.querySelector<HTMLButtonElement>(".report-panel-actions button");
    if (create === null) throw new Error("create chart control is required");
    create.click();
    const apply = root.querySelector<HTMLButtonElement>('.report-editor button[type="submit"]');
    if (apply === null) throw new Error("apply chart control is required");
    apply.click();
    await vi.waitFor(() => { expect(root.querySelector(".report-card-title")?.textContent).toBe("Report"); });
    expect(undo().disabled).toBe(true);
    expect(redo().disabled).toBe(true);
    expect(root.querySelector('[role="status"]')?.textContent).toContain("undo/redo cleared");

    // Rebuild a real view-history stack before each subsequent external chart mutation.
    format().click();
    expect(undo().disabled).toBe(false);
    const edit = [...root.querySelectorAll<HTMLButtonElement>("button")].find(button => button.textContent === "Edit chart");
    if (edit === undefined) throw new Error("edit chart control is required");
    edit.click();
    const title = root.querySelector<HTMLInputElement>('[aria-label="Chart title"]');
    if (title === null) throw new Error("chart title control is required");
    title.value = "Edited chart";
    title.dispatchEvent(new InputEvent("input", { bubbles: true }));
    const editApply = root.querySelector<HTMLButtonElement>('.report-editor button[type="submit"]');
    if (editApply === null) throw new Error("edit apply control is required");
    editApply.click();
    await vi.waitFor(() => { expect(root.querySelector(".report-card-title")?.textContent).toBe("Edited chart"); });
    expect(undo().disabled).toBe(true);
    expect(redo().disabled).toBe(true);

    format().click();
    expect(undo().disabled).toBe(false);
    const remove = [...root.querySelectorAll<HTMLButtonElement>("button")].find(button => button.textContent === "Delete chart");
    if (remove === undefined) throw new Error("delete chart control is required");
    remove.click();
    expect(root.querySelector(".report-card-title")).toBeNull();
    expect(undo().disabled).toBe(true);
    expect(redo().disabled).toBe(true);
    expect(root.querySelector('[role="status"]')?.textContent).toContain("Accepted data and formatting are preserved");
    app.destroy();
  });

  it("hides chart canvas during a pending scalar edit and keeps PNG unavailable", async () => {
    const root = rootElement();
    const client = new AppClient();
    const app = mountDesigner(root, client, new SnapshotHost(presentation([chart()])));
    await app.ready;
    const open = root.querySelector<HTMLButtonElement>("[data-open-project]");
    if (open === null) throw new Error("open control is required");
    open.click();
    await vi.waitFor(() => { expect([...root.querySelectorAll("button")].some(button => button.textContent === "Edit chart")).toBe(true); });
    const value = root.querySelector<HTMLInputElement>("input[data-initial-number]");
    if (value === null) throw new Error("number input is required");
    value.value = "19";
    value.dispatchEvent(new InputEvent("input", { bubbles: true }));
    expect(root.querySelector("[data-report-host] canvas")).toBeNull();
    const downloads = [...root.querySelectorAll<HTMLButtonElement>("button")].filter(button => button.textContent === "Download PNG");
    expect(downloads.every(button => button.disabled)).toBe(true);
    app.destroy();
  });
});
