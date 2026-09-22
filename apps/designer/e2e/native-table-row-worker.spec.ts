import { expect, test, type Page } from "@playwright/test";

const shortcut = process.platform === "darwin" ? "Meta" : "Control";
const rows = "0012\t3\ttrue\t2024-02-29\nノート\t0\tfalse\t2026-09-13";

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
}

test("native row dialogs preserve focus and controls at narrow width", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await inventory(page);

  const add = page.getByRole("button", { name: "Add row", exact: true });
  await add.click();
  const addDialog = page.getByRole("dialog", { name: "Add row", exact: true });
  await expect(addDialog.getByLabel("item", { exact: true })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(add).toBeFocused();

  await page.getByRole("checkbox", { name: /^Select row / }).first().check();
  const remove = page.getByRole("button", { name: "Remove selected rows", exact: true });
  await remove.click();
  const removeDialog = page.getByRole("dialog", { name: "Remove selected rows", exact: true });
  await expect(removeDialog.getByRole("button", { name: "Remove rows", exact: true })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(remove).toBeFocused();
});
