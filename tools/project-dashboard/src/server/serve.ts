import { existsSync, statSync } from "node:fs";
import { open } from "node:fs/promises";
import { createServer, type Server, type ServerResponse } from "node:http";
import { extname, resolve, sep } from "node:path";
import { pipeline } from "node:stream/promises";

import type { DashboardProjection } from "../shared/model.js";
import { healthyProjection, partialProjection } from "./fixtures.js";
import { observeRepository, readServerCredential } from "./github.js";

const HOST = "127.0.0.1";
const DEFAULT_PORT = 4174;
const STATIC_ROOT = resolve(import.meta.dirname, "../../dist");

type ObservationProvider = () => Promise<DashboardProjection>;

const contentTypes: Record<string, string> = {
  ".css": "text/css; charset=utf-8",
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".map": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
};

function securityHeaders(): Record<string, string> {
  return {
    "Cache-Control": "no-store",
    "Content-Security-Policy": "default-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'",
    "Referrer-Policy": "no-referrer",
    "X-Content-Type-Options": "nosniff",
    "X-Frame-Options": "DENY",
  };
}

function fixtureProvider(name: string): ObservationProvider | null {
  if (name === "healthy") return () => Promise.resolve(healthyProjection());
  if (name === "partial") return () => Promise.resolve(partialProjection());
  return null;
}

function liveProvider(): ObservationProvider {
  const token = readServerCredential();
  return async () => observeRepository(token === undefined ? {} : { token });
}

function rejectMethod(response: ServerResponse): void {
  response.writeHead(405, { Allow: "GET, HEAD" });
  response.end();
}

function resolveStaticPath(pathname: string, staticRoot: string): string | null {
  let decoded: string;
  try {
    decoded = decodeURIComponent(pathname);
  } catch {
    return null;
  }
  if (decoded.includes("\0")) return null;
  const relative = decoded === "/" ? "index.html" : decoded.replace(/^\/+/, "");
  const candidate = resolve(staticRoot, relative);
  return candidate === staticRoot || candidate.startsWith(`${staticRoot}${sep}`)
    ? candidate
    : null;
}

export function createDashboardServer(
  provider: ObservationProvider,
  staticRoot = STATIC_ROOT,
): Server {
  return createServer((request, response) => {
    void (async () => {
      const method = request.method ?? "GET";
      const url = new URL(request.url ?? "/", `http://${HOST}`);
      for (const [name, value] of Object.entries(securityHeaders())) {
        response.setHeader(name, value);
      }
      if (url.pathname === "/api/project") {
        if (method !== "GET" && method !== "HEAD") {
          rejectMethod(response);
          return;
        }
        const body = JSON.stringify(await provider());
        response.writeHead(200, {
          "Content-Type": "application/json; charset=utf-8",
          "Content-Length": Buffer.byteLength(body),
        });
        response.end(method === "HEAD" ? undefined : body);
        return;
      }
      if (url.pathname.startsWith("/api/")) {
        response.writeHead(404);
        response.end();
        return;
      }
      if (method !== "GET" && method !== "HEAD") {
        rejectMethod(response);
        return;
      }
      const requested = resolveStaticPath(url.pathname, staticRoot);
      if (requested === null || !existsSync(requested) || !statSync(requested).isFile()) {
        response.writeHead(404);
        response.end();
        return;
      }
      response.writeHead(200, {
        "Content-Type": contentTypes[extname(requested)] ?? "application/octet-stream",
      });
      if (method === "HEAD") {
        response.end();
        return;
      }
      await pipeline((await open(requested, "r")).createReadStream(), response);
    })().catch(() => {
      if (!response.headersSent) response.writeHead(500);
      response.end();
    });
  });
}

export function portFromEnvironment(value: string | undefined): number {
  if (value === undefined) return DEFAULT_PORT;
  const port = /^\d+$/.test(value) ? Number(value) : Number.NaN;
  if (!Number.isInteger(port) || port < 1 || port > 65_535) {
    throw new Error("DASHBOARD_PORT must be an integer from 1 to 65535");
  }
  return port;
}

if (process.argv[1] === import.meta.filename) {
  const fixture = process.env.DASHBOARD_FIXTURE;
  const provider = fixture === undefined ? liveProvider() : fixtureProvider(fixture);
  if (provider === null) throw new Error("DASHBOARD_FIXTURE must be healthy or partial");
  const port = portFromEnvironment(process.env.DASHBOARD_PORT);
  createDashboardServer(provider).listen(port, HOST, () => {
    process.stdout.write(`Project Dashboard listening on http://${HOST}:${String(port)}\n`);
  });
}
