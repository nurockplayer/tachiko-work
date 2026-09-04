import { readFile } from "node:fs/promises";
import { expect, test, type Page } from "@playwright/test";

const operations = (await readFile(new URL("./fixtures/operations-tracker.tsv", import.meta.url), "utf8")).trimEnd();
const shortcut = process.platform === "darwin" ? "Meta" : "Control";
const cell = (page: Page, row: number, column: number) => page.locator(`[role=gridcell][data-row="${String(row)}"][data-col="${String(column)}"]`);
const rows = (page: Page) => page.locator("[aria-label='Tracker cells'] tbody tr");

async function paste(page: Page, text: string): Promise<void> {
  await page.evaluate(value => navigator.clipboard.writeText(value), text);
  await page.keyboard.press(`${shortcut}+V`);
}
async function createTracker(page: Page): Promise<void> {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Moonfall Balance" })).toBeVisible();
  page.once("dialog", dialog => dialog.accept());
  await page.getByRole("button", { name: "New Tracker", exact: true }).click();
  await expect(page.getByRole("grid", { name: "Tracker cells" })).toBeVisible();
  await page.getByRole("button", { name: "Append row", exact: true }).click();
  await expect(rows(page)).toHaveCount(1);
}
async function applyCell(page: Page, row: number, column: number, value: string): Promise<void> {
  await cell(page, row, column).click();
  if (column === 2) await page.getByLabel("Cell value", { exact: true }).selectOption(value);
  else await page.getByLabel("Cell value", { exact: true }).fill(value);
  await page.getByRole("button", { name: "Apply to selection", exact: true }).click();
  await expect(cell(page, row, column)).toHaveText(value);
}
async function saveAs(page: Page): Promise<void> {
  page.once("dialog", dialog => dialog.accept("operations.roproj"));
  await page.getByRole("button", { name: "Save As", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Save As complete");
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "false");
}

test.use({ permissions: ["clipboard-read", "clipboard-write"] });

test("40-row operations tracker supports external clipboard, edits, views, history and durable Save", async ({ page }) => {
  test.setTimeout(60_000);
  await createTracker(page);
  await cell(page, 0, 0).click();
  await paste(page, operations);
  await expect(rows(page)).toHaveCount(40);
  await expect(cell(page, 0, 0)).toHaveText("Review incoming support queue");
  await expect(cell(page, 39, 0)).toHaveText("Prepare next operations handoff");
  await cell(page, 0, 0).click();
  await page.keyboard.press("ArrowRight");
  await expect(cell(page, 0, 1)).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(cell(page, 0, 2)).toBeFocused();
  await page.keyboard.press("Home");
  await expect(cell(page, 0, 0)).toBeFocused();
  await page.keyboard.press("PageDown");
  await expect(cell(page, 15, 0)).toBeFocused();
  await page.keyboard.press(`${shortcut}+Home`);
  await expect(cell(page, 0, 0)).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByLabel("Cell value", { exact: true })).toBeFocused();
  await page.getByLabel("Cell value", { exact: true }).fill("Uncommitted draft");
  await page.getByRole("button", { name: "Cancel edit", exact: true }).click();
  await expect(cell(page, 0, 0)).toHaveText("Review incoming support queue");

  await applyCell(page, 0, 0, "Review urgent support queue");
  await applyCell(page, 0, 1, "12");
  await applyCell(page, 0, 2, "true");
  await cell(page, 0, 0).click();
  const revision = await page.getByTestId("revision").textContent();
  await paste(page, "Must not publish\t99\tfalse\nInvalid second row\t2\tmaybe");
  await expect(page.getByRole("alert")).toContainText("Tracker operation not completed");
  await expect(page.getByTestId("revision")).toHaveText(revision ?? "");
  await expect(cell(page, 0, 0)).toHaveText("Review urgent support queue");
  await expect(cell(page, 1, 0)).toHaveText("Triage failed import reports");

  await cell(page, 0, 0).click();
  await cell(page, 1, 2).click({ modifiers: ["Shift"] });
  await expect(page.locator('[role=gridcell][aria-selected="true"]')).toHaveCount(6);
  await page.keyboard.press(`${shortcut}+C`);
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  expect(copied.replaceAll("\r\n", "\n")).toBe("Review urgent support queue\t12\ttrue\nTriage failed import reports\t2\tfalse");
  // An external plain-text editing surface receives the native browser paste.
  await page.evaluate(() => {
    const textarea = document.createElement("textarea");
    textarea.setAttribute("aria-label", "External clipboard destination");
    document.body.append(textarea);
  });
  await page.getByLabel("External clipboard destination").focus();
  await page.keyboard.press(`${shortcut}+V`);
  await expect(page.getByLabel("External clipboard destination")).toHaveValue(copied.replaceAll("\r\n", "\n"));

  await cell(page, 0, 0).click();
  await page.getByRole("button", { name: "Bold", exact: true }).click();
  await page.getByRole("button", { name: "Fill", exact: true }).click();
  await page.getByRole("button", { name: "Wrap", exact: true }).click();
  await page.getByRole("button", { name: "Border", exact: true }).click();
  await page.getByRole("button", { name: "Alignment", exact: true }).click();
  await page.getByRole("button", { name: "Column width", exact: true }).click();
  await page.getByRole("button", { name: "Row height", exact: true }).click();
  await expect(cell(page, 0, 0)).toHaveClass(/cell-bold.*cell-fill.*cell-wrap.*cell-border/);
  await expect(cell(page, 0, 0)).toHaveCSS("text-align", "center");
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(rows(page).first()).toHaveAttribute("style", "height:36px");
  await page.getByRole("button", { name: "Redo", exact: true }).click();
  await expect(rows(page).first()).toHaveAttribute("style", "height:56px");
  await page.screenshot({ path: "test-results/tracker-formatted.png", fullPage: true });

  await page.getByLabel("Sort column").selectOption("estimate");
  await expect(cell(page, 0, 1)).toHaveText("1");
  await page.getByRole("button", { name: "Ascending", exact: true }).click();
  await expect(cell(page, 0, 1)).toHaveText("12");
  await page.getByLabel("Sort column").selectOption("");
  await page.getByLabel("Find / filter").fill("urgent");
  await page.getByLabel("Find / filter").press("Tab");
  await expect(rows(page)).toHaveCount(1);
  await page.getByLabel("Find / filter").fill("");
  await page.getByLabel("Find / filter").press("Tab");
  await expect(rows(page)).toHaveCount(40);

  await page.getByRole("button", { name: "Append row", exact: true }).click();
  await expect(rows(page)).toHaveCount(41);
  await cell(page, 40, 0).click();
  await page.getByRole("button", { name: "Remove selected rows", exact: true }).click();
  await expect(rows(page)).toHaveCount(40);
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(rows(page)).toHaveCount(41);
  await page.getByRole("button", { name: "Redo", exact: true }).click();
  await expect(rows(page)).toHaveCount(40);

  await cell(page, 0, 0).click();
  await page.getByRole("button", { name: "Move rows down", exact: true }).click();
  await expect(cell(page, 1, 0)).toHaveText("Review urgent support queue");
  await saveAs(page);
  await applyCell(page, 1, 1, "13");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Save complete");
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "false");
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Project closed");
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(rows(page)).toHaveCount(40);
  await expect(cell(page, 1, 0)).toHaveText("Review urgent support queue");
  await expect(cell(page, 1, 1)).toHaveText("13");
  await expect(cell(page, 1, 2)).toHaveText("true");
  await expect(cell(page, 1, 0)).toHaveClass(/cell-bold.*cell-fill.*cell-wrap.*cell-border/);
  await expect(cell(page, 1, 0)).toHaveCSS("text-align", "center");
  await expect(rows(page).first()).toHaveAttribute("style", "height:56px");
  await applyCell(page, 1, 0, "Review resolved support queue");
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "true");
});

test("stale Save preserves current edits and another tab's durable project", async ({ page, context }) => {
  await createTracker(page);
  await applyCell(page, 0, 0, "Original task");
  await saveAs(page);
  const other = await context.newPage();
  await other.goto("/");
  await expect(other.getByRole("heading", { name: "Moonfall Balance" })).toBeVisible();
  await other.getByLabel("Saved project").selectOption("operations.roproj");
  other.once("dialog", dialog => dialog.accept());
  await other.getByRole("button", { name: "Open", exact: true }).click();
  await expect(cell(other, 0, 0)).toHaveText("Original task");
  await applyCell(other, 0, 0, "Saved by other tab");
  await other.getByRole("button", { name: "Save", exact: true }).click();
  await expect(other.locator(".notice.success")).toContainText("Save complete");
  await applyCell(page, 0, 0, "Unsaved local task");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("changed elsewhere");
  await expect(cell(page, 0, 0)).toHaveText("Unsaved local task");
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "true");
  await other.getByRole("button", { name: "Close", exact: true }).click();
  await other.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(cell(other, 0, 0)).toHaveText("Saved by other tab");
});

test("empty tracker accepts quoted multiline clipboard text and keeps multiline edits after reopen", async ({ page }) => {
  await page.goto("/");
  page.once("dialog", dialog => dialog.accept());
  await page.getByRole("button", {name: "New Tracker", exact: true}).click();
  await page.getByRole("gridcell", {name: "Paste rows here, or choose Append row."}).click();
  await paste(page, '"\nFirst\nSecond"\t1\tfalse');
  await expect(page.getByLabel("Cell value", {exact: true})).toHaveValue("\nFirst\nSecond");
  await page.getByLabel("Cell value", {exact: true}).fill("\nEdited\nSecond");
  await page.getByRole("button", {name: "Apply to selection", exact: true}).click();
  await expect(page.getByLabel("Cell value", {exact: true})).toHaveValue("\nEdited\nSecond");
  await saveAs(page);
  await page.getByRole("button", {name: "Close", exact: true}).click();
  await page.getByRole("button", {name: "Open project", exact: true}).click();
  await expect(page.getByLabel("Cell value", {exact: true})).toHaveValue("\nEdited\nSecond");
});
