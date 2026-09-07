import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { fixture, MAIN, STEWARD } from "../operational/github-fixture.js";

interface WorkerLike {
  fetch(request: Request, env: Record<string, unknown>, context: unknown): Response | Promise<Response>;
}

const workerPath = fileURLToPath(new URL("../../src/cloudflare/worker.ts", import.meta.url));
let worker: WorkerLike;

beforeAll(async () => {
  expect(existsSync(workerPath), "#342 Worker adapter is not implemented yet").toBe(true);
  const module = await vi.importActual<{ default?: WorkerLike; worker?: WorkerLike }>("../../src/cloudflare/worker.js");
  worker = module.default ?? module.worker!;
  expect(worker?.fetch).toBeTypeOf("function");
});

function harness() {
  const remote = fixture();
  const assetRequests: Request[] = [];
  const env: Record<string, unknown> = {
    GITHUB_TOKEN: remote.options().token,
    DASHBOARD_REPOSITORY: remote.repository,
    DASHBOARD_TRUSTED_STEWARD_LOGINS: STEWARD,
    ASSETS: {
      fetch: async (input: RequestInfo | URL, init?: RequestInit) => {
        const request = new Request(input, init);
        assetRequests.push(request.clone());
        return new Response("<!doctype html><title>Tachiko Dashboard</title>", { headers: { "content-type": "text/html" } });
      },
    },
  };
  return { remote, env, assetRequests };
}

async function call(env: Record<string, unknown>, path = "/api/projection", init?: RequestInit) {
  return await worker.fetch(new Request(`https://dashboard.example${path}`, init), env, { waitUntil() {}, passThroughOnException() {} });
}

describe("#342 Worker request boundary", () => {
  it("reuses the merged Dashboard observation truth through a read-only API response", async () => {
    const { remote, env } = harness();
    const original = globalThis.fetch;
    globalThis.fetch = remote.fetch;
    try {
      const response = await call(env, "/api/projection", { headers: { Authorization: "Bearer browser-session-canary" } });
      const body = await response.text();
      expect(response.status).toBe(200);
      expect(response.headers.get("cache-control")).toContain("no-store");
      expect(body).not.toContain(remote.options().token);
      const value = JSON.parse(body) as { repository: string; projection: { executive: { mainSha: unknown } } };
      expect(value.repository).toBe(remote.repository);
      expect(value.projection.executive.mainSha).toEqual({ availability: "complete", value: MAIN });
      expect(remote.violations).toEqual([]);
    } finally {
      globalThis.fetch = original;
    }
  });

  it("keeps control methods/routes unavailable and rejects a cross-origin API read", async () => {
    const { env } = harness();
    for (const method of ["POST", "PUT", "PATCH", "DELETE"]) {
      expect((await call(env, "/api/projection", { method })).status).toBe(405);
    }
    for (const route of ["/api/merge", "/api/dispatch", "/api/run-agent"]) {
      expect((await call(env, route)).status).toBe(404);
    }
    expect((await call(env, "/api/projection", { headers: { Origin: "https://other.example", "Sec-Fetch-Site": "cross-site" } })).status).toBe(403);
  });

  it("serves static assets without adding the repository credential to the asset request", async () => {
    const { env, assetRequests, remote } = harness();
    const response = await call(env, "/");
    expect(response.status).toBe(200);
    expect(await response.text()).not.toContain(remote.options().token);
    expect(assetRequests).toHaveLength(1);
    expect(assetRequests[0]?.headers.get("authorization") ?? "").not.toContain(remote.options().token);
  });

  it("fails closed when a required server-side binding is absent", async () => {
    const { env } = harness();
    const response = await call({ ...env, GITHUB_TOKEN: "" });
    expect(response.status).toBeGreaterThanOrEqual(400);
  });

  it("does not retain current MAIN across an upstream failure and recovers from a later observation", async () => {
    const { remote, env } = harness();
    const original = globalThis.fetch;
    globalThis.fetch = remote.fetch;
    try {
      remote.failures.add("main");
      remote.setErrorText(`upstream refused ${remote.options().token}`);
      const failed = await call(env);
      const failedText = await failed.text();
      expect(failed.status).toBe(200);
      expect(failedText).not.toContain(remote.options().token);
      expect(JSON.parse(failedText).projection.executive.mainSha).toEqual({ availability: "unavailable", value: null });

      remote.failures.clear();
      remote.data.main = "4".repeat(40);
      const recovered = await call(env);
      expect((await recovered.json()).projection.executive.mainSha).toEqual({ availability: "complete", value: remote.data.main });
      expect(remote.violations).toEqual([]);
    } finally {
      globalThis.fetch = original;
    }
  });
});
