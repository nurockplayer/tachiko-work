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

export type CapacityLimits = Record<CapacityLimitName, number>;

export type CapacityUsage = {
  collections: number;
  total_entities: number;
  populated_cells: number;
  formulas: number;
  formula_ast_nodes: number;
  dependency_edges: number;
  tables: Record<string, {rows: number; fields: number}>;
};

export type CapacityProjection = {revision: string; usage: CapacityUsage; limits: CapacityLimits};

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

/** One durable field constraint; absence of the projection means unconstrained. */
export type FieldConstraint =
  | { kind: "text_literal_set"; values: string[] }
  | { kind: "number_inclusive_range"; min: number; max: number };

/** One complete constraint replacement; `none` clears the declaration. */
export type FieldConstraintInput = FieldConstraint | { kind: "none" };

/** Display-only number formatting stored on a Number field. */
export type NumberFormat = {
  decimals: number;
  kind: { type: "plain" } | { type: "currency"; code: string } | { type: "percent" };
  thousands_separator: boolean;
};

export type ConstraintFailureReason =
  | "field_value"
  | "existing_values"
  | "declaration";

/** Structured refusal: the allowed shape and how many rows or values failed. */
export type ConstraintFailureProjection = {
  reason: ConstraintFailureReason;
  schema: string;
  field: string;
  constraint: FieldConstraint;
  offending_rows: number;
};

export type ColumnProjection = {
  id: string;
  key: string;
  display_name: string;
  /** True when a stored name exists, even if equal to key. Omission means fallback. */
  display_name_explicit?: true;
  field_type: string;
  required: boolean;
  dropdown_options?: string[];
  /** Declared durable constraint; omission means unconstrained. */
  constraint?: FieldConstraint;
  /** Declared display-only number format; omission means default formatting. */
  number_format?: NumberFormat;
};

export type TableRowProjection = {
  id: string;
  key: string;
  /** fields contains present values only; blank_fields lists absent IDs in column order (omission means []). */
  blank_fields?: string[];
  fields: FieldProjection[];
};

export type TableProjection = {
  tracker_profile?: boolean;
  native_table_profile?: boolean;
  revision: string;
  collection: CollectionSummary;
  columns: ColumnProjection[];
  rows: TableRowProjection[];
  /**
   * One total per Number column, keyed by column id and observed at the same
   * revision as rows. Null means a contributing value is unavailable; blank
   * cells are skipped.
   */
  column_totals?: Record<string, number | null>;
};

export type SortDirection = "asc" | "desc";

/** Every comparison takes the column's declared kind; text compares case-insensitively. */
export type ComparisonOperator = "=" | "<>" | "<" | "<=" | ">" | ">=";

/** One literal member of an `in_set` filter; it must fit the filtered column's type. */
export type ScalarLiteral = number | string | boolean;

/** One AND-ed view filter. Comparisons and sets match only same-kind values; blanks match `is_blank`. */
export type ViewFilter =
  | { kind: "text_equals"; field: string; value: string }
  | { kind: "text_contains"; field: string; value: string }
  | { kind: "number_compare"; field: string; op: ComparisonOperator; value: number }
  | { kind: "date_compare"; field: string; op: ComparisonOperator; value: string }
  | { kind: "is_blank"; field: string }
  | { kind: "not_blank"; field: string }
  | { kind: "in_set"; field: string; values: ScalarLiteral[] };

export type ViewSortKey = { field: string; direction: SortDirection };

/**
 * Stateless view state owned by the caller, never by the document: sort keys
 * apply in order with blanks last, filters are AND-ed, and `keep_entities`
 * (at most 256) stay visible even when a filter excludes them.
 */
export type TableView = {
  sort?: ViewSortKey[];
  filters?: ViewFilter[];
  keep_entities?: string[];
};

/** One bounded window of a table; `count` 0 returns totals and columns only. */
export type TableWindowProjection = {
  /** Same profile flags as `TableProjection`; present only when true. */
  tracker_profile?: boolean;
  native_table_profile?: boolean;
  revision: string;
  collection: CollectionSummary;
  columns: ColumnProjection[];
  /** Deterministic digest of the column definitions; stable across row edits. */
  columns_revision: string;
  /** Rows left after filters, plus kept entities. */
  total_rows: number;
  unfiltered_rows: number;
  start: number;
  rows: TableRowProjection[];
  /**
   * One total per Number column, keyed by column id and observed at the same
   * revision as rows. Null means a contributing value is unavailable; blank
   * cells are skipped.
   */
  column_totals?: Record<string, number | null>;
};

/** Zero-based position in the filtered, sorted view; `index` appears only when found. */
export type LocatedInViewProjection = {
  revision: string;
  result: "found" | "filtered_out";
  index?: number;
};

/** Number totals over the named entities at one revision; blank cells are skipped, as in `column_totals`. */
export type ColumnTotalsProjection = {
  revision: string;
  rows: number;
  column_totals: Record<string, number | null>;
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
  /** Sum of the groups at this revision; null whenever the result is unavailable. */
  total: number | null;
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
  code: FailureCode;
  message: string;
  current_revision: string;
  diagnostics: DiagnosticProjection[];
  /** Present only for the `capacity_exceeded` refusal. */
  limit?: CapacityLimitName;
  maximum?: number;
  attempted?: number;
  collection?: string;
  name_failures?: NameFailure[];
  /** Present only when a field constraint refused the request. */
  constraint_failure?: ConstraintFailureProjection;
  /** Present only for the `use_window_query` refusal: the collection's row count. */
  row_count?: number;
};

/** Runtime accounting limit named by a capacity refusal. */
export type CapacityLimitName =
  | "collections"
  | "table_fields"
  | "table_rows"
  | "total_entities"
  | "populated_cells"
  | "formulas"
  | "formula_ast_nodes"
  | "dependency_edges";

export type CapacityFailureProjection = FailureProjection & {
  code: "capacity_exceeded";
  limit: CapacityLimitName;
  maximum: number;
  attempted: number;
} & (
  | { limit: "table_fields" | "table_rows"; collection: string }
  | { limit: Exclude<CapacityLimitName, "table_fields" | "table_rows">; collection?: never }
);

const CAPACITY_LIMIT_NAMES: readonly CapacityLimitName[] = [
  "collections",
  "table_fields",
  "table_rows",
  "total_entities",
  "populated_cells",
  "formulas",
  "formula_ast_nodes",
  "dependency_edges",
];

/** Validate the detailed wire payload for a capacity refusal. */
export function isCapacityFailure(failure: FailureProjection): failure is CapacityFailureProjection {
  if (failure.code !== "capacity_exceeded" ||
      !CAPACITY_LIMIT_NAMES.includes(failure.limit as CapacityLimitName) ||
      !Number.isSafeInteger(failure.maximum) || failure.maximum! < 0 ||
      !Number.isSafeInteger(failure.attempted) || failure.attempted! <= failure.maximum!) {
    return false;
  }
  if (failure.limit === "table_fields" || failure.limit === "table_rows") {
    return typeof failure.collection === "string" && failure.collection.length > 0;
  }
  return failure.collection === undefined;
}

/** Known runtime failure codes, while allowing forward-compatible additions. */
export type FailureCode =
  | "required_values_missing"
  | "capacity_exceeded"
  | (string & {});

/**
 * Stable refusal codes for `updateFormula`. Each failure carries a human
 * message in `FailureProjection.message`. Syntax and reference failures add
 * one diagnostic for the refused formula with a `position` or `reference`
 * fact; a cycle failure carries the authoritative validation diagnostics,
 * whose location is the cycle's canonical member.
 */
export type FormulaFailureCode =
  | "formula_syntax"
  | "formula_unknown_reference"
  | "formula_type_mismatch"
  | "formula_cycle";

const FORMULA_FAILURE_CODES: readonly string[] = [
  "formula_syntax",
  "formula_unknown_reference",
  "formula_type_mismatch",
  "formula_cycle",
];

export type FormulaFailureProjection = FailureProjection & { code: FormulaFailureCode };

export function isFormulaFailure(failure: FailureProjection): failure is FormulaFailureProjection {
  return FORMULA_FAILURE_CODES.includes(failure.code);
}

export type TrackerCommand =
  | { type: "edit_cells"; expected_revision: string; edits: Array<{ target: FieldTarget; input: ScalarEditInput }> }
  | { type: "paste_cells"; expected_revision: string; collection: string; start_entity: string | null; start_field: string; rows: PasteCellInput[][] }
  | { type: "append_row"; expected_revision: string; collection: string }
  | { type: "remove_rows"; expected_revision: string; entities: string[] }
  | { type: "insert_row"; expected_revision: string; collection: string; initializers: Array<{field: string; input: ScalarEditInput}> }
  | { type: "remove_table_rows"; expected_revision: string; collection: string; entities: string[] }
  | { type: "add_column"; expected_revision: string; collection: string; name: string; field_type: string; required?: boolean; initializers?: Array<{entity: string; input: ScalarEditInput}> }
  | { type: "set_column_requiredness"; expected_revision: string; collection: string; field: string; required: boolean }
  | { type: "set_column_constraint"; expected_revision: string; collection: string; field: string; constraint: FieldConstraintInput }
  | { type: "set_column_number_format"; expected_revision: string; collection: string; field: string; number_format: NumberFormat | null }
  | { type: "move_column"; expected_revision: string; collection: string; field: string; to_index: number }
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

/** required defaults to true; optional native authoring admits every scalar type (#598). */
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
  | { type: "duplicate_collection"; expected_revision: string; collection: string; name: string; column_names?: Record<string, string> }
  | { type: "bootstrap"; occurrence_id: string }
  | { type: "query_capacity"; expected_revision?: string }
  | { type: "query_table"; collection: string }
  | {
      type: "query_table_window";
      expected_revision: string;
      collection: string;
      view: TableView;
      start: number;
      count: number;
    }
  | {
      type: "locate_in_view";
      expected_revision: string;
      collection: string;
      view: TableView;
      entity: string;
    }
  | {
      type: "query_column_totals";
      expected_revision: string;
      collection: string;
      entities: string[];
    }
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
  | { type: "capacity"; payload: CapacityProjection }
  | { type: "opened"; payload: OpenedProjection }
  | { type: "table"; payload: TableProjection }
  | { type: "table_window"; payload: TableWindowProjection }
  | { type: "located_in_view"; payload: LocatedInViewProjection }
  | { type: "column_totals"; payload: ColumnTotalsProjection }
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
