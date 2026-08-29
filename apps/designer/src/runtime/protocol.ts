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
  control_field: FieldTarget;
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
  editable_number: boolean;
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

export type FailureProjection = {
  code: string;
  message: string;
  current_revision: string;
  diagnostics: DiagnosticProjection[];
};

export type DesignerRequest =
  | { type: "bootstrap" }
  | { type: "query_table"; collection: string }
  | {
      type: "query_fields";
      expected_revision: string;
      fields: FieldTarget[];
    }
  | {
      type: "edit_number";
      expected_revision: string;
      target: FieldTarget;
      input: string;
    };

export type DesignerResponse =
  | { type: "bootstrap"; payload: BootstrapProjection }
  | { type: "table"; payload: TableProjection }
  | { type: "fields"; payload: FieldBatchProjection }
  | { type: "published"; payload: PublicationProjection };

export type DesignerWireReply =
  | { status: "ok"; response: DesignerResponse }
  | { status: "error"; error: FailureProjection };

export type WorkerRequest = {
  id: number;
  request: DesignerRequest;
};

export type WorkerReply =
  | { id: number; status: "ok"; response: DesignerResponse }
  | { id: number; status: "error"; error: FailureProjection };

export const fieldTargetKey = (target: FieldTarget): string =>
  `${target.entity}.${target.field}`;
