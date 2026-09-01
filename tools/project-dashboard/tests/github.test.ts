import { describe, expect, it } from "vitest";

import { observeRepository } from "../src/server/github.js";
import { normalizeRepository } from "../src/server/normalize.js";

const MAIN = "1111111111111111111111111111111111111111";
const HEAD = "2222222222222222222222222222222222222222";

function graphResponse(hasNextPage = false) {
  return {
    data: {
      repository: {
        url: "https://github.example/repository",
        defaultBranchRef: {
          name: "main",
          target: { oid: MAIN, url: `https://github.example/commit/${MAIN}` },
        },
        roadmap: {
          oid: "roadmap-oid",
          text: "## Current horizon\n\n> **06 · Team Workspace Beta**\n\n## Product stages",
        },
        issues: {
          pageInfo: { hasNextPage },
          nodes: [
            {
              number: 169,
              title: "Dashboard",
              url: "https://github.example/issues/169",
              state: "OPEN",
              labels: {
                pageInfo: { hasNextPage: false },
                nodes: [{ name: "agent:codex" }, { name: "state:ready" }],
              },
              milestone: null,
              blockedBy: { pageInfo: { hasNextPage: false }, nodes: [] },
            },
          ],
        },
        pullRequests: {
          pageInfo: { hasNextPage: false },
          nodes: [
            {
              number: 225,
              title: "Dashboard",
              url: "https://github.example/pulls/225",
              state: "OPEN",
              isDraft: false,
              headRefOid: HEAD,
              baseRefOid: MAIN,
              baseRefName: "main",
              closingIssuesReferences: {
                pageInfo: { hasNextPage: false },
                nodes: [{ number: 169 }],
              },
              comments: { pageInfo: { hasNextPage: false }, nodes: [] },
              reviews: { pageInfo: { hasNextPage: false }, nodes: [] },
              reviewThreads: { pageInfo: { hasNextPage: false }, nodes: [] },
              statusCheckRollup: {
                contexts: {
                  pageInfo: { hasNextPage: false },
                  nodes: [
                    {
                      __typename: "CheckRun",
                      id: "check-1",
                      name: "project-dashboard-browser",
                      status: "COMPLETED",
                      conclusion: "SKIPPED",
                      url: "https://github.example/checks/1",
                      detailsUrl: null,
                    },
                  ],
                },
              },
            },
          ],
        },
        recent: { pageInfo: { hasNextPage: true }, nodes: [] },
      },
    },
  };
}

function fakeFetch(hasNextPage = false) {
  const requests: { url: string; init?: RequestInit }[] = [];
  const implementation = (async (input: string | URL | Request, init?: RequestInit) => {
    const url =
      typeof input === "string"
        ? input
        : input instanceof URL
          ? input.href
          : input.url;
    requests.push(init === undefined ? { url } : { url, init });
    if (url.endsWith("/graphql")) {
      return new Response(JSON.stringify(graphResponse(hasNextPage)), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }
    return new Response(
      JSON.stringify({ status: "ahead", merge_base_commit: { sha: MAIN } }),
      { status: 200, headers: { "Content-Type": "application/json" } },
    );
  }) as typeof fetch;
  return { implementation, requests };
}

describe("GitHub observation adapter", () => {
  it("uses only a bounded query/GET path and treats skipped browser evidence as failure", async () => {
    const fake = fakeFetch();
    const observation = await observeRepository({
      token: "server_secret",
      fetchImpl: fake.implementation,
    });
    const projection = normalizeRepository(observation);

    expect(observation.availability).toBe("complete");
    expect(projection.deliveries[0]?.checks.state).toBe("blocked");
    expect(projection.deliveries[0]?.evidence.automatedBrowser.state).toBe("blocked");
    expect(fake.requests.map((request) => request.init?.method)).toEqual(["POST", "GET"]);
    const graphBody = fake.requests[0]?.init?.body;
    if (typeof graphBody !== "string") throw new Error("GraphQL body must be JSON text");
    expect(graphBody).toContain("query DashboardRepository");
    expect(graphBody).not.toContain("mutation");
    expect(JSON.stringify(projection)).not.toContain("server_secret");
  });

  it("marks completeness-required pagination as partial but keeps bounded recent history valid", async () => {
    const fake = fakeFetch(true);
    const observation = await observeRepository({ fetchImpl: fake.implementation });

    expect(observation.availability).toBe("incomplete");
    expect(observation.issuesAvailability).toBe("incomplete");
    expect(observation.recentActivityAvailability).toBe("complete");
    expect(normalizeRepository(observation).fetchHealth).toBe("partial");
  });
});
