import { describe, expect, it } from "vitest";

import { normalizeRepositorySnapshot } from "../src/server/normalize.ts";
import type { RawIssue, RawPullRequest, RawRepositorySnapshot } from "../src/shared/types.ts";

const observedAt = "2026-08-30T00:00:00.000Z";
const mainSha = "a".repeat(40);
const headSha = "b".repeat(40);

function snapshot(overrides: Partial<RawRepositorySnapshot> = {}): RawRepositorySnapshot {
  return {
    repoName: "nurockplayer/tachiko-work",
    repoUrl: "https://github.com/nurockplayer/tachiko-work",
    observedAt,
    mainSha,
    productHorizon: "05 · Designer MVP",
    productHorizonUrl: `https://github.com/nurockplayer/tachiko-work/blob/${mainSha}/docs/product/product-roadmap.md`,
    fetchHealth: "healthy",
    failures: [],
    issues: [],
    pullRequests: [],
    recentCompletions: [],
    ...overrides,
  };
}

function issue(number = 169): RawIssue {
  return {
    number,
    title: "Build read-only dashboard",
    url: `https://github.com/nurockplayer/tachiko-work/issues/${number}`,
    body: "## Status\n\n**READY FOR BOUNDED IMPLEMENTATION**\n\nOwner: `agent:codex`",
    updatedAt: observedAt,
    milestone: null,
    blockedBy: [],
    comments: [
      {
        id: "handoff-issue",
        body: "<!-- agent-handoff:v1 -->\nOWNER: agent:codex\nSTATE: ready",
        url: `https://github.com/nurockplayer/tachiko-work/issues/${number}#issuecomment-1`,
        createdAt: observedAt,
        updatedAt: observedAt,
      },
    ],
  };
}

function pullRequest(): RawPullRequest {
  return {
    number: 200,
    title: "Dashboard v0",
    url: "https://github.com/nurockplayer/tachiko-work/pull/200",
    body: "Closes #169",
    headSha,
    baseRefName: "main",
    baseSha: mainSha,
    mergeBaseSha: mainSha,
    relationToMain: "current",
    authorityPathsChangedOnMain: [],
    requiredChecks: [],
    issueNumbers: [169],
    comments: [
      {
        id: "handoff-pr",
        body: `<!-- agent-handoff:v1 -->\nOWNER: agent:codex\nSTATE: merge-ready\nHEAD: ${headSha}\nLAST CHECKED MAIN: ${mainSha}`,
        url: "https://github.com/nurockplayer/tachiko-work/pull/200#issuecomment-2",
        createdAt: observedAt,
        updatedAt: observedAt,
      },
    ],
    checksObservedHeadSha: headSha,
    checks: [{ name: "test", integrationId: null, status: "completed", conclusion: "success", url: null }],
    reviewDecision: "approved",
    reviews: [{ state: "approved", headSha, url: "https://github.com/review", submittedAt: observedAt }],
    reviewThreads: [],
    updatedAt: observedAt,
  };
}

describe("normalizeRepositorySnapshot", () => {
  it("keeps a Ready Issue without a PR as its own healthy delivery lane", () => {
    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()] }));

    expect(projection.deliveries).toHaveLength(1);
    expect(projection.deliveries[0]).toMatchObject({
      issue: { number: 169, readiness: "ready" },
      owner: "agent:codex",
      phase: "ready",
      pr: null,
      action: { owner: "none" },
    });
  });

  it("does not let a stale no-PR handoff override live Issue readiness or request human action", () => {
    const coordinated = issue();
    coordinated.comments[0]!.body = [
      "<!-- agent-handoff:v1 -->",
      "OWNER: agent:codex",
      "STATE: parked",
      `LAST CHECKED MAIN: ${"d".repeat(40)}`,
      "HUMAN ACTION: required",
    ].join("\n");

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [coordinated] }));
    const lane = projection.deliveries[0];

    expect(lane?.handoff.condition).toBe("stale");
    expect(lane?.issue.readiness).toBe("ready");
    expect(lane?.phase).toBe("ready");
    expect(lane?.action.owner).toBe("none");
    expect(projection.attention.humanActionRequired).toBe(false);
  });

  it("does not turn guardrail prose into parked, blocked, or human-required state", () => {
    const coordinated = issue();
    coordinated.body += "\n\n## Guardrails\nPark on overlap. Blocked work must not race. No human action is required.";
    coordinated.comments[0]!.body = `<!-- agent-handoff:v1 -->\nOWNER: agent:codex\nSTATE: implementing\nLAST CHECKED MAIN: ${mainSha}\nHUMAN ACTION: none`;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [coordinated] }));

    expect(projection.deliveries[0]?.issue.readiness).toBe("active");
    expect(projection.deliveries[0]?.phase).toBe("implementing");
    expect(projection.deliveries[0]?.action.owner).toBe("none");
    expect(projection.attention.humanActionRequired).toBe(false);
  });

  it("omits uncoordinated backlog prose from active delivery lanes", () => {
    const backlog = issue(50);
    backlog.comments = [];
    backlog.body = "## Status\nReady for future research when dependencies clear.";

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [backlog] }));

    expect(projection.deliveries).toEqual([]);
  });

  it("treats live blocked-by dependencies as blocked even when Issue prose says Ready", () => {
    const dependent = issue();
    dependent.blockedBy = [{ number: 187, title: "Open canonical project", url: "https://github.com/nurockplayer/tachiko-work/issues/187" }];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [dependent] }));
    const lane = projection.deliveries[0];

    expect(lane?.issue.readiness).toBe("blocked");
    expect(lane?.phase).toBe("blocked");
    expect(lane?.blockers).toContain("Live Issue dependencies block this lane: #187.");
  });

  it("does not treat Decision-Ready authority work as production implementation readiness", () => {
    const decision = issue(190);
    decision.body = "## Status\n\n**DECISION-READY**\n\nOwner: `agent:chatgpt`";
    decision.comments[0]!.body = "<!-- agent-handoff:v1 -->\nOWNER: agent:chatgpt\nSTATE: Decision-Ready";

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [decision] }));

    expect(projection.deliveries).toEqual([]);
  });

  it("invalidates checks and a merge-ready handoff when the PR head moves", () => {
    const pr = pullRequest();
    pr.headSha = "c".repeat(40);

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.checks.status).toBe("unknown");
    expect(lane?.handoff.condition).toBe("inconsistent");
    expect(lane?.phase).not.toBe("merge_gate");
    expect(lane?.blockers).toContain("Checks were not observed for the current PR head.");
  });

  it("projects a current substantive review finding as review-fix without founder escalation", () => {
    const pr = pullRequest();
    pr.reviewDecision = "changes_requested";
    pr.reviewThreads = [
      { resolved: false, outdated: false, body: "[P2] Preserve exact-head identity", url: "https://github.com/thread" },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.phase).toBe("review_fix");
    expect(lane?.reviews.substantiveUnresolvedCount).toBe(1);
    expect(lane?.action).toMatchObject({ owner: "codex" });
    expect(projection.attention.humanActionRequired).toBe(false);
  });

  it("treats an old-head changes-requested review without a current finding as rereview", () => {
    const pr = pullRequest();
    pr.headSha = "c".repeat(40);
    pr.checksObservedHeadSha = pr.headSha;
    pr.comments[0]!.body = pr.comments[0]!.body.replaceAll(headSha, pr.headSha);
    pr.reviewDecision = "changes_requested";
    pr.reviewThreads = [];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.reviews.status).toBe("stale");
    expect(lane?.phase).toBe("rereview");
    expect(lane?.action.owner).toBe("none");
  });

  it("never treats green exact-head checks with hosted review pending as merge-ready", () => {
    const pr = pullRequest();
    pr.reviewDecision = "review_required";
    pr.reviews = [];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.phase).toBe("rereview");
  });

  it("never treats an optional green check as satisfying an unobserved required check", () => {
    const pr = pullRequest();
    pr.requiredChecks = [{ name: "release", integrationId: null }];
    pr.checks = [{ name: "optional-smoke", integrationId: null, status: "completed", conclusion: "success", url: null }];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.checks.status).toBe("success");
    expect(lane?.checks.requiredStatus).toBe("unsatisfied");
    expect(lane?.phase).toBe("validating");
    expect(lane?.blockers).toContain("Required check release was not observed for the current PR head.");
  });

  it("does not let a stale handoff parked claim override live PR state", () => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body
      .replace("STATE: merge-ready", "STATE: parked")
      .replace(mainSha, "d".repeat(40));

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.handoff.condition).toBe("stale");
    expect(lane?.phase).toBe("validating");
    expect(lane?.issue.readiness).toBe("active");
    expect(lane?.action.owner).toBe("none");
  });

  it("preserves a current blocked handoff state for an open PR", () => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body.replace("STATE: merge-ready", "STATE: blocked");

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.handoff.condition).toBe("current");
    expect(lane?.issue.readiness).toBe("blocked");
    expect(lane?.phase).toBe("blocked");
    expect(lane?.blockers).toContain("The current canonical handoff reports this lane blocked.");
  });

  it("does not block the merge gate on P3-only review threads", () => {
    const pr = pullRequest();
    pr.reviewThreads = [
      { resolved: false, outdated: false, body: "[P3] Consider a shorter label", url: "https://github.com/thread" },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.reviews.unresolvedThreadCount).toBe(1);
    expect(lane?.reviews.substantiveUnresolvedCount).toBe(0);
    expect(lane?.phase).toBe("merge_gate");
  });

  it("keeps independent simultaneous work in separate lanes", () => {
    const issueTwo = issue(187);
    issueTwo.title = "Open canonical project";
    issueTwo.milestone = "05 · Designer MVP";

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue(), issueTwo] }));

    expect(projection.deliveries.map((lane) => lane.issue.number)).toEqual([187, 169]);
    expect(projection.currentWork.currentHorizon).toEqual(["issue-187"]);
    expect(projection.currentWork.independent).toEqual(["issue-169"]);
  });

  it("keeps stacked pull requests for one Issue in distinct delivery lanes", () => {
    const first = pullRequest();
    const second = pullRequest();
    second.number = 201;
    second.url = "https://github.com/nurockplayer/tachiko-work/pull/201";
    second.headSha = "c".repeat(40);
    second.checksObservedHeadSha = second.headSha;
    second.comments[0]!.body = second.comments[0]!.body.replaceAll(headSha, second.headSha);

    const projection = normalizeRepositorySnapshot(
      snapshot({ issues: [issue()], pullRequests: [first, second] }),
    );

    expect(projection.deliveries.map((lane) => lane.id)).toEqual([
      "issue-169-pr-200",
      "issue-169-pr-201",
    ]);
    expect(new Set(projection.deliveries.map((lane) => lane.id)).size).toBe(2);
    expect(projection.deliveries.map((lane) => lane.phase)).toEqual(["blocked", "blocked"]);
    for (const lane of projection.deliveries) {
      expect(lane.blockers).toContain("Multiple open pull requests claim Issue #169: #200, #201.");
    }
  });

  it("keeps merge readiness validating while live-main reconciliation is unknown", () => {
    const pr = pullRequest();
    pr.relationToMain = "unknown";
    pr.mergeBaseSha = null;
    pr.authorityPathsChangedOnMain = null;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.authorityDrift).toBe("unknown");
    expect(lane?.phase).toBe("validating");
    expect(lane?.blockers).toContain("Live-main and authority-drift reconciliation could not be observed.");
  });

  it("marks main movement as suspected authority drift until the handoff reconciles it", () => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body.replace(mainSha, "d".repeat(40));
    pr.relationToMain = "diverged";
    pr.authorityPathsChangedOnMain = ["docs/decisions/ADR-0029-dashboard-boundary.md"];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.authorityDrift).toBe("suspected");
    expect(projection.deliveries[0]?.handoff.condition).toBe("stale");
    expect(projection.deliveries[0]?.blockers).toContain(
      "Accepted-authority candidates changed on main: docs/decisions/ADR-0029-dashboard-boundary.md.",
    );
  });

  it("keeps current-work classification and dependency health unknown when roadmap authority is unavailable", () => {
    const projection = normalizeRepositorySnapshot(
      snapshot({
        productHorizon: null,
        fetchHealth: "partial",
        failures: ["Product Roadmap observation failed."],
        issues: [issue()],
      }),
    );

    expect(projection.currentWork).toMatchObject({
      currentHorizon: [],
      independent: [],
      unclassified: ["issue-169"],
      horizonStatus: "unknown",
      dependencyHealth: "unknown",
    });
  });

  it("preserves partial and unknown state when GitHub source data is unavailable", () => {
    const projection = normalizeRepositorySnapshot(
      snapshot({
        mainSha: null,
        fetchHealth: "partial",
        failures: ["GitHub pull-request observation failed"],
        pullRequests: null,
      }),
    );

    expect(projection.repo.fetchHealth).toBe("partial");
    expect(projection.repo.mainSha).toBeNull();
    expect(projection.attention.humanActionRequired).toBeNull();
    expect(projection.attention.reasons).toContain("GitHub pull-request observation failed");
  });
});
