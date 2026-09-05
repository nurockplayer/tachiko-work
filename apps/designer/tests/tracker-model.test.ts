import { describe, expect, it } from "vitest";

import {
  cellKey,
  compareFields,
  emptyTrackerView,
  encodeTsv,
  orderedRows,
  parseTrackerView,
  parseTsv,
} from "../src/tracker-model.ts";
import type { FieldProjection, StoredValueProjection, TableProjection } from "../src/runtime/protocol.ts";

it("requires authoritative collection IDs for spreadsheet state even without Budget views", () => {
  const view = emptyTrackerView();
  view.interop = {
    version: 1,
    metadata: { version: 1, sheets: [{
      schema_id: "sheet", name: "Imported", has_header: true,
      columns: [{ field_id: "amount", name: "Amount", width: null }], rows: [],
    }] },
    ledger: [], source: { name: "data.csv", format: "csv", base64: "YQ==" }, tableViews: {},
  };
  const input = JSON.stringify(view);
  expect(view.budgetViews).toBeUndefined();
  expect(() => parseTrackerView(input)).toThrow("requires authoritative project collection IDs");
  expect(() => parseTrackerView(input, ["foreign"])).toThrow();
  expect(parseTrackerView(input, ["sheet"])).toEqual(view);
});

function field(id: string, stored: StoredValueProjection | null, error = false): FieldProjection {
  return {
    target: { entity: id, field: "value" },
    address: `${id}.value`,
    stored,
    formula: null,
    calculated: null,
    diagnostics: error ? [{ code: "invalid", message: "Invalid value", path: id }] : [],
    editable_scalar: null,
  };
}

function table(fields: FieldProjection[]): TableProjection {
  return {
    revision: "resident/4",
    collection: { id: "tracker", key: "tracker", entity_count: fields.length },
    columns: [{ id: "value", key: "value", field_type: "number" }],
    rows: fields.map(value => ({ id: value.target.entity, key: value.target.entity, fields: [value] })),
  };
}

describe("Driver Tracker clipboard grammar", () => {
  it("parses quoted tabs, CRLF, multiline text, and doubled quotes exactly", () => {
    expect(parseTsv('"a\tb"\t"line 1\r\nline 2"\t"say ""yes"""\r\nnext\t2\tfalse\r\n')).toEqual([
      ["a\tb", "line 1\r\nline 2", 'say "yes"'],
      ["next", "2", "false"],
    ]);
  });

  it("preserves empty cells and admits one optional row terminator", () => {
    expect(parseTsv("")).toEqual([[""]]);
    expect(parseTsv("\t\t\r\n")).toEqual([["", "", ""]]);
    expect(parseTsv("a\t\r\n\tb")).toEqual([["a", ""], ["", "b"]]);
    expect(parseTsv("a\rb\nc")).toEqual([["a"], ["b"], ["c"]]);
  });

  it.each([
    ["ragged", "a\tb\nc"],
    ["unclosed quote", '"unfinished'],
    ["trailing text after quote", '"closed"tail'],
    ["NUL", "bad\0value"],
    ["too many columns", "a\tb\tc\td"],
    ["too many rows", Array.from({ length: 129 }, () => "row").join("\n")],
    ["too many characters", "x".repeat(48_001)],
  ])("rejects %s without a partial parse", (_label, input) => {
    expect(() => parseTsv(input)).toThrow();
  });

  it("accepts exact clipboard row, column, and character limits", () => {
    const rows = Array.from({ length: 128 }, () => ["a", "b", "c"]);
    expect(parseTsv(encodeTsv(rows))).toEqual(rows);
    expect(parseTsv("x".repeat(48_000))).toEqual([["x".repeat(48_000)]]);
  });

  it.each([
    [["first"], [""]],
    [[""], [""]],
    [["a\tb", '"quote"', "line\r\nline"], ["", "日本語", "false"]],
  ])("round-trips clipboard cells including a final empty row: %j", (...rows) => {
    expect(parseTsv(encodeTsv(rows))).toEqual(rows);
  });
});

describe("Driver Tracker deterministic view ordering", () => {
  it("compares Number and Boolean values by their typed values", () => {
    const numeric = [field("ten", { kind: "number", value: 10 }), field("two", { kind: "number", value: 2 })];
    expect([...numeric].sort(compareFields).map(value => value.target.entity)).toEqual(["two", "ten"]);
    expect([...numeric].sort((a, b) => compareFields(a, b, true)).map(value => value.target.entity)).toEqual(["ten", "two"]);
    const booleans = [field("true", { kind: "boolean", value: true }), field("false", { kind: "boolean", value: false })];
    expect(booleans.sort(compareFields).map(value => value.target.entity)).toEqual(["false", "true"]);
  });

  it("uses deterministic text ordering without locale collation", () => {
    const texts = ["ä", "z", "A", "a"].map(value => field(value, { kind: "text", value }));
    expect(texts.sort(compareFields).map(value => value.target.entity)).toEqual(["A", "a", "z", "ä"]);
  });

  it("keeps valid values before missing and error values in either direction, with stable ties", () => {
    const fields = [
      field("error-a", { kind: "number", value: 0 }, true),
      field("missing-a", null),
      field("same-b", { kind: "number", value: 2 }),
      field("same-a", { kind: "number", value: 2 }),
      field("one", { kind: "number", value: 1 }),
      field("missing-b", null),
      field("error-b", null, true),
    ];
    const before = structuredClone(fields);
    expect([...fields].sort(compareFields).map(value => value.target.entity)).toEqual([
      "one", "same-b", "same-a", "missing-a", "missing-b", "error-a", "error-b",
    ]);
    expect([...fields].sort((a, b) => compareFields(a, b, true)).map(value => value.target.entity)).toEqual([
      "same-b", "same-a", "one", "missing-a", "missing-b", "error-a", "error-b",
    ]);
    expect(compareFields(undefined, fields[0])).toBeLessThan(0);
    expect(compareFields(undefined, undefined)).toBe(0);
    expect(fields).toEqual(before);
  });

  it("applies saved stable IDs and preserves unlisted row order without mutating the table or view", () => {
    const source = table(["a", "b", "c", "d"].map(id => field(id, null)));
    const view = { ...emptyTrackerView(), order: ["c", "unknown", "a"] };
    const sourceBefore = structuredClone(source), viewBefore = structuredClone(view);
    expect(orderedRows(source, view).map(row => row.id)).toEqual(["c", "a", "b", "d"]);
    expect(orderedRows(source, emptyTrackerView()).map(row => row.id)).toEqual(["a", "b", "c", "d"]);
    expect(source).toEqual(sourceBefore);
    expect(view).toEqual(viewBefore);
  });
});

describe("Driver Tracker view sidecar admission", () => {
  const budgetId = "00000000-0000-4000-8000-000000000001";
  const budget = () => ({ version: 1, active: budgetId, views: [{ id: budgetId, name: "Budget", collection: "schema-items" }] });

  it("validates Budget collection bindings against the opened snapshot rather than the sidecar", () => {
    const view = { ...emptyTrackerView(), budgetViews: budget() };
    const bytes = JSON.stringify(view);
    expect(parseTrackerView(bytes, ["schema-items", "schema-summary"]).budgetViews).toEqual(view.budgetViews);
    expect(() => parseTrackerView(bytes, ["schema-summary"])).toThrow("unavailable collection ID");
    expect(() => parseTrackerView(bytes, [])).toThrow("unavailable collection ID");
    expect(() => parseTrackerView(bytes)).toThrow("collection IDs");
    expect(JSON.stringify(view)).toBe(bytes);
  });

  it.each([null, { ...budget(), views: [null] }, { ...budget(), views: ["schema-items"] }, { ...budget(), views: [{ collection: "schema-items" }] }])("rejects malformed Budget metadata explicitly without a raw TypeError: %j", budgetViews => {
    const bytes = JSON.stringify({ ...emptyTrackerView(), budgetViews });
    let failure: unknown;
    try { parseTrackerView(bytes, ["schema-items"]); } catch (error) { failure = error; }
    expect(failure).toBeInstanceOf(Error);
    expect(failure).not.toBeInstanceOf(TypeError);
    expect((failure as Error).message).toContain("Invalid Budget views");
  });

  it("keeps legacy Tracker sidecars independent of Budget collection admission", () => {
    const bytes = JSON.stringify(emptyTrackerView());
    expect(parseTrackerView(bytes)).toEqual(emptyTrackerView());
    expect(parseTrackerView(bytes, [])).toEqual(emptyTrackerView());
    expect(parseTrackerView(undefined, ["schema-items"])).toEqual(emptyTrackerView());
  });

  it("round-trips supported formatting and opaque tuple keys with independent defaults", () => {
    const view = emptyTrackerView();
    const key = cellKey("entity.with.dot", "field");
    view.cells[key] = { bold: true, fill: false, wrap: true, border: true, align: "right" };
    view.order = ["entity.with.dot"];
    view.widths.field = 320;
    view.rowHeight = 80;
    view.header = false;
    view.formats[cellKey("entity.with.dot", "field")] = "currency-jpy";
    expect(parseTrackerView(JSON.stringify(view))).toEqual(view);
    const legacy = { ...view } as Record<string, unknown>;
    delete legacy.formats;
    expect(parseTrackerView(JSON.stringify(legacy)).formats).toEqual({});
    expect(parseTrackerView()).toEqual(emptyTrackerView());
    expect(cellKey("entity.with", "dot.field")).not.toBe(key);
    expect(emptyTrackerView().cells).toEqual({});
  });

  it.each([
    ["version", 2], ["order", [7]], ["cells", null], ["cells", []],
    ["widths", []], ["widths", { field: 121 }], ["rowHeight", 37], ["header", "true"],
    ["cells", { cell: [] }], ["cells", { cell: { bold: "true" } }],
    ["cells", { cell: { font: "custom" } }], ["cells", { cell: { align: ["left"] } }],
    ["cells", { cell: { align: "justify" } }],
    ["formats", { cell: "excel-custom" }],
  ])("rejects malformed %s: %j", (key, value) => {
    expect(() => parseTrackerView(JSON.stringify({ ...emptyTrackerView(), [key]: value }))).toThrow();
  });

  it.each(["{", "null", "[]", '"view"'])("rejects invalid top-level input %s", input => {
    expect(() => parseTrackerView(input)).toThrow();
  });
});
