import { expect, test } from "@playwright/test";

import type { DashboardProjection } from "../src/shared/model.js";

test("renders five source-linked read-only control-room surfaces", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Live Project Control Room" })).toBeVisible();
  await expect(page.getByText("TW / LIVE · READ ONLY", { exact: true })).toBeVisible();
  await expect(page.locator("[data-surface]")).toHaveCount(5);
  await expect(page.getByText("Delivery command center")).toBeVisible();
  await expect(page.getByText("Critical path / current work")).toBeVisible();
  await expect(page.getByText("Recent merges & activity")).toBeVisible();
  await expect(page.getByText("Authority / attention")).toBeVisible();
  await expect(page.getByText("MERGEABLE", { exact: true })).toHaveCount(2);
  await expect(page.getByRole("button", { name: /merge|dispatch|run agent/i })).toHaveCount(0);
  await expect(page.locator("a[data-source-kind='github']").first()).toHaveAttribute("rel", "noreferrer noopener");
});

test("is usable at mobile width and disables decorative motion", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/");

  const surfaces = page.locator("[data-surface]");
  await expect(surfaces).toHaveCount(5);
  for (let index = 0; index < 5; index += 1) {
    await expect(surfaces.nth(index)).toBeVisible();
  }
  const overflow = await page.evaluate(() =>
    document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(overflow).toBeLessThanOrEqual(0);
  await expect(page.locator(".ambient-grid")).toHaveCSS("animation-name", "none");
});

test("renders source failure as partial and Unknown", async ({ page }) => {
  await page.route("**/api/project", async (route) => {
    const response = await route.fetch();
    const projection = (await response.json()) as DashboardProjection;
    await route.fulfill({
      response,
      json: {
        ...projection,
        fetchHealth: "partial",
        executive: {
          ...projection.executive,
          mainSha: {
            value: null,
            availability: "partial",
            source: { label: "Live main", url: "https://github.example/main", kind: "github" },
          },
          humanAction: {
            value: null,
            availability: "partial",
            source: { label: "Steward watches", url: "https://github.example", kind: "structured" },
          },
        },
        deliveries: projection.deliveries.map((lane, index) =>
          index !== 0 || lane.issue === null
            ? lane
            : {
                ...lane,
                issue: {
                  ...lane.issue,
                  blockedBy: [],
                  dependenciesAvailability: "partial",
                  availability: "partial",
                },
              }),
      },
    });
  });
  await page.goto("/");

  await expect(page.getByText("PARTIAL", { exact: true })).toBeVisible();
  await expect(page.getByText("Unknown", { exact: true }).first()).toBeVisible();
  await expect(
    page.locator(".identity-item").filter({ hasText: "BLOCKED BY" }).getByText("Unknown", { exact: true }),
  ).toBeVisible();
});

test("preserves unavailable state for wholly unavailable counts", async ({ page }) => {
  await page.route("**/api/project", async (route) => {
    const response = await route.fetch();
    const projection = (await response.json()) as DashboardProjection;
    await route.fulfill({
      response,
      json: {
        ...projection,
        executive: {
          ...projection.executive,
          activeCount: { ...projection.executive.activeCount, value: null, availability: "unavailable" },
          readyCount: { ...projection.executive.readyCount, value: null, availability: "unavailable" },
        },
      },
    });
  });
  await page.goto("/");

  const counts = page.locator(".executive-cell").filter({ hasText: "ACTIVE / READY" });
  await expect(counts.getByText("Unknown", { exact: true })).toBeVisible();
  await expect(counts.getByText("unavailable", { exact: true })).toBeVisible();
});

test("shows direct GitHub and Steward facts without a final verdict", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByText("GitHub native fields · displayed verbatim")).toBeVisible();
  await expect(page.getByText("2222222222222222222222222222222222222222", { exact: true }).first()).toBeVisible();
  await expect(page.getByText("GREEN · human action none")).toBeVisible();
  await expect(
    page.locator(".executive-cell").filter({ hasText: "HUMAN ACTION" }).getByRole("link", { name: "Steward watch" }),
  ).toBeVisible();
  await expect(page.getByText(/final merge verdict/)).toBeVisible();
  await expect(page.getByText(/merge ready|merge-ready|can_merge/i)).toHaveCount(0);
});

test("keeps pull identity complete when only checks are partial", async ({ page }) => {
  await page.route("**/api/project", async (route) => {
    const response = await route.fetch();
    const projection = (await response.json()) as DashboardProjection;
    await route.fulfill({
      response,
      json: {
        ...projection,
        deliveries: projection.deliveries.map((lane, index) =>
          index !== 0 || lane.pullRequest === null
            ? lane
            : {
                ...lane,
                pullRequest: {
                  ...lane.pullRequest,
                  availability: "partial",
                  checks: { availability: "partial", items: [] },
                },
              }),
      },
    });
  });
  await page.goto("/");

  await expect(
    page.locator(".fact-group").filter({ hasText: "Pull request identity" })
      .getByText("complete", { exact: true }),
  ).toBeVisible();
  await expect(
    page.locator(".fact-group").filter({ hasText: "Checks" })
      .getByText("partial", { exact: true }),
  ).toBeVisible();
});

test("keeps PR core visible while a missing head clears only head-dependent current facts", async ({ page }) => {
  await page.route("**/api/project", async (route) => {
    const response = await route.fetch();
    const projection = (await response.json()) as DashboardProjection;
    await route.fulfill({
      response,
      json: {
        ...projection,
        deliveries: projection.deliveries.map((lane, index) =>
          index !== 0 || lane.pullRequest === null
            ? lane
            : {
                ...lane,
                pullRequest: {
                  ...lane.pullRequest,
                  headSha: null,
                  headAvailability: "partial",
                  mergeable: null,
                  mergeStateStatus: null,
                  reviewDecision: null,
                  nativeAvailability: "partial",
                  checks: { availability: "partial", items: [] },
                  reviews: { availability: "partial", items: [] },
                  handoff: { status: "unknown", value: null, reason: "PR head identity Unknown", source: null },
                  stewardWatch: { status: "unknown", value: null, reason: "PR head identity Unknown", source: null },
                },
              }),
      },
    });
  });
  await page.goto("/");

  const card = page.locator(".lane-card").first();
  await expect(card.locator(".fact-group").filter({ hasText: "Pull request identity" }).getByText("OPEN", { exact: true }))
    .toBeVisible();
  await expect(card.locator(".fact-group").filter({ hasText: "Pull request head" }).getByText("Unknown", { exact: true }))
    .toBeVisible();
  await expect(card.locator(".fact-group").filter({ hasText: "GitHub native fields" }).getByText("Unknown", { exact: true }))
    .toHaveCount(3);
});

test("renders fully observed absent Issue metadata as none observed", async ({ page }) => {
  await page.route("**/api/project", async (route) => {
    const response = await route.fetch();
    const projection = (await response.json()) as DashboardProjection;
    const firstLane = projection.deliveries[0];
    await route.fulfill({
      response,
      json: firstLane?.issue === null || firstLane?.issue === undefined
        ? projection
        : {
            ...projection,
            deliveries: [{
              ...firstLane,
              issue: {
                ...firstLane.issue,
                labels: [],
                labelsAvailability: "complete",
                milestone: null,
                milestoneAvailability: "complete",
              },
            }],
          },
    });
  });
  await page.goto("/");

  await expect(
    page.locator(".identity-item").filter({ hasText: "LABELS" }).getByText("None observed", { exact: true }),
  ).toBeVisible();
  await expect(
    page.locator(".identity-item").filter({ hasText: "MILESTONE" }).getByText("None observed", { exact: true }),
  ).toBeVisible();
});

test("renders a fully observed empty delivery set as none observed", async ({ page }) => {
  await page.route("**/api/project", async (route) => {
    const response = await route.fetch();
    const projection = (await response.json()) as DashboardProjection;
    await route.fulfill({
      response,
      json: { ...projection, deliveries: [], deliveriesAvailability: "complete" },
    });
  });
  await page.goto("/");

  await expect(page.getByText("No active delivery lanes observed", { exact: true })).toBeVisible();
  await expect(page.getByText("Delivery observation Unknown", { exact: true })).toHaveCount(0);
});

test("distinguishes a complete empty linked-Issue observation from Unknown", async ({ page }) => {
  await page.route("**/api/project", async (route) => {
    const response = await route.fetch();
    const projection = (await response.json()) as DashboardProjection;
    await route.fulfill({
      response,
      json: {
        ...projection,
        deliveries: projection.deliveries.map((lane, index) =>
          index !== 0 || lane.pullRequest === null
            ? lane
            : {
                ...lane,
                pullRequest: {
                  ...lane.pullRequest,
                  linkedIssueNumbers: [],
                  linkageAvailability: "complete",
                },
              }),
      },
    });
  });
  await page.goto("/");

  await expect(
    page.locator(".identity-item").filter({ hasText: "LINKED ISSUES" }).getByText("None observed", { exact: true }),
  ).toBeVisible();
});

test("renders an incomplete empty linked-Issue observation as Unknown", async ({ page }) => {
  await page.route("**/api/project", async (route) => {
    const response = await route.fetch();
    const projection = (await response.json()) as DashboardProjection;
    await route.fulfill({
      response,
      json: {
        ...projection,
        deliveries: projection.deliveries.map((lane, index) =>
          index !== 0 || lane.pullRequest === null
            ? lane
            : {
                ...lane,
                pullRequest: {
                  ...lane.pullRequest,
                  linkedIssueNumbers: [],
                  linkageAvailability: "partial",
                },
              }),
      },
    });
  });
  await page.goto("/");

  await expect(
    page.locator(".identity-item").filter({ hasText: "LINKED ISSUES" }).getByText("Unknown", { exact: true }),
  ).toBeVisible();
});

test("refresh preserves keyboard focus and announces completion", async ({ page }) => {
  await page.goto("/");
  const refresh = page.getByRole("button", { name: "Refresh observation" });
  await refresh.focus();
  await refresh.press("Enter");

  await expect(page.getByRole("button", { name: "Refresh observation" })).toBeFocused();
  await expect(page.locator("[aria-live='polite']")).toContainText("Observation refreshed");
});

test("refresh failure renders an Unknown current surface and recovery control", async ({ page }) => {
  let requests = 0;
  await page.route("**/api/project", async (route) => {
    requests += 1;
    if (requests === 1) await route.continue();
    else await route.fulfill({ status: 503, body: "unavailable" });
  });
  await page.goto("/");
  const refresh = page.getByRole("button", { name: "Refresh observation" });
  await refresh.focus();
  await refresh.press("Enter");

  await expect(page.getByRole("heading", { name: "Live Project Control Room" })).toBeVisible();
  await expect(refresh).toBeEnabled();
  await expect(refresh).toBeFocused();
  await expect(page.getByRole("alert")).toContainText("current observation is Unknown");
  await expect(
    page.locator(".executive-cell").filter({ hasText: "FETCH" }).getByText("UNAVAILABLE", { exact: true }).first(),
  ).toBeVisible();
  await expect(page.getByText("1111111111111111111111111111111111111111", { exact: true })).toHaveCount(0);
  await expect(page.locator(".lane-card")).toHaveCount(0);
  await expect(page.getByText("Delivery observation Unknown", { exact: true })).toBeVisible();
  await expect(page.locator("#critical-path .path-node")).toHaveCount(0);
  await expect(page.locator("#recent-activity .activity-item")).toHaveCount(0);
  await expect(page.getByText("Current observation unavailable", { exact: true })).toBeVisible();
  await expect(page.getByText("Observed native policy facts", { exact: true })).toHaveCount(0);
  await expect(page.locator(".availability-complete")).toHaveCount(0);
  await expect(page.locator(".structured-status").filter({ hasText: "current" })).toHaveCount(0);
  await expect(page.locator("[aria-live='polite']")).toContainText("refresh failed");
});
