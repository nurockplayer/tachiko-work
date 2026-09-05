import { describe, expect, it } from "vitest";
import { createInteropState, parseInteropState, type InteropState } from "../src/interop-state.ts";
import type { ImportedProjection, SourceStyle } from "../src/runtime/interop-protocol.ts";

const style = (): SourceStyle => ({ number_format: null, bold: false, fill: null, wrap: false, border: false, alignment: null });
function fixture(): InteropState {
  return {
    version: 1,
    metadata: { version: 1, sheets: [
      { schema_id: "schema_a", name: "資料", has_header: true, columns: [{ field_id: "field_a", name: "Amount", width: 12 }], rows: [{ entity_id: "row_a", styles: [style()] }] },
      { schema_id: "schema_b", name: "Summary", has_header: false, columns: [{ field_id: "field_b", name: "Total", width: null }], rows: [] },
    ] },
    ledger: [{ category: "converted", code: "bound_reference", location: "資料!A2", message: "Bound to stable identities.", blocking: false }],
    source: { name: "原始.csv", format: "csv", base64: "YQ==" },
    tableViews: { schema_a: { sortField: "field_a", descending: true, filterField: null, filterText: "" } },
  };
}
const collections = ["schema_a", "schema_b"];
const firstSheet = (state: InteropState) => {
  const sheet = state.metadata.sheets[0];
  if (!sheet) throw new Error("Fixture has no sheet");
  return sheet;
};

describe("private spreadsheet source state", () => {
  it("round-trips bounded state and returns detached validated structures", () => {
    const input = fixture();
    const parsed = parseInteropState(JSON.parse(JSON.stringify(input)), collections);
    expect(parsed).toEqual(input);
    const direct = parseInteropState(input, collections);
    firstSheet(direct).columns[0]!.name = "Changed";
    expect(firstSheet(input).columns[0]?.name).toBe("Amount");
  });

  it("validates against exact admitted schema IDs, independent of array order", () => {
    expect(parseInteropState(fixture(), [...collections].reverse())).toEqual(fixture());
    for (const ids of [[], ["schema_a"], ["schema_a", "foreign"], ["schema_a", "schema_a"], [...collections, "extra"]]) {
      expect(() => parseInteropState(fixture(), ids)).toThrow();
    }
  });

  it("rejects unknown fields, foreign view subjects and malformed nested shapes", () => {
    const mutations: Array<(value: InteropState) => void> = [
      value => { Object.assign(value, { extra: true }); },
      value => { Object.assign(value.metadata, { extra: true }); },
      value => { Object.assign(firstSheet(value), { extra: true }); },
      value => { Object.assign(firstSheet(value).columns[0]!, { extra: true }); },
      value => { Object.assign(firstSheet(value).rows[0]!, { extra: true }); },
      value => { Object.assign(firstSheet(value).rows[0]!.styles[0]!, { script: "ignored?" }); },
      value => { Object.assign(value.ledger[0]!, { safe: true }); },
      value => { Object.assign(value.source, { revision: "untrusted" }); },
      value => { value.tableViews.foreign = { sortField: null, descending: false, filterField: null, filterText: "" }; },
      value => { value.tableViews.schema_a!.sortField = "field_b"; },
      value => { Object.assign(value.tableViews.schema_a!, { hidden: true }); },
      value => { Object.assign(value.metadata, { sheets: {} }); },
      value => { Object.assign(value.ledger[0]!, { blocking: "false" }); },
      value => { Object.assign(value.ledger[0]!, { category: "safe" }); },
      value => { Object.assign(firstSheet(value), { has_header: 1 }); },
      value => { Object.assign(firstSheet(value).rows[0]!.styles[0]!, { bold: 1 }); },
    ];
    for (const mutate of mutations) {
      const input = fixture();
      mutate(input);
      expect(() => parseInteropState(input, collections)).toThrow();
    }
  });

  it("rejects duplicate identities, labels and out-of-profile dimensions", () => {
    const mutations: Array<(value: InteropState) => void> = [
      value => { value.metadata.sheets[1]!.schema_id = "schema_a"; },
      value => { value.metadata.sheets[1]!.name = "資料"; },
      value => { value.metadata.sheets[1]!.columns[0]!.field_id = "field_a"; },
      value => { value.metadata.sheets[1]!.rows = [{ entity_id: "row_a", styles: [style()] }]; },
      value => { firstSheet(value).columns.push({ field_id: "another", name: "Amount", width: null }); },
      value => { firstSheet(value).rows[0]!.styles = []; },
      value => { firstSheet(value).columns[0]!.width = Infinity; },
      value => { firstSheet(value).columns[0]!.width = 0; },
      value => { firstSheet(value).columns[0]!.width = 256; },
      value => { firstSheet(value).rows = Array.from({ length: 65 }, (_, i) => ({ entity_id: `row_${String(i)}`, styles: [style()] })); },
      value => { firstSheet(value).columns = Array.from({ length: 17 }, (_, i) => ({ field_id: `field_${String(i)}`, name: `Column ${String(i)}`, width: null })); },
      value => { firstSheet(value).name = ""; },
      value => { firstSheet(value).name = "\ud800"; },
      value => { firstSheet(value).name = "x\0y"; },
      value => { firstSheet(value).name = "x".repeat(4097); },
      value => { value.tableViews.schema_a!.filterText = "x".repeat(257); },
    ];
    for (const mutate of mutations) {
      const input = fixture();
      mutate(input);
      expect(() => parseInteropState(input)).toThrow();
    }
  });

  it("rejects inherited dictionaries and preserves opaque prototype-like IDs safely", () => {
    const inherited = fixture();
    inherited.tableViews = Object.create({ schema_a: inherited.tableViews.schema_a }) as InteropState["tableViews"];
    expect(() => parseInteropState(inherited)).toThrow();
    const opaque = fixture();
    firstSheet(opaque).schema_id = "__proto__";
    opaque.tableViews = Object.fromEntries([["__proto__", { sortField: "field_a", descending: false, filterField: null, filterText: "" }]]);
    const parsed = parseInteropState(opaque, ["__proto__", "schema_b"]);
    expect(Object.hasOwn(parsed.tableViews, "__proto__")).toBe(true);
    expect(Object.getPrototypeOf(parsed.tableViews)).toBe(Object.prototype);
  });

  it("requires canonical base64 and validates decoded source and UTF-8 name limits", () => {
    for (const base64 of ["", "YR==", "YQ", "YQ===", "YQ==\n", "====", "YQ--", "YQ==YQ=="]) {
      const input = fixture(); input.source.base64 = base64;
      expect(() => parseInteropState(input)).toThrow();
    }
    const maximum = fixture();
    maximum.source.base64 = btoa("x".repeat(2 * 1024 * 1024));
    expect(parseInteropState(maximum).source.base64).toBe(maximum.source.base64);
    maximum.source.base64 = btoa("x".repeat(2 * 1024 * 1024 + 1));
    expect(() => parseInteropState(maximum)).toThrow();
    const longName = fixture(); longName.source.name = "📄".repeat(64);
    expect(() => parseInteropState(longName)).toThrow();
  });

  it("bounds aggregate metadata and fidelity evidence without removing blocking facts", () => {
    const blocked = fixture(); blocked.ledger[0]!.blocking = true;
    expect(parseInteropState(blocked).ledger[0]?.blocking).toBe(true);
    const ledger = fixture(); ledger.ledger = Array.from({ length: 513 }, () => ({ ...ledger.ledger[0]! }));
    expect(() => parseInteropState(ledger)).toThrow();
    const messages = fixture(); messages.ledger = Array.from({ length: 20 }, () => ({ ...messages.ledger[0]!, message: "x".repeat(4096) }));
    expect(() => parseInteropState(messages)).toThrow();
    const metadata = fixture(); firstSheet(metadata).rows = Array.from({ length: 20 }, (_, i) => ({ entity_id: `row_${String(i)}`, styles: [{ ...style(), number_format: "x".repeat(4096) }] }));
    expect(() => parseInteropState(metadata)).toThrow();
  });

  it("constructs source preservation without detaching bytes and validates the imported catalog", () => {
    const state = fixture();
    const imported: ImportedProjection = {
      opened: { bootstrap: { title: "Imported", revision: "resident/0", default_collection: "sheet_1", collections: state.metadata.sheets.map(sheet => ({ id: sheet.schema_id, key: sheet.name, entity_count: sheet.rows.length })) },
        table: { revision: "resident/0", collection: { id: "schema_a", key: "sheet_1", entity_count: 0 }, columns: [], rows: [] } },
      metadata: state.metadata, ledger: state.ledger,
    };
    const bytes = new Uint8Array([0, 1, 2, 127, 128, 255]).buffer;
    const created = createInteropState(imported, { name: "original.xlsx", format: "xlsx", bytes });
    expect(atob(created.source.base64)).toBe(String.fromCharCode(0, 1, 2, 127, 128, 255));
    expect(bytes.byteLength).toBe(6);
    expect(Object.keys(created.tableViews)).toEqual(collections);
    imported.opened.bootstrap.collections.pop();
    expect(() => createInteropState(imported, { name: "original.xlsx", format: "xlsx", bytes })).toThrow();
  });
});
