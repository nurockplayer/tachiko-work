import { afterEach, describe, expect, it, vi } from "vitest";
import { mountDesigner } from "../src/designer-app.ts";
import { createInteropState } from "../src/interop-state.ts";
import { defaultBudgetViews } from "../src/budget-views.ts";
import { emptyTrackerView } from "../src/tracker-model.ts";
import type { DesignerClient } from "../src/runtime/client.ts";
import type { DesignerProjectHost } from "../src/host/browser-project-host.ts";
import type { ImportedProjection, SourceWorkbook, SpreadsheetExport } from "../src/runtime/interop-protocol.ts";

const imported: ImportedProjection = {
  opened: {
    bootstrap: { title: "Imported workbook", revision: "resident/0", default_collection: "sheet_1", collections: [{ id: "sheet-id", key: "sheet_1", entity_count: 1 }] },
    table: { revision: "resident/0", collection: { id: "sheet-id", key: "sheet_1", entity_count: 1 }, columns: [{ id: "amount-id", key: "amount", field_type: "number" }], rows: [{ id: "row-id", key: "row_1", fields: [{ target: { entity: "row-id", field: "amount-id" }, address: "row_1.amount", stored: { kind: "number", value: 12 }, formula: null, calculated: null, diagnostics: [], editable_scalar: "number" }] }] },
  },
  metadata: { version: 1, sheets: [{ schema_id: "sheet-id", name: "Amounts", has_header: true, columns: [{ field_id: "amount-id", name: "Amount", width: null }], rows: [{ entity_id: "row-id", styles: [{ number_format: null, bold: false, fill: null, wrap: false, border: false, alignment: null }] }] }] },
  ledger: [],
};
const book: SourceWorkbook = { sheets: [{ name: "Another source", has_header: true, columns: [{ name: "Other", width: null }], rows: [] }], ledger: [] };
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
}
function button(root: HTMLElement, label: string): HTMLButtonElement {
  const found = [...root.querySelectorAll("button")].find(value => value.textContent === label);
  if (!found) throw new Error(`Missing ${label}`);
  return found;
}
const cleanups: Array<() => void> = [];
afterEach(() => { for (const cleanup of cleanups.splice(0)) cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); document.body.replaceChildren(); });

async function fixture(numberFormat?: string, numberValue = 12) {
  const project = structuredClone(imported);
  project.opened.table.rows[0]!.fields[0]!.stored = { kind: "number", value: numberValue };
  if (numberFormat !== undefined) project.metadata.sheets[0]!.rows[0]!.styles[0]!.number_format = numberFormat;
  const root = document.createElement("div"); document.body.append(root);
  const inspection = deferred<SourceWorkbook>();
  const receipt: SpreadsheetExport = { revision: "resident/0", bytes: new Uint8Array([80, 75, 1, 2]).buffer, ledger: [{ category: "converted", code: "captured-ledger", location: "Amounts", message: "Captured export evidence", blocking: false }] };
  const client = {
    bootstrap: vi.fn(async () => structuredClone(project.opened.bootstrap)),
    inspectProject: vi.fn(async () => structuredClone(project.opened)),
    inspectImportedProject: vi.fn(async () => structuredClone(project.opened)),
    openProject: vi.fn(async () => structuredClone(project.opened)),
    queryTable: vi.fn(async () => structuredClone(project.opened.table)),
    queryFields: vi.fn(async () => ({ revision: "resident/0", fields: [] })),
    exportProject: vi.fn(async () => ({ revision: "resident/0", bytes: new ArrayBuffer(8) })),
    closeProject: vi.fn(async () => {}), close: vi.fn(),
    editNumber: vi.fn(async () => { throw new Error("Unused edit"); }),
    editText: vi.fn(async () => { throw new Error("Unused edit"); }),
    editBoolean: vi.fn(async () => { throw new Error("Unused edit"); }),
    editDate: vi.fn(async () => { throw new Error("Unused edit"); }),
    inspectSpreadsheet: vi.fn(() => inspection.promise),
    exportSpreadsheet: vi.fn<NonNullable<DesignerClient["exportSpreadsheet"]>>().mockResolvedValue(receipt),
    importSpreadsheet: vi.fn<NonNullable<DesignerClient["importSpreadsheet"]>>().mockImplementation(async (_bytes, _format, _options, _selection, validate) => { validate?.(project); return structuredClone(project); }),
  } satisfies DesignerClient;
  const source = { name: "saved.csv", format: "csv" as const, bytes: new TextEncoder().encode("Amount\n12\n").buffer };
  const presentation = JSON.stringify({ ...emptyTrackerView(), budgetViews: defaultBudgetViews(["sheet-id"]), interop: createInteropState(project, source) });
  const host = {
    list: vi.fn(async () => [{ name: "saved.roproj", byte_length: 8, saved_at: "2026-09-05T00:00:00Z" }]),
    read: vi.fn(async () => new ArrayBuffer(8)),
    readSnapshot: vi.fn(async () => ({ bytes: new ArrayBuffer(8), presentation })),
    publish: vi.fn(async () => {}), update: vi.fn(async () => {}),
  } satisfies DesignerProjectHost;
  vi.stubGlobal("confirm", vi.fn(() => true));
  const download = vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:captured-test");
  vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
  vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});
  const app = mountDesigner(root, client, host); cleanups.push(() => { app.destroy(); });
  await app.ready;
  root.querySelector<HTMLButtonElement>("[data-open-project]")!.click();
  await vi.waitFor(() => { expect(button(root, "Export XLSX").disabled).toBe(false); });
  const inspect = async (name = "other.csv") => {
    const file = root.querySelector<HTMLInputElement>('[aria-label="Spreadsheet file"]')!;
    Object.defineProperty(file, "files", { value: [new File(["Other\n"], name)], configurable: true });
    file.dispatchEvent(new Event("change"));
    await vi.waitFor(() => { expect(client.inspectSpreadsheet).toHaveBeenCalledTimes(1); });
  };
  return { root, app, client, host, receipt, inspection, inspect, download, project };
}

describe("spreadsheet export review ownership", () => {
  it.each(["accept", "cancel"])("keeps captured %s actionable when deferred inspection completes and rebuilds the DOM", async action => {
    const { root, client, host, receipt, inspection, inspect, download } = await fixture();
    await inspect();
    button(root, "Export XLSX").click();
    await vi.waitFor(() => { expect(root.querySelector('[aria-label="Export compatibility review"]')).not.toBeNull(); });
    const previous = root.querySelector('[aria-label="Export compatibility review"]');
    expect(root.querySelector<HTMLButtonElement>("[data-save-project]")?.disabled).toBe(true);
    inspection.resolve(book);
    await vi.waitFor(() => { expect(root.textContent).toContain("Another source"); });
    expect(root.querySelector('[aria-label="Export compatibility review"]')).not.toBe(previous);
    expect(root.textContent).toContain("Captured export evidence");
    expect(root.textContent).toContain("revision resident/0");
    expect(button(root, "Cancel export").disabled).toBe(false);
    expect(button(root, "Acknowledge losses and download XLSX").disabled).toBe(false);
    button(root, action === "accept" ? "Acknowledge losses and download XLSX" : "Cancel export").click();
    expect(root.querySelector('[aria-label="Export compatibility review"]')).toBeNull();
    expect(root.querySelector<HTMLButtonElement>("[data-save-project]")?.disabled).toBe(false);
    expect(client.exportSpreadsheet).toHaveBeenCalledTimes(1);
    if (action === "accept") {
      expect(download).toHaveBeenCalledTimes(1);
      const blob = download.mock.calls[0]?.[0];
      expect(blob).toBeInstanceOf(Blob);
      expect(new Uint8Array(await (blob as Blob).arrayBuffer())).toEqual(new Uint8Array(receipt.bytes));
    } else expect(download).not.toHaveBeenCalled();
    root.querySelector<HTMLButtonElement>("[data-save-project]")!.click();
    await vi.waitFor(() => { expect(host.update).toHaveBeenCalledTimes(1); });
  });

  it("rejects a stale captured revision before creating any download", async () => {
    const { root, receipt, download } = await fixture();
    receipt.revision = "resident/previous";
    button(root, "Export XLSX").click();
    await vi.waitFor(() => { expect(root.querySelector('[aria-label="Export compatibility review"]')).not.toBeNull(); });
    button(root, "Acknowledge losses and download XLSX").click();
    expect(download).not.toHaveBeenCalled();
    expect(root.textContent).toContain("captured spreadsheet is no longer current");
    expect(root.querySelector<HTMLButtonElement>("[data-save-project]")?.disabled).toBe(false);
  });

  it.each(["export", "inspection"])("does not resurrect review or lock replacement UI when deferred %s completes after destroy", async pending => {
    const { root, app, client, receipt, inspection, inspect, download } = await fixture();
    const exported = deferred<SpreadsheetExport>();
    if (pending === "export") client.exportSpreadsheet = vi.fn(() => exported.promise);
    else await inspect();
    button(root, "Export XLSX").click();
    await vi.waitFor(() => { expect(client.exportSpreadsheet).toHaveBeenCalledTimes(1); });
    if (pending === "inspection") await vi.waitFor(() => { expect(root.querySelector('[aria-label="Export compatibility review"]')).not.toBeNull(); });
    app.destroy();
    const replacement = document.createElement("button"); replacement.textContent = "Replacement Save"; root.append(replacement);
    exported.resolve(receipt); inspection.resolve(book);
    // Flush both the export continuation and inspection's render callback.
    await new Promise<void>(resolve => { setTimeout(resolve, 0); });
    expect(root.children).toHaveLength(1);
    expect(root.firstElementChild).toBe(replacement);
    expect(replacement.disabled).toBe(false);
    expect(download).not.toHaveBeenCalled();
    expect(client.close).toHaveBeenCalled();
  });
});

describe("localized imported currency presentation", () => {
  it("imports localized JPY as JPY and preserves its original pattern until an explicit different format is selected", async () => {
    const pattern = "[$¥-411]#,##0";
    const { root, client, inspection, inspect, project } = await fixture(pattern);
    await inspect("localized-jpy.xlsx");
    inspection.resolve({ sheets: [{ name: "Amounts", has_header: true, columns: [{ name: "Amount", width: null }], rows: [[{ value: { kind: "number", value: 12 }, formula: null, style: structuredClone(project.metadata.sheets[0]!.rows[0]!.styles[0]!) }]] }], ledger: [] });
    await vi.waitFor(() => { expect(button(root, "Accept types and import").disabled).toBe(false); });
    button(root, "Accept types and import").click();
    await vi.waitFor(() => { expect(root.textContent).toContain("Spreadsheet imported"); });
    expect(client.importSpreadsheet).toHaveBeenCalledTimes(1);
    expect(root.querySelector("[data-format-cycle]")?.textContent).toBe("JPY");
    expect(root.querySelector("[data-formatted-number]")?.textContent).toBe(new Intl.NumberFormat("ja-JP", { style: "currency", currency: "JPY", maximumFractionDigits: 0 }).format(12));
    const exportAndCancel = async (expectedFormat: string) => {
      const prior = client.exportSpreadsheet.mock.calls.length;
      button(root, "Export XLSX").click();
      await vi.waitFor(() => { expect(root.querySelector('[aria-label="Export compatibility review"]')).not.toBeNull(); });
      expect(client.exportSpreadsheet.mock.calls[prior]?.[1].sheets[0]?.rows[0]?.styles[0]?.number_format).toBe(expectedFormat);
      button(root, "Cancel export").click();
    };
    await exportAndCancel(pattern);
    // JPY -> Percentage -> USD -> Number -> JPY. Only explicit changes
    // replace the source format; returning to JPY restores its exact pattern.
    for (const [label, exportedPattern] of [["Percentage", "0.00%"], ["USD", "$0.00"], ["Number", "0.00"], ["JPY", pattern]]) {
      root.querySelector<HTMLButtonElement>("[data-format-cycle]")!.click();
      expect(root.querySelector("[data-format-cycle]")?.textContent).toBe(label);
      await exportAndCancel(exportedPattern!);
    }
    expect(project.metadata.sheets[0]?.rows[0]?.styles[0]?.number_format).toBe(pattern);
    expect(root.querySelector<HTMLInputElement>('input[type="number"]')?.value).toBe("12");
  });
});


describe("mixed-section imported presentation", () => {
  it("shows positive one as Number and retains the entire original pattern on export", async () => {
    const pattern = "0;0%";
    const { root, client } = await fixture(pattern, 1);
    expect(root.querySelector("[data-format-cycle]")?.textContent).toBe("Number");
    expect(root.querySelector("[data-formatted-number]")?.textContent).toBe("1");
    button(root, "Export XLSX").click();
    await vi.waitFor(() => { expect(root.querySelector('[aria-label="Export compatibility review"]')).not.toBeNull(); });
    expect(client.exportSpreadsheet.mock.calls[0]?.[1].sheets[0]?.rows[0]?.styles[0]?.number_format).toBe(pattern);
    button(root, "Cancel export").click();
  });
});
