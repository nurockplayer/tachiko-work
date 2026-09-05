import { describe, expect, it, vi } from "vitest";
import { mountReportPanel, type ReportPanelState } from "../src/report-panel.ts";
import type { ReportChart } from "../src/report-model.ts";
import type { NumberFormat } from "../src/tracker-model.ts";
import type { FieldProjection, TableProjection } from "../src/runtime/protocol.ts";

const renderReportChart = vi.hoisted(() => vi.fn());
vi.mock("../src/report-renderer.ts", () => ({
  renderReportChart,
  formatReportNumber: (value: number, format: NumberFormat = "number") => format === "number" ? String(value) : `${format}:${String(value)}`,
}));

const collectionId = "00000000-0000-4000-8000-000000000010";
const firstRow = "00000000-0000-4000-8000-000000000011";
const secondRow = "00000000-0000-4000-8000-000000000012";
const thirdRow = "00000000-0000-4000-8000-000000000013";
const amountId = "00000000-0000-4000-8000-000000000021";
const secondAmountId = "00000000-0000-4000-8000-000000000023";
const categoryId = "00000000-0000-4000-8000-000000000022";
const chartId = "00000000-0000-4000-8000-000000000031";

function field(entity: string, fieldId: string, stored: FieldProjection["stored"]): FieldProjection {
  return { target: { entity, field: fieldId }, address: `${entity}.${fieldId}`, stored, formula: null, calculated: null, diagnostics: [], editable_scalar: stored?.kind === "number" ? "number" : stored?.kind === "text" ? "text" : null };
}

function table(): TableProjection {
  return {
    revision: "resident/7",
    collection: { id: collectionId, key: "Monthly sales", entity_count: 3 },
    columns: [
      { id: categoryId, key: "Store", field_type: "text" },
      { id: amountId, key: "Revenue", field_type: "number" },
    ],
    rows: [
      { id: firstRow, key: "row_1", fields: [field(firstRow, categoryId, { kind: "text", value: "東京" }), field(firstRow, amountId, { kind: "number", value: 100 })] },
      { id: secondRow, key: "row_2", fields: [field(secondRow, categoryId, { kind: "text", value: "Osaka" }), field(secondRow, amountId, { kind: "number", value: 200 })] },
      { id: thirdRow, key: "row_3", fields: [field(thirdRow, categoryId, null), field(thirdRow, amountId, { kind: "number", value: 300 })] },
    ],
  };
}

function chart(): ReportChart {
  return { id: chartId, collectionId, entityIds: [firstRow, secondRow], categoryFieldId: categoryId, series: [{ fieldId: amountId, label: "Revenue" }], kind: "column", title: "Sales", xLabel: "Store", yLabel: "JPY", legend: true };
}

function mount(options: Partial<Parameters<typeof mountReportPanel>[1]> = {}) {
  const host = document.createElement("div");
  const state: ReportPanelState = { draft: null };
  const activeState = options.state ?? state;
  const onChartsChange = vi.fn();
  const rerender = vi.fn();
  mountReportPanel(host, {
    table: table(), charts: [], formats: {}, collectionIds: [collectionId], current: true, busy: false, state: activeState,
    onChartsChange, onDraftChange: vi.fn(), rerender, onExport: vi.fn().mockResolvedValue(undefined), ...options,
  });
  return { host, state: activeState, onChartsChange, rerender };
}

function button(root: HTMLElement, text: string): HTMLButtonElement {
  const result = [...root.querySelectorAll("button")].find(item => item.textContent === text);
  if (!result) throw new Error(`Missing button ${text}`);
  return result;
}

function control(root: HTMLElement, name: string): HTMLInputElement | HTMLSelectElement {
  const result = root.querySelector<HTMLInputElement | HTMLSelectElement>(`[aria-label="${name}"]`);
  if (!result) throw new Error(`Missing control ${name}`);
  return result;
}

describe("report chart panel", () => {
  it("uses human source and row labels and keeps binding IDs out of the UI", () => {
    const { host, state, rerender } = mount();
    expect(host.textContent).toContain("No saved charts for this source yet.");
    button(host, "Create chart from selected source").click();
    expect(state.draft?.creating).toBe(true);
    expect(rerender).toHaveBeenCalledOnce();
    // The caller normally rerenders after this callback; mimic that root-owned lifecycle.
    host.replaceChildren();
    mountReportPanel(host, { table: table(), charts: [], formats: {}, collectionIds: [collectionId], current: true, busy: false, state, onChartsChange: vi.fn(), onDraftChange: vi.fn(), rerender: vi.fn(), onExport: vi.fn().mockResolvedValue(undefined) });
    expect(host.textContent).toContain("東京");
    expect(host.textContent).toContain("Row 3");
    expect(host.textContent).not.toContain(collectionId);
    expect(host.textContent).not.toContain(firstRow);
    expect(host.textContent).not.toContain(amountId);
    expect(host.textContent).toContain("Monthly sales");
    expect((control(host, "Chart source") as HTMLSelectElement).selectedOptions[0]?.textContent).toBe("Monthly sales");
  });

  it("retains a draft when full candidate validation rejects limits or title", () => {
    const tooMany = Array.from({ length: 17 }, (_, index) => `00000000-0000-4000-8000-${String(index + 100).padStart(12, "0")}`);
    const state: ReportPanelState = { draft: { creating: true, chart: { ...chart(), id: "00000000-0000-4000-8000-000000000041", entityIds: tooMany } } };
    const { host, onChartsChange } = mount({ state });
    button(host, "Apply chart").click();
    expect(onChartsChange).not.toHaveBeenCalled();
    expect(state.draft?.chart.entityIds).toHaveLength(17);
    expect(host.textContent).toContain("between 1 and 16 rows");

    state.draft!.chart.entityIds = [firstRow];
    control(host, "Chart title").value = "";
    control(host, "Chart title").dispatchEvent(new Event("input"));
    button(host, "Apply chart").click();
    expect(onChartsChange).not.toHaveBeenCalled();
    expect(state.draft?.chart.title).toBe("");
    expect(host.textContent).toContain("non-blank");
  });

  it("projects ready charts into an accessible table and supports edit/delete", () => {
    const saved = chart();
    renderReportChart.mockImplementation(() => document.createElement("canvas"));
    const onExport = vi.fn().mockResolvedValue(undefined);
    const { host, state, onChartsChange, rerender } = mount({ charts: [saved], onExport });
    expect(host.textContent).toContain("Sales data");
    expect(host.textContent).toContain("Revenue");
    expect(host.textContent).toContain("100");
    expect(host.querySelector("table")).not.toBeNull();
    button(host, "Edit chart").click();
    expect(state.draft?.creating).toBe(false);
    expect(rerender).toHaveBeenCalled();
    host.replaceChildren();
    mountReportPanel(host, { table: table(), charts: [saved], formats: {}, collectionIds: [collectionId], current: true, busy: false, state, onChartsChange, onDraftChange: vi.fn(), rerender: vi.fn(), onExport });
    const title = control(host, "Chart title");
    title.value = "Edited";
    title.dispatchEvent(new Event("input"));
    button(host, "Apply chart").click();
    expect(onChartsChange).toHaveBeenCalledWith([expect.objectContaining({ id: chartId, title: "Edited" })]);

    const deleteRoot = document.createElement("div");
    const deleted = vi.fn();
    mountReportPanel(deleteRoot, { table: table(), charts: [saved], formats: {}, collectionIds: [collectionId], current: true, busy: false, state: { draft: null }, onChartsChange: deleted, onDraftChange: vi.fn(), rerender: vi.fn(), onExport });
    button(deleteRoot, "Delete chart").click();
    expect(deleted).toHaveBeenCalledWith([]);
  });

  it("shows source failures as unavailable and disables PNG", () => {
    const saved = chart();
    saved.entityIds = [firstRow, "00000000-0000-4000-8000-000000000099"];
    const { host, state, onChartsChange } = mount({ charts: [saved] });
    expect(host.textContent).toContain("Chart unavailable");
    expect((button(host, "Download PNG")).disabled).toBe(true);
    button(host, "Edit chart").click();
    expect(state.draft?.creating).toBe(false);
    const deleteRoot = document.createElement("div");
    mountReportPanel(deleteRoot, { table: table(), charts: [saved], formats: {}, collectionIds: [collectionId], current: true, busy: false, state: { draft: null }, onChartsChange, onDraftChange: vi.fn(), rerender: vi.fn(), onExport: vi.fn().mockResolvedValue(undefined) });
    button(deleteRoot, "Delete chart").click();
    expect(onChartsChange).toHaveBeenCalledWith([]);

    const pending = mount({ charts: [chart()], current: false });
    expect(pending.host.textContent).toContain("waiting for current data");
    expect(pending.host.querySelector("canvas")).toBeNull();
    expect(button(pending.host, "Download PNG").disabled).toBe(true);
  });

  it("does not accumulate Add numeric series controls when the editor redraws", () => {
    const source = table();
    source.columns.push({ id: secondAmountId, key: "Cost", field_type: "number" });
    for (const row of source.rows) row.fields.push(field(row.id, secondAmountId, { kind: "number", value: 50 }));
    const { host, state } = mount({ table: source });
    button(host, "Create chart from selected source").click();
    host.replaceChildren();
    mountReportPanel(host, { table: source, charts: [], formats: {}, collectionIds: [collectionId], current: true, busy: false, state, onChartsChange: vi.fn(), onDraftChange: vi.fn(), rerender: vi.fn(), onExport: vi.fn().mockResolvedValue(undefined) });
    expect(host.querySelectorAll(".report-add-series")).toHaveLength(1);
    button(host, "Add numeric series").click();
    expect(host.querySelectorAll(".report-add-series")).toHaveLength(1);
    expect(host.textContent).toContain("Cost");
  });
});
