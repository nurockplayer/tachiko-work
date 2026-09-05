import type { FieldProjection, TableProjection } from "./runtime/protocol.ts";
import { compareFields } from "./tracker-model.ts";

/** Private presentation state; field references are stable IDs, never column positions. */
export type GenericTableView = {
  sortField: string | null;
  descending: boolean;
  filterField: string | null;
  filterText: string;
};
export const emptyGenericTableView = (): GenericTableView => ({ sortField: null, descending: false, filterField: null, filterText: "" });

export function validateGenericTableView(input: unknown, allowedFieldIds: string[]): GenericTableView {
  if (typeof input !== "object" || input === null || Array.isArray(input)) throw new Error("Invalid imported table view.");
  const view = input as Record<string, unknown>;
  const field = (value: unknown): value is string | null => value === null || (typeof value === "string" && allowedFieldIds.includes(value));
  if (Object.keys(view).some(key => !["sortField", "descending", "filterField", "filterText"].includes(key)) ||
    !field(view.sortField) || !field(view.filterField) || typeof view.descending !== "boolean" ||
    typeof view.filterText !== "string" || view.filterText.length > 256) {
    throw new Error("Unsupported imported table view: choose known columns and a filter of at most 256 characters.");
  }
  return { sortField: view.sortField, descending: view.descending, filterField: view.filterField, filterText: view.filterText };
}

/** Adapt already-authoritative calculation results to the shared typed comparator. */
function comparable(field: FieldProjection | undefined): FieldProjection | undefined {
  if (!field || field.diagnostics.length || field.formula === null) return field;
  if (field.calculated?.status === "value" && Number.isFinite(field.calculated.value)) {
    return { ...field, stored: { kind: "number", value: field.calculated.value } };
  }
  if (field.calculated?.status === "failure") {
    return { ...field, stored: null, diagnostics: [{ code: field.calculated.code, message: field.calculated.message, path: field.address }] };
  }
  return { ...field, stored: null };
}

function displayedText(field: FieldProjection | undefined): string {
  if (!field) return "";
  if (field.diagnostics.length) return `Error: ${field.diagnostics.map(diagnostic => `${diagnostic.code} ${diagnostic.message}`).join(", ")}`;
  if (field.formula !== null) {
    if (field.calculated?.status === "value" && Number.isFinite(field.calculated.value)) return String(field.calculated.value);
    if (field.calculated?.status === "failure") return `Error: ${field.calculated.code} ${field.calculated.message}`;
    return "Unavailable";
  }
  if (!field.stored) return "";
  return field.stored.kind === "reference" ? field.stored.entity : String(field.stored.value);
}

/** Sort/filter only the returned row projection; preserve canonical input and stable subjects. */
export function projectInteropTable(table: TableProjection, view: GenericTableView): TableProjection {
  const checked = validateGenericTableView(view, table.columns.map(column => column.id));
  const needle = checked.filterText.toLowerCase();
  const columns = checked.filterField === null ? table.columns.map(column => column.id) : [checked.filterField];
  const rows = table.rows.filter(row => needle === "" || columns.some(id => displayedText(row.fields.find(field => field.target.field === id)).toLowerCase().includes(needle)));
  if (checked.sortField !== null) {
    const sortField = checked.sortField;
    rows.sort((left, right) => compareFields(
      comparable(left.fields.find(field => field.target.field === sortField)),
      comparable(right.fields.find(field => field.target.field === sortField)),
      checked.descending,
    ));
  }
  return { ...table, rows };
}

export function mountInteropTableView(
  root: HTMLElement,
  table: TableProjection,
  view: GenericTableView,
  disabled: boolean,
  onChange: (next: GenericTableView) => void,
): void {
  const fieldIds = table.columns.map(column => column.id);
  let current = validateGenericTableView(view, fieldIds);
  const controls = document.createElement("fieldset");
  controls.disabled = disabled;
  controls.setAttribute("aria-label", "Imported table sort and filter");
  const label = (name: string, element: HTMLElement): void => {
    const wrapper = document.createElement("label");
    const text = document.createElement("span"); text.textContent = name;
    element.setAttribute("aria-label", name); wrapper.append(text, element); controls.append(wrapper);
  };
  const select = (name: string, emptyLabel: string, selected: string | null): HTMLSelectElement => {
    const element = document.createElement("select");
    const option = (text: string, value: string): void => {
      const entry = document.createElement("option"); entry.textContent = text; entry.value = value; element.append(entry);
    };
    option(emptyLabel, "");
    for (const column of table.columns) option(column.key.replaceAll("_", " "), column.id);
    element.value = selected ?? "";
    label(name, element); return element;
  };
  const sort = select("Sort by", "Original order", current.sortField);
  const descending = document.createElement("input"); descending.type = "checkbox"; descending.checked = current.descending; label("Sort descending", descending);
  const filter = select("Filter column", "All columns", current.filterField);
  const text = document.createElement("input"); text.type = "text"; text.value = current.filterText; text.maxLength = 256; label("Filter text", text);
  const changed = (): void => {
    if (disabled) return;
    current = validateGenericTableView({ sortField: sort.value || null, descending: descending.checked, filterField: filter.value || null, filterText: text.value }, fieldIds);
    onChange({ ...current });
  };
  for (const control of [sort, descending, filter]) control.addEventListener("change", changed);
  text.addEventListener("input", changed);
  root.append(controls);
}
