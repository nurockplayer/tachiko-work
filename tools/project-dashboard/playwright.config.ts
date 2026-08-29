import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  use: {
    baseURL: "http://127.0.0.1:4178",
    trace: "retain-on-failure",
  },
  webServer: {
    command: "DASHBOARD_FIXTURE=pressure-tests pnpm start",
    url: "http://127.0.0.1:4178/api/health",
    reuseExistingServer: false,
    timeout: 30_000,
  },
});
