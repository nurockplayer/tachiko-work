import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { downloadCurrentReport, type ReportExportState } from "../src/report-export.ts";

const render = vi.hoisted(() => vi.fn());
vi.mock("../src/report-renderer.ts", () => ({ renderReportChart: render }));

function state(): ReportExportState {
  return {
    occurrence: Symbol("project"), alive: true, current: true, hasDrafts: false, formats: {},
    charts: [{ id: "00000000-0000-4000-8000-000000000260", collectionId: "source", entityIds: ["row"], categoryFieldId: null, series: [{ fieldId: "amount", label: "Actual" }], kind: "line", title: "Current report", xLabel: "", yLabel: "", legend: true }],
    table: { revision: "resident/2", collection: { id: "source", key: "Budget", entity_count: 1 }, columns: [{ id: "amount", key: "Actual", field_type: "number" }], rows: [{ id: "row", key: "Row", fields: [{ target: { entity: "row", field: "amount" }, address: "[row.amount]", stored: { kind: "number", value: 999 }, formula: { source: "1+1" }, calculated: { status: "value", value: 2 }, diagnostics: [], editable_scalar: null }] }] },
  };
}

describe("PNG publication currentness", () => {
  let encode: BlobCallback;
  let current: ReportExportState;
  const click = vi.fn();
  const createUrl = vi.fn(() => "blob:report");
  const revokeUrl = vi.fn();
  beforeEach(() => {
    vi.useFakeTimers();
    current = state();
    render.mockReset().mockReturnValue({ toBlob: (callback: BlobCallback) => { encode = callback; } });
    click.mockClear(); createUrl.mockClear(); revokeUrl.mockClear();
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(click);
    vi.spyOn(URL, "createObjectURL").mockImplementation(createUrl);
    vi.spyOn(URL, "revokeObjectURL").mockImplementation(revokeUrl);
  });
  afterEach(() => { vi.runAllTimers(); vi.useRealTimers(); vi.restoreAllMocks(); });

  it("renders authoritative calculated values and publishes only after PNG encoding", async () => {
    const pending = downloadCurrentReport(current.charts[0]!.id, () => current);
    expect(render.mock.calls[0]?.[0]).toMatchObject({ revision: "resident/2", series: [{ values: [2] }] });
    expect(click).not.toHaveBeenCalled();
    encode(new Blob(["encoded PNG"], { type: "image/png" }));
    await pending;
    expect(click).toHaveBeenCalledOnce();
    expect(createUrl).toHaveBeenCalledOnce();
    vi.runAllTimers();
    expect(revokeUrl).toHaveBeenCalledWith("blob:report");
  });

  const changes: [string, (value: ReportExportState) => void][] = [
    ["revision", value => { value.table!.revision = "resident/3"; }],
    ["collection", value => { value.table!.collection.id = "other"; }],
    ["same-revision replacement occurrence", value => { value.occurrence = Symbol("replacement"); }],
    ["refresh pending", value => { value.current = false; }],
    ["draft", value => { value.hasDrafts = true; }],
    ["closed occurrence", value => { value.alive = false; }],
    ["missing table", value => { value.table = null; }],
    ["deleted chart", value => { value.charts = []; }],
    ["changed chart", value => { value.charts[0]!.title = "Changed"; }],
    ["changed format", value => { value.formats['["row","amount"]'] = "percentage"; }],
  ];
  it.each(changes)("refuses a delayed PNG after %s", async (_name, change) => {
    const pending = downloadCurrentReport(current.charts[0]!.id, () => current);
    // The host can replace its snapshot or mutate a view object during encoding.
    current = { ...current };
    change(current);
    encode(new Blob(["encoded PNG"], { type: "image/png" }));
    await expect(pending).rejects.toThrow("changed while PNG");
    expect(createUrl).not.toHaveBeenCalled();
    expect(click).not.toHaveBeenCalled();
  });

  it.each([null, new Blob([], { type: "image/png" }), new Blob(["wrong"], { type: "image/jpeg" })])("rejects failed encoders", async blob => {
    const pending = downloadCurrentReport(current.charts[0]!.id, () => current);
    encode(blob);
    await expect(pending).rejects.toThrow("encode");
    expect(createUrl).not.toHaveBeenCalled();
  });

  it("does not render failed formulas or pending drafts", async () => {
    current.table!.rows[0]!.fields[0]!.calculated = { status: "failure", code: "cycle", message: "cycle" };
    await expect(downloadCurrentReport(current.charts[0]!.id, () => current)).rejects.toThrow("unavailable value");
    current.hasDrafts = true;
    await expect(downloadCurrentReport(current.charts[0]!.id, () => current)).rejects.toThrow("pending edits");
    expect(render).not.toHaveBeenCalled();
  });
});
