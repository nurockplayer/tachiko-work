import { describe, expect, it } from "vitest";

import { healthyObservation, partialObservation } from "../src/server/fixtures.js";
import { normalizeRepository } from "../src/server/normalize.js";

const HEAD = "2222222222222222222222222222222222222222";

function addGreenOperationalEvidence(observation: ReturnType<typeof healthyObservation>): void {
  const pull = observation.pullRequests[0];
  if (pull === undefined) throw new Error("fixture missing pull request");
  const comment = (id: string, body: string) => ({
    body,
    id,
    kind: "issue-comment" as const,
    authorLogin: "nurockplayer",
    authorAssociation: "OWNER" as const,
    url: `https://github.example/comments/${id}`,
    createdAt: "2026-09-01T00:00:00.000Z",
    updatedAt: null,
    edited: false,
    topLevel: true,
    trustedProducer: true,
  });
  pull.comments.push(
    comment(
      "release-evidence",
      [
        "<!-- operational-evidence:v1",
        "KIND: validation",
        "PR: 225",
        `HEAD: ${HEAD}`,
        "RUN: release-225",
        "NAME: release-check",
        "RESULT: pass",
        "-->",
      ].join("\n"),
    ),
    comment(
      "review-evidence",
      [
        "<!-- operational-evidence:v1",
        "KIND: review",
        "PR: 225",
        `HEAD: ${HEAD}`,
        "RUN: review-225",
        "NAME: project-review",
        "RESULT: clean",
        "-->",
      ].join("\n"),
    ),
  );
}

describe("normalizeRepository", () => {
  it("keeps independent lanes and exact evidence classes separate", () => {
    const projection = normalizeRepository(healthyObservation());

    expect(projection.fetchHealth).toBe("healthy");
    expect(projection.deliveries.map((lane) => lane.issue?.number ?? null)).toEqual([
      169, 223,
    ]);
    expect(projection.deliveries[0]?.pullRequest?.headSha).toBe(
      "2222222222222222222222222222222222222222",
    );
    expect(projection.deliveries[0]?.checks.state).toBe("satisfied");
    expect(projection.deliveries[0]?.review.state).toBe("unknown");
    expect(projection.deliveries[0]?.evidence.automatedBrowser.state).toBe("satisfied");
    expect(projection.deliveries[0]?.evidence.perceptualReview.state).toBe("unknown");
    expect(projection.deliveries[0]?.mergeGate.state).not.toBe("satisfied");
    expect(projection.deliveries[0]?.phase).toBe("rereview");
  });

  it("makes a moving-head handoff stale instead of trusting narrative claims", () => {
    const observation = healthyObservation();
    const pull = observation.pullRequests[0];
    if (pull === undefined) throw new Error("fixture missing pull request");
    pull.comments[0] = {
      ...pull.comments[0]!,
      body: pull.comments[0]!.body.replace(
        "2222222222222222222222222222222222222222",
        "9999999999999999999999999999999999999999",
      ),
    };

    const lane = normalizeRepository(observation).deliveries[0];
    expect(lane?.handoff.state).toBe("unknown");
    expect(lane?.handoff.reason).toBe("head-mismatch");
    expect(lane?.mergeGate.state).toBe("unknown");
  });

  it("degrades unavailable sources to explicit partial and Unknown", () => {
    const projection = normalizeRepository(partialObservation());

    expect(projection.fetchHealth).toBe("partial");
    expect(projection.executive.mainSha.state).toBe("unknown");
    expect(projection.executive.productHorizon.state).toBe("unknown");
    expect(projection.attention.some((item) => item.state === "unknown")).toBe(true);
    expect(projection.humanAction.state).toBe("unknown");
  });

  it("never serializes the server credential", () => {
    const observation = healthyObservation();
    observation.serverCredential = "github_pat_never_reaches_the_browser";

    expect(JSON.stringify(normalizeRepository(observation))).not.toContain(
      observation.serverCredential,
    );
  });

  it("keeps structured human ownership visible as an attention condition", () => {
    const observation = healthyObservation();
    const issue = observation.issues[1];
    if (issue === undefined) throw new Error("fixture missing issue");
    issue.labels = ["agent:human", "state:ready"];

    const projection = normalizeRepository(observation);
    expect(projection.humanAction.state).toBe("blocked");
    expect(
      projection.attention.some(
        (item) => item.issueNumber === issue.number && item.reason === "human-action-required",
      ),
    ).toBe(true);
  });

  it("fails strict coordination closed when comment pagination is incomplete", () => {
    const observation = healthyObservation();
    const pull = observation.pullRequests[0];
    if (pull === undefined) throw new Error("fixture missing pull request");
    pull.commentsAvailability = "incomplete";

    const lane = normalizeRepository(observation).deliveries[0];
    expect(lane?.handoff.state).toBe("unknown");
    expect(lane?.stewardWatch.state).toBe("unknown");
    expect(lane?.mergeGate.state).toBe("unknown");
  });

  it("distinguishes missing readiness from an explicit negative label", () => {
    const observation = healthyObservation();
    const issue = observation.issues[1];
    if (issue === undefined) throw new Error("fixture missing issue");
    issue.labels = ["agent:codex"];
    expect(normalizeRepository(observation).deliveries[1]?.readiness.state).toBe("unknown");

    issue.labels = ["agent:codex", "state:parked"];
    expect(normalizeRepository(observation).deliveries[1]?.readiness.state).toBe("blocked");
  });

  it("projects changed authority paths as reconciliation-needed Unknown", () => {
    const observation = healthyObservation();
    const pull = observation.pullRequests[0];
    if (pull === undefined) throw new Error("fixture missing pull request");
    pull.authorityChanges = [
      { path: "docs/governance/project-governance.md", url: "https://github.example/compare" },
    ];

    const authority = normalizeRepository(observation).deliveries[0]?.authority;
    expect(authority?.state).toBe("unknown");
    expect(authority?.reason).toBe("authority-drift-suspected");
    expect(authority?.sources[0]?.url).toBe("https://github.example/compare");
    expect(normalizeRepository(observation).deliveries[0]?.phase).toBe("rereview");
  });

  it("never lets changed or unavailable authority pass an otherwise-green merge gate", () => {
    const changed = healthyObservation();
    addGreenOperationalEvidence(changed);
    expect(normalizeRepository(changed).deliveries[0]?.mergeGate.state).toBe("satisfied");
    const changedPull = changed.pullRequests[0];
    if (changedPull === undefined) throw new Error("fixture missing pull request");
    changedPull.authorityChanges = [
      { path: "docs/vision/product-constitution.md", url: "https://github.example/compare" },
    ];
    expect(normalizeRepository(changed).deliveries[0]).toMatchObject({
      authority: { state: "unknown" },
      mergeGate: { state: "unknown" },
      phase: "rereview",
    });

    const unavailable = healthyObservation();
    addGreenOperationalEvidence(unavailable);
    const unavailablePull = unavailable.pullRequests[0];
    if (unavailablePull === undefined) throw new Error("fixture missing pull request");
    unavailablePull.authorityAvailability = "unavailable";
    expect(normalizeRepository(unavailable).deliveries[0]).toMatchObject({
      authority: { state: "unknown" },
      mergeGate: { state: "unknown" },
      phase: "unknown",
    });
  });

  it("keeps truncated dependencies Unknown in the critical-path projection", () => {
    const observation = healthyObservation();
    const issue = observation.issues[1];
    if (issue === undefined) throw new Error("fixture missing issue");
    issue.dependencyAvailability = "incomplete";

    const node = normalizeRepository(observation).criticalPath.nodes.find(
      (item) => item.issueNumber === issue.number,
    );
    expect(node?.state).toBe("unknown");
  });

  it("preserves explicit Blocked and dependency Waiting as distinct conditions", () => {
    const blocked = healthyObservation();
    const blockedIssue = blocked.issues[1];
    if (blockedIssue === undefined) throw new Error("fixture missing issue");
    blockedIssue.labels = ["agent:codex", "state:blocked"];
    expect(
      normalizeRepository(blocked).criticalPath.nodes.find(
        (item) => item.issueNumber === blockedIssue.number,
      )?.state,
    ).toBe("blocked");

    const waiting = healthyObservation();
    const waitingIssue = waiting.issues[0];
    if (waitingIssue === undefined) throw new Error("fixture missing issue");
    waitingIssue.blockedBy = [
      { number: 200, state: "OPEN", url: "https://github.example/issues/200" },
    ];
    const lane = normalizeRepository(waiting).deliveries[0];
    expect(lane?.readiness.state).toBe("waiting");
    expect(lane?.mergeGate.state).toBe("waiting");
    expect(lane?.phase).toBe("waiting");
  });

  it("routes failed exact-head checks to blocked phase and attention", () => {
    const observation = healthyObservation();
    const pull = observation.pullRequests[0];
    if (pull === undefined) throw new Error("fixture missing pull request");
    pull.checks[0] = { ...pull.checks[0]!, status: "failure" };

    const projection = normalizeRepository(observation);
    expect(projection.deliveries[0]?.phase).toBe("blocked");
    expect(
      projection.attention.some(
        (item) => item.issueNumber === 169 && item.reason === "native-check-failed",
      ),
    ).toBe(true);
  });

  it("keeps partial label evidence Unknown throughout a pull-request lane", () => {
    const observation = healthyObservation();
    const issue = observation.issues[0];
    if (issue === undefined) throw new Error("fixture missing issue");
    issue.labelsAvailability = "incomplete";

    const lane = normalizeRepository(observation).deliveries[0];
    expect(lane?.owner).toBe("unknown");
    expect(lane?.readiness.state).toBe("unknown");
    expect(lane?.humanAction.state).toBe("unknown");
    expect(lane?.mergeGate.state).not.toBe("satisfied");
    expect(lane?.phase).toBe("unknown");
  });

  it.each([
    ["missing", ["state:ready"]],
    ["conflicting", ["agent:codex", "agent:human", "state:ready"]],
  ])("keeps %s ownership Unknown for both pull-request and no-PR lanes", (_name, labels) => {
    const observation = healthyObservation();
    const pullIssue = observation.issues[0];
    const issueOnly = observation.issues[1];
    if (pullIssue === undefined || issueOnly === undefined) {
      throw new Error("fixture missing issue");
    }
    pullIssue.labels = [...labels];
    issueOnly.labels = [...labels];

    const projection = normalizeRepository(observation);
    for (const issueNumber of [169, 223]) {
      const lane = projection.deliveries.find((item) => item.issue?.number === issueNumber);
      expect(lane?.owner).toBe("unknown");
      expect(lane?.humanAction.state).toBe("unknown");
      expect(
        projection.attention.some(
          (item) => item.issueNumber === issueNumber && item.label === "Issue ownership Unknown",
        ),
      ).toBe(true);
    }
    expect(projection.deliveries[0]?.mergeGate.state).not.toBe("satisfied");
    expect(projection.humanAction.state).toBe("unknown");
  });

  it.each(["state:blocked", "state:parked"])(
    "keeps Ready plus %s as conflicting Unknown evidence",
    (negativeLabel) => {
      const observation = healthyObservation();
      const issue = observation.issues[0];
      if (issue === undefined) throw new Error("fixture missing issue");
      issue.labels = ["agent:codex", "state:ready", negativeLabel];

      const lane = normalizeRepository(observation).deliveries[0];
      expect(lane?.readiness).toMatchObject({
        state: "unknown",
        reason: "source-identity-conflict",
      });
      expect(lane?.mergeGate.state).not.toBe("satisfied");
      expect(lane?.phase).toBe("unknown");
    },
  );

  it("keeps open pull requests visible when native Issue linkage is missing", () => {
    const observation = healthyObservation();
    const source = observation.pullRequests[0];
    if (source === undefined) throw new Error("fixture missing pull request");
    observation.pullRequests.push({
      ...source,
      number: 226,
      title: "Unlinked docs change",
      url: "https://github.example/pulls/226",
      closingIssueNumbers: [],
      comments: [],
      checks: [],
      reviews: [],
      threads: [],
    });

    const lane = normalizeRepository(observation).deliveries.find(
      (item) => item.pullRequest?.number === 226,
    );
    expect(lane?.issue).toBeNull();
    expect(lane?.readiness.state).toBe("unknown");
  });

  it("exposes exact check provenance on the delivery lane", () => {
    const lane = normalizeRepository(healthyObservation()).deliveries[0];
    expect(lane?.sources.some((source) => source.url.endsWith("/checks/browser"))).toBe(true);
  });
});
