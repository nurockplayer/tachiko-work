import { test, expect } from "@playwright/test";
import { startDashboard } from "../src/server/application.js";
import { fixture } from "../tests/operational/github-fixture.js";

test("shared Worker UI rejects executable source URLs and escapes href attributes", async ({ page }) => {
  const remote = fixture();
  const app = await startDashboard(remote.options());
  try {
    remote.data.comments[0]!.html_url = "javascript:window.__sourceLinkExecuted=true";
    await page.goto(app.origin);
    await expect(page.getByTestId("attention")).toContainText("HOLD");
    await expect(page.getByRole("link", { name: "HOLD", exact: true })).toHaveCount(0);

    remote.data.comments[0]!.html_url = 'https://github.com/" data-injected="yes';
    await page.getByRole("button", { name: "Refresh" }).click();
    const link = page.getByRole("link", { name: "HOLD", exact: true });
    await expect(link).toHaveCount(1);
    await expect(link).not.toHaveAttribute("data-injected");
    await expect(link).toHaveAttribute("href", remote.data.comments[0]!.html_url);
    expect(remote.violations).toEqual([]);
  } finally { await app.close(); }
});
