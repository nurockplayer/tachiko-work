import type { DesignerClient } from "./runtime/client.ts";
import type { CleanupOperation, CleanupPreview, FidelityFinding, ImportedProjection, ImportFieldType, ImportOptions, ImportSelection, SourceWorkbook, SpreadsheetExport, SpreadsheetFormat } from "./runtime/interop-protocol.ts";
import type { TableProjection } from "./runtime/protocol.ts";

const TYPES: ImportFieldType[] = ["text", "number", "boolean", "date"];
function element<K extends keyof HTMLElementTagNameMap>(tag: K, text = ""): HTMLElementTagNameMap[K] {
  const result = document.createElement(tag); result.textContent = text; return result;
}
function label(root: HTMLElement, name: string, control: HTMLElement): void {
  const wrapper = element("label", name); control.setAttribute("aria-label", name); wrapper.append(control); root.append(wrapper);
}
function select(root: HTMLElement, name: string, options: Array<[string, string]>, value?: string): HTMLSelectElement {
  const control = element("select");
  for (const [id, text] of options) { const item = element("option", text); item.value = id; control.append(item); }
  if (value !== undefined) control.value = value;
  label(root, name, control); return control;
}
function input(root: HTMLElement, name: string, value = ""): HTMLInputElement {
  const control = element("input"); control.value = value; label(root, name, control); return control;
}
function button(root: HTMLElement, text: string, action: () => void): HTMLButtonElement {
  const control = element("button", text); control.type = "button"; control.addEventListener("click", action); root.append(control); return control;
}
export function mountFidelityLedger(root: HTMLElement, findings: FidelityFinding[]): void {
  const section = element("section"); section.setAttribute("aria-label", "Compatibility ledger");
  section.append(element("h3", "Compatibility ledger"));
  if (!findings.length) section.append(element("p", "No additional source features were reported by the declared profile."));
  const list = element("ul");
  for (const item of findings) list.append(element("li", `${item.category} · ${item.location} · ${item.code}: ${item.message}${item.blocking ? " — import blocked" : ""}`));
  section.append(list); root.append(section);
}
export function downloadSpreadsheet(exported: SpreadsheetExport, format: SpreadsheetFormat): void {
  const blob = new Blob([exported.bytes], {type: format === "xlsx" ? "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" : "text/csv;charset=utf-8"});
  const url = URL.createObjectURL(blob); const link = element("a"); link.href = url; link.download = `tachiko-export.${format}`; link.click(); setTimeout(() => { URL.revokeObjectURL(url); }, 1000);
}

export type ImportSource = {name: string; format: SpreadsheetFormat; bytes: ArrayBuffer};
/** Inspection is local to this panel; only explicit acceptance replaces the resident. */
export class SpreadsheetImportPanel {
  #source: ImportSource | null = null;
  #book: SourceWorkbook | null = null;
  #selection: ImportSelection = {column_types: [], extra_columns: []};
  #options: ImportOptions = {delimiter: ",", header: true};
  #pending = false;
  #message = "";
  constructor(private readonly client: DesignerClient, private readonly accept: (source: ImportSource, options: ImportOptions, selection: ImportSelection) => Promise<ImportedProjection>, private readonly changed: () => void) {}
  mount(root: HTMLElement, disabled: boolean): void {
    const panel = element("details"); panel.open = this.#source !== null; panel.append(element("summary", "Import CSV / XLSX"));
    panel.append(element("p", "Profile: up to 2 MiB, 4 sheets, 16 columns, 64 data rows per sheet and 32 numeric formulas, subject to project admission limits. CSV starts as Text: select types explicitly. Dates use YYYY-MM-DD. Ambiguous values remain Text. Original bytes and this ledger are saved with the browser project."));
    const controls = element("fieldset"); controls.disabled = disabled || this.#pending;
    const file = element("input"); file.type = "file"; file.accept = ".csv,.xlsx"; label(controls, "Spreadsheet file", file);
    const delimiter = select(controls, "CSV delimiter", [[",", "Comma"], [";", "Semicolon"], ["\t", "Tab"]], this.#options.delimiter);
    const header = element("input"); header.type = "checkbox"; header.checked = this.#options.header; label(controls, "CSV first row is header", header);
    const inspect = async (): Promise<void> => {
      this.#book = null; this.#selection = {column_types: [], extra_columns: []};
      if (!this.#source || !this.client.inspectSpreadsheet) {
        this.#pending = false;
        this.#message = this.#source ? "Spreadsheet inspection is unavailable. Choose a source after the runtime is available." : "Choose a spreadsheet source to inspect.";
        this.changed(); return;
      }
      this.#pending = true; this.#message = "Inspecting source…"; this.changed();
      try {
        const book = await this.client.inspectSpreadsheet(this.#source.bytes, this.#source.format, this.#options);
        this.#book = book;
        this.#selection = {column_types: book.sheets.map(sheet => sheet.columns.map((_, col) => {
          const types = new Set(sheet.rows.map(row => row[col]?.formula ? "number" : row[col]?.value.kind).filter(kind => kind !== undefined && kind !== "empty"));
          return types.size === 1 && this.#source?.format === "xlsx" ? [...types][0] as ImportFieldType : "text";
        })), extra_columns: book.sheets.map(() => [])};
        this.#message = "Inspect the data, compatibility findings and types before accepting.";
      } catch (error) { this.#message = error instanceof Error ? error.message : String(error); }
      finally { this.#pending = false; this.changed(); }
    };
    file.addEventListener("change", () => {
      const candidate = file.files?.[0]; if (!candidate) return;
      // A new selection invalidates every prior inspection before any fallible
      // validation/read. Failure must never make the old source acceptable.
      this.#source = null; this.#book = null; this.#selection = {column_types: [], extra_columns: []}; this.#pending = false;
      if (!/\.(csv|xlsx)$/i.test(candidate.name) || candidate.size > 2097152 || candidate.size === 0) { this.#message = "Choose a CSV or XLSX file of 1..2097152 bytes."; this.changed(); return; }
      this.#pending = true; this.#message = "Reading source…"; this.changed();
      void candidate.arrayBuffer().then(bytes => { this.#source = {name: candidate.name, bytes, format: /\.xlsx$/i.test(candidate.name) ? "xlsx" : "csv"}; return inspect(); }).catch((error: unknown) => { this.#pending = false; this.#message = error instanceof Error ? error.message : String(error); this.changed(); });
    });
    const reconfigure = (): void => { this.#options = {delimiter: delimiter.value, header: header.checked}; void inspect(); };
    delimiter.addEventListener("change", reconfigure); header.addEventListener("change", reconfigure);
    if (this.#book) {
      for (const [sheetIndex, sheet] of this.#book.sheets.entries()) {
        const group = element("section"); group.append(element("h3", `${sheet.name} · ${String(sheet.rows.length)} rows`));
        for (const [columnIndex, column] of sheet.columns.entries()) {
          const type = select(group, `${sheet.name}: ${column.name} type`, TYPES.map(kind => [kind, kind]), this.#selection.column_types[sheetIndex]?.[columnIndex]);
          type.addEventListener("change", () => { const row = this.#selection.column_types[sheetIndex]; if (row) row[columnIndex] = type.value as ImportFieldType; });
        }
        const table = element("table"); const heading = element("tr"); for (const col of sheet.columns) heading.append(element("th", col.name)); table.append(heading);
        for (const row of sheet.rows.slice(0, 8)) { const tr = element("tr"); for (const cell of row) tr.append(element("td", cell.formula ? `=${cell.formula}` : cell.value.kind === "empty" ? "[missing]" : String(cell.value.value))); table.append(tr); } group.append(table);
        group.append(element("p", "Optional output columns let split and type conversion preserve source cells. They begin missing; cleanup publishes typed values."));
        for (const [index, extra] of (this.#selection.extra_columns[sheetIndex] ?? []).entries()) {
          const name = input(group, `${sheet.name} output ${String(index + 1)} name`, extra.name);
          const type = select(group, `${sheet.name} output ${String(index + 1)} type`, TYPES.map(kind => [kind, kind]), extra.field_type);
          name.addEventListener("input", () => { extra.name = name.value; }); type.addEventListener("change", () => { extra.field_type = type.value as ImportFieldType; });
        }
        const add = button(group, `Add output column to ${sheet.name}`, () => { this.#selection.extra_columns[sheetIndex]?.push({name: `Output ${String(this.#selection.extra_columns[sheetIndex].length + 1)}`, field_type: "text"}); this.changed(); });
        add.disabled = sheet.columns.length + (this.#selection.extra_columns[sheetIndex]?.length ?? 0) >= 16;
        controls.append(group);
      }
      mountFidelityLedger(controls, this.#book.ledger);
      const accept = button(controls, "Accept types and import", () => { void (async () => {
        if (!this.#source || !this.#book || this.#pending) return; this.#pending = true; this.changed();
        try { await this.accept(this.#source, this.#options, this.#selection); this.#source = null; this.#book = null; this.#message = "Import accepted."; }
        catch (error) { this.#message = error instanceof Error ? error.message : String(error); }
        finally { this.#pending = false; this.changed(); }
      })(); }); accept.disabled = this.#book.ledger.some(item => item.blocking);
    }
    controls.append(element("p", this.#message)); panel.append(controls); root.append(panel);
  }
}

export function mountCleanupPanel(root: HTMLElement, table: TableProjection, disabled: boolean, preview: (operation: CleanupOperation) => Promise<CleanupPreview>, commit: (value: CleanupPreview) => Promise<void>): void {
  const panel = element("details"); panel.append(element("summary", "Clean imported data"));
  const controls = element("fieldset"); controls.disabled = disabled;
  controls.append(element("p", "Preview an atomic change before committing. A successful cleanup clears session Undo/Redo. Split and conversion write into existing output columns; source cells stay intact. Row numbers below refer to canonical data order, independent of sorting and filtering."));
  const operation = select(controls, "Cleanup operation", ["trim", "replace", "split", "convert", "fill", "deduplicate"].map(value => [value, value]));
  const columns = table.columns.map(col => [col.id, col.key] as [string, string]);
  const field = select(controls, "Cleanup source column", columns);
  const row = select(controls, "Cleanup source row", table.rows.map((item, index) => [item.id, `Row ${String(index + 1)} (${item.key})`]));
  const destination = select(controls, "Cleanup destination column", columns);
  const destination2 = select(controls, "Cleanup second split column", columns);
  const value = input(controls, "Cleanup find / separator / fill value");
  const replacement = input(controls, "Cleanup replacement");
  const output = element("div"); output.setAttribute("aria-live", "polite");
  button(controls, "Preview cleanup", () => { void (async () => {
    controls.disabled = true; output.replaceChildren();
    try {
      const fields = table.rows.map(item => ({entity: item.id, field: field.value}));
      const presentFields = fields.filter(target => table.rows.find(item => item.id === target.entity)?.fields.some(item => item.target.field === target.field));
      const source = {entity: row.value, field: field.value};
      let request: CleanupOperation;
      switch (operation.value) {
        case "trim": request = {kind: "trim", fields: presentFields}; break;
        case "replace": request = {kind: "replace", fields: presentFields, find: value.value, replacement: replacement.value}; break;
        case "split": request = {kind: "split", source, destinations: [destination.value, destination2.value].map(id => ({entity: row.value, field: id})), separator: value.value}; break;
        case "convert": request = {kind: "convert", source, destination: {entity: row.value, field: destination.value}}; break;
        case "deduplicate": request = {kind: "deduplicate", entities: table.rows.map(item => item.id), key_fields: [field.value]}; break;
        default: {
          const type = table.columns.find(col => col.id === field.value)?.field_type.toLowerCase();
          if (type === "boolean" && !["true", "false"].includes(value.value)) throw new Error("Boolean fill requires true or false.");
          request = {kind: "fill", fields: fields.filter(target => !table.rows.find(item => item.id === target.entity)?.fields.some(item => item.target.field === target.field)), input: type === "number" ? {kind: "number", input: value.value} : type === "boolean" ? {kind: "boolean", value: value.value === "true"} : {kind: type === "date" ? "date" : "text", value: value.value}};
        }
      }
      const result = await preview(request);
      output.append(element("h3", "Cleanup preview"));
      for (const change of result.changes) {
        const col = table.columns.find(item => item.id === change.target.field);
        const index = table.rows.findIndex(item => item.id === change.target.entity);
        output.append(element("p", `Row ${String(index + 1)} · ${col?.key ?? change.target.field}: ${change.before ? JSON.stringify(change.before.stored) : "[missing]"} → ${change.after ? JSON.stringify(change.after) : "[removed]"}`));
      }
      output.append(element("p", `${String(result.removed_entities.length)} rows removed. Preview revision: ${result.revision}.`));
      button(output, "Commit exact cleanup preview", () => { void commit(result).catch((error: unknown) => { output.append(element("p", error instanceof Error ? error.message : String(error))); }); });
    } catch (error) { output.append(element("p", error instanceof Error ? error.message : String(error))); }
    finally { controls.disabled = disabled; }
  })(); });
  controls.append(output); panel.append(controls); root.append(panel);
}
