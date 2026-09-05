import type { CleanupOperation, CleanupPreview, FidelityFinding, ImportedProjection, SourceWorkbook, SpreadsheetExport, SpreadsheetOperation } from "./interop-protocol.ts";

export type FieldTarget = {
  entity: string;
  field: string;
};

export type CollectionSummary = {
  id: string;
  key: string;
  entity_count: number;
};

export type BootstrapProjection = {
  title: string;
  revision: string;
  default_collection: string;
  collections: CollectionSummary[];
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

export type DiagnosticProjection = {
  code: string;
  message: string;
  path: string;
};

export type FieldProjection = {
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
  revision: string;
  collection: CollectionSummary;
  columns: Array<{ id: string; key: string; field_type: string; dropdown_options?: string[] }>;
  rows: Array<{ id: string; key: string; fields: FieldProjection[] }>;
};

export type FieldBatchProjection = {
  revision: string;
  fields: FieldProjection[];
};

export type PublicationProjection = {
  base_revision: string;
  resulting_revision: string;
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

export type FailureProjection = {
  code: string;
  message: string;
  current_revision: string;
  diagnostics: DiagnosticProjection[];
};

export type TrackerCommand =
  | { type: "paste_cells"; expected_revision: string; collection: string; start_entity: string | null; start_field: string; rows: string[][] }
  | { type: "append_row"; expected_revision: string; collection: string }
  | { type: "remove_rows"; expected_revision: string; entities: string[] }
  | { type: "undo" | "redo"; expected_revision: string };

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
  | { type: "bootstrap"; occurrence_id: string }
  | { type: "query_table"; collection: string }
  | {
      type: "query_fields";
      expected_revision: string;
      fields: FieldTarget[];
    }
  | {
      type: "edit_scalar";
      expected_revision: string;
      target: FieldTarget;
      input:
        | { kind: "number"; input: string }
        | { kind: "text"; value: string }
        | { kind: "boolean"; value: boolean }
        | { kind: "date"; value: string };
    }
  | {
      type: "formula_update";
      expected_revision: string;
      target: FieldTarget;
      source: string;
    };

export type DesignerResponse =
  | {type: "cleanup_preview"; payload: CleanupPreview}
  | {type: "import_preview"; payload: SourceWorkbook}
  | {type: "imported"; payload: ImportedProjection}
  | {type: "spreadsheet_exported"; payload: {revision: string; byte_length: number; ledger: FidelityFinding[]}}
  | { type: "bootstrap"; payload: BootstrapProjection }
  | { type: "opened"; payload: OpenedProjection }
  | { type: "table"; payload: TableProjection }
  | { type: "fields"; payload: FieldBatchProjection }
  | { type: "published"; payload: PublicationProjection }
  | { type: "project_exported"; payload: ProjectExportProjection };

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
  | { id: number; kind: "inspect_project"; bytes: ArrayBuffer }
  | { id: number; kind: "export_project"; expected_revision: string }
  | { id: number; kind: "close_project" };

export type WorkerReply =
  | {id: number; status: "spreadsheet_exported"; export: SpreadsheetExport}
  | { id: number; status: "ok"; response: DesignerResponse }
  | { id: number; status: "project_exported"; export: ProjectExport }
  | { id: number; status: "closed" }
  | { id: number; status: "error"; error: FailureProjection };

export const fieldTargetKey = (target: FieldTarget): string =>
  JSON.stringify([target.entity, target.field]);
