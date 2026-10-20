import type { CleanupOperation, CleanupPreview, FidelityFinding, ImportedProjection, SourceWorkbook, SpreadsheetExport, SpreadsheetOperation } from "./interop-protocol.ts";

export type FieldTarget = {
  entity: string;
  field: string;
};

export type CollectionSummary = {
  id: string;
  key: string;
  display_name: string;
  entity_count: number;
};

export type BootstrapProjection = {
  title: string;
  revision: string;
  default_collection: string;
  collections: CollectionSummary[];
  keyed_grouped_sum_definition_ids?: string[];
};

export type OpenedProjection = {
  bootstrap: BootstrapProjection;
  table: TableProjection;
};

export type StoredValueProjection =
  | { kind: "number"; value: number }
  | { kind: "text"; value: string }
  | { kind: "boolean"; value: boolean }
  | { kind: "date"; value: string }
  | { kind: "reference"; entity: string };

export type CalculationProjection =
  | { status: "value"; value: number }
  | { status: "failure"; code: string; message: string }
  | { status: "unavailable" };

export type SemanticSubjectProjection = { kind: string; value: string | Record<string, string> };

export type DiagnosticProjection = {
  code: string;
  message: string;
  path: string;
  subjects?: SemanticSubjectProjection[];
  related_subjects?: SemanticSubjectProjection[];
  facts?: Array<{name: string; value: string}>;
};

export type FieldProjection = {
  /** Absent stored value; omission means false. Empty Text is present. */
  blank?: boolean;
  target: FieldTarget;
  address: string;
  stored: StoredValueProjection | null;
  formula: { source: string } | null;
  calculated: CalculationProjection | null;
  diagnostics: DiagnosticProjection[];
  editable_scalar: "number" | "text" | "boolean" | "date" | null;
};

export type TableProjection = {
  tracker_profile?: boolean;
  native_table_profile?: boolean;
  revision: string;
  collection: CollectionSummary;
  columns: Array<{ id: string; key: string; display_name: string; field_type: string; required: boolean; dropdown_options?: string[] }>;
  /** fields contains present values only; blank_fields lists absent IDs in column order (omission means []). */
  rows: Array<{ id: string; key: string; blank_fields?: string[]; fields: FieldProjection[] }>;
};

export type FieldBatchProjection = {
  revision: string;
  fields: FieldProjection[];
};

export type FieldDependencyProjection = {
  target: FieldTarget;
  value: FieldProjection;
};

export type FieldExplanationProjection = {
  revision: string;
  target: FieldTarget;
  value: FieldProjection;
  formula_source: string | null;
  direct_dependencies: FieldDependencyProjection[];
  /** Direct calculated dependents only, in stable-ID order. */
  downstream_impacts: FieldTarget[];
  diagnostics: DiagnosticProjection[];
};

export type SchemaFieldTarget = { schema: string; field: string };

export type PublicationProjection = {
  base_revision: string;
  resulting_revision: string;
  /** Stable schema presentation targets, including tables with no rows. */
  schemas: string[];
  schema_fields: SchemaFieldTarget[];
  entities: string[];
  fields: FieldTarget[];
  affected_calculations: FieldTarget[];
};

export type ProjectExportProjection = {
  revision: string;
  byte_length: number;
};

export type ProjectExport = {
  revision: string;
  bytes: ArrayBuffer;
};

/** Opaque canonical path/byte entries from Rust; named/v3 occurrences use v3. */
export type CanonicalProjectFile = { path: string; bytes: ArrayBuffer };

export type CanonicalTreeExport = {
  revision: string;
  files: CanonicalProjectFile[];
};

export type OccurrenceProjection = {
  scope: string;
  revision: string;
};

export type KeyedGroupedSumDefinitionInput = {
  id: string;
  orders_schema: string;
  order_lookup_key_field: string;
  order_quantity_field: string;
  products_schema: string;
  product_key_field: string;
  product_category_field: string;
  product_price_field: string;
};

export type KeyedGroupedSumProjection = {
  definition_id: string;
  revision: string;
  groups: Array<{category: string; value: number}>;
  diagnostics: Array<{code: string; entity: string | null; field: string | null; lookup_key: string | null; candidates: string[]}>;
};

export type KeyedGroupedSumPublishedProjection = {
  publication: PublicationProjection;
  result: KeyedGroupedSumProjection;
};

export type DuplicateCollectionProjection = {
  publication: PublicationProjection;
  collection: CollectionSummary;
};

export type NameFailure = {
  reason: string;
  inputs: Array<{kind: "table"} | {kind: "column"; index: number; schema: string | null}>;
  subjects: SemanticSubjectProjection[];
  effective_name: string | null;
};

export type FailureProjection = {
  code: string;
  message: string;
  current_revision: string;
  diagnostics: DiagnosticProjection[];
  name_failures?: NameFailure[];
};

export type TrackerCommand =
  | { type: "edit_cells"; expected_revision: string; edits: Array<{ target: FieldTarget; input: ScalarEditInput }> }
  | { type: "paste_cells"; expected_revision: string; collection: string; start_entity: string | null; start_field: string; rows: PasteCellInput[][] }
  | { type: "append_row"; expected_revision: string; collection: string }
  | { type: "remove_rows"; expected_revision: string; entities: string[] }
  | { type: "insert_row"; expected_revision: string; collection: string; initializers: Array<{field: string; input: ScalarEditInput}> }
  | { type: "remove_table_rows"; expected_revision: string; collection: string; entities: string[] }
  | { type: "add_column"; expected_revision: string; collection: string; name: string; field_type: string; required?: boolean; initializers?: Array<{entity: string; input: ScalarEditInput}> }
  | { type: "set_column_requiredness"; expected_revision: string; collection: string; field: string; required: boolean }
  | { type: "rename_column"; expected_revision: string; collection: string; field: string; name: string }
  | { type: "remove_column"; expected_revision: string; collection: string; field: string }
  | { type: "undo" | "redo"; expected_revision: string };

export type ScalarEditInput =
  | { kind: "clear" }
  | { kind: "number"; input: string }
  | { kind: "text"; value: string }
  | { kind: "boolean"; value: boolean }
  | { kind: "date"; value: string };

/** Raw strings parse as the declared scalar type; only an explicit clear removes a value. */
export type PasteCellInput = string | ScalarEditInput;

/** required defaults to true; optional native authoring admits Text/Number only. */
export type NewTableColumnInput = {name: string; field_type: string; required?: boolean};

export type FormulaCopy = {
  source: FieldTarget;
  destinations: FieldTarget[];
  fixed_references: FieldTarget[];
  relative_rows: boolean;
  relative_columns: boolean;
};

export type DesignerRequest =
  | {type: "preview_cleanup"; expected_revision: string; operation: CleanupOperation}
  | {type: "commit_cleanup"; expected_revision: string; preview_id: string}
  | TrackerCommand
  | (FormulaCopy & { type: "copy_formula"; expected_revision: string })
  | { type: "new_tracker"; occurrence_id: string }
  | { type: "new_budget"; occurrence_id: string }
  | { type: "new_table"; occurrence_id: string; name: string; columns: NewTableColumnInput[] }
  | { type: "duplicate_collection"; expected_revision: string; collection: string; name: string }
  | { type: "bootstrap"; occurrence_id: string }
  | { type: "query_table"; collection: string }
  | {
      type: "query_fields";
      expected_revision: string;
      fields: FieldTarget[];
    }
  | {
      type: "explain_field";
      expected_revision: string;
      target: FieldTarget;
    }
  | {
      type: "edit_scalar";
      expected_revision: string;
      target: FieldTarget;
      input: ScalarEditInput;
    }
  | {
      type: "formula_update";
      expected_revision: string;
      target: FieldTarget;
      source: string;
    }
  | {type: "create_keyed_grouped_sum"; expected_revision: string; definition: KeyedGroupedSumDefinitionInput}
  | {type: "query_keyed_grouped_sum"; definition_id: string};

export type DesignerResponse =
  | {type: "cleanup_preview"; payload: CleanupPreview}
  | {type: "import_preview"; payload: SourceWorkbook}
  | {type: "imported"; payload: ImportedProjection}
  | {type: "spreadsheet_exported"; payload: {revision: string; byte_length: number; ledger: FidelityFinding[]}}
  | { type: "bootstrap"; payload: BootstrapProjection }
  | { type: "opened"; payload: OpenedProjection }
  | { type: "table"; payload: TableProjection }
  | { type: "fields"; payload: FieldBatchProjection }
  | { type: "field_explanation"; payload: FieldExplanationProjection }
  | { type: "published"; payload: PublicationProjection }
  | { type: "duplicated"; payload: DuplicateCollectionProjection }
  | { type: "project_exported"; payload: ProjectExportProjection }
  | { type: "canonical_tree_exported"; payload: ProjectExportProjection }
  | { type: "portable_ro_exported"; payload: ProjectExportProjection }
  | { type: "portable_ro_verified"; payload: { accepted: boolean } }
  | { type: "occurrence_observed"; payload: OccurrenceProjection }
  | { type: "keyed_grouped_sum"; payload: KeyedGroupedSumProjection }
  | { type: "keyed_grouped_sum_published"; payload: KeyedGroupedSumPublishedProjection };

export type DesignerWireReply =
  | { status: "ok"; response: DesignerResponse }
  | { status: "error"; error: FailureProjection };

export type WorkerRequest =
  | {id: number; kind: "spreadsheet"; operation: SpreadsheetOperation; bytes: ArrayBuffer}
  | { id: number; kind: "command"; request: DesignerRequest }
  | {
      id: number;
      kind: "open_project";
      occurrence_id: string;
      bytes: ArrayBuffer;
    }
  | {
      id: number;
      kind: "open_local_document";
      occurrence_id: string;
      bytes: ArrayBuffer;
    }
  | { id: number; kind: "inspect_project"; bytes: ArrayBuffer }
  | { id: number; kind: "export_project"; expected_revision: string }
  | { id: number; kind: "export_project_v3"; expected_revision: string }
  | { id: number; kind: "export_canonical_tree"; expected_revision: string }
  | { id: number; kind: "export_portable_ro"; expected_revision: string }
  | { id: number; kind: "verify_portable_ro"; bytes: ArrayBuffer }
  | { id: number; kind: "open_portable_ro"; occurrence_id: string; bytes: ArrayBuffer }
  | { id: number; kind: "observe_occurrence" }
  | { id: number; kind: "close_project" };

export type WorkerReply =
  | {id: number; status: "spreadsheet_exported"; export: SpreadsheetExport}
  | { id: number; status: "ok"; response: DesignerResponse }
  | { id: number; status: "project_exported"; export: ProjectExport }
  | { id: number; status: "canonical_tree_exported"; export: CanonicalTreeExport }
  | { id: number; status: "portable_ro_exported"; export: ProjectExport }
  | { id: number; status: "closed" }
  | { id: number; status: "error"; error: FailureProjection };

export const fieldTargetKey = (target: FieldTarget): string =>
  JSON.stringify([target.entity, target.field]);
