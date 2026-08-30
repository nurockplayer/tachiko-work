import { expect, test } from "@playwright/test";

test("renders the five source-linked control-room surfaces without false merge-ready state", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { level: 1, name: /live control room/i })).toBeVisible();
  for (const heading of ["Executive strip", "Delivery command center", "Current work", "Recent merges", "Authority & attention"]) {
    await expect(page.getByRole("heading", { name: heading })).toBeVisible();
  }

  await expect(page.getByRole("link", { name: /roadmap source/i })).toBeVisible();
  const dashboardLane = page.getByRole("article", { name: /issue 169/i });
  await expect(dashboardLane).toContainText(/inconsistent/i);
  await expect(dashboardLane).toContainText("Checks were not observed for the current PR head");
  await expect(dashboardLane).toContainText(/base tip/i);
  await expect(dashboardLane).toContainText(/merge base/i);
  await expect(dashboardLane).toContainText(/handoff state/i);
  await expect(dashboardLane).toContainText(/handoff checked main/i);
  await expect(dashboardLane.locator(".definition").filter({ hasText: "Handoff updated" }).locator("time"))
    .toHaveAttribute("datetime", "2026-08-30T00:00:00.000Z");
  await expect(dashboardLane).not.toContainText("merge ready");
  await expect(page.getByRole("article", { name: /issue 163/i })).toContainText(/review fix/i);
  await expect(page.getByRole("list", { name: /current work sequence/i })).toContainText("Independent tooling / research lane");
  await expect(page.locator(".attention-panel > .section-heading .status-badge")).toHaveClass(/cyber-badge--green/);
  await expect(page.getByRole("button", { name: /merge|run agent|dispatch/i })).toHaveCount(0);

  const refresh = page.getByRole("button", { name: /refresh projection/i });
  await refresh.focus();
  await refresh.click();
  await expect(page.getByRole("button", { name: /refresh projection/i })).toBeFocused();
  await expect(page.getByRole("status")).toHaveText(/projection refreshed/i);
});

test("remains usable at mobile width and fully suppresses decorative motion", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/");

  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(overflow).toBeLessThanOrEqual(1);
  await expect(page.getByRole("article", { name: /issue 187/i })).toBeVisible();
  const animationName = await page.locator(".status-pulse").first().evaluate((element) => getComputedStyle(element).animationName);
  expect(animationName).toBe("none");
});

test("renders source failure as explicit partial and unknown state", async ({ page }) => {
  await page.route("**/api/projection", async (route) => {
    const response = await route.fetch();
    const projection = (await response.json()) as Record<string, unknown> & {
      repo: Record<string, unknown>;
      attention: Record<string, unknown>;
      currentWork: Record<string, unknown>;
    };
    projection.repo.fetchHealth = "partial";
    projection.repo.mainSha = null;
    projection.repo.failures = ["GitHub pull-request observation failed"];
    projection.repo.productHorizon = null;
    projection.currentWork.currentHorizon = [];
    projection.currentWork.independent = [];
    projection.currentWork.unclassified = ["issue-187", "issue-169", "issue-163"];
    projection.currentWork.horizonStatus = "unknown";
    projection.currentWork.dependencyHealth = "unknown";
    projection.attention.humanActionRequired = null;
    projection.attention.reasons = ["GitHub pull-request observation failed"];
    await route.fulfill({ response, json: projection });
  });
  await page.goto("/");

  await expect(page.getByTestId("fetch-health")).toHaveText(/partial/i);
  await expect(page.getByTestId("main-sha")).toHaveText(/unknown/i);
  await expect(page.getByTestId("human-action")).toHaveText(/unknown/i);
  await expect(page.getByRole("list", { name: /current work sequence/i })).toContainText("Horizon classification Unknown");
  await expect(page.getByText("GitHub pull-request observation failed", { exact: true })).toBeVisible();
});
