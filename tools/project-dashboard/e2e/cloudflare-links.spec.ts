import { test, expect } from "@playwright/test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { unstable_dev } from "wrangler";
import { startDashboard } from "../src/server/application.js";
import { dashboardHtml } from "../src/server/shared.js";
import { DEFAULT_REPO, fixture, SECRET, STEWARD } from "../tests/operational/github-fixture.js";

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

test("local Worker browser journey rejects hostile links and refreshes new source data", async ({ page }) => {
  const assets = await mkdtemp(join(tmpdir(), "tachiko-dashboard-worker-"));
  await writeFile(join(assets, "index.html"), dashboardHtml);
  const app = await unstable_dev("tests/cloudflare/worker-browser-fixture.ts", {
    config: "wrangler.jsonc",
    local: true,
    ip: "127.0.0.1",
    port: 0,
    inspectorPort: 0,
    persist: false,
    assets,
    vars: {
      GITHUB_TOKEN: SECRET,
      DASHBOARD_REPOSITORY: DEFAULT_REPO,
      DASHBOARD_TRUSTED_STEWARD_LOGINS: STEWARD,
    },
    logLevel: "error",
    experimental: {
      disableExperimentalWarning: true,
      disableDevRegistry: true,
      watch: false,
    },
  });
  const origin = `http://${app.address}:${app.port}`;
  try {
    await page.goto(origin);
    await expect(page.getByTestId("attention")).toContainText("HOLD");
    await expect(page.getByRole("link", { name: "HOLD", exact: true })).toHaveCount(0);

    expect(await page.evaluate(async () => (await fetch("/__test__/set-safe")).status)).toBe(204);
    await page.getByRole("button", { name: "Refresh" }).click();
    const link = page.getByRole("link", { name: "HOLD", exact: true });
    await expect(link).toHaveCount(1);
    await expect(link).toHaveAttribute("href", "https://github.com/acceptance-fixture/dashboard/pull/322#issuecomment-700");

    expect(await page.evaluate(async () => (await fetch("/__test__/set-quote")).status)).toBe(204);
    await page.getByRole("button", { name: "Refresh" }).click();
    await expect(link).not.toHaveAttribute("data-injected");
    await expect(link).toHaveAttribute("href", 'https://github.com/" data-injected="yes');
  } finally {
    await app.stop();
    await rm(assets, { recursive: true, force: true });
  }
});
