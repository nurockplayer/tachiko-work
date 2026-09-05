import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test, type Page } from "@playwright/test";

const PRODUCT_GAPS_PROJECT = fileURLToPath(
  new URL("../../../dogfood/product-gaps.roproj", import.meta.url),
);

test("Moonfall Number edit selectively refreshes DPS and rejects an invalid candidate", async ({
  page,
}) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Moonfall Balance" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Weapons" })).toBeVisible();
  await expect(page.getByTestId("revision")).toContainText("resident/0");
  await expect(page.getByLabel("Damage for Iron Sword")).toHaveValue("36");
  await expect(page.locator('[data-field="iron_sword.dps"] output')).toHaveText("40");

  await page.getByLabel("Damage for Iron Sword").fill("45");
  await page
    .locator('[data-field="iron_sword.damage"]')
    .getByRole("button", { name: "Apply" })
    .click();

  await expect(page.getByTestId("revision")).toContainText("resident/1");
  await expect(page.getByLabel("Damage for Iron Sword")).toHaveValue("45");
  await expect(page.locator('[data-field="iron_sword.dps"] output')).toHaveText("50");
  await expect(page.locator(".notice.success")).toContainText("Publication complete");

  await page.getByLabel("Attack Interval for Iron Sword").fill("0");
  await page
    .locator('[data-field="iron_sword.attack_interval"]')
    .getByRole("button", { name: "Apply" })
    .click();

  await expect(page.getByRole("alert")).toContainText("Edit not published");
  await expect(page.getByRole("alert")).toContainText("formula.division_by_zero");
  await expect(page.getByTestId("revision")).toContainText("resident/1");
  // The rejected candidate remains an editable draft; calculations stay canonical.
  await expect(page.getByLabel("Attack Interval for Iron Sword")).toHaveValue("0");
  await expect(page.getByLabel("Damage for Iron Sword")).toHaveValue("45");
  await expect(page.locator('[data-field="iron_sword.dps"] output')).toHaveText("50");
});

test("canonical Product Gap project edits typed values, refreshes priority, and round-trips", async ({
  page,
}) => {
  await page.goto("/");

  page.once("dialog", async (dialog) => dialog.accept());
  await page.locator("[data-import-project]").setInputFiles(PRODUCT_GAPS_PROJECT);
  await expect(page.locator(".notice.success")).toContainText("Project opened");
  await expect(page.getByRole("heading", { name: "Tachiko Work Product Gaps" })).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Product Gaps", exact: true }),
  ).toBeVisible();
  await expect(page.getByTestId("revision")).toContainText("resident/0");

  const designerRow = page.getByRole("row", { name: /Designer Profile Bound/ });
  const authoringRow = page.getByRole("row", { name: /Schema Authoring Missing/ });
  await expect(designerRow.locator(".formula-cell output")).toHaveText("10");
  await expect(authoringRow.locator(".formula-cell output")).toHaveText("9");
  await expect(page.getByLabel("Title for Schema Authoring Missing")).toHaveValue(
    "Schema and field authoring is not exposed",
  );

  const title = page.getByLabel("Title for Designer Profile Bound");
  await title.fill("Designer admits ordinary projects");
  await title.locator("xpath=ancestor::form").getByRole("button", { name: "Apply" }).click();
  await expect(page.getByTestId("revision")).toContainText("resident/1");
  await expect(title).toHaveValue("Designer admits ordinary projects");

  const impact = page.getByLabel("Impact for Designer Profile Bound");
  await impact.fill("3");
  await impact.locator("xpath=ancestor::form").getByRole("button", { name: "Apply" }).click();
  await expect(page.getByTestId("revision")).toContainText("resident/2");
  await expect(designerRow.locator(".formula-cell output")).toHaveText("8");
  await expect(authoringRow.locator(".formula-cell output")).toHaveText("9");
  await expect(page.getByLabel("Title for Schema Authoring Missing")).toHaveValue(
    "Schema and field authoring is not exposed",
  );

  const confirmed = page.getByLabel("Confirmed for Designer Profile Bound");
  await confirmed.uncheck();
  await confirmed.locator("xpath=ancestor::form").getByRole("button", { name: "Apply" }).click();
  await expect(page.getByTestId("revision")).toContainText("resident/3");
  await expect(confirmed).not.toBeChecked();

  await impact.fill("");
  await impact.locator("xpath=ancestor::form").getByRole("button", { name: "Apply" }).click();
  await expect(page.getByRole("alert")).toContainText("Edit not published");
  await expect(page.getByRole("alert")).toContainText("not a finite Number");
  await expect(page.getByTestId("revision")).toContainText("resident/3");
  await expect(impact).toHaveValue("");
  await expect(impact).toHaveAttribute("data-initial-number", "3");
  await expect(designerRow.locator(".formula-cell output")).toHaveText("8");
  // Restore the accepted value to explicitly abandon the rejected draft.
  await impact.fill("3");

  page.once("dialog", async (dialog) => dialog.accept("product-gaps-edited.roproj"));
  await page.getByRole("button", { name: "Save As" }).click();
  await expect(page.locator(".notice.success")).toContainText("Save As complete");
  await expect(page.getByTestId("durability")).toContainText("Saved");

  await page.getByRole("button", { name: "Close" }).click();
  await expect(page.getByRole("heading", { name: "No project open" })).toBeVisible();
  await page.getByLabel("Saved project").selectOption("product-gaps-edited.roproj");
  await page.getByRole("button", { name: "Open project" }).click();

  await expect(page.getByRole("heading", { name: "Tachiko Work Product Gaps" })).toBeVisible();
  await expect(page.getByTestId("revision")).toContainText("resident/0");
  await expect(page.getByLabel("Title for Designer Profile Bound")).toHaveValue(
    "Designer admits ordinary projects",
  );
  await expect(page.getByLabel("Impact for Designer Profile Bound")).toHaveValue("3");
  await expect(page.getByLabel("Confirmed for Designer Profile Bound")).not.toBeChecked();
  await expect(
    page
      .getByRole("row", { name: /Designer Profile Bound/ })
      .locator(".formula-cell output"),
  ).toHaveText("8");
  await expect(
    page
      .getByRole("row", { name: /Schema Authoring Missing/ })
      .locator(".formula-cell output"),
  ).toHaveText("9");
  await expect(page.getByTestId("durability")).toContainText("Saved");
});

test("canonical Save As survives close and reload while existing destinations remain unchanged", async ({
  page,
}) => {
  await page.goto("/");

  page.once("dialog", async (dialog) => dialog.accept("source.roproj"));
  await page.getByRole("button", { name: "Save As" }).click();
  await expect(page.locator(".notice.success")).toContainText("Save As complete");
  await expect(page.getByTestId("durability")).toContainText("Saved");

  const externalRoot = await mkdtemp(join(tmpdir(), "tachiko-designer-open-"));
  const externalProject = join(externalRoot, "source.roproj");
  await materializeBrowserProject(page, "source.roproj", externalProject);
  await page.locator("[data-import-project]").setInputFiles(externalProject);
  await expect(page.locator(".notice.success")).toContainText("Project opened");
  await expect(page.getByTestId("revision")).toContainText("resident/0");
  await expect(page.getByLabel("Damage for Iron Sword")).toHaveValue("36");

  await page.getByLabel("Damage for Iron Sword").fill("45");
  await page
    .locator('[data-field="iron_sword.damage"]')
    .getByRole("button", { name: "Apply" })
    .click();
  await expect(page.getByTestId("revision")).toContainText("resident/1");
  await expect(page.locator('[data-field="iron_sword.dps"] output')).toHaveText("50");
  await expect(page.getByTestId("durability")).toContainText("Unsaved changes");

  await page.getByLabel("Name for Iron Sword").fill("Longsword\n+1");
  await page
    .locator('[data-field="iron_sword.name"]')
    .getByRole("button", { name: "Apply" })
    .click();
  await expect(page.getByTestId("revision")).toContainText("resident/2");
  await expect(page.getByLabel("Name for Iron Sword")).toHaveValue("Longsword\n+1");

  await page.getByLabel("Enabled for Iron Sword").uncheck();
  await page
    .locator('[data-field="iron_sword.enabled"]')
    .getByRole("button", { name: "Apply" })
    .click();
  await expect(page.getByTestId("revision")).toContainText("resident/3");
  await expect(page.getByLabel("Enabled for Iron Sword")).not.toBeChecked();

  page.once("dialog", async (dialog) => dialog.accept("edited.roproj"));
  await page.getByRole("button", { name: "Save As" }).click();
  await expect(page.locator(".notice.success")).toContainText("Save As complete");
  await expect(page.getByTestId("durability")).toContainText("Saved");

  await page.getByLabel("Damage for Iron Sword").fill("54");
  await page
    .locator('[data-field="iron_sword.damage"]')
    .getByRole("button", { name: "Apply" })
    .click();
  await expect(page.locator('[data-field="iron_sword.dps"] output')).toHaveText("60");
  page.once("dialog", async (dialog) => dialog.accept("edited.roproj"));
  await page.getByRole("button", { name: "Save As" }).click();
  await expect(page.getByRole("alert")).toContainText("never overwrites");
  await expect(page.getByTestId("durability")).toContainText("Unsaved changes");

  page.once("dialog", async (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Close" }).click();
  await expect(page.getByRole("heading", { name: "No project open" })).toBeVisible();
  await page.getByLabel("Saved project").selectOption("edited.roproj");
  await page.getByRole("button", { name: "Open project" }).click();
  await expect(page.getByRole("heading", { name: "Moonfall Balance" })).toBeVisible();
  await expect(page.getByTestId("revision")).toContainText("resident/0");
  await expect(page.getByLabel("Damage for Iron Sword")).toHaveValue("45");
  await expect(page.getByLabel("Name for Iron Sword")).toHaveValue("Longsword\n+1");
  await expect(page.getByLabel("Enabled for Iron Sword")).not.toBeChecked();
  await expect(page.locator('[data-field="iron_sword.dps"] output')).toHaveText("50");
  await expect(page.getByTestId("durability")).toContainText("Saved");

  await page.reload();
  await page.getByLabel("Saved project").selectOption("edited.roproj");
  page.once("dialog", async (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Open", exact: true }).click();
  await expect(page.getByLabel("Damage for Iron Sword")).toHaveValue("45");
  await expect(page.getByLabel("Name for Iron Sword")).toHaveValue("Longsword\n+1");
  await expect(page.getByLabel("Enabled for Iron Sword")).not.toBeChecked();
  await expect(page.locator('[data-field="iron_sword.dps"] output')).toHaveText("50");
  await expect(page.getByTestId("durability")).toContainText("Saved");
  await rm(externalRoot, { recursive: true, force: true });
});

test("monthly Budget uses the Rust formula lifecycle and preserves Date-bearing v2 meaning", async ({ page }) => {
  await page.goto("/");

  page.once("dialog", async (dialog) => dialog.accept());
  await page.getByRole("button", { name: "New Budget", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Monthly Budget" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Budget Items", exact: true })).toBeVisible();
  await expect(page.getByLabel("Due Date for Utilities")).toHaveValue("2026-09-15");

  const actual = page.getByLabel("Actual for Utilities");
  await actual.fill("200");
  await actual.locator("xpath=ancestor::form").getByRole("button", { name: "Apply" }).click();
  await expect(page.locator('[data-field="utilities.variance"] output')).toHaveText("20");

  await page.getByLabel("Collection", { exact: true }).selectOption("budget_summary");
  await expect(page.getByRole("heading", { name: "Budget Summary", exact: true })).toBeVisible();
  await expect(page.locator('[data-field="monthly_summary.actual_total"] output')).toHaveText("1,400");

  const remaining = page.getByLabel("Formula for Remaining for Monthly Summary");
  await remaining.fill("([monthly_summary.planned_total] - [utilities.actual])");
  await remaining.locator("xpath=ancestor::form").getByRole("button", { name: "Apply formula" }).click();
  await expect(page.locator('[data-field="monthly_summary.remaining"] output')).toHaveText("1,180");
  await expect(page.locator(".notice.success")).toContainText("Publication complete");

  page.once("dialog", async (dialog) => dialog.accept("september-budget.roproj"));
  await page.getByRole("button", { name: "Save As", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Save As complete");
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page.getByLabel("Saved project").selectOption("september-budget.roproj");
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await page.getByLabel("Collection", { exact: true }).selectOption("budget_summary");
  await expect(page.locator('[data-field="monthly_summary.remaining"] output')).toHaveText("1,180");
  await expect(page.getByLabel("Month for Monthly Summary")).toHaveValue("2026-09-01");
});

async function materializeBrowserProject(
  page: Page,
  name: string,
  destination: string,
): Promise<void> {
  const bundle = await page.evaluate(
    async (projectName) =>
      new Promise<number[]>((resolve, reject) => {
        const open = indexedDB.open("tachiko-designer-projects");
        open.addEventListener("error", () => {
          reject(new Error("Could not open the browser project store.", { cause: open.error }));
        });
        open.addEventListener("success", () => {
          const request = open.result
            .transaction("projects", "readonly")
            .objectStore("projects")
            .get(projectName);
          request.addEventListener("error", () => {
            reject(new Error("Could not read the browser project.", { cause: request.error }));
          });
          request.addEventListener("success", () => {
            const record = request.result as { bytes: ArrayBuffer } | undefined;
            if (record === undefined) {
              reject(new Error(`Missing browser project '${projectName}'.`));
              return;
            }
            resolve([...new Uint8Array(record.bytes)]);
          });
        });
      }),
    name,
  );
  for (const file of decodeProjectBundle(Uint8Array.from(bundle))) {
    const path = join(destination, file.path);
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, file.bytes);
  }
}

function decodeProjectBundle(bytes: Uint8Array): Array<{ path: string; bytes: Uint8Array }> {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const decoder = new TextDecoder();
  let offset = 8;
  const count = view.getUint32(offset, true);
  offset += 4;
  const files = [];
  for (let index = 0; index < count; index += 1) {
    const pathLength = view.getUint16(offset, true);
    offset += 2;
    const byteLength = view.getUint32(offset, true);
    offset += 4;
    const path = decoder.decode(bytes.slice(offset, offset + pathLength));
    offset += pathLength;
    const content = bytes.slice(offset, offset + byteLength);
    offset += byteLength;
    files.push({ path, bytes: content });
  }
  return files;
}
