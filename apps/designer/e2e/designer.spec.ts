import { expect, test } from "@playwright/test";

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
