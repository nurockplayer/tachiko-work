import { expect, test, type Page } from "@playwright/test";

// Steward-owned Issue #443 seed: an ordinary native-table journey, without
// replacing the Worker/runtime or manufacturing a publication response.
const shortcut = process.platform === "darwin" ? "Meta" : "Control";
const rows = "0012\t3\ttrue\t2024-02-29\nノート\t0\tfalse\t2026-09-13\n紙\t-2\ttrue\t2026-01-01";

test.use({ permissions: ["clipboard-read", "clipboard-write"] });

async function inventory(page: Page): Promise<void> {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Moonfall Balance" })).toBeVisible();
  await page.getByRole("button", { name: "New Table", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "New table", exact: true });
  await dialog.getByLabel("Table name", { exact: true }).fill("Inventory");
  for (const [index, name, type] of [
    [0, "item", "Text"], [1, "quantity", "Number"],
    [2, "active", "Boolean"], [3, "received", "Date"],
  ] as const) {
    if (index > 0) await dialog.getByRole("button", { name: "Add column", exact: true }).click();
    await dialog.getByLabel("Column name", { exact: true }).nth(index).fill(name);
    await dialog.getByLabel("Column type", { exact: true }).nth(index).selectOption({ label: type });
  }
  await dialog.getByRole("button", { name: "Create table", exact: true }).click();
  await expect(page.getByRole("grid", { name: "Inventory cells", exact: true })).toBeVisible();
  await page.getByRole("gridcell", { name: "Paste rows here, or choose Append row." }).click();
  await page.evaluate(text => navigator.clipboard.writeText(text), rows);
  await page.keyboard.press(`${shortcut}+V`);
  await expect(page.getByRole("gridcell", { name: "0012", exact: true })).toBeVisible();
  await expect(page.getByRole("gridcell", { name: "ノート", exact: true })).toBeVisible();
  await expect(page.locator("[data-generic-cell]")).toHaveCount(12);
}

async function cells(page: Page) {
  return page.locator("[data-generic-cell]").evaluateAll(elements => elements.map(element => ({
    entity: element.getAttribute("data-generic-entity"),
    field: element.getAttribute("data-generic-field"),
    value: element.getAttribute("data-generic-value"),
  })).sort((a, b) => `${a.entity ?? ""}/${a.field ?? ""}`.localeCompare(`${b.entity ?? ""}/${b.field ?? ""}`)));
}

async function addRowDialog(page: Page, quantity = "5") {
  const action = page.getByRole("button", { name: "Add row", exact: true });
  // Missing product control is evidence only after populated Inventory succeeds.
  await expect(action).toBeVisible();
  await action.click();
  const dialog = page.getByRole("dialog", { name: "Add row", exact: true });
  await dialog.getByLabel("item", { exact: true }).fill("封筒");
  await dialog.getByLabel("quantity", { exact: true }).fill(quantity);
  await dialog.getByLabel("active", { exact: true }).selectOption("true");
  await dialog.getByLabel("received", { exact: true }).fill("2026-09-22");
  await dialog.getByLabel("Use these values for the new row", { exact: true }).check();
  return dialog;
}

async function saveAs(page: Page, name: string): Promise<void> {
  page.once("dialog", dialog => dialog.accept(name));
  await page.getByRole("button", { name: "Save As", exact: true }).click();
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "false");
}

test("Inventory row lifecycle preserves surviving identities through history and reopen", async ({ page }) => {
  await inventory(page);
  const original = await cells(page);
  const oldIds = new Set(original.map(cell => cell.entity));
  expect(oldIds.size).toBe(3);
  const beforeAddRevision = await page.getByTestId("revision").textContent();
  const dialog = await addRowDialog(page);
  await dialog.getByRole("button", { name: "Add row", exact: true }).click();
  await expect(page.getByRole("gridcell", { name: "封筒", exact: true })).toBeVisible();
  await expect(page.locator("[data-generic-cell]")).toHaveCount(16);
  const afterAdd = await cells(page);
  expect(afterAdd.filter(cell => oldIds.has(cell.entity))).toEqual(original);
  const inserted = afterAdd.filter(cell => !oldIds.has(cell.entity));
  expect(inserted).toHaveLength(4);
  expect(new Set(inserted.map(cell => cell.entity)).size).toBe(1);
  expect(inserted.every(cell => cell.entity && cell.field)).toBe(true);
  expect(new Set(inserted.map(cell => cell.field))).toEqual(new Set(original.map(cell => cell.field)));
  expect(inserted.map(cell => cell.value).sort()).toEqual(["2026-09-22", "5", "true", "封筒"].sort());
  await expect(page.getByTestId("revision")).not.toHaveText(beforeAddRevision ?? "");

  const existingRow = page.getByRole("row").filter({ has: page.getByRole("gridcell", { name: "ノート", exact: true }) });
  const newRow = page.getByRole("row").filter({ has: page.getByRole("gridcell", { name: "封筒", exact: true }) });
  await existingRow.getByRole("checkbox", { name: /^Select row / }).check();
  await newRow.getByRole("checkbox", { name: /^Select row / }).check();
  const removedIds = new Set(afterAdd.filter(cell => cell.value === "ノート" || cell.value === "封筒").map(cell => cell.entity));
  expect(removedIds.size).toBe(2);
  const kept = afterAdd.filter(cell => !removedIds.has(cell.entity));
  await page.getByRole("button", { name: "Remove selected rows", exact: true }).click();
  const remove = page.getByRole("dialog", { name: "Remove selected rows", exact: true });
  await expect(remove).toContainText(/2 rows/);
  await expect(remove).toContainText(/delet|lost|loss/i);
  await remove.getByRole("button", { name: "Remove rows", exact: true }).click();
  await expect.poll(() => cells(page)).toEqual(kept);
  const removedRevision = await page.getByTestId("revision").textContent();

  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect.poll(() => cells(page)).toEqual(afterAdd);
  await expect(page.getByTestId("revision")).not.toHaveText(removedRevision ?? "");
  await page.getByRole("button", { name: "Redo", exact: true }).click();
  await expect.poll(() => cells(page)).toEqual(kept);
  await expect(page.getByTestId("revision")).not.toHaveText(removedRevision ?? "");
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect.poll(() => cells(page)).toEqual(afterAdd);

  await saveAs(page, "row-lifecycle.roproj");
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page.getByLabel("Saved project", { exact: true }).selectOption("row-lifecycle.roproj");
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect.poll(() => cells(page)).toEqual(afterAdd);
});

test("invalid row preserves saved state, history and the user's draft", async ({ page }) => {
  await inventory(page);
  await saveAs(page, "before-row-refusal.roproj");
  const before = await cells(page);
  const revision = await page.getByTestId("revision").textContent();
  const history = await page.getByRole("region", { name: "Session history", exact: true }).textContent();
  const dialog = await addRowDialog(page, "NaN");
  await dialog.getByRole("button", { name: "Add row", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(/invalid|finite|rejected/i);
  await expect(dialog).toBeVisible();
  await expect(dialog.getByLabel("item", { exact: true })).toHaveValue("封筒");
  await expect(dialog.getByLabel("quantity", { exact: true })).toHaveValue("NaN");
  expect(await cells(page)).toEqual(before);
  await expect(page.getByTestId("revision")).toHaveText(revision ?? "");
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "false");
  await expect(page.getByRole("region", { name: "Session history", exact: true })).toHaveText(history ?? "");
});


test("an explicitly acknowledged blank Text value is a complete native row", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await inventory(page);
  const before = await cells(page);
  const ids = new Set(before.map(cell => cell.entity));
  const dialog = await addRowDialog(page);
  await dialog.getByLabel("item", { exact: true }).fill("");
  await dialog.getByLabel("active", { exact: true }).selectOption("false");
  await dialog.getByRole("button", { name: "Add row", exact: true }).focus();
  await page.keyboard.press("Enter");
  await expect(page.locator("[data-generic-cell]")).toHaveCount(16);
  const after = await cells(page);
  expect(after.filter(cell => ids.has(cell.entity))).toEqual(before);
  expect(after.filter(cell => !ids.has(cell.entity)).map(cell => cell.value).sort()).toEqual(
    ["", "5", "false", "2026-09-22"].sort(),
  );
});
