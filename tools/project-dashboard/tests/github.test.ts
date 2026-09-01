import { describe, expect, it, vi } from "vitest";

import {
  DASHBOARD_QUERY,
  observeRepository,
  parseProductHorizon,
  projectGraphResponse,
  type DashboardGraphResponse,
} from "../src/server/github.js";

const MAIN_SHA = "1111111111111111111111111111111111111111";
const HEAD_SHA = "2222222222222222222222222222222222222222";

function graph(): DashboardGraphResponse {
  return {
    data: {
      repository: {
        url: "https://github.example/repository",
        defaultBranchRef: { target: { oid: MAIN_SHA, url: "https://github.example/main" } },
        roadmap: { text: "## Current horizon\n\n> **06 · Team Workspace Beta**\n\n## Later" },
        issues: {
          pageInfo: { hasNextPage: false },
          nodes: [{
            number: 229,
            title: "Dashboard",
            url: "https://github.example/issues/229",
            state: "OPEN",
            labels: {
              pageInfo: { hasNextPage: false },
              nodes: [{ name: "agent:codex" }, { name: "state:ready" }],
            },
            milestone: null,
            blockedBy: {
              pageInfo: { hasNextPage: false },
              nodes: [{ number: 200, state: "CLOSED", url: "https://github.example/issues/200" }],
            },
          }],
        },
        pullRequests: {
          pageInfo: { hasNextPage: false },
          nodes: [{
            number: 230,
            title: "Dashboard PR",
            url: "https://github.example/pulls/230",
            state: "OPEN",
            isDraft: false,
            headRefOid: HEAD_SHA,
            baseRefOid: MAIN_SHA,
            baseRefName: "main",
            mergeable: "MERGEABLE",
            mergeStateStatus: "UNSTABLE",
            reviewDecision: "REVIEW_REQUIRED",
            closingIssuesReferences: {
              pageInfo: { hasNextPage: false },
              nodes: [{ number: 229 }],
            },
            comments: {
              pageInfo: { hasNextPage: false },
              nodes: [
                {
                  id: "handoff",
                  body: [
                    "<!-- agent-handoff:v1 -->",
                    "ISSUE: 229",
                    "PR: 230",
                    "OWNER: agent:codex",
                    "STATE: active",
                    `HEAD: ${HEAD_SHA}`,
                    `MAIN: ${MAIN_SHA}`,
                  ].join("\n"),
                  url: "https://github.example/comments/handoff",
                  createdAt: "2026-09-02T00:00:00Z",
                  updatedAt: "2026-09-02T00:00:00Z",
                  lastEditedAt: null,
                  author: { login: "nurockplayer" },
                  authorAssociation: "OWNER",
                },
                {
                  id: "watch",
                  body: [
                    "<!-- project-steward-watch:v1 -->",
                    "VERDICT: GREEN",
                    `HEAD: ${HEAD_SHA}`,
                    `MAIN: ${MAIN_SHA}`,
                    "HUMAN_ACTION: none",
                  ].join("\n"),
                  url: "https://github.example/comments/watch",
                  createdAt: "2026-09-02T00:00:00Z",
                  updatedAt: "2026-09-02T00:00:00Z",
                  lastEditedAt: null,
                  author: { login: "nurockplayer" },
                  authorAssociation: "OWNER",
                },
              ],
            },
            reviews: {
              pageInfo: { hasNextPage: false },
              nodes: [{
                id: "review",
                url: "https://github.example/reviews/1",
                state: "COMMENTED",
                author: { login: "reviewer" },
                commit: { oid: HEAD_SHA },
              }],
            },
            statusCheckRollup: {
              contexts: {
                pageInfo: { hasNextPage: false },
                nodes: [{
                  __typename: "CheckRun",
                  id: "check",
                  name: "build",
                  status: "COMPLETED",
                  conclusion: "NEUTRAL",
                  url: "https://github.example/checks/1",
                  detailsUrl: null,
                }],
              },
            },
          }],
        },
        recent: {
          pageInfo: { hasNextPage: false },
          nodes: [{
            number: 227,
            title: "Recent merge",
            url: "https://github.example/pulls/227",
            mergedAt: "2026-09-01T00:00:00Z",
            mergeCommit: { oid: "3333333333333333333333333333333333333333" },
          }],
        },
      },
    },
  };
}

describe("Dashboard GitHub observation", () => {
  it("uses a fixed read-only query and has no can_merge output contract", () => {
    expect(DASHBOARD_QUERY).toContain("mergeable mergeStateStatus reviewDecision");
    expect(DASHBOARD_QUERY).not.toMatch(/\bmutation\b/i);
    expect(DASHBOARD_QUERY).not.toMatch(/can_?merge/i);
  });

  it("displays native and structured facts without synthesizing policy", () => {
    const projection = projectGraphResponse(graph(), "2026-09-02T00:00:00Z");
    const pull = projection.deliveries[0]?.pullRequest;

    expect(projection.fetchHealth).toBe("healthy");
    expect(projection.executive.productHorizon.value).toBe("06 · Team Workspace Beta");
    expect(pull).toMatchObject({
      mergeable: "MERGEABLE",
      mergeStateStatus: "UNSTABLE",
      reviewDecision: "REVIEW_REQUIRED",
      handoff: { status: "current", value: "active · agent:codex" },
      stewardWatch: { status: "current", value: "GREEN · human action none" },
    });
    expect(pull?.checks.items[0]).toMatchObject({
      status: "COMPLETED",
      conclusion: "NEUTRAL",
    });
    expect(JSON.stringify(projection)).not.toMatch(/mergeGate|mergeReady|canMerge|can_merge/);
  });

  it("makes malformed exact-head evidence explicit", () => {
    const response = graph();
    const repository = response.data?.repository;
    const pull = repository?.pullRequests.nodes?.[0];
    const comment = pull?.comments?.nodes?.[0];
    if (comment !== null && comment !== undefined) {
      comment.body = comment.body.replace(HEAD_SHA, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    }

    const projection = projectGraphResponse(response);
    expect(projection.fetchHealth).toBe("healthy");
    expect(projection.executive.activeCount.value).toBe(1);
    expect(projection.deliveries[0]?.pullRequest?.handoff).toMatchObject({
      status: "unknown",
      reason: "Agent handoff head-mismatch",
    });
    expect(projection.attention.some((item) => item.level === "unknown")).toBe(true);
  });

  it("marks omitted recent merge identities partial instead of silently complete", () => {
    const response = graph();
    const recent = response.data?.repository?.recent.nodes?.[0];
    if (recent !== null && recent !== undefined) recent.mergeCommit = null;

    const projection = projectGraphResponse(response);
    expect(projection.recentActivity).toMatchObject({ availability: "partial", items: [] });
    expect(projection.fetchHealth).toBe("partial");
  });

  it("treats the bounded recent window as complete when more history exists", () => {
    const response = graph();
    const recent = response.data?.repository?.recent;
    if (recent !== undefined) recent.pageInfo.hasNextPage = true;

    const projection = projectGraphResponse(response);
    expect(projection.recentActivity.availability).toBe("complete");
    expect(projection.fetchHealth).toBe("healthy");
  });

  it("does not call truncated comments missing or count truncated labels", () => {
    const response = graph();
    const repository = response.data?.repository;
    const pull = repository?.pullRequests.nodes?.[0];
    if (pull !== null && pull !== undefined && pull.comments !== null) {
      pull.comments.pageInfo.hasNextPage = true;
    }
    const issue = repository?.issues.nodes?.[0];
    if (issue !== null && issue !== undefined && issue.labels !== null) {
      issue.labels.pageInfo.hasNextPage = true;
    }

    const projection = projectGraphResponse(response);
    expect(projection.fetchHealth).toBe("partial");
    expect(projection.executive.activeCount.value).toBeNull();
    expect(projection.deliveries[0]?.pullRequest?.handoff).toMatchObject({
      status: "unknown",
      reason: "Agent handoff comment observation incomplete",
    });
  });

  it("does not use truncated closing-Issue references as exact evidence context", () => {
    const response = graph();
    const pull = response.data?.repository?.pullRequests.nodes?.[0];
    if (
      pull !== null &&
      pull !== undefined &&
      pull.closingIssuesReferences !== null
    ) {
      pull.closingIssuesReferences.pageInfo.hasNextPage = true;
    }

    const projection = projectGraphResponse(response);
    expect(projection.deliveries[0]?.pullRequest).toMatchObject({
      handoff: { status: "unknown", reason: "Issue linkage Unknown" },
      stewardWatch: { status: "unknown", reason: "Issue linkage Unknown" },
    });
  });

  it("treats a missing check rollup as Unknown rather than an empty complete list", () => {
    const response = graph();
    const pull = response.data?.repository?.pullRequests.nodes?.[0];
    if (pull !== null && pull !== undefined) pull.statusCheckRollup = null;

    const projection = projectGraphResponse(response);
    expect(projection.deliveries[0]?.pullRequest?.checks).toMatchObject({
      availability: "partial",
      items: [],
    });
    expect(projection.fetchHealth).toBe("partial");
  });

  it("keeps unmatched Issue-to-PR linkage and human action Unknown when pulls truncate", () => {
    const response = graph();
    const repository = response.data?.repository;
    const firstIssue = repository?.issues.nodes?.[0];
    if (repository !== null && repository !== undefined && firstIssue !== null && firstIssue !== undefined) {
      repository.issues.nodes?.push({
        ...firstIssue,
        number: 231,
        title: "Issue outside the observed pull window",
        url: "https://github.example/issues/231",
      });
      repository.pullRequests.pageInfo.hasNextPage = true;
    }

    const projection = projectGraphResponse(response);
    const issueOnlyLane = projection.deliveries.find((lane) => lane.issue?.number === 231);
    expect(issueOnlyLane).toMatchObject({
      pullRequest: null,
      linkageAvailability: "partial",
    });
    expect(projection.executive.humanAction).toMatchObject({
      value: null,
      availability: "partial",
    });
  });

  it("keeps an incomplete empty dependency observation Unknown", () => {
    const response = graph();
    const issue = response.data?.repository?.issues.nodes?.[0];
    if (issue !== null && issue !== undefined && issue.blockedBy !== null) {
      issue.blockedBy.nodes = [];
      issue.blockedBy.pageInfo.hasNextPage = true;
    }

    const projection = projectGraphResponse(response);
    expect(projection.deliveries[0]?.issue).toMatchObject({
      blockedBy: [],
      dependenciesAvailability: "partial",
    });
  });

  it("links the no-human-action aggregate to its contributing Steward watch", () => {
    const projection = projectGraphResponse(graph());

    expect(projection.executive.humanAction).toMatchObject({
      value: "None in current watches",
      source: {
        label: "Steward watch",
        url: "https://github.example/comments/watch",
        kind: "structured",
      },
    });
  });

  it("marks an unavailable Roadmap value partial", () => {
    const response = graph();
    const repository = response.data?.repository;
    if (repository !== null && repository !== undefined) repository.roadmap = null;

    const projection = projectGraphResponse(response);
    expect(projection.fetchHealth).toBe("partial");
    expect(projection.executive.productHorizon).toMatchObject({
      value: null,
      availability: "partial",
    });
  });

  it("never serializes credentials and fails unavailable on request errors", async () => {
    const fetchImpl = vi.fn<typeof fetch>().mockRejectedValue(new Error("secret github_pat_test"));
    const projection = await observeRepository({ token: "github_pat_test", fetchImpl });

    expect(projection.fetchHealth).toBe("unavailable");
    expect(JSON.stringify(projection)).not.toContain("github_pat_test");
  });
});

describe("Product Roadmap horizon", () => {
  it("accepts one exact value in one exact section", () => {
    expect(parseProductHorizon("## Current horizon\n\n> **06 · Team Workspace Beta**\n\n## Next"))
      .toBe("06 · Team Workspace Beta");
  });

  it("returns Unknown input for duplicate or malformed authority", () => {
    expect(parseProductHorizon("## Current horizon\n> **One**\n> **Two**")).toBeNull();
    expect(parseProductHorizon("## Current horizon\nReady in prose")).toBeNull();
  });
});
