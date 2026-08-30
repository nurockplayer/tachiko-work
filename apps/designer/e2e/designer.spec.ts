import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

test("Moonfall Number edit selectively refreshes DPS and rejects an invalid candidate", async ({
  page,
}) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Moonfall Balance" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Weapons" })).toBeVisible();
  await expect(page.getByTestId("revision")).toContainText("resident/0");
  await expect(page.getByLabel("Damage for Iron Sword")).toHaveValue("36");
  await expect(page.locator('[data-field="iron_sword.dps"] output')).toHaveText("40");
  await expect(page.getByTestId("control-value")).toHaveText("200");

  await page.getByLabel("Damage for Iron Sword").fill("45");
  await page
    .locator('[data-field="iron_sword.damage"]')
    .getByRole("button", { name: "Apply" })
    .click();

  await expect(page.getByTestId("revision")).toContainText("resident/1");
  await expect(page.getByLabel("Damage for Iron Sword")).toHaveValue("45");
  await expect(page.locator('[data-field="iron_sword.dps"] output')).toHaveText("50");
  await expect(page.getByTestId("control-value")).toHaveText("200");
  await expect(page.locator(".notice.success")).toContainText("Publication complete");

  await page.getByLabel("Attack Interval for Iron Sword").fill("0");
  await page
    .locator('[data-field="iron_sword.attack_interval"]')
    .getByRole("button", { name: "Apply" })
    .click();

  await expect(page.getByRole("alert")).toContainText("Edit not published");
  await expect(page.getByRole("alert")).toContainText("formula.division_by_zero");
  await expect(page.getByTestId("revision")).toContainText("resident/1");
  await expect(page.getByLabel("Attack Interval for Iron Sword")).toHaveValue("0.9");
  await expect(page.getByLabel("Damage for Iron Sword")).toHaveValue("45");
  await expect(page.locator('[data-field="iron_sword.dps"] output')).toHaveText("50");
  await expect(page.getByTestId("control-value")).toHaveText("200");
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

  await page.getByLabel("Name for Iron Sword").fill("Longsword");
  await page
    .locator('[data-field="iron_sword.name"]')
    .getByRole("button", { name: "Apply" })
    .click();
  await expect(page.getByTestId("revision")).toContainText("resident/2");
  await expect(page.getByLabel("Name for Iron Sword")).toHaveValue("Longsword");

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
  await expect(page.getByLabel("Name for Iron Sword")).toHaveValue("Longsword");
  await expect(page.getByLabel("Enabled for Iron Sword")).not.toBeChecked();
  await expect(page.locator('[data-field="iron_sword.dps"] output')).toHaveText("50");
  await expect(page.getByTestId("durability")).toContainText("Saved");

  await page.reload();
  await page.getByLabel("Saved project").selectOption("edited.roproj");
  page.once("dialog", async (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Open", exact: true }).click();
  await expect(page.getByLabel("Damage for Iron Sword")).toHaveValue("45");
  await expect(page.getByLabel("Name for Iron Sword")).toHaveValue("Longsword");
  await expect(page.getByLabel("Enabled for Iron Sword")).not.toBeChecked();
  await expect(page.locator('[data-field="iron_sword.dps"] output')).toHaveText("50");
  await expect(page.getByTestId("durability")).toContainText("Saved");
  await rm(externalRoot, { recursive: true, force: true });
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
