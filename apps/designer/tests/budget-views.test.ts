import { describe, expect, it } from "vitest";
import { addBudgetView, defaultBudgetViews, deleteBudgetView, duplicateBudgetView, parseBudgetViews, renameBudgetView, reorderBudgetViews, type BudgetViews } from "../src/budget-views.ts";

const first = "00000000-0000-4000-8000-000000000001";
const second = "00000000-0000-4000-8000-000000000002";
const third = "00000000-0000-4000-8000-000000000003";
const collections = ["stable-items", "stable-summary"];
function fixture(): BudgetViews {
  return { version: 1, active: first, views: [
    { id: first, name: "本月計畫", collection: collections[0]! },
    { id: second, name: "Summary", collection: collections[1]! },
  ] };
}

describe("private Budget views", () => {
  it("round-trips independently and binds collections by stable identity", () => {
    const original = fixture();
    const parsed = parseBudgetViews(JSON.parse(JSON.stringify(original)), collections);
    expect(parsed).toEqual(original);
    parsed.views[0]!.name = "Changed";
    expect(original.views[0]!.name).toBe("本月計畫");
    expect(() => parseBudgetViews(original, ["renamed-label", collections[1]!])).toThrow("collection ID");
  });

  it.each([null, [], {}, { ...fixture(), version: 2 }, { ...fixture(), active: third },
    { ...fixture(), views: [] }, { ...fixture(), extra: true },
    { ...fixture(), views: Array.from({ length: 33 }, () => fixture().views[0]) },
    { ...fixture(), views: [fixture().views[0], fixture().views[0]] },
  ])("rejects malformed persisted state %#", value => {
    expect(() => parseBudgetViews(value, collections)).toThrow("Invalid Budget views");
  });

  it.each(["", "  ", "x".repeat(81), "line\nbreak", "bad\0name"])("rejects invalid names %j without mutation", name => {
    const original = fixture();
    expect(() => renameBudgetView(original, first, name)).toThrow();
    expect(original).toEqual(fixture());
    const malformed = fixture(); malformed.views[0]!.name = name;
    expect(() => parseBudgetViews(malformed, collections)).toThrow();
  });

  it("accepts 80 Unicode characters and rejects malformed IDs or unavailable collections", () => {
    expect(renameBudgetView(fixture(), first, "🗓".repeat(80)).views[0]!.name).toBe("🗓".repeat(80));
    for (const patch of [{ id: "not-a-uuid" }, { collection: "unknown" }, { surprise: true }]) {
      const input = fixture(); Object.assign(input.views[0]!, patch);
      expect(() => parseBudgetViews(input, collections)).toThrow();
    }
  });

  it("adds and duplicates equivalent views without changing their source or binding", () => {
    const original = fixture();
    const duplicate = duplicateBudgetView(original, first, third, "Next month view", collections);
    expect(duplicate.active).toBe(third);
    expect(duplicate.views[2]).toEqual({ id: third, name: "Next month view", collection: "stable-items" });
    expect(original).toEqual(fixture());
    expect(addBudgetView(original, { id: third, name: "More", collection: "stable-summary" }, collections).views).toHaveLength(3);
    expect(() => duplicateBudgetView(original, first, second, "Duplicate ID", collections)).toThrow();
    expect(() => duplicateBudgetView(original, third, third, "Missing source", collections)).toThrow();
  });

  it("reorders and renames presentation only, preserving active ID and all collection bindings", () => {
    const original = fixture();
    const next = reorderBudgetViews(renameBudgetView(original, first, "Renamed"), [second, first]);
    expect(next.active).toBe(first);
    expect(next.views).toEqual([original.views[1], { ...original.views[0], name: "Renamed" }]);
    expect(original).toEqual(fixture());
    for (const ids of [[first], [first, first], [first, third]]) expect(() => reorderBudgetViews(original, ids)).toThrow();
  });

  it("deletes only a view, selects a surviving neighbor, and refuses the final deletion", () => {
    const original = fixture();
    const next = deleteBudgetView(original, first);
    expect(next).toEqual({ version: 1, active: second, views: [original.views[1]] });
    expect(deleteBudgetView(original, second).active).toBe(first);
    expect(() => deleteBudgetView(next, second)).toThrow("at least one");
    expect(() => deleteBudgetView(original, third)).toThrow("unavailable");
    expect(original).toEqual(fixture());
  });

  it("creates persistable defaults and enforces the 32-view bound on additions", () => {
    const defaults = defaultBudgetViews(collections);
    expect(parseBudgetViews(defaults, collections)).toEqual(defaults);
    expect(defaults.views.map(view => view.collection)).toEqual(collections);
    expect(() => defaultBudgetViews([])).toThrow();
    expect(() => defaultBudgetViews(["same", "same"])).toThrow();
    const full = defaultBudgetViews(Array.from({ length: 32 }, (_, index) => `collection-${String(index)}`));
    expect(() => addBudgetView(full, { id: third, name: "Overflow", collection: full.views[0]!.collection }, full.views.map(view => view.collection))).toThrow();
  });
});
