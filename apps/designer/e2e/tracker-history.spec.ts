import { createHash } from "node:crypto";
import { cp, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test, type Page } from "@playwright/test";

const feedback = "Tracker undo/redo cleared after an edit outside Tracker. Accepted data and formatting are preserved.";
const firstCell = (page: Page) => page.locator('[role=gridcell][data-row="0"][data-col="0"]');
let fixtureRoot: string;
let mixedProject: string;

test.beforeAll(async () => {
  fixtureRoot = await mkdtemp(join(tmpdir(), "tachiko-mixed-history-"));
  mixedProject = join(fixtureRoot, "mixed.roproj");
  await cp(new URL("../../../dogfood/operations-tracker.roproj", import.meta.url), mixedProject, { recursive: true });
  const schemas = JSON.parse(await readFile(join(mixedProject, "schemas.json"), "utf8")) as Array<{ id: string; key: string; fields: unknown[] }>;
  schemas.push({ id: "ordinary", key: "ordinary", fields: [{ id: "budget", key: "budget", field_type: { type: "number" }, required: true }] });
  schemas.sort((left, right) => left.id.localeCompare(right.id));
  await writeFile(join(mixedProject, "schemas.json"), `${JSON.stringify(schemas, null, 2)}\n`);
  const entity = { id: "ordinary_row", key: "operations", schema: "ordinary", fields: { budget: { kind: "number", value: 10 } } };
  const shard = createHash("sha256").update(entity.id).digest("hex")[0];
  const shardPath = join(mixedProject, "entities", `${shard ?? ""}.jsonl`);
  const existing = (await readFile(shardPath, "utf8")).trim().split("\n").filter(Boolean).map(line => JSON.parse(line) as { id: string });
  existing.push(entity);
  existing.sort((left, right) => left.id.localeCompare(right.id));
  await writeFile(shardPath, `${existing.map(item => JSON.stringify(item)).join("\n")}\n`);
});
test.afterAll(async () => { await rm(fixtureRoot, { recursive: true, force: true }); });

async function openMixed(page: Page): Promise<void> {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Moonfall Balance" })).toBeVisible();
  page.once("dialog", dialog => dialog.accept());
  await page.locator("[data-import-project]").setInputFiles(mixedProject);
  await expect(page.locator(".notice.success")).toContainText("Project opened");
  await page.getByLabel("Collection", { exact: true }).selectOption("tracker");
  await expect(firstCell(page)).toBeVisible();
}
async function trackerEdit(page: Page, value: string): Promise<void> {
  await firstCell(page).click();
  await page.getByLabel("Cell value", { exact: true }).fill(value);
  await page.getByRole("button", { name: "Apply to selection", exact: true }).click();
  await expect(firstCell(page)).toHaveText(value);
}
async function genericEdit(page: Page, value: string, accepted: boolean): Promise<void> {
  await page.getByLabel("Collection", { exact: true }).selectOption("ordinary");
  await page.getByLabel("Budget for Operations").fill(value);
  await page.locator('[data-field="ordinary_row.budget"]').getByRole("button", { name: "Apply", exact: true }).click();
  if (accepted) await expect(page.getByLabel("Budget for Operations")).toHaveValue(value);
  else await expect(page.getByRole("alert")).toBeVisible();
}

test("accepted generic edit clears mixed tracker histories while preserving both collections and formatting", async ({ page }) => {
  await openMixed(page);
  await trackerEdit(page, "Accepted tracker task");
  await page.getByRole("button", { name: "Bold", exact: true }).click();
  await genericEdit(page, "20", true);
  await page.getByLabel("Collection", { exact: true }).selectOption("tracker");
  await expect(page.getByRole("button", { name: "Undo", exact: true })).toBeDisabled();
  await expect(page.getByRole("button", { name: "Redo", exact: true })).toBeDisabled();
  await expect(page.getByText(feedback, { exact: true })).toBeVisible();
  await expect(firstCell(page)).toHaveText("Accepted tracker task");
  await expect(firstCell(page)).toHaveClass(/cell-bold/);

  // New tracker history must start after the accepted generic edit.
  await trackerEdit(page, "Later tracker task");
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(firstCell(page)).toHaveText("Accepted tracker task");
  await page.getByLabel("Collection", { exact: true }).selectOption("ordinary");
  await expect(page.getByLabel("Budget for Operations")).toHaveValue("20");
  await page.getByLabel("Collection", { exact: true }).selectOption("tracker");
  page.once("dialog", dialog => dialog.accept("mixed-history.roproj"));
  await page.getByRole("button", { name: "Save As", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Save As complete");
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await page.getByLabel("Collection", { exact: true }).selectOption("tracker");
  await expect(firstCell(page)).toHaveText("Accepted tracker task");
  await expect(firstCell(page)).toHaveClass(/cell-bold/);
  await page.getByLabel("Collection", { exact: true }).selectOption("ordinary");
  await expect(page.getByLabel("Budget for Operations")).toHaveValue("20");
});

test("accepted generic edit clears a tracker redo branch", async ({ page }) => {
  await openMixed(page);
  await trackerEdit(page, "First tracker change");
  await trackerEdit(page, "Second tracker change");
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(firstCell(page)).toHaveText("First tracker change");
  await expect(page.getByRole("button", { name: "Redo", exact: true })).toBeEnabled();
  await genericEdit(page, "25", true);
  await page.getByLabel("Collection", { exact: true }).selectOption("tracker");
  await expect(page.getByRole("button", { name: "Undo", exact: true })).toBeDisabled();
  await expect(page.getByRole("button", { name: "Redo", exact: true })).toBeDisabled();
  await expect(firstCell(page)).toHaveText("First tracker change");
  await expect(page.getByText(feedback, { exact: true })).toBeVisible();
});

test("read-only switches and rejected generic edits preserve tracker undo and redo", async ({ page }) => {
  await openMixed(page);
  await trackerEdit(page, "First tracker change");
  await trackerEdit(page, "Second tracker change");
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(firstCell(page)).toHaveText("First tracker change");
  await page.getByLabel("Collection", { exact: true }).selectOption("ordinary");
  await expect(page.getByLabel("Budget for Operations")).toHaveValue("10");
  await page.getByLabel("Collection", { exact: true }).selectOption("tracker");
  await expect(page.getByRole("button", { name: "Redo", exact: true })).toBeEnabled();
  await genericEdit(page, "", false);
  await page.getByLabel("Collection", { exact: true }).selectOption("tracker");
  await expect(page.getByRole("button", { name: "Undo", exact: true })).toBeEnabled();
  await expect(page.getByRole("button", { name: "Redo", exact: true })).toBeEnabled();
  await genericEdit(page, "10", false);
  await page.getByLabel("Collection", { exact: true }).selectOption("tracker");
  await expect(page.getByRole("button", { name: "Undo", exact: true })).toBeEnabled();
  await expect(page.getByRole("button", { name: "Redo", exact: true })).toBeEnabled();
  await page.getByRole("button", { name: "Redo", exact: true }).click();
  await expect(firstCell(page)).toHaveText("Second tracker change");
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(firstCell(page)).toHaveText("First tracker change");
  await page.getByLabel("Collection", { exact: true }).selectOption("ordinary");
  await expect(page.getByLabel("Budget for Operations")).toHaveValue("10");
});
