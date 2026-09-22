import { expect, test, type Page } from "@playwright/test";

// Steward-owned Issue #444 seed: an ordinary native-table journey, without
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


async function saveAs(page: Page, name: string): Promise<void> {
  page.once("dialog", dialog => dialog.accept(name));
  await page.getByRole("button", { name: "Save As", exact: true }).click();
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "false");
}

async function savedBytes(page: Page, name: string): Promise<number[]> {
  return page.evaluate(projectName => new Promise<number[]>((resolve, reject) => {
    const open = indexedDB.open("tachiko-designer-projects");
    open.addEventListener("error", () => { reject(new Error("Cannot read saved project.")); });
    open.addEventListener("success", () => {
      const database = open.result;
      const transaction = database.transaction("projects", "readonly");
      const request = transaction.objectStore("projects").get(projectName);
      request.addEventListener("error", () => { reject(new Error("Cannot read project record.")); });
      request.addEventListener("success", () => {
        const record = request.result as { bytes: ArrayBuffer } | undefined;
        if (!record) { reject(new Error("Saved project is missing.")); return; }
        resolve([...new Uint8Array(record.bytes)]);
      });
      transaction.addEventListener("complete", () => { database.close(); });
      transaction.addEventListener("abort", () => { database.close(); reject(new Error("Read aborted.")); });
    });
  }), name);
}

const grid = (page: Page) => page.getByRole("grid", { name: "Inventory cells", exact: true });
const headers = (page: Page) => grid(page).getByRole("columnheader").filter({ hasNot: page.getByRole("checkbox") });
async function rowIds(page: Page): Promise<(string | null)[]> {
  return grid(page).locator("tbody tr").evaluateAll(elements => elements.map(row => row.querySelector("[data-generic-cell]")?.getAttribute("data-generic-entity") ?? null));
}

for (const width of [1280, 390]) {
  test(`native ordering preserves semantic bytes, history and reopen at ${String(width)}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 844 });
    await inventory(page);
    await saveAs(page, "ordered-inventory.roproj");
    const original = await cells(page);
    const originalRows = await rowIds(page);
    const originalBytes = await savedBytes(page, "ordered-inventory.roproj");
    const revision = await page.getByTestId("revision").textContent();
    expect(originalRows).toHaveLength(3);
    expect(new Set(originalRows).size).toBe(3);
    await expect(headers(page)).toHaveText(["item", "quantity", "active", "received"]);

    const left = page.getByRole("button", { name: "Move column left", exact: true });
    // A missing-control RED is qualified only after valid ordinary paste/save.
    await expect(left).toBeVisible();
    const selector = page.getByLabel("Column to change", { exact: true });
    await selector.selectOption({ label: "quantity" });
    const field = await selector.inputValue();
    await left.focus();
    await page.keyboard.press("Enter");
    await expect(headers(page)).toHaveText(["quantity", "item", "active", "received"]);
    await expect(selector).toHaveValue(field);
    await expect(selector).toBeFocused();
    await expect(left).toBeDisabled();
    expect(await cells(page)).toEqual(original);
    await expect(page.getByTestId("revision")).toHaveText(revision ?? "");
    await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "true");

    const target = grid(page).getByRole("row").filter({ has: page.getByRole("gridcell", { name: "ノート", exact: true }) });
    await target.getByRole("checkbox", { name: /^Select row / }).check();
    await page.getByRole("button", { name: "Move row up", exact: true }).focus();
    await page.keyboard.press("Enter");
    await expect.poll(() => rowIds(page)).toEqual([originalRows[1], originalRows[0], originalRows[2]]);
    await expect(target.getByRole("checkbox", { name: "Select row 1", exact: true })).toBeChecked();
    await expect(target.getByRole("checkbox", { name: "Select row 1", exact: true })).toBeFocused();
    await expect(page.getByRole("button", { name: "Move row up", exact: true })).toBeDisabled();
    expect(await cells(page)).toEqual(original);
    await expect(page.getByTestId("revision")).toHaveText(revision ?? "");

    await page.getByRole("button", { name: "Undo", exact: true }).click();
    await expect.poll(() => rowIds(page)).toEqual(originalRows);
    await expect(headers(page)).toHaveText(["quantity", "item", "active", "received"]);
    await page.getByRole("button", { name: "Undo", exact: true }).click();
    await expect(headers(page)).toHaveText(["item", "quantity", "active", "received"]);
    await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "false");
    await page.getByRole("button", { name: "Redo", exact: true }).click();
    await expect(headers(page)).toHaveText(["quantity", "item", "active", "received"]);
    await page.getByRole("button", { name: "Redo", exact: true }).click();
    await expect.poll(() => rowIds(page)).toEqual([originalRows[1], originalRows[0], originalRows[2]]);
    await expect(page.getByTestId("revision")).toHaveText(revision ?? "");
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "false");
    expect(await savedBytes(page, "ordered-inventory.roproj")).toEqual(originalBytes);
    await page.getByRole("button", { name: "Close", exact: true }).click();
    await page.getByLabel("Saved project", { exact: true }).selectOption("ordered-inventory.roproj");
    await page.getByRole("button", { name: "Open project", exact: true }).click();
    await expect(headers(page)).toHaveText(["quantity", "item", "active", "received"]);
    await expect.poll(() => rowIds(page)).toEqual([originalRows[1], originalRows[0], originalRows[2]]);
    expect(await cells(page)).toEqual(original);
  });
}

test("mixed native lifecycle appends new IDs and never retargets surviving ordered cells", async ({ page }) => {
  await inventory(page);
  const original = await cells(page);
  const originalRows = await rowIds(page);
  const left = page.getByRole("button", { name: "Move column left", exact: true });
  await expect(left).toBeVisible();
  await page.getByLabel("Column to change", { exact: true }).selectOption({ label: "quantity" });
  await left.click();
  await expect(headers(page)).toHaveText(["quantity", "item", "active", "received"]);
  const target = grid(page).getByRole("row").filter({ has: page.getByRole("gridcell", { name: "ノート", exact: true }) });
  await target.getByRole("checkbox", { name: /^Select row / }).check();
  await page.getByRole("button", { name: "Move row up", exact: true }).click();
  await expect.poll(() => rowIds(page)).toEqual([originalRows[1], originalRows[0], originalRows[2]]);

  await page.getByLabel("Column to change", { exact: true }).selectOption({ label: "quantity" });
  await page.getByRole("button", { name: "Rename column", exact: true }).click();
  const rename = page.getByRole("dialog", { name: "Rename column", exact: true });
  await rename.getByLabel("New column name", { exact: true }).fill("stock");
  await rename.getByRole("button", { name: "Rename column", exact: true }).click();
  await expect(headers(page)).toHaveText(["stock", "item", "active", "received"]);
  expect(await cells(page)).toEqual(original);

  await page.getByRole("button", { name: "Add row", exact: true }).click();
  const add = page.getByRole("dialog", { name: "Add row", exact: true });
  await add.getByLabel("item", { exact: true }).fill("封筒");
  await add.getByLabel("stock", { exact: true }).fill("5");
  await add.getByLabel("active", { exact: true }).selectOption("false");
  await add.getByLabel("received", { exact: true }).fill("2026-09-22");
  await add.getByLabel("Use these values for the new row", { exact: true }).check();
  await add.getByRole("button", { name: "Add row", exact: true }).click();
  await expect(page.getByRole("gridcell", { name: "封筒", exact: true })).toBeVisible();
  const expandedRows = await rowIds(page);
  expect(expandedRows.slice(0, 3)).toEqual([originalRows[1], originalRows[0], originalRows[2]]);
  expect(expandedRows[3]).toBeTruthy();
  expect(originalRows).not.toContain(expandedRows[3]);
  expect((await cells(page)).filter(cell => originalRows.includes(cell.entity))).toEqual(original);

  const removed = grid(page).getByRole("row").filter({ has: page.getByRole("gridcell", { name: "0012", exact: true }) });
  // Clear selection explicitly: semantic actions need not retain old selections.
  for (const checkbox of await grid(page).getByRole("checkbox", { name: /^Select row \d+$/ }).all()) await checkbox.uncheck();
  await removed.getByRole("checkbox", { name: /^Select row / }).check();
  await page.getByRole("button", { name: "Remove selected rows", exact: true }).click();
  const remove = page.getByRole("dialog", { name: "Remove selected rows", exact: true });
  await remove.getByRole("button", { name: "Remove rows", exact: true }).click();
  await expect.poll(() => rowIds(page)).toEqual([originalRows[1], originalRows[2], expandedRows[3]]);
  expect((await cells(page)).filter(cell => originalRows.includes(cell.entity))).toEqual(original.filter(cell => cell.entity !== originalRows[0]));
  await saveAs(page, "mixed-order.roproj");
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page.getByLabel("Saved project", { exact: true }).selectOption("mixed-order.roproj");
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(headers(page)).toHaveText(["stock", "item", "active", "received"]);
  await expect.poll(() => rowIds(page)).toEqual([originalRows[1], originalRows[2], expandedRows[3]]);
});
