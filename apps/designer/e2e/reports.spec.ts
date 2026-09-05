import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { expect, test, type Page, type TestInfo } from "@playwright/test";

const reports = (page: Page) => page.getByRole("region", { name: "Report charts", exact: true });
const chart = (page: Page, title: string) => reports(page).locator("article").filter({ has: page.getByRole("heading", { name: title, exact: true }) });
async function newBudget(page: Page): Promise<void> {
  await page.goto("/");
  page.once("dialog", dialog => dialog.accept());
  await page.getByRole("button", { name: "New Budget", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Budget Items", exact: true })).toBeVisible();
}
async function createChart(page: Page, title: string, kind: "Column" | "Line", field: string, category: string): Promise<void> {
  const panel = reports(page);
  await panel.getByRole("button", { name: "Create chart from selected source", exact: true }).click();
  await panel.getByLabel("Chart title", { exact: true }).fill(title);
  await panel.getByLabel("Chart type", { exact: true }).selectOption({ label: kind });
  await panel.getByLabel("Category field", { exact: true }).selectOption({ label: category });
  await panel.getByLabel("Series 1 field", { exact: true }).selectOption({ label: field });
  await panel.getByLabel("Series 1 label", { exact: true }).fill(field === "Variance" ? "Budget variance" : "Actual spending");
  await panel.getByLabel("X axis label", { exact: true }).fill("Budget category");
  await panel.getByLabel("Y axis label", { exact: true }).fill("Amount");
  await panel.getByLabel("Show legend", { exact: true }).check();
  await panel.getByRole("button", { name: "Apply chart", exact: true }).click();
  await expect(chart(page, title)).toBeVisible();
}
async function applyScalar(page: Page, label: string, value: string): Promise<void> {
  const input = page.getByLabel(label, { exact: true });
  await input.fill(value);
  await input.locator("xpath=ancestor::form").getByRole("button", { name: "Apply", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Publication complete");
}
async function saveAs(page: Page, name: string): Promise<void> {
  page.once("dialog", dialog => dialog.accept(name));
  await page.getByRole("button", { name: "Save As", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Save As complete");
}
async function capturePng(page: Page, title: string, name: string, info: TestInfo) {
  const revision = await page.getByTestId("revision").textContent();
  const data = await chart(page, title).locator("table").innerText();
  const pending = page.waitForEvent("download");
  await chart(page, title).getByRole("button", { name: "Download PNG", exact: true }).click();
  const download = await pending;
  const path = info.outputPath(name);
  await download.saveAs(path);
  const bytes = await readFile(path);
  expect([...bytes.subarray(0, 8)]).toEqual([137, 80, 78, 71, 13, 10, 26, 10]);
  const pixels = await page.evaluate(async base64 => {
    const image = new Image(); image.src = `data:image/png;base64,${base64}`;
    await image.decode();
    const canvas = document.createElement("canvas"); canvas.width = image.width; canvas.height = image.height;
    const context = canvas.getContext("2d"); if (!context) throw new Error("PNG decoder context unavailable");
    context.drawImage(image, 0, 0);
    const rgba = context.getImageData(0, 0, canvas.width, canvas.height).data;
    let colored = 0;
    for (let i = 0; i < rgba.length; i += 4) if (rgba[i + 3] && ((rgba[i] ?? 255) < 235 || (rgba[i + 1] ?? 255) < 235 || (rgba[i + 2] ?? 255) < 235)) colored++;
    const hash = await crypto.subtle.digest("SHA-256", rgba);
    return { width: image.width, height: image.height, colored, rgbaSha256: [...new Uint8Array(hash)].map(value => value.toString(16).padStart(2, "0")).join("") };
  }, bytes.toString("base64"));
  expect(pixels.width).toBeGreaterThanOrEqual(800);
  expect(pixels.height).toBeGreaterThanOrEqual(400);
  expect(pixels.colored).toBeGreaterThan(1000);
  await info.attach(name, { path, contentType: "image/png" });
  return { file: name, revision, data, sha256: createHash("sha256").update(bytes).digest("hex"), ...pixels };
}

test("Budget column and line charts follow current values and survive atomic browser save/reopen with actual PNG evidence", async ({ page }, info) => {
  await newBudget(page);
  await createChart(page, "Monthly spending", "Column", "Actual", "Name");
  await createChart(page, "Spending trend", "Line", "Variance", "Name");
  await expect(chart(page, "Monthly spending").locator("tbody tr")).toHaveText(["Rent1,200", "Utilities160"]);
  await expect(chart(page, "Spending trend").locator("tbody tr")).toHaveText(["Rent0", "Utilities-20"]);
  const before = await capturePng(page, "Monthly spending", "spending-before.png", info);
  const draft = page.getByLabel("Actual for Utilities", { exact: true });
  await draft.fill("200");
  // Pending input cannot become a trustworthy current report.
  await expect(chart(page, "Monthly spending").getByRole("button", { name: "Download PNG", exact: true })).toBeDisabled();
  await applyScalar(page, "Actual for Utilities", "200");
  await expect(chart(page, "Monthly spending").locator("tbody tr")).toHaveText(["Rent1,200", "Utilities200"]);
  await expect(chart(page, "Spending trend").locator("tbody tr")).toHaveText(["Rent0", "Utilities20"]);
  const after = await capturePng(page, "Monthly spending", "spending-after.png", info);
  expect(after.revision).not.toBe(before.revision);
  expect(after.rgbaSha256).not.toBe(before.rgbaSha256);
  await saveAs(page, "charts.roproj");
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Project opened");
  await expect(chart(page, "Monthly spending").locator("tbody tr")).toHaveText(["Rent1,200", "Utilities200"]);
  await expect(chart(page, "Spending trend").locator("tbody tr")).toHaveText(["Rent0", "Utilities20"]);
  await chart(page, "Spending trend").getByRole("button", { name: "Edit chart", exact: true }).click();
  await expect(reports(page).getByLabel("Chart type", { exact: true })).toHaveValue("line");
  await expect(reports(page).getByLabel("X axis label", { exact: true })).toHaveValue("Budget category");
  await expect(reports(page).getByLabel("Y axis label", { exact: true })).toHaveValue("Amount");
  await expect(reports(page).getByLabel("Series 1 label", { exact: true })).toHaveValue("Budget variance");
  await expect(reports(page).getByLabel("Show legend", { exact: true })).toBeChecked();
  await reports(page).getByRole("button", { name: "Cancel", exact: true }).click();
  const reopened = await capturePng(page, "Spending trend", "spending-reopened-line.png", info);
  const tableContained = () => chart(page, "Spending trend").evaluate(card => {
    const table = card.querySelector("table");
    return table !== null && table.getBoundingClientRect().right <= card.getBoundingClientRect().right;
  });
  expect(await tableContained()).toBe(true);
  await page.screenshot({ path: info.outputPath("reports-reopened.png"), fullPage: true });
  await page.setViewportSize({ width: 375, height: 900 });
  expect(await tableContained()).toBe(true);
  await chart(page, "Spending trend").screenshot({ path: info.outputPath("report-mobile.png") });
  const manifest = info.outputPath("report-evidence.json");
  await writeFile(manifest, JSON.stringify({ source: "actual Rust Budget + browser UI", expectedValuesBefore: [1200, 160], expectedValuesAfter: [1200, 200], expectedFormulaVarianceBefore: [0, -20], expectedFormulaVarianceAfter: [0, 20], artifacts: [before, after, reopened], subjectiveVisualApproval: "not claimed" }, null, 2));
  await info.attach("report-evidence", { path: manifest, contentType: "application/json" });
});

test("imported charts retain selected source order through filtering, disclose XLSX loss and refuse missing series", async ({ page }, info) => {
  await page.goto("/");
  await page.getByText("Import CSV / XLSX", { exact: true }).click();
  await page.getByLabel("Spreadsheet file", { exact: true }).setInputFiles({ name: "report-source.csv", mimeType: "text/csv", buffer: Buffer.from("Name,Amount\nAlpha,10\nBeta,20\nMissing,\n") });
  await page.getByLabel("Imported table: Amount type", { exact: true }).selectOption("number");
  page.once("dialog", dialog => dialog.accept());
  await page.getByRole("button", { name: "Accept types and import", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Spreadsheet imported");
  await createChart(page, "Incomplete source", "Column", "Amount", "Name");
  await expect(chart(page, "Incomplete source")).toContainText("Chart unavailable");
  await expect(chart(page, "Incomplete source").getByRole("button", { name: "Download PNG", exact: true })).toBeDisabled();
  await reports(page).getByRole("button", { name: "Create chart from selected source", exact: true }).click();
  await reports(page).getByLabel("Chart title", { exact: true }).fill("Selected imported values");
  await reports(page).getByLabel("Category field", { exact: true }).selectOption({ label: "Name" });
  await reports(page).getByLabel("Missing (Row 3)", { exact: true }).uncheck();
  await reports(page).getByRole("button", { name: "Apply chart", exact: true }).click();
  const data = chart(page, "Selected imported values").locator("tbody tr");
  await expect(data).toHaveText(["Alpha10", "Beta20"]);
  await page.getByLabel("Sort by", { exact: true }).selectOption({ label: "Amount" });
  await page.getByLabel("Sort descending", { exact: true }).check();
  await page.getByLabel("Filter column", { exact: true }).selectOption({ label: "Name" });
  await page.getByLabel("Filter text", { exact: true }).fill("Beta");
  await expect(page.locator(".table-scroll tbody tr")).toHaveCount(1);
  await expect(data).toHaveText(["Alpha10", "Beta20"]);
  await capturePng(page, "Selected imported values", "imported-selected-values.png", info);
  await page.getByRole("button", { name: "Export XLSX", exact: true }).click();
  const review = page.getByRole("region", { name: "Export compatibility review", exact: true });
  await expect(review).toContainText(/chart/i);
  await expect(review).toContainText(/not preserved|not preserve|omitted/i);
  await expect(review.getByRole("button", { name: "Acknowledge losses and download XLSX", exact: true })).toBeVisible();
  await review.getByRole("button", { name: "Cancel export", exact: true }).click();
});
