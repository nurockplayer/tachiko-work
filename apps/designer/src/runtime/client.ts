import type { CleanupOperation, CleanupPreview, ImportedProjection, ImportOptions, ImportSelection, InteropMetadata, NativeTrackerExportPresentation, SourceWorkbook, SpreadsheetExport, SpreadsheetFormat } from "./interop-protocol.ts";
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
  inspectSpreadsheet?(bytes: ArrayBuffer, format: SpreadsheetFormat, csvOptions: ImportOptions): Promise<SourceWorkbook>;
  importSpreadsheet?(bytes: ArrayBuffer, format: SpreadsheetFormat, csvOptions: ImportOptions, selection: ImportSelection, validate?: (candidate: ImportedProjection) => void): Promise<ImportedProjection>;
  inspectImportedProject?(bytes: ArrayBuffer, metadata: InteropMetadata): Promise<OpenedProjection>;
  exportSpreadsheet?(expectedRevision: string, metadata: InteropMetadata, format: SpreadsheetFormat, collection: string): Promise<SpreadsheetExport>;
  exportNativeTrackerSpreadsheet?(expectedRevision: string, presentation: NativeTrackerExportPresentation, format: SpreadsheetFormat): Promise<SpreadsheetExport>;
  previewCleanup?(expectedRevision: string, operation: CleanupOperation): Promise<CleanupPreview>;
  commitCleanup?(expectedRevision: string, previewId: string): Promise<PublicationProjection>;
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
