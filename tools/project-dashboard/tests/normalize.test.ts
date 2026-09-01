import { describe, expect, it } from "vitest";

import { healthyObservation, partialObservation } from "../src/server/fixtures.js";
import { normalizeRepository } from "../src/server/normalize.js";

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
  });

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
