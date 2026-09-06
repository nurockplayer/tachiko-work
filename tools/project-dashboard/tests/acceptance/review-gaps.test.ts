import { describe, expect, it } from "vitest";

import {
  projectObservation,
  type DashboardProjection,
  type IssueObservation,
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

function unavailable<T>(value: T | null): Observed<T> {
  return { availability: "unavailable", value };
}

function issue(
  overrides: Partial<IssueObservation> = {},
): IssueObservation {
  return {
    number: 229,
    title: "Issue 229",
    state: "OPEN",
    dependencies: complete([]),
    ...overrides,
  };
}

function pull(
  overrides: Partial<PullObservation> = {},
): PullObservation {
  return {
    number: 321,
    title: "PR 321",
    state: "OPEN",
    draft: true,
    head: complete(HEAD),
    base: complete(MAIN),
    linkedIssues: complete([229]),
    ...overrides,
  };
}

function watch(): StewardWatchObservation {
  return {
    prNumber: 321,
    availability: "complete",
    verdict: "GREEN",
    humanAction: "none",
    head: HEAD,
    main: MAIN,
    sourceUrl: "https://github.example/pull/321#watch",
  };
}

function snapshot(overrides: Partial<ObservationSnapshot> = {}): ObservationSnapshot {
  return {
    main: complete(MAIN),
    issues: complete([issue()]),
    pullRequests: complete([pull()]),
    stewardWatches: complete([watch()]),
    recentActivity: complete([]),
    ...overrides,
  };
}

function prLane(projection: DashboardProjection, number = 321) {
  return projection.deliveries.find((lane) => lane.pullRequestNumber === number);
}

function issueOnlyLane(projection: DashboardProjection, number = 229) {
  return projection.deliveries.find(
    (lane) => lane.issueNumber === number && lane.pullRequestNumber === null,
  );
}

describe("#229 acceptance review gaps", () => {
  it("asserts every exposed delivery fact on complete evidence", () => {
    const projection = projectObservation({ latest: snapshot() });

    expect(prLane(projection)).toEqual({
      issueNumber: 229,
      issueTitle: { availability: "complete", value: "Issue 229" },
      issueState: { availability: "complete", value: "OPEN" },
      dependencies: { availability: "complete", value: [] },
      pullRequestNumber: 321,
      pullRequestTitle: { availability: "complete", value: "PR 321" },
      pullRequestState: { availability: "complete", value: "OPEN" },
      pullRequestDraft: { availability: "complete", value: true },
      pullRequestHead: { availability: "complete", value: HEAD },
      pullRequestBase: { availability: "complete", value: MAIN },
      linkedIssues: { availability: "complete", value: [229] },
    });
  });

  it("projects non-default CLOSED/MERGED/non-draft delivery facts", () => {
    const projection = projectObservation({
      latest: snapshot({
        issues: complete([issue({ state: "CLOSED" })]),
        pullRequests: complete([
          pull({ state: "MERGED", draft: false }),
        ]),
      }),
    });

    expect(prLane(projection)).toMatchObject({
      issueState: { availability: "complete", value: "CLOSED" },
      pullRequestState: { availability: "complete", value: "MERGED" },
      pullRequestDraft: { availability: "complete", value: false },
    });
  });

  it("makes critical path unavailable and empty when Issue discovery is unavailable even with retained payload", () => {
    const projection = projectObservation({
      latest: snapshot({ issues: unavailable([issue()]) }),
    });

    expect(projection.criticalPath).toEqual({
      availability: "unavailable",
      issueNumbers: [],
    });
  });

  it("makes critical path unavailable and empty when a dependency family is unavailable with retained payload", () => {
    const projection = projectObservation({
      latest: snapshot({
        issues: complete([
          issue({ dependencies: unavailable([200]) }),
        ]),
      }),
    });

    expect(projection.criticalPath).toEqual({
      availability: "unavailable",
      issueNumbers: [],
    });
  });

  it("preserves independently complete Issue data when PR discovery is partial", () => {
    const projection = projectObservation({
      latest: snapshot({ pullRequests: partial([pull()]) }),
    });

    expect(issueOnlyLane(projection)).toMatchObject({
      issueNumber: 229,
      issueTitle: { availability: "complete", value: "Issue 229" },
      issueState: { availability: "complete", value: "OPEN" },
      dependencies: { availability: "complete", value: [] },
      pullRequestNumber: null,
      pullRequestTitle: { availability: "partial", value: null },
      pullRequestState: { availability: "partial", value: null },
      pullRequestDraft: { availability: "partial", value: null },
      pullRequestHead: { availability: "partial", value: null },
      pullRequestBase: { availability: "partial", value: null },
      linkedIssues: { availability: "partial", value: null },
    });
    expect(prLane(projection)).toBeUndefined();
  });

  it("preserves independently complete Issue data when PR discovery is unavailable with retained payload", () => {
    const projection = projectObservation({
      latest: snapshot({ pullRequests: unavailable([pull()]) }),
    });

    expect(issueOnlyLane(projection)).toMatchObject({
      issueNumber: 229,
      issueTitle: { availability: "complete", value: "Issue 229" },
      issueState: { availability: "complete", value: "OPEN" },
      dependencies: { availability: "complete", value: [] },
      pullRequestNumber: null,
      pullRequestTitle: { availability: "unavailable", value: null },
      pullRequestState: { availability: "unavailable", value: null },
      pullRequestDraft: { availability: "unavailable", value: null },
      pullRequestHead: { availability: "unavailable", value: null },
      pullRequestBase: { availability: "unavailable", value: null },
      linkedIssues: { availability: "unavailable", value: null },
    });
    expect(prLane(projection)).toBeUndefined();
  });
});
