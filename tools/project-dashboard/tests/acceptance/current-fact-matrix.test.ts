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
  dependencies: Observed<readonly number[]> = complete([]),
): IssueObservation {
  return {
    number: 229,
    title: "Issue 229",
    state: "OPEN",
    dependencies,
  };
}

function pull(head: Observed<string> = complete(HEAD)): PullObservation {
  return {
    number: 321,
    title: "PR 321",
    state: "OPEN",
    draft: true,
    head,
    base: complete(MAIN),
    linkedIssues: complete([229]),
  };
}

const WATCH: StewardWatchObservation = {
  prNumber: 321,
  availability: "complete",
  verdict: "GREEN",
  humanAction: "none",
  head: HEAD,
  main: MAIN,
  sourceUrl: "https://github.example/pull/321#watch",
};

function snapshot(overrides: Partial<ObservationSnapshot> = {}): ObservationSnapshot {
  return {
    main: complete(MAIN),
    issues: complete([issue()]),
    pullRequests: complete([pull()]),
    stewardWatches: complete([WATCH]),
    recentActivity: complete([]),
    ...overrides,
  };
}

function prLane(projection: DashboardProjection) {
  return projection.deliveries.find((lane) => lane.pullRequestNumber === 321);
}

describe("#229 bounded current-fact availability matrix", () => {
  it("preserves partial availability while clearing a retained live-main payload", () => {
    const projection = projectObservation({
      latest: snapshot({ main: partial(MAIN) }),
    });

    expect(projection.executive.mainSha).toEqual({
      availability: "partial",
      value: null,
    });
  });

  it("clears Issue dependencies with the outer unavailable Issue family", () => {
    const projection = projectObservation({
      latest: snapshot({ issues: unavailable([issue(complete([200]))]) }),
    });

    expect(prLane(projection)).toMatchObject({
      issueNumber: 229,
      issueTitle: { availability: "unavailable", value: null },
      issueState: { availability: "unavailable", value: null },
      dependencies: { availability: "unavailable", value: null },
      pullRequestNumber: 321,
      pullRequestTitle: { availability: "complete", value: "PR 321" },
    });
  });

  it("preserves unavailable dependency-family availability while clearing retained edges", () => {
    const projection = projectObservation({
      latest: snapshot({
        issues: complete([issue(unavailable([200]))]),
      }),
    });

    expect(prLane(projection)?.dependencies).toEqual({
      availability: "unavailable",
      value: null,
    });
    expect(prLane(projection)?.issueTitle).toEqual({
      availability: "complete",
      value: "Issue 229",
    });
  });

  it("preserves each non-complete PR-head availability while clearing retained SHA", () => {
    for (const head of [partial(HEAD), unavailable(HEAD)] as const) {
      const projection = projectObservation({
        latest: snapshot({ pullRequests: complete([pull(head)]) }),
      });

      expect(prLane(projection)?.pullRequestHead).toEqual({
        availability: head.availability,
        value: null,
      });
    }
  });

  it("keeps a complete-empty recent-activity observation known empty", () => {
    const projection = projectObservation({
      latest: snapshot({ recentActivity: complete([]) }),
    });

    expect(projection.recentActivity).toEqual({
      availability: "complete",
      value: [],
    });
  });
});
