import { expect, test, type Page } from "@playwright/test";
import { fileURLToPath } from "node:url";
const fixture = (name: string): string => fileURLToPath(new URL(`../tests/fixtures/interop/${name}`, import.meta.url));
async function inspect(page: Page, name: string): Promise<void> {
  await page.goto("/");
  await page.getByText("Import CSV / XLSX", {exact: true}).click();
  await page.getByLabel("Spreadsheet file", {exact: true}).setInputFiles(fixture(name));
  await expect(page.getByRole("button", {name: "Accept types and import", exact: true})).toBeVisible();
}
async function accept(page: Page): Promise<void> {
  page.once("dialog", dialog => dialog.accept());
  await page.getByRole("button", {name: "Accept types and import", exact: true}).click();
  await expect(page.locator(".notice.success")).toContainText("Spreadsheet imported");
}
test("ordinary reference workbook imports with typed values and live numeric formulas", async ({page}, testInfo) => {
  await inspect(page, "reference-two-sheet.xlsx");
  await accept(page);
  await expect(page.getByRole("heading", {name: "Budget", exact: true})).toBeVisible();
  await expect(page.locator(".table-scroll textarea").first()).toHaveValue("00123");
  const amount = page.getByLabel("Amount for Sheet 1 Row 1", {exact: true});
  await amount.fill("130");
  await amount.locator("xpath=ancestor::form").getByRole("button", {name: "Apply", exact: true}).click();
  await expect(page.locator(".table-scroll output").first()).toHaveText("170");
  await expect(page.getByLabel("Date for Sheet 1 Row 1", {exact: true})).toHaveValue("2026-09-05");
  page.once("dialog", dialog => dialog.accept("ordinary-import.roproj"));
  await page.getByRole("button", {name: "Save As", exact: true}).click();
  await expect(page.locator(".notice.success")).toContainText("Save As complete");
  await page.getByRole("button", {name: "Close", exact: true}).click();
  await page.getByRole("button", {name: "Open project", exact: true}).click();
  await expect(page.locator(".notice.success")).toContainText("Project opened");
  await expect(amount).toHaveValue("130");
  await expect(page.locator(".table-scroll output").first()).toHaveText("170");
  await page.getByRole("button", {name: "Export XLSX", exact: true}).click();
  await expect(page.getByRole("button", {name: "Acknowledge losses and download XLSX", exact: true})).toBeVisible();
  const downloading = page.waitForEvent("download");
  await page.getByRole("button", {name: "Acknowledge losses and download XLSX", exact: true}).click();
  const download = await downloading;
  await download.saveAs(testInfo.outputPath("ordinary-edited.xlsx"));
});
test("CSV inspection preserves leading zero Text and adds typed output columns", async ({page}) => {
  await inspect(page, "messy-utf8.csv");
  await page.getByRole("button", {name: /^Add output column/}).click();
  await accept(page);
  await expect(page.locator(".table-scroll textarea").first()).toHaveValue("00123");
});
