import { expect, test, type Page } from "@playwright/test";
import { fileURLToPath } from "node:url";

type Corruption = "unknown field" | "invalid base64";

async function saveAs(page: Page, name: string): Promise<void> {
  page.once("dialog", dialog => dialog.accept(name));
  await page.getByRole("button", { name: "Save As", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Save As complete");
}

/** Change only the private sidecar; capture every persisted property for exact preservation checks. */
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
          const presentation = JSON.parse(record.presentation) as {
            interop: { metadata: { sheets: Array<{ columns: Array<{ field_id: string }> }> }; source: { base64: string } };
          };
          if (corruption === "unknown field") {
            // All collection IDs and sidecar shapes stay valid: only Rust can reject this binding.
            presentation.interop.metadata.sheets[0]!.columns[0]!.field_id = "unknown-import-field";
          } else {
            presentation.interop.source.base64 = "%%% invalid source %%%";
          }
          record.presentation = JSON.stringify(presentation);
          store.put(record);
        }
        captured = { ...record, bytes: Array.from(new Uint8Array(record.bytes)) };
      };
    };
  }), { name, corruption });
}

for (const corruption of ["unknown field", "invalid base64"] as const) {
  test(`rejected imported ${corruption} retains the active Tracker, history, and stored candidate`, async ({ page }) => {
    const pageErrors: string[] = [];
    page.on("pageerror", error => pageErrors.push(error.message));
    await page.goto("/");
    await page.getByText("Import CSV / XLSX", { exact: true }).click();
    await page.getByLabel("Spreadsheet file", { exact: true }).setInputFiles(
      fileURLToPath(new URL("../tests/fixtures/interop/messy-utf8.csv", import.meta.url)),
    );
    await expect(page.getByRole("button", { name: "Accept types and import", exact: true })).toBeVisible();
    page.once("dialog", dialog => dialog.accept());
    await page.getByRole("button", { name: "Accept types and import", exact: true }).click();
    await expect(page.locator(".notice.success")).toContainText("Spreadsheet imported");
    await expect(page.locator(".table-scroll textarea").first()).toHaveValue("00123");
    const candidateName = "rejected-import.roproj";
    await saveAs(page, candidateName);

    await page.getByRole("button", { name: "New Tracker", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Driver Tracker", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Append row", exact: true }).click();
    const taskCell = page.locator('[role="gridcell"][data-row="0"][data-col="0"]');
    await expect(taskCell).toHaveText("");
    const originalTask = await taskCell.textContent();
    await taskCell.click();
    await page.getByLabel("Cell value", { exact: true }).fill("Retain accepted Tracker work");
    await page.getByRole("button", { name: "Apply to selection", exact: true }).click();
    await expect(taskCell).toHaveText("Retain accepted Tracker work");
    const revision = await page.getByTestId("revision").textContent();
    await expect(page.getByRole("button", { name: "Undo", exact: true })).toBeEnabled();
    const corruptedRecord = await storedRecord(page, candidateName, corruption);

    await page.getByLabel("Saved project", { exact: true }).selectOption(candidateName);
    page.once("dialog", dialog => dialog.accept());
    await page.getByRole("button", { name: "Open", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("Project not opened");
    await expect(page.getByRole("alert")).toContainText(corruption === "unknown field"
      ? "invalid interop field, label, or width mapping"
      : "Unsupported saved spreadsheet state");
    await expect(page.getByRole("alert")).not.toContainText("TypeError");
    await expect(page.getByRole("heading", { name: "Driver Tracker", exact: true })).toBeVisible();
    await expect(page.getByTestId("revision")).toHaveText(revision ?? "");
    await expect(taskCell).toHaveText("Retain accepted Tracker work");
    await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "true");
    expect(await storedRecord(page, candidateName)).toEqual(corruptedRecord);

    // Real worker history commands prove the resident was never replaced by the inspected import.
    await page.getByRole("button", { name: "Undo", exact: true }).click();
    await expect(taskCell).toHaveText(originalTask ?? "");
    await expect(page.getByTestId("revision")).not.toHaveText(revision ?? "");
    await page.getByRole("button", { name: "Redo", exact: true }).click();
    await expect(taskCell).toHaveText("Retain accepted Tracker work");
    await saveAs(page, "retained-tracker.roproj");
    await page.getByRole("button", { name: "Close", exact: true }).click();
    await expect(page.getByRole("heading", { name: "No project open", exact: true })).toBeVisible();
    await page.getByLabel("Saved project", { exact: true }).selectOption("retained-tracker.roproj");
    await page.getByRole("button", { name: "Open project", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Driver Tracker", exact: true })).toBeVisible();
    await expect(taskCell).toHaveText("Retain accepted Tracker work");
    expect(await storedRecord(page, candidateName)).toEqual(corruptedRecord);
    expect(pageErrors).toEqual([]);
  });
}
