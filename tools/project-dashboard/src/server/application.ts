import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import {
  assertDashboardRepository,
  createDashboardEnvelope,
  dashboardHtml,
  observeGitHub,
  unknownDashboardEnvelope,
  type DashboardEnvelope,
  type DashboardOptions,
} from "./shared.js";

export type { DashboardEnvelope, DashboardOptions } from "./shared.js";
export {
  createDashboardEnvelope,
  dashboardHtml,
  observeGitHub,
  unknownDashboardEnvelope,
} from "./shared.js";
export { RECENT_ACTIVITY_WINDOW } from "./shared.js";

export interface RunningDashboard {
  origin: string;
  close(): Promise<void>;
}

function send(
  response: ServerResponse,
  status: number,
  contentType: string,
  body: string,
): void {
  response.writeHead(status, {
    "content-type": contentType,
    "cache-control": "no-store",
    "x-content-type-options": "nosniff",
    "referrer-policy": "no-referrer",
  });
  response.end(body);
}

/** The real HTTP/application boundary used by the eventual CLI and browser tests. */
export async function startDashboard(
  options: DashboardOptions & { port?: number },
): Promise<RunningDashboard> {
  const repository = options.repository;
  assertDashboardRepository(repository);
  const server = createServer(async (request: IncomingMessage, response: ServerResponse) => {
    const host = request.headers.host ?? "";
    let url: URL;
    try {
      url = new URL(request.url ?? "/", `http://${host}`);
    } catch {
      return send(response, 400, "text/plain; charset=utf-8", "Bad request");
    }
    const local =
      /^127\.0\.0\.1(?::\d+)?$/.test(host) || /^\[::1\](?::\d+)?$/.test(host);
    if (
      !local ||
      request.headers.origin !== undefined ||
      request.headers["sec-fetch-site"] === "cross-site"
    ) {
      return send(response, 403, "text/plain; charset=utf-8", "Forbidden");
    }
    if (url.pathname === "/api/projection") {
      if (url.search !== "") {
        return send(response, 400, "text/plain; charset=utf-8", "Bad request");
      }
      if (request.method !== "GET") {
        return send(response, 405, "text/plain; charset=utf-8", "Method not allowed");
      }
      try {
        return send(
          response,
          200,
          "application/json; charset=utf-8",
          JSON.stringify(createDashboardEnvelope(options, await observeGitHub(options))),
        );
      } catch {
        return send(
          response,
          200,
          "application/json; charset=utf-8",
          JSON.stringify(unknownDashboardEnvelope(repository)),
        );
      }
    }
    if (url.pathname === "/" && (request.method === "GET" || request.method === "HEAD")) {
      return send(
        response,
        200,
        "text/html; charset=utf-8",
        request.method === "HEAD" ? "" : dashboardHtml,
      );
    }
    return send(response, 404, "text/plain; charset=utf-8", "Not found");
  });
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(options.port ?? 0, "127.0.0.1", resolve);
  });
  const address = server.address();
  if (address === null || typeof address === "string") {
    server.close();
    throw new Error("Dashboard did not bind TCP loopback");
  }
  return {
    origin: `http://127.0.0.1:${address.port}`,
    close: () =>
      new Promise((resolve, reject) =>
        server.close((error) => (error === undefined ? resolve() : reject(error))),
      ),
  };
}
