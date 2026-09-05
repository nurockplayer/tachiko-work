import { parseInteropState, type InteropState } from "./interop-state.ts";
import type { FieldProjection, TableProjection } from "./runtime/protocol.ts";
import { parseBudgetViews, type BudgetViews } from "./budget-views.ts";
export type CellStyle = {
    bold?: boolean;
    fill?: boolean;
    wrap?: boolean;
    border?: boolean;
    align?: "left" | "center" | "right";
};
export type NumberFormat = "number" | "percentage" | "currency-jpy" | "currency-usd";
export type TrackerView = {
    budgetViews?: BudgetViews;
    interop?: InteropState;
    version: 1;
    cells: Record<string, CellStyle>;
    order: string[];
    widths: Record<string, number>;
    rowHeight: number;
    header: boolean;
    formats: Record<string, NumberFormat>;
};
export const emptyTrackerView = (): TrackerView => ({ version: 1, cells: {}, order: [], widths: {}, rowHeight: 36, header: true, formats: {} });
export const cellKey = (entity: string, field: string): string => JSON.stringify([entity, field]);
/** Budget bindings require IDs from the admitted project snapshot, never the sidecar itself. */
export function parseTrackerView(input?: string, collectionIds?: string[]): TrackerView {
    if (input === undefined)
        return emptyTrackerView();
    const parsed: unknown = JSON.parse(input);
    if (typeof parsed !== "object" || parsed === null)
        throw new Error("Saved tracker layout is invalid.");
    const view = parsed as Record<string, unknown>;
    if (view.version !== 1 || !Array.isArray(view.order) || !view.order.every(x => typeof x === "string") || typeof view.cells !== "object" || view.cells === null || Array.isArray(view.cells) || typeof view.widths !== "object" || view.widths === null || Array.isArray(view.widths) || (typeof view.rowHeight !== "number" || ![36, 56, 80].includes(view.rowHeight)) || typeof view.header !== "boolean" || (view.formats !== undefined && (typeof view.formats !== "object" || view.formats === null || Array.isArray(view.formats))))
        throw new Error("Unsupported tracker layout. Original saved project is unchanged.");
    for (const style of Object.values(view.cells as Record<string, unknown>)) {
        if (typeof style !== "object" || style === null || Array.isArray(style) || Object.entries(style).some(([key, value]) => key === "align" ? (typeof value !== "string" || !["left", "center", "right"].includes(value)) : !["bold", "fill", "wrap", "border"].includes(key) || typeof value !== "boolean"))
            throw new Error("Unsupported cell formatting.");
    }
    if (Object.values(view.widths as Record<string, number>).some(width => ![120, 200, 320].includes(width)))
        throw new Error("Unsupported column width.");
    const formats = view.formats ?? {};
    if (!Object.values(formats).every(format => typeof format === "string" && ["number", "percentage", "currency-jpy", "currency-usd"].includes(format)))
        throw new Error("Unsupported number presentation format.");
    let budgetViews: BudgetViews | undefined;
    if (view.budgetViews !== undefined) {
        if (collectionIds === undefined)
            throw new Error("Saved Budget layout requires authoritative project collection IDs.");
        budgetViews = parseBudgetViews(view.budgetViews, collectionIds);
    }
    const interop = view.interop === undefined ? undefined : parseInteropState(view.interop, collectionIds);
    return { ...(interop === undefined ? {} : {interop}), ...(view as Omit<TrackerView, "formats">), ...(interop === undefined ? {} : {interop}), formats: formats as Record<string, NumberFormat>, ...(budgetViews === undefined ? {} : { budgetViews }) };
}
export function displayField(field?: FieldProjection): string {
    if (field?.diagnostics.length)
        return `Error: ${field.diagnostics.map(d => d.code).join(", ")}`;
    if (field?.stored == null)
        return "";
    return field.stored.kind === "reference" ? field.stored.entity : String(field.stored.value);
}
// Fixed category order: valid typed values, missing, diagnostics. Direction applies
// only within valid values; equal values retain the input view's stable order.
export function compareFields(a?: FieldProjection, b?: FieldProjection, descending = false): number {
    const rank = (f?: FieldProjection): number => f?.diagnostics.length ? 2 : f?.stored == null ? 1 : 0;
    const category = rank(a) - rank(b);
    if (category !== 0)
        return category;
    if (rank(a) !== 0)
        return 0;
    const av = a?.stored, bv = b?.stored;
    let result: number;
    if (av?.kind === "number" && bv?.kind === "number")
        result = av.value - bv.value;
    else if (av?.kind === "boolean" && bv?.kind === "boolean")
        result = Number(av.value) - Number(bv.value);
    else {
        const x = displayField(a), y = displayField(b);
        result = x < y ? -1 : x > y ? 1 : 0;
    }
    return descending ? -result : result;
}
export function orderedRows(table: TableProjection, view: TrackerView): TableProjection["rows"] {
    const positions = new Map(view.order.map((id, index) => [id, index]));
    return [...table.rows].sort((a, b) => (positions.get(a.id) ?? Infinity) - (positions.get(b.id) ?? Infinity));
}
export function parseTsv(text: string): string[][] {
    if (text.length > 48000 || text.includes("\0"))
        throw new Error("Paste is limited to 48,000 characters and cannot contain NUL.");
    const rows: string[][] = [];
    let row: string[] = [], value = "", quoted = false, closed = false;
    for (let i = 0; i < text.length; i++) {
        const ch = text.charAt(i);
        if (quoted) {
            if (ch === '"') {
                if (text[i + 1] === '"') {
                    value += '"';
                    i++;
                }
                else {
                    quoted = false;
                    closed = true;
                }
            }
            else
                value += ch;
        }
        else if (ch === '"' && value === "" && !closed)
            quoted = true;
        else if (ch === "\t" || ch === "\n" || ch === "\r") {
            row.push(value);
            value = "";
            closed = false;
            if (ch !== "\t") {
                rows.push(row);
                row = [];
                if (ch === "\r" && text[i + 1] === "\n")
                    i++;
            }
        }
        else {
            if (closed)
                throw new Error("Unexpected text after a quoted clipboard cell.");
            value += ch;
        }
    }
    if (quoted)
        throw new Error("Clipboard has an unclosed quoted cell.");
    if (value !== "" || row.length > 0 || !/[\r\n]$/.test(text)) {
        row.push(value);
        rows.push(row);
    }
    if (rows.length === 0 || rows.length > 128 || rows.some(r => r.length !== rows[0]?.length || r.length > 3))
        throw new Error("Paste a rectangular range of up to 128 rows and 3 columns.");
    return rows;
}
export const encodeTsv = (rows: string[][]): string => rows.map(row => row.map(value => value === "" || /[\t\r\n"]/.test(value) ? `"${value.replaceAll('"', '""')}"` : value).join("\t")).join("\r\n");
