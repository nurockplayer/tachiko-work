import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const workerPath = fileURLToPath(new URL("../../src/cloudflare/worker.ts", import.meta.url));
const wranglerPath = fileURLToPath(new URL("../../wrangler.jsonc", import.meta.url));

describe("#342 Cloudflare deployment seam", () => {
  it("has the bounded Worker and Wrangler integration required before cloud behavior can be exercised", () => {
    expect(
      { worker: existsSync(workerPath), wrangler: existsSync(wranglerPath) },
      "qualified RED: #229 is merged, but the Cloudflare Worker/Wrangler seam does not exist yet",
    ).toEqual({ worker: true, wrangler: true });
  });

  it("does not introduce durable storage bindings into the bounded Dashboard deployment", () => {
    if (!existsSync(wranglerPath)) return;
    const config = readFileSync(wranglerPath, "utf8");
    expect(config).toMatch(/\"main\"\s*:\s*\"src\/cloudflare\/worker\.ts\"/);
    expect(config).toMatch(/\"binding\"\s*:\s*\"ASSETS\"/);
    expect(config).not.toMatch(/\"(?:kv_namespaces|durable_objects|r2_buckets|d1_databases)\"\s*:/);
  });
});
