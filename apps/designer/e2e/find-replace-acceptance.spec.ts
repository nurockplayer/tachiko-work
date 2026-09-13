import { expect, test, type Locator, type Page } from "@playwright/test";

const shortcut = process.platform === "darwin" ? "Meta" : "Control";

async function paste(page: Page, text: string): Promise<void> {
  await page.evaluate(value => navigator.clipboard.writeText(value), text);
  await page.keyboard.press(`${shortcut}+V`);
}

async function createFindTable(page: Page, rows: string): Promise<void> {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Moonfall Balance" })).toBeVisible();
  await page.getByRole("button", { name: "New Table", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "New table", exact: true });
  await expect(dialog).toBeVisible();
  await dialog.getByLabel("Table name", { exact: true }).fill("FindCanary");
  for (const [index, name, type] of [
    [0, "name", "Text"],
    [1, "note", "Text"],
    [2, "score", "Number"],
    [3, "active", "Boolean"],
    [4, "day", "Date"],
  ] as const) {
    if (index > 0) await dialog.getByRole("button", { name: "Add column", exact: true }).click();
    await dialog.getByLabel("Column name", { exact: true }).nth(index).fill(name);
    await dialog.getByLabel("Column type", { exact: true }).nth(index).selectOption({ label: type });
  }
  await dialog.getByRole("button", { name: "Create table", exact: true }).click();
  const grid = page.locator("[data-native-table-grid]");
  await expect(grid).toBeVisible();
  await grid.getByRole("gridcell", { name: "Paste rows here, or choose Append row." }).click();
  await paste(page, rows);
}

async function saveAs(page: Page, name: string): Promise<void> {
  page.once("dialog", prompt => prompt.accept(name));
  await page.getByRole("button", { name: "Save As", exact: true }).click();
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "false");
}

async function openFindReplace(page: Page): Promise<Locator> {
  const open = page.getByRole("button", { name: /find\s*\/\s*replace/i });
  await expect(open).toBeVisible();
  await open.click();
  const dialog = page.getByRole("dialog", { name: /find\s*\/\s*replace/i });
  await expect(dialog).toBeVisible();
  return dialog;
}

async function scopeToNameAndNote(dialog: Locator): Promise<void> {
  const name = dialog.getByRole("checkbox", { name: "name", exact: true });
  const note = dialog.getByRole("checkbox", { name: "note", exact: true });
  await expect(name).toBeVisible();
  await expect(note).toBeVisible();
  if (!(await name.isChecked())) await name.check();
  if (!(await note.isChecked())) await note.check();
}

test.use({ permissions: ["clipboard-read", "clipboard-write"] });

test("find and no-match are read-only, report the bounded Text-field match set, and navigate without publishing", async ({ page }) => {
  await createFindTable(
    page,
    "Ada\tAda\t12\ttrue\t2026-09-13\nAda\tother\t12\tfalse\t2026-09-14\nother\tAda\t12\ttrue\t2026-09-15",
  );
  await saveAs(page, "find-readonly.roproj");
  const revision = await page.getByTestId("revision").textContent();
  const dialog = await openFindReplace(page);
  await scopeToNameAndNote(dialog);
  const find = dialog.getByRole("textbox", { name: "Find text", exact: true });
  await find.fill("Ada");
  await dialog.getByRole("button", { name: "Find next", exact: true }).click();
  await expect(dialog).toContainText(/4\s+matches?/i);
  await dialog.getByRole("button", { name: "Find next", exact: true }).click();
  await expect(page.getByTestId("revision")).toHaveText(revision ?? "");
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "false");

  await find.fill("missing");
  await dialog.getByRole("button", { name: "Find next", exact: true }).click();
  await expect(dialog).toContainText(/0\s+matches?/i);
  await expect(page.getByTestId("revision")).toHaveText(revision ?? "");
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "false");
});

test("replace current changes one stable Text target and leaves the remaining matches navigable", async ({ page }) => {
  await createFindTable(
    page,
    "Ada\tAda\t12\ttrue\t2026-09-13\nAda\tother\t12\tfalse\t2026-09-14\nother\tAda\t12\ttrue\t2026-09-15",
  );
  const dialog = await openFindReplace(page);
  await scopeToNameAndNote(dialog);
  await dialog.getByRole("textbox", { name: "Find text", exact: true }).fill("Ada");
  await dialog.getByRole("textbox", { name: "Replace text", exact: true }).fill("Grace");
  await dialog.getByRole("button", { name: "Find next", exact: true }).click();
  await dialog.getByRole("button", { name: "Replace current", exact: true }).click();
  await expect(page.getByRole("gridcell", { name: "Grace", exact: true })).toHaveCount(1);
  await expect(page.getByRole("gridcell", { name: "Ada", exact: true })).toHaveCount(3);
  await expect(dialog).toContainText(/3\s+matches?/i);
});

test("replace all previews the complete scoped set, publishes once, survives Undo Redo and save reopen", async ({ page }) => {
  await createFindTable(
    page,
    "Ada\tAda\t12\ttrue\t2026-09-13\nAda\tother\t12\tfalse\t2026-09-14\nother\tAda\t12\ttrue\t2026-09-15",
  );
  const before = await page.getByTestId("revision").textContent();
  const dialog = await openFindReplace(page);
  await scopeToNameAndNote(dialog);
  await dialog.getByRole("textbox", { name: "Find text", exact: true }).fill("Ada");
  await dialog.getByRole("textbox", { name: "Replace text", exact: true }).fill("Done");
  await dialog.getByRole("button", { name: "Preview replace all", exact: true }).click();
  await expect(dialog).toContainText(/4\s+matches?/i);
  await expect(dialog).toContainText(/name/i);
  await expect(dialog).toContainText(/note/i);
  await dialog.getByRole("button", { name: "Commit replace all", exact: true }).click();

  await expect(page.getByRole("gridcell", { name: "Ada", exact: true })).toHaveCount(0);
  await expect(page.getByRole("gridcell", { name: "Done", exact: true })).toHaveCount(4);
  await expect(page.getByRole("gridcell", { name: "12", exact: true })).toHaveCount(3);
  const after = await page.getByTestId("revision").textContent();
  expect(after).not.toBe(before);

  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(page.getByRole("gridcell", { name: "Ada", exact: true })).toHaveCount(4);
  await expect(page.getByRole("gridcell", { name: "Done", exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "Redo", exact: true }).click();
  await expect(page.getByRole("gridcell", { name: "Done", exact: true })).toHaveCount(4);

  await saveAs(page, "find-replace.roproj");
  await page.getByRole("banner").getByRole("button", { name: "Close", exact: true }).click();
  await page.getByLabel("Saved project", { exact: true }).selectOption("find-replace.roproj");
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(page.getByRole("gridcell", { name: "Done", exact: true })).toHaveCount(4);
});

test("editing either preview input with real multi-keystroke typing invalidates immediately without losing focus or caret", async ({ page }) => {
  await createFindTable(
    page,
    "Ada\tAda\t12\ttrue\t2026-09-13\nAda\tother\t12\tfalse\t2026-09-14\nother\tAda\t12\ttrue\t2026-09-15",
  );
  const dialog = await openFindReplace(page);
  await scopeToNameAndNote(dialog);
  const find = dialog.getByRole("textbox", { name: "Find text", exact: true });
  const replacement = dialog.getByRole("textbox", { name: "Replace text", exact: true });
  await find.click();
  await page.keyboard.type("Ada");
  await replacement.click();
  await page.keyboard.type("Done");
  await dialog.getByRole("button", { name: "Preview replace all", exact: true }).click();
  await expect(dialog.getByLabel("Replace all preview", { exact: true })).toBeVisible();

  await find.click();
  await page.keyboard.press(`${shortcut}+A`);
  await page.keyboard.type("Ada again");
  await expect(find).toBeFocused();
  await expect(find).toHaveValue("Ada again");
  await expect(dialog.getByLabel("Replace all preview", { exact: true })).toHaveCount(0);
  await expect(dialog.getByRole("button", { name: "Commit replace all", exact: true })).toBeDisabled();

  await replacement.click();
  await page.keyboard.press(`${shortcut}+A`);
  await page.keyboard.type("Grace");
  await page.keyboard.press("ArrowLeft");
  await page.keyboard.type("!");
  await expect(replacement).toBeFocused();
  await expect(replacement).toHaveValue("Grac!e");
});

test("reopening the same resident occurrence discards an uncommitted replace-all preview", async ({ page }) => {
  await createFindTable(
    page,
    "Ada\tAda\t12\ttrue\t2026-09-13\nAda\tother\t12\tfalse\t2026-09-14\nother\tAda\t12\ttrue\t2026-09-15",
  );
  await saveAs(page, "find-replace-stale-preview.roproj");
  const dialog = await openFindReplace(page);
  await scopeToNameAndNote(dialog);
  await dialog.getByRole("textbox", { name: "Find text", exact: true }).fill("Ada");
  await dialog.getByRole("textbox", { name: "Replace text", exact: true }).fill("Done");
  await dialog.getByRole("button", { name: "Preview replace all", exact: true }).click();
  await expect(dialog.getByLabel("Replace all preview", { exact: true })).toBeVisible();

  await page.getByRole("banner").getByRole("button", { name: "Close", exact: true }).click();
  await page.getByLabel("Saved project", { exact: true }).selectOption("find-replace-stale-preview.roproj");
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  const reopened = await openFindReplace(page);
  await expect(reopened.getByLabel("Replace all preview", { exact: true })).toHaveCount(0);
  await expect(reopened.getByRole("button", { name: "Commit replace all", exact: true })).toBeDisabled();
});

test("Text replacement does not rewrite a matching rendered Number value", async ({ page }) => {
  await createFindTable(
    page,
    "12\tAda\t12\ttrue\t2026-09-13\nAda\t12\t12\tfalse\t2026-09-14\nother\tother\t12\ttrue\t2026-09-15",
  );
  const dialog = await openFindReplace(page);
  await scopeToNameAndNote(dialog);
  await dialog.getByRole("textbox", { name: "Find text", exact: true }).fill("12");
  await dialog.getByRole("textbox", { name: "Replace text", exact: true }).fill("twelve");
  await dialog.getByRole("button", { name: "Preview replace all", exact: true }).click();
  await expect(dialog).toContainText(/2\s+matches?/i);
  await dialog.getByRole("button", { name: "Commit replace all", exact: true }).click();
  await expect(page.getByRole("gridcell", { name: "twelve", exact: true })).toHaveCount(2);
  await expect(page.getByRole("gridcell", { name: "12", exact: true })).toHaveCount(3);
});
