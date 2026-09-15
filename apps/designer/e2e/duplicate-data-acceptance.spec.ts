import { expect, test, type Page } from "@playwright/test";

const cell = (page: Page, field: string) => page.locator(`[data-field$=".${field}"]`);
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

async function selectView(page: Page, name: string): Promise<void> {
  const view = page.getByLabel("View", { exact: true });
  await view.selectOption({ label: name });
  await expect(view.locator("option:checked")).toHaveText(name);
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

async function formulaTarget(page: Page, collection: string, field: string): Promise<string> {
  const labels = await tools(page).getByLabel("Formula target", { exact: true }).locator("option").allTextContents();
  const label = labels.find(candidate => candidate.startsWith(`${collection} / `) && candidate.endsWith(` / ${field}`));
  if (label === undefined) throw new Error(`Missing ${field} formula target in ${collection}`);
  return label;
}

async function duplicateActiveData(page: Page, name: string): Promise<void> {
  page.once("dialog", dialog => dialog.accept(name));
  const duplicate = page.getByRole("button", { name: /^Duplicate (?:table(?: data)?|data)$/i });
  await expect(duplicate).toBeVisible();
  await duplicate.click();
  const view = page.getByLabel("View", { exact: true });
  await expect(view.getByRole("option", { name, exact: true })).toHaveCount(1);
  await expect(view.locator("option:checked")).toHaveText(name);
}

async function attemptDuplicateActiveData(page: Page, name: string): Promise<void> {
  page.once("dialog", dialog => dialog.accept(name));
  await page.getByRole("button", { name: /^Duplicate (?:table(?: data)?|data)$/i }).click();
}

async function addBudgetViews(page: Page, count: number): Promise<void> {
  for (let index = 0; index < count; index += 1) {
    const name = `Extra view ${String(index)}`;
    page.once("dialog", dialog => dialog.accept(name));
    await page.getByRole("button", { name: "Add view", exact: true }).click();
    await expect(page.getByLabel("View", { exact: true }).locator("option:checked")).toHaveText(name);
    await expect(page.getByRole("button", { name: "Add view", exact: true })).toBeEnabled();
  }
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

test("duplicated Driver data is independent, retargets internal formulas, and keeps external dependencies", async ({ page }) => {
  await newBudget(page);
  await selectView(page, "Budget Summary");

  await formula(page, summary("Remaining"), [
    { reference: summary("Planned Total") },
    " + ",
    { reference: item("Utilities", "Actual") },
  ]);
  const sourceBefore = await cell(page, "remaining").locator("output").textContent();
  expect(sourceBefore).not.toBeNull();

  await duplicateActiveData(page, "October Summary");
  const copyBefore = await cell(page, "remaining").locator("output").textContent();
  expect(copyBefore).toBe(sourceBefore);

  const copyPlanned = await formulaTarget(page, "October Summary", "Planned Total");
  await formula(page, copyPlanned, ["1000"]);
  const copyAfterInternalEdit = await cell(page, "remaining").locator("output").textContent();
  expect(copyAfterInternalEdit).not.toBe(copyBefore);

  await selectView(page, "Budget Summary");
  await expect(cell(page, "remaining").locator("output")).toHaveText(sourceBefore ?? "");

  await selectView(page, "Budget Items");
  await applyScalar(page, "Actual for Utilities", "200");

  await selectView(page, "Budget Summary");
  const sourceAfterExternalEdit = await cell(page, "remaining").locator("output").textContent();
  expect(sourceAfterExternalEdit).not.toBe(sourceBefore);

  await selectView(page, "October Summary");
  const copyAfterExternalEdit = await cell(page, "remaining").locator("output").textContent();
  expect(copyAfterExternalEdit).not.toBe(copyAfterInternalEdit);

  await selectView(page, "Budget Summary");
  await expect(cell(page, "planned_total").locator("output")).not.toHaveText("1,000");
});

test("duplicate is one reversible semantic action and survives save/reopen as a separate collection", async ({ page }) => {
  await newBudget(page);
  await selectView(page, "Budget Summary");
  const sourcePlanned = await cell(page, "planned_total").locator("output").textContent();

  await duplicateActiveData(page, "October Summary");
  await expect(cell(page, "planned_total").locator("output")).toHaveText(sourcePlanned ?? "");

  const history = page.getByRole("region", { name: "Session history", exact: true });
  await history.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(page.getByLabel("View", { exact: true }).getByRole("option", { name: "October Summary", exact: true })).toHaveCount(0);
  await history.getByRole("button", { name: "Redo", exact: true }).click();
  await expect(page.getByLabel("View", { exact: true }).getByRole("option", { name: "October Summary", exact: true })).toHaveCount(1);

  await namedAction(page, "Save As", "budget-duplicate.roproj");
  await expect(page.getByTestId("durability")).toContainText("Saved");
  await reopen(page, "budget-duplicate.roproj");

  await selectView(page, "Budget Summary");
  await expect(cell(page, "planned_total").locator("output")).toHaveText(sourcePlanned ?? "");
  await selectView(page, "October Summary");
  await expect(cell(page, "planned_total").locator("output")).toHaveText(sourcePlanned ?? "");
});

test("duplicate preflight rejects an overlong view name without semantic publication", async ({ page }) => {
  await newBudget(page);
  const revision = await page.getByTestId("revision").textContent();
  const name = "x".repeat(81);
  await attemptDuplicateActiveData(page, name);
  await expect(page.getByRole("alert")).toContainText("Data not duplicated");
  await expect(page.getByTestId("revision")).toHaveText(revision ?? "");
  await expect(page.getByLabel("View", { exact: true }).getByRole("option", { name, exact: true })).toHaveCount(0);
});

test("duplicate preflight rejects the 33rd Budget view without semantic publication", async ({ page }) => {
  await newBudget(page);
  await addBudgetViews(page, 30);
  const revision = await page.getByTestId("revision").textContent();
  await attemptDuplicateActiveData(page, "Overflow view");
  await expect(page.getByRole("alert")).toContainText("Data not duplicated");
  await expect(page.getByTestId("revision")).toHaveText(revision ?? "");
  await expect(page.getByLabel("View", { exact: true }).getByRole("option", { name: "Overflow view", exact: true })).toHaveCount(0);
});

test("duplicate uses the runtime canonical key for Unicode whitespace", async ({ page }) => {
  await newBudget(page);
  const name = "October\u0085Summary";
  await duplicateActiveData(page, name);
  await expect(page.getByRole("heading", { name, exact: true })).toBeVisible();
});

test("deleting a duplicate view keeps it deleted across semantic undo and redo", async ({ page }) => {
  await newBudget(page);
  await duplicateActiveData(page, "October Summary");
  await page.getByRole("button", { name: "Delete view", exact: true }).click();
  const view = page.getByLabel("View", { exact: true });
  await expect(view.getByRole("option", { name: "October Summary", exact: true })).toHaveCount(0);

  const history = page.getByRole("region", { name: "Session history", exact: true });
  await history.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(view.getByRole("option", { name: "October Summary", exact: true })).toHaveCount(0);
  await history.getByRole("button", { name: "Redo", exact: true }).click();
  await expect(view.getByRole("option", { name: "October Summary", exact: true })).toHaveCount(0);

  const collection = page.getByLabel("Collection", { exact: true });
  const duplicateCollection = collection.locator("option", { hasText: "October-summary" });
  await expect(duplicateCollection).toHaveCount(1);
  await collection.selectOption(await duplicateCollection.getAttribute("value") ?? "");
  await expect(collection.locator("option:checked")).toContainText("October-summary");
});
