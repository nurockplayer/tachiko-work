import { afterEach, describe, expect, it } from "vitest";
import worker from "../../src/cloudflare/worker.js";
import {
  createDashboardEnvelope,
  observeGitHub,
} from "../../src/server/shared.js";
import { fixture, MAIN, SECRET, STEWARD } from "../operational/github-fixture.js";

const originalFetch = globalThis.fetch;

afterEach(() => {
  globalThis.fetch = originalFetch;
});

function environment(remote: ReturnType<typeof fixture>) {
  const assetRequests: Request[] = [];
  const env: Record<string, unknown> = {
    GITHUB_TOKEN: remote.options().token,
    DASHBOARD_REPOSITORY: remote.repository,
    DASHBOARD_TRUSTED_STEWARD_LOGINS: STEWARD,
    ASSETS: {
      fetch: async (input: RequestInfo | URL, init?: RequestInit) => {
        const request = new Request(input, init);
        assetRequests.push(request.clone());
        return new Response("asset", {
          headers: { "content-type": "text/plain" },
        });
      },
    },
  };
  return { env, assetRequests };
}

async function call(
  env: Record<string, unknown>,
  path: string,
  init?: RequestInit,
): Promise<Response> {
  return await worker.fetch(
    new Request(`https://dashboard.example${path}`, init),
    env,
    {},
  );
}

describe("Cloudflare Worker adapter unit boundaries", () => {
  it("uses the shared observer and envelope without changing complete projection facts", async () => {
    const remote = fixture();
    const { env } = environment(remote);
    const snapshot = await observeGitHub(remote.options());
    const expected = createDashboardEnvelope(remote.options(), snapshot);
    globalThis.fetch = remote.fetch;

    const response = await call(env, "/api/projection", {
      headers: { authorization: "Bearer browser-session-canary" },
    });
    const actual = (await response.json()) as typeof expected;

    expect(response.status).toBe(200);
    expect(actual).toEqual(expected);
    expect(JSON.stringify(actual)).not.toContain(SECRET);
    expect(remote.requests.every((request) => request.redirect === "error")).toBe(true);
    expect(remote.requests.every((request) => request.headers.get("user-agent") === "tachiko-work-project-dashboard/0.1")).toBe(true);
    expect(remote.violations).toEqual([]);
  });

  it("strips credential and cookie headers before forwarding a static asset request", async () => {
    const remote = fixture();
    const { env, assetRequests } = environment(remote);
    const response = await call(env, "/", {
      headers: {
        authorization: `Bearer ${SECRET}`,
        cookie: `session=${SECRET}`,
        accept: "text/html",
      },
    });

    expect(response.status).toBe(200);
    expect(await response.text()).toBe("asset");
    expect(assetRequests).toHaveLength(1);
    expect(assetRequests[0]?.headers.get("authorization")).toBeNull();
    expect(assetRequests[0]?.headers.get("cookie")).toBeNull();
    expect(assetRequests[0]?.headers.get("accept")).toBe("text/html");
    expect(assetRequests[0]?.url).toBe("https://dashboard.example/");
  });

  it("does not send the GitHub token to an untrusted pagination destination", async () => {
    const remote = fixture();
    const observedRequests: Request[] = [];
    const maliciousFetch: typeof globalThis.fetch = async (input, init) => {
      const request = new Request(input, init);
      observedRequests.push(request.clone());
      const response = await remote.fetch(input, init);
      if (
        new URL(request.url).pathname === `/repos/${remote.repository}/issues` &&
        new URL(request.url).searchParams.get("state") === "open"
      ) {
        const body = await response.text();
        const headers = new Headers(response.headers);
        headers.set("link", "<https://attacker.example/collect>; rel=\"next\"");
        return new Response(body, {
          status: response.status,
          headers,
        });
      }
      return response;
    };

    const snapshot = await observeGitHub({
      ...remote.options(),
      fetch: maliciousFetch,
    });

    expect(snapshot.issues.availability).toBe("partial");
    expect(observedRequests.every((request) => new URL(request.url).origin === "https://api.github.com")).toBe(true);
    expect(observedRequests.some((request) => request.url.includes("attacker.example"))).toBe(false);
    expect(remote.violations).toEqual([]);
  });

  it("keeps control routes and cross-origin API reads unavailable", async () => {
    const remote = fixture();
    const { env } = environment(remote);
    for (const method of ["POST", "PUT", "PATCH", "DELETE"]) {
      expect((await call(env, "/api/projection", { method })).status).toBe(405);
    }
    expect(
      (
        await call(env, "/api/projection", {
          headers: {
            origin: "https://other.example",
            "sec-fetch-site": "cross-site",
          },
        })
      ).status,
    ).toBe(403);
    expect(
      (
        await worker.fetch(
          new Request("https://dashboard.example/api/projection", {
            headers: { host: "attacker.example" },
          }),
          env,
          {},
        )
      ).status,
    ).toBe(403);
    expect(
      (
        await worker.fetch(
          new Request("http://dashboard.example/api/projection"),
          env,
          {},
        )
      ).status,
    ).toBe(403);
    expect((await call(env, "/api/merge")).status).toBe(404);
  });

  it("does not expose a token echoed by upstream data or malformed configuration", async () => {
    const remote = fixture();
    const { env } = environment(remote);
    globalThis.fetch = remote.fetch;
    remote.data.issues[0]!.title = SECRET;
    for (const bindings of [env, { ...env, DASHBOARD_REPOSITORY: SECRET }]) {
      const response = await call(bindings, "/api/projection");
      expect(response.status).toBe(500);
      expect(await response.text()).not.toContain(SECRET);
    }
    expect(remote.violations).toEqual([]);
  });

  it("fails closed on unavailable assets without exposing binding errors", async () => {
    const remote = fixture();
    const { env } = environment(remote);
    expect((await call({ ...env, ASSETS: undefined }, "/")).status).toBe(503);
    const response = await call({ ...env, ASSETS: { fetch() { throw new Error(SECRET); } } }, "/");
    expect(response.status).toBe(502);
    expect(await response.text()).not.toContain(SECRET);
  });

  it("fails closed when the server-only GitHub binding is empty", async () => {
    const remote = fixture();
    const { env } = environment(remote);
    const response = await call({ ...env, GITHUB_TOKEN: "" }, "/api/projection");

    expect(response.status).toBe(500);
    const body = await response.text();
    expect(body).not.toContain(SECRET);
  });
});
