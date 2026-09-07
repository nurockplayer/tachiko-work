import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: "./e2e",
  testMatch: ["**/operational.spec.ts", "**/operational-security.spec.ts"],
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 15_000,
  expect: { timeout: 3_000 },
  reporter: "list",
  use: { browserName: "chromium", headless: true, serviceWorkers: "block", trace: "retain-on-failure" },
});
