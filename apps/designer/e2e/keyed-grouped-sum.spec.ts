import { fileURLToPath } from "node:url";

import { expect, test, type Locator, type Page } from "@playwright/test";

const CANARY_PROJECT = fileURLToPath(
  new URL("./fixtures/keyed-grouped-sum-v1.roproj", import.meta.url),
);

const groupedSummary = (page: Page) =>
  page.getByRole("region", { name: "Grouped summary", exact: true });

const groupedResult = (page: Page) =>
  page.getByRole("region", { name: "Grouped summary result", exact: true });

async function openCanary(page: Page): Promise<void> {
  await page.goto("/");
  await page.locator("[data-import-project]").setInputFiles(CANARY_PROJECT);
  const notice = page.locator(".notice").first();
  await expect(notice).toBeVisible();
  if (!(await notice.evaluate(element => element.classList.contains("success")))) {
    throw new Error(`Canary project was not admitted: ${await notice.innerText()}`);
  }
  await expect(notice).toContainText("Project opened");
  await expect(
    page.getByRole("heading", { name: "Keyed Grouped Sum Acceptance", exact: true }),
  ).toBeVisible();
  await expect(page.getByTestId("revision")).toContainText("resident/0");
}

async function applyField(page: Page, label: string, value: string): Promise<void> {
  const control = page.getByLabel(label, { exact: true });
  await control.fill(value);
  await control
    .locator("xpath=ancestor::form")
    .getByRole("button", { name: "Apply", exact: true })
    .click();
  await expect(page.locator(".notice.success")).toContainText("Publication complete");
}

async function expectGroups(
  result: Locator,
  hardware: string,
  services: string,
): Promise<void> {
  await expect(result).toContainText(new RegExp(`hardware\\s+${hardware}`));
  await expect(result).toContainText(new RegExp(`services\\s+${services}`));
}

async function saveAs(page: Page, name: string): Promise<void> {
  page.once("dialog", dialog => dialog.accept(name));
  await page.getByRole("button", { name: "Save As", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Save As complete");
}

async function reopen(page: Page, name: string): Promise<void> {
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page.getByLabel("Saved project", { exact: true }).selectOption(name);
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Project opened");
}

test("keyed grouped-sum canary is a valid ordinary two-table project", async ({ page }) => {
  await openCanary(page);

  await page.getByLabel("Collection", { exact: true }).selectOption("orders");
  await expect(page.getByRole("heading", { name: "Orders", exact: true })).toBeVisible();
  await expect(page.getByLabel("Product Code for Order A", { exact: true })).toHaveValue("P-100");
  await expect(page.getByLabel("Quantity for Order A", { exact: true })).toHaveValue("-1");
  await expect(page.getByLabel("Product Code for Order B", { exact: true })).toHaveValue("P-200");
  await expect(page.getByLabel("Quantity for Order C", { exact: true })).toHaveValue("3");

  await page.getByLabel("Collection", { exact: true }).selectOption("products");
  await expect(page.getByRole("heading", { name: "Products", exact: true })).toBeVisible();
  await expect(page.getByLabel("Product Code for Product A", { exact: true })).toHaveValue("P-100");
  await expect(page.getByLabel("Category for Product A", { exact: true })).toHaveValue("hardware");
  await expect(page.getByLabel("Price for Product B", { exact: true })).toHaveValue("5");
});

test("Driver explicitly upgrades, authors, saves, and invalidates a live keyed grouped sum", async ({
  page,
}) => {
  await openCanary(page);

  const panel = groupedSummary(page);
  await expect(panel).toBeVisible();

  await panel.getByLabel("Orders table", { exact: true }).selectOption({ label: "Orders" });
  await panel
    .getByLabel("Order lookup key", { exact: true })
    .selectOption({ label: "Product Code" });
  await panel
    .getByLabel("Order quantity", { exact: true })
    .selectOption({ label: "Quantity" });
  await panel.getByLabel("Products table", { exact: true }).selectOption({ label: "Products" });
  await panel
    .getByLabel("Product lookup key", { exact: true })
    .selectOption({ label: "Product Code" });
  await panel
    .getByLabel("Product category", { exact: true })
    .selectOption({ label: "Category" });
  await panel
    .getByLabel("Product price", { exact: true })
    .selectOption({ label: "Price" });

  page.once("dialog", async dialog => {
    expect(dialog.message()).toMatch(/format 2/i);
    expect(dialog.message()).toMatch(/upgrade|migrate/i);
    await dialog.accept();
  });
  await panel
    .getByRole("button", { name: "Create grouped summary", exact: true })
    .click();
  await expect(page.locator(".notice.success")).toContainText("Publication complete");

  let result = groupedResult(page);
  await expectGroups(result, "4", "10");

  await page.getByLabel("Collection", { exact: true }).selectOption("products");
  await applyField(page, "Price for Product A", "3");
  await expectGroups(result, "6", "10");

  await saveAs(page, "keyed-grouped-sum.roproj");
  await reopen(page, "keyed-grouped-sum.roproj");
  result = groupedResult(page);
  await expectGroups(result, "6", "10");

  await page.getByLabel("Collection", { exact: true }).selectOption("products");
  await applyField(page, "Product Code for Product B", "P-100");
  await expect(result).toContainText("lookup.ambiguous_key");
  await expect(result).not.toContainText(/hardware\s+6/);
  await expect(result).not.toContainText(/services\s+10/);

  await applyField(page, "Product Code for Product B", "P-300");
  await expect(result).toContainText("lookup.missing_key");
  await expect(result).not.toContainText(/hardware\s+6/);
  await expect(result).not.toContainText(/services\s+10/);
});
