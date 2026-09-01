import { expect, test } from "@playwright/test";

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
    const projection = (await response.json()) as {
      executive: Record<string, unknown>;
      deliveries: ({ issue: Record<string, unknown> | null } & Record<string, unknown>)[];
    } & Record<string, unknown>;
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

test("distinguishes a complete empty linked-Issue observation from Unknown", async ({ page }) => {
  await page.route("**/api/project", async (route) => {
    const response = await route.fetch();
    const projection = (await response.json()) as {
      deliveries: ({ pullRequest: Record<string, unknown> | null } & Record<string, unknown>)[];
    } & Record<string, unknown>;
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
    const projection = (await response.json()) as {
      deliveries: ({ pullRequest: Record<string, unknown> | null } & Record<string, unknown>)[];
    } & Record<string, unknown>;
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
