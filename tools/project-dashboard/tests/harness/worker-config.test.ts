import { readFileSync } from "node:fs";
import { expect, it } from "vitest";

it("keeps all public ingress disabled and routes assets through the Worker", () => {
  const config = JSON.parse(readFileSync(new URL("../../wrangler.jsonc", import.meta.url), "utf8"));
  expect(config.workers_dev).toBe(false);
  expect(config.preview_urls).toBe(false);
  expect(config.routes).toEqual([]);
  expect(config.assets).toMatchObject({ directory: "./dist/assets", binding: "ASSETS", run_worker_first: true });
  expect(config.vars).not.toHaveProperty("GITHUB_TOKEN");
  expect(config).not.toHaveProperty("account_id");
  expect(config.observability.enabled).toBe(false);
});
