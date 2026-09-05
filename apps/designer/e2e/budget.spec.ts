import { expect, test, type Page } from "@playwright/test";

test.describe.configure({ mode: "serial" });
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
  await expect(tools(page).getByLabel("Formula target", { exact: true })).toBeEnabled();
}

async function applyScalar(page: Page, label: string, value: string): Promise<void> {
  const control = page.getByLabel(label, { exact: true });
  await control.fill(value);
  await control.locator("xpath=ancestor::form").getByRole("button", { name: "Apply", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Publication complete");
}

/** References are selected by visible names; the user never types an address. */
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
}

async function copy(page: Page, source: string, destinations: string[], fixed: string[] = [], relativeRows = true, relativeColumns = true): Promise<void> {
  const panel = tools(page);
  await panel.getByLabel("Copy formula from", { exact: true }).selectOption({ label: source });
  await panel.getByLabel("Copy destinations", { exact: true }).selectOption(destinations.map(label => ({ label })));
  await panel.getByLabel("Fixed references", { exact: true }).selectOption(fixed.map(label => ({ label })));
  await panel.getByLabel("Relative rows", { exact: true }).setChecked(relativeRows);
  await panel.getByLabel("Relative columns", { exact: true }).setChecked(relativeColumns);
  await panel.getByRole("button", { name: "Copy formula", exact: true }).click();
}

async function namedAction(page: Page, action: string, name: string): Promise<void> {
  page.once("dialog", dialog => dialog.accept(name));
  await page.getByRole("button", { name: action, exact: true }).click();
}

async function reopen(page: Page, name: string): Promise<void> {
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page.getByLabel("Saved project", { exact: true }).selectOption(name);
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Project opened");
}

test("Date and Number display drafts survive rerender, failed save, close and reopen", async ({ page }) => {
  await newBudget(page);
  await applyScalar(page, "Due Date for Utilities", "2026-10-17");
  const actual = page.getByLabel("Actual for Utilities", { exact: true });
  await actual.fill("200");
  await cell(page, "utilities.actual").getByRole("button", { name: "Number", exact: true }).click();
  await expect(actual).toHaveValue("200");
  await actual.locator("xpath=ancestor::form").getByRole("button", { name: "Apply", exact: true }).click();
  await expect(cell(page, "utilities.actual").locator("[data-formatted-number]")).toHaveText("￥200");
  await applyScalar(page, "Planned for Utilities", "0.2");
  await cell(page, "utilities.planned").getByRole("button", { name: "Number", exact: true }).click();
  await cell(page, "utilities.planned").getByRole("button", { name: "JPY", exact: true }).click();
  await expect(cell(page, "utilities.planned").locator("[data-formatted-number]")).toHaveText("20%");
  await expect(page.getByText(/Percentage: 0.2 means 20%/)).toBeVisible();
  await namedAction(page, "Save As", "budget-original.roproj");
  await expect(page.locator(".notice.success")).toContainText("Save As complete");

  await applyScalar(page, "Actual for Utilities", "250");
  const acceptedRevision = await page.getByTestId("revision").textContent();
  await namedAction(page, "Save As", "budget-original.roproj");
  await expect(page.getByRole("alert")).toContainText("Project not saved");
  await expect(page.getByTestId("revision")).toHaveText(acceptedRevision ?? "");
  await expect(actual).toHaveValue("250");
  await expect(cell(page, "utilities.variance").locator("output")).toHaveText("249.8");
  await expect(page.getByLabel("Due Date for Utilities", { exact: true })).toHaveValue("2026-10-17");
  await namedAction(page, "Save As", "budget-recovered.roproj");
  await expect(page.locator(".notice.success")).toContainText("Save As complete");
  await reopen(page, "budget-recovered.roproj");
  await expect(cell(page, "utilities.actual").locator("[data-formatted-number]")).toHaveText("￥250");
  await expect(cell(page, "utilities.planned").locator("[data-formatted-number]")).toHaveText("20%");
  await expect(page.getByLabel("Due Date for Utilities", { exact: true })).toHaveValue("2026-10-17");
  await expect(cell(page, "utilities.variance").locator("output")).toHaveText("249.8");
  await reopen(page, "budget-original.roproj");
  await expect(page.getByLabel("Actual for Utilities", { exact: true })).toHaveValue("200");
});

test("name picker authors a Number cell and copies references relatively across columns", async ({ page }) => {
  await newBudget(page);
  await formula(page, item("Rent", "Planned"), [{ reference: item("Utilities", "Planned") }, " + 10"]);
  await expect(cell(page, "rent.planned").locator("output")).toHaveText("190");
  await copy(page, item("Rent", "Planned"), [item("Rent", "Variance")], [], false, true);
  // Planned -> Variance shifts the referenced Utilities Planned -> Utilities Variance.
  await expect(cell(page, "rent.variance").locator("output")).toHaveText("-10");
  await applyScalar(page, "Actual for Utilities", "200");
  await expect(cell(page, "rent.variance").locator("output")).toHaveText("30");
  await namedAction(page, "Save As", "budget-across.roproj");
  await expect(page.locator(".notice.success")).toContainText("Save As complete");
  await reopen(page, "budget-across.roproj");
  await expect(cell(page, "rent.planned").locator("output")).toHaveText("190");
  await expect(cell(page, "rent.variance").locator("output")).toHaveText("30");
});

test("row fill respects fixed and cross-sheet references and rejects an invalid atomic range", async ({ page }) => {
  await newBudget(page);
  await formula(page, item("Rent", "Variance"), [{ reference: item("Rent", "Actual") }, " - ", { reference: item("Rent", "Planned") }, " + 1"]);
  await expect(cell(page, "rent.variance").locator("output")).toHaveText("1");
  await copy(page, item("Rent", "Variance"), [item("Utilities", "Variance")]);
  await expect(cell(page, "utilities.variance").locator("output")).toHaveText("-19");
  await copy(page, item("Rent", "Variance"), [item("Utilities", "Variance")], [item("Rent", "Planned")]);
  await expect(cell(page, "utilities.variance").locator("output")).toHaveText("-1,039");
  await formula(page, item("Rent", "Variance"), [{ reference: item("Rent", "Actual") }, " + ", { reference: summary("Actual Total") }]);
  await expect(cell(page, "rent.variance").locator("output")).toHaveText("2,560");
  await copy(page, item("Rent", "Variance"), [item("Utilities", "Variance")]);
  await expect(cell(page, "utilities.variance").locator("output")).toHaveText("1,520");

  await formula(page, item("Rent", "Variance"), [{ reference: item("Rent", "Actual") }, " + ", { reference: summary("Actual Total") }, " + 1"]);
  await expect(cell(page, "rent.variance").locator("output")).toHaveText("2,561");
  const before = await page.getByTestId("revision").textContent();
  // First destination is valid; the second makes the relative Actual reference out of bounds.
  await copy(page, item("Rent", "Variance"), [item("Utilities", "Variance"), item("Utilities", "Actual")]);
  await expect(page.getByRole("alert")).toContainText("Edit not published");
  await expect(page.getByTestId("revision")).toHaveText(before ?? "");
  await expect(cell(page, "utilities.variance").locator("output")).toHaveText("1,520");
  await expect(page.getByLabel("Actual for Utilities", { exact: true })).toHaveValue("160");
  await tools(page).getByRole("button", { name: "Cancel copy draft", exact: true }).click();
  await formula(page, item("Rent", "Variance"), ["unsupported(1)"]);
  await expect(page.getByRole("alert")).toContainText("Edit not published");
  await expect(page.getByTestId("revision")).toHaveText(before ?? "");
  await expect(cell(page, "rent.variance").locator("output")).toHaveText("2,561");
  await tools(page).getByRole("button", { name: "Cancel formula draft", exact: true }).click();
  await applyScalar(page, "Actual for Utilities", "200");
  await expect(cell(page, "utilities.variance").locator("output")).toHaveText("1,600");
});

test("equivalent views can be added, duplicated, renamed, reordered and deleted without retargeting formulas", async ({ page }) => {
  await newBudget(page);
  await namedAction(page, "Add view", "Planning desk");
  await expect(page.getByRole("heading", { name: "Planning desk", exact: true })).toBeVisible();
  await namedAction(page, "Duplicate view", "Review copy");
  await expect(page.getByRole("heading", { name: "Review copy", exact: true })).toBeVisible();
  await namedAction(page, "Rename view", "Approved plan");
  await expect(page.getByRole("heading", { name: "Approved plan", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Move view up", exact: true }).click();
  await expect(page.locator("[data-budget-view] option")).toHaveText(["Budget Items", "Budget Summary", "Approved plan", "Planning desk"]);
  await page.getByRole("button", { name: "Move view down", exact: true }).click();
  await expect(page.locator("[data-budget-view] option")).toHaveText(["Budget Items", "Budget Summary", "Planning desk", "Approved plan"]);
  await applyScalar(page, "Actual for Utilities", "200");
  await expect(cell(page, "utilities.variance").locator("output")).toHaveText("20");
  await page.getByLabel("View", { exact: true }).selectOption({ label: "Budget Summary" });
  await expect(cell(page, "monthly_summary.actual_total").locator("output")).toHaveText("1,400");
  await namedAction(page, "Rename view", "Month totals");
  await expect(page.getByRole("heading", { name: "Month totals", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Move view up", exact: true }).click();
  await page.getByLabel("View", { exact: true }).selectOption({ label: "Planning desk" });
  await page.getByRole("button", { name: "Delete view", exact: true }).click();
  await expect(page.locator("[data-budget-view] option")).toHaveText(["Month totals", "Budget Items", "Approved plan"]);
  await page.getByLabel("View", { exact: true }).selectOption({ label: "Month totals" });
  await expect(cell(page, "monthly_summary.actual_total").locator("output")).toHaveText("1,400");
  await expect(cell(page, "monthly_summary.remaining").locator("output")).toHaveText("-20");
  await namedAction(page, "Save As", "budget-views.roproj");
  await expect(page.locator(".notice.success")).toContainText("Save As complete");
  await reopen(page, "budget-views.roproj");
  await expect(page.locator("[data-budget-view] option")).toHaveText(["Month totals", "Budget Items", "Approved plan"]);
  await expect(page.getByRole("heading", { name: "Month totals", exact: true })).toBeVisible();
  await expect(cell(page, "monthly_summary.actual_total").locator("output")).toHaveText("1,400");
  await expect(cell(page, "monthly_summary.remaining").locator("output")).toHaveText("-20");
});
