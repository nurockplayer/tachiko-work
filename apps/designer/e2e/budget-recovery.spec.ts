import { expect, test, type Dialog, type Page } from "@playwright/test";

test.describe.configure({ mode: "default" });
type Corruption = "unknown collection" | "null view" | "null budgetViews";

async function createBudget(page: Page): Promise<void> {
  await page.goto("/");
  page.once("dialog", dialog => dialog.accept());
  await page.getByRole("button", { name: "New Budget", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Monthly Budget", exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Budget Items", exact: true })).toBeVisible();
}

async function saveAs(page: Page, name: string): Promise<void> {
  page.once("dialog", dialog => dialog.accept(name));
  await page.getByRole("button", { name: "Save As", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Save As complete");
}

/** Corrupt only private presentation; keep the Rust project bytes and host metadata exact. */
async function storedRecord(page: Page, name: string, corruption?: Corruption): Promise<unknown> {
  return page.evaluate(({ name, corruption }) => new Promise((resolve, reject) => {
    const open = indexedDB.open("tachiko-designer-projects", 2);
    open.onerror = () => { reject(open.error ?? new Error("Project database unavailable")); };
    open.onsuccess = () => {
      const database = open.result;
      const transaction = database.transaction("projects", corruption ? "readwrite" : "readonly");
      let captured: unknown;
      transaction.oncomplete = () => { database.close(); resolve(captured); };
      transaction.onabort = () => { database.close(); reject(transaction.error ?? new Error("Project transaction aborted")); };
      const store = transaction.objectStore("projects");
      const get = store.get(name);
      get.onsuccess = () => {
        const record = get.result as { name: string; bytes: ArrayBuffer; saved_at: string; presentation: string } | undefined;
        if (!record) { transaction.abort(); return; }
        if (corruption) {
          const presentation = JSON.parse(record.presentation) as { budgetViews: { views: Array<{ collection: string } | null> } | null };
          if (!presentation.budgetViews) { transaction.abort(); return; }
          if (corruption === "unknown collection") presentation.budgetViews.views[0]!.collection = "missing-schema-id";
          else if (corruption === "null view") presentation.budgetViews.views = [null];
          else presentation.budgetViews = null;
          record.presentation = JSON.stringify(presentation);
          store.put(record);
        }
        captured = { ...record, bytes: Array.from(new Uint8Array(record.bytes)) };
      };
    };
  }), { name, corruption });
}

for (const corruption of ["unknown collection", "null view", "null budgetViews"] as const) {
  test(`opening a Budget with ${corruption} retains the active Rust Tracker occurrence and undo history`, async ({ page }) => {
    await createBudget(page);
    const name = "rejected-budget.roproj";
    await saveAs(page, name);
    const corruptedRecord = await storedRecord(page, name, corruption);

    await page.getByRole("button", { name: "New Tracker", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Driver Tracker", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Append row", exact: true }).click();
    const taskCell = page.locator('[role="gridcell"][data-row="0"][data-col="0"]');
    await expect(taskCell).toHaveText("");
    const originalTask = await taskCell.textContent();
    await taskCell.click();
    await page.getByLabel("Cell value", { exact: true }).fill("Keep my accepted Tracker work");
    await page.getByRole("button", { name: "Apply to selection", exact: true }).click();
    await expect(taskCell).toHaveText("Keep my accepted Tracker work");
    const revision = await page.getByTestId("revision").textContent();
    await expect(page.getByRole("button", { name: "Undo", exact: true })).toBeEnabled();

    await page.getByLabel("Saved project", { exact: true }).selectOption(name);
    page.once("dialog", dialog => dialog.accept());
    await page.getByRole("button", { name: "Open", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("Project not opened");
    await expect(page.getByRole("alert")).not.toContainText("TypeError");
    await expect(page.getByRole("alert")).not.toContainText("Cannot read properties");
    await expect(page.getByRole("heading", { name: "Driver Tracker", exact: true })).toBeVisible();
    await expect(page.getByTestId("revision")).toHaveText(revision ?? "");
    await expect(taskCell).toHaveText("Keep my accepted Tracker work");
    await expect(page.getByRole("button", { name: "Undo", exact: true })).toBeEnabled();
    expect(await storedRecord(page, name)).toEqual(corruptedRecord);

    // Undo and redo execute real worker commands. A stale UI over a replaced runtime cannot pass.
    await page.getByRole("button", { name: "Undo", exact: true }).click();
    await expect(taskCell).toHaveText(originalTask ?? "");
    await expect(page.getByTestId("revision")).not.toHaveText(revision ?? "");
    await page.getByRole("button", { name: "Redo", exact: true }).click();
    await expect(taskCell).toHaveText("Keep my accepted Tracker work");
    await saveAs(page, "retained-tracker.roproj");
    await page.getByRole("button", { name: "Close", exact: true }).click();
    await page.getByLabel("Saved project", { exact: true }).selectOption("retained-tracker.roproj");
    await page.getByRole("button", { name: "Open project", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Driver Tracker", exact: true })).toBeVisible();
    await expect(taskCell).toHaveText("Keep my accepted Tracker work");
    expect(await storedRecord(page, name)).toEqual(corruptedRecord);
  });
}

test("a rejected Budget Number keeps its draft through save rejection and formatting, and supports retry or cancel", async ({ page }) => {
  await createBudget(page);
  const formula = page.getByLabel("Formula for Variance for Rent", { exact: true });
  await formula.fill("1 / [rent.planned]");
  await formula.locator("xpath=ancestor::form").getByRole("button", { name: "Apply formula", exact: true }).click();
  await expect(page.locator('[data-field="rent.variance"] output')).toHaveText("0.0008");
  const acceptedRevision = await page.getByTestId("revision").textContent();
  const planned = page.getByLabel("Planned for Rent", { exact: true });
  await planned.fill("0");
  await planned.locator("xpath=ancestor::form").getByRole("button", { name: "Apply", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Edit not published");
  await expect(page.getByRole("alert")).toContainText("formula.division_by_zero");
  await expect(planned).toHaveValue("0");
  await expect(page.getByTestId("revision")).toHaveText(acceptedRevision ?? "");
  await expect(page.locator('[data-field="rent.variance"] output')).toHaveText("0.0008");
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "true");

  // Saving must reject before prompting or publishing any partially accepted state.
  const dialogs: string[] = [];
  const rejectPrompt = async (dialog: Dialog): Promise<void> => { dialogs.push(dialog.message()); await dialog.dismiss(); };
  page.on("dialog", rejectPrompt);
  await page.getByRole("button", { name: "Save As", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Project not saved");
  expect(dialogs).toEqual([]);
  page.off("dialog", rejectPrompt);
  await page.locator('[data-field="rent.planned"]').getByRole("button", { name: "Number", exact: true }).click();
  await expect(planned).toHaveValue("0");
  await expect(page.locator('[data-field="rent.planned"] [data-formatted-number]')).toHaveText("￥1,200");

  await planned.fill("2");
  await planned.locator("xpath=ancestor::form").getByRole("button", { name: "Apply", exact: true }).click();
  await expect(page.locator('[data-field="rent.variance"] output')).toHaveText("0.5");
  await planned.fill("0");
  await planned.locator("xpath=ancestor::form").getByRole("button", { name: "Apply", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Edit not published");
  await expect(planned).toHaveValue("0");
  await page.getByRole("button", { name: "Cancel pending Budget edits", exact: true }).click();
  await expect(planned).toHaveValue("2");
  await expect(page.locator('[data-field="rent.variance"] output')).toHaveText("0.5");
  await saveAs(page, "accepted-budget.roproj");
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page.getByLabel("Saved project", { exact: true }).selectOption("accepted-budget.roproj");
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(planned).toHaveValue("2");
  await expect(page.locator('[data-field="rent.variance"] output')).toHaveText("0.5");
});
