import { test as base, expect } from "@playwright/test";
import { startDashboard, type RunningDashboard } from "../src/server/application.js";
import { fixture } from "../tests/operational/github-fixture.js";

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

test("Issue, PR, and recent-activity titles are rendered as inert text", async ({ page, app, remote }) => {
  const issueTitle = '<img src=x onerror="window.__issueXss=true">';
  const prTitle = '<img src=x onerror="window.__prXss=true">';
  const activityTitle = '<img src=x onerror="window.__activityXss=true">';
  remote.data.issues[0]!.title = issueTitle;
  remote.data.pulls[0]!.title = prTitle;
  remote.data.activity[0]!.title = activityTitle;

  await page.goto(app.origin);

  const delivery = page.getByTestId("delivery-229-322");
  await expect(delivery).toContainText(issueTitle);
  await expect(delivery).toContainText(prTitle);
  await expect(delivery.locator("img")).toHaveCount(0);

  const activity = page.getByTestId("recent-activity");
  await expect(activity).toContainText(activityTitle);
  await expect(activity.locator("img")).toHaveCount(0);

  expect(await page.evaluate(() => ({
    issue: Reflect.get(window, "__issueXss"),
    pr: Reflect.get(window, "__prXss"),
    activity: Reflect.get(window, "__activityXss"),
  }))).toEqual({ issue: undefined, pr: undefined, activity: undefined });
});
