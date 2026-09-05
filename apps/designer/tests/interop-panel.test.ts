import { describe, expect, it, vi } from "vitest";
import { mountCleanupPanel, SpreadsheetImportPanel } from "../src/interop-panel.ts";
import type { CleanupOperation, CleanupPreview, SourceWorkbook } from "../src/runtime/interop-protocol.ts";
import type { DesignerClient } from "../src/runtime/client.ts";
import type { FieldProjection, StoredValueProjection, TableProjection } from "../src/runtime/protocol.ts";

const target = (entity: string) => ({ entity, field: "stable-column" });
function field(entity: string, stored: StoredValueProjection): FieldProjection {
  return { target: target(entity), address: `${entity}.value`, stored, formula: null, calculated: null, diagnostics: [], editable_scalar: null };
}
function sparseTable(kind = "text"): TableProjection {
  return {
    revision: "resident/12", collection: { id: "stable-sheet", key: "Imported data", entity_count: 3 },
    columns: [{ id: "stable-column", key: "Human column name", field_type: kind }],
    rows: [
      { id: "first", key: "row_1", fields: [field("first", { kind: "text", value: " Alice " })] },
      { id: "missing", key: "row_2", fields: [] },
      { id: "last", key: "row_3", fields: [field("last", { kind: "text", value: " Bob " })] },
    ],
  };
}
function fixture(table = sparseTable()) {
  const root = document.createElement("div");
  const plan: CleanupPreview = {
    preview_id: "opaque-plan-91", revision: table.revision,
    changes: [{ target: target("first"), before: table.rows[0]?.fields[0] ?? null, after: { kind: "text", value: "Alice" } }],
    removed_entities: [],
  };
  const preview = vi.fn<(operation: CleanupOperation) => Promise<CleanupPreview>>().mockResolvedValue(plan);
  const commit = vi.fn<(preview: CleanupPreview) => Promise<void>>().mockResolvedValue(undefined);
  mountCleanupPanel(root, table, false, preview, commit);
  return { root, table, plan, preview, commit };
}
function control(root: HTMLElement, name: string): HTMLInputElement | HTMLSelectElement {
  const found = root.querySelector<HTMLInputElement | HTMLSelectElement>(`[aria-label="${name}"]`);
  if (!found) throw new Error(`Missing control: ${name}`);
  return found;
}
function click(root: HTMLElement, name: string): void {
  const found = [...root.querySelectorAll("button")].find(button => button.textContent === name);
  if (!found) throw new Error(`Missing button: ${name}`);
  found.click();
}

describe("imported cleanup panel admission", () => {
  it.each(["trim", "replace"])("%s sends present targets and leaves missing cells absent", async operation => {
    const { root, table, preview } = fixture();
    const before = structuredClone(table);
    control(root, "Cleanup operation").value = operation;
    control(root, "Cleanup find / separator / fill value").value = ".*";
    control(root, "Cleanup replacement").value = "$1";
    click(root, "Preview cleanup");
    await vi.waitFor(() => { expect(root.textContent).toContain("Cleanup preview"); });
    const fields = [target("first"), target("last")];
    expect(preview).toHaveBeenCalledExactlyOnceWith(operation === "trim" ? { kind: "trim", fields } : { kind: "replace", fields, find: ".*", replacement: "$1" });
    expect(table).toEqual(before);
    expect(table.rows[1]?.fields).toEqual([]);
  });

  it.each(["trim", "replace"])("%s does not silently omit a present incompatible value", async operation => {
    const table = sparseTable();
    table.rows[2]!.fields = [field("last", { kind: "number", value: 0 })];
    const { root, preview, commit } = fixture(table);
    preview.mockRejectedValue(new Error("cleanup requires an existing Text source"));
    control(root, "Cleanup operation").value = operation;
    control(root, "Cleanup find / separator / fill value").value = "Alice";
    click(root, "Preview cleanup");
    await vi.waitFor(() => { expect(root.textContent).toContain("cleanup requires an existing Text source"); });
    expect(preview.mock.calls[0]?.[0]).toMatchObject({ fields: [target("first"), target("last")] });
    expect(commit).not.toHaveBeenCalled();
    expect(root.textContent).not.toContain("Commit exact cleanup preview");
    expect(table.rows[2]?.fields[0]?.stored).toEqual({ kind: "number", value: 0 });
  });

  it("commits the exact preview identity and revision even after controls change, and displays a rejected commit", async () => {
    const { root, plan, preview, commit } = fixture();
    click(root, "Preview cleanup");
    await vi.waitFor(() => { expect(root.textContent).toContain("Preview revision: resident/12"); });
    expect(root.textContent).toContain("Human column name");
    control(root, "Cleanup operation").value = "deduplicate";
    control(root, "Cleanup source row").value = "last";
    commit.mockRejectedValue(new Error("Preview revision is no longer current"));
    click(root, "Commit exact cleanup preview");
    await vi.waitFor(() => { expect(root.textContent).toContain("Preview revision is no longer current"); });
    expect(commit).toHaveBeenCalledExactlyOnceWith(plan);
    expect(commit.mock.calls[0]?.[0]).toBe(plan);
    expect(preview).toHaveBeenCalledTimes(1);
  });

  it.each(["TRUE", "1", "false ", "yes", ""])("rejects nonliteral Boolean fill %j before calling the runtime", async value => {
    const { root, preview, commit } = fixture(sparseTable("boolean"));
    control(root, "Cleanup operation").value = "fill";
    control(root, "Cleanup find / separator / fill value").value = value;
    click(root, "Preview cleanup");
    await vi.waitFor(() => { expect(root.textContent).toContain("Boolean fill requires true or false."); });
    expect(preview).not.toHaveBeenCalled();
    expect(commit).not.toHaveBeenCalled();
  });

  it.each([true, false])("Boolean fill %s targets only missing slots and retains existing false", async value => {
    const table = sparseTable("boolean");
    table.rows[0]!.fields = [field("first", { kind: "boolean", value: false })];
    table.rows[2]!.fields = [field("last", { kind: "boolean", value: true })];
    const before = structuredClone(table);
    const { root, preview } = fixture(table);
    control(root, "Cleanup operation").value = "fill";
    control(root, "Cleanup find / separator / fill value").value = String(value);
    click(root, "Preview cleanup");
    await vi.waitFor(() => { expect(root.textContent).toContain("Cleanup preview"); });
    expect(preview).toHaveBeenCalledExactlyOnceWith({ kind: "fill", fields: [target("missing")], input: { kind: "boolean", value } });
    expect(table).toEqual(before);
  });
});

const sourceBook: SourceWorkbook = { sheets: [{ name: "Old inspected source", has_header: true, columns: [{ name: "Value", width: null }], rows: [] }], ledger: [] };
function importPanelFixture(inspectionAvailable = true) {
  const root = document.createElement("div");
  const unused = async (): Promise<never> => { throw new Error("Resident must not be touched during source inspection"); };
  const inspectSpreadsheet = vi.fn<NonNullable<DesignerClient["inspectSpreadsheet"]>>().mockResolvedValue(sourceBook);
  const client: DesignerClient = {
    bootstrap: unused, openProject: unused, exportProject: unused, closeProject: unused,
    queryTable: unused, queryFields: unused, editNumber: unused, editText: unused, editBoolean: unused, editDate: unused, close: vi.fn(),
    ...(inspectionAvailable ? { inspectSpreadsheet } : {}),
  };
  const accept = vi.fn<ConstructorParameters<typeof SpreadsheetImportPanel>[1]>().mockRejectedValue(new Error("Unexpected resident replacement"));
  const render = (): void => { root.replaceChildren(); panel.mount(root, false); };
  const panel = new SpreadsheetImportPanel(client, accept, render);
  render();
  const choose = (file: File | null): void => {
    const input = control(root, "Spreadsheet file");
    Object.defineProperty(input, "files", { value: file ? [file] : [], configurable: true });
    input.dispatchEvent(new Event("change"));
  };
  const acceptedSourceVisible = (): boolean => [...root.querySelectorAll("button")].some(button => button.textContent === "Accept types and import");
  return { root, client, accept, inspectSpreadsheet, choose, acceptedSourceVisible };
}

describe("spreadsheet source replacement failures", () => {
  it("discards the previous inspected source before a replacement read and never restores it on failure", async () => {
    const { root, choose, inspectSpreadsheet, accept, acceptedSourceVisible } = importPanelFixture();
    choose(new File(["Value\nold\n"], "old.csv"));
    await vi.waitFor(() => { expect(acceptedSourceVisible()).toBe(true); });
    click(root, "Add output column to Old inspected source");
    let rejectRead!: (error: Error) => void;
    const replacement = new File(["Value\nnew\n"], "new.csv");
    vi.spyOn(replacement, "arrayBuffer").mockReturnValue(new Promise<ArrayBuffer>((_resolve, reject) => { rejectRead = reject; }));
    choose(replacement);
    expect(acceptedSourceVisible()).toBe(false);
    expect(root.textContent).not.toContain("Old inspected source");
    expect(root.querySelector("fieldset")?.disabled).toBe(true);
    rejectRead(new Error("Replacement file cannot be read"));
    await vi.waitFor(() => { expect(root.textContent).toContain("Replacement file cannot be read"); });
    expect(root.querySelector("fieldset")?.disabled).toBe(false);
    expect(acceptedSourceVisible()).toBe(false);
    expect(inspectSpreadsheet).toHaveBeenCalledTimes(1);
    expect(accept).not.toHaveBeenCalled();
    // A later successful replacement can recover, with no old output columns.
    choose(new File(["Value\nretry\n"], "retry.csv"));
    await vi.waitFor(() => { expect(acceptedSourceVisible()).toBe(true); });
    expect(root.querySelector('[aria-label="Old inspected source output 1 name"]')).toBeNull();
    expect(inspectSpreadsheet.mock.calls[1]?.[0]).toEqual(new TextEncoder().encode("Value\nretry\n").buffer);
  });

  it.each([new File(["not a workbook"], "invalid.txt"), new File([], "empty.csv")])("cannot accept an old source after invalid replacement $name", async replacement => {
    const { root, choose, inspectSpreadsheet, accept, acceptedSourceVisible } = importPanelFixture();
    choose(new File(["Value\nold\n"], "old.csv"));
    await vi.waitFor(() => { expect(acceptedSourceVisible()).toBe(true); });
    choose(replacement);
    expect(root.textContent).toContain("Choose a CSV or XLSX file");
    expect(acceptedSourceVisible()).toBe(false);
    expect(root.textContent).not.toContain("Old inspected source");
    expect(root.querySelector("fieldset")?.disabled).toBe(false);
    expect(inspectSpreadsheet).toHaveBeenCalledTimes(1);
    expect(accept).not.toHaveBeenCalled();
  });

  it("reports unavailable inspection after reading without leaving the panel locked", async () => {
    const { root, choose, client, inspectSpreadsheet, accept, acceptedSourceVisible } = importPanelFixture(false);
    choose(new File(["Value\nnew\n"], "new.csv"));
    await vi.waitFor(() => { expect(root.textContent).toContain("Spreadsheet inspection is unavailable"); });
    expect(root.querySelector("fieldset")?.disabled).toBe(false);
    expect(acceptedSourceVisible()).toBe(false);
    expect(accept).not.toHaveBeenCalled();
    // The unavailable capability does not strand the chooser; retry works.
    client.inspectSpreadsheet = inspectSpreadsheet;
    choose(new File(["Value\nretry\n"], "retry.csv"));
    await vi.waitFor(() => { expect(acceptedSourceVisible()).toBe(true); });
    expect(root.querySelector("fieldset")?.disabled).toBe(false);
  });

  it("retains no previously accepted selection when replacement inspection rejects", async () => {
    const { root, choose, inspectSpreadsheet, accept, acceptedSourceVisible } = importPanelFixture();
    choose(new File(["Value\nold\n"], "old.csv"));
    await vi.waitFor(() => { expect(acceptedSourceVisible()).toBe(true); });
    inspectSpreadsheet.mockRejectedValueOnce(new Error("Replacement workbook is invalid"));
    choose(new File(["Value\nnew\n"], "new.csv"));
    await vi.waitFor(() => { expect(root.textContent).toContain("Replacement workbook is invalid"); });
    expect(acceptedSourceVisible()).toBe(false);
    expect(root.querySelector("fieldset")?.disabled).toBe(false);
    expect(accept).not.toHaveBeenCalled();
  });
});

function pendingResult<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}
const namedBook = (name: string): SourceWorkbook => ({ ...sourceBook, sheets: [{ ...sourceBook.sheets[0]!, name }] });
const settle = async (): Promise<void> => { await new Promise<void>(resolve => { setTimeout(resolve, 0); }); };

describe("spreadsheet inspection generation", () => {
  it.each(["success", "failure"])("ignores an obsolete file read %s after a newer source is inspected", async outcome => {
    const { root, choose, inspectSpreadsheet, accept, acceptedSourceVisible } = importPanelFixture();
    const old = new File(["Value\nold\n"], "old.csv");
    const read = pendingResult<ArrayBuffer>(); vi.spyOn(old, "arrayBuffer").mockReturnValue(read.promise);
    choose(old);
    inspectSpreadsheet.mockResolvedValue(namedBook("Newest source"));
    choose(new File(["Value\nnew\n"], "new.csv"));
    await vi.waitFor(() => { expect(acceptedSourceVisible()).toBe(true); });
    const current = root.innerHTML;
    if (outcome === "success") read.resolve(new TextEncoder().encode("Value\nold\n").buffer);
    else read.reject(new Error("Obsolete read failure"));
    await settle();
    expect(root.innerHTML).toBe(current);
    expect(inspectSpreadsheet).toHaveBeenCalledTimes(1);
    click(root, "Accept types and import");
    expect(accept.mock.calls[0]?.[0].name).toBe("new.csv");
    expect(accept.mock.calls[0]?.[0].bytes).toEqual(new TextEncoder().encode("Value\nnew\n").buffer);
    await settle();
  });

  it.each(["success", "failure"])("ignores obsolete inspection %s without clearing the newer pending inspection", async outcome => {
    const { root, choose, inspectSpreadsheet, acceptedSourceVisible, accept } = importPanelFixture();
    const old = pendingResult<SourceWorkbook>(), latest = pendingResult<SourceWorkbook>();
    inspectSpreadsheet.mockReturnValueOnce(old.promise).mockReturnValueOnce(latest.promise);
    choose(new File(["Value\nold\n"], "old.csv"));
    await vi.waitFor(() => { expect(inspectSpreadsheet).toHaveBeenCalledTimes(1); });
    choose(new File(["Value\nnew\n"], "new.csv"));
    await vi.waitFor(() => { expect(inspectSpreadsheet).toHaveBeenCalledTimes(2); });
    if (outcome === "success") old.resolve(namedBook("Obsolete source"));
    else old.reject(new Error("Obsolete inspection failure"));
    await settle();
    expect(root.querySelector("fieldset")?.disabled).toBe(true);
    expect(acceptedSourceVisible()).toBe(false);
    expect(root.textContent).not.toContain("Obsolete");
    latest.resolve(namedBook("Latest source"));
    await vi.waitFor(() => { expect(root.textContent).toContain("Latest source"); });
    expect(root.querySelector("fieldset")?.disabled).toBe(false);
    click(root, "Accept types and import");
    expect(accept.mock.calls[0]?.[0].name).toBe("new.csv");
    await settle();
  });

  it.each(["success", "failure"])("keeps the newest source/options pair when older option inspection returns %s", async outcome => {
    const { root, choose, inspectSpreadsheet, accept, acceptedSourceVisible } = importPanelFixture();
    choose(new File(["Value\nsource\n"], "source.csv"));
    await vi.waitFor(() => { expect(acceptedSourceVisible()).toBe(true); });
    const old = pendingResult<SourceWorkbook>();
    inspectSpreadsheet.mockReturnValueOnce(old.promise).mockResolvedValueOnce(namedBook("Tab interpretation"));
    const comma = control(root, "CSV delimiter"); comma.value = ";"; comma.dispatchEvent(new Event("change"));
    const tab = control(root, "CSV delimiter"); tab.value = "\t"; tab.dispatchEvent(new Event("change"));
    await vi.waitFor(() => { expect(root.textContent).toContain("Tab interpretation"); });
    const current = root.innerHTML;
    if (outcome === "success") old.resolve(namedBook("Semicolon interpretation"));
    else old.reject(new Error("Obsolete option failure"));
    await settle();
    expect(root.innerHTML).toBe(current);
    expect(inspectSpreadsheet.mock.calls[1]?.[2]).toEqual({ delimiter: ";", header: true });
    expect(inspectSpreadsheet.mock.calls[2]?.[2]).toEqual({ delimiter: "\t", header: true });
    click(root, "Accept types and import");
    expect(accept.mock.calls[0]?.[1]).toEqual({ delimiter: "\t", header: true });
    await settle();
  });

  it.each(["clear", "invalid"])("invalidates in-flight inspection when the next selection is %s", async action => {
    const { root, choose, inspectSpreadsheet, acceptedSourceVisible, accept } = importPanelFixture();
    const old = pendingResult<SourceWorkbook>(); inspectSpreadsheet.mockReturnValueOnce(old.promise);
    choose(new File(["Value\nold\n"], "old.csv"));
    await vi.waitFor(() => { expect(inspectSpreadsheet).toHaveBeenCalledTimes(1); });
    choose(action === "clear" ? null : new File(["invalid"], "invalid.txt"));
    old.resolve(namedBook("Obsolete source"));
    await settle();
    expect(acceptedSourceVisible()).toBe(false);
    expect(root.textContent).not.toContain("Obsolete source");
    expect(root.querySelector("fieldset")?.disabled).toBe(false);
    expect(accept).not.toHaveBeenCalled();
  });

  it("refuses a detached old accept button after a newer inspection succeeds", async () => {
    const { root, choose, inspectSpreadsheet, acceptedSourceVisible, accept } = importPanelFixture();
    choose(new File(["Value\nold\n"], "old.csv"));
    await vi.waitFor(() => { expect(acceptedSourceVisible()).toBe(true); });
    const oldAccept = [...root.querySelectorAll("button")].find(button => button.textContent === "Accept types and import")!;
    inspectSpreadsheet.mockResolvedValueOnce(namedBook("Latest source"));
    choose(new File(["Value\nnew\n"], "new.csv"));
    await vi.waitFor(() => { expect(root.textContent).toContain("Latest source"); });
    oldAccept.click();
    expect(accept).not.toHaveBeenCalled();
    click(root, "Accept types and import");
    expect(accept).toHaveBeenCalledTimes(1);
    expect(accept.mock.calls[0]?.[0].name).toBe("new.csv");
    await settle();
  });
});

describe("late inspection and acceptance capture", () => {
  it.each(["success", "failure"])("ignores source A inspection %s after source B has already completed", async outcome => {
    const { root, choose, inspectSpreadsheet, accept } = importPanelFixture();
    const old = pendingResult<SourceWorkbook>();
    inspectSpreadsheet.mockReturnValueOnce(old.promise).mockResolvedValueOnce(namedBook("Source B"));
    choose(new File(["Value\nA\n"], "A.csv"));
    await vi.waitFor(() => { expect(inspectSpreadsheet).toHaveBeenCalledTimes(1); });
    choose(new File(["Value\nB\n"], "B.csv"));
    await vi.waitFor(() => { expect(root.textContent).toContain("Source B"); });
    const current = root.innerHTML;
    if (outcome === "success") old.resolve(namedBook("Source A"));
    else old.reject(new Error("Late A failure"));
    await settle();
    expect(root.innerHTML).toBe(current);
    click(root, "Accept types and import");
    expect(accept.mock.calls[0]?.[0]).toMatchObject({ name: "B.csv", bytes: new TextEncoder().encode("Value\nB\n").buffer });
    await settle();
  });

  it("holds the explicitly accepted source/options/selection fixed until acceptance settles", async () => {
    const { root, choose, accept, acceptedSourceVisible, inspectSpreadsheet } = importPanelFixture();
    choose(new File(["Value\nA\n"], "A.csv"));
    await vi.waitFor(() => { expect(acceptedSourceVisible()).toBe(true); });
    const acceptance = pendingResult<never>(); accept.mockReturnValueOnce(acceptance.promise);
    click(root, "Accept types and import");
    choose(new File(["Value\nB\n"], "B.csv"));
    const delimiter = control(root, "CSV delimiter"); delimiter.value = ";"; delimiter.dispatchEvent(new Event("change"));
    await settle();
    expect(inspectSpreadsheet).toHaveBeenCalledTimes(1);
    expect(accept).toHaveBeenCalledTimes(1);
    expect(accept.mock.calls[0]?.[0].name).toBe("A.csv");
    expect(accept.mock.calls[0]?.[1]).toEqual({ delimiter: ",", header: true });
    expect(accept.mock.calls[0]?.[2]).toEqual({ column_types: [["text"]], extra_columns: [[]] });
    expect(root.querySelector("fieldset")?.disabled).toBe(true);
    acceptance.reject(new Error("Import was rejected"));
    await vi.waitFor(() => { expect(root.textContent).toContain("Import was rejected"); });
    expect(root.querySelector("fieldset")?.disabled).toBe(false);
    expect(acceptedSourceVisible()).toBe(true);
  });
});
