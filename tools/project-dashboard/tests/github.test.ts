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
    expect(projection.deliveriesAvailability).toBe("complete");
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

  it("keeps a fully observed empty delivery set complete", () => {
    const response = graph();
    const repository = response.data?.repository;
    if (repository !== null && repository !== undefined) {
      repository.issues.nodes = [];
      repository.pullRequests.nodes = [];
    }

    const projection = projectGraphResponse(response);
    expect(projection.deliveries).toEqual([]);
    expect(projection.deliveriesAvailability).toBe("complete");
  });

  it("tracks empty Issue metadata independently from other Issue facts", () => {
    const response = graph();
    const issue = response.data?.repository?.issues.nodes?.[0];
    if (issue !== null && issue !== undefined && issue.labels !== null) {
      issue.labels.nodes = [];
      issue.blockedBy = null;
    }

    const projection = projectGraphResponse(response);
    expect(projection.deliveries[0]?.issue).toMatchObject({
      labels: [],
      labelsAvailability: "complete",
      milestone: null,
      milestoneAvailability: "complete",
      dependenciesAvailability: "partial",
    });
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

  it("classifies a misplaced canonical marker as malformed rather than missing", () => {
    const response = graph();
    const pull = response.data?.repository?.pullRequests.nodes?.[0];
    const comment = pull?.comments?.nodes?.[0];
    if (comment !== null && comment !== undefined) comment.body = `preface\n${comment.body}`;

    const projection = projectGraphResponse(response);
    expect(projection.deliveries[0]?.pullRequest?.handoff).toMatchObject({
      status: "unknown",
      reason: "Agent handoff marker-not-first-line",
    });
  });

  it("does not let an untrusted duplicate suppress current owner evidence", () => {
    const response = graph();
    const comments = response.data?.repository?.pullRequests.nodes?.[0]?.comments;
    const ownerComment = comments?.nodes?.[0];
    if (comments !== null && comments !== undefined && ownerComment !== null && ownerComment !== undefined) {
      comments.nodes?.push({
        ...ownerComment,
        id: "untrusted-duplicate",
        url: "https://github.example/comments/untrusted",
        author: { login: "attacker" },
        authorAssociation: "NONE",
      });
    }

    const projection = projectGraphResponse(response);
    expect(projection.deliveries[0]?.pullRequest?.handoff).toMatchObject({
      status: "current",
      source: { url: "https://github.example/comments/handoff" },
    });
  });

  it("does not let an untrusted marker make an unrelated PR relevant", () => {
    const response = graph();
    const pulls = response.data?.repository?.pullRequests;
    const pull = pulls?.nodes?.[0];
    const watch = pull?.comments?.nodes?.[1];
    if (
      pulls !== undefined &&
      pull !== null &&
      pull !== undefined &&
      watch !== null &&
      watch !== undefined
    ) {
      pulls.nodes?.push({
        ...pull,
        number: 231,
        url: "https://github.example/pulls/231",
        closingIssuesReferences: {
          pageInfo: { hasNextPage: false },
          nodes: [{ number: 999 }],
        },
        comments: {
          pageInfo: { hasNextPage: false },
          nodes: [{
            ...watch,
            id: "untrusted-watch",
            url: "https://github.example/comments/untrusted-watch",
            author: { login: "attacker" },
            authorAssociation: "NONE",
          }],
        },
      });
    }

    const projection = projectGraphResponse(response);
    expect(projection.deliveries.find((lane) => lane.pullRequest?.number === 231)?.pullRequest)
      .toMatchObject({ stewardWatch: { status: "unknown", reason: "Steward watch producer-untrusted" } });
    expect(projection.executive.humanAction).toMatchObject({
      value: "None in current watches",
      availability: "complete",
    });
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
      linkageAvailability: "partial",
      handoff: { status: "unknown", reason: "Issue linkage Unknown" },
      stewardWatch: { status: "unknown", reason: "Issue linkage Unknown" },
    });
  });

  it("associates every observed Issue closed by a multi-Issue pull request", () => {
    const response = graph();
    const repository = response.data?.repository;
    const firstIssue = repository?.issues.nodes?.[0];
    const pull = repository?.pullRequests.nodes?.[0];
    if (
      repository !== null &&
      repository !== undefined &&
      firstIssue !== null &&
      firstIssue !== undefined &&
      pull !== null &&
      pull !== undefined &&
      pull.closingIssuesReferences !== null
    ) {
      repository.issues.nodes?.push({
        ...firstIssue,
        number: 231,
        title: "Second linked Issue",
        url: "https://github.example/issues/231",
      });
      pull.closingIssuesReferences.nodes?.push({ number: 231 });
    }

    const projection = projectGraphResponse(response);
    const linkedLanes = projection.deliveries.filter((lane) => lane.pullRequest?.number === 230);
    expect(linkedLanes.map((lane) => lane.issue?.number)).toEqual([229, 231]);
    expect(linkedLanes.every((lane) => lane.linkageAvailability === "complete")).toBe(true);
    expect(linkedLanes.every((lane) => lane.pullRequest?.handoff.status === "unknown")).toBe(true);
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
    expect(projection.deliveries[0]?.pullRequest).toMatchObject({
      identityAvailability: "complete",
      nativeAvailability: "complete",
    });
    expect(projection.executive.humanAction).toMatchObject({
      value: "None in current watches",
      availability: "complete",
    });
    expect(projection.fetchHealth).toBe("partial");
  });

  it("scopes GraphQL errors to their affected response path", () => {
    const response = graph();
    const repository = response.data?.repository;
    if (repository !== null && repository !== undefined) repository.roadmap = null;
    response.errors = [{ message: "Roadmap unavailable", path: ["repository", "roadmap"] }];

    const projection = projectGraphResponse(response);
    expect(projection.fetchHealth).toBe("partial");
    expect(projection.executive.productHorizon.availability).toBe("partial");
    expect(projection.executive.activeCount).toMatchObject({ value: 1, availability: "complete" });
    expect(projection.deliveries[0]?.pullRequest).toMatchObject({
      checks: { availability: "complete" },
      handoff: { status: "current" },
      stewardWatch: { status: "current" },
    });
    expect(projection.executive.humanAction).toMatchObject({
      value: "None in current watches",
      availability: "complete",
    });
  });

  it("keeps structured evidence current when only check observation errors", () => {
    const response = graph();
    response.errors = [{
      message: "Checks unavailable",
      path: ["repository", "pullRequests", "nodes", 0, "statusCheckRollup"],
    }];

    const projection = projectGraphResponse(response);
    expect(projection.deliveries[0]?.pullRequest).toMatchObject({
      checks: { availability: "partial" },
      handoff: { status: "current" },
      stewardWatch: { status: "current" },
    });
    expect(projection.executive.humanAction).toMatchObject({
      value: "None in current watches",
      availability: "complete",
    });
  });

  it("keeps no-human-action Unknown when an unclassified PR can hide a watch", () => {
    const response = graph();
    const pulls = response.data?.repository?.pullRequests;
    const pull = pulls?.nodes?.[0];
    if (pulls !== undefined && pull !== null && pull !== undefined) {
      pulls.nodes?.push({
        ...pull,
        number: 231,
        url: "https://github.example/pulls/231",
        closingIssuesReferences: {
          pageInfo: { hasNextPage: false },
          nodes: [{ number: 999 }],
        },
        comments: null,
      });
    }

    const projection = projectGraphResponse(response);
    expect(projection.deliveries.find((lane) => lane.pullRequest?.number === 231)?.pullRequest)
      .toMatchObject({ stewardWatch: { status: "unknown" } });
    expect(projection.executive.humanAction).toMatchObject({
      value: null,
      availability: "partial",
    });
  });

  it("does not invent a status-context head when GitHub omits its commit", () => {
    const response = graph();
    const pull = response.data?.repository?.pullRequests.nodes?.[0];
    if (pull !== null && pull !== undefined) {
      pull.statusCheckRollup = {
        contexts: {
          pageInfo: { hasNextPage: false },
          nodes: [{
            __typename: "StatusContext",
            id: "status",
            context: "legacy status",
            state: "SUCCESS",
            targetUrl: "https://github.example/status/1",
            commit: null,
          }],
        },
      };
    }

    const projection = projectGraphResponse(response);
    expect(projection.deliveries[0]?.pullRequest?.checks.items[0]).toMatchObject({
      name: "legacy status",
      headSha: null,
    });
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

  it("keeps Issue-to-PR linkage complete when only unrelated pull details are partial", () => {
    const response = graph();
    const repository = response.data?.repository;
    const firstIssue = repository?.issues.nodes?.[0];
    const pull = repository?.pullRequests.nodes?.[0];
    if (
      repository !== null &&
      repository !== undefined &&
      firstIssue !== null &&
      firstIssue !== undefined &&
      pull !== null &&
      pull !== undefined
    ) {
      repository.issues.nodes?.push({
        ...firstIssue,
        number: 231,
        title: "Issue with no implementation pull request",
        url: "https://github.example/issues/231",
      });
      pull.statusCheckRollup = null;
    }

    const projection = projectGraphResponse(response);
    const issueOnlyLane = projection.deliveries.find((lane) => lane.issue?.number === 231);
    expect(projection.fetchHealth).toBe("partial");
    expect(issueOnlyLane).toMatchObject({
      pullRequest: null,
      linkageAvailability: "complete",
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

  it("keeps milestone errors out of the critical-path availability", () => {
    const response = graph();
    response.errors = [{
      message: "Milestone unavailable",
      path: ["repository", "issues", "nodes", 0, "milestone"],
    }];

    const projection = projectGraphResponse(response);
    expect(projection.deliveries[0]?.issue).toMatchObject({
      milestoneAvailability: "partial",
      availability: "partial",
    });
    expect(projection.criticalPath.availability).toBe("complete");
    expect(projection.fetchHealth).toBe("partial");
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
