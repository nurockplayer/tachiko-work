import { mkdtemp, writeFile } from "node:fs/promises";
import { request, type IncomingHttpHeaders } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import { healthyProjection } from "../src/server/fixtures.js";
import { createDashboardServer, portFromEnvironment } from "../src/server/serve.js";

const servers: ReturnType<typeof createDashboardServer>[] = [];

afterEach(async () => {
  await Promise.all(servers.splice(0).map((server) => new Promise<void>((resolve) => {
    server.close(() => {
      resolve();
    });
  })));
});

async function startServer() {
  const root = await mkdtemp(join(tmpdir(), "tachiko-dashboard-test-"));
  await writeFile(join(root, "index.html"), "dashboard");
  const server = createDashboardServer(async () => healthyProjection(), root);
  servers.push(server);
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (address === null || typeof address === "string") {
    throw new Error("server address missing");
  }
  return address.port;
}

async function send(port: number, path: string, method = "GET") {
  return new Promise<{
    status: number;
    body: string;
    headers: IncomingHttpHeaders;
  }>((resolve, reject) => {
    const requestHandle = request(
      { host: "127.0.0.1", port, path, method },
      (response) => {
        let body = "";
        response.setEncoding("utf8");
        response.on("data", (chunk: string) => {
          body += chunk;
        });
        response.on("end", () => {
          resolve({
            status: response.statusCode ?? 0,
            body,
            headers: response.headers,
          });
        });
      },
    );
    requestHandle.on("error", reject);
    requestHandle.end();
  });
}

describe("dashboard server", () => {
  it("accepts only a complete integer port string", () => {
    expect(portFromEnvironment(undefined)).toBe(4174);
    expect(portFromEnvironment("49173")).toBe(49_173);
    for (const value of ["4174invalid", "1.5", "", "0", "65536"]) {
      expect(() => portFromEnvironment(value)).toThrow(
        "DASHBOARD_PORT must be an integer from 1 to 65535",
      );
    }
  });

  it("serves only a no-store read projection without credentials", async () => {
    const response = await send(await startServer(), "/api/project");

    expect(response.status).toBe(200);
    expect(response.headers["cache-control"]).toBe("no-store");
    expect(response.headers["access-control-allow-origin"]).toBeUndefined();
    expect(response.body).not.toContain("github_pat_");
  });

  it("rejects control methods and exposes no merge endpoint", async () => {
    const port = await startServer();

    expect((await send(port, "/api/project", "POST")).status).toBe(405);
    expect((await send(port, "/api/merge")).status).toBe(404);
    expect((await send(port, "/..%2F..%2Fetc%2Fpasswd")).status).toBe(404);
  });

  it("streams validated static files", async () => {
    const response = await send(await startServer(), "/");

    expect(response.status).toBe(200);
    expect(response.body).toBe("dashboard");
  });
});
