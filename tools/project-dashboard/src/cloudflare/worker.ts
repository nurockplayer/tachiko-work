import {
  assertDashboardRepository,
  createDashboardEnvelope,
  observeGitHub,
  unknownDashboardEnvelope,
  type DashboardOptions,
} from "../server/shared.js";

interface AssetBinding {
  fetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response>;
}

type WorkerEnvironment = Readonly<Record<string, unknown>>;

const SAFE_ASSET_HEADERS = [
  "accept",
  "accept-encoding",
  "accept-language",
  "if-modified-since",
  "if-none-match",
  "range",
] as const;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isAssetBinding(value: unknown): value is AssetBinding {
  return isRecord(value) && typeof value.fetch === "function";
}

function requiredString(environment: WorkerEnvironment, name: string): string {
  const value = environment[name];
  if (typeof value !== "string" || value.trim() === "") {
    throw new Error(`${name} is required`);
  }
  return value;
}

function dashboardOptions(environment: WorkerEnvironment): DashboardOptions {
  const repository = requiredString(environment, "DASHBOARD_REPOSITORY");
  assertDashboardRepository(repository);
  const trustedStewardLogins = requiredString(
    environment,
    "DASHBOARD_TRUSTED_STEWARD_LOGINS",
  )
    .split(",")
    .map((login) => login.trim())
    .filter((login) => login !== "");
  if (trustedStewardLogins.length === 0) {
    throw new Error("DASHBOARD_TRUSTED_STEWARD_LOGINS must name at least one login");
  }
  return {
    repository,
    token: requiredString(environment, "GITHUB_TOKEN"),
    trustedStewardLogins,
  };
}

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: {
      "content-type": "application/json; charset=utf-8",
      "cache-control": "no-store",
      "x-content-type-options": "nosniff",
      "referrer-policy": "no-referrer",
    },
  });
}

function textResponse(body: string, status: number): Response {
  return new Response(body, {
    status,
    headers: {
      "content-type": "text/plain; charset=utf-8",
      "cache-control": "no-store",
      "x-content-type-options": "nosniff",
      "referrer-policy": "no-referrer",
    },
  });
}

function sanitizedAssetRequest(request: Request): Request {
  const headers = new Headers();
  for (const name of SAFE_ASSET_HEADERS) {
    const value = request.headers.get(name);
    if (value !== null) headers.set(name, value);
  }
  return new Request(request.url, { method: request.method, headers });
}

async function serveAsset(request: Request, environment: WorkerEnvironment): Promise<Response> {
  const binding = environment.ASSETS;
  if (!isAssetBinding(binding)) return textResponse("Dashboard assets unavailable", 503);
  try {
    const response = await binding.fetch(sanitizedAssetRequest(request));
    const headers = new Headers(response.headers);
    headers.set("x-content-type-options", "nosniff");
    headers.set("referrer-policy", "no-referrer");
    return new Response(response.body, {
      status: response.status,
      statusText: response.statusText,
      headers,
    });
  } catch {
    return textResponse("Dashboard assets unavailable", 502);
  }
}

function isAllowedRequestOrigin(request: Request, url: URL): boolean {
  const host = request.headers.get("host");
  if (host !== null && host.toLowerCase() !== url.host.toLowerCase()) return false;
  if (url.protocol === "https:") return true;
  if (url.protocol !== "http:") return false;
  return (
    url.hostname === "127.0.0.1" ||
    url.hostname === "localhost" ||
    url.hostname === "::1" ||
    url.hostname === "[::1]"
  );
}

function isCrossOriginRequest(request: Request): boolean {
  return (
    request.headers.get("origin") !== null ||
    request.headers.get("sec-fetch-site") === "cross-site"
  );
}

const worker = {
  async fetch(request: Request, environment: WorkerEnvironment, _context: unknown): Promise<Response> {
    let url: URL;
    try {
      url = new URL(request.url);
    } catch {
      return textResponse("Bad request", 400);
    }
    if (!isAllowedRequestOrigin(request, url) || isCrossOriginRequest(request)) {
      return textResponse("Forbidden", 403);
    }

    if (url.pathname === "/api/projection") {
      if (url.search !== "") return textResponse("Bad request", 400);
      if (request.method !== "GET") return textResponse("Method not allowed", 405);
      try {
        const options = dashboardOptions(environment);
        const latest = await observeGitHub(options);
        const envelope = createDashboardEnvelope(options, latest);
        const serialized = JSON.stringify(envelope);
        if (serialized.includes(options.token)) {
          return jsonResponse(unknownDashboardEnvelope(""), 500);
        }
        return jsonResponse(envelope);
      } catch {
        // Configuration errors are intentionally indistinguishable from a generic
        // unavailable response; neither secrets nor upstream diagnostics cross the boundary.
        return jsonResponse(unknownDashboardEnvelope(""), 500);
      }
    }

    if (url.pathname.startsWith("/api/")) return textResponse("Not found", 404);
    if (request.method !== "GET" && request.method !== "HEAD") {
      return textResponse("Not found", 404);
    }
    return serveAsset(request, environment);
  },
};

export { worker };
export default worker;
