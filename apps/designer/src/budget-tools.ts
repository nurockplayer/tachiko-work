import { fieldTargetKey, type FieldProjection, type FieldTarget, type FormulaCopy, type TableProjection } from "./runtime/protocol.ts";

export type BudgetToolsDraft = {
  target?: FieldTarget;
  source?: string;
  copySource?: FieldTarget;
  destinations?: FieldTarget[];
  fixedReferences?: FieldTarget[];
  relativeRows?: boolean;
  relativeColumns?: boolean;
};
export type BudgetToolsOptions = {
  tables: TableProjection[];
  currentCollection: string;
  disabled: boolean;
  draft: BudgetToolsDraft;
  changed: () => void;
  /** Re-render current controls after clearing a completed or cancelled draft. */
  completed: () => void;
  updateFormula: (target: FieldTarget, source: string) => Promise<void>;
  copyFormula: (request: FormulaCopy) => Promise<void>;
};
export function hasBudgetToolsDraft(draft: BudgetToolsDraft): boolean {
  return Object.keys(draft).length > 0;
}
type Choice = { field: FieldProjection; collection: string; label: string };
const humanize = (text: string): string => text.replace(/[_-]+/g, " ").replace(/\b\w/g, value => value.toUpperCase());

/** Render authoring gestures only. Parsing, binding and publication belong to Rust. */
export function mountBudgetTools(root: HTMLElement, options: BudgetToolsOptions): void {
  const { draft } = options;
  const choices: Choice[] = options.tables.flatMap(table => table.rows.flatMap(row =>
    table.columns.filter(column => column.field_type.toLowerCase() === "number").flatMap(column => {
      const field = row.fields.find(value => value.target.field === column.id);
      return field ? [{ field, collection: table.collection.id, label: [table.collection.key, row.key, column.key].map(humanize).join(" / ") }] : [];
    })));
  const local = choices.filter(choice => choice.collection === options.currentCollection);
  const panel = document.createElement("section");
  panel.className = "budget-tools";
  panel.setAttribute("aria-label", "Budget formulas");
  const heading = document.createElement("h2"); heading.textContent = "Budget formulas"; panel.append(heading);
  const note = document.createElement("p");
  note.textContent = "Supported: + - * / and min(a, b), max(a, b). Insert references by name. Rust validates and calculates formulas. Applying or copying a formula clears session undo history. Converting a formula back to a scalar value is not currently supported.";
  panel.append(note);
  const controls = document.createElement("fieldset"); controls.disabled = options.disabled; panel.append(controls);
  const status = document.createElement("p"); status.setAttribute("role", "status"); panel.append(status);
  const label = (name: string, element: HTMLElement): void => {
    const wrapper = document.createElement("label");
    const text = document.createElement("span"); text.textContent = name;
    wrapper.append(text, element); controls.append(wrapper); element.setAttribute("aria-label", name);
  };
  const select = (name: string, items: Choice[], selected?: FieldTarget, multiple = false): HTMLSelectElement => {
    const element = document.createElement("select"); element.multiple = multiple;
    for (const item of items) {
      const option = document.createElement("option"); option.value = fieldTargetKey(item.field.target); option.textContent = item.label;
      option.selected = selected !== undefined && option.value === fieldTargetKey(selected); element.append(option);
    }
    if (selected !== undefined && !items.some(item => fieldTargetKey(item.field.target) === fieldTargetKey(selected))) {
      const unavailable = document.createElement("option"); unavailable.value = ""; unavailable.textContent = "Draft target is unavailable in this table — choose a target or cancel"; element.append(unavailable); element.value = "";
    }
    label(name, element); return element;
  };
  const resolve = (element: HTMLSelectElement): Choice | undefined => choices.find(choice => fieldTargetKey(choice.field.target) === element.value);
  const selectedTargets = (element: HTMLSelectElement): FieldTarget[] => Array.from(element.selectedOptions).flatMap(option => {
    const match = choices.find(choice => fieldTargetKey(choice.field.target) === option.value); return match ? [match.field.target] : [];
  });
  const button = (text: string, action: () => void): HTMLButtonElement => {
    const element = document.createElement("button"); element.type = "button"; element.textContent = text;
    element.addEventListener("click", action); controls.append(element); return element;
  };
  const run = async (operation: () => Promise<void>, clear: () => void): Promise<void> => {
    controls.disabled = true; status.textContent = "Applying formula change…";
    try { await operation(); clear(); status.textContent = "Formula change applied."; options.changed(); options.completed(); }
    catch (error) { status.textContent = error instanceof Error ? error.message : String(error); }
    finally { controls.disabled = options.disabled; }
  };
  const target = select("Formula target", local, draft.target);
  const source = document.createElement("textarea"); source.value = draft.source ?? resolve(target)?.field.formula?.source ?? ""; label("Formula source", source);
  target.addEventListener("change", () => {
    const choice = resolve(target); if (!choice) return;
    draft.target = choice.field.target; draft.source = choice.field.formula?.source ?? ""; source.value = draft.source; options.changed();
  });
  const rememberSource = (): void => {
    const choice = resolve(target); if (choice) draft.target = choice.field.target;
    draft.source = source.value; options.changed();
  };
  source.addEventListener("input", rememberSource);
  const reference = select("Insert reference from", choices);
  button("Insert reference", () => {
    const choice = resolve(reference); if (!choice) return;
    const start = source.selectionStart; const end = source.selectionEnd;
    const inserted = `[${choice.field.address}]`;
    source.value = source.value.slice(0, start) + inserted + source.value.slice(end);
    source.selectionStart = source.selectionEnd = start + inserted.length; rememberSource(); source.focus();
  });
  button("Apply formula", () => {
    const choice = resolve(target); if (!choice) return;
    void run(() => options.updateFormula(choice.field.target, source.value), () => { delete draft.target; delete draft.source; });
  });
  button("Cancel formula draft", () => {
    delete draft.target; delete draft.source; options.changed(); options.completed();
  });
  const copyNote = document.createElement("p");
  copyNote.textContent = "Copy uses the current canonical table row and column order, not view order. Cross-collection references always stay fixed."; controls.append(copyNote);
  const copySource = select("Copy formula from", local.filter(choice => choice.field.formula !== null), draft.copySource);
  const destinations = select("Copy destinations", local, undefined, true);
  const fixed = select("Fixed references", choices, undefined, true);
  for (const [element, values] of [[destinations, draft.destinations], [fixed, draft.fixedReferences]] as const) {
    for (const option of element.options) option.selected = values?.some(value => fieldTargetKey(value) === option.value) ?? false;
  }
  const checkbox = (name: string, checked: boolean): HTMLInputElement => {
    const element = document.createElement("input"); element.type = "checkbox"; element.checked = checked; label(name, element); return element;
  };
  const rows = checkbox("Relative rows", draft.relativeRows ?? true);
  const columns = checkbox("Relative columns", draft.relativeColumns ?? true);
  const rememberCopy = (): void => {
    const choice = resolve(copySource); if (choice) draft.copySource = choice.field.target;
    draft.destinations = selectedTargets(destinations); draft.fixedReferences = selectedTargets(fixed);
    draft.relativeRows = rows.checked; draft.relativeColumns = columns.checked; options.changed();
  };
  for (const element of [copySource, destinations, fixed, rows, columns]) element.addEventListener("change", rememberCopy);
  const clearCopy = (): void => {
    delete draft.copySource; delete draft.destinations; delete draft.fixedReferences; delete draft.relativeRows; delete draft.relativeColumns;
    for (const element of [destinations, fixed]) for (const option of element.options) option.selected = false;
    rows.checked = true; columns.checked = true;
  };
  button("Copy formula", () => {
    const choice = resolve(copySource); if (!choice) return;
    const targets = selectedTargets(destinations);
    if (!targets.length) { status.textContent = "Choose at least one copy destination."; return; }
    void run(() => options.copyFormula({ source: choice.field.target, destinations: targets, fixed_references: selectedTargets(fixed), relative_rows: rows.checked, relative_columns: columns.checked }), clearCopy);
  });
  button("Cancel copy draft", () => { clearCopy(); options.changed(); options.completed(); });
  root.append(panel);
}
