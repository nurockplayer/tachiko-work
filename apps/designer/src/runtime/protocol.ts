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
  editable_scalar: "number" | "text" | "boolean" | null;
};

export type TableProjection = {
  revision: string;
  collection: CollectionSummary;
  columns: Array<{ id: string; key: string; field_type: string }>;
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

export type DesignerRequest =
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
        | { kind: "boolean"; value: boolean };
    };

export type DesignerResponse =
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
  | { id: number; kind: "command"; request: DesignerRequest }
  | {
      id: number;
      kind: "open_project";
      occurrence_id: string;
      bytes: ArrayBuffer;
    }
  | { id: number; kind: "export_project"; expected_revision: string }
  | { id: number; kind: "close_project" };

export type WorkerReply =
  | { id: number; status: "ok"; response: DesignerResponse }
  | { id: number; status: "project_exported"; export: ProjectExport }
  | { id: number; status: "closed" }
  | { id: number; status: "error"; error: FailureProjection };

export const fieldTargetKey = (target: FieldTarget): string =>
  JSON.stringify([target.entity, target.field]);
