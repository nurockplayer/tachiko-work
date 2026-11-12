import type { CleanupOperation, CleanupPreview, FidelityFinding, ImportedProjection, ImportedWindowProjection, SourceWorkbook, SpreadsheetExport, SpreadsheetOperation } from "./interop-protocol.ts";

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

/** `projection_bytes` refuses an import and `saved_bytes` a whole-session save; neither is a counted resource. */
export type CapacityLimits = Record<
  Exclude<CapacityLimitName, "projection_bytes" | "saved_bytes" | "source_bytes" | "expanded_bytes" | "xml_nodes" | "serialized_source_bytes">,
  number
>;

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

/**
 * The reply a working-size open produces: the bootstrap plus one bounded
 * window, never the whole table. `window.total_rows` may exceed the rows the
 * window carries, so page with the window query.
 */
export type OpenedWindowProjection = {
  profile: "working_size";
  bootstrap: BootstrapProjection;
  window: TableWindowProjection;
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
  /** Sheet's display text for a Number or Date cell, present only when the window asked for it. */
  display?: string;
};

/** One durable field constraint; absence of the projection means unconstrained. */
export type FieldConstraint =
  | { kind: "text_literal_set"; values: string[] }
  | { kind: "number_inclusive_range"; min: number; max: number };

/** One complete constraint replacement; `none` clears the declaration. */
export type FieldConstraintInput = FieldConstraint | { kind: "none" };

export type DateFormat = "iso" | "mdy" | "dmy" | "ymd_slash" | "ymd_cjk" | "month_day_year" | "day_month_year";

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
  date_format?: DateFormat;
};

export type TableRowProjection = {
  id: string;
  key: string;
  /**
   * One-based stored row position, set only by a window. A host shows this
   * number and uses it for A1 geometry, never the displayed row index.
   */
  position?: number;
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

/**
 * One AND-ed view filter. Comparisons and sets match only same-kind values;
 * blanks match `is_blank`. The `display_*` filters compare the text the grid
 * shows, on every column kind including Reference, where a blank, failed,
 * unavailable or Reference cell has the text `""`.
 */
export type ViewFilter =
  | { kind: "text_equals"; field: string; value: string }
  | { kind: "text_contains"; field: string; value: string }
  | { kind: "number_compare"; field: string; op: ComparisonOperator; value: number }
  | { kind: "date_compare"; field: string; op: ComparisonOperator; value: string }
  | { kind: "is_blank"; field: string }
  | { kind: "not_blank"; field: string }
  | { kind: "in_set"; field: string; values: ScalarLiteral[] }
  /** Trimmed, case-insensitive containment; an empty value matches blank cells. */
  | { kind: "display_contains"; field: string; value: string }
  /** Exact display-text equality with no folding; `""` means blank. */
  | { kind: "display_in"; field: string; values: string[] };

/**
 * One sort key. `text_order` stays `bytes` (UTF-8) by default; `natural` orders
 * Text case-insensitively with digit runs compared as numbers, and has no
 * effect on other column kinds.
 */
export type ViewSortKey = {
  field: string;
  direction: SortDirection;
  text_order?: "bytes" | "natural";
};

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
   * Set when the reply's bytes ended this window before its count. Continue at
   * `start + rows.length`; `total_rows` says where the view ends.
   */
  byte_limited?: true;
  /**
   * One total per Number column, keyed by column id and observed at the same
   * revision as rows. Null means a contributing value is unavailable; blank
   * cells are skipped.
   */
  column_totals?: Record<string, number | null>;
};

/**
 * Zero-based position in the filtered, sorted view; `index` appears only when
 * found. `position` is the one-based stored row position and appears exactly
 * with `index`.
 */
export type LocatedInViewProjection = {
  revision: string;
  result: "found" | "filtered_out";
  index?: number;
  position?: number;
};

/**
 * The distinct display texts of one column inside a view, in Sheet's list
 * order: `""` first when any row has no display text, then natural order
 * (case-insensitive, digit runs compared as numbers). `truncated` is true when
 * more values exist than the reply carries or the byte budget admits.
 */
export type DistinctValuesProjection = {
  revision: string;
  field: string;
  values: string[];
  truncated: boolean;
};

/** One cell position inside a view: a zero-based view row and a column id. */
export type FindPosition = { row: number; field: string };

/** Which way Find walks from `FindPosition`, wrapping at either end. */
export type FindDirection = "next" | "previous";

/** One matching cell: its entity, column, zero-based view row, one-based stored row position and match ordinal. */
export type FoundCell = {
  entity: string;
  field: string;
  row: number;
  position: number;
  ordinal: number;
};

/**
 * One Find step over a view: `total` counts every matching cell, and `found`
 * is absent when nothing matches. A blank query always gives `total` 0.
 */
export type FindInViewProjection = {
  revision: string;
  total: number;
  found?: FoundCell;
};

/** Number totals over the named entities at one revision; blank cells are skipped, as in `column_totals`. */
export type ColumnTotalsProjection = {
  revision: string;
  rows: number;
  column_totals: Record<string, number | null>;
};

/** How a Date grouping dimension is bucketed; any non-Date column always groups by exact value. */
export type GroupBucket = "value" | "month" | "year";

/** One grouping column of a grouped-totals query; `bucket` defaults to `"value"`. */
export type GroupDimension = {
  field: string;
  bucket?: GroupBucket;
};

/** Which rows a grouped-totals query covers; every form resolves to collection order. */
export type GroupScope =
  | { kind: "all" }
  | { kind: "entities"; entities: string[] }
  | { kind: "except"; entities: string[] };

/** One typed grouping key; `blank` carries no value and sorts after every present value. */
export type GroupKey =
  | { kind: "boolean"; value: boolean }
  | { kind: "date"; value: string }
  | { kind: "month"; value: string }
  | { kind: "year"; value: string }
  | { kind: "text"; value: string }
  | { kind: "blank" };

/** One group of a grouped-totals reply. */
export type GroupTotalsRow = {
  keys: GroupKey[];
  rows: number;
  totals: (number | null)[];
};

/** Number totals grouped by one or two keys over the rows of one scope, at one revision. */
export type GroupTotalsProjection = {
  revision: string;
  dimensions: GroupDimension[];
  measures: string[];
  rows: number;
  totals: (number | null)[];
  groups: GroupTotalsRow[];
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
  /**
   * Set when this publication reached more targets than a reply lists.
   * `entities`, `fields` and `affected_calculations` are then empty: re-query
   * the visible window and totals.
   */
  many?: true;
  /**
   * Every changed cell, shaped as a window's field with its display text. Only
   * a working-size session sets it, and not when `many` is set.
   */
  changed?: FieldProjection[];
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
  /** Half-open UTF-8 byte offsets into the original A1 text. */
  span?: {start: number; end: number};
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
  | "dependency_edges"
  | "projection_bytes"
  | "source_bytes"
  | "expanded_bytes"
  | "xml_nodes"
  | "serialized_source_bytes"
  | "saved_bytes";

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
  "projection_bytes",
  "source_bytes",
  "expanded_bytes",
  "xml_nodes",
  "serialized_source_bytes",
  "saved_bytes",
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
  | "last_table"
  | "table_referenced"
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
  | { type: "change_column_type"; expected_revision: string; collection: string; field: string; field_type: string }
  | { type: "set_column_requiredness"; expected_revision: string; collection: string; field: string; required: boolean }
  | { type: "set_column_constraint"; expected_revision: string; collection: string; field: string; constraint: FieldConstraintInput }
  | { type: "set_column_number_format"; expected_revision: string; collection: string; field: string; number_format: NumberFormat | null }
  | { type: "set_column_date_format"; expected_revision: string; collection: string; field: string; date_format: DateFormat | null }
  | { type: "move_column"; expected_revision: string; collection: string; field: string; to_index: number }
  | { type: "rename_table"; expected_revision: string; collection: string; name: string }
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

/** Explicit, revision-pinned A1 geometry. Field and entity ids are stable identities. */
export type A1SheetGeometry = {collectionId: string; headerRow: number; rows: string[]; columns: string[]};
export type A1Geometry = Omit<A1SheetGeometry, "collectionId"> & {sheets?: Record<string, A1SheetGeometry>};
export type TranslateA1FormulaInput = {
  expectedRevision: string; collectionId: string; rowEntityId: string; targetFieldId: string;
  text: string; geometry: A1Geometry;
};
export type A1TranslationNote = "relative_copy_semantics_changed" | "range_tracks_collection";
export type A1RefusalCode = "unknown_sheet" | "out_of_bounds" | "header_reference" | "unsupported_range"
  | "unsupported_criterion" | "unsupported_function" | "unsupported_operator" | "formula_syntax"
  | "formula_type_mismatch" | "formula_cycle" | "invalid_geometry" | "stale_revision";
export type A1Translation = {revision: string; source: string; notes: A1TranslationNote[]};

export type DesignerRequest =
  | {type: "translate_a1_formula"; expected_revision: string; collection_id: string; row_entity_id: string; target_field_id: string; text: string; geometry: {header_row: number; rows: string[]; columns: string[]; sheets?: Record<string, {collection_id: string; header_row: number; rows: string[]; columns: string[]}>}}
  | {type: "preview_cleanup"; expected_revision: string; operation: CleanupOperation}
  | {type: "commit_cleanup"; expected_revision: string; preview_id: string}
  | TrackerCommand
  | (FormulaCopy & { type: "copy_formula"; expected_revision: string })
  | { type: "new_tracker"; occurrence_id: string; profile?: "working_size" }
  | { type: "new_budget"; occurrence_id: string; profile?: "working_size" }
  | { type: "new_table"; occurrence_id: string; name: string; columns: NewTableColumnInput[]; profile?: "working_size" }
  | { type: "duplicate_collection"; expected_revision: string; collection: string; name: string; column_names?: Record<string, string> }
  | { type: "delete_table"; expected_revision: string; collection: string }
  | { type: "add_table"; expected_revision: string; name: string; columns: NewTableColumnInput[] }
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
      /** Ask each Number or Date cell to carry its display text. */
      display?: boolean;
    }
  | {
      type: "locate_in_view";
      expected_revision: string;
      collection: string;
      view: TableView;
      entity: string;
    }
  | {
      type: "query_distinct_values";
      expected_revision: string;
      collection: string;
      view: TableView;
      field: string;
      limit: number;
    }
  | {
      type: "find_in_view";
      expected_revision: string;
      collection: string;
      view: TableView;
      query: string;
      fields: string[];
      from?: FindPosition;
      direction: FindDirection;
    }
  | {
      type: "query_column_totals";
      expected_revision: string;
      collection: string;
      entities: string[];
    }
  | {
      type: "query_view_totals";
      expected_revision: string;
      collection: string;
      view: TableView;
    }
  | {
      type: "query_group_totals";
      expected_revision: string;
      collection: string;
      dimensions: GroupDimension[];
      measures: string[];
      scope: GroupScope;
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
  | {type: "a1_translation"; payload: A1Translation}
  | {type: "cleanup_preview"; payload: CleanupPreview}
  | {type: "import_preview"; payload: SourceWorkbook}
  | {type: "imported"; payload: ImportedProjection}
  | {type: "imported_window"; payload: ImportedWindowProjection}
  | {type: "spreadsheet_exported"; payload: {revision: string; byte_length: number; ledger: FidelityFinding[]}}
  | { type: "bootstrap"; payload: BootstrapProjection }
  | { type: "capacity"; payload: CapacityProjection }
  | { type: "opened"; payload: OpenedProjection }
  | { type: "opened_window"; payload: OpenedWindowProjection }
  | { type: "table"; payload: TableProjection }
  | { type: "table_window"; payload: TableWindowProjection }
  | { type: "located_in_view"; payload: LocatedInViewProjection }
  | { type: "distinct_values"; payload: DistinctValuesProjection }
  | { type: "found_in_view"; payload: FindInViewProjection }
  | { type: "column_totals"; payload: ColumnTotalsProjection }
  | { type: "group_totals"; payload: GroupTotalsProjection }
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
      profile?: "working_size";
    }
  | {
      id: number;
      kind: "open_local_document";
      occurrence_id: string;
      bytes: ArrayBuffer;
      profile?: "working_size";
    }
  | { id: number; kind: "inspect_project"; bytes: ArrayBuffer; profile?: "working_size" }
  | { id: number; kind: "export_project"; expected_revision: string }
  | { id: number; kind: "export_project_v3"; expected_revision: string }
  | { id: number; kind: "export_canonical_tree"; expected_revision: string }
  | { id: number; kind: "export_portable_ro"; expected_revision: string }
  | { id: number; kind: "verify_portable_ro"; bytes: ArrayBuffer }
  | { id: number; kind: "open_portable_ro"; occurrence_id: string; bytes: ArrayBuffer; profile?: "working_size" }
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
