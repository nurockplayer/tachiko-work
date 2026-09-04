import { reconcileTextEdit, normalizeLineEndings } from "./text-edit.ts";
import type { TableProjection, TrackerCommand } from "./runtime/protocol.ts";
import { cellKey, compareFields, displayField, emptyTrackerView, encodeTsv, orderedRows, parseTsv, type CellStyle, type TrackerView } from "./tracker-model.ts";
type Options = {
    command(request: TrackerCommand): Promise<void>;
    changed(): void;
    failed(error: unknown): void;
    render(): void;
};
type History = {
    kind: "semantic";
} | {
    kind: "view";
    before: TrackerView;
    after: TrackerView;
};
const html = (value: string): string => value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
export class TrackerGrid {
    view: TrackerView = emptyTrackerView();
    #table: TableProjection | null = null;
    #anchor: [
        number,
        number
    ] = [0, 0];
    #focus: [
        number,
        number
    ] = [0, 0];
    #filter = "";
    #sort = "";
    #descending = false;
    #draft: string | null = null;
    #undo: History[] = [];
    #redo: History[] = [];
    readonly #options: Options;
    constructor(options: Options) { this.#options = options; }
    get pending(): boolean { return this.#draft !== null; }
    reset(view = emptyTrackerView()): void { this.view = view; this.#undo = []; this.#redo = []; this.#anchor = [0, 0]; this.#focus = [0, 0]; this.#filter = ""; this.#sort = ""; this.#draft = null; }
    #rows(): TableProjection["rows"] {
        if (this.#table === null)
            return [];
        const rows = orderedRows(this.#table, this.view).filter(row => row.fields.some(field => displayField(field).toLowerCase().includes(this.#filter.toLowerCase())));
        if (this.#sort)
            rows.sort((a, b) => compareFields(a.fields.find(f => f.target.field === this.#sort), b.fields.find(f => f.target.field === this.#sort), this.#descending));
        return rows;
    }
    #bounds(): number[] { return [Math.min(this.#anchor[0], this.#focus[0]), Math.max(this.#anchor[0], this.#focus[0]), Math.min(this.#anchor[1], this.#focus[1]), Math.max(this.#anchor[1], this.#focus[1])]; }
    #selected(): Array<{
        entity: string;
        field: string;
    }> {
        const [top = 0, bottom = 0, left = 0, right = 0] = this.#bounds();
        return this.#rows().slice(top, bottom + 1).flatMap(row => this.#table?.columns.slice(left, right + 1).map(col => ({ entity: row.id, field: col.id })) ?? []);
    }
    markup(table: TableProjection, busy: boolean): string {
        this.#table = table;
        const rows = this.#rows();
        this.#focus[0] = Math.min(this.#focus[0], Math.max(rows.length - 1, 0));
        this.#anchor[0] = Math.min(this.#anchor[0], Math.max(rows.length - 1, 0));
        const [top = 0, bottom = 0, left = 0, right = 0] = this.#bounds();
        const disabled = busy ? "disabled" : "";
        const activeRow = rows[this.#focus[0]], activeCol = table.columns[this.#focus[1]];
        const field = activeRow?.fields.find(f => f.target.field === activeCol?.id);
        const value = this.#draft ?? displayField(field);
        return `<section class="tracker" aria-label="Driver tracker">
      <div class="tracker-toolbar">
        <button data-tracker="undo" ${busy || this.#undo.length === 0 ? "disabled" : ""}>Undo</button><button data-tracker="redo" ${busy || this.#redo.length === 0 ? "disabled" : ""}>Redo</button>
        <button data-tracker="append" ${disabled}>Append row</button><button data-tracker="delete" ${busy || !rows.length ? "disabled" : ""}>Remove selected rows</button>
        <button data-tracker="up" ${disabled}>Move rows up</button><button data-tracker="down" ${disabled}>Move rows down</button>
        <label>Find / filter <input data-tracker-filter aria-label="Find / filter" value="${html(this.#filter)}" ${disabled}></label>
        <label>Sort <select data-tracker-sort aria-label="Sort column" ${disabled}><option value="">Manual order</option>${table.columns.map(c => `<option value="${html(c.id)}" ${this.#sort === c.id ? "selected" : ""}>${html(c.key)}</option>`).join("")}</select></label>
        <button data-tracker="direction" ${disabled}>${this.#descending ? "Descending" : "Ascending"}</button>
      </div>
      <div class="tracker-toolbar" aria-label="Formatting">
        ${["bold", "fill", "wrap", "border"].map(action => `<button data-tracker="${action}" ${disabled}>${action.charAt(0).toUpperCase()}${action.slice(1)}</button>`).join("")}
        <button data-tracker="align" ${disabled}>Alignment</button><button data-tracker="width" ${disabled}>Column width</button><button data-tracker="height" ${disabled}>Row height</button><button data-tracker="header" ${disabled}>Header emphasis</button>
        <button data-tracker="original" ${disabled}>Original order</button>
        <button data-tracker="all" ${disabled}>Select all</button>
      </div>
      <form data-tracker-edit class="tracker-editor"><label>Cell value ${activeCol?.dropdown_options?.length ? `<select aria-label="Cell value" ${disabled}>${activeCol.dropdown_options.map(option => `<option ${value === option ? "selected" : ""}>${html(option)}</option>`).join("")}</select>` : activeCol?.field_type.toLowerCase() === "text" ? `<textarea aria-label="Cell value" rows="2" ${disabled}></textarea>` : `<input aria-label="Cell value" value="${html(value)}" ${disabled}>`}</label><button ${busy || !activeRow ? "disabled" : ""}>Apply to selection</button><button type="button" data-tracker="cancel" ${disabled}>Cancel edit</button></form>
      <p class="tracker-help">${String(rows.length)} of ${String(table.rows.length)} rows · Click a cell; Shift-click selects a range. Arrow keys, Tab, Home/End, Page Up/Down, Enter to edit. Copy/paste with your usual keyboard shortcut. Paste uses task, estimate, done (true/false); up to 128 rows. Clear filter and choose manual original order before a multi-row paste.</p>
      <div class="tracker-scroll"><table role="grid" tabindex="0" aria-label="Tracker cells" aria-rowcount="${String(rows.length + 1)}"><thead class="${this.view.header ? "tracker-header-emphasis" : ""}"><tr><th>#</th>${table.columns.map(c => `<th scope="col" style="min-width:${String(this.view.widths[c.id] ?? 200)}px">${html(c.key)}</th>`).join("")}</tr></thead><tbody>${rows.length === 0 ? '<tr><td role="gridcell" tabindex="0" data-row="0" data-col="0" colspan="4">Paste rows here, or choose Append row.</td></tr>' : ""}${rows.map((row, r) => `<tr data-entity-id="${html(row.id)}" style="height:${String(this.view.rowHeight)}px"><th scope="row">${String(r + 1)}</th>${table.columns.map((col, c) => {
            const style = this.view.cells[cellKey(row.id, col.id)] ?? {};
            const selected = r >= top && r <= bottom && c >= left && c <= right;
            return `<td role="gridcell" data-row="${String(r)}" data-col="${String(c)}" aria-selected="${String(selected)}" tabindex="${r === this.#focus[0] && c === this.#focus[1] && !busy ? "0" : "-1"}" class="${style.bold ? "cell-bold " : ""}${style.fill ? "cell-fill " : ""}${style.wrap ? "cell-wrap " : ""}${style.border ? "cell-border" : ""}" style="text-align:${style.align ?? "left"}">${html(displayField(row.fields.find(f => f.target.field === col.id)))}</td>`;
        }).join("")}</tr>`).join("")}</tbody></table></div></section>`;
    }
    bind(root: HTMLElement, busy: boolean): void {
        if (busy)
            return;
        const focusCell = (): void => { root.querySelector<HTMLElement>(`[data-row="${String(this.#focus[0])}"][data-col="${String(this.#focus[1])}"]`)?.focus(); };
        root.querySelectorAll<HTMLElement>("[role=gridcell]").forEach(cell => {
            cell.addEventListener("click", event => { if (this.pending) {
                this.#options.failed(new Error("Apply or cancel the cell draft before selecting another cell."));
                return;
            } this.#focus = [Number(cell.dataset.row), Number(cell.dataset.col)]; if (!event.shiftKey)
                this.#anchor = [...this.#focus]; this.#options.render(); focusCell(); });
            cell.addEventListener("keydown", event => {
                if (event.key === "Enter") {
                    root.querySelector<HTMLInputElement>("[aria-label='Cell value']")?.focus();
                    event.preventDefault();
                    return;
                }
                let [r, c] = this.#focus;
                switch (event.key) {
                    case "ArrowDown":
                        r++;
                        break;
                    case "ArrowUp":
                        r--;
                        break;
                    case "ArrowLeft":
                        c--;
                        break;
                    case "ArrowRight":
                        c++;
                        break;
                    case "Tab":
                        c += event.shiftKey ? -1 : 1;
                        if (c > 2) {
                            r++;
                            c = 0;
                        }
                        if (c < 0) {
                            r--;
                            c = 2;
                        }
                        break;
                    case "Home":
                        c = 0;
                        if (event.ctrlKey || event.metaKey)
                            r = 0;
                        break;
                    case "End":
                        c = 2;
                        if (event.ctrlKey || event.metaKey)
                            r = this.#rows().length - 1;
                        break;
                    case "PageDown":
                        r += 15;
                        break;
                    case "PageUp":
                        r -= 15;
                        break;
                    default: return;
                }
                event.preventDefault();
                this.#focus = [Math.max(0, Math.min(r, this.#rows().length - 1)), Math.max(0, Math.min(c, 2))];
                if (!event.shiftKey || event.key === "Tab")
                    this.#anchor = [...this.#focus];
                this.#options.render();
                focusCell();
            });
        });
        root.querySelector("[role=grid]")?.addEventListener("paste", event => {
            const clipboard = event as ClipboardEvent;
            clipboard.preventDefault();
            try {
                const text = clipboard.clipboardData?.getData("text/plain");
                if (text === undefined)
                    throw new Error("Plain text clipboard is unavailable.");
                void this.#paste(parseTsv(text)).catch((error: unknown) => { this.#options.failed(error); });
            }
            catch (error) {
                this.#options.failed(error);
            }
        });
        root.querySelector("[role=grid]")?.addEventListener("copy", event => {
            const clipboard = event as ClipboardEvent;
            const [top = 0, bottom = 0, left = 0, right = 0] = this.#bounds();
            clipboard.clipboardData?.setData("text/plain", encodeTsv(this.#rows().slice(top, bottom + 1).map(row => this.#table?.columns.slice(left, right + 1).map(col => displayField(row.fields.find(f => f.target.field === col.id))) ?? [])));
            clipboard.preventDefault();
        });
        const editor = root.querySelector<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>("[aria-label='Cell value']");
        if (editor instanceof HTMLTextAreaElement) {
            const column = this.#table?.columns[this.#focus[1]];
            editor.value = normalizeLineEndings(this.#draft ?? displayField(this.#rows()[this.#focus[0]]?.fields.find(field => field.target.field === column?.id)));
        }
        editor?.addEventListener("input", () => { this.#draft = editor.value; this.#options.changed(); });
        root.querySelector("[data-tracker-edit]")?.addEventListener("submit", event => { event.preventDefault(); if (editor)
            void this.#apply(editor.value).catch((error: unknown) => { this.#options.failed(error); }); });
        root.querySelector<HTMLInputElement>("[data-tracker-filter]")?.addEventListener("change", event => { if (this.pending) {
            this.#options.failed(new Error("Apply or cancel the cell draft before filtering."));
            return;
        } this.#filter = (event.target as HTMLInputElement).value; this.#anchor = [0, 0]; this.#focus = [0, 0]; this.#options.render(); });
        root.querySelector<HTMLSelectElement>("[data-tracker-sort]")?.addEventListener("change", event => { if (this.pending) {
            this.#options.failed(new Error("Apply or cancel the cell draft before sorting."));
            return;
        } this.#sort = (event.target as HTMLSelectElement).value; this.#anchor = [0, 0]; this.#focus = [0, 0]; this.#options.render(); });
        root.querySelectorAll<HTMLButtonElement>("[data-tracker]").forEach(button => { button.addEventListener("click", () => { void this.#action(button.dataset.tracker ?? "").catch((error: unknown) => { this.#options.failed(error); }); }); });
    }
    async #execute(request: TrackerCommand): Promise<void> { await this.#options.command(request); this.#undo.push({ kind: "semantic" }); this.#redo = []; if (this.#undo.length > 64)
        this.#undo.shift(); this.#draft = null; this.#options.changed(); this.#options.render(); }
    async #paste(rows: string[][]): Promise<void> {
        const table = this.#table;
        if (!table)
            return;
        if (this.pending)
            throw new Error("Apply or cancel the cell draft before pasting.");
        if (rows.length > 1 && (this.#sort || this.#filter || this.view.order.length > 0))
            throw new Error("Multi-row paste requires original row order with no filter. Choose Original order first.");
        const field = table.columns[this.#focus[1]];
        if (!field)
            return;
        await this.#execute({ type: "paste_cells", expected_revision: table.revision, collection: table.collection.key, start_entity: this.#rows()[this.#focus[0]]?.id ?? null, start_field: field.id, rows });
    }
    async #apply(value: string): Promise<void> {
        const table = this.#table;
        if (!table)
            return;
        const [top = 0, bottom = 0, left = 0, right = 0] = this.#bounds();
        if ((bottom !== top || right !== left) && (this.#sort || this.#filter || this.view.order.length))
            throw new Error("Range editing requires original order with no filter.");
        const field = table.columns[left];
        if (!field)
            return;
        await this.#execute({ type: "paste_cells", expected_revision: table.revision, collection: table.collection.key, start_entity: this.#rows()[top]?.id ?? null, start_field: field.id, rows: this.#rows().slice(top, bottom + 1).map(row => table.columns.slice(left, right + 1).map(column => {
            const original = row.fields.find(cell => cell.target.field === column.id)?.stored;
            return original?.kind === "text" ? reconcileTextEdit(original.value, normalizeLineEndings(original.value), value) : value;
        })) });
    }
    #changeView(change: () => void): void { const before = structuredClone(this.view); change(); this.#undo.push({ kind: "view", before, after: structuredClone(this.view) }); this.#redo = []; if (this.#undo.length > 64)
        this.#undo.shift(); this.#options.changed(); this.#options.render(); }
    async #action(action: string): Promise<void> {
        const table = this.#table;
        if (!table)
            return;
        if (action === "cancel") {
            this.#draft = null;
            this.#options.changed();
            this.#options.render();
            return;
        }
        if (this.pending)
            throw new Error("Apply or cancel the cell draft first.");
        if (action === "undo" || action === "redo") {
            const source = action === "undo" ? this.#undo : this.#redo, target = action === "undo" ? this.#redo : this.#undo;
            const entry = source.at(-1);
            if (!entry)
                return;
            if (entry.kind === "semantic")
                await this.#options.command({ type: action, expected_revision: table.revision });
            else
                this.view = structuredClone(action === "undo" ? entry.before : entry.after);
            source.pop();
            target.push(entry);
            this.#options.changed();
            this.#options.render();
            return;
        }
        if (action === "append") {
            await this.#execute({ type: "append_row", expected_revision: table.revision, collection: table.collection.key });
            return;
        }
        if (action === "delete") {
            const entities = [...new Set(this.#selected().map(cell => cell.entity))];
            if (entities.length)
                await this.#execute({ type: "remove_rows", expected_revision: table.revision, entities });
            return;
        }
        if (action === "all") {
            this.#anchor = [0, 0];
            this.#focus = [Math.max(0, this.#rows().length - 1), 2];
            this.#options.render();
            return;
        }
        if (action === "direction") {
            this.#descending = !this.#descending;
            this.#options.render();
            return;
        }
        if (action === "up" || action === "down") {
            if (this.#sort || this.#filter)
                throw new Error("Clear sort and filter before moving rows.");
            const [top = 0, bottom = 0] = this.#bounds();
            const order = this.#rows().map(row => row.id);
            if ((action === "up" && top === 0) || (action === "down" && bottom >= order.length - 1))
                return;
            this.#changeView(() => { const moved = order.splice(top, bottom - top + 1); order.splice(top + (action === "up" ? -1 : 1), 0, ...moved); this.view.order = order; });
            return;
        }
        this.#changeView(() => {
            if (action === "original")
                this.view.order = [];
            else if (action === "header")
                this.view.header = !this.view.header;
            else if (action === "height")
                this.view.rowHeight = this.view.rowHeight === 36 ? 56 : this.view.rowHeight === 56 ? 80 : 36;
            else {
                const resized = new Set<string>();
                for (const cell of this.#selected()) {
                    if (action === "width") {
                        if (resized.has(cell.field))
                            continue;
                        resized.add(cell.field);
                        const width = this.view.widths[cell.field] ?? 200;
                        this.view.widths[cell.field] = width === 200 ? 320 : width === 320 ? 120 : 200;
                        continue;
                    }
                    const key = cellKey(cell.entity, cell.field), style: CellStyle = { ...this.view.cells[key] };
                    if (action === "align")
                        style.align = style.align === "center" ? "right" : style.align === "right" ? "left" : "center";
                    else if (action === "bold" || action === "fill" || action === "wrap" || action === "border")
                        style[action] = !style[action];
                    this.view.cells[key] = style;
                }
            }
        });
    }
}
