import { test as base, expect } from "@playwright/test";
import { startDashboard, type RunningDashboard } from "../src/server/application.js";
import { fixture, MAIN } from "../tests/operational/github-fixture.js";

type Remote = ReturnType<typeof fixture>;

const test = base.extend<{ remote: Remote; app: RunningDashboard }>({
  remote: async ({}, use) => {
    const remote = fixture();
    await use(remote);
    expect(remote.violations).toEqual([]);
  },
  app: async ({ remote }, use) => {
    const app = await startDashboard(remote.options());
    try { await use(app); } finally { await app.close(); }
  },
});

test("every source-backed Dashboard surface exposes usable provenance", async ({ page, app, remote }) => {
  await page.goto(app.origin);

  await expect(page.getByTestId("main-sha").locator(`a[href="${remote.web}/commit/${MAIN}"]`)).toBeVisible();
  await expect(page.getByTestId("delivery-229-322").locator(`a[href="${remote.web}/issues/229"]`)).toBeVisible();
  await expect(page.getByTestId("delivery-229-322").locator(`a[href="${remote.web}/pull/322"]`)).toBeVisible();
  await expect(page.getByTestId("current-work").locator(`a[href="${remote.web}/issues"]`)).toBeVisible();
  await expect(page.getByTestId("recent-activity").locator(`a[href^="${remote.web}/pulls"]`)).toBeVisible();
  await expect(page.getByTestId("attention").locator(`a[href="${remote.web}/pull/322#issuecomment-700"]`).first()).toBeVisible();
});
