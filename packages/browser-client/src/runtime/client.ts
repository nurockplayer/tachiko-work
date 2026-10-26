import type { CleanupOperation, CleanupPreview, ImportedProjection, ImportOptions, ImportSelection, InteropMetadata, NativeBudgetExportPresentation, NativeTrackerExportPresentation, SourceWorkbook, SpreadsheetExport, SpreadsheetFormat } from "./interop-protocol.ts";
import type {
  FormulaCopy,
  TrackerCommand,
  BootstrapProjection,
  FieldBatchProjection,
  FieldExplanationProjection,
  FieldTarget,
  CanonicalTreeExport,
  CanonicalProjectFile,
  OccurrenceProjection,
  OpenedProjection,
  ProjectExport,
  PublicationProjection,
  DuplicateCollectionProjection,
  KeyedGroupedSumDefinitionInput,
  KeyedGroupedSumProjection,
  KeyedGroupedSumPublishedProjection,
  NewTableColumnInput,
  TableProjection,
  TableView,
  TableWindowProjection,
  LocatedInViewProjection,
  FailureProjection,
  FieldConstraintInput,
  NumberFormat,
} from "./protocol.ts";

export interface DesignerClient {
  inspectSpreadsheet?(bytes: ArrayBuffer, format: SpreadsheetFormat, csvOptions: ImportOptions): Promise<SourceWorkbook>;
  importSpreadsheet?(bytes: ArrayBuffer, format: SpreadsheetFormat, csvOptions: ImportOptions, selection: ImportSelection, validate?: (candidate: ImportedProjection) => void): Promise<ImportedProjection>;
  inspectImportedProject?(bytes: ArrayBuffer, metadata: InteropMetadata): Promise<OpenedProjection>;
  openImportedProject?(bytes: ArrayBuffer, metadata: InteropMetadata): Promise<OpenedProjection>;
  exportSpreadsheet?(expectedRevision: string, metadata: InteropMetadata, format: SpreadsheetFormat, collection: string): Promise<SpreadsheetExport>;
  /** Rust-derived native tables; CSV selects collection, XLSX exports all scalar tables. */
  exportNativeTablesSpreadsheet?(expectedRevision: string, collection: string, format: SpreadsheetFormat): Promise<SpreadsheetExport>;
  exportNativeTrackerSpreadsheet?(expectedRevision: string, presentation: NativeTrackerExportPresentation, format: SpreadsheetFormat): Promise<SpreadsheetExport>;
  exportNativeBudgetSpreadsheet?(expectedRevision: string, presentation: NativeBudgetExportPresentation, format: SpreadsheetFormat): Promise<SpreadsheetExport>;
  previewCleanup?(expectedRevision: string, operation: CleanupOperation): Promise<CleanupPreview>;
  commitCleanup?(expectedRevision: string, previewId: string): Promise<PublicationProjection>;
  copyFormula?(expectedRevision: string, request: FormulaCopy): Promise<PublicationProjection>;
  newTracker?(): Promise<OpenedProjection>;
  newBudget?(): Promise<OpenedProjection>;
  newTable?(name: string, columns: NewTableColumnInput[]): Promise<OpenedProjection>;
  duplicateCollection?(expectedRevision: string, collection: string, name: string): Promise<DuplicateCollectionProjection>;
  trackerCommand?(request: TrackerCommand): Promise<PublicationProjection>;
  clearField?(expectedRevision: string, target: FieldTarget): Promise<PublicationProjection>;
  setColumnRequiredness?(expectedRevision: string, collection: string, field: string, required: boolean): Promise<PublicationProjection>;
  /** Replace one durable constraint; `none` clears it. Constrained projects export as v3 only. */
  setColumnConstraint?(expectedRevision: string, collection: string, field: string, constraint: FieldConstraintInput): Promise<PublicationProjection>;
  /** Replace or clear one Number column's durable display format. */
  setColumnNumberFormat?(expectedRevision: string, collection: string, field: string, numberFormat: NumberFormat | null): Promise<PublicationProjection>;
  bootstrap(): Promise<BootstrapProjection>;
  inspectProject?(bytes: ArrayBuffer): Promise<OpenedProjection>;
  openProject(bytes: ArrayBuffer): Promise<OpenedProjection>;
  openCanonicalTree?(files: readonly CanonicalProjectFile[]): Promise<OpenedProjection>;
  openLocalDocument?(bytes: ArrayBuffer): Promise<OpenedProjection>;
  exportProject(expectedRevision: string): Promise<ProjectExport>;
  exportProjectV3?(expectedRevision: string): Promise<ProjectExport>;
  exportCanonicalTree?(expectedRevision: string): Promise<CanonicalTreeExport>;
  exportPortableRo?(expectedRevision: string): Promise<ProjectExport>;
  verifyPortableRo?(bytes: ArrayBuffer): Promise<void>;
  openPortableRo?(bytes: ArrayBuffer): Promise<OpenedProjection>;
  observeOccurrence?(): Promise<OccurrenceProjection>;
  closeProject(): Promise<void>;
  /** Complete table; collections above 1,000 rows refuse with `use_window_query`. */
  queryTable(collection: string): Promise<TableProjection>;
  /** Rows `[start, start+count)` of a stateless view; `count` is 0..256, 0 for totals only. */
  queryTableWindow(
    expectedRevision: string,
    collection: string,
    view: TableView,
    start: number,
    count: number,
  ): Promise<TableWindowProjection>;
  /** The entity's zero-based position in that same view, or `filtered_out`. */
  locateInView(
    expectedRevision: string,
    collection: string,
    view: TableView,
    entity: string,
  ): Promise<LocatedInViewProjection>;
  queryFields(
    expectedRevision: string,
    fields: FieldTarget[],
  ): Promise<FieldBatchProjection>;
  explainField(
    expectedRevision: string,
    target: FieldTarget,
  ): Promise<FieldExplanationProjection>;
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
  createKeyedGroupedSum?(expectedRevision: string, definition: KeyedGroupedSumDefinitionInput): Promise<KeyedGroupedSumPublishedProjection>;
  queryKeyedGroupedSum?(definitionId: string): Promise<KeyedGroupedSumProjection>;
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
