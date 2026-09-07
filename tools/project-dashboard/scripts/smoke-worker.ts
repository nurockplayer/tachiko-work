import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import { unstable_dev } from "wrangler";
import { chromium } from "@playwright/test";

// Real local workerd + Static Assets. No cloud account or GitHub credential.
const app = await unstable_dev("src/cloudflare/worker.ts", {
  config: "wrangler.jsonc", local: true, ip: "127.0.0.1", port: 0,
  inspectorPort: 0, persist: false, logLevel: "error",
  vars: { GITHUB_TOKEN: "" },
  experimental: { disableExperimentalWarning: true, disableDevRegistry: true, watch: false },
});
const origin = `http://${app.address}:${app.port}`;
const request = (path: string, init?: RequestInit) => fetch(new URL(path, origin), init);
try {
  assert.deepEqual(await readdir("dist/assets"), ["index.html"]);
  const html = await readFile("dist/assets/index.html", "utf8");
  assert(!html.includes("GITHUB_TOKEN"));
  assert(!html.includes("api.github.com"));
  const response = await request("/");
  assert.equal(response.status, 200);
  assert.equal(await response.text(), html);
  assert((await request("/api/projection")).status >= 400);
  assert.equal((await request("/api/merge")).status, 404);
  assert.equal((await request("/api/projection", { method: "POST" })).status, 405);
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage();
    const external: string[] = [];
    page.on("request", request => { if (new URL(request.url()).origin !== origin) external.push(request.url()); });
    await page.goto(origin);
    await page.getByTestId("main-sha").waitFor();
    assert.equal(await page.locator("section").count(), 5);
    assert.match(await page.getByTestId("main-sha").innerText(), /Unknown/);
    assert.deepEqual(external, []);
  } finally { await browser.close(); }
  console.log("Worker local smoke PASS: workerd, shared assets, missing-secret denial, five browser surfaces, same-origin requests. Access/live activation unverified.");
} finally { await app.stop(); }
