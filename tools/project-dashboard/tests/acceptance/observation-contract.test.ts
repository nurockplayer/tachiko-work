import { describe, expect, it } from "vitest";

import {
  projectObservation,
  type DashboardProjection,
  type ObservationSnapshot,
  type Observed,
  type PullObservation,
  type StewardWatchObservation,
} from "../../src/server/observation.js";

const MAIN = "1".repeat(40);
const HEAD = "2".repeat(40);

function complete<T>(value: T): Observed<T> {
  return { availability: "complete", value };
}

function partial<T>(value: T | null): Observed<T> {
  return { availability: "partial", value };
}

function unavailable<T>(): Observed<T> {
  return { availability: "unavailable", value: null };
}

function watch(
  overrides: Partial<StewardWatchObservation> = {},
): StewardWatchObservation {
  return {
    prNumber: 400,
    availability: "complete",
    verdict: "GREEN",
    humanAction: "none",
    head: HEAD,
    main: MAIN,
    ...overrides,
  };
}

function pull(overrides: Partial<PullObservation> = {}): PullObservation {
  return {
    number: 400,
    title: "Fresh Dashboard v0.1",
    state: "OPEN",
    draft: true,
    head: complete(HEAD),
    base: complete(MAIN),
    linkedIssues: complete([229]),
    ...overrides,
  };
}

function fullSnapshot(): ObservationSnapshot {
  return {
    main: complete(MAIN),
    issues: complete([
      {
        number: 229,
        title: "Dashboard v0.1",
        state: "OPEN",
        dependencies: complete([]),
      },
    ]),
    pullRequests: complete([pull()]),
    stewardWatches: complete([watch()]),
    recentActivity: complete([
      { number: 399, title: "Previous merge" },
    ]),
  };
}

function deliveryForPr(projection: DashboardProjection, prNumber = 400) {
  const delivery = projection.deliveries.find(
    (candidate) => candidate.pullRequestNumber === prNumber,
  );
  expect(delivery, `expected delivery for PR #${prNumber}`).toBeDefined();
  return delivery!;
}

function expectCurrentValueCleared(projection: DashboardProjection) {
  expect(projection.executive.mainSha.value).toBeNull();
  expect(projection.recentActivity.value).toBeNull();
  expect(projection.attention.hold.value).toBeNull();
  expect(projection.attention.humanAction.value).toBeNull();

  for (const delivery of projection.deliveries) {
    expect(delivery.issueTitle.value).toBeNull();
    expect(delivery.issueState.value).toBeNull();
    expect(delivery.dependencies.value).toBeNull();
    expect(delivery.pullRequestTitle.value).toBeNull();
    expect(delivery.pullRequestState.value).toBeNull();
    expect(delivery.pullRequestDraft.value).toBeNull();
    expect(delivery.pullRequestHead.value).toBeNull();
    expect(delivery.pullRequestBase.value).toBeNull();
    expect(delivery.linkedIssues.value).toBeNull();
    expect(delivery.stewardWatch.status).toBe("unknown");
    expect(delivery.stewardWatch.verdict).toBeNull();
    expect(delivery.stewardWatch.humanAction).toBeNull();
  }
}

describe("#229 Dashboard v0.1 acceptance: current observation truth", () => {
  it("projects the five bounded surfaces from one fully observed current snapshot", () => {
    const projection = projectObservation({ latest: fullSnapshot() });

    expect(Object.keys(projection).sort()).toEqual([
      "attention",
      "criticalPath",
      "deliveries",
      "executive",
      "recentActivity",
    ]);
    expect(projection.executive.mainSha).toEqual({
      availability: "complete",
      value: MAIN,
    });
    expect(projection.recentActivity.availability).toBe("complete");

    const delivery = deliveryForPr(projection);
    expect(delivery.pullRequestTitle).toEqual({
      availability: "complete",
      value: "Fresh Dashboard v0.1",
    });
    expect(delivery.pullRequestHead.value).toBe(HEAD);
    expect(delivery.stewardWatch).toMatchObject({
      status: "current",
      verdict: "GREEN",
      humanAction: "none",
    });
  });

  it("keeps an exact-current Steward watch PR-scoped when one PR links multiple Issues", () => {
    const latest = fullSnapshot();
    latest.issues = complete([
      ...latest.issues.value!,
      {
        number: 304,
        title: "Second linked Issue",
        state: "OPEN",
        dependencies: complete([]),
      },
    ]);
    latest.pullRequests = complete([
      pull({ linkedIssues: complete([229, 304]) }),
    ]);
    latest.stewardWatches = complete([
      watch({ verdict: "HOLD", humanAction: "required" }),
    ]);

    const projection = projectObservation({ latest });
    const delivery = deliveryForPr(projection);

    expect(delivery.linkedIssues).toEqual({
      availability: "complete",
      value: [229, 304],
    });
    expect(delivery.stewardWatch).toMatchObject({
      status: "current",
      verdict: "HOLD",
      humanAction: "required",
    });
    expect(projection.attention.hold).toEqual({
      availability: "complete",
      value: true,
    });
    expect(projection.attention.humanAction).toEqual({
      availability: "complete",
      value: "required",
    });
  });

  it("does not let incomplete Issue↔PR linkage demote an otherwise exact-current Steward watch", () => {
    const latest = fullSnapshot();
    latest.pullRequests = complete([
      pull({ linkedIssues: partial([229]) }),
    ]);
    latest.stewardWatches = complete([
      watch({ verdict: "HOLD", humanAction: "required" }),
    ]);

    const projection = projectObservation({ latest });
    const delivery = deliveryForPr(projection);

    expect(delivery.linkedIssues.availability).toBe("partial");
    expect(delivery.stewardWatch).toMatchObject({
      status: "current",
      verdict: "HOLD",
      humanAction: "required",
    });
    expect(projection.attention.hold.value).toBe(true);
    expect(projection.attention.humanAction.value).toBe("required");
  });

  it("makes only head-bound current facts Unknown when PR head identity is not observed", () => {
    const latest = fullSnapshot();
    latest.pullRequests = complete([
      pull({ head: partial(null) }),
    ]);

    const projection = projectObservation({ latest });
    const delivery = deliveryForPr(projection);

    expect(delivery.pullRequestTitle).toEqual({
      availability: "complete",
      value: "Fresh Dashboard v0.1",
    });
    expect(delivery.pullRequestState).toEqual({
      availability: "complete",
      value: "OPEN",
    });
    expect(delivery.pullRequestHead).toEqual({
      availability: "partial",
      value: null,
    });
    expect(delivery.stewardWatch).toEqual({
      status: "unknown",
      verdict: null,
      humanAction: null,
    });
  });

  it("makes MAIN-bound watch evidence Unknown without erasing independent Issue/PR core facts", () => {
    const latest = fullSnapshot();
    latest.main = partial(null);

    const projection = projectObservation({ latest });
    const delivery = deliveryForPr(projection);

    expect(projection.executive.mainSha).toEqual({
      availability: "partial",
      value: null,
    });
    expect(delivery.issueTitle.value).toBe("Dashboard v0.1");
    expect(delivery.pullRequestTitle.value).toBe("Fresh Dashboard v0.1");
    expect(delivery.pullRequestHead.value).toBe(HEAD);
    expect(delivery.stewardWatch).toEqual({
      status: "unknown",
      verdict: null,
      humanAction: null,
    });
  });

  it("scopes an incomplete dependency observation to the dependency family", () => {
    const latest = fullSnapshot();
    latest.issues = complete([
      {
        number: 229,
        title: "Dashboard v0.1",
        state: "OPEN",
        dependencies: partial([200]),
      },
    ]);

    const projection = projectObservation({ latest });
    const delivery = deliveryForPr(projection);

    expect(delivery.issueTitle.value).toBe("Dashboard v0.1");
    expect(delivery.issueState.value).toBe("OPEN");
    expect(delivery.dependencies.availability).toBe("partial");
    expect(projection.criticalPath.availability).toBe("partial");
    expect(delivery.pullRequestTitle.value).toBe("Fresh Dashboard v0.1");
  });

  it("represents complete empty dependencies and linkage as known absence, not Unknown", () => {
    const latest = fullSnapshot();
    latest.issues = complete([
      {
        number: 229,
        title: "Dashboard v0.1",
        state: "OPEN",
        dependencies: complete([]),
      },
    ]);
    latest.pullRequests = complete([
      pull({ linkedIssues: complete([]) }),
    ]);

    const projection = projectObservation({ latest });
    const delivery = deliveryForPr(projection);

    expect(delivery.dependencies).toEqual({
      availability: "complete",
      value: [],
    });
    expect(delivery.linkedIssues).toEqual({
      availability: "complete",
      value: [],
    });
  });

  it("does not retain prior current values after a failed refresh", () => {
    const previous = projectObservation({ latest: fullSnapshot() });
    const failed: ObservationSnapshot = {
      main: unavailable(),
      issues: unavailable(),
      pullRequests: unavailable(),
      stewardWatches: unavailable(),
      recentActivity: unavailable(),
    };

    const projection = projectObservation({ latest: failed, previous });

    expectCurrentValueCleared(projection);
    expect(JSON.stringify(projection)).not.toContain(MAIN);
    expect(JSON.stringify(projection)).not.toContain(HEAD);
    expect(JSON.stringify(projection)).not.toContain("Fresh Dashboard v0.1");
  });

  it("preserves sufficient positive HOLD/required evidence under incomplete surrounding discovery", () => {
    const latest = fullSnapshot();
    latest.stewardWatches = partial([
      watch({ verdict: "HOLD", humanAction: "required" }),
    ]);

    const projection = projectObservation({ latest });

    expect(projection.attention.hold).toEqual({
      availability: "complete",
      value: true,
    });
    expect(projection.attention.humanAction).toEqual({
      availability: "complete",
      value: "required",
    });
  });

  it("does not fabricate healthy/none conclusions from incomplete absence", () => {
    const latest = fullSnapshot();
    latest.stewardWatches = partial([]);

    const projection = projectObservation({ latest });

    expect(projection.attention.hold.value).toBeNull();
    expect(projection.attention.hold.availability).toBe("partial");
    expect(projection.attention.humanAction.value).toBeNull();
    expect(projection.attention.humanAction.availability).toBe("partial");
  });

  it("keeps merge policy, checks/reviews, handoff parsing, and negative health summaries out of v0.1 current truth", () => {
    const projection = projectObservation({ latest: fullSnapshot() });
    const serialized = JSON.stringify(projection);

    expect(serialized).not.toMatch(/merge[_-]?ready|can[_-]?merge|mergeable/i);
    expect(serialized).not.toMatch(/agent-handoff|handoff/i);
    expect(serialized).not.toMatch(/reviewDecision|statusCheck|checks/i);
    expect(serialized).not.toMatch(/healthy|no blockers|human action none/i);
  });
});
