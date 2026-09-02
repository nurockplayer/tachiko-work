import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  testMatch: "experimental-client.spec.ts",
  fullyParallel: false,
  retries: 0,
  reporter: "line",
  use: {
    baseURL: "http://127.0.0.1:4174",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  webServer: {
    command:
      "pnpm exec vite --host 127.0.0.1 --port 4174 ../../examples/experimental-designer-client",
    url: "http://127.0.0.1:4174",
    reuseExistingServer: false,
  },
});
