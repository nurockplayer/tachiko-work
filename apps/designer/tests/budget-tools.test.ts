import { expect, it, vi } from "vitest";
import { hasBudgetToolsDraft, mountBudgetTools, type BudgetToolsDraft } from "../src/budget-tools.ts";
import type { TableProjection } from "../src/runtime/protocol.ts";

function table(id: string): TableProjection {
  return {
    revision: "resident/0", collection: { id, key: id, entity_count: 2 },
    columns: [{ id: "amount", key: "amount", field_type: "Number" }],
    rows: ["first", "second"].map((key, index) => ({ id: `${id}_${key}`, key,
      fields: [{ target: { entity: `${id}_${key}`, field: "amount" }, address: `${id}_${key}.amount`,
        stored: index ? { kind: "number" as const, value: 3 } : null,
        formula: index ? null : { source: "1 + 2" }, calculated: null, diagnostics: [], editable_scalar: index ? "number" : null }] })),
  };
}
function setup(draft: BudgetToolsDraft = {}) {
  const root = document.createElement("div");
  const updateFormula = vi.fn(async () => {}); const copyFormula = vi.fn(async () => {});
  const options = { tables: [table("budget"), table("rates")], currentCollection: "budget", disabled: false, draft, changed: vi.fn(), updateFormula, copyFormula };
  mountBudgetTools(root, options);
  const select = (label: string): HTMLSelectElement => root.querySelector<HTMLSelectElement>(`select[aria-label="${label}"]`)!;
  const click = (text: string): void => { Array.from(root.querySelectorAll("button")).find(button => button.textContent === text)!.click(); };
  return { root, options, select, click, updateFormula, copyFormula };
}
it("inserts cross-collection Rust addresses and publishes the selected numeric target", async () => {
  const { root, options, select, click, updateFormula } = setup();
  const target = select("Formula target"); target.selectedIndex = 1; target.dispatchEvent(new Event("change"));
  const reference = select("Insert reference from"); reference.selectedIndex = 2;
  click("Insert reference");
  expect(root.querySelector("textarea")!.value).toBe("[rates_first.amount]");
  expect(hasBudgetToolsDraft(options.draft)).toBe(true);
  click("Apply formula");
  expect(updateFormula).toHaveBeenCalledWith({ entity: "budget_second", field: "amount" }, "[rates_first.amount]");
  await vi.waitFor(() => { expect(hasBudgetToolsDraft(options.draft)).toBe(false); });
});
it("copies with exact selected destinations and reference intent", async () => {
  const { root, select, click, copyFormula, options } = setup();
  select("Copy destinations").options[1]!.selected = true;
  select("Fixed references").options[2]!.selected = true;
  root.querySelector<HTMLInputElement>('[aria-label="Relative columns"]')!.checked = false;
  select("Copy destinations").dispatchEvent(new Event("change"));
  expect(hasBudgetToolsDraft(options.draft)).toBe(true);
  click("Copy formula");
  expect(copyFormula).toHaveBeenCalledWith({ source: { entity: "budget_first", field: "amount" }, destinations: [{ entity: "budget_second", field: "amount" }], fixed_references: [{ entity: "rates_first", field: "amount" }], relative_rows: true, relative_columns: false });
  await vi.waitFor(() => { expect(hasBudgetToolsDraft(options.draft)).toBe(false); });
});
it("retains rejected drafts across remount and treats labels as text", async () => {
  const draft: BudgetToolsDraft = { target: { entity: "budget_second", field: "amount" }, source: "invalid(" };
  const { root, options, click } = setup(draft);
  options.updateFormula.mockRejectedValueOnce(new Error("Unbound formula"));
  click("Apply formula");
  await vi.waitFor(() => { expect(root.textContent).toContain("Unbound formula"); });
  expect(draft.source).toBe("invalid(");
  root.replaceChildren();
  options.tables[0]!.collection.key = "<img src=x onerror=alert(1)>";
  mountBudgetTools(root, options);
  expect(root.querySelector("textarea")!.value).toBe("invalid(");
  expect(root.querySelector("img")).toBeNull();
  expect(root.textContent).toContain("<Img Src=X Onerror=Alert(1)>");
});
it("does not retarget a draft when its target is unavailable in the current table", () => {
  const { select, click, updateFormula } = setup({ target: { entity: "removed", field: "amount" }, source: "1 + 9" });
  expect(select("Formula target").value).toBe("");
  click("Apply formula");
  expect(updateFormula).not.toHaveBeenCalled();
});
