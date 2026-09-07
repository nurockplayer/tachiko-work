import { describe, expect, it } from "vitest";
import { observeGitHub } from "../../src/server/shared.js";
import { fixture, connection } from "../operational/github-fixture.js";

describe("bounded remote observation", () => {
  it("terminates repeated REST pages and preserves partial availability", async () => {
    const remote = fixture();
    let pages = 0;
    const snapshot = await observeGitHub({ ...remote.options(), maxExternalRequests: 200, fetch: async (input, init) => {
      const request = new Request(input, init);
      if (new URL(request.url).pathname === `/repos/${remote.repository}/issues`) {
        pages++;
        // Finite harness breaker makes the old unbounded loop fail an assertion,
        // rather than hanging or relying on a test timeout.
        if (pages > 4) return new Response("upstream breaker", { status: 503 });
        return new Response("[]", { headers: { link: `<${request.url}>; rel="next"` } });
      }
      return remote.fetch(input, init);
    } });
    expect(snapshot.issues.availability).toBe("partial");
    expect(pages).toBe(1);
    expect(remote.violations).toEqual([]);
  });

  it("terminates repeated GraphQL cursors without calling linkage complete", async () => {
    const remote = fixture();
    let pages = 0;
    const snapshot = await observeGitHub({ ...remote.options(), maxExternalRequests: 200, fetch: async (input, init) => {
      const request = new Request(input, init);
      if (new URL(request.url).pathname === "/graphql") {
        pages++;
        if (pages > 4) return new Response("upstream breaker", { status: 503 });
        return Response.json({ data: { repository: { pullRequest: { closingIssuesReferences: connection(remote.data.linked, true) } } } });
      }
      return remote.fetch(input, init);
    } });
    expect(snapshot.pullRequests.value?.[0]?.linkedIssues.availability).toBe("partial");
    expect(pages).toBe(2);
    expect(remote.violations).toEqual([]);
  });

  it("caps a stream of distinct next pages instead of trusting endless pagination", async () => {
    const remote = fixture();
    let pages = 0;
    const snapshot = await observeGitHub({ ...remote.options(), maxExternalRequests: 200, fetch: async (input, init) => {
      const request = new Request(input, init);
      if (new URL(request.url).pathname === `/repos/${remote.repository}/issues`) {
        pages++;
        if (pages > 105) return new Response("upstream breaker", { status: 503 });
        return new Response("[]", { headers: { link: `<${remote.api}/issues?state=open&page=${pages + 1}>; rel="next"` } });
      }
      return remote.fetch(input, init);
    } });
    expect(snapshot.issues.availability).toBe("partial");
    expect(pages).toBe(100);
    expect(remote.violations).toEqual([]);
  });

  it("caps a stream of distinct GraphQL cursors as partial", async () => {
    const remote = fixture();
    let pages = 0;
    const snapshot = await observeGitHub({ ...remote.options(), maxExternalRequests: 200, fetch: async (input, init) => {
      const request = new Request(input, init);
      if (new URL(request.url).pathname === "/graphql") {
        pages++;
        if (pages > 105) return new Response("upstream breaker", { status: 503 });
        return Response.json({ data: { repository: { pullRequest: { closingIssuesReferences: {
          nodes: remote.data.linked, pageInfo: { hasNextPage: true, endCursor: String(pages) },
        } } } } });
      }
      return remote.fetch(input, init);
    } });
    expect(snapshot.pullRequests.value?.[0]?.linkedIssues.availability).toBe("partial");
    expect(pages).toBe(100);
    expect(remote.violations).toEqual([]);
  });

  it("accepts a terminal page at the pagination boundary", async () => {
    const remote = fixture();
    let pages = 0;
    const snapshot = await observeGitHub({ ...remote.options(), maxExternalRequests: 200, fetch: async (input, init) => {
      const request = new Request(input, init);
      if (new URL(request.url).pathname === `/repos/${remote.repository}/issues`) {
        pages++;
        const headers = pages < 100
          ? { link: `<${remote.api}/issues?state=open&page=${pages + 1}>; rel="next"` }
          : undefined;
        return new Response("[]", { headers });
      }
      return remote.fetch(input, init);
    } });
    expect(snapshot.issues.availability).toBe("complete");
    expect(pages).toBe(100);
    expect(remote.violations).toEqual([]);
  });

  it("keeps an empty complete pull request collection complete for watches", async () => {
    const remote = fixture();
    remote.data.pulls.length = 0;
    const snapshot = await observeGitHub(remote.options());
    expect(snapshot.pullRequests.availability).toBe("complete");
    expect(snapshot.pullRequests.value).toEqual([]);
    expect(snapshot.stewardWatches.availability).toBe("complete");
    expect(snapshot.stewardWatches.value).toEqual([]);
    expect(remote.violations).toEqual([]);
  });

  it("stops aggregate refresh fan-out at the external request budget", async () => {
    const remote = fixture();
    for (let number = 1000; number < 1025; number++) {
      remote.data.issues.push({ number, title: `Issue ${number}`, state: "open", html_url: `${remote.web}/issues/${number}` });
      remote.addPull(number, [number]);
    }
    const snapshot = await observeGitHub(remote.options());
    expect(remote.requests).toHaveLength(50);
    expect([
      snapshot.main.availability,
      snapshot.issues.availability,
      snapshot.pullRequests.availability,
      snapshot.stewardWatches.availability,
      snapshot.recentActivity.availability,
    ].some((availability) => availability !== "complete")).toBe(true);
    expect(remote.violations).toEqual([]);
  });

  it("observes every entity while bounding simultaneous per-entity requests", async () => {
    const remote = fixture();
    for (let number = 1000; number < 1012; number++) {
      remote.data.issues.push({ number, title: `Issue ${number}`, state: "open", html_url: `${remote.web}/issues/${number}` });
      remote.addPull(number, [number]);
    }
    let active = 0;
    let maximum = 0;
    const snapshot = await observeGitHub({ ...remote.options(), fetch: async (input, init) => {
      active++;
      maximum = Math.max(maximum, active);
      try {
        await new Promise(resolve => setTimeout(resolve, 2));
        return await remote.fetch(input, init);
      } finally { active--; }
    } });
    expect(maximum).toBeLessThanOrEqual(6);
    expect(snapshot.issues.availability).toBe("complete");
    expect(snapshot.issues.value).toHaveLength(remote.data.issues.length);
    expect(snapshot.pullRequests.availability).toBe("complete");
    expect(snapshot.pullRequests.value).toHaveLength(remote.data.pulls.length);
    expect(snapshot.stewardWatches.value).toHaveLength(remote.data.pulls.length);
    expect(remote.violations).toEqual([]);
  });
});
