import { expect, test, type Page } from "@playwright/test";

const shortcut = process.platform === "darwin" ? "Meta" : "Control";
const inventoryRows = "0012\t3\ttrue\t2024-02-29\nノート\t0\tfalse\t2026-09-13\n紙\t-2\ttrue\t2026-01-01";

test.use({ permissions: ["clipboard-read", "clipboard-write"] });

async function paste(page: Page, text: string): Promise<void> {
  await page.evaluate(value => navigator.clipboard.writeText(value), text);
  await page.keyboard.press(`${shortcut}+V`);
}

async function createInventory(page: Page): Promise<void> {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Moonfall Balance" })).toBeVisible();
  const newTable = page.getByRole("button", { name: "New Table", exact: true });
  await expect(newTable).toBeVisible();
  await newTable.click();
  const dialog = page.getByRole("dialog", { name: "New table", exact: true });
  await expect(dialog).toBeVisible();
  await dialog.getByLabel("Table name", { exact: true }).fill("Inventory");
  for (const [index, name, type] of [
    [0, "item", "Text"],
    [1, "quantity", "Number"],
    [2, "active", "Boolean"],
    [3, "received", "Date"],
  ] as const) {
    if (index > 0) await dialog.getByRole("button", { name: "Add column", exact: true }).click();
    await dialog.getByLabel("Column name", { exact: true }).nth(index).fill(name);
    await dialog.getByLabel("Column type", { exact: true }).nth(index).selectOption({ label: type });
  }
  await dialog.getByRole("button", { name: "Create table", exact: true }).click();
  await expect(page.getByRole("grid", { name: "Inventory cells", exact: true })).toBeVisible();
}

test("Driver creates Inventory, pastes typed rows, saves, reopens, and continues the same table", async ({ page }) => {
  await createInventory(page);
  const headers = page.getByRole("columnheader");
  await expect(headers).toHaveText(["item", "quantity", "active", "received"]);
  await page.getByRole("gridcell", { name: "Paste rows here, or choose Append row." }).click();
  await paste(page, inventoryRows);
  await expect(page.getByRole("gridcell", { name: "0012", exact: true })).toBeVisible();
  await expect(page.getByRole("gridcell", { name: "2024-02-29", exact: true })).toBeVisible();
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "true");
  page.once("dialog", dialog => dialog.accept("inventory.roproj"));
  await page.getByRole("button", { name: "Save As", exact: true }).click();
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "false");
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page.getByLabel("Saved project", { exact: true }).selectOption("inventory.roproj");
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(page.getByRole("gridcell", { name: "ノート", exact: true })).toBeVisible();
  await page.getByRole("gridcell", { name: "0012", exact: true }).click();
  await page.getByLabel("Cell value", { exact: true }).fill("0013");
  await page.getByRole("button", { name: "Apply to selection", exact: true }).click();
  await expect(page.getByRole("gridcell", { name: "0013", exact: true })).toBeVisible();
});

test("invalid New Table candidates leave the current saved occurrence and durability state unchanged", async ({ page }) => {
  await page.goto("/");
  page.once("dialog", dialog => dialog.accept());
  await page.getByRole("button", { name: "New Tracker", exact: true }).click();
  page.once("dialog", dialog => dialog.accept("resident.roproj"));
  await page.getByRole("button", { name: "Save As", exact: true }).click();
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "false");
  const newTable = page.getByRole("button", { name: "New Table", exact: true });
  await expect(newTable).toBeVisible();
  await newTable.click();
  const dialog = page.getByRole("dialog", { name: "New table", exact: true });
  await dialog.getByLabel("Table name", { exact: true }).fill("Inventory");
  await dialog.getByLabel("Column name", { exact: true }).nth(0).fill("item");
  await dialog.getByRole("button", { name: "Add column", exact: true }).click();
  await dialog.getByLabel("Column name", { exact: true }).nth(1).fill("item");
  await dialog.getByRole("button", { name: "Create table", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(/duplicate|invalid/i);
  await expect(page.getByRole("grid", { name: "Tracker cells", exact: true })).toBeVisible();
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "false");
});

test("invalid typed paste and stale first edit do not publish into newly created Inventory", async ({ page }) => {
  await createInventory(page);
  await page.getByRole("gridcell", { name: "Paste rows here, or choose Append row." }).click();
  await paste(page, "0012\t3\ttrue\t2024-02-29");
  const revision = await page.getByTestId("revision").textContent();
  await paste(page, "valid\t3\ttrue\t2024-02-29\ninvalid\tNaN\tyes\t2026-02-30");
  await expect(page.getByRole("alert")).toContainText(/invalid|rejected/i);
  await expect(page.getByTestId("revision")).toHaveText(revision ?? "");
  await page.getByRole("gridcell", { name: "0012", exact: true }).click();
  await page.getByLabel("Cell value", { exact: true }).fill("0013");
  await page.getByRole("button", { name: "Apply to selection", exact: true }).click();
  await page.getByRole("gridcell", { name: "0013", exact: true }).click();
  await page.getByLabel("Cell value", { exact: true }).fill("must not publish");
  await page.getByRole("button", { name: "Apply to selection", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(/stale|refresh/i);
  await expect(page.getByRole("gridcell", { name: "0013", exact: true })).toBeVisible();
});
