import { test as base, expect } from "@playwright/test";
import { startDashboard, type RunningDashboard } from "../src/server/application.js";
import { fixture, MAIN, HEAD, SECRET, watchBody } from "../tests/operational/github-fixture.js";

type Remote = ReturnType<typeof fixture>;
const test = base.extend<{ remote: Remote; app: RunningDashboard }>({
  remote: async ({}, use) => { const remote = fixture(); await use(remote); expect(remote.violations).toEqual([]); },
  app: async ({ remote }, use) => {
    const app = await startDashboard(remote.options());
    try { await use(app); } finally { await app.close(); }
  },
});
const headings = ["Executive strip", "Delivery command center", "Current work", "Recent activity", "Authority & attention"];

test("real raw-source -> server -> browser journey renders five source-linked surfaces", async ({ page, app, remote }) => {
  await page.goto(app.origin);
  for (const heading of headings) await expect(page.getByRole("heading", { name: heading, exact: true })).toBeVisible();
  await expect(page.getByTestId("main-sha")).toHaveText(MAIN);
  const lane = page.getByTestId("delivery-229-322");
  await expect(lane).toContainText(remote.data.issues[0]!.title);
  await expect(lane).toContainText(remote.data.pulls[0]!.title);
  await expect(lane.locator(`a[href="${remote.web}/issues/229"]`)).toBeVisible();
  await expect(lane.locator(`a[href="${remote.web}/pull/322"]`)).toBeVisible();
  await expect(page.getByTestId("delivery-231-none")).toContainText("Independent issue");
  await expect(page.getByTestId("current-work")).toContainText("229");
  await expect(page.getByTestId("recent-activity")).toContainText("Previously merged change");
  await expect(page.getByTestId("attention")).toContainText("HOLD");
  await expect(page.getByTestId("attention").locator(`a[href="${remote.web}/pull/322#issuecomment-700"]`).first()).toBeVisible();
  await expect(page.getByRole("button", { name: /merge|dispatch|run agent/i })).toHaveCount(0);
  expect(remote.requests.length).toBeGreaterThan(0);
});

test("a refresh changes real upstream data and preserves keyboard focus", async ({ page, app, remote }) => {
  await page.goto(app.origin);
  await expect(page.getByTestId("delivery-229-322")).toContainText(remote.data.issues[0]!.title);
  const refresh = page.getByRole("button", { name: "Refresh", exact: true });
  await refresh.focus();
  remote.data.issues[0]!.title = "Changed at the GitHub transport boundary";
  await refresh.press("Enter");
  await expect(page.getByTestId("delivery-229-322")).toContainText(remote.data.issues[0]!.title);
  await expect(page.getByRole("button", { name: "Refresh", exact: true })).toBeFocused();
});

test("partial dependencies render Unknown while independent Issue and PR facts remain visible", async ({ page, app, remote }) => {
  remote.partial.add("dependencies");
  await page.goto(app.origin);
  await expect(page.getByTestId("delivery-229-322")).toContainText(remote.data.issues[0]!.title);
  await expect(page.getByTestId("current-work")).toContainText(/Unknown/i);
  await expect(page.getByTestId("current-work")).toContainText(/partial/i);
  await expect(page.getByTestId("attention")).toContainText("HOLD");
});

test("browser transport failure clears old current content; later success restores only new data", async ({ page, app, remote }) => {
  await page.goto(app.origin);
  await expect(page.getByTestId("main-sha")).toHaveText(MAIN);
  await page.route("**/api/projection", route => route.abort("failed"));
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(page.getByTestId("main-sha")).toContainText(/Unknown/i);
  await expect(page.getByTestId("deliveries")).not.toContainText(remote.data.issues[0]!.title);
  await expect(page.getByTestId("recent-activity")).not.toContainText("Previously merged change");
  await expect(page.getByTestId("attention")).not.toContainText("HOLD");
  remote.data.main = "5".repeat(40);
  remote.data.issues[0]!.title = "New recovery observation";
  remote.data.comments = [remote.comment(710, watchBody("GREEN", "none", HEAD, remote.data.main))];
  await page.unroute("**/api/projection");
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(page.getByTestId("main-sha")).toHaveText(remote.data.main);
  await expect(page.getByTestId("delivery-229-322")).toContainText("New recovery observation");
});

test("AMBER/none does not become a clean bill of health; required remains positive", async ({ page, app, remote }) => {
  remote.data.comments = [remote.comment(700, watchBody("AMBER", "none"))];
  await page.goto(app.origin);
  await expect(page.getByTestId("main-sha")).toHaveText(MAIN);
  const attention = page.getByTestId("attention");
  await expect(attention).not.toContainText(/HOLD|healthy|no blockers|merge.ready|no action required|HUMAN_ACTION: none/i);
  remote.data.comments = [remote.comment(700, watchBody("AMBER", "required"))];
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(attention).toContainText(/required/i);
  await expect(attention).not.toContainText("HOLD");
});

test("390px and reduced motion stay readable with long titles and full identities", async ({ page, app, remote }) => {
  remote.data.issues[0]!.title = "VeryLongUnbrokenIssueTitle".repeat(16);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto(app.origin);
  for (const heading of headings) await expect(page.getByRole("heading", { name: heading, exact: true })).toBeVisible();
  await expect(page.getByTestId("main-sha")).toHaveText(MAIN);
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)).toBeLessThanOrEqual(1);
  expect(await page.evaluate(() => document.getAnimations().filter(animation => animation.playState === "running").length)).toBe(0);
  await expect(page.getByRole("button", { name: "Refresh", exact: true })).toBeEnabled();
});

test("real credential and hostile upstream title never become browser secrets, requests, or executable HTML", async ({ page, context, app, remote }) => {
  const hostile = '<img src=x onerror="window.__dashboardXss=true">';
  remote.data.issues[0]!.title = hostile;
  const traffic: string[] = []; const offOrigin: string[] = []; const unreadable: string[] = []; const pending: Promise<void>[] = [];
  await context.route("**/*", async route => {
    const request = route.request();
    traffic.push(request.url(), JSON.stringify(request.headers()), request.postData() ?? "");
    if (new URL(request.url()).origin !== app.origin) { offOrigin.push(request.url()); await route.abort(); }
    else await route.continue();
  });
  page.on("response", response => {
    pending.push(response.text().then(body => { traffic.push(body); }).catch(() => { unreadable.push(response.url()); }));
  });
  await page.goto(app.origin);
  await expect(page.getByTestId("delivery-229-322")).toContainText(hostile);
  await expect(page.getByTestId("delivery-229-322").locator("img")).toHaveCount(0);
  expect(await page.evaluate(() => Reflect.get(window, "__dashboardXss"))).toBeUndefined();
  await Promise.all(pending);
  expect(traffic.join("\n")).not.toContain(SECRET);
  expect(await page.content()).not.toContain(SECRET);
  expect(await page.evaluate(() => JSON.stringify({ local: { ...localStorage }, session: { ...sessionStorage }, cookie: document.cookie }))).not.toContain(SECRET);
  expect(offOrigin).toEqual([]);
  expect(unreadable).toEqual([]);
  expect(remote.requests.some(request => request.headers.get("authorization")?.includes(SECRET))).toBe(true);
});
