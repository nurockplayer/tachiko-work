import type { CleanupOperation, CleanupPreview } from "./runtime/interop-protocol.ts";
import type { FieldProjection, FieldTarget, TableProjection } from "./runtime/protocol.ts";

type Match = { target: FieldTarget; column: string; row: string; value: string };

type Options = {
  preview(operation: CleanupOperation): Promise<CleanupPreview>;
  commit(preview: CleanupPreview): Promise<void>;
};

const escapeHtml = (value: string): string => value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");

function textMatch(field: FieldProjection, selected: ReadonlySet<string>, find: string): string | null {
  if (!selected.has(field.target.field) || field.formula !== null || field.stored?.kind !== "text" || find === "") return null;
  return field.stored.value.includes(find) ? field.stored.value : null;
}

/** Derive literal Find targets from semantic Text values, never rendered cells or view position. */
export function findLiteralTextMatches(table: TableProjection, fields: ReadonlySet<string>, find: string): Match[] {
  return table.rows.flatMap(row => row.fields.flatMap(field => {
    const value = textMatch(field, fields, find);
    if (value === null) return [];
    return [{ target: field.target, column: table.columns.find(column => column.id === field.target.field)?.key ?? field.target.field, row: row.key, value }];
  }));
}

export class FindReplacePanel {
  #open = false;
  #root: HTMLElement | null = null;
  #table: TableProjection | null = null;
  #find = "";
  #replacement = "";
  #selected = new Set<string>();
  #matches: Match[] = [];
  #current = -1;
  #preview: { key: string; value: CleanupPreview } | null = null;
  #message = "Select one or more Text fields, then find exact case-sensitive literal text.";
  #generation = 0;
  #running = false;

  constructor(private readonly options: Options) {}

  open(root: HTMLElement, table: TableProjection, disabled: boolean): void {
    this.#open = true;
    this.mount(root, table, disabled);
  }

  close(): void {
    this.#open = false;
    this.#preview = null;
    this.#generation += 1;
    this.#root?.querySelector("[data-find-replace-dialog]")?.remove();
  }

  mount(root: HTMLElement, table: TableProjection, disabled: boolean): void {
    const previous = this.#table;
    this.#root = root;
    this.#table = table;
    if (previous !== null && previous.collection.id !== table.collection.id) {
      this.#find = "";
      this.#replacement = "";
      this.#selected.clear();
      this.#current = -1;
      this.#message = "Select one or more Text fields, then find exact case-sensitive literal text.";
    }
    if (previous !== null && previous.revision !== table.revision) this.#invalidatePreview();
    this.#refreshMatches();
    this.#render(disabled);
  }

  #key(): string {
    const table = this.#table;
    return JSON.stringify([table?.collection.id, table?.revision, this.#find, this.#replacement, [...this.#selected].sort()]);
  }

  #invalidatePreview(): void {
    this.#preview = null;
    this.#generation += 1;
  }

  #refreshMatches(): void {
    if (this.#table === null) return;
    const previous = this.#matches[this.#current];
    this.#matches = findLiteralTextMatches(this.#table, this.#selected, this.#find);
    const index = previous === undefined ? -1 : this.#matches.findIndex(match => match.target.entity === previous.target.entity && match.target.field === previous.target.field);
    this.#current = index;
  }

  #focusCurrent(): void {
    const current = this.#matches[this.#current];
    if (!current || !this.#root) return;
    const entity = JSON.stringify(current.target.entity);
    const field = JSON.stringify(current.target.field);
    const cell = [...this.#root.querySelectorAll<HTMLElement>("[data-generic-cell]")].find(candidate => candidate.dataset.genericEntity === entity && candidate.dataset.genericField === field);
    if (typeof cell?.scrollIntoView === "function") cell.scrollIntoView({ block: "nearest", inline: "nearest" });
    cell?.focus();
  }

  #render(disabled: boolean): void {
    const root = this.#root;
    const table = this.#table;
    if (!root || !table) return;
    root.querySelector("[data-find-replace-dialog]")?.remove();
    if (!this.#open) return;
    const dialog = document.createElement("dialog");
    dialog.dataset.findReplaceDialog = "";
    dialog.setAttribute("aria-label", "Find / Replace");
    const available = table.columns.filter(column => column.field_type.toLowerCase() === "text");
    const current = this.#matches[this.#current];
    const preview = this.#preview;
    dialog.innerHTML = `<form data-find-replace-form>
      <h2>Find / Replace</h2>
      <p>Matches are case-sensitive literal Text values in this active table. Search does not publish changes.</p>
      <label>Find text<input aria-label="Find text" maxlength="4096"></label>
      <label>Replace text<input aria-label="Replace text" maxlength="4096"></label>
      <fieldset><legend>Text fields</legend>${available.map(column => `<label><input type="checkbox" aria-label="${escapeHtml(column.key)}" value="${escapeHtml(column.id)}" ${this.#selected.has(column.id) ? "checked" : ""}>${escapeHtml(column.key)}</label>`).join("") || "<p>No Text fields are available.</p>"}</fieldset>
      <p role="status">${escapeHtml(this.#message)} ${String(this.#matches.length)} ${this.#matches.length === 1 ? "match" : "matches"}.${current ? ` Current: ${escapeHtml(current.column)} in ${escapeHtml(current.row)}.` : ""}</p>
      <div><button type="button" data-find-action="next">Find next</button><button type="button" data-find-action="current" ${this.#current < 0 ? "disabled" : ""}>Replace current</button><button type="button" data-find-action="preview">Preview replace all</button><button type="button" data-find-action="commit" ${preview === null ? "disabled" : ""}>Commit replace all</button><button type="button" data-find-action="close">Close</button></div>
      ${preview === null ? "" : `<section aria-label="Replace all preview"><h3>Replace all preview</h3><p>${String(preview.value.changes.length)} changes at revision ${escapeHtml(preview.value.revision)}.</p><ul>${preview.value.changes.map(change => { const match = this.#matches.find(item => item.target.entity === change.target.entity && item.target.field === change.target.field); return `<li>${escapeHtml(match?.column ?? change.target.field)} in ${escapeHtml(match?.row ?? change.target.entity)}</li>`; }).join("")}</ul></section>`}
    </form>`;
    (root.querySelector(".table-workbench") ?? root).append(dialog);
    const find = dialog.querySelector<HTMLInputElement>("[aria-label='Find text']");
    const replacement = dialog.querySelector<HTMLInputElement>("[aria-label='Replace text']");
    if (find) find.value = this.#find;
    if (replacement) replacement.value = this.#replacement;
    const locked = disabled || this.#running;
    dialog.querySelectorAll<HTMLInputElement | HTMLButtonElement>("input, button").forEach(control => { if (control.dataset.findAction !== "close") control.disabled = locked || control.disabled; });
    find?.addEventListener("input", () => {
      this.#find = find.value;
      this.#invalidatePreview();
      this.#render(disabled);
    });
    replacement?.addEventListener("input", () => {
      this.#replacement = replacement.value;
      this.#invalidatePreview();
      this.#render(disabled);
    });
    dialog.querySelectorAll<HTMLInputElement>("input[type='checkbox']").forEach(control => { control.addEventListener("change", () => {
      if (control.checked) this.#selected.add(control.value); else this.#selected.delete(control.value);
      this.#current = -1;
      this.#invalidatePreview();
      this.#refreshMatches();
      this.#render(disabled);
    }); });
    dialog.querySelectorAll<HTMLButtonElement>("[data-find-action]").forEach(button => { button.addEventListener("click", () => { void this.#action(button.dataset.findAction ?? "", disabled); }); });
    if (typeof dialog.show === "function") dialog.show(); else dialog.setAttribute("open", "");
  }

  async #action(action: string, disabled: boolean): Promise<void> {
    if (action === "close") { this.close(); return; }
    if (this.#running || this.#table === null) return;
    this.#refreshMatches();
    if (action === "next") {
      this.#current = this.#matches.length === 0 ? -1 : (this.#current + 1) % this.#matches.length;
      this.#message = this.#matches.length === 0 ? "No matching Text cells in the selected fields." : "Read-only match navigation.";
      this.#render(disabled);
      this.#focusCurrent();
      return;
    }
    if (this.#matches.length === 0) {
      this.#message = "No matching Text cells in the selected fields; no change was published.";
      this.#render(disabled);
      return;
    }
    if (action === "preview") {
      await this.#requestPreview(this.#matches.map(match => match.target), disabled);
      return;
    }
    if (action === "current") {
      const current = this.#matches[this.#current];
      if (current === undefined) return;
      await this.#requestPreview([current.target], disabled, true);
      return;
    }
    if (action === "commit" && this.#preview !== null && this.#preview.key === this.#key()) await this.#commit(this.#preview.value, disabled, true);
  }

  async #requestPreview(fields: FieldTarget[], disabled: boolean, commit = false): Promise<void> {
    if (this.#find === "" || this.#replacement === "") {
      this.#message = "Find text and replacement text are both required.";
      this.#render(disabled);
      return;
    }
    const generation = ++this.#generation;
    const key = this.#key();
    this.#running = true;
    this.#render(disabled);
    try {
      const preview = await this.options.preview({ kind: "replace", fields, find: this.#find, replacement: this.#replacement });
      if (generation !== this.#generation || key !== this.#key()) return;
      if (preview.changes.length === 0) {
        this.#message = "No values would change; no replacement was published.";
        return;
      }
      this.#preview = { key, value: preview };
      this.#message = commit ? "Replacing the selected current Text cell." : "Review the exact scoped replacement preview before committing.";
      if (commit) await this.#commit(preview, disabled);
    } catch (error) {
      if (generation === this.#generation) this.#message = error instanceof Error ? error.message : String(error);
    } finally {
      this.#running = false;
      this.#render(disabled);
    }
  }

  async #commit(preview: CleanupPreview, disabled: boolean, closeAfter = false): Promise<void> {
    this.#running = true;
    this.#render(disabled);
    try {
      await this.options.commit(preview);
      this.#preview = null;
      this.#message = "Replacement published as one reversible semantic action.";
      if (closeAfter) this.#open = false;
    } catch (error) {
      this.#message = error instanceof Error ? error.message : String(error);
    } finally {
      this.#running = false;
      this.#render(disabled);
    }
  }
}
