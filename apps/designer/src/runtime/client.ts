import type {
  BootstrapProjection,
  FieldBatchProjection,
  FieldTarget,
  OpenedProjection,
  ProjectExport,
  PublicationProjection,
  TableProjection,
  FailureProjection,
} from "./protocol.ts";

export interface DesignerClient {
  bootstrap(): Promise<BootstrapProjection>;
  openProject(bytes: ArrayBuffer): Promise<OpenedProjection>;
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
  editText(
    expectedRevision: string,
    target: FieldTarget,
    value: string,
  ): Promise<PublicationProjection>;
  editBoolean(
    expectedRevision: string,
    target: FieldTarget,
    value: boolean,
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
