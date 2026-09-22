import { expect, test, type Page } from "@playwright/test";

// Steward-owned Issue #442 seed: an ordinary native-table journey, without
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
  await expect(page.getByRole("columnheader")).toHaveText(["item", "quantity", "active", "received"]);
  await expect(page.locator("[data-generic-cell]")).toHaveCount(12);
}

async function cells(page: Page) {
  return page.locator("[data-generic-cell]").evaluateAll(elements => elements.map(element => ({
    entity: element.getAttribute("data-generic-entity"),
    field: element.getAttribute("data-generic-field"),
    value: element.getAttribute("data-generic-value"),
  })).sort((a, b) => `${a.entity}/${a.field}`.localeCompare(`${b.entity}/${b.field}`)));
}

async function addDialog(page: Page, name: string, type: string, value: string) {
  const action = page.getByRole("button", { name: "Add column", exact: true });
  // A baseline failure here is meaningful only after inventory() succeeded.
  await expect(action).toBeVisible();
  await action.click();
  const dialog = page.getByRole("dialog", { name: "Add column", exact: true });
  await dialog.getByLabel("Column name", { exact: true }).fill(name);
  await dialog.getByLabel("Column type", { exact: true }).selectOption({ label: type });
  await dialog.getByLabel("Value for existing rows", { exact: true }).fill(value);
  await dialog.getByLabel("Use this value for every existing row", { exact: true }).check();
  return dialog;
}

async function saveAs(page: Page, name: string): Promise<void> {
  page.once("dialog", dialog => dialog.accept(name));
  await page.getByRole("button", { name: "Save As", exact: true }).click();
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "false");
}

test("Inventory column lifecycle preserves identities through history and reopen", async ({ page }) => {
  await inventory(page);
  const original = await cells(page);
  expect(original.every(cell => cell.entity && cell.field)).toBe(true);
  const initialRevision = await page.getByTestId("revision").textContent();
  const dialog = await addDialog(page, "supplier", "Text", "共通 supplier");
  await dialog.getByRole("button", { name: "Add column", exact: true }).click();
  await expect(page.getByRole("columnheader", { name: "supplier", exact: true })).toBeVisible();
  await expect(page.locator("[data-generic-cell]")).toHaveCount(15);
  const added = (await cells(page)).filter(cell => !original.some(before => before.field === cell.field));
  expect(added).toHaveLength(3);
  expect(new Set(added.map(cell => cell.field)).size).toBe(1);
  expect(new Set(added.map(cell => cell.entity))).toEqual(new Set(original.map(cell => cell.entity)));
  expect(added.map(cell => cell.value)).toEqual(["共通 supplier", "共通 supplier", "共通 supplier"]);
  const supplierId = added[0]?.field;
  expect(supplierId).toBeTruthy();
  expect((await cells(page)).filter(cell => cell.field !== supplierId)).toEqual(original);
  await expect(page.getByTestId("revision")).not.toHaveText(initialRevision ?? "");

  await page.getByLabel("Column to change", { exact: true }).selectOption({ label: "supplier" });
  await page.getByRole("button", { name: "Rename column", exact: true }).click();
  const rename = page.getByRole("dialog", { name: "Rename column", exact: true });
  await rename.getByLabel("New column name", { exact: true }).fill("vendor");
  await rename.getByRole("button", { name: "Rename column", exact: true }).click();
  await expect(page.getByRole("columnheader", { name: "vendor", exact: true })).toBeVisible();
  expect((await cells(page)).filter(cell => cell.field === supplierId)).toEqual(added);

  await page.getByRole("gridcell", { name: "共通 supplier", exact: true }).first().click();
  await page.getByLabel("Cell value", { exact: true }).fill("京都 supplier");
  await page.getByRole("button", { name: "Apply to selection", exact: true }).click();
  await expect(page.getByRole("gridcell", { name: "京都 supplier", exact: true })).toBeVisible();
  const edited = await cells(page);
  const beforeRemoveRevision = await page.getByTestId("revision").textContent();
  await page.getByLabel("Column to change", { exact: true }).selectOption({ label: "vendor" });
  await page.getByRole("button", { name: "Remove column", exact: true }).click();
  const remove = page.getByRole("dialog", { name: "Remove column", exact: true });
  await expect(remove).toContainText("vendor");
  await remove.getByRole("button", { name: "Remove column", exact: true }).click();
  await expect(page.getByRole("columnheader", { name: "vendor", exact: true })).toHaveCount(0);
  await expect.poll(() => cells(page)).toEqual(original);
  const removedRevision = await page.getByTestId("revision").textContent();
  expect(removedRevision).not.toBe(beforeRemoveRevision);

  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect.poll(() => cells(page)).toEqual(edited);
  const inverseRevision = await page.getByTestId("revision").textContent();
  expect(inverseRevision).not.toBe(removedRevision);
  expect(inverseRevision).not.toBe(beforeRemoveRevision);
  await page.getByRole("button", { name: "Redo", exact: true }).click();
  await expect.poll(() => cells(page)).toEqual(original);
  await expect(page.getByTestId("revision")).not.toHaveText(removedRevision ?? "");
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect.poll(() => cells(page)).toEqual(edited);

  await saveAs(page, "column-lifecycle.roproj");
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page.getByLabel("Saved project", { exact: true }).selectOption("column-lifecycle.roproj");
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(page.getByRole("columnheader", { name: "vendor", exact: true })).toBeVisible();
  await expect.poll(() => cells(page)).toEqual(edited);
});

for (const candidate of [
  { name: "item", type: "Text", value: "duplicate" },
  { name: "cost", type: "Number", value: "NaN" },
]) {
  test(`refused column ${candidate.name} preserves saved values, revision and history`, async ({ page }) => {
    await inventory(page);
    await saveAs(page, "before-refusal.roproj");
    const before = await cells(page);
    const revision = await page.getByTestId("revision").textContent();
    const history = await page.getByRole("region", { name: "Session history", exact: true }).textContent();
    const dialog = await addDialog(page, candidate.name, candidate.type, candidate.value);
    await dialog.getByRole("button", { name: "Add column", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText(/duplicate|invalid|finite|rejected/i);
    expect(await cells(page)).toEqual(before);
    await expect(page.getByTestId("revision")).toHaveText(revision ?? "");
    await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "false");
    await expect(page.getByRole("region", { name: "Session history", exact: true })).toHaveText(history ?? "");
  });
}
