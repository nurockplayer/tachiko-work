import { describe, expect, it, vi } from "vitest";

import { FindReplacePanel, findLiteralTextMatches } from "../src/find-replace-panel.ts";
import type { CleanupPreview } from "../src/runtime/interop-protocol.ts";
import type { FieldProjection, TableProjection } from "../src/runtime/protocol.ts";

const field = (entity: string, name: string, value: string, formula: { source: string } | null = null): FieldProjection => ({
  target: { entity, field: name }, address: `${entity}.${name}`, stored: { kind: "text", value }, formula, calculated: null, diagnostics: [], editable_scalar: "text",
});

const table = (): TableProjection => ({
  native_table_profile: true,
  revision: "resident/1",
  collection: { id: "find-table", key: "Find table", entity_count: 2 },
  columns: [{ id: "name", key: "name", field_type: "Text" }, { id: "note", key: "note", field_type: "Text" }, { id: "score", key: "score", field_type: "Number" }],
  rows: [
    { id: "ada", key: "ada", fields: [field("ada", "name", "Ada Ada"), field("ada", "note", "Ada"), { target: { entity: "ada", field: "score" }, address: "ada.score", stored: { kind: "number", value: 12 }, formula: null, calculated: null, diagnostics: [], editable_scalar: "number" }] },
    { id: "formula", key: "formula", fields: [field("formula", "name", "Ada", { source: "=\"Ada\"" }), field("formula", "note", "ada")] },
  ],
});

function click(root: HTMLElement, label: string): void {
  const button = [...root.querySelectorAll<HTMLButtonElement>("button")].find(candidate => candidate.textContent === label);
  if (!button) throw new Error(`Missing ${label}`);
  button.click();
}

describe("native Find / Replace target derivation", () => {
  it("uses stored case-sensitive literal Text targets and excludes formula and non-Text values", () => {
    const matches = findLiteralTextMatches(table(), new Set(["name", "note"]), "Ada");
    expect(matches.map(match => match.target)).toEqual([
      { entity: "ada", field: "name" },
      { entity: "ada", field: "note" },
    ]);
    expect(findLiteralTextMatches(table(), new Set(["name", "note"]), "ada")).toEqual([
      expect.objectContaining({ target: { entity: "formula", field: "note" } }),
    ]);
  });

  it("sends one preview for the complete stable scoped set and commits its opaque identity", async () => {
    const root = document.createElement("div");
    document.body.append(root);
    const preview: CleanupPreview = {
      preview_id: "opaque-preview", revision: "resident/1",
      changes: [{ target: { entity: "ada", field: "name" }, before: field("ada", "name", "Ada Ada"), after: { kind: "text", value: "Done Done" } }], removed_entities: [],
    };
    const request = vi.fn().mockResolvedValue(preview);
    const commit = vi.fn().mockResolvedValue(undefined);
    const panel = new FindReplacePanel({ preview: request, commit });
    panel.open(root, table(), false);
    for (const label of ["name", "note"]) {
      const checkbox = root.querySelector<HTMLInputElement>(`[aria-label="${label}"]`);
      if (!checkbox) throw new Error(`Missing ${label}`);
      checkbox.checked = true;
      checkbox.dispatchEvent(new Event("change", { bubbles: true }));
    }
    const find = root.querySelector<HTMLInputElement>("[aria-label='Find text']");
    const replacement = root.querySelector<HTMLInputElement>("[aria-label='Replace text']");
    if (!find || !replacement) throw new Error("Missing Find / Replace inputs");
    find.value = "Ada";
    find.dispatchEvent(new Event("input", { bubbles: true }));
    replacement.value = "Done";
    replacement.dispatchEvent(new Event("input", { bubbles: true }));
    click(root, "Preview replace all");
    await vi.waitFor(() => { expect(request).toHaveBeenCalledOnce(); });
    expect(request).toHaveBeenCalledWith({ kind: "replace", find: "Ada", replacement: "Done", fields: [
      { entity: "ada", field: "name" }, { entity: "ada", field: "note" },
    ] });
    await vi.waitFor(() => { expect(root.textContent).toContain("Replace all preview"); });
    click(root, "Commit replace all");
    await vi.waitFor(() => { expect(commit).toHaveBeenCalledWith(preview); });
  });

  it("invalidates an in-flight preview when the semantic revision changes", async () => {
    const root = document.createElement("div");
    document.body.append(root);
    let resolve!: (value: CleanupPreview) => void;
    const request = vi.fn().mockReturnValue(new Promise<CleanupPreview>(done => { resolve = done; }));
    const commit = vi.fn();
    const panel = new FindReplacePanel({ preview: request, commit });
    const first = table();
    panel.open(root, first, false);
    const checkbox = root.querySelector<HTMLInputElement>("[aria-label='name']");
    const find = root.querySelector<HTMLInputElement>("[aria-label='Find text']");
    const replacement = root.querySelector<HTMLInputElement>("[aria-label='Replace text']");
    if (!checkbox || !find || !replacement) throw new Error("Missing Find / Replace controls");
    checkbox.checked = true;
    checkbox.dispatchEvent(new Event("change", { bubbles: true }));
    find.value = "Ada";
    find.dispatchEvent(new Event("input", { bubbles: true }));
    replacement.value = "Done";
    replacement.dispatchEvent(new Event("input", { bubbles: true }));
    click(root, "Preview replace all");
    await vi.waitFor(() => { expect(request).toHaveBeenCalledOnce(); });
    panel.mount(root, { ...first, revision: "resident/2" }, false);
    resolve({ preview_id: "obsolete", revision: "resident/1", changes: [], removed_entities: [] });
    await new Promise<void>(done => { setTimeout(done, 0); });
    expect(root.querySelector<HTMLButtonElement>("[data-find-action='commit']")?.disabled).toBe(true);
    expect(commit).not.toHaveBeenCalled();
  });
});
