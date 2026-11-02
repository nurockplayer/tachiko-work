import type { FieldProjection, FieldTarget, OpenedProjection, StoredValueProjection } from "./protocol.ts";

export type SpreadsheetFormat = "csv" | "xlsx";
export type ImportFieldType = "text" | "number" | "boolean" | "date";
/** `header_rows` (1-based header row per sheet; missing or null = detect) and `keep_summary_rows` are XLSX only; CSV refuses them. */
export type ImportOptions = { delimiter: string; header: boolean; header_rows?: (number | null)[]; keep_summary_rows?: boolean };
/** Where the imported grid sat in the source sheet; omitted when the header is row 1 and no Total row was recognised. */
export type SourceLayout = { rows_above_header?: number; summary_row?: { row: number; cells: Array<{ column: number; aggregate: "total" | "average" | "count"; formula: string }> } };
export type FidelityFinding = {
  category: "native_equivalent" | "preserved_readable" | "converted" | "unsupported_safe_disabled" | "lossy_on_export";
  code: string; location: string; message: string; blocking: boolean;
};
export type SourceValue = {kind: "empty"} | {kind: "text" | "date"; value: string} | {kind: "number"; value: number} | {kind: "boolean"; value: boolean};
export type SourceStyle = { number_format: string | null; bold: boolean; fill: string | null; wrap: boolean; border: boolean; alignment: string | null };
export type SourceCell = { value: SourceValue; formula: string | null; style: SourceStyle };
export type SourceWorkbook = { sheets: Array<{name: string; has_header: boolean; columns: Array<{name: string; width: number | null}>; rows: SourceCell[][]; layout?: SourceLayout}>; ledger: FidelityFinding[] };
export type ImportSelection = { column_types: ImportFieldType[][]; extra_columns: Array<Array<{name: string; field_type: ImportFieldType}>> };
export type InteropMetadata = { version: number; sheets: Array<{schema_id: string; name: string; has_header: boolean; columns: Array<{field_id: string; name: string; width: number | null}>; rows: Array<{entity_id: string; styles: SourceStyle[]}>}> };
/** Private outbound-only binding for the stock 128-row Driver Tracker. */
export type NativeTrackerExportPresentation = { version: number; rows: Array<{entity_id: string; styles: SourceStyle[]}> };
/** Private outbound-only binding for the native Budget collections and views. */
export type NativeBudgetExportPresentation = {
  version: number;
  active_view: string;
  views: Array<{id: string; name: string; collection_id: string}>;
  collections: Array<{collection_id: string; rows: Array<{entity_id: string; styles: SourceStyle[]}>}>;
};
export type ImportedProjection = { opened: OpenedProjection; metadata: InteropMetadata; ledger: FidelityFinding[] };
export type SpreadsheetExport = { revision: string; bytes: ArrayBuffer; ledger: FidelityFinding[] };
export type SpreadsheetOperation =
  | {type: "inspect"; format: SpreadsheetFormat; csv_options: ImportOptions}
  | {type: "import"; format: SpreadsheetFormat; csv_options: ImportOptions; selection: ImportSelection; occurrence_id: string; install: boolean}
  | {type: "inspect_project"; metadata: InteropMetadata}
  | {type: "open_project"; occurrence_id: string; metadata: InteropMetadata}
  | {type: "export"; expected_revision: string; metadata: InteropMetadata; format: SpreadsheetFormat; collection: string}
  | {type: "export_native_tables"; expected_revision: string; collection: string; format: SpreadsheetFormat}
  | {type: "export_native_tracker"; expected_revision: string; presentation: NativeTrackerExportPresentation; format: SpreadsheetFormat}
  | {type: "export_native_budget"; expected_revision: string; presentation: NativeBudgetExportPresentation; format: SpreadsheetFormat};
export type ScalarInput = {kind: "number"; input: string} | {kind: "text" | "date"; value: string} | {kind: "boolean"; value: boolean};
export type CleanupOperation =
  | {kind: "trim"; fields: FieldTarget[]}
  | {kind: "replace"; fields: FieldTarget[]; find: string; replacement: string}
  | {kind: "split"; source: FieldTarget; destinations: FieldTarget[]; separator: string}
  | {kind: "convert"; source: FieldTarget; destination: FieldTarget}
  | {kind: "fill"; fields: FieldTarget[]; input: ScalarInput}
  | {kind: "deduplicate"; entities: string[]; key_fields: string[]};
export type CleanupPreview = {preview_id: string; revision: string; changes: Array<{target: FieldTarget; before: FieldProjection | null; after: StoredValueProjection | null}>; removed_entities: string[]};
