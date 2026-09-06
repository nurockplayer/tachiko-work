import { readFile } from "node:fs/promises";
import { expect, test, type Page } from "@playwright/test";

const cell = (page: Page, address: string) => page.locator(`[data-field="${address}"]`);
const tools = (page: Page) => page.getByRole("region", { name: "Budget formulas", exact: true });
const item = (row: string, field: string) => `Budget Items / ${row} / ${field}`;
const summary = (field: string) => `Budget Summary / Monthly Summary / ${field}`;

async function newBudget(page: Page): Promise<void> {
  await page.goto("/");
  page.once("dialog", dialog => dialog.accept());
  await page.getByRole("button", { name: "New Budget", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Monthly Budget", exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Budget Items", exact: true })).toBeVisible();
}

async function applyScalar(page: Page, label: string, value: string): Promise<void> {
  const control = page.getByLabel(label, { exact: true });
  await control.fill(value);
  await control.locator("xpath=ancestor::form").getByRole("button", { name: "Apply", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Publication complete");
}

async function formula(page: Page, target: string, parts: Array<{ reference: string } | string>): Promise<void> {
  const panel = tools(page);
  await panel.getByLabel("Formula target", { exact: true }).selectOption({ label: target });
  const source = panel.getByLabel("Formula source", { exact: true });
  await source.fill("");
  for (const part of parts) {
    if (typeof part === "string") {
      await source.press("End");
      await source.pressSequentially(part);
    } else {
      await panel.getByLabel("Insert reference from", { exact: true }).selectOption({ label: part.reference });
      await panel.getByRole("button", { name: "Insert reference", exact: true }).click();
    }
  }
  await panel.getByRole("button", { name: "Apply formula", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Publication complete");
}

async function namedAction(page: Page, action: string, name: string): Promise<void> {
  page.once("dialog", dialog => dialog.accept(name));
  await page.getByRole("button", { name: action, exact: true }).click();
}

async function saveAndReopen(page: Page, name: string): Promise<void> {
  await namedAction(page, "Save As", name);
  await expect(page.locator(".notice.success")).toContainText("Save As complete");
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page.getByLabel("Saved project", { exact: true }).selectOption(name);
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Project opened");
}

const exportReview = (page: Page) => page.getByRole("region", { name: "Export compatibility review", exact: true });

test("native Budget XLSX preserves cross-collection formula meaning after alias changes and reopen", async ({ page }, testInfo) => {
  await newBudget(page);
  await applyScalar(page, "Actual for Utilities", "200");
  await formula(page, item("Rent", "Variance"), [
    { reference: item("Rent", "Actual") },
    " + ",
    { reference: summary("Actual Total") },
  ]);
  await expect(cell(page, "rent.variance").locator("output")).toHaveText("2,600");

  await namedAction(page, "Add view", "Planning desk");
  await namedAction(page, "Duplicate view", "Review copy");
  await namedAction(page, "Rename view", "Approved plan");
  await page.getByRole("button", { name: "Move view up", exact: true }).click();
  await saveAndReopen(page, "native-budget-export.roproj");
  await expect(cell(page, "rent.variance").locator("output")).toHaveText("2,600");

  await page.getByRole("button", { name: "Export Budget XLSX", exact: true }).click();
  const review = exportReview(page);
  await expect(review).toBeVisible();
  await expect(review).toContainText(/formula/i);
  await expect(review).toContainText(/chart/i);
  await expect(review).toContainText(/view|alias/i);

  const downloading = page.waitForEvent("download");
  await page.getByRole("button", { name: "Acknowledge losses and download XLSX", exact: true }).click();
  const download = await downloading;
  const outputPath = testInfo.outputPath("native-budget.xlsx");
  await download.saveAs(outputPath);
  expect((await readFile(outputPath)).subarray(0, 4)).toEqual(Buffer.from([0x50, 0x4b, 0x03, 0x04]));
});

test("native Budget CSV is a selected-view calculated-values escape with losses disclosed first", async ({ page }, testInfo) => {
  await newBudget(page);
  await applyScalar(page, "Actual for Utilities", "200");
  await page.getByLabel("View", { exact: true }).selectOption({ label: "Budget Summary" });
  await expect(cell(page, "monthly_summary.actual_total").locator("output")).toHaveText("1,400");

  await page.getByRole("button", { name: "Export Budget CSV", exact: true }).click();
  const review = exportReview(page);
  await expect(review).toBeVisible();
  await expect(review).toContainText(/calculated|values/i);
  await expect(review).toContainText(/formula/i);
  await expect(review).toContainText(/collection|sheet/i);

  const downloading = page.waitForEvent("download");
  await page.getByRole("button", { name: "Acknowledge losses and download CSV", exact: true }).click();
  const download = await downloading;
  const outputPath = testInfo.outputPath("native-budget-summary.csv");
  await download.saveAs(outputPath);
  const csv = await readFile(outputPath, "utf8");
  expect(csv).toContain("1400");
  expect(csv).not.toMatch(/^PK/u);
});

test("a published Budget change invalidates an already prepared export review", async ({ page }) => {
  await newBudget(page);
  await page.getByRole("button", { name: "Export Budget XLSX", exact: true }).click();
  const review = exportReview(page);
  await expect(review).toBeVisible();
  const before = await page.getByTestId("revision").textContent();

  await applyScalar(page, "Actual for Utilities", "201");
  await expect(page.getByTestId("revision")).not.toHaveText(before ?? "");
  await expect(review).toBeHidden();
});
