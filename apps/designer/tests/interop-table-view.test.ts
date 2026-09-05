import { describe, expect, it, vi } from "vitest";
import { emptyGenericTableView, mountInteropTableView, projectInteropTable, validateGenericTableView } from "../src/interop-table-view.ts";
import type { FieldProjection, StoredValueProjection, TableProjection } from "../src/runtime/protocol.ts";

function field(id: string, stored: StoredValueProjection | null): FieldProjection {
  return { target: { entity: id, field: "stable-value" }, address: `[${id}.value]`, stored, formula: null, calculated: null, diagnostics: [], editable_scalar: null };
}
function table(fields: FieldProjection[]): TableProjection {
  return { revision: "resident/9", collection: { id: "stable-schema", key: "Imported sheet", entity_count: fields.length }, columns: [{ id: "stable-value", key: "Display value", field_type: "number" }], rows: fields.map(value => ({ id: value.target.entity, key: value.target.entity, fields: [value] })) };
}
const sorted = (source: TableProjection, descending = false) => projectInteropTable(source, { ...emptyGenericTableView(), sortField: "stable-value", descending }).rows.map(row => row.id);
function formula(id: string, calculated: FieldProjection["calculated"]): FieldProjection {
  return { ...field(id, null), formula: { source: "DO NOT EVALUATE THIS SOURCE" }, calculated };
}

describe("imported table projection", () => {
  it("sorts Numbers and calculated formulas numerically with stable ties, missing values next and diagnostics last in both directions", () => {
    const bad = field("diagnostic", { kind: "number", value: -100 });
    bad.diagnostics = [{ code: "invalid", message: "Invalid source", path: "value" }];
    const source = table([
      field("ten", { kind: "number", value: 10 }),
      field("missing", null),
      formula("two-first", { status: "value", value: 2 }),
      bad,
      field("two-second", { kind: "number", value: 2 }),
      formula("failed", { status: "failure", code: "formula.division_by_zero", message: "Division by zero" }),
      formula("unavailable", { status: "unavailable" }),
    ]);
    const before = structuredClone(source);
    expect(sorted(source)).toEqual(["two-first", "two-second", "ten", "missing", "unavailable", "diagnostic", "failed"]);
    expect(sorted(source, true)).toEqual(["ten", "two-first", "two-second", "missing", "unavailable", "diagnostic", "failed"]);
    expect(source).toEqual(before);
    expect(projectInteropTable(source, emptyGenericTableView()).rows.map(row => row.id)).toEqual(source.rows.map(row => row.id));
    expect(projectInteropTable(source, emptyGenericTableView()).rows).not.toBe(source.rows);
  });

  it("sorts canonical Date, Boolean and Text values without locale-dependent collation", () => {
    expect(sorted(table([field("later", { kind: "date", value: "2026-01-01" }), field("earlier", { kind: "date", value: "2025-12-31" })]))).toEqual(["earlier", "later"]);
    expect(sorted(table([field("yes", { kind: "boolean", value: true }), field("no", { kind: "boolean", value: false })]))).toEqual(["no", "yes"]);
    const texts = table(["ä", "z", "A", "a"].map(text => field(text, { kind: "text", value: text })));
    expect(sorted(texts)).toEqual(["A", "a", "z", "ä"]);
    expect(sorted(texts, true)).toEqual(["ä", "z", "a", "A"]);
  });

  it("filters literal case-insensitive displayed data and authoritative results without evaluating regex or formulas", () => {
    const source = table([
      field("text", { kind: "text", value: "Alpha [.*]" }),
      formula("calculated", { status: "value", value: 200 }),
      formula("error", { status: "failure", code: "formula.division_by_zero", message: "Division by zero" }),
      field("date", { kind: "date", value: "2026-09-05" }),
      field("bool", { kind: "boolean", value: false }),
      field("missing", null),
    ]);
    const before = structuredClone(source);
    const matching = (filterText: string) => projectInteropTable(source, { ...emptyGenericTableView(), filterText }).rows.map(row => row.id);
    expect(matching("ALPHA")).toEqual(["text"]);
    expect(matching("[.*]")).toEqual(["text"]);
    expect(matching("200")).toEqual(["calculated"]);
    expect(matching("DIVISION BY ZERO")).toEqual(["error"]);
    expect(matching("2026-09")).toEqual(["date"]);
    expect(matching("FALSE")).toEqual(["bool"]);
    expect(matching("DO NOT EVALUATE")).toEqual([]);
    expect(matching("")).toHaveLength(6);
    expect(source).toEqual(before);
  });

  it("uses stable column IDs for filters and keeps row subjects and canonical metadata", () => {
    const source = table([field("first", { kind: "text", value: "first" }), field("second", { kind: "text", value: "second" })]);
    source.columns.push({ id: "stable-note", key: "Notes", field_type: "text" });
    source.rows[0]!.fields.push({ ...field("first", { kind: "text", value: "Needle" }), target: { entity: "first", field: "stable-note" } });
    const output = projectInteropTable(source, { ...emptyGenericTableView(), filterField: "stable-note", filterText: "needle" });
    expect(output.rows).toEqual([source.rows[0]]);
    expect(output.rows[0]!.fields[1]!.target).toEqual({ entity: "first", field: "stable-note" });
    expect(output.revision).toBe(source.revision);
    expect(output.collection).toEqual(source.collection);
    expect(projectInteropTable(source, { ...emptyGenericTableView(), filterField: "stable-value", filterText: "needle" }).rows).toEqual([]);
  });
});

describe("imported table view admission and controls", () => {
  it.each([null, [], {}, { ...emptyGenericTableView(), sortField: "unknown" }, { ...emptyGenericTableView(), filterField: "Display value" }, { ...emptyGenericTableView(), descending: "false" }, { ...emptyGenericTableView(), filterText: null }, { ...emptyGenericTableView(), filterText: "a".repeat(257) }, { ...emptyGenericTableView(), extra: true }])("rejects malformed private state %#", value => {
    expect(() => validateGenericTableView(value, ["stable-value"])).toThrow();
  });

  it("returns independent defaults and validates exact boundaries without normalizing the filter", () => {
    const source = { sortField: "stable-value", descending: true, filterField: null, filterText: " ".repeat(256) };
    const next = validateGenericTableView(source, ["stable-value"]);
    expect(next).toEqual(source);
    expect(next).not.toBe(source);
    expect(emptyGenericTableView()).not.toBe(emptyGenericTableView());
  });

  it("exposes human labels, selects stable IDs and returns changes without modifying the original state", () => {
    const root = document.createElement("div");
    const source = table([field("row", { kind: "number", value: 2 })]);
    const view = emptyGenericTableView();
    const changed = vi.fn();
    mountInteropTableView(root, source, view, false, changed);
    expect(root.textContent).toContain("Display value");
    expect(root.textContent).not.toContain("stable-value");
    const sort = root.querySelector<HTMLSelectElement>('[aria-label="Sort by"]')!;
    sort.value = "stable-value"; sort.dispatchEvent(new Event("change"));
    const descending = root.querySelector<HTMLInputElement>('[aria-label="Sort descending"]')!;
    descending.checked = true; descending.dispatchEvent(new Event("change"));
    const filter = root.querySelector<HTMLSelectElement>('[aria-label="Filter column"]')!;
    filter.value = "stable-value"; filter.dispatchEvent(new Event("change"));
    const text = root.querySelector<HTMLInputElement>('[aria-label="Filter text"]')!;
    text.value = "[.*]"; text.dispatchEvent(new InputEvent("input"));
    expect(changed).toHaveBeenLastCalledWith({ sortField: "stable-value", descending: true, filterField: "stable-value", filterText: "[.*]" });
    expect(view).toEqual(emptyGenericTableView());
    expect(text.maxLength).toBe(256);
  });

  it("blocks changes when the source projection is disabled", () => {
    const root = document.createElement("div");
    const changed = vi.fn();
    mountInteropTableView(root, table([]), emptyGenericTableView(), true, changed);
    expect(root.querySelector("fieldset")!.disabled).toBe(true);
    root.querySelector("select")!.dispatchEvent(new Event("change"));
    expect(changed).not.toHaveBeenCalled();
  });
});
