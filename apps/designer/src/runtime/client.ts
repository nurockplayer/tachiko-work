import type {
  BootstrapProjection,
  FieldBatchProjection,
  FieldTarget,
  ProjectExport,
  PublicationProjection,
  TableProjection,
  FailureProjection,
} from "./protocol.ts";

export interface DesignerClient {
  bootstrap(): Promise<BootstrapProjection>;
  openProject(bytes: ArrayBuffer): Promise<BootstrapProjection>;
  exportProject(expectedRevision: string): Promise<ProjectExport>;
  closeProject(): Promise<void>;
  queryTable(collection: string): Promise<TableProjection>;
  queryFields(
    expectedRevision: string,
    fields: FieldTarget[],
  ): Promise<FieldBatchProjection>;
  editNumber(
    expectedRevision: string,
    target: FieldTarget,
    input: string,
  ): Promise<PublicationProjection>;
  close(): void | Promise<void>;
}

export class DesignerRuntimeError extends Error {
  readonly failure: FailureProjection;

  constructor(failure: FailureProjection) {
    super(failure.message);
    this.name = "DesignerRuntimeError";
    this.failure = failure;
  }
}
