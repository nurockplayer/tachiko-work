import { mkdtemp, writeFile } from "node:fs/promises";
import { request, type IncomingHttpHeaders } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import { healthyObservation } from "../src/server/fixtures.js";
import { createDashboardServer } from "../src/server/serve.js";

const servers: ReturnType<typeof createDashboardServer>[] = [];

afterEach(async () => {
  await Promise.all(servers.splice(0).map((server) => new Promise<void>((resolve) => {
    server.close(() => { resolve(); });
  })));
});

async function startServer() {
  const root = await mkdtemp(join(tmpdir(), "tachiko-dashboard-test-"));
  await writeFile(join(root, "index.html"), "dashboard");
  const server = createDashboardServer(async () => healthyObservation(), root);
  servers.push(server);
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (address === null || typeof address === "string") throw new Error("server address missing");
  return { port: address.port };
}

async function send(port: number, path: string, method = "GET") {
  return new Promise<{ status: number; body: string; headers: IncomingHttpHeaders }>((resolve, reject) => {
    let responseHeaders: IncomingHttpHeaders;
    const req = request({ host: "127.0.0.1", port, path, method }, (response) => {
      responseHeaders = response.headers;
      let body = "";
      response.setEncoding("utf8");
      response.on("data", (chunk: string) => { body += chunk; });
      response.on("end", () => {
        resolve({ status: response.statusCode ?? 0, body, headers: responseHeaders });
      });
    });
    req.on("error", reject);
    req.end();
  });
}

describe("dashboard server", () => {
  it("serves only a no-store read projection without credentials", async () => {
    const { port } = await startServer();
    const response = await send(port, "/api/project");

    expect(response.status).toBe(200);
    expect(response.headers["cache-control"]).toBe("no-store");
    expect(response.headers["access-control-allow-origin"]).toBeUndefined();
    expect(response.body).not.toContain("github_pat_");
  });

  it("rejects control methods and exposes no merge endpoint", async () => {
    const { port } = await startServer();

    expect((await send(port, "/api/project", "POST")).status).toBe(405);
    expect((await send(port, "/api/merge")).status).toBe(404);
    expect((await send(port, "/..%2F..%2Fetc%2Fpasswd")).status).toBe(404);
  });
});
