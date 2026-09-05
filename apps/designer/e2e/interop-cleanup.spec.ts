import { expect, test, type Locator, type Page } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const fixture = fileURLToPath(new URL("../tests/fixtures/interop/messy-utf8.csv", import.meta.url));
const sheet = "Imported table";
const rows = (page: Page): Locator => page.locator(".table-scroll tbody tr");
const cell = (page: Page, row: number, column: number): Locator => rows(page).nth(row).locator("td").nth(column);

async function cleanup(
  page: Page,
  operation: string,
  column: string,
  expectedChanges: string[],
  configure?: () => Promise<void>,
): Promise<void> {
  await page.getByText("Clean imported data", { exact: true }).click();
  await page.getByLabel("Cleanup operation", { exact: true }).selectOption(operation);
  await page.getByLabel("Cleanup source column", { exact: true }).selectOption({ label: column });
  await configure?.();
  const before = await page.getByTestId("revision").textContent();
  await page.getByRole("button", { name: "Preview cleanup", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Cleanup preview", exact: true })).toBeVisible();
  for (const change of expectedChanges) await expect(page.locator("details").filter({ hasText: "Clean imported data" })).toContainText(change);
  await expect(page.getByTestId("revision")).toHaveText(before ?? "");
  await page.getByRole("button", { name: "Commit exact cleanup preview", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Cleanup committed");
  await expect(page.getByTestId("revision")).not.toHaveText(before ?? "");
}

async function expectFinalValues(page: Page): Promise<void> {
  await expect(rows(page)).toHaveCount(3);
  await expect(cell(page, 0, 0).locator("textarea")).toHaveValue("00123");
  await expect(cell(page, 0, 1).locator("textarea")).toHaveValue("Ada");
  await expect(cell(page, 0, 2).locator("textarea")).toHaveValue("Ada|Lovelace");
  await expect(cell(page, 0, 3).locator('input[type="number"]')).toHaveValue("12.5");
  await expect(cell(page, 0, 4).locator("textarea")).toHaveValue("09/05/2026");
  await expect(cell(page, 0, 5).locator("textarea")).toHaveValue("quoted; comma");
  await expect(cell(page, 0, 6).locator("textarea")).toHaveValue("Ada");
  await expect(cell(page, 0, 7).locator("textarea")).toHaveValue("Lovelace");
  await expect(cell(page, 0, 8).locator('input[type="number"]')).toHaveValue("12.5");
  await expect(cell(page, 1, 0).locator("textarea")).toHaveValue("00456");
  await expect(cell(page, 1, 3).locator('input[type="number"]')).toHaveValue("8");
  await expect(cell(page, 1, 5).locator("textarea")).toHaveValue("first line\nsecond line");
  await expect(cell(page, 1, 8)).toHaveClass("empty-cell");
  await expect(cell(page, 2, 0).locator("textarea")).toHaveValue("00789");
  await expect(cell(page, 2, 1).locator("textarea")).toHaveValue("林");
  await expect(cell(page, 2, 3).locator('input[type="number"]')).toHaveValue("0");
}

test("messy CSV completes explicit typing, atomic cleanup, stock editing, durable reopen and disclosed exports", async ({ page }, testInfo) => {
  test.setTimeout(120_000);
  const pageErrors: string[] = [];
  page.on("pageerror", error => pageErrors.push(error.message));
  await page.goto("/");
  await page.getByText("Import CSV / XLSX", { exact: true }).click();
  await page.getByLabel("Spreadsheet file", { exact: true }).setInputFiles(fixture);
  await expect(page.getByRole("button", { name: "Accept types and import", exact: true })).toBeVisible();
  await page.getByLabel(`${sheet}: Amount type`, { exact: true }).selectOption("number");
  await expect(page.getByLabel(`${sheet}: Date type`, { exact: true })).toHaveValue("text");
  for (const [index, name, type] of [[1, "First", "text"], [2, "Last", "text"], [3, "Amount copy", "number"]] as const) {
    await page.getByRole("button", { name: `Add output column to ${sheet}`, exact: true }).click();
    await page.getByLabel(`${sheet} output ${String(index)} name`, { exact: true }).fill(name);
    await page.getByLabel(`${sheet} output ${String(index)} type`, { exact: true }).selectOption(type);
  }
  page.once("dialog", dialog => dialog.accept());
  await page.getByRole("button", { name: "Accept types and import", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Spreadsheet imported");
  await expect(rows(page)).toHaveCount(4);
  await expect(cell(page, 0, 0).locator("textarea")).toHaveValue("00123");
  await expect(cell(page, 0, 4).locator("textarea")).toHaveValue("09/05/2026");
  await expect(cell(page, 3, 3)).toHaveClass("empty-cell");
  await expect(cell(page, 0, 6)).toHaveClass("empty-cell");

  await cleanup(page, "trim", "Name", ['"value":" Ada "', '"value":"Ada"']);
  await cleanup(page, "replace", "Notes", ['"value":"quoted, comma"', '"value":"quoted; comma"'], async () => {
    await page.getByLabel("Cleanup find / separator / fill value", { exact: true }).fill(",");
    await page.getByLabel("Cleanup replacement", { exact: true }).fill(";");
  });
  await cleanup(page, "split", "Combined", ["First: [missing]", '"value":"Ada"', "Last: [missing]", '"value":"Lovelace"'], async () => {
    await page.getByLabel("Cleanup source row", { exact: true }).selectOption({ index: 0 });
    await page.getByLabel("Cleanup destination column", { exact: true }).selectOption({ label: "First" });
    await page.getByLabel("Cleanup second split column", { exact: true }).selectOption({ label: "Last" });
    await page.getByLabel("Cleanup find / separator / fill value", { exact: true }).fill("|");
  });
  await cleanup(page, "convert", "Amount", ["Amount copy: [missing]", '"value":12.5'], async () => {
    await page.getByLabel("Cleanup source row", { exact: true }).selectOption({ index: 0 });
    await page.getByLabel("Cleanup destination column", { exact: true }).selectOption({ label: "Amount copy" });
  });
  await cleanup(page, "fill", "Amount", ["Row 4 · Amount: [missing]", '"value":0'], async () => {
    await page.getByLabel("Cleanup find / separator / fill value", { exact: true }).fill("0");
  });
  await cleanup(page, "deduplicate", "ID", ["1 rows removed", "Row 3 · ID", "[removed]"]);
  await expect(rows(page)).toHaveCount(3);

  await page.getByLabel("Sort by", { exact: true }).selectOption({ label: "Amount" });
  await page.getByLabel("Sort descending", { exact: true }).check();
  await expect(cell(page, 0, 0).locator("textarea")).toHaveValue("00123");
  await expect(cell(page, 2, 0).locator("textarea")).toHaveValue("00789");
  await page.getByLabel("Filter column", { exact: true }).selectOption({ label: "Name" });
  await page.getByLabel("Filter text", { exact: true }).fill("Grace");
  await expect(rows(page)).toHaveCount(1);
  await expect(cell(page, 0, 0).locator("textarea")).toHaveValue("00456");
  const amount = cell(page, 0, 3);
  await amount.locator('input[type="number"]').fill("8");
  await amount.getByRole("button", { name: "Apply", exact: true }).click();
  await expect(amount.locator('input[type="number"]')).toHaveValue("8");
  await expect(page.getByTestId("revision")).toHaveText("resident/7");

  const savedName = "cleanup-journey.roproj";
  page.once("dialog", dialog => dialog.accept(savedName));
  await page.getByRole("button", { name: "Save As", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Save As complete");
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "false");
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await expect(page.getByRole("heading", { name: "No project open", exact: true })).toBeVisible();
  await page.getByLabel("Saved project", { exact: true }).selectOption(savedName);
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(page.getByTestId("revision")).toHaveText("resident/0");
  await expect(page.getByLabel("Filter text", { exact: true })).toHaveValue("Grace");
  await expect(page.getByLabel("Sort descending", { exact: true })).toBeChecked();
  await expect(rows(page)).toHaveCount(1);
  await expect(cell(page, 0, 0).locator("textarea")).toHaveValue("00456");
  await expect(cell(page, 0, 3).locator('input[type="number"]')).toHaveValue("8");
  await page.getByLabel("Filter text", { exact: true }).fill("");
  await expectFinalValues(page);

  for (const format of ["CSV", "XLSX"] as const) {
    await page.getByRole("button", { name: `Export ${format}`, exact: true }).click();
    const review = page.getByRole("region", { name: "Export compatibility review", exact: true });
    await expect(review).toBeVisible();
    await expect(review).toContainText("Review export conversions and losses");
    if (format === "CSV") await expect(review).toContainText("lossy_on_export");
    else await expect(review).toContainText("bound_formula_absolute_a1");
    const downloading = page.waitForEvent("download");
    await page.getByRole("button", { name: `Acknowledge losses and download ${format}`, exact: true }).click();
    const download = await downloading;
    const outputPath = testInfo.outputPath(`cleaned.${format.toLowerCase()}`);
    await download.saveAs(outputPath);
    await testInfo.attach(`cleaned-${format.toLowerCase()}`, { path: outputPath, contentType: format === "CSV" ? "text/csv" : "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" });
    expect(await download.failure()).toBeNull();
    const bytes = await readFile(outputPath);
    if (format === "CSV") {
      expect(bytes.toString("utf8")).toBe(
        "ID,Name,Combined,Amount,Date,Notes,First,Last,Amount copy\r\n" +
        "00123,Ada,Ada|Lovelace,12.5,09/05/2026,quoted; comma,Ada,Lovelace,12.5\r\n" +
        '00456,Grace,Grace|Hopper,8,2026-09-05,"first line\nsecond line","","",""\r\n' +
        '00789,林,林|小明,0,2026-09-05,  trim me  ,"","",""\r\n',
      );
    } else {
      expect(bytes.subarray(0, 4)).toEqual(Buffer.from([0x50, 0x4b, 0x03, 0x04]));
      expect(bytes.byteLength).toBeGreaterThan(1000);
    }
  }
  await expect(page.getByRole("button", { name: "Export XLSX", exact: true })).toBeEnabled();
  await page.getByText("Import CSV / XLSX", { exact: true }).click();
  await page.getByLabel("Spreadsheet file", { exact: true }).setInputFiles(testInfo.outputPath("cleaned.xlsx"));
  await expect(page.getByRole("button", { name: "Accept types and import", exact: true })).toBeVisible();
  await expect(page.getByLabel(`${sheet}: ID type`, { exact: true })).toHaveValue("text");
  await expect(page.getByLabel(`${sheet}: Date type`, { exact: true })).toHaveValue("text");
  await expect(page.getByLabel(`${sheet}: Amount type`, { exact: true })).toHaveValue("number");
  page.once("dialog", dialog => dialog.accept());
  await page.getByRole("button", { name: "Accept types and import", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Spreadsheet imported");
  await expectFinalValues(page);
  expect(pageErrors).toEqual([]);
});
