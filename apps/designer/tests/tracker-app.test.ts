import { afterEach, expect, it, vi } from "vitest";
import { mountDesigner, type MountedDesigner } from "../src/designer-app.ts";
import type { DesignerClient } from "../src/runtime/client.ts";
import type { DesignerProjectHost } from "../src/host/browser-project-host.ts";
import type { TableProjection } from "../src/runtime/protocol.ts";
let mounted: MountedDesigner | undefined;
afterEach(() => { mounted?.destroy(); document.body.replaceChildren(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });
function fixture(): TableProjection {
    return { tracker_profile: true, revision: "resident/0", collection: { id: "tracker", key: "tracker", entity_count: 1 }, columns: [{ id: "task", key: "task", field_type: "Text" }, { id: "estimate", key: "estimate", field_type: "Number" }, { id: "done", key: "done", field_type: "Boolean", dropdown_options: ["true", "false"] }], rows: [{ id: "item", key: "item", fields: [{ target: { entity: "item", field: "task" }, address: "item.task", stored: { kind: "text", value: "Accepted work" }, formula: null, calculated: null, diagnostics: [], editable_scalar: "text" }] }] };
}
async function setup(original = "Accepted work", secondRow = false, stock = true) {
    let table = fixture();
    if (!stock) { delete table.tracker_profile; table.collection.id = "ordinary"; }
    const field = table.rows[0]?.fields[0];
    if (field) field.stored = {kind: "text", value: original};
    if (secondRow && table.rows[0]) {
        const row = structuredClone(table.rows[0]);
        row.id = "other"; row.key = "other";
        row.fields = row.fields.map(field => ({...field, target: {...field.target, entity: "other"}}));
        table.rows.push(row);
        table.collection.entity_count = 2;
    }
    const queryTable = vi.fn(async () => structuredClone(table));
    const trackerCommand = vi.fn(async () => {
        const base = table.revision;
        table = { ...table, revision: "resident/1" };
        return { base_revision: base, resulting_revision: table.revision, entities: ["item"], fields: [], affected_calculations: [] };
    });
    const exportProject = vi.fn(async (revision: string) => ({ revision, bytes: new ArrayBuffer(1) }));
    const publish = vi.fn(async () => { });
    const client: DesignerClient = {
        bootstrap: async () => ({ title: "Driver Tracker", revision: table.revision, default_collection: "tracker", collections: [table.collection] }),
        queryTable, trackerCommand,
        openProject: vi.fn(), exportProject, closeProject: vi.fn(), queryFields: vi.fn(), editNumber: vi.fn(), editText: vi.fn(), editBoolean: vi.fn(), editDate: vi.fn(), close: vi.fn(),
    };
    const host: DesignerProjectHost = { list: async () => [], read: vi.fn(), publish };
    const root = document.createElement("div");
    document.body.append(root);
    mounted = mountDesigner(root, client, host);
    await mounted.ready;
    return { root, exportProject, publish, queryTable, trackerCommand };
}
const click = (root: HTMLElement, selector: string): void => { const button = root.querySelector<HTMLButtonElement>(selector); expect(button).not.toBeNull(); button?.click(); };
it("accepted publication stays saveable when its projection refresh fails and can retry", async () => {
    const { root, exportProject, publish, queryTable, trackerCommand } = await setup();
    queryTable.mockRejectedValueOnce(new Error("temporary read failure"));
    click(root, '[data-tracker="append"]');
    await vi.waitFor(() => { expect(root.textContent).toContain("Edit published; refresh incomplete"); });
    expect(trackerCommand).toHaveBeenCalledTimes(1);
    expect(root.querySelector('[data-testid="revision"]')?.textContent).toBe("resident/1");
    expect(root.querySelector('[data-testid="durability"]')?.getAttribute("data-dirty")).toBe("true");
    vi.stubGlobal("prompt", vi.fn(() => "recovery.roproj"));
    click(root, "[data-save-project]");
    await vi.waitFor(() => { expect(publish).toHaveBeenCalledOnce(); });
    expect(exportProject).toHaveBeenCalledWith("resident/1");
    await vi.waitFor(() => { expect(root.textContent).toContain("Save As complete"); });
    click(root, "[data-tracker-refresh]");
    await vi.waitFor(() => { expect(root.querySelector("[data-tracker-refresh]")).toBeNull(); });
    expect(trackerCommand).toHaveBeenCalledTimes(1);
    expect(root.querySelector<HTMLButtonElement>('[data-tracker="undo"]')?.disabled).toBe(false);
});
it("a pending tracker draft cannot be silently saved or retargeted by filtering", async () => {
    const { root, publish, trackerCommand } = await setup();
    const editor = root.querySelector<HTMLInputElement>('[aria-label="Cell value"]');
    if (!editor)
        throw new Error("No editor");
    editor.value = "Unapplied draft";
    editor.dispatchEvent(new Event("input", { bubbles: true }));
    const prompt = vi.fn();
    vi.stubGlobal("prompt", prompt);
    click(root, "[data-save-project]");
    expect(publish).not.toHaveBeenCalled();
    expect(prompt).not.toHaveBeenCalled();
    expect(root.textContent).toContain("Apply or cancel the cell draft");
    const filter = root.querySelector<HTMLInputElement>("[data-tracker-filter]");
    if (!filter)
        throw new Error("No filter");
    filter.value = "does not match";
    filter.dispatchEvent(new Event("change", { bubbles: true }));
    expect(root.querySelector<HTMLInputElement>('[aria-label="Cell value"]')?.value).toBe("Unapplied draft");
    expect(root.querySelector("[role=gridcell]")?.textContent).toBe("Accepted work");
    expect(trackerCommand).not.toHaveBeenCalled();
});


it("tracker Text editor preserves multiline values and rejects silent CRLF normalization", async () => {
    const original = "\nFirst\r\nSecond";
    const {root, trackerCommand} = await setup(original);
    const editor = root.querySelector<HTMLTextAreaElement>('textarea[aria-label="Cell value"]');
    expect(editor).not.toBeNull();
    if (!editor) throw new Error("No text editor");
    expect(editor.value).toBe("\nFirst\nSecond");
    root.querySelector<HTMLFormElement>("[data-tracker-edit]")?.dispatchEvent(new Event("submit", {bubbles: true, cancelable: true}));
    await vi.waitFor(() => { expect(trackerCommand).toHaveBeenCalledOnce(); });
    expect(trackerCommand.mock.calls[0]).toEqual([expect.objectContaining({rows: [[original]]})]);
    await vi.waitFor(() => { expect(root.querySelector<HTMLButtonElement>('[data-tracker="undo"]')?.disabled).toBe(false); });
    const changedEditor = root.querySelector<HTMLTextAreaElement>('textarea[aria-label="Cell value"]');
    if (!changedEditor) throw new Error("No text editor");
    changedEditor.value = "Modified\nSecond";
    changedEditor.dispatchEvent(new Event("input", {bubbles: true}));
    root.querySelector<HTMLFormElement>("[data-tracker-edit]")?.dispatchEvent(new Event("submit", {bubbles: true, cancelable: true}));
    await vi.waitFor(() => { expect(root.textContent).toContain("original bytes remain unchanged"); });
    expect(trackerCommand).toHaveBeenCalledTimes(1);
    expect(root.querySelector<HTMLTextAreaElement>('[aria-label="Cell value"]')?.value).toBe("Modified\nSecond");
});


it("keyboard navigation cannot retarget a pending draft to another row", async () => {
    const {root, trackerCommand} = await setup("Accepted work", true);
    const editor = root.querySelector<HTMLTextAreaElement>('[aria-label="Cell value"]');
    if (!editor) throw new Error("No editor");
    editor.value = "Draft for first row";
    editor.dispatchEvent(new Event("input", {bubbles: true}));
    const firstCell = root.querySelector<HTMLElement>('[data-row="0"][data-col="0"]');
    firstCell?.focus();
    firstCell?.dispatchEvent(new KeyboardEvent("keydown", {key: "ArrowDown", bubbles: true, cancelable: true}));
    root.querySelector<HTMLFormElement>("[data-tracker-edit]")?.dispatchEvent(new Event("submit", {bubbles: true, cancelable: true}));
    await vi.waitFor(() => { expect(trackerCommand).toHaveBeenCalledOnce(); });
    expect(trackerCommand.mock.calls[0]).toEqual([expect.objectContaining({start_entity: "item", rows: [["Draft for first row"]]})]);
});


it("an ordinary collection named tracker keeps its generic scalar editor", async () => {
    const {root, trackerCommand} = await setup("Ordinary text", false, false);
    expect(root.querySelector('[aria-label="Driver tracker"]')).toBeNull();
    expect(root.querySelector('[data-edit-form][data-edit-kind="text"] textarea')).not.toBeNull();
    expect(trackerCommand).not.toHaveBeenCalled();
});

it("row moves and their undo keep selection attached to the same entity", async () => {
    const {root, trackerCommand} = await setup("First", true);
    click(root, '[data-tracker="down"]');
    expect(root.querySelector('[data-row="1"][data-col="0"]')?.getAttribute("aria-selected")).toBe("true");
    click(root, '[data-tracker="undo"]');
    expect(root.querySelector('[data-row="0"][data-col="0"]')?.getAttribute("aria-selected")).toBe("true");
    click(root, '[data-tracker="redo"]');
    expect(root.querySelector('[data-row="1"][data-col="0"]')?.getAttribute("aria-selected")).toBe("true");
    const editor = root.querySelector<HTMLTextAreaElement>('[aria-label="Cell value"]');
    if (!editor) throw new Error("No editor");
    editor.value = "Moved first row";
    editor.dispatchEvent(new Event("input", {bubbles: true}));
    root.querySelector<HTMLFormElement>("[data-tracker-edit]")?.dispatchEvent(new Event("submit", {bubbles: true, cancelable: true}));
    await vi.waitFor(() => { expect(trackerCommand).toHaveBeenCalledOnce(); });
    expect(trackerCommand.mock.calls[0]).toEqual([expect.objectContaining({start_entity: "item", rows: [["Moved first row"]]})]);
});
