import type {
  FidelityFinding,
  ImportedProjection,
  InteropMetadata,
  SourceStyle,
  SpreadsheetFormat,
} from "./runtime/interop-protocol.ts";
import {
  emptyGenericTableView,
  validateGenericTableView,
  type GenericTableView,
} from "./interop-table-view.ts";

/** Private host presentation/source state; canonical meaning stays in Rust. */
export type InteropState = {
  version: 1;
  metadata: InteropMetadata;
  ledger: FidelityFinding[];
  source: { name: string; format: SpreadsheetFormat; base64: string };
  tableViews: Record<string, GenericTableView>;
};

const MAX_SOURCE_BYTES = 2 * 1024 * 1024;
const MAX_SOURCE_BASE64 = Math.ceil(MAX_SOURCE_BYTES / 3) * 4;
const MAX_METADATA_BYTES = 65_536;
const MAX_LEDGER_BYTES = 65_536;
const MAX_LABEL_BYTES = 4096;
const encoder = new TextEncoder();
const categories = new Set<FidelityFinding["category"]>([
  "native_equivalent", "preserved_readable", "converted",
  "unsupported_safe_disabled", "lossy_on_export",
]);
const fail = (): never => { throw new Error("Unsupported saved spreadsheet state. Original project is unchanged."); };

function record(value: unknown, keys: readonly string[]): Record<string, unknown> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return fail();
  const prototype: unknown = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) return fail();
  if (Object.keys(value).length !== keys.length || Object.keys(value).some(key => !keys.includes(key))) return fail();
  return value as Record<string, unknown>;
}
function text(value: unknown, maximum = MAX_LABEL_BYTES, empty = false): string {
  if (typeof value !== "string" || (!empty && value.length === 0) || value.includes("\0") || encoder.encode(value).byteLength > maximum) return fail();
  // Reject unpaired UTF-16 surrogates instead of silently replacing source names.
  if (value.toWellFormed() !== value) return fail();
  return value;
}
function boolean(value: unknown): boolean { return typeof value === "boolean" ? value : fail(); }
function array(value: unknown, maximum: number, minimum = 0): unknown[] {
  if (!Array.isArray(value) || value.length < minimum || value.length > maximum) return fail();
  return value as unknown[];
}
function unique(value: string, seen: Set<string>): string {
  if (seen.has(value)) return fail();
  seen.add(value);
  return value;
}
function byteBound(value: unknown, maximum: number): void {
  if (encoder.encode(JSON.stringify(value)).byteLength > maximum) fail();
}
function optionalText(value: unknown): string | null { return value === null ? null : text(value, MAX_LABEL_BYTES, true); }
function style(value: unknown): SourceStyle {
  const input = record(value, ["number_format", "bold", "fill", "wrap", "border", "alignment"]);
  return {
    number_format: optionalText(input.number_format), bold: boolean(input.bold), fill: optionalText(input.fill),
    wrap: boolean(input.wrap), border: boolean(input.border), alignment: optionalText(input.alignment),
  };
}
function parseMetadata(value: unknown, collectionIds?: string[]): InteropMetadata {
  const input = record(value, ["version", "sheets"]);
  if (input.version !== 1) return fail();
  const schemas = new Set<string>(), fields = new Set<string>(), rows = new Set<string>(), names = new Set<string>();
  const sheets = array(input.sheets, 4, 1).map(value => {
    const sheet = record(value, ["schema_id", "name", "has_header", "columns", "rows"]);
    const schema_id = unique(text(sheet.schema_id), schemas);
    const name = text(sheet.name);
    // Match Rust's ASCII case-insensitive worksheet-name matching.
    unique(name.replace(/[A-Z]/g, ch => ch.toLowerCase()), names);
    const labels = new Set<string>();
    const columns = array(sheet.columns, 16, 1).map(value => {
      const column = record(value, ["field_id", "name", "width"]);
      const width = column.width;
      if (width !== null && (typeof width !== "number" || !Number.isFinite(width) || width <= 0 || width > 255)) return fail();
      return { field_id: unique(text(column.field_id), fields), name: unique(text(column.name), labels), width };
    });
    const mappedRows = array(sheet.rows, 64).map(value => {
      const row = record(value, ["entity_id", "styles"]);
      const styles = array(row.styles, columns.length, columns.length).map(style);
      return { entity_id: unique(text(row.entity_id), rows), styles };
    });
    return { schema_id, name, has_header: boolean(sheet.has_header), columns, rows: mappedRows };
  });
  if (collectionIds !== undefined) {
    if (collectionIds.length !== schemas.size || new Set(collectionIds).size !== collectionIds.length || collectionIds.some(id => !schemas.has(id))) fail();
  }
  const metadata = { version: 1, sheets };
  byteBound(metadata, MAX_METADATA_BYTES);
  return metadata;
}
function parseLedger(value: unknown): FidelityFinding[] {
  const ledger = array(value, 512).map(value => {
    const finding = record(value, ["category", "code", "location", "message", "blocking"]);
    const category = text(finding.category);
    if (!categories.has(category as FidelityFinding["category"])) return fail();
    return {
      category: category as FidelityFinding["category"], code: text(finding.code, 256),
      location: text(finding.location, MAX_LABEL_BYTES, true), message: text(finding.message), blocking: boolean(finding.blocking),
    };
  });
  byteBound(ledger, MAX_LEDGER_BYTES);
  return ledger;
}
function parseSource(value: unknown): InteropState["source"] {
  const source = record(value, ["name", "format", "base64"]);
  const name = text(source.name, 255);
  if (source.format !== "csv" && source.format !== "xlsx") return fail();
  if (typeof source.base64 !== "string" || source.base64.length === 0 || source.base64.length > MAX_SOURCE_BASE64 ||
      source.base64.length % 4 !== 0 || !/^[A-Za-z0-9+/]+={0,2}$/.test(source.base64)) return fail();
  const decoded = atob(source.base64);
  if (decoded.length === 0 || decoded.length > MAX_SOURCE_BYTES || btoa(decoded) !== source.base64) return fail();
  return { name, format: source.format, base64: source.base64 };
}

/** Validate host-only shapes; Rust separately verifies candidate semantic bindings. */
export function parseInteropState(input: unknown, collectionIds?: string[]): InteropState {
  const state = record(input, ["version", "metadata", "ledger", "source", "tableViews"]);
  if (state.version !== 1) return fail();
  const metadata = parseMetadata(state.metadata, collectionIds);
  const ledger = parseLedger(state.ledger);
  const source = parseSource(state.source);
  if (state.tableViews === null || typeof state.tableViews !== "object" || Array.isArray(state.tableViews)) return fail();
  const views = record(state.tableViews, Object.keys(state.tableViews));
  const known = new Map(metadata.sheets.map(sheet => [sheet.schema_id, sheet.columns.map(column => column.field_id)]));
  if (Object.keys(views).length > metadata.sheets.length) return fail();
  const tableViews = Object.fromEntries(Object.entries(views).map(([id, value]) => {
    const fields = known.get(id);
    if (fields === undefined) return fail();
    const view = record(value, ["sortField", "descending", "filterField", "filterText"]);
    text(view.filterText, 1024, true);
    return [id, validateGenericTableView(view, fields)];
  }));
  return { version: 1, metadata, ledger, source, tableViews };
}

/** Preserve exact original bytes in the same private host record as its evidence. */
export function createInteropState(
  imported: ImportedProjection,
  source: { name: string; format: SpreadsheetFormat; bytes: ArrayBuffer },
): InteropState {
  if (source.bytes.byteLength === 0 || source.bytes.byteLength > MAX_SOURCE_BYTES) return fail();
  const bytes = new Uint8Array(source.bytes);
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += 8192) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 8192));
  }
  return parseInteropState({
    version: 1, metadata: imported.metadata, ledger: imported.ledger,
    source: { name: source.name, format: source.format, base64: btoa(binary) },
    tableViews: Object.fromEntries(imported.metadata.sheets.map(sheet => [sheet.schema_id, emptyGenericTableView()])),
  }, imported.opened.bootstrap.collections.map(collection => collection.id));
}
