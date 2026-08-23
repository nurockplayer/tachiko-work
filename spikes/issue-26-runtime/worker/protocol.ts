export type SemanticDocument = {
  id: string;
  title: string;
  schemas: Record<string, unknown>;
  entities: Record<
    string,
    {
      id: string;
      key: string;
      schema: string;
      fields: Record<string, unknown>;
    }
  >;
};

export type FieldAddress = {
  entity: string;
  field: string;
};

export type RuntimeCommand =
  | { type: "overview" }
  | { type: "calculate" }
  | { type: "set_scalar"; address: FieldAddress; input: string }
  | {
      type: "merge";
      base: SemanticDocument;
      theirs: SemanticDocument;
    };

export type CalculatedProjection = {
  field: { entity: string; field: string };
  address: string;
  value: number;
};

export type ProjectionValue =
  | { type: "number"; value: number }
  | { type: "text"; value: string }
  | { type: "boolean"; value: boolean }
  | { type: "reference"; entity: string }
  | { type: "formula"; expression: unknown };

export type ProjectionPatch = {
  field: { entity: string; field: string };
  value: ProjectionValue | null;
};

export type CommandResult =
  | {
      type: "overview";
      schema_count: number;
      entity_count: number;
      formula_count: number;
    }
  | { type: "calculation"; calculated: CalculatedProjection[] }
  | {
      type: "mutation";
      change_count: number;
      diff_text: string;
      patches: ProjectionPatch[];
    }
  | {
      type: "merge";
      merged: boolean;
      conflict_count: number;
      change_count: number;
      diff_text: string;
      patches: ProjectionPatch[];
    };

export type CommandResponse = {
  revision: number;
  result: CommandResult;
};

export type WireRequest =
  | { type: "generate_synthetic"; entity_count: number }
  | { type: "open_synthetic"; entity_count: number }
  | { type: "open"; document: SemanticDocument }
  | { type: "execute"; command: RuntimeCommand }
  | { type: "snapshot" }
  | {
      type: "execute_snapshot";
      document: SemanticDocument;
      command: RuntimeCommand;
    };

export type WireResult =
  | { type: "generated"; document: SemanticDocument }
  | { type: "opened"; revision: number }
  | { type: "command"; response: CommandResponse }
  | { type: "snapshot"; document: SemanticDocument }
  | {
      type: "snapshot_execution";
      document: SemanticDocument;
      response: CommandResponse;
    };

export type WireReply =
  | { ok: true; result: WireResult }
  | { ok: false; error: string };

export type WorkerRequest = {
  id: number;
  request: WireRequest;
};

export type WorkerReply =
  | { id: number; ok: true; result: WireResult }
  | { id: number; ok: false; error: string };
