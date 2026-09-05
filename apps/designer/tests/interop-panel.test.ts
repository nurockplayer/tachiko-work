import { describe, expect, it, vi } from "vitest";
import { mountCleanupPanel } from "../src/interop-panel.ts";
import type { CleanupOperation, CleanupPreview } from "../src/runtime/interop-protocol.ts";
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
