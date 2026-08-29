import { once } from "node:events";

import { afterEach, describe, expect, it } from "vitest";

import { createDashboardServer } from "../src/server/server.ts";
import { fixtureProjection } from "../src/server/fixture.ts";

const servers: Array<ReturnType<typeof createDashboardServer>> = [];

afterEach(async () => {
  await Promise.all(
    servers.splice(0).map(
      (server) =>
        new Promise<void>((resolve, reject) => {
          server.close((error) => (error === undefined ? resolve() : reject(error)));
        }),
    ),
  );
});

async function listen() {
  const secret = "github_pat_must_never_reach_browser";
  const server = createDashboardServer({
    loadProjection: async () => fixtureProjection(),
    assetRoot: new URL("../dist", import.meta.url),
  });
  servers.push(server);
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  if (address === null || typeof address === "string") throw new Error("Expected an ephemeral TCP address");
  return { baseUrl: `http://127.0.0.1:${address.port}`, secret };
}

describe("dashboard HTTP boundary", () => {
  it("exposes only the normalized read-only projection without credentials", async () => {
    const { baseUrl, secret } = await listen();
    const response = await fetch(`${baseUrl}/api/projection`);
    const body = await response.text();

    expect(response.status).toBe(200);
    expect(response.headers.get("cache-control")).toBe("no-store");
    expect(response.headers.get("access-control-allow-origin")).toBeNull();
    expect(body).toContain("nurockplayer/tachiko-work");
    expect(body).not.toContain(secret);
  });

  it("rejects mutation methods and unknown API routes", async () => {
    const { baseUrl } = await listen();

    expect((await fetch(`${baseUrl}/api/projection`, { method: "POST" })).status).toBe(405);
    expect((await fetch(`${baseUrl}/api/merge`)).status).toBe(404);
  });

  it("rejects malformed or traversal-like asset paths without escaping the build root", async () => {
    const { baseUrl } = await listen();

    expect((await fetch(`${baseUrl}/%E0%A4%A`)).status).toBe(400);
    expect((await fetch(`${baseUrl}/%2e%2e/%2e%2e/package.json`)).status).toBe(404);
  });
});
