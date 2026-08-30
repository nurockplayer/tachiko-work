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
    defaultBranchName: "main",
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
    authorAssociation: "OWNER",
    updatedAt: observedAt,
    lastEditedAt: null,
    milestone: null,
    blockedBy: [],
    commentsComplete: true,
    comments: [
      {
        id: "handoff-issue",
        body: "<!-- agent-handoff:v1 -->\nOWNER: agent:codex\nSTATE: ready",
        url: `https://github.com/nurockplayer/tachiko-work/issues/${number}#issuecomment-1`,
        authorAssociation: "OWNER",
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
    author: { login: "nurockplayer", type: "user" },
    isDraft: false,
    headSha,
    baseRefName: "main",
    baseSha: mainSha,
    mergeBaseSha: mainSha,
    relationToMain: "current",
    changedPaths: ["tools/project-dashboard/src/server/normalize.ts"],
    authorityPathsChangedOnMain: [],
    mergeable: "mergeable",
    mergeStateStatus: "clean",
    requiredChecks: [],
    issueNumbers: [169],
    issueNumbersComplete: true,
    commentsComplete: true,
    comments: [
      {
        id: "handoff-pr",
        body: [
          "<!-- agent-handoff:v1 -->",
          "ISSUE: #169",
          "OWNER: agent:codex",
          "STATE: merge-ready",
          `HEAD: ${headSha}`,
          `LAST CHECKED MAIN: ${mainSha}`,
          "SCOPE BOUNDARY: bounded dashboard tooling",
          "VALIDATION EVIDENCE: exact-head gates passed",
          "UNRESOLVED REVIEW STATE: none",
          "NEXT ACTION: merge gate",
          "HUMAN ACTION: none",
        ].join("\n"),
        url: "https://github.com/nurockplayer/tachiko-work/pull/200#issuecomment-2",
        authorAssociation: "OWNER",
        createdAt: observedAt,
        updatedAt: observedAt,
      },
    ],
    checksObservedHeadSha: headSha,
    checks: [{ name: "test", integrationId: null, attemptAt: null, status: "completed", conclusion: "success", url: null }],
    reviewDecision: "approved",
    reviews: [{ state: "approved", author: "reviewer", body: "", headSha, url: "https://github.com/review", submittedAt: observedAt }],
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

  it("does not accept a Ready claim from an untrusted Issue author", () => {
    const external = issue();
    external.authorAssociation = "NONE";
    external.comments = [];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [external] }));

    expect(projection.deliveries).toEqual([]);
    expect(projection.attention).toMatchObject({
      humanActionRequired: true,
      reasons: ["No Ready delivery remains; the Project Steward must select or ready successor work."],
    });
  });

  it("accepts a trusted pre-PR handoff as an independent readiness signal", () => {
    const external = issue();
    external.authorAssociation = "NONE";
    external.comments[0]!.body += `\nLAST CHECKED MAIN: ${mainSha}`;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [external] }));

    expect(projection.deliveries[0]).toMatchObject({
      issue: { readiness: "ready" },
      owner: "agent:codex",
      phase: "ready",
    });
  });

  it("ignores an untrusted pre-PR handoff marker and human-action claim", () => {
    const injected = issue();
    injected.body = "## Status\n\nBacklog\n\nOwner: `agent:codex`";
    injected.comments[0]!.authorAssociation = "NONE";
    injected.comments[0]!.body = [
      "<!-- agent-handoff:v1 -->",
      "OWNER: agent:codex",
      "STATE: human_required",
      `LAST CHECKED MAIN: ${mainSha}`,
      "HUMAN ACTION: Steward approval required",
    ].join("\n");

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [injected] }));

    expect(projection.deliveries).toEqual([]);
    expect(projection.attention).toMatchObject({
      humanActionRequired: true,
      reasons: ["No Ready delivery remains; the Project Steward must select or ready successor work."],
    });
  });

  it("does not treat an explicit Not Ready state as Ready", () => {
    const notReady = issue();
    notReady.comments[0]!.body = [
      "<!-- agent-handoff:v1 -->",
      "OWNER: agent:codex",
      "STATE: Not Ready",
      `LAST CHECKED MAIN: ${mainSha}`,
    ].join("\n");

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [notReady] }));

    expect(projection.deliveries).toEqual([]);
    expect(projection.attention).toMatchObject({
      humanActionRequired: true,
      reasons: ["No Ready delivery remains; the Project Steward must select or ready successor work."],
    });
  });

  it.each(["Not yet Ready", "Not currently Ready"])(
    "does not let qualified negated readiness reach the merge gate: %s",
    (status) => {
      const notReady = issue();
      notReady.body = `## Status\n\n**${status}**\n\nOwner: \`agent:codex\``;

      const projection = normalizeRepositorySnapshot(snapshot({ issues: [notReady], pullRequests: [pullRequest()] }));

      expect(projection.deliveries[0]?.issue.readiness).toBe("unknown");
      expect(projection.deliveries[0]?.phase).toBe("validating");
    },
  );

  it.each([
    "Not blocked; Ready for bounded implementation.",
    "Never blocked. Ready.",
  ])("does not treat unrelated negation as negated readiness: %s", (status) => {
    const ready = issue();
    ready.body = `## Status\n\n${status}\n\nOwner: \`agent:codex\``;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [ready] }));

    expect(projection.deliveries[0]?.issue.readiness).toBe("ready");
    expect(projection.deliveries[0]?.phase).toBe("ready");
  });

  it.each([
    "Previously Not Ready; now Ready.",
    "Not Ready (resolved); Ready.",
    "Ready. Previously Not Ready.",
    "Previously Backlog; now Ready.",
    "Ready; formerly Backlog.",
  ])("honors the current Ready claim after a historical Not Ready claim: %s", (status) => {
    const ready = issue();
    ready.body = `## Status\n\n${status}\n\nOwner: \`agent:codex\``;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [ready] }));

    expect(projection.deliveries[0]?.issue.readiness).toBe("ready");
    expect(projection.deliveries[0]?.phase).toBe("ready");
  });

  it.each([
    "Ready; now Not Ready.",
    "Ready; now Not Ready pending approval.",
    "Ready; Not Ready after approval.",
    "Ready; now Backlog.",
    "Ready; Ready only after approval.",
    "Ready; ready to proceed once access is granted.",
  ])("honors a current Not Ready claim after an earlier Ready claim: %s", (status) => {
    const notReady = issue();
    notReady.body = `## Status\n\n${status}\n\nOwner: \`agent:codex\``;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [notReady] }));

    expect(projection.deliveries).toEqual([]);
  });

  it.each([
    "Previously blocked; Ready.",
    "Was blocked; Ready.",
    "Blocked (resolved); Ready.",
    "Blocked but resolved; Ready.",
    "Blocked, but now resolved; Ready.",
  ])("does not treat a historical blocked claim as a current blocker: %s", (status) => {
    const ready = issue();
    ready.body = `## Status\n\n${status}\n\nOwner: \`agent:codex\``;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [ready] }));

    expect(projection.deliveries[0]?.issue.readiness).toBe("ready");
    expect(projection.deliveries[0]?.phase).toBe("ready");
  });

  it.each([
    "Blocked; now Ready.",
    "Parked; now Ready.",
    "Decision-Ready (resolved); now Ready.",
  ])("honors a current Ready claim after an earlier delivery state: %s", (status) => {
    const ready = issue();
    ready.body = `## Status\n\n${status}\n\nOwner: \`agent:codex\``;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [ready] }));

    expect(projection.deliveries[0]?.issue.readiness).toBe("ready");
    expect(projection.deliveries[0]?.phase).toBe("ready");
  });

  it.each([
    { status: "Ready; now Blocked.", readiness: "blocked" },
    { status: "Ready; now Parked.", readiness: "parked" },
  ])("honors a current $readiness claim after an earlier Ready claim", ({ status, readiness }) => {
    const changed = issue();
    changed.body = `## Status\n\n${status}\n\nOwner: \`agent:codex\``;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [changed] }));

    expect(projection.deliveries[0]?.issue.readiness).toBe(readiness);
  });

  it.each([
    "Blocked; now no longer Blocked.",
    "Parked; now no longer Parked.",
    "Blocked; now Unblocked.",
    "Parked; now Unparked.",
    "Active; now Inactive.",
  ])("fails closed after a current delivery state is explicitly cleared: %s", (status) => {
    const cleared = issue();
    cleared.body = `## Status\n\n${status}\n\nOwner: \`agent:codex\``;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [cleared], pullRequests: [pullRequest()] }));
    const lane = projection.deliveries[0];

    expect(lane?.issue.readiness).toBe("unknown");
    expect(lane?.phase).toBe("validating");
    expect(lane?.phase).not.toBe("merge_gate");
  });

  it.each([
    { status: "No longer Blocked; now Blocked.", readiness: "blocked" },
    { status: "No longer Parked; now Parked.", readiness: "parked" },
    { status: "Unblocked; now Blocked.", readiness: "blocked" },
    { status: "Unparked; now Parked.", readiness: "parked" },
    { status: "Inactive; now Active.", readiness: "active" },
  ])("honors a reasserted current $readiness state", ({ status, readiness }) => {
    const changed = issue();
    changed.body = `## Status\n\n${status}\n\nOwner: \`agent:codex\``;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [changed] }));

    expect(projection.deliveries[0]?.issue.readiness).toBe(readiness);
  });

  it.each([
    "Not parked; Ready for bounded implementation.",
    "Not currently parked; Ready.",
    "Never parked. Ready.",
  ])("does not treat a negated parked claim as parked: %s", (status) => {
    const ready = issue();
    ready.body = `## Status\n\n${status}\n\nOwner: \`agent:codex\``;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [ready] }));

    expect(projection.deliveries[0]?.issue.readiness).toBe("ready");
    expect(projection.deliveries[0]?.phase).toBe("ready");
  });

  it("uses claim-bounded parked parsing for handoff-derived status", () => {
    const ready = issue();
    ready.comments[0]!.body = [
      "<!-- agent-handoff:v1 -->",
      "OWNER: agent:codex",
      "STATE: Not parked; Ready",
      `LAST CHECKED MAIN: ${mainSha}`,
    ].join("\n");

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [ready] }));

    expect(projection.deliveries[0]?.issue.readiness).toBe("ready");
    expect(projection.deliveries[0]?.phase).toBe("ready");
  });

  it("preserves an affirmative parked Issue status", () => {
    const parked = issue();
    parked.body = "## Status\n\nParked\n\nOwner: `agent:codex`";

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [parked] }));

    expect(projection.deliveries[0]?.issue.readiness).toBe("parked");
    expect(projection.deliveries[0]?.phase).toBe("parked");
  });

  it("uses claim-bounded blocked negation for handoff-derived status", () => {
    const ready = issue();
    ready.comments[0]!.body = [
      "<!-- agent-handoff:v1 -->",
      "OWNER: agent:codex",
      "STATE: Not currently blocked; Ready",
      `LAST CHECKED MAIN: ${mainSha}`,
    ].join("\n");

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [ready] }));

    expect(projection.deliveries[0]?.issue.readiness).toBe("ready");
    expect(projection.deliveries[0]?.phase).toBe("ready");
  });

  it("does not treat a qualified negated active handoff state as active", () => {
    const ready = issue();
    ready.comments[0]!.body = [
      "<!-- agent-handoff:v1 -->",
      "OWNER: agent:codex",
      "STATE: Not currently active",
      `LAST CHECKED MAIN: ${mainSha}`,
    ].join("\n");

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [ready] }));

    expect(projection.deliveries).toEqual([]);
  });

  it("honors a cleared active Issue transition instead of entering the merge gate", () => {
    const inactive = issue();
    inactive.body = "## Status\n\nActive; now no longer Active.\n\nOwner: `agent:codex`";

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [inactive], pullRequests: [pullRequest()] }));
    const lane = projection.deliveries[0];

    expect(lane?.issue.readiness).toBe("unknown");
    expect(lane?.phase).toBe("validating");
    expect(lane?.phase).not.toBe("merge_gate");
  });

  it("honors a current active Issue claim after an earlier cleared claim", () => {
    const active = issue();
    active.body = "## Status\n\nNo longer Active; now Active.\n\nOwner: `agent:codex`";

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [active] }));

    expect(projection.deliveries[0]?.issue.readiness).toBe("active");
    expect(projection.deliveries[0]?.phase).toBe("implementing");
  });

  it.each(["Human required", "human_required", "Human required pending a decision"])(
    "routes authoritative %s status to human action without a handoff",
    (status) => {
      const escalated = issue();
      escalated.body = `## Status\n\n${status}\n\nOwner: \`agent:codex\``;
      escalated.comments = [];

      const projection = normalizeRepositorySnapshot(snapshot({ issues: [escalated] }));
      const lane = projection.deliveries[0];

      expect(projection.deliveries).toHaveLength(1);
      expect(lane?.issue.readiness).toBe("unknown");
      expect(lane?.phase).toBe("human_required");
      expect(lane?.action.owner).toBe("human");
      expect(projection.attention.humanActionRequired).toBe(true);
    },
  );

  it("honors a cleared Human-Required transition", () => {
    const cleared = issue();
    cleared.body = [
      "## Status",
      "",
      "Human-Required; now no longer Human-Required; Ready",
      "",
      "Owner: `agent:codex`",
    ].join("\n");
    cleared.comments = [];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [cleared] }));
    const lane = projection.deliveries[0];

    expect(lane?.issue.readiness).toBe("ready");
    expect(lane?.phase).toBe("ready");
    expect(lane?.action.owner).toBe("none");
    expect(projection.attention.humanActionRequired).toBe(false);
  });

  it("honors a current Human-Required claim after an earlier cleared claim", () => {
    const escalated = issue();
    escalated.body = "## Status\n\nNo longer Human-Required; now Human-Required.\n\nOwner: `agent:codex`";
    escalated.comments = [];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [escalated] }));

    expect(projection.deliveries[0]?.phase).toBe("human_required");
    expect(projection.attention.humanActionRequired).toBe(true);
  });

  it("lets a later explicit Ready state clear an earlier Human-Required claim", () => {
    const ready = issue();
    ready.body = "## Status\n\nHuman-Required; now Ready.\n\nOwner: `agent:codex`";
    ready.comments = [];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [ready] }));

    expect(projection.deliveries[0]?.issue.readiness).toBe("ready");
    expect(projection.deliveries[0]?.phase).toBe("ready");
    expect(projection.deliveries[0]?.action.owner).toBe("none");
    expect(projection.attention.humanActionRequired).toBe(false);
  });

  it("honors a cleared Human-Required transition in the canonical handoff state", () => {
    const cleared = issue();
    cleared.comments[0]!.body = [
      "<!-- agent-handoff:v1 -->",
      "OWNER: agent:codex",
      "STATE: Human-Required; now no longer Human-Required; Ready",
      `LAST CHECKED MAIN: ${mainSha}`,
    ].join("\n");

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [cleared] }));
    const lane = projection.deliveries[0];

    expect(lane?.issue.readiness).toBe("ready");
    expect(lane?.phase).toBe("ready");
    expect(lane?.action.owner).toBe("none");
    expect(projection.attention.humanActionRequired).toBe(false);
  });

  it("does not let an operational handoff elevate an unrecognized Issue status", () => {
    const backlog = issue();
    backlog.body = "## Status\n\nBacklog\n\nOwner: `agent:codex`";

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [backlog], pullRequests: [pullRequest()] }));
    const lane = projection.deliveries[0];

    expect(lane?.issue.readiness).toBe("unknown");
    expect(lane?.phase).toBe("validating");
    expect(lane?.blockers).toContain("The authoritative Issue status does not affirm that this lane is Ready or active.");
    expect(lane?.action.owner).toBe("human");
  });

  it("does not let a stale no-PR handoff override live Issue readiness but preserves human action", () => {
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
    expect(lane?.phase).toBe("human_required");
    expect(lane?.action.owner).toBe("human");
    expect(projection.attention.humanActionRequired).toBe(true);
  });

  it("preserves a stale no-PR blocked handoff while requiring reconciliation", () => {
    const coordinated = issue();
    coordinated.comments[0]!.body = [
      "<!-- agent-handoff:v1 -->",
      "OWNER: agent:codex",
      "STATE: blocked",
      `LAST CHECKED MAIN: ${"d".repeat(40)}`,
      "HUMAN ACTION: none",
    ].join("\n");

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [coordinated] }));
    const lane = projection.deliveries[0];

    expect(lane?.handoff.condition).toBe("stale");
    expect(lane?.pr).toBeNull();
    expect(lane?.issue.readiness).toBe("blocked");
    expect(lane?.phase).toBe("blocked");
    expect(lane?.blockers).toContain("Canonical handoff has not reconciled the observed live main.");
    expect(lane?.action.owner).toBe("codex");
  });

  it("routes a current no-PR blocked handoff to its delivery owner", () => {
    const coordinated = issue();
    coordinated.comments[0]!.body = [
      "<!-- agent-handoff:v1 -->",
      "OWNER: agent:codex",
      "STATE: blocked",
      `LAST CHECKED MAIN: ${mainSha}`,
      "HUMAN ACTION: none",
    ].join("\n");

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [coordinated] }));
    const lane = projection.deliveries[0];

    expect(lane?.handoff.condition).toBe("current");
    expect(lane?.phase).toBe("blocked");
    expect(lane?.blockers).toContain("The current canonical handoff reports this lane blocked.");
    expect(lane?.action.owner).toBe("codex");
    expect(lane?.action.reason).toBe("The current canonical handoff reports this lane blocked.");
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
    expect(lane?.action).toEqual({
      owner: "human",
      reason: "Issue dependency state requires Project Steward reconciliation.",
    });
    expect(projection.attention.humanActionRequired).toBe(true);
  });

  it("routes a pull request with live blocked-by dependencies to the Project Steward", () => {
    const dependent = issue();
    dependent.blockedBy = [{ number: 187, title: "Open canonical project", url: "https://github.com/nurockplayer/tachiko-work/issues/187" }];

    const projection = normalizeRepositorySnapshot(snapshot({
      issues: [dependent],
      pullRequests: [pullRequest()],
    }));
    const lane = projection.deliveries[0];

    expect(lane?.issue.readiness).toBe("blocked");
    expect(lane?.phase).toBe("blocked");
    expect(lane?.action).toEqual({
      owner: "human",
      reason: "Issue dependency state requires Project Steward reconciliation.",
    });
    expect(projection.attention.humanActionRequired).toBe(true);
  });

  it("keeps truncated Issue dependency state unknown and out of the merge gate", () => {
    const truncated = issue();
    truncated.blockedBy = null;
    const pr = pullRequest();

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [truncated], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.issue.readiness).toBe("unknown");
    expect(lane?.phase).toBe("validating");
    expect(lane?.blockers).toContain("Issue dependency state could not be fully observed.");
    expect(lane?.action.owner).toBe("human");
    expect(projection.attention.humanActionRequired).toBe(true);
  });

  it("does not let a merge-ready handoff override an authoritative Not Ready Issue", () => {
    const notReady = issue();
    notReady.body = "## Status\n\n**NOT READY**\n\nOwner: `agent:codex`";
    const pr = pullRequest();

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [notReady], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.issue.readiness).toBe("unknown");
    expect(lane?.phase).toBe("validating");
  });

  it("does not let a merge-ready handoff override an authoritative blocked Issue", () => {
    const blocked = issue();
    blocked.body = "## Status\n\n**BLOCKED**\n\nOwner: `agent:codex`";
    const pr = pullRequest();

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [blocked], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.issue.readiness).toBe("blocked");
    expect(lane?.phase).toBe("blocked");
    expect(lane?.blockers).toContain("The authoritative Issue status reports this lane blocked.");
  });

  it.each(["BLOCKED", "Blocked pending an upstream fix"])(
    "routes an authoritative %s Issue without a PR to the Steward",
    (status) => {
      const blocked = issue();
      blocked.body = `## Status\n\n**${status}**\n\nOwner: \`agent:codex\``;

      const projection = normalizeRepositorySnapshot(snapshot({ issues: [blocked] }));
      const lane = projection.deliveries[0];

      expect(lane?.phase).toBe("blocked");
      expect(lane?.blockers).toContain("The authoritative Issue status reports this lane blocked.");
      expect(lane?.action.owner).toBe("human");
      expect(projection.attention.humanActionRequired).toBe(true);
    },
  );

  it("does not treat Decision-Ready authority work as production implementation readiness", () => {
    const decision = issue(190);
    decision.body = "## Status\n\n**DECISION-READY**\n\nOwner: `agent:chatgpt`";
    decision.comments[0]!.body = "<!-- agent-handoff:v1 -->\nOWNER: agent:chatgpt\nSTATE: Decision-Ready";

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [decision] }));

    expect(projection.deliveries).toEqual([]);
  });

  it.each([
    "[Decision][M05 P1] Choose the delivery boundary",
    "[Research][M05 P1] Evaluate the delivery boundary",
  ])("keeps ordinary Ready %s work outside production delivery", (title) => {
    const authorityWork = issue();
    authorityWork.title = title;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [authorityWork] }));

    expect(projection.deliveries).toEqual([]);
  });

  it.each(["Decision", "Research"])(
    "allows a Decision-Ready %s authority Issue with a focused PR through the merge gate",
    (kind) => {
      const decision = issue();
      decision.title = `[${kind}][M05 P1] Choose the dashboard delivery boundary`;
      decision.body = "## Status\n\n**DECISION-READY**\n\nOwner: `agent:codex`";
      const pr = pullRequest();
      pr.changedPaths = ["docs/decisions/ADR-0029-dashboard-boundary.md"];

      const projection = normalizeRepositorySnapshot(snapshot({ issues: [decision], pullRequests: [pr] }));

      expect(projection.deliveries[0]?.issue.readiness).toBe("active");
      expect(projection.deliveries[0]?.phase).toBe("merge_gate");
    },
  );

  it("does not treat unrelated negation as negated Decision-Ready authority", () => {
    const decision = issue();
    decision.title = "[Decision][M05 P1] Choose the dashboard delivery boundary";
    decision.body = "## Status\n\nNot blocked; Decision-Ready.\n\nOwner: `agent:codex`";
    const pr = pullRequest();
    pr.changedPaths = ["docs/decisions/ADR-0029-dashboard-boundary.md"];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [decision], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.phase).toBe("merge_gate");
  });

  it("lets a conditional Decision-Ready claim supersede an earlier affirmative claim", () => {
    const decision = issue();
    decision.title = "[Decision][M05 P1] Choose the dashboard delivery boundary";
    decision.body = [
      "## Status",
      "",
      "Decision-Ready.",
      "",
      "Decision-Ready only after Steward approval.",
      "",
      "Owner: `agent:codex`",
    ].join("\n");
    const pr = pullRequest();
    pr.changedPaths = ["docs/decisions/ADR-0029-dashboard-boundary.md"];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [decision], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.issue.readiness).toBe("unknown");
    expect(lane?.phase).toBe("validating");
    expect(lane?.action.owner).toBe("human");
  });

  it("lets a conditional Active claim supersede an earlier Ready claim", () => {
    const delivery = issue();
    delivery.body = [
      "## Status",
      "",
      "Ready.",
      "",
      "Active only after Steward approval.",
      "",
      "Owner: `agent:codex`",
    ].join("\n");

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [delivery], pullRequests: [pullRequest()] }));

    expect(projection.deliveries[0]?.issue.readiness).toBe("unknown");
    expect(projection.deliveries[0]?.phase).toBe("validating");
  });

  it.each(["Decision", "Research"])(
    "keeps a Decision-Ready %s authority Issue without a focused PR outside delivery",
    (kind) => {
      const decision = issue();
      decision.title = `[${kind}][M05 P1] Choose the dashboard delivery boundary`;
      decision.body = "## Status\n\n**DECISION-READY**\n\nOwner: `agent:codex`";

      const projection = normalizeRepositorySnapshot(snapshot({ issues: [decision] }));

      expect(projection.deliveries).toEqual([]);
    },
  );

  it("preserves human escalation from a Decision-Ready authority Issue without a pull request", () => {
    const decision = issue();
    decision.title = "[Decision][M05 P1] Choose the dashboard delivery boundary";
    decision.body = "## Status\n\n**DECISION-READY**\n\nOwner: `agent:codex`";
    decision.comments[0]!.body = [
      "<!-- agent-handoff:v1 -->",
      "OWNER: agent:codex",
      "STATE: human_required",
      "HUMAN ACTION: Steward approval required",
      `LAST CHECKED MAIN: ${mainSha}`,
    ].join("\n");

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [decision] }));
    const lane = projection.deliveries[0];

    expect(projection.deliveries).toHaveLength(1);
    expect(lane?.issue.readiness).toBe("unknown");
    expect(lane?.phase).toBe("human_required");
    expect(lane?.action.owner).toBe("human");
    expect(projection.attention.humanActionRequired).toBe(true);
    expect(projection.attention.reasons).toContain(
      `#${decision.number}: The canonical coordination state requests human or Steward action.`,
    );
  });

  it.each([
    "NOT DECISION-READY",
    "Not Decision-Ready",
    "Not yet Decision Ready",
    "Not currently Decision-Ready",
    "Decision-Ready: false",
  ])("rejects a negated Decision-Ready authority status: %s", (status) => {
    const decision = issue();
    decision.title = "[Decision][M05 P1] Choose the dashboard delivery boundary";
    decision.body = `## Status\n\n**${status}**\n\nOwner: \`agent:codex\``;
    const pr = pullRequest();
    pr.changedPaths = ["docs/decisions/ADR-0029-dashboard-boundary.md"];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [decision], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.issue.readiness).toBe("unknown");
    expect(projection.deliveries[0]?.phase).toBe("validating");
  });

  it.each(["Decision", "Research"])(
    "keeps a Decision-Ready %s authority Issue paired with implementation changes out of the merge gate",
    (kind) => {
      const decision = issue();
      decision.title = `[${kind}][M05 P1] Choose the dashboard delivery boundary`;
      decision.body = "## Status\n\n**DECISION-READY**\n\nOwner: `agent:codex`";
      const pr = pullRequest();
      pr.changedPaths = [
        "docs/decisions/ADR-0029-dashboard-boundary.md",
        "crates/workspace-engine/src/lib.rs",
      ];

      const projection = normalizeRepositorySnapshot(snapshot({ issues: [decision], pullRequests: [pr] }));
      const lane = projection.deliveries[0];

      expect(lane?.issue.readiness).toBe("unknown");
      expect(lane?.phase).toBe("validating");
      expect(lane?.blockers).toContain(
        "Decision-Ready authorizes only a focused authority or specification pull request.",
      );
      expect(lane?.action.owner).toBe("codex");
    },
  );

  it("keeps Decision-Ready scope unknown when changed paths cannot be observed completely", () => {
    const decision = issue();
    decision.title = "[Decision][M05 P1] Choose the dashboard delivery boundary";
    decision.body = "## Status\n\n**DECISION-READY**\n\nOwner: `agent:codex`";
    const pr = pullRequest();
    pr.changedPaths = null;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [decision], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.phase).toBe("validating");
    expect(lane?.blockers).toContain("Pull-request changed paths could not be fully observed.");
  });

  it.each([
    "docs/vision/product-constitution.md",
    "docs/vision/design-principles.md",
  ])("allows focused vision authority output for a Decision-Ready Issue: %s", (path) => {
    const decision = issue();
    decision.title = "[Decision][M05 P1] Choose the dashboard delivery boundary";
    decision.body = "## Status\n\n**DECISION-READY**\n\nOwner: `agent:codex`";
    const pr = pullRequest();
    pr.changedPaths = [path];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [decision], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.issue.readiness).toBe("active");
    expect(projection.deliveries[0]?.phase).toBe("merge_gate");
  });

  it("keeps a linked Decision Issue out of the merge gate", () => {
    const decision = issue();
    decision.title = "[Decision][M05 P1] Choose the delivery boundary";
    const pr = pullRequest();

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [decision], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.issue.readiness).toBe("unknown");
    expect(lane?.phase).toBe("validating");
  });

  it.each([
    "[Decision][M05 P1] Choose the delivery boundary",
    "[Research][M05 P1] Evaluate the delivery boundary",
  ])("routes an open PR for non-Decision-Ready authority work to the Steward: %s", (title) => {
    const authorityWork = issue();
    authorityWork.title = title;

    const projection = normalizeRepositorySnapshot(snapshot({
      issues: [authorityWork],
      pullRequests: [pullRequest()],
    }));
    const lane = projection.deliveries[0];

    expect(lane?.issue.readiness).toBe("unknown");
    expect(lane?.phase).toBe("validating");
    expect(lane?.blockers).toContain(
      "The linked Decision or Research Issue is not affirmatively Decision-Ready.",
    );
    expect(lane?.action.owner).toBe("human");
    expect(projection.attention.humanActionRequired).toBe(true);
  });

  it("does not treat a near-match Issue title as authority-only work", () => {
    const delivery = issue();
    delivery.title = "[Researching] Improve dashboard evidence";

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [delivery] }));

    expect(projection.deliveries[0]?.phase).toBe("ready");
  });

  it("does not infer readiness from general Issue prose without an explicit Status section", () => {
    const ungoverned = issue();
    ungoverned.body = "Build a production-ready dashboard for the founder.\n\nOwner: `agent:codex`";

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [ungoverned] }));

    expect(projection.deliveries).toEqual([]);
  });

  it.each([
    "Backlog — mark Ready once scope is approved.",
    "Future Ready.",
    "Ready after approval.",
    "Ready pending approval.",
    "Ready only after the Steward approves scope.",
    "Ready only when the dependency closes.",
    "Ready subject to Steward approval.",
    "Ready: once scope is approved.",
    "Ready = when the dependency closes.",
    "Ready - pending Steward approval.",
    "Pending Steward approval — Ready.",
    "Pending review is complete, but scope is pending — Ready.",
  ])("does not treat a future conditional Ready reference as authoritative readiness: %s", (status) => {
    const backlog = issue();
    backlog.body = `## Status\n\n${status}\n\nOwner: \`agent:codex\``;

    const projection = normalizeRepositorySnapshot(snapshot({
      issues: [backlog],
      pullRequests: [pullRequest()],
    }));
    const lane = projection.deliveries[0];

    expect(lane?.issue.readiness).toBe("unknown");
    expect(lane?.phase).toBe("validating");
    expect(lane?.action.owner).toBe("human");
  });

  it("does not let an infinitive-qualified future Ready claim override a current blocker", () => {
    const blocked = issue();
    blocked.body = "## Status\n\nBlocked; ready to proceed once access is granted.\n\nOwner: `agent:codex`";

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [blocked] }));
    const lane = projection.deliveries[0];

    expect(lane?.issue.readiness).toBe("blocked");
    expect(lane?.phase).toBe("blocked");
    expect(lane?.action.owner).toBe("human");
  });

  it.each([
    { prior: "Blocked", readiness: "blocked", phase: "blocked" },
    { prior: "Parked", readiness: "parked", phase: "parked" },
    { prior: "Active", readiness: "active", phase: "implementing" },
  ])("reveals an earlier $prior state when a later Ready claim becomes conditional", ({ prior, readiness, phase }) => {
    const changed = issue();
    changed.body = `## Status\n\n${prior}; Ready; ready to proceed once access is granted.\n\nOwner: \`agent:codex\``;
    changed.comments = [];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [changed] }));

    expect(projection.deliveries[0]?.issue.readiness).toBe(readiness);
    expect(projection.deliveries[0]?.phase).toBe(phase);
  });

  it.each([
    "No pending blockers — Ready.",
    "Pending review is complete — Ready.",
  ])("preserves an affirmatively resolved Ready prefix: %s", (status) => {
    const ready = issue();
    ready.body = `## Status\n\n${status}\n\nOwner: \`agent:codex\``;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [ready] }));
    const lane = projection.deliveries[0];

    expect(lane?.issue.readiness).toBe("ready");
    expect(lane?.phase).toBe("ready");
  });

  it.each([
    "Pending Steward approval — Active.",
    "Pending approval — Implementing.",
  ])("does not treat a pending-qualified active claim as active: %s", (status) => {
    const pending = issue();
    pending.body = `## Status\n\n${status}\n\nOwner: \`agent:codex\``;

    const projection = normalizeRepositorySnapshot(snapshot({
      issues: [pending],
      pullRequests: [pullRequest()],
    }));
    const lane = projection.deliveries[0];

    expect(lane?.issue.readiness).toBe("unknown");
    expect(lane?.phase).toBe("validating");
    expect(lane?.action.owner).toBe("human");
  });

  it.each([
    "No pending blockers — Active.",
    "Pending review is complete — Implementing.",
  ])("preserves an affirmatively resolved active prefix: %s", (status) => {
    const active = issue();
    active.body = `## Status\n\n${status}\n\nOwner: \`agent:codex\``;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [active] }));

    expect(projection.deliveries[0]?.issue.readiness).toBe("active");
  });

  it("does not treat a suffixed Status heading as authoritative readiness", () => {
    const rationale = issue();
    rationale.body = "## Status rationale\n\nBuild a production-ready dashboard.\n\nOwner: `agent:codex`";

    const projection = normalizeRepositorySnapshot(snapshot({
      issues: [rationale],
      pullRequests: [pullRequest()],
    }));

    expect(projection.deliveries[0]?.issue.readiness).toBe("unknown");
    expect(projection.deliveries[0]?.phase).toBe("validating");
    expect(projection.deliveries[0]?.action.owner).toBe("human");
  });

  it("does not let a nested follow-up heading override authoritative readiness", () => {
    const ready = issue();
    ready.body = [
      "## Status",
      "",
      "Ready",
      "",
      "### Blocked follow-ups",
      "",
      "This old follow-up is blocked.",
      "",
      "Owner: `agent:codex`",
    ].join("\n");

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [ready] }));

    expect(projection.deliveries[0]?.issue.readiness).toBe("ready");
    expect(projection.deliveries[0]?.phase).toBe("ready");
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

  it("surfaces a missing PR handoff as delivery-agent reconciliation work", () => {
    const pr = pullRequest();
    pr.comments = [];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.handoff.condition).toBe("missing");
    expect(lane?.phase).toBe("validating");
    expect(lane?.blockers).toContain("Canonical handoff is missing for pull request #200.");
    expect(lane?.action.owner).toBe("codex");
  });

  it("does not require an agent handoff for a human-owned pull request", () => {
    const humanOwned = issue();
    humanOwned.body = "## Status\n\nReady\n\nOwner: `nurockplayer`";
    const pr = pullRequest();
    pr.comments = [];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [humanOwned], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.handoff.condition).toBe("missing");
    expect(lane?.phase).toBe("merge_gate");
    expect(lane?.blockers).not.toContain("Canonical handoff is missing for pull request #200.");
    expect(lane?.action.owner).toBe("none");
  });

  it("assigns a failing human-owned pull request to its human owner", () => {
    const humanOwned = issue();
    humanOwned.body = "## Status\n\nReady\n\nOwner: `nurockplayer`";
    const pr = pullRequest();
    pr.comments = [];
    pr.checks = [{
      name: "test",
      integrationId: null,
      attemptAt: null,
      status: "completed",
      conclusion: "failure",
      url: null,
    }];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [humanOwned], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.checks.status).toBe("failure");
    expect(lane?.action.owner).toBe("human");
    expect(projection.attention.humanActionRequired).toBe(true);
  });

  it.each([
    { label: "unobserved", author: null },
    { label: "bot-authored", author: { login: "codex[bot]", type: "bot" as const } },
    { label: "mismatched user", author: { login: "contributor", type: "user" as const } },
  ])("requires a handoff when a human Issue has $label PR authorship", ({ author }) => {
    const humanOwned = issue();
    humanOwned.body = "## Status\n\nReady\n\nOwner: `nurockplayer`";
    const pr = pullRequest();
    pr.author = author;
    pr.comments = [];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [humanOwned], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.phase).toBe("validating");
    expect(lane?.blockers).toContain("Canonical handoff is missing for pull request #200.");
  });

  it.each([
    "## Status\n\nReady",
    "## Status\n\nReady\n\nOwner: `mystery-owner`",
  ])("routes a required handoff to the Steward when Issue ownership is unrecognized: %s", (body) => {
    const unowned = issue();
    unowned.body = body;
    const pr = pullRequest();
    pr.comments = [];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [unowned], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.phase).toBe("validating");
    expect(projection.deliveries[0]?.blockers).toContain("Canonical handoff is missing for pull request #200.");
    expect(projection.deliveries[0]?.action).toEqual({
      owner: "human",
      reason: "Required delivery work has no recognized owner; Project Steward reconciliation is required.",
    });
    expect(projection.attention).toMatchObject({
      humanActionRequired: true,
      reasons: ["#169: Required delivery work has no recognized owner; Project Steward reconciliation is required."],
    });
  });

  it("routes a Ready pre-PR Issue with a malformed agent owner to the Steward", () => {
    const unowned = issue();
    unowned.body = "## Status\n\nReady\n\nOwner: `agent:`";
    unowned.comments = [];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [unowned] }));

    expect(projection.deliveries[0]?.phase).toBe("ready");
    expect(projection.deliveries[0]?.action).toEqual({
      owner: "human",
      reason: "Required delivery work has no recognized owner; Project Steward reconciliation is required.",
    });
    expect(projection.attention).toMatchObject({
      humanActionRequired: true,
      reasons: ["#169: Required delivery work has no recognized owner; Project Steward reconciliation is required."],
    });
  });

  it.each(["codex", "chatgpt"])("admits a Ready pre-PR Issue with recognized bare owner %s", (owner) => {
    const ready = issue();
    ready.body = `## Status\n\nReady\n\nOwner: \`${owner}\``;
    ready.comments = [];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [ready] }));

    expect(projection.deliveries).toHaveLength(1);
    expect(projection.deliveries[0]?.issue.readiness).toBe("ready");
    expect(projection.deliveries[0]?.phase).toBe("ready");
    expect(projection.attention.humanActionRequired).toBe(false);
  });

  it("still blocks an inconsistent optional handoff on a human-owned pull request", () => {
    const humanOwned = issue();
    humanOwned.body = "## Status\n\nReady\n\nOwner: `nurockplayer`";
    const pr = pullRequest();
    pr.comments.push({
      ...pr.comments[0]!,
      id: "duplicate-human-handoff",
      url: "https://github.com/nurockplayer/tachiko-work/pull/200#issuecomment-duplicate",
    });

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [humanOwned], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.handoff.condition).toBe("inconsistent");
    expect(lane?.phase).not.toBe("merge_gate");
    expect(lane?.blockers).toContain("Canonical handoff conflicts with live PR identity or is duplicated.");
  });

  it("projects a current substantive review finding as review-fix without founder escalation", () => {
    const pr = pullRequest();
    pr.reviewDecision = "changes_requested";
    pr.reviewThreads = [
      { resolved: false, outdated: false, comments: ["[P2] Preserve exact-head identity"], url: "https://github.com/thread" },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.phase).toBe("review_fix");
    expect(lane?.reviews.substantiveUnresolvedCount).toBe(1);
    expect(lane?.action).toMatchObject({ owner: "codex" });
    expect(projection.attention.humanActionRequired).toBe(false);
  });

  it("blocks a substantive comment-only review body on the current head", () => {
    const pr = pullRequest();
    pr.reviews = [...(pr.reviews ?? []), {
      state: "commented",
      author: "codex",
      body: "[P2] Preserve comment-only review evidence",
      headSha,
      url: "https://github.com/review/comment-only",
      submittedAt: observedAt,
    }];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.reviews.substantiveUnresolvedCount).toBe(1);
    expect(lane?.phase).toBe("review_fix");
    expect(lane?.blockers).toContain("1 substantive review finding(s) remain unresolved.");
    expect(lane?.reviews.sourceRefs).toContainEqual(expect.objectContaining({
      label: "Current-head substantive review body",
      url: "https://github.com/review/comment-only",
      observedIdentity: headSha,
    }));
  });

  it.each([
    "This fails to compile on Windows.",
    "Tests don't pass on Windows.",
    "The build does not pass.",
    "CI isn't passing.",
    "Tests are failing on Windows.",
    "The build is broken.",
    "Compilation is failing.",
  ])("blocks an unlabeled comment-only build failure on the current head: %s", (body) => {
    const pr = pullRequest();
    pr.reviews = [...(pr.reviews ?? []), {
      state: "commented",
      author: "codex",
      body,
      headSha,
      url: "https://github.com/review/comment-only-build-failure",
      submittedAt: observedAt,
    }];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.reviews.substantiveUnresolvedCount).toBe(1);
    expect(lane?.phase).toBe("review_fix");
    expect(lane?.blockers).toContain("1 substantive review finding(s) remain unresolved.");
  });

  it.each([
    "There is no null check, so this crashes on empty input.",
    "No null check means this crashes on empty input.",
    "No null check caused this to crash on empty input.",
    "No null check, which means this crashes on empty input.",
    "No null check, causing this to crash on empty input.",
    "This throws for an empty input.",
    "This deletes user data.",
    "This erases data on retry.",
    "User data is deleted.",
    "Data is erased on retry.",
    "Deletion of user data occurs here.",
    "No user data is deleted, and data is erased.",
    "No user data is deleted and data is erased.",
    "No crashes occur and this causes data loss.",
    "No security issues and data is erased.",
    "No security checks and data is erased.",
    "This enters an infinite loop on empty input.",
    "This hangs forever.",
    "This never terminates.",
    "This loops forever.",
    "This fails to terminate.",
    "This never completes.",
    "This does not terminate.",
    "This won't terminate.",
    "This cannot terminate.",
    "This never returns.",
    "This never halts.",
    "This does not halt.",
    "This runs forever.",
    "This is non-terminating.",
    "This livelocks.",
    "This is nonterminating.",
    "This never stops.",
    "This does not stop.",
    "This is endless.",
    "This is stuck forever.",
    "This keeps looping forever.",
    "This loops indefinitely.",
    "This runs indefinitely.",
    "This spins indefinitely.",
    "This is stuck in a loop.",
    "This never ends.",
    "This loops endlessly.",
    "This runs endlessly.",
    "This isn't terminating.",
    "This is not exiting.",
    "This recurses forever.",
    "This recurses indefinitely.",
    "This deadlocks.",
    "This waits forever.",
    "This stalls forever.",
    "This never reaches completion.",
    "This cannot be terminated.",
    "This recurses infinitely.",
    "This loops without end.",
    "This runs without end.",
    "This is an infinite recursion.",
    "This has infinite recursion.",
    "This enters a cycle forever.",
    "This cycles forever.",
    "This has an unbounded loop.",
    "This loop is unbounded.",
    "This never converges.",
    "This is an unbounded cycle.",
    "This is an infinite cycle.",
    "This remains stuck in a cycle.",
    "This is trapped in a cycle.",
    "No authorization check prevents users from bypassing access control.",
    "No mutex prevents this race condition.",
    "No mutex prevents deadlocks.",
    "No mutexes prevent race conditions.",
    "The lock does not prevent a deadlock.",
    "Synchronization does not prevent data races.",
    "A mutex prevents this race condition but does not prevent deadlock.",
    "No CSRF token prevents unauthorized requests.",
    "This permits SQL injection for crafted input.",
    "SQL injection permits data loss and checks passed.",
    "SQL injection is prevented and code injection permits data loss.",
    "SQL injection is impossible and command injection occurs.",
    "No input validation prevents SQL injection.",
    "No SQL injection and command injection occurs.",
  ])("blocks an unlabeled comment-only runtime failure on the current head: %s", (body) => {
    const pr = pullRequest();
    pr.reviews = [...(pr.reviews ?? []), {
      state: "commented",
      author: "codex",
      body,
      headSha,
      url: "https://github.com/review/comment-only-runtime-failure",
      submittedAt: observedAt,
    }];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.reviews.substantiveUnresolvedCount).toBe(1);
    expect(lane?.phase).toBe("review_fix");
  });

  it("does not let a P3 label suppress a concrete comment-only correctness failure", () => {
    const pr = pullRequest();
    pr.reviews = [...(pr.reviews ?? []), {
      state: "commented",
      author: "codex",
      body: "[P3] This crashes on empty input.",
      headSha,
      url: "https://github.com/review/p3-runtime-failure",
      submittedAt: observedAt,
    }];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.reviews.substantiveUnresolvedCount).toBe(1);
    expect(projection.deliveries[0]?.phase).toBe("review_fix");
  });

  it("does not block on a substantive comment-only review body from an old head", () => {
    const pr = pullRequest();
    pr.reviews = [...(pr.reviews ?? []), {
      state: "commented",
      author: "codex",
      body: "[P2] Finding from the preceding head",
      headSha: "c".repeat(40),
      url: "https://github.com/review/old-comment-only",
      submittedAt: "2026-08-29T23:00:00.000Z",
    }];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.reviews.substantiveUnresolvedCount).toBe(0);
    expect(lane?.phase).toBe("merge_gate");
  });

  it.each([
    "Automated review completed without inline findings.",
    "Security and correctness checks passed; no blocking issues.",
    "No P1/P2 findings.",
    "No blocking correctness issues.",
    "No security and correctness issues were found.",
    "P2 findings: none.",
    "P0: 0.",
    "Security: none.",
    "Blocking issues: none.",
    "Data integrity: none.",
    "Data-integrity: 0.",
    "Security issues found: 0.",
    "P2 findings: none (all checks passed).",
    "P2 findings resolved.",
    "P1 review complete.",
    "Not a [P2] issue.",
    "P2 issue: not found.",
    "P0: absent.",
    "Tests don't fail on Windows.",
    "Tests are not failing on Windows.",
    "This does not throw for an empty input.",
    "This does not delete user data.",
    "No user data is deleted.",
    "No deletion of user data.",
    "User data is not erased.",
    "No user data is deleted, and data is not erased.",
    "No user data is deleted and data is not erased.",
    "No crashes occur and this causes no data loss.",
    "No crashes means this works as expected.",
    "No crashes occur, so this works as expected.",
    "No null check, so this does not crash.",
    "P2 findings: none, so tests pass.",
    "No crashes occur, which means this works as expected.",
    "No crashes occur, causing tests to pass.",
    "No security issues and data is not erased.",
    "No security checks and data is not erased.",
    "No infinite loops occur.",
    "This does not hang on empty input.",
    "The parser never hangs.",
    "This never enters an infinite loop.",
    "This does not loop forever.",
    "This never fails to terminate.",
    "This does not fail to terminate.",
    "This does not run forever.",
    "This is not non-terminating.",
    "This does not fail to halt.",
    "No livelock occurs.",
    "This does not cause a livelock.",
    "This does not enter a livelock.",
    "This does not livelock.",
    "This won't fail to terminate.",
    "This cannot fail to terminate.",
    "This is not endless.",
    "This is not stuck forever.",
    "This does not keep looping forever.",
    "This does not loop indefinitely.",
    "This does not run indefinitely.",
    "This does not spin indefinitely.",
    "This is not stuck in a loop.",
    "This does not fail to end.",
    "This is not an infinite loop.",
    "This does not loop endlessly.",
    "This does not run endlessly.",
    "This is not endlessly looping.",
    "This isn't looping indefinitely.",
    "This does not recurse forever.",
    "This does not deadlock.",
    "This does not wait forever.",
    "This does not stall forever.",
    "This is not an infinite recursion.",
    "This does not loop without end.",
    "This does not run without end.",
    "This does not cycle forever.",
    "This does not have an unbounded loop.",
    "This loop is not unbounded.",
    "This does not fail to converge.",
    "This is not a cycle.",
    "Could you rename this local for clarity?",
    "An authorization check prevents users from bypassing access control.",
    "A mutex prevents this race condition.",
    "The lock prevents deadlocks.",
    "Mutexes prevent race conditions.",
    "Synchronization prevents data races.",
    "A CSRF token prevents unauthorized requests.",
    "This does not permit SQL injection.",
    "SQL injection is prevented.",
    "SQL injection was blocked.",
    "SQL injection is impossible.",
    "Command injection isn't possible.",
    "SQL injection checks passed.",
    "SQL injection never happens.",
    "SQL injection never occurs.",
    "SQL injection was not observed.",
    "SQL injection test passed.",
    "Authorization checks prevent users from bypassing access control.",
    "SQL injection was never observed.",
    "SQL injection has not occurred.",
    "SQL injection is disallowed.",
    "SQL injection is disabled.",
    "SQL injection is ruled out.",
    "The system prevents SQL injection.",
    "The authorization guard blocks code injection.",
    "Input validation prevents SQL injection.",
    "Looks good.",
    "LGTM",
    "SQL injection has never occurred.",
    "[P3] Consider a shorter label.",
    "[P3] This does not crash on empty input.",
  ])("does not infer a substantive finding from a clean comment-only review summary: %s", (body) => {
    const pr = pullRequest();
    pr.reviews = [...(pr.reviews ?? []), {
      state: "commented",
      author: "codex",
      body,
      headSha,
      url: "https://github.com/review/summary",
      submittedAt: observedAt,
    }];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.reviews.substantiveUnresolvedCount).toBe(0);
    expect(lane?.phase).toBe("merge_gate");
  });

  it.each([
    "Blocking: this save path can overwrite user data.",
    "Correctness issue: the projection can report a stale head as current.",
    "Security risk: an untrusted comment can control the handoff.",
    "Security, an untrusted comment can control the handoff.",
    "Security checks passed, but P2 correctness issue remains.",
    "No P1 findings, except P2 correctness issue remains.",
    "P2 issue is not resolved.",
    "P1 review has not completed.",
    "P2 issue is not yet resolved.",
    "P1 review hasn't completed.",
    "Not fixed: [P2] this path overwrites data.",
    "No test covers this [P1] regression.",
    "No SQL injection and command injection occurs.",
  ])("blocks an affirmative equivalent comment-only finding: %s", (body) => {
    const pr = pullRequest();
    pr.reviews = [...(pr.reviews ?? []), {
      state: "commented",
      author: "codex",
      body,
      headSha,
      url: "https://github.com/review/equivalent",
      submittedAt: observedAt,
    }];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.reviews.substantiveUnresolvedCount).toBe(1);
    expect(projection.deliveries[0]?.phase).toBe("review_fix");
  });

  it("routes delivery reconciliation to the claimed ChatGPT agent", () => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body.replace("OWNER: agent:codex", "OWNER: agent:chatgpt");
    pr.reviewThreads = [
      { resolved: false, outdated: false, comments: ["[P2] Correctness finding"], url: "https://github.com/thread" },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.owner).toBe("agent:chatgpt");
    expect(lane?.phase).toBe("review_fix");
    expect(lane?.action.owner).toBe("chatgpt");
  });

  it("preserves provider-neutral ownership for another recognized delivery agent", () => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body.replace("OWNER: agent:codex", "OWNER: agent:other-provider");
    pr.reviewThreads = [
      { resolved: false, outdated: false, comments: ["[P2] Correctness finding"], url: "https://github.com/thread" },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.owner).toBe("agent:other-provider");
    expect(lane?.phase).toBe("review_fix");
    expect(lane?.action.owner).toBe("agent");
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
    expect(lane?.action.owner).toBe("codex");
  });

  it("never treats green exact-head checks with hosted review pending as merge-ready", () => {
    const pr = pullRequest();
    pr.reviewDecision = "review_required";
    pr.reviews = [];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));

    const lane = projection.deliveries[0];

    expect(lane?.phase).toBe("rereview");
    expect(lane?.blockers).toContain("GitHub requires an approving review for the current PR head.");
    expect(lane?.action.owner).toBe("codex");
  });

  it("surfaces current changes-requested review state without relying on an unresolved thread", () => {
    const pr = pullRequest();
    pr.reviewDecision = "changes_requested";
    pr.reviews = [{
      state: "changes_requested",
      author: "reviewer",
      body: "",
      headSha,
      url: "https://github.com/review",
      submittedAt: observedAt,
    }];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.phase).toBe("review_fix");
    expect(lane?.blockers).toContain("GitHub reports changes requested for the current PR head.");
    expect(lane?.action.owner).toBe("codex");
  });

  it("surfaces incomplete review observation as a delivery blocker", () => {
    const pr = pullRequest();
    pr.reviewDecision = "unknown";
    pr.reviews = null;
    pr.reviewThreads = null;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.phase).toBe("rereview");
    expect(lane?.blockers).toContain("Reviews were not fully observed for the current PR head.");
    expect(lane?.blockers).toContain("GitHub review decision could not be observed.");
    expect(lane?.action.owner).toBe("codex");
  });

  it("surfaces a pending optional exact-head check as delivery work", () => {
    const pr = pullRequest();
    pr.requiredChecks = [{ name: "test", integrationId: null }];
    pr.checks = [
      { name: "test", integrationId: null, attemptAt: null, status: "completed", conclusion: "success", url: null },
      { name: "optional-smoke", integrationId: null, attemptAt: null, status: "in_progress", conclusion: null, url: null },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.checks.status).toBe("pending");
    expect(lane?.checks.requiredStatus).toBe("satisfied");
    expect(lane?.phase).toBe("validating");
    expect(lane?.blockers).toContain("optional-smoke is in progress.");
    expect(lane?.action.owner).toBe("codex");
  });

  it.each([
    { status: "completed" as const, conclusion: "failure" as const },
    { status: "in_progress" as const, conclusion: null },
  ])("ignores a superseded $status attempt after a successful rerun", (superseded) => {
    const pr = pullRequest();
    pr.requiredChecks = [{ name: "test", integrationId: null }];
    pr.checks = [
      {
        name: "test",
        integrationId: null,
        attemptAt: "2026-08-30T00:01:00Z",
        status: "completed",
        conclusion: "success",
        url: null,
      },
      {
        name: "test",
        integrationId: null,
        attemptAt: "2026-08-30T00:00:00Z",
        status: superseded.status,
        conclusion: superseded.conclusion,
        url: null,
      },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.checks.status).toBe("success");
    expect(lane?.checks.requiredStatus).toBe("satisfied");
    expect(lane?.phase).toBe("merge_gate");
  });

  it("fails closed when a queued rerun has no per-run timestamp to supersede an older failure", () => {
    const pr = pullRequest();
    pr.requiredChecks = [{ name: "test", integrationId: null }];
    pr.checks = [
      {
        name: "test",
        integrationId: null,
        attemptAt: "2026-08-30T00:01:00Z",
        status: "completed",
        conclusion: "failure",
        url: null,
      },
      {
        name: "test",
        integrationId: null,
        attemptAt: null,
        status: "queued",
        conclusion: null,
        url: null,
      },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.checks.status).toBe("unknown");
    expect(lane?.checks.requiredStatus).toBe("unknown");
    expect(lane?.phase).toBe("validating");
    expect(lane?.blockers).toContain("The latest check attempt could not be identified from GitHub's per-run timestamps.");
    expect(lane?.blockers).toContain("Required checks remain unknown because the latest check attempt could not be identified.");
  });

  it("never treats an optional green check as satisfying an unobserved required check", () => {
    const pr = pullRequest();
    pr.requiredChecks = [{ name: "release", integrationId: null }];
    pr.checks = [{
      name: "optional-smoke",
      integrationId: null,
      attemptAt: null,
      status: "completed",
      conclusion: "success",
      url: null,
    }];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.checks.status).toBe("success");
    expect(lane?.checks.requiredStatus).toBe("unsatisfied");
    expect(lane?.phase).toBe("validating");
    expect(lane?.blockers).toContain("Required check release was not observed for the current PR head.");
    expect(lane?.action.owner).toBe("codex");
  });

  it("assigns an unknown required-check set to the delivery agent", () => {
    const pr = pullRequest();
    pr.requiredChecks = null;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.checks.requiredStatus).toBe("unknown");
    expect(lane?.phase).toBe("validating");
    expect(lane?.action.owner).toBe("codex");
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
    expect(lane?.action.owner).toBe("codex");
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
      { resolved: false, outdated: false, comments: ["[P3] Consider a shorter label"], url: "https://github.com/thread" },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.reviews.unresolvedThreadCount).toBe(1);
    expect(lane?.reviews.substantiveUnresolvedCount).toBe(0);
    expect(lane?.phase).toBe("merge_gate");
  });

  it("does not let a P3 label suppress a concrete inline correctness failure", () => {
    const pr = pullRequest();
    pr.reviewThreads = [
      { resolved: false, outdated: false, comments: ["[P3] This crashes on empty input."], url: "https://github.com/thread" },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.reviews.substantiveUnresolvedCount).toBe(1);
    expect(projection.deliveries[0]?.phase).toBe("review_fix");
  });

  it("recognizes a badge-prefixed P3 review as non-substantive", () => {
    const pr = pullRequest();
    pr.reviewThreads = [{
      resolved: false,
      outdated: false,
      comments: ["**<sub><sub>![P3 Badge](https://img.shields.io/badge/P3-yellow?style=flat)</sub></sub>  Consider a shorter label"],
      url: "https://github.com/thread",
    }];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.reviews.unresolvedThreadCount).toBe(1);
    expect(lane?.reviews.substantiveUnresolvedCount).toBe(0);
    expect(lane?.phase).toBe("merge_gate");
  });

  it.each([
    "Done.",
    `Fixed in ${headSha}.`,
    "Thanks, applied this suggestion.",
  ])("does not let an acknowledgment promote a P3 thread: %s", (reply) => {
    const pr = pullRequest();
    pr.reviewThreads = [{
      resolved: false,
      outdated: false,
      comments: ["[P3] Consider a shorter label", reply],
      url: "https://github.com/thread",
    }];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.reviews.unresolvedThreadCount).toBe(1);
    expect(lane?.reviews.substantiveUnresolvedCount).toBe(0);
    expect(lane?.phase).toBe("merge_gate");
  });

  it("lets substantive severity evidence override non-substantive wording", () => {
    const pr = pullRequest();
    pr.reviewThreads = [
      { resolved: false, outdated: false, comments: ["[P1] This trivial-looking bug loses data."], url: "https://github.com/thread" },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.reviews.substantiveUnresolvedCount).toBe(1);
    expect(lane?.phase).toBe("review_fix");
  });

  it("keeps unresolved substantive findings blocking after their code location is outdated", () => {
    const pr = pullRequest();
    pr.reviewThreads = [
      { resolved: false, outdated: true, comments: ["[P2] Correctness finding still unresolved"], url: "https://github.com/thread" },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.reviews.substantiveUnresolvedCount).toBe(1);
    expect(lane?.phase).toBe("review_fix");
  });

  it("treats unlabeled unresolved correctness findings as substantive", () => {
    const pr = pullRequest();
    pr.reviewThreads = [
      { resolved: false, outdated: false, comments: ["This returns the wrong result and loses user data."], url: "https://github.com/thread" },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.reviews.substantiveUnresolvedCount).toBe(1);
    expect(lane?.phase).toBe("review_fix");
  });

  it.each([
    "The parser fails to reject stale approvals.",
    "This breaks exact-head validation.",
    "[P3] rename suggestion; [P2] this loses data.",
    "Could you refactor this to prevent data loss?",
    "Please update docs because this is incorrect.",
    "This fails to compile on Windows.",
    "There is no null check, so this crashes on empty input.",
    "No null check means this crashes on empty input.",
    "No null check caused this to crash on empty input.",
    "No null check, which means this crashes on empty input.",
    "No null check, causing this to crash on empty input.",
    "This throws for an empty input.",
    "This deletes user data.",
    "User data is deleted.",
    "No user data is deleted, and data is erased.",
    "No user data is deleted and data is erased.",
    "No crashes occur and this causes data loss.",
    "No security issues and data is erased.",
    "No security checks and data is erased.",
    "This enters an infinite loop on empty input.",
    "This hangs forever.",
    "This never terminates.",
    "This loops forever.",
    "This fails to terminate.",
    "This never completes.",
    "This does not terminate.",
    "This won't terminate.",
    "This cannot terminate.",
    "This never returns.",
    "This never halts.",
    "This does not halt.",
    "This runs forever.",
    "This is non-terminating.",
    "This livelocks.",
    "This is nonterminating.",
    "This never stops.",
    "This does not stop.",
    "This is endless.",
    "This is stuck forever.",
    "This keeps looping forever.",
    "This loops indefinitely.",
    "This runs indefinitely.",
    "This spins indefinitely.",
    "This is stuck in a loop.",
    "This never ends.",
    "This loops endlessly.",
    "This runs endlessly.",
    "This isn't terminating.",
    "This is not exiting.",
    "This recurses forever.",
    "This recurses indefinitely.",
    "This deadlocks.",
    "This waits forever.",
    "This stalls forever.",
    "This never reaches completion.",
    "This cannot be terminated.",
    "This recurses infinitely.",
    "This loops without end.",
    "This runs without end.",
    "This is an infinite recursion.",
    "This has infinite recursion.",
    "This enters a cycle forever.",
    "This cycles forever.",
    "This has an unbounded loop.",
    "This loop is unbounded.",
    "This never converges.",
    "This is an unbounded cycle.",
    "This is an infinite cycle.",
    "This remains stuck in a cycle.",
    "This is trapped in a cycle.",
    "No authorization check prevents users from bypassing access control.",
    "No mutex prevents this race condition.",
    "No mutex prevents deadlocks.",
    "No mutexes prevent race conditions.",
    "The lock does not prevent a deadlock.",
    "Synchronization does not prevent data races.",
    "A mutex prevents this race condition but does not prevent deadlock.",
    "No CSRF token prevents unauthorized requests.",
    "This permits SQL injection for crafted input.",
    "SQL injection permits data loss and checks passed.",
    "SQL injection is prevented and code injection permits data loss.",
    "SQL injection is impossible and command injection occurs.",
    "No input validation prevents SQL injection.",
    "No test covers this [P1] regression.",
  ])("fails closed on an unlabeled correctness finding: %s", (body) => {
    const pr = pullRequest();
    pr.reviewThreads = [
      { resolved: false, outdated: false, comments: [body], url: "https://github.com/thread" },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.reviews.substantiveUnresolvedCount).toBe(1);
    expect(projection.deliveries[0]?.phase).toBe("review_fix");
  });

  it("does not promote an unlabeled pure-maintainability suggestion to a substantive finding", () => {
    const pr = pullRequest();
    pr.reviewThreads = [
      {
        resolved: false,
        outdated: false,
        comments: ["Could you rename this local for clarity?", "Please update this stale comment."],
        url: "https://github.com/thread",
      },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.reviews.unresolvedThreadCount).toBe(1);
    expect(lane?.reviews.substantiveUnresolvedCount).toBe(0);
    expect(lane?.phase).toBe("merge_gate");
  });

  it.each([
    "No data loss.",
    "No regression observed.",
    "No wrong result.",
    "This does not lose user data.",
    "No P3 findings.",
    "No bugs found.",
    "No errors found.",
    "No failure observed.",
    "No failures found.",
    "No defects detected.",
    "No breakage.",
    "No regressions observed.",
    "No security and correctness issues were found.",
    "No breakages.",
    "No data losses.",
    "This is not wrong.",
    "This isn't unsafe.",
    "No user data is deleted.",
    "No deletion of user data.",
    "User data is not erased.",
    "No user data is deleted, and data is not erased.",
    "No user data is deleted and data is not erased.",
    "No crashes occur and this causes no data loss.",
    "No crashes occur, so this works as expected.",
    "No null check, so this does not crash.",
    "P2 findings: none, so tests pass.",
    "No crashes occur, which means this works as expected.",
    "No crashes occur, causing tests to pass.",
    "No security issues and data is not erased.",
    "No security checks and data is not erased.",
    "No infinite loops occur.",
    "This does not hang on empty input.",
    "The parser never hangs.",
    "This never enters an infinite loop.",
    "This does not loop forever.",
    "This never fails to terminate.",
    "This does not fail to terminate.",
    "This does not run forever.",
    "This is not non-terminating.",
    "This does not fail to halt.",
    "No livelock occurs.",
    "This does not cause a livelock.",
    "This does not enter a livelock.",
    "This does not livelock.",
    "This won't fail to terminate.",
    "This cannot fail to terminate.",
    "This is not endless.",
    "This is not stuck forever.",
    "This does not keep looping forever.",
    "This does not loop indefinitely.",
    "This does not run indefinitely.",
    "This does not spin indefinitely.",
    "This is not stuck in a loop.",
    "This does not fail to end.",
    "This is not an infinite loop.",
    "This does not loop endlessly.",
    "This does not run endlessly.",
    "This is not endlessly looping.",
    "This isn't looping indefinitely.",
    "This does not recurse forever.",
    "This does not deadlock.",
    "This does not wait forever.",
    "This does not stall forever.",
    "This is not an infinite recursion.",
    "This does not loop without end.",
    "This does not run without end.",
    "This does not cycle forever.",
    "This does not have an unbounded loop.",
    "This loop is not unbounded.",
    "This does not fail to converge.",
    "This is not a cycle.",
    "An authorization check prevents users from bypassing access control.",
    "A mutex prevents this race condition.",
    "The lock prevents deadlocks.",
    "Mutexes prevent race conditions.",
    "Synchronization prevents data races.",
    "A CSRF token prevents unauthorized requests.",
    "This does not permit SQL injection.",
    "SQL injection is prevented.",
    "SQL injection was blocked.",
    "SQL injection is impossible.",
    "Command injection isn't possible.",
    "SQL injection checks passed.",
    "SQL injection never happens.",
    "SQL injection never occurs.",
    "SQL injection was not observed.",
    "SQL injection test passed.",
    "Authorization checks prevent users from bypassing access control.",
    "SQL injection was never observed.",
    "SQL injection has not occurred.",
    "SQL injection is disallowed.",
    "SQL injection is disabled.",
    "SQL injection is ruled out.",
    "The system prevents SQL injection.",
    "The authorization guard blocks code injection.",
    "Input validation prevents SQL injection.",
    "Looks good.",
    "LGTM",
    "SQL injection has never occurred.",
  ])("does not promote a negated unlabeled impact statement: %s", (body) => {
    const pr = pullRequest();
    pr.reviewThreads = [
      { resolved: false, outdated: false, comments: [body], url: "https://github.com/thread" },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.reviews.substantiveUnresolvedCount).toBe(0);
    expect(projection.deliveries[0]?.phase).toBe("merge_gate");
  });

  it("lets a later unlabeled substantive reply override an initial P3", () => {
    const pr = pullRequest();
    pr.reviewThreads = [{
      resolved: false,
      outdated: false,
      comments: ["[P3] Initial suggestion", "This later reply identifies a wrong result."],
      url: "https://github.com/thread",
    }];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.reviews.substantiveUnresolvedCount).toBe(1);
    expect(lane?.phase).toBe("review_fix");
  });

  it("keeps an incomplete canonical PR handoff out of the merge gate", () => {
    const pr = pullRequest();
    pr.comments[0]!.body = `<!-- agent-handoff:v1 -->\nOWNER: agent:codex\nSTATE: merge-ready\nHEAD: ${headSha}\nLAST CHECKED MAIN: ${mainSha}`;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.handoff.condition).toBe("inconsistent");
    expect(lane?.phase).toBe("validating");
  });

  it("assigns incomplete live-main handoff reconciliation to the delivery agent", () => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body
      .replace("OWNER: agent:codex", "OWNER: agent:chatgpt")
      .replace(`\nLAST CHECKED MAIN: ${mainSha}`, "");

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.handoff.condition).toBe("unknown");
    expect(lane?.owner).toBe("agent:codex");
    expect(lane?.phase).toBe("validating");
    expect(lane?.blockers).toContain("Canonical handoff could not be fully reconciled with the observed PR and live main.");
    expect(lane?.action.owner).toBe("codex");
  });

  it("preserves affirmative human action from an incompletely reconciled handoff", () => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body
      .replace("HUMAN ACTION: none", "HUMAN ACTION: Steward approval required")
      .replace(`\nLAST CHECKED MAIN: ${mainSha}`, "");

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.handoff.condition).toBe("unknown");
    expect(lane?.phase).toBe("human_required");
    expect(lane?.action.owner).toBe("human");
    expect(projection.attention.humanActionRequired).toBe(true);
  });

  it("rejects formatting-only mandatory handoff values", () => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body.replace(
      "VALIDATION EVIDENCE: exact-head gates passed",
      "VALIDATION EVIDENCE: **",
    );

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.handoff.condition).toBe("inconsistent");
    expect(lane?.phase).toBe("validating");
  });

  it("accepts a nonempty escalation section as the mandatory handoff record", () => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body.replace(
      "HUMAN ACTION: none",
      "## Escalation\n\nNone",
    );

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.handoff.condition).toBe("current");
    expect(lane?.phase).toBe("merge_gate");
    expect(lane?.action.owner).toBe("none");
  });

  it.each([
    "Human action is not required",
    "no",
    "false",
    "not necessary",
    "not applicable",
    "Human action isn't required",
    "Human action is not currently required",
    "not currently needed",
    "unnecessary",
  ])("does not escalate an explicit negative human-action label: %s", (claim) => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body.replace(
      "HUMAN ACTION: none",
      `HUMAN ACTION: ${claim}`,
    );

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.handoff.condition).toBe("current");
    expect(lane?.phase).toBe("merge_gate");
    expect(lane?.action.owner).toBe("none");
  });

  it.each([
    "yes",
    "Founder to choose option A",
  ])("treats a non-negative human-action label as affirmative: %s", (claim) => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body.replace(
      "HUMAN ACTION: none",
      `HUMAN ACTION: ${claim}`,
    );

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.phase).toBe("human_required");
    expect(lane?.action.owner).toBe("human");
  });

  it.each([
    "Escalation is not required",
    "Human escalation is not needed",
    "Steward approval is not required",
  ])("does not escalate a negated escalation-section claim: %s", (claim) => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body.replace(
      "HUMAN ACTION: none",
      `## Escalation\n\n${claim}`,
    );

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.handoff.condition).toBe("current");
    expect(lane?.phase).toBe("merge_gate");
    expect(lane?.action.owner).toBe("none");
  });

  it.each([
    "Steward approval",
    "Founder review",
    "Escalate to the Steward",
  ])("projects affirmative authority requests from escalation records: %s", (claim) => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body.replace(
      "HUMAN ACTION: none",
      `## Escalation\n\n${claim}`,
    );

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.phase).toBe("human_required");
    expect(lane?.action.owner).toBe("human");
  });

  it("does not let a negative human-action label mask a positive escalation record", () => {
    const pr = pullRequest();
    pr.comments[0]!.body += "\n## Escalation\n\nSteward approval";

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.phase).toBe("human_required");
    expect(lane?.action.owner).toBe("human");
  });

  it("preserves affirmative human action from an inconsistent duplicate handoff", () => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body.replace("HUMAN ACTION: none", "HUMAN ACTION: required");
    pr.comments.push({
      ...pr.comments[0]!,
      id: "handoff-pr-duplicate",
      body: pr.comments[0]!.body.replace("HUMAN ACTION: required", "HUMAN ACTION: none"),
      updatedAt: "2026-08-30T00:01:00.000Z",
    });

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.handoff.condition).toBe("inconsistent");
    expect(lane?.phase).toBe("human_required");
    expect(lane?.blockers).toContain("Canonical handoff conflicts with live PR identity or is duplicated.");
    expect(lane?.action.owner).toBe("human");
  });

  it("assigns an operational Not Ready handoff to the delivery agent when the Issue is Ready", () => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body.replace("STATE: merge-ready", "STATE: Not Ready");

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.issue.readiness).toBe("unknown");
    expect(lane?.phase).toBe("validating");
    expect(lane?.blockers).not.toContain("The authoritative Issue status does not affirm that this lane is Ready or active.");
    expect(lane?.blockers).toContain("The current canonical handoff does not affirm merge-ready delivery state.");
    expect(lane?.action.owner).toBe("codex");
  });

  it.each(["implementing", "validating", "review_fix"])(
    "requires merge-ready handoff state before exposing the merge gate: %s",
    (state) => {
      const pr = pullRequest();
      pr.comments[0]!.body = pr.comments[0]!.body.replace("STATE: merge-ready", `STATE: ${state}`);

      const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
      const lane = projection.deliveries[0];

      expect(lane?.phase).toBe("validating");
      expect(lane?.blockers).toContain("The current canonical handoff does not affirm merge-ready delivery state.");
      expect(lane?.action.owner).toBe("codex");
    },
  );

  it("does not treat a negated merge-ready handoff state as terminal", () => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body.replace("STATE: merge-ready", "STATE: not merge-ready");

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.phase).toBe("validating");
    expect(lane?.blockers).toContain("The current canonical handoff does not affirm merge-ready delivery state.");
    expect(lane?.action.owner).toBe("codex");
  });

  it("accepts the canonical underscore merge-ready handoff spelling", () => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body.replace("STATE: merge-ready", "STATE: merge_ready");

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.issue.readiness).toBe("active");
    expect(lane?.phase).toBe("merge_gate");
    expect(lane?.blockers).not.toContain("The current canonical handoff does not affirm merge-ready delivery state.");
  });

  it("assigns authoritative blocked Issue readiness to the Steward", () => {
    const blocked = issue();
    blocked.body = "## Status\n\nBlocked\n\nOwner: `agent:codex`";

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [blocked], pullRequests: [pullRequest()] }));
    const lane = projection.deliveries[0];

    expect(lane?.phase).toBe("blocked");
    expect(lane?.blockers).toContain("The authoritative Issue status reports this lane blocked.");
    expect(lane?.action.owner).toBe("human");
  });

  it("projects a requested action from an escalation section", () => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body.replace(
      "HUMAN ACTION: none",
      "## Escalation\n\nHuman action required",
    );

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.handoff.condition).toBe("current");
    expect(lane?.phase).toBe("human_required");
    expect(lane?.action.owner).toBe("human");
  });

  it("does not count empty handoff sections as mandatory evidence", () => {
    const pr = pullRequest();
    pr.comments[0]!.body = [
      "<!-- agent-handoff:v1 -->",
      "ISSUE: #169",
      "STATE: merge-ready",
      `HEAD: ${headSha}`,
      `LAST CHECKED MAIN: ${mainSha}`,
      "HUMAN ACTION: none",
      "## Scope",
      "## Validation",
      "## Unresolved Review",
      "## Next",
    ].join("\n");

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.handoff.condition).toBe("inconsistent");
    expect(projection.deliveries[0]?.phase).toBe("validating");
  });

  it("keeps draft pull requests out of the merge gate", () => {
    const pr = pullRequest();
    pr.isDraft = true;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.phase).toBe("validating");
    expect(lane?.blockers).toContain("Pull request #200 is still a draft.");
    expect(lane?.action.owner).toBe("codex");
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

  it("uses the canonical handoff Issue when a PR has no closing reference", () => {
    const pr = pullRequest();
    pr.issueNumbers = [];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));

    expect(projection.deliveries).toHaveLength(1);
    expect(projection.deliveries[0]?.id).toBe("issue-169-pr-200");
    expect(projection.deliveries[0]?.issue.number).toBe(169);
  });

  it("does not let an unassociated PR number claim or suppress the same-numbered Issue", () => {
    const unassociated = pullRequest();
    unassociated.issueNumbers = [];
    unassociated.comments = [];
    const sameNumberedIssue = issue(unassociated.number);

    const projection = normalizeRepositorySnapshot(snapshot({
      issues: [sameNumberedIssue],
      pullRequests: [unassociated],
    }));

    expect(projection.deliveries).toHaveLength(2);
    const issueLane = projection.deliveries.find((lane) => lane.id === "issue-200");
    const prLane = projection.deliveries.find((lane) => lane.id === "issue-200-pr-200");
    expect(issueLane?.issue.number).toBe(200);
    expect(issueLane?.pr).toBeNull();
    expect(prLane?.pr?.number).toBe(200);
  });

  it("keeps a stale canonical handoff Issue claim associated with its PR", () => {
    const pr = pullRequest();
    pr.issueNumbers = [];
    pr.comments[0]!.body = pr.comments[0]!.body.replace(mainSha, "d".repeat(40));

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));

    expect(projection.deliveries).toHaveLength(1);
    expect(projection.deliveries[0]?.id).toBe("issue-169-pr-200");
    expect(projection.deliveries[0]?.issue.number).toBe(169);
    expect(projection.deliveries[0]?.handoff.condition).toBe("stale");
    expect(projection.deliveries[0]?.phase).toBe("validating");
  });

  it("preserves a stale handoff owner for delivery reconciliation", () => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body
      .replace("OWNER: agent:codex", "OWNER: agent:chatgpt")
      .replace(`LAST CHECKED MAIN: ${mainSha}`, `LAST CHECKED MAIN: ${"d".repeat(40)}`);

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.handoff.condition).toBe("stale");
    expect(lane?.owner).toBe("agent:chatgpt");
    expect(lane?.phase).toBe("validating");
    expect(lane?.action.owner).toBe("chatgpt");
  });

  it("keeps truncated PR handoff observation unknown", () => {
    const pr = pullRequest();
    pr.commentsComplete = false;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue(), issue(170)], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.handoff.condition).toBe("unknown");
    expect(projection.deliveries[0]?.phase).toBe("validating");
    for (const lane of projection.deliveries) {
      expect(lane.phase).toBe("validating");
      expect(lane.blockers).toContain("Pull-request Issue ownership could not be fully observed.");
    }
  });

  it("does not advertise a no-PR Issue as Ready when handoff observation is truncated", () => {
    const truncated = issue();
    truncated.commentsComplete = false;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [truncated] }));

    expect(projection.deliveries[0]?.handoff.condition).toBe("unknown");
    expect(projection.deliveries[0]?.issue.readiness).toBe("unknown");
    expect(projection.deliveries[0]?.phase).toBe("unknown");
  });

  it("parks non-current product-milestone work outside independent and active lanes", () => {
    const future = issue(206);
    future.milestone = "06 · Team Workspace Beta";

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [future] }));

    expect(projection.deliveries[0]?.phase).toBe("parked");
    expect(projection.deliveries[0]?.blockers).toContain(
      "Issue #206 belongs to non-current product milestone 06 · Team Workspace Beta; the live current horizon is 05 · Designer MVP.",
    );
    expect(projection.currentWork.independent).toEqual([]);
    expect(projection.currentWork.otherHorizon).toEqual(["issue-206"]);
    expect(projection.deliveries[0]?.action.owner).toBe("none");
    expect(projection.attention).toMatchObject({
      humanActionRequired: true,
      reasons: ["No Ready delivery remains; the Project Steward must select or ready successor work."],
    });
  });

  it("routes a non-current product-milestone pull request to the Project Steward", () => {
    const future = issue();
    future.milestone = "06 · Team Workspace Beta";

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [future], pullRequests: [pullRequest()] }));
    const lane = projection.deliveries[0];

    expect(lane?.phase).toBe("parked");
    expect(lane?.action.owner).toBe("human");
    expect(lane?.blockers).toContain(
      "Issue #169 belongs to non-current product milestone 06 · Team Workspace Beta; the live current horizon is 05 · Designer MVP.",
    );
    expect(projection.attention.humanActionRequired).toBe(true);
  });

  it("keeps an approved decision stale until every counted approval covers the exact head", () => {
    const pr = pullRequest();
    pr.reviews = [
      { state: "approved", author: "current-reviewer", body: "", headSha, url: "https://github.com/review/current", submittedAt: observedAt },
      { state: "approved", author: "stale-reviewer", body: "", headSha: "c".repeat(40), url: "https://github.com/review/stale", submittedAt: "2026-08-29T23:00:00.000Z" },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.reviews.status).toBe("stale");
    expect(projection.deliveries[0]?.reviews.reviewedHeadSha).toBeNull();
    expect(projection.deliveries[0]?.phase).toBe("rereview");
  });

  it("uses only each reviewer's latest opinion for exact-head coverage", () => {
    const pr = pullRequest();
    pr.reviews = [
      { state: "approved", author: "reviewer", body: "", headSha, url: "https://github.com/review/current", submittedAt: observedAt },
      { state: "approved", author: "reviewer", body: "", headSha: "c".repeat(40), url: "https://github.com/review/stale", submittedAt: "2026-08-29T23:00:00.000Z" },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.reviews.status).toBe("current");
    expect(projection.deliveries[0]?.reviews.reviewedHeadSha).toBe(headSha);
    expect(projection.deliveries[0]?.phase).toBe("merge_gate");
  });

  it("does not count a review body superseded by the same reviewer's later approval", () => {
    const pr = pullRequest();
    pr.reviews = [
      { state: "commented", author: "reviewer", body: "[P2] Clarify the current behavior", headSha, url: "https://github.com/review/finding", submittedAt: "2026-08-29T23:00:00.000Z" },
      { state: "approved", author: "reviewer", body: "Looks good.", headSha, url: "https://github.com/review/approval", submittedAt: observedAt },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.reviews.substantiveUnresolvedCount).toBe(0);
    expect(projection.deliveries[0]?.reviews.status).toBe("current");
    expect(projection.deliveries[0]?.phase).toBe("merge_gate");
  });

  it("retains a review finding across a later unrelated comment by the same reviewer", () => {
    const pr = pullRequest();
    pr.reviews = [
      { state: "commented", author: "reviewer", body: "[P2] Clarify the current behavior", headSha, url: "https://github.com/review/finding", submittedAt: "2026-08-29T23:00:00.000Z" },
      { state: "commented", author: "reviewer", body: "Additional context for the review.", headSha, url: "https://github.com/review/context", submittedAt: observedAt },
      { state: "approved", author: "other-reviewer", body: "", headSha, url: "https://github.com/review/approval", submittedAt: observedAt },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.reviews.substantiveUnresolvedCount).toBe(1);
    expect(projection.deliveries[0]?.phase).toBe("review_fix");
  });

  it("does not clear an earlier review finding with an unrelated negative comment", () => {
    const pr = pullRequest();
    pr.reviews = [
      { state: "commented", author: "reviewer", body: "[P2] Fix data loss", headSha, url: "https://github.com/review/finding", submittedAt: "2026-08-29T23:00:00.000Z" },
      { state: "commented", author: "reviewer", body: "No formatting issues.", headSha, url: "https://github.com/review/formatting", submittedAt: observedAt },
      { state: "approved", author: "other-reviewer", body: "", headSha, url: "https://github.com/review/approval", submittedAt: observedAt },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.reviews.substantiveUnresolvedCount).toBe(1);
    expect(projection.deliveries[0]?.phase).toBe("review_fix");
  });

  it("clears an earlier review finding with an explicit later resolution by the same reviewer", () => {
    const pr = pullRequest();
    pr.reviews = [
      { state: "commented", author: "reviewer", body: "[P2] Clarify the current behavior", headSha, url: "https://github.com/review/finding", submittedAt: "2026-08-29T23:00:00.000Z" },
      { state: "commented", author: "reviewer", body: "P2 findings resolved.", headSha, url: "https://github.com/review/resolved", submittedAt: observedAt },
      { state: "approved", author: "other-reviewer", body: "", headSha, url: "https://github.com/review/approval", submittedAt: observedAt },
    ];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.reviews.substantiveUnresolvedCount).toBe(0);
    expect(projection.deliveries[0]?.phase).toBe("merge_gate");
  });

  it("keeps an approved decision out of merge gate without observed exact-head approval", () => {
    const pr = pullRequest();
    pr.reviews = [];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.reviews.status).toBe("unknown");
    expect(projection.deliveries[0]?.phase).toBe("rereview");
  });

  it("requires GitHub's exact merge gate to be clean before projecting merge gate", () => {
    const pr = pullRequest();
    pr.mergeStateStatus = "blocked";

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.phase).toBe("validating");
    expect(projection.deliveries[0]?.blockers).toContain("GitHub reports that pull request #200 is blocked from merging.");
    expect(projection.deliveries[0]?.action.owner).toBe("codex");
  });

  it("assigns merge-conflict repair to the delivery agent", () => {
    const pr = pullRequest();
    pr.mergeable = "conflicting";
    pr.mergeStateStatus = "dirty";

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.phase).toBe("validating");
    expect(lane?.blockers).toContain("GitHub reports that pull request #200 has merge conflicts.");
    expect(lane?.action.owner).toBe("codex");
  });

  it("requires a handoff update after an Issue scope edit", () => {
    const edited = issue();
    edited.lastEditedAt = "2026-08-30T00:01:00.000Z";
    const pr = pullRequest();

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [edited], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.phase).toBe("validating");
    expect(projection.deliveries[0]?.blockers).toContain(
      "Issue #169 scope was edited after the canonical handoff; explicit reconciliation is required.",
    );
  });

  it("requires a no-PR Issue handoff update after an Issue scope edit", () => {
    const edited = issue();
    edited.lastEditedAt = "2026-08-30T00:01:00.000Z";

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [edited] }));

    expect(projection.deliveries[0]?.phase).toBe("validating");
    expect(projection.deliveries[0]?.blockers).toContain(
      "Issue #169 scope was edited after the canonical handoff; explicit reconciliation is required.",
    );
  });

  it("blocks a canonical handoff Issue that conflicts with the PR closing reference", () => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body.replace("ISSUE: #169", "ISSUE: #170");

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue(), issue(170)], pullRequests: [pr] }));

    expect(projection.deliveries).toHaveLength(1);
    expect(projection.deliveries[0]?.phase).toBe("blocked");
    expect(projection.deliveries[0]?.blockers).toContain(
      "Canonical handoff claims Issue #170, but pull request #200 closes Issue #169.",
    );
  });

  it("blocks an unlabelled canonical handoff Issue that conflicts with the PR closing reference", () => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body.replace("ISSUE: #169", "Issue #170");

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));

    expect(projection.deliveries[0]?.phase).toBe("blocked");
    expect(projection.deliveries[0]?.blockers).toContain(
      "Canonical handoff claims Issue #170, but pull request #200 closes Issue #169.",
    );
  });

  it("blocks a stale canonical handoff Issue that conflicts with the PR closing reference", () => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body
      .replace("ISSUE: #169", "ISSUE: #170")
      .replace(mainSha, "d".repeat(40));

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue(), issue(170)], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(projection.deliveries).toHaveLength(1);
    expect(lane?.handoff.condition).toBe("stale");
    expect(lane?.phase).toBe("blocked");
    expect(lane?.blockers).toContain(
      "Canonical handoff claims Issue #170, but pull request #200 closes Issue #169.",
    );
  });

  it("keeps a green PR out of merge gate when roadmap horizon observation is unavailable", () => {
    const projection = normalizeRepositorySnapshot(snapshot({
      productHorizon: null,
      fetchHealth: "partial",
      failures: ["Product Roadmap observation failed."],
      issues: [issue()],
      pullRequests: [pullRequest()],
    }));

    expect(projection.currentWork.horizonStatus).toBe("unknown");
    expect(projection.deliveries[0]?.phase).toBe("validating");
    expect(projection.deliveries[0]?.blockers).toContain("Product Roadmap horizon could not be observed.");
    expect(projection.deliveries[0]?.action).toEqual({
      owner: "human",
      reason: "Product Roadmap authority requires Project Steward reconciliation.",
    });
    expect(projection.attention.humanActionRequired).toBe(true);
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
      expect(lane.action.owner).toBe("human");
    }
    expect(projection.attention.humanActionRequired).toBe(true);
  });

  it("includes mismatched handoff claims in cross-PR ownership conflicts", () => {
    const first = pullRequest();
    first.comments[0]!.body = first.comments[0]!.body.replace("ISSUE: #169", "ISSUE: #170");
    const second = pullRequest();
    second.number = 201;
    second.url = "https://github.com/nurockplayer/tachiko-work/pull/201";
    second.body = "Closes #170";
    second.issueNumbers = [170];
    second.headSha = "c".repeat(40);
    second.checksObservedHeadSha = second.headSha;
    second.reviews![0]!.headSha = second.headSha;
    second.comments[0]!.body = second.comments[0]!.body
      .replace("ISSUE: #169", "ISSUE: #170")
      .replaceAll(headSha, second.headSha);

    const projection = normalizeRepositorySnapshot(snapshot({
      issues: [issue(), issue(170)],
      pullRequests: [first, second],
    }));

    expect(projection.deliveries.map((lane) => lane.phase)).toEqual(["blocked", "blocked"]);
    for (const lane of projection.deliveries) {
      expect(lane.blockers).toContain("Multiple open pull requests claim Issue #170: #200, #201.");
    }
  });

  it("reserves every observed Issue claim from inconsistent duplicate handoffs", () => {
    const first = pullRequest();
    first.comments.push({
      ...first.comments[0]!,
      id: "handoff-pr-duplicate",
      body: first.comments[0]!.body
        .replace("ISSUE: #169", "ISSUE: #170")
        .replace("OWNER: agent:codex", "OWNER: agent:chatgpt"),
      url: "https://github.com/nurockplayer/tachiko-work/pull/200#issuecomment-3",
      updatedAt: "2026-08-30T00:01:00.000Z",
    });
    const second = pullRequest();
    second.number = 201;
    second.url = "https://github.com/nurockplayer/tachiko-work/pull/201";
    second.body = "Closes #170";
    second.issueNumbers = [170];
    second.headSha = "c".repeat(40);
    second.checksObservedHeadSha = second.headSha;
    second.reviews![0]!.headSha = second.headSha;
    second.comments[0]!.body = second.comments[0]!.body
      .replace("ISSUE: #169", "ISSUE: #170")
      .replaceAll(headSha, second.headSha);

    const projection = normalizeRepositorySnapshot(snapshot({
      issues: [issue(), issue(170)],
      pullRequests: [first, second],
    }));

    expect(projection.deliveries.map((lane) => lane.phase)).toEqual(["blocked", "blocked"]);
    const firstLane = projection.deliveries.find((lane) => lane.pr?.number === 200);
    expect(firstLane).toMatchObject({ issue: { number: 169 }, owner: "agent:codex" });
    expect(firstLane?.handoff).toMatchObject({
      condition: "inconsistent",
      claimedIssueNumber: null,
      observedIssueNumbers: [169, 170],
    });
    expect(firstLane?.blockers).not.toContain(
      "Canonical handoff claims Issue #170, but pull request #200 closes Issue #169.",
    );
    for (const lane of projection.deliveries) {
      expect(lane.blockers).toContain("Multiple open pull requests claim Issue #170: #200, #201.");
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
    expect(lane?.action.owner).toBe("codex");
  });

  it("keeps a PR targeting a non-default branch out of the merge gate", () => {
    const pr = pullRequest();
    pr.baseRefName = "release";

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const lane = projection.deliveries[0];

    expect(lane?.phase).toBe("validating");
    expect(lane?.blockers).toContain("Pull request #200 targets release instead of the live default branch main.");
    expect(lane?.action.owner).toBe("codex");
  });

  it("fails the merge gate closed when the default branch identity is unknown", () => {
    const pr = pullRequest();

    const projection = normalizeRepositorySnapshot(snapshot({
      defaultBranchName: null,
      issues: [issue()],
      pullRequests: [pr],
    }));
    const lane = projection.deliveries[0];

    expect(lane?.phase).toBe("validating");
    expect(lane?.blockers).toContain("Default branch identity could not be observed.");
    expect(lane?.action.owner).toBe("codex");
  });

  it("keeps omitted closing-Issue ownership out of standalone Ready and merge gates", () => {
    const pr = pullRequest();
    pr.issueNumbersComplete = false;

    const projection = normalizeRepositorySnapshot(snapshot({
      fetchHealth: "partial",
      failures: ["PR #200 closing-Issue observation was truncated."],
      issues: [issue(169), issue(170)],
      pullRequests: [pr],
    }));

    expect(projection.deliveries.map((lane) => lane.issue.number).toSorted()).toEqual([169, 170]);
    for (const lane of projection.deliveries) {
      expect(lane.phase).toBe("validating");
      expect(lane.blockers).toContain("Pull-request Issue ownership could not be fully observed.");
      expect(lane.action.owner).toBe("codex");
    }
  });

  it("fails closed when a reference-less PR handoff may be outside the observed comments", () => {
    const pr = pullRequest();
    pr.issueNumbers = [];
    pr.commentsComplete = false;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [issue()], pullRequests: [pr] }));
    const issueLane = projection.deliveries.find((lane) => lane.issue.number === 169);
    const prLane = projection.deliveries.find((lane) => lane.pr?.number === 200);

    expect(issueLane?.phase).toBe("validating");
    expect(issueLane?.blockers).toContain("Pull-request Issue ownership could not be fully observed.");
    expect(issueLane?.action.owner).toBe("codex");
    expect(prLane?.phase).not.toBe("merge_gate");
  });

  it("blocks PR lanes that claim the same secondary Issue", () => {
    const first = pullRequest();
    first.issueNumbers = [169, 170];
    const second = pullRequest();
    second.number = 201;
    second.url = "https://github.com/nurockplayer/tachiko-work/pull/201";
    second.issueNumbers = [171, 170];
    second.headSha = "c".repeat(40);
    second.checksObservedHeadSha = second.headSha;
    second.comments[0]!.body = second.comments[0]!.body.replaceAll(headSha, second.headSha);

    const projection = normalizeRepositorySnapshot(snapshot({
      issues: [issue(169), issue(170), issue(171)],
      pullRequests: [first, second],
    }));

    expect(projection.deliveries).toHaveLength(2);
    expect(projection.deliveries.map((lane) => lane.issue.number).toSorted()).toEqual([169, 171]);
    for (const lane of projection.deliveries) {
      expect(lane.phase).toBe("blocked");
      expect(lane.blockers).toContain("Multiple open pull requests claim Issue #170: #200, #201.");
    }
  });

  it("blocks one PR that claims multiple Issues and suppresses duplicate standalone lanes", () => {
    const pr = pullRequest();
    pr.issueNumbers = [169, 170];

    const projection = normalizeRepositorySnapshot(snapshot({
      issues: [issue(169), issue(170)],
      pullRequests: [pr],
    }));
    const lane = projection.deliveries[0];

    expect(projection.deliveries).toHaveLength(1);
    expect(lane?.phase).toBe("blocked");
    expect(lane?.blockers).toContain("Pull request #200 claims multiple Issues (#169, #170), violating the one-Issue delivery boundary.");
    expect(lane?.action.owner).toBe("codex");
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

  it("preserves a known human-action request when an unrelated observation is partial", () => {
    const pr = pullRequest();
    pr.comments[0]!.body = pr.comments[0]!.body
      .replace("STATE: merge-ready", "STATE: human_required")
      .replace("HUMAN ACTION: none", "HUMAN ACTION: required");

    const projection = normalizeRepositorySnapshot(snapshot({
      fetchHealth: "partial",
      failures: ["Recent completion observation failed."],
      issues: [issue()],
      pullRequests: [pr],
    }));

    expect(projection.attention.humanActionRequired).toBe(true);
    expect(projection.attention.reasons).toContain("#169: The canonical coordination state requests human or Steward action.");
  });

  it("keeps independent attention confidence when only recent completion history is unavailable", () => {
    const projection = normalizeRepositorySnapshot(snapshot({
      fetchHealth: "partial",
      failures: ["Recent completion observation failed."],
      issues: [issue()],
      pullRequests: [pullRequest()],
      recentCompletions: null,
    }));

    expect(projection.repo.fetchHealth).toBe("partial");
    expect(projection.recentCompletions).toEqual([]);
    expect(projection.attention).toMatchObject({ humanActionRequired: false, reasons: [] });
  });

  it.each(["Owner: `tachikoma`", ""])(
    "retains an explicit human-required Issue without an agent owner: %s",
    (owner) => {
      const escalated = issue();
      escalated.body = `## Status\n\nHuman required\n\n${owner}`;
      escalated.comments = [];

      const projection = normalizeRepositorySnapshot(snapshot({ issues: [escalated] }));

      expect(projection.deliveries).toHaveLength(1);
      expect(projection.deliveries[0]?.phase).toBe("human_required");
      expect(projection.deliveries[0]?.action.owner).toBe("human");
      expect(projection.attention.humanActionRequired).toBe(true);
    },
  );

  it("continues omitting an ordinary human-owned Ready Issue without a pull request", () => {
    const humanOwned = issue();
    humanOwned.body = "## Status\n\nReady\n\nOwner: `tachikoma`";
    humanOwned.comments = [];

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [humanOwned] }));

    expect(projection.deliveries).toEqual([]);
  });

  it("preserves a no-PR human escalation when dependency observation is incomplete", () => {
    const truncated = issue();
    truncated.blockedBy = null;
    truncated.comments[0]!.body = [
      "<!-- agent-handoff:v1 -->",
      "OWNER: agent:codex",
      "STATE: human_required",
      `LAST CHECKED MAIN: ${mainSha}`,
      "HUMAN ACTION: required",
    ].join("\n");

    const projection = normalizeRepositorySnapshot(snapshot({
      fetchHealth: "partial",
      failures: ["Issue #169 dependency observation was truncated."],
      issues: [truncated],
    }));

    expect(projection.deliveries).toHaveLength(1);
    expect(projection.deliveries[0]?.issue.readiness).toBe("unknown");
    expect(projection.attention.humanActionRequired).toBe(true);
  });

  it("keeps aggregate dependency health partial when a truncated no-PR Issue is omitted", () => {
    const truncated = issue();
    truncated.blockedBy = null;

    const projection = normalizeRepositorySnapshot(snapshot({ issues: [truncated] }));

    expect(projection.deliveries).toEqual([]);
    expect(projection.currentWork.dependencyHealth).toBe("partial");
  });
});
