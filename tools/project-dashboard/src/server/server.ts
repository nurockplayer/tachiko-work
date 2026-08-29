import { createReadStream } from "node:fs";
import { stat } from "node:fs/promises";
import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import { extname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

import type { RepositoryProjection } from "../shared/types.ts";

interface DashboardServerOptions {
  loadProjection: () => Promise<RepositoryProjection>;
  assetRoot: URL | string;
}

const contentTypes: Record<string, string> = {
  ".css": "text/css; charset=utf-8",
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
};

function securityHeaders(): Record<string, string> {
  return {
    "Content-Security-Policy": "default-src 'self'; base-uri 'none'; connect-src 'self'; frame-ancestors 'none'; img-src 'self' data:; object-src 'none'; script-src 'self'; style-src 'self'",
    "Referrer-Policy": "no-referrer",
    "X-Content-Type-Options": "nosniff",
    "X-Frame-Options": "DENY",
  };
}

function jsonResponse(response: ServerResponse, status: number, value: unknown): void {
  response.writeHead(status, {
    ...securityHeaders(),
    "Cache-Control": "no-store",
    "Content-Type": "application/json; charset=utf-8",
  });
  response.end(JSON.stringify(value));
}

async function serveAsset(
  response: ServerResponse,
  assetRoot: string,
  requestPath: string,
  headOnly: boolean,
): Promise<void> {
  let decoded: string;
  try {
    decoded = decodeURIComponent(requestPath);
  } catch {
    response.writeHead(400, securityHeaders());
    response.end("Malformed request path.");
    return;
  }
  const candidate = resolve(assetRoot, decoded === "/" ? "index.html" : `.${decoded}`);
  if (candidate !== assetRoot && !candidate.startsWith(`${assetRoot}${sep}`)) {
    response.writeHead(404, securityHeaders());
    response.end();
    return;
  }

  let file = candidate;
  try {
    if (!(await stat(file)).isFile()) file = resolve(assetRoot, "index.html");
    const info = await stat(file);
    if (!info.isFile()) throw new Error("not a file");
  } catch {
    response.writeHead(404, securityHeaders());
    response.end("Dashboard assets have not been built. Run pnpm build.");
    return;
  }

  response.writeHead(200, {
    ...securityHeaders(),
    "Cache-Control": file.includes(`${sep}assets${sep}`) ? "public, max-age=31536000, immutable" : "no-cache",
    "Content-Type": contentTypes[extname(file)] ?? "application/octet-stream",
  });
  if (headOnly) {
    response.end();
    return;
  }
  createReadStream(file).pipe(response);
}

export function createDashboardServer(options: DashboardServerOptions): Server {
  const assetRoot = typeof options.assetRoot === "string" ? resolve(options.assetRoot) : resolve(fileURLToPath(options.assetRoot));
  const handleRequest = async (request: IncomingMessage, response: ServerResponse): Promise<void> => {
    const method = request.method ?? "GET";
    const requestUrl = new URL(request.url ?? "/", "http://127.0.0.1");
    if (method !== "GET" && method !== "HEAD") {
      response.writeHead(405, { ...securityHeaders(), Allow: "GET, HEAD" });
      response.end();
      return;
    }
    if (requestUrl.pathname === "/api/health") {
      jsonResponse(response, 200, { status: "ok", mode: "read-only" });
      return;
    }
    if (requestUrl.pathname === "/api/projection") {
      try {
        const projection = await options.loadProjection();
        jsonResponse(response, 200, projection);
      } catch {
        jsonResponse(response, 503, { error: "Repository projection is unavailable." });
      }
      return;
    }
    if (requestUrl.pathname.startsWith("/api/")) {
      jsonResponse(response, 404, { error: "Read-only endpoint not found." });
      return;
    }
    await serveAsset(response, assetRoot, requestUrl.pathname, method === "HEAD");
  };
  return createServer((request, response) => {
    void handleRequest(request, response).catch(() => {
      if (!response.headersSent) jsonResponse(response, 500, { error: "Read-only dashboard request failed." });
      else response.destroy();
    });
  });
}
