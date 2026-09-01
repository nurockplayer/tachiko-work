import { describe, expect, it } from "vitest";

import { observeRepository } from "../src/server/github.js";
import { normalizeRepository } from "../src/server/normalize.js";

const MAIN = "1111111111111111111111111111111111111111";
const HEAD = "2222222222222222222222222222222222222222";
const MERGE_BASE = "0000000000000000000000000000000000000000";

function graphResponse(
  hasNextPage = false,
  labelsHaveNextPage = hasNextPage,
  hasRoadmap = true,
  pullsHaveNextPage = false,
  closingIssuesHaveNextPage = false,
) {
  return {
    data: {
      repository: {
        url: "https://github.example/repository",
        defaultBranchRef: {
          name: "main",
          target: { oid: MAIN, url: `https://github.example/commit/${MAIN}` },
        },
        roadmap: hasRoadmap
          ? {
              oid: "roadmap-oid",
              text: "## Current horizon\n\n> **06 · Team Workspace Beta**\n\n## Product stages",
            }
          : null,
        issues: {
          pageInfo: { hasNextPage },
          nodes: [
            {
              number: 169,
              title: "Dashboard",
              url: "https://github.example/issues/169",
              state: "OPEN",
              labels: {
                pageInfo: { hasNextPage: labelsHaveNextPage },
                nodes: [{ name: "agent:codex" }, { name: "state:ready" }],
              },
              milestone: null,
              blockedBy: { pageInfo: { hasNextPage: false }, nodes: [] },
            },
          ],
        },
        pullRequests: {
          pageInfo: { hasNextPage: pullsHaveNextPage },
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
                pageInfo: { hasNextPage: closingIssuesHaveNextPage },
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
                      name: "Live Project Dashboard browser journey",
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

function fakeFetch(
  hasNextPage = false,
  labelsHaveNextPage = hasNextPage,
  hasRoadmap = true,
  pullsHaveNextPage = false,
  closingIssuesHaveNextPage = false,
) {
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
      return new Response(
        JSON.stringify(
          graphResponse(
            hasNextPage,
            labelsHaveNextPage,
            hasRoadmap,
            pullsHaveNextPage,
            closingIssuesHaveNextPage,
          ),
        ),
        {
        status: 200,
        headers: { "Content-Type": "application/json" },
        },
      );
    }
    return new Response(
      JSON.stringify({ status: "ahead", merge_base_commit: { sha: MAIN } }),
      { status: 200, headers: { "Content-Type": "application/json" } },
    );
  }) as typeof fetch;
  return { implementation, requests };
}

function fakeBehindAuthorityFetch() {
  const requests: string[] = [];
  const implementation = (async (input: string | URL | Request) => {
    const url =
      typeof input === "string"
        ? input
        : input instanceof URL
          ? input.href
          : input.url;
    requests.push(url);
    if (url.endsWith("/graphql")) {
      return new Response(JSON.stringify(graphResponse()), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }
    const body = url.endsWith(`${MAIN}...${HEAD}`)
      ? { status: "diverged", merge_base_commit: { sha: MERGE_BASE } }
      : {
          status: "ahead",
          merge_base_commit: { sha: MERGE_BASE },
          files: [
            { filename: "docs/architecture/document-model.md" },
            { filename: "docs/decisions/ADR-0032-dashboard.md" },
            { filename: "docs/security/threat-model.md" },
            { filename: "docs/vision/product-constitution.md" },
            { filename: "docs/discussions/history.md" },
          ],
        };
    return new Response(JSON.stringify(body), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
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

  it("does not grant readiness, ownership, or merge state from truncated labels", async () => {
    const fake = fakeFetch(false, true);
    const observation = await observeRepository({ fetchImpl: fake.implementation });
    const lane = normalizeRepository(observation).deliveries[0];

    expect(observation.issues[0]?.labelsAvailability).toBe("incomplete");
    expect(lane?.owner).toBe("unknown");
    expect(lane?.readiness.state).toBe("unknown");
    expect(lane?.mergeGate.state).not.toBe("satisfied");
  });

  it("marks a missing Product Roadmap source partial and the horizon Unknown", async () => {
    const fake = fakeFetch(false, false, false);
    const observation = await observeRepository({ fetchImpl: fake.implementation });
    const projection = normalizeRepository(observation);

    expect(observation.availability).toBe("incomplete");
    expect(projection.fetchHealth).toBe("partial");
    expect(projection.executive.productHorizon.state).toBe("unknown");
  });

  it.each([
    ["top-level pull pagination", true, false],
    ["closing-Issue pagination", false, true],
  ])("keeps merge state Unknown for incomplete %s", async (_name, pullPage, closingPage) => {
    const fake = fakeFetch(false, false, true, pullPage, closingPage);
    const observation = await observeRepository({ fetchImpl: fake.implementation });

    expect(observation.implementationLinkageAvailability).toBe("incomplete");
    expect(normalizeRepository(observation).deliveries[0]?.mergeGate.state).not.toBe(
      "satisfied",
    );
  });

  it("fails closed when Accepted ADR or Principle authority changed after the merge base", async () => {
    const fake = fakeBehindAuthorityFetch();
    const observation = await observeRepository({ fetchImpl: fake.implementation });
    const pull = observation.pullRequests[0];

    expect(fake.requests).toHaveLength(3);
    expect(pull?.authorityAvailability).toBe("complete");
    expect(pull?.authorityChanges.map((change) => change.path)).toEqual([
      "docs/architecture/document-model.md",
      "docs/decisions/ADR-0032-dashboard.md",
      "docs/security/threat-model.md",
      "docs/vision/product-constitution.md",
    ]);
    expect(normalizeRepository(observation).deliveries[0]?.authority.state).toBe("unknown");
  });
});
