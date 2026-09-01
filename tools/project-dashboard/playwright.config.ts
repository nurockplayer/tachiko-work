import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  retries: 0,
  reporter: "line",
  use: {
    baseURL: "http://127.0.0.1:4174",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  webServer: {
    command: "DASHBOARD_FIXTURE=healthy DASHBOARD_PORT=4174 pnpm serve",
    url: "http://127.0.0.1:4174/api/project",
    reuseExistingServer: false,
  },
});
