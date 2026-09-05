import type {
  FormulaCopy,
  TrackerCommand,
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
  copyFormula?(expectedRevision: string, request: FormulaCopy): Promise<PublicationProjection>;
  newTracker?(): Promise<OpenedProjection>;
  newBudget?(): Promise<OpenedProjection>;
  trackerCommand?(request: TrackerCommand): Promise<PublicationProjection>;
  bootstrap(): Promise<BootstrapProjection>;
  inspectProject?(bytes: ArrayBuffer): Promise<OpenedProjection>;
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
  editDate(
    expectedRevision: string,
    target: FieldTarget,
    value: string,
  ): Promise<PublicationProjection>;
  updateFormula?(
    expectedRevision: string,
    target: FieldTarget,
    source: string,
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
