import { describe, expect, it } from "vitest";

import { parseReportCharts, projectReportChart, type ReportChart } from "../src/report-model.ts";
import { cellKey, type NumberFormat } from "../src/tracker-model.ts";
import type { FieldProjection, StoredValueProjection, TableProjection } from "../src/runtime/protocol.ts";

const chartId = "00000000-0000-4000-8000-000000000260";

function field(entity: string, fieldId: string, stored: StoredValueProjection | null, extra: Partial<FieldProjection> = {}): FieldProjection {
  return {
    target: { entity, field: fieldId },
    address: `[${entity}.${fieldId}]`,
    stored,
    formula: null,
    calculated: null,
    diagnostics: [],
    editable_scalar: stored?.kind === "number" ? "number" : null,
    ...extra,
  };
}

function table(rows: TableProjection["rows"] = [
  { id: "a", key: "A", fields: [field("a", "name", { kind: "text", value: "Alpha" }), field("a", "value", { kind: "number", value: 10 })] },
  { id: "b", key: "B", fields: [field("b", "name", { kind: "text", value: "Beta" }), field("b", "value", { kind: "number", value: 20 })] },
]): TableProjection {
  return {
    revision: "resident/7",
    collection: { id: "people", key: "People", entity_count: rows.length },
    columns: [
      { id: "name", key: "Name", field_type: "text" },
      { id: "value", key: "Value", field_type: "number" },
    ],
    rows,
  };
}

function chart(overrides: Partial<ReportChart> = {}): ReportChart {
  return {
    id: chartId,
    collectionId: "people",
    entityIds: ["a", "b"],
    categoryFieldId: "name",
    series: [{ fieldId: "value", label: "Value" }],
    kind: "column",
    title: "Values",
    xLabel: "Person",
    yLabel: "Amount",
    legend: true,
    ...overrides,
  };
}

describe("private report chart model", () => {
  it("parses the closed bounded shape while permitting bindings that are absent from a later table", () => {
    const parsed = parseReportCharts([chart({ entityIds: ["deleted-row"] })], ["people"]);
    expect(parsed[0]?.entityIds).toEqual(["deleted-row"]);
    expect(projectReportChart(parsed[0]!, table(), {})).toEqual({ status: "unavailable", message: "A selected report chart row is unavailable." });
  });

  it("rejects unknown properties, foreign collections, duplicate bindings, and bound overflow", () => {
    expect(() => parseReportCharts([{ ...chart(), extra: true }], ["people"])).toThrow(/property/);
    expect(() => parseReportCharts([chart({ collectionId: "other" })], ["people"])).toThrow(/authoritative/);
    expect(() => parseReportCharts([chart({ entityIds: ["a", "a"] })], ["people"])).toThrow(/unique/);
    expect(() => parseReportCharts([chart({ series: [{ fieldId: "value", label: "A" }, { fieldId: "value", label: "B" }] })], ["people"])).toThrow(/unique/);
    expect(() => parseReportCharts([chart({ entityIds: Array.from({ length: 17 }, (_, index) => String(index)) })], ["people"])).toThrow(/16/);
    expect(() => parseReportCharts([chart({ title: "x".repeat(81) })], ["people"])).toThrow(/80/);
    expect(() => parseReportCharts([chart({ id: "00000000-0000-4000-8000-00000000026A" })], ["people"])).toThrow(/UUID/);
  });

  it("uses saved entity order independently of table sorting and reports missing optional fields", () => {
    const reordered = table([table().rows[1]!, table().rows[0]!]);
    const projection = projectReportChart(chart(), reordered, {});
    expect(projection.status).toBe("ready");
    if (projection.status === "ready") expect(projection.labels).toEqual(["Alpha", "Beta"]);
    expect(projectReportChart(chart({ categoryFieldId: "missing" }), table(), {})).toEqual({ status: "unavailable", message: "A report chart category field is unavailable." });
  });

  it("accepts current calculated formula values only and never stored fallback values", () => {
    const formulaValue = field("a", "value", { kind: "number", value: 999 }, { formula: { source: "[a.base] * 2" }, calculated: { status: "value", value: 12 } });
    const formulaFailure = field("b", "value", { kind: "number", value: 999 }, { formula: { source: "[b.base] * 2" }, calculated: { status: "failure", code: "cycle", message: "cycle" } });
    const source = table([
      { id: "a", key: "A", fields: [field("a", "name", { kind: "text", value: "A" }), formulaValue] },
      { id: "b", key: "B", fields: [field("b", "name", { kind: "text", value: "B" }), formulaFailure] },
    ]);
    expect(projectReportChart(chart(), source, {})).toEqual({ status: "unavailable", message: "Numeric series 'Value' has an unavailable value." });
    formulaFailure.calculated = { status: "value", value: 0 };
    const current = projectReportChart(chart(), source, {});
    expect(current.status).toBe("ready");
    if (current.status === "ready") expect(current.series[0]?.values).toEqual([12, 0]);
  });

  it("supports text, date, boolean and implicit row categories and keeps long source labels unavailable", () => {
    const scalarTable = table([
      { id: "a", key: "A", fields: [field("a", "category", { kind: "date", value: "2026-09-05" }), field("a", "value", { kind: "number", value: 1 })] },
      { id: "b", key: "B", fields: [field("b", "category", { kind: "boolean", value: false }), field("b", "value", { kind: "number", value: 2 })] },
    ]);
    const scalar = projectReportChart(chart({ categoryFieldId: "category" }), { ...scalarTable, columns: [{ id: "category", key: "Category", field_type: "date" }, ...scalarTable.columns.slice(1)] }, {});
    expect(scalar.status).toBe("ready");
    if (scalar.status === "ready") expect(scalar.labels).toEqual(["2026-09-05", "false"]);
    const implicit = projectReportChart(chart({ categoryFieldId: null }), table(), {});
    expect(implicit.status).toBe("ready");
    if (implicit.status === "ready") expect(implicit.labels).toEqual(["Row 1", "Row 2"]);
    const long = table([{ id: "a", key: "A", fields: [field("a", "name", { kind: "text", value: "x".repeat(81) }), field("a", "value", { kind: "number", value: 1 })] }, table().rows[1]!]);
    expect(projectReportChart(chart(), long, {})).toEqual({ status: "unavailable", message: "A report chart category label exceeds 80 Unicode code points." });
  });

  it("inherits one number format and downgrades mixed cells to plain Number", () => {
    const formats: Record<string, NumberFormat> = {
      [cellKey("a", "value")]: "currency-jpy",
      [cellKey("b", "value")]: "currency-usd",
    };
    const mixed = projectReportChart(chart(), table(), formats);
    expect(mixed.status).toBe("ready");
    if (mixed.status === "ready") {
      expect(mixed.numberFormat).toBe("number");
      expect(mixed.note).toMatch(/Mixed/);
    }
    const jpy = projectReportChart(chart(), table(), { [cellKey("a", "value")]: "currency-jpy", [cellKey("b", "value")]: "currency-jpy" });
    expect(jpy.status).toBe("ready");
    if (jpy.status === "ready") expect(jpy.numberFormat).toBe("currency-jpy");
  });

  it("requires the wire-exact lowercase number field type even when row data is present", () => {
    const upper = { ...table(), columns: table().columns.map(column => column.id === "value" ? { ...column, field_type: "NUMBER" } : column) };
    expect(projectReportChart(chart(), upper, {})).toEqual({ status: "unavailable", message: "Numeric series 'Value' has an unavailable field." });
  });
});
