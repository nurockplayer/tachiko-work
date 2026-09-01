import { expect, test } from "@playwright/test";

test("renders five source-linked read-only control-room surfaces", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Live Project Control Room" })).toBeVisible();
  await expect(page.locator("[data-surface]" )).toHaveCount(5);
  await expect(page.getByText("Command center")).toBeVisible();
  await expect(page.getByText("Critical path · current work")).toBeVisible();
  await expect(page.getByText("Recent merges & activity")).toBeVisible();
  await expect(page.getByText("Attention & reconciliation")).toBeVisible();
  await expect(page.getByText("Automated browser · satisfied")).toBeVisible();
  await expect(page.getByText("Perceptual review · unknown").first()).toBeVisible();
  await expect(page.getByRole("button", { name: /merge|dispatch|run agent/i })).toHaveCount(0);
  await expect(page.locator("a[data-evidence-class='direct']").first()).toBeVisible();
});

test("is usable at mobile width and disables decorative motion", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/");
  await expect(page.locator("[data-surface='delivery-command-center']")).toBeVisible();

  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(overflow).toBeLessThanOrEqual(0);
  await expect(page.locator(".ambient-grid")).toHaveCSS("animation-name", "none");
});

test("renders source failure as partial and Unknown", async ({ page }) => {
  await page.route("**/api/project", async (route) => {
    const response = await route.fetch();
    const projection = (await response.json()) as Record<string, unknown>;
    await route.fulfill({
      response,
      json: {
        ...projection,
        fetchHealth: "partial",
        executive: {
          ...(projection.executive as Record<string, unknown>),
          mainSha: {
            state: "unknown",
            value: "Unknown",
            source: { label: "Live main", url: "https://github.example", evidenceClass: "direct" },
          },
        },
        humanAction: { state: "unknown", reason: "observation-incomplete", label: "Human action state Unknown", sources: [] },
      },
    });
  });
  await page.goto("/");

  await expect(page.getByText("PARTIAL", { exact: true })).toBeVisible();
  await expect(page.getByText("Unknown", { exact: true }).first()).toBeVisible();
});

test("refresh preserves keyboard focus and announces completion", async ({ page }) => {
  await page.goto("/");
  const refresh = page.getByRole("button", { name: "Refresh observation" });
  await refresh.focus();
  await refresh.press("Enter");

  await expect(refresh).toBeFocused();
  await expect(page.locator("[aria-live='polite']")).toContainText("Observation refreshed");
});
