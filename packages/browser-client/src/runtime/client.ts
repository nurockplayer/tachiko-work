import type { A1Translation, TranslateA1FormulaInput } from "./protocol.ts";
import type { CleanupOperation, CleanupPreview, ImportedProjection, ImportedWindowProjection, ImportOptions, ImportSelection, InteropMetadata, NativeBudgetExportPresentation, NativeTrackerExportPresentation, SourceWorkbook, SpreadsheetExport, SpreadsheetFormat } from "./interop-protocol.ts";
import type {
  FormulaCopy,
  TrackerCommand,
  BootstrapProjection,
  CapacityProjection,
  FieldBatchProjection,
  FieldExplanationProjection,
  FieldTarget,
  CanonicalTreeExport,
  CanonicalProjectFile,
  OccurrenceProjection,
  OpenedProjection,
  OpenedWindowProjection,
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
  DistinctValuesProjection,
  FindDirection,
  FindInViewProjection,
  FindPosition,
  ColumnTotalsProjection,
  GroupDimension,
  GroupScope,
  GroupTotalsProjection,
  FailureProjection,
  FieldConstraintInput,
  NumberFormat,
} from "./protocol.ts";

export interface DesignerClient {
  inspectSpreadsheet?(bytes: ArrayBuffer, format: SpreadsheetFormat, csvOptions: ImportOptions): Promise<SourceWorkbook>;
  importSpreadsheet?(bytes: ArrayBuffer, format: SpreadsheetFormat, csvOptions: ImportOptions, selection: ImportSelection, validate?: (candidate: ImportedProjection) => void): Promise<ImportedProjection>;
  /** Import with the working-size profile: the reply carries one bounded window, not the whole table. */
  importSpreadsheetWorkingSize?(bytes: ArrayBuffer, format: SpreadsheetFormat, csvOptions: ImportOptions, selection: ImportSelection, validate?: (candidate: ImportedWindowProjection) => void): Promise<ImportedWindowProjection>;
  inspectImportedProject?(bytes: ArrayBuffer, metadata: InteropMetadata): Promise<OpenedProjection>;
  inspectImportedProjectWorkingSize?(bytes: ArrayBuffer, metadata: InteropMetadata): Promise<OpenedWindowProjection>;
  openImportedProject?(bytes: ArrayBuffer, metadata: InteropMetadata): Promise<OpenedProjection>;
  openImportedProjectWorkingSize?(bytes: ArrayBuffer, metadata: InteropMetadata): Promise<OpenedWindowProjection>;
  exportSpreadsheet?(expectedRevision: string, metadata: InteropMetadata, format: SpreadsheetFormat, collection: string): Promise<SpreadsheetExport>;
  /** Rust-derived native tables; CSV selects collection, XLSX exports all scalar tables. */
  exportNativeTablesSpreadsheet?(expectedRevision: string, collection: string, format: SpreadsheetFormat): Promise<SpreadsheetExport>;
  exportNativeTrackerSpreadsheet?(expectedRevision: string, presentation: NativeTrackerExportPresentation, format: SpreadsheetFormat): Promise<SpreadsheetExport>;
  exportNativeBudgetSpreadsheet?(expectedRevision: string, presentation: NativeBudgetExportPresentation, format: SpreadsheetFormat): Promise<SpreadsheetExport>;
  translateA1Formula?(input: TranslateA1FormulaInput): Promise<A1Translation>;
  previewCleanup?(expectedRevision: string, operation: CleanupOperation): Promise<CleanupPreview>;
  commitCleanup?(expectedRevision: string, previewId: string): Promise<PublicationProjection>;
  copyFormula?(expectedRevision: string, request: FormulaCopy): Promise<PublicationProjection>;
  newTracker?(): Promise<OpenedProjection>;
  newBudget?(): Promise<OpenedProjection>;
  newTable?(name: string, columns: NewTableColumnInput[]): Promise<OpenedProjection>;
  /** Create a table whose session profile is the working-size one. */
  newTableWorkingSize?(name: string, columns: NewTableColumnInput[]): Promise<OpenedWindowProjection>;
  duplicateCollection?(expectedRevision: string, collection: string, name: string, columnNames?: Record<string, string>): Promise<DuplicateCollectionProjection>;
  renameTable?(expectedRevision: string, collection: string, name: string): Promise<PublicationProjection>;
  trackerCommand?(request: TrackerCommand): Promise<PublicationProjection>;
  clearField?(expectedRevision: string, target: FieldTarget): Promise<PublicationProjection>;
  /** Convert every value of one scalar column to another scalar type as one undoable edit. */
  changeColumnType?(expectedRevision: string, collection: string, field: string, fieldType: string): Promise<PublicationProjection>;
  setColumnRequiredness?(expectedRevision: string, collection: string, field: string, required: boolean): Promise<PublicationProjection>;
  /** Replace one durable constraint; `none` clears it. Constrained projects export as v3 only. */
  setColumnConstraint?(expectedRevision: string, collection: string, field: string, constraint: FieldConstraintInput): Promise<PublicationProjection>;
  /** Replace or clear one Number column's durable display format. */
  setColumnNumberFormat?(expectedRevision: string, collection: string, field: string, numberFormat: NumberFormat | null): Promise<PublicationProjection>;
  /** Move one column to a zero-based index; the order is durable document state. */
  moveColumn?(expected: string, collection: string, field: string, toIndex: number): Promise<PublicationProjection>;
  bootstrap(): Promise<BootstrapProjection>;
  queryCapacity?(expectedRevision?: string): Promise<CapacityProjection>;
  inspectProject?(bytes: ArrayBuffer): Promise<OpenedProjection>;
  inspectProjectWorkingSize?(bytes: ArrayBuffer): Promise<OpenedWindowProjection>;
  openProject(bytes: ArrayBuffer): Promise<OpenedProjection>;
  openProjectWorkingSize?(bytes: ArrayBuffer): Promise<OpenedWindowProjection>;
  openCanonicalTree?(files: readonly CanonicalProjectFile[]): Promise<OpenedProjection>;
  openCanonicalTreeWorkingSize?(files: readonly CanonicalProjectFile[]): Promise<OpenedWindowProjection>;
  openLocalDocument?(bytes: ArrayBuffer): Promise<OpenedProjection>;
  openLocalDocumentWorkingSize?(bytes: ArrayBuffer): Promise<OpenedWindowProjection>;
  exportProject(expectedRevision: string): Promise<ProjectExport>;
  exportProjectV3?(expectedRevision: string): Promise<ProjectExport>;
  exportCanonicalTree?(expectedRevision: string): Promise<CanonicalTreeExport>;
  exportPortableRo?(expectedRevision: string): Promise<ProjectExport>;
  verifyPortableRo?(bytes: ArrayBuffer): Promise<void>;
  openPortableRo?(bytes: ArrayBuffer): Promise<OpenedProjection>;
  openPortableRoWorkingSize?(bytes: ArrayBuffer): Promise<OpenedWindowProjection>;
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
    options?: { display?: boolean },
  ): Promise<TableWindowProjection>;
  /** The entity's zero-based position in that same view, or `filtered_out`. */
  locateInView(
    expectedRevision: string,
    collection: string,
    view: TableView,
    entity: string,
  ): Promise<LocatedInViewProjection>;
  /**
   * Distinct display texts of one column over the view's rows, in Sheet's list
   * order (`""` first, then natural order); `limit` is 1..=1,000.
   */
  queryDistinctValues(
    expectedRevision: string,
    collection: string,
    view: TableView,
    field: string,
    limit: number,
  ): Promise<DistinctValuesProjection>;
  /**
   * The next or previous cell of the view whose display text contains `query`
   * (case-insensitive), with its ordinal and the view's total match count.
   * `fields` are the shown columns in shown order; a blank query finds nothing.
   */
  findInView(
    expectedRevision: string,
    collection: string,
    view: TableView,
    query: string,
    fields: string[],
    from: FindPosition | undefined,
    direction: FindDirection,
  ): Promise<FindInViewProjection>;
  /** Number totals over exactly these entities; unknown, duplicate or foreign ids refuse. */
  queryColumnTotals(
    expectedRevision: string,
    collection: string,
    entities: string[],
  ): Promise<ColumnTotalsProjection>;
  /**
   * Number totals over the rows a view shows, summed in collection order;
   * `keep_entities` rows count even when filtered out.
   */
  queryViewTotals(
    expectedRevision: string,
    collection: string,
    view: TableView,
  ): Promise<ColumnTotalsProjection>;
  /** Number totals grouped by one or two columns over the rows one scope covers. */
  queryGroupTotals(
    expectedRevision: string,
    collection: string,
    dimensions: GroupDimension[],
    measures: string[],
    scope: GroupScope,
  ): Promise<GroupTotalsProjection>;
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
