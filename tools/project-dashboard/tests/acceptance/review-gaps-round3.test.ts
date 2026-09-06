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
const WATCH_URL = "https://github.example/pull/321#watch";

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
  number: number,
  dependencies: Observed<readonly number[]> = complete([]),
): IssueObservation {
  return {
    number,
    title: `Issue ${number}`,
    state: "OPEN",
    dependencies,
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

function watch(
  overrides: Partial<StewardWatchObservation> = {},
): StewardWatchObservation {
  return {
    prNumber: 321,
    availability: "complete",
    verdict: "GREEN",
    humanAction: "none",
    head: HEAD,
    main: MAIN,
    sourceUrl: WATCH_URL,
    ...overrides,
  };
}

function snapshot(overrides: Partial<ObservationSnapshot> = {}): ObservationSnapshot {
  return {
    main: complete(MAIN),
    issues: complete([issue(229)]),
    pullRequests: complete([pull()]),
    stewardWatches: complete([watch()]),
    recentActivity: complete([{ number: 320, title: "Recent merge" }]),
    ...overrides,
  };
}

function prLane(projection: DashboardProjection, number = 321) {
  return projection.deliveries.find((lane) => lane.pullRequestNumber === number);
}

function issueLane(projection: DashboardProjection, number: number) {
  return projection.deliveries.find((lane) => lane.issueNumber === number);
}

describe("#229 final acceptance review gaps", () => {
  it("clears retained PR-base payloads when the base family is non-complete", () => {
    for (const base of [partial(MAIN), unavailable(MAIN)] as const) {
      const projection = projectObservation({
        latest: snapshot({
          pullRequests: complete([pull({ base })]),
        }),
      });

      expect(prLane(projection)?.pullRequestBase).toEqual({
        availability: base.availability,
        value: null,
      });
      expect(prLane(projection)?.pullRequestTitle).toEqual({
        availability: "complete",
        value: "PR 321",
      });
    }
  });

  it("does not promote an exact-current AMBER watch to HOLD attention", () => {
    const projection = projectObservation({
      latest: snapshot({
        stewardWatches: complete([watch({ verdict: "AMBER" })]),
      }),
    });

    expect(projection.attention.items).toEqual([]);
  });

  it("keeps AMBER required-action evidence positive without inventing HOLD", () => {
    const projection = projectObservation({
      latest: snapshot({
        stewardWatches: complete([
          watch({ verdict: "AMBER", humanAction: "required" }),
        ]),
      }),
    });

    expect(projection.attention.items).toEqual([
      {
        kind: "human-action-required",
        prNumber: 321,
        sourceUrl: WATCH_URL,
      },
    ]);
  });

  it("preserves unlinked complete Issues alongside linked PR lanes", () => {
    const projection = projectObservation({
      latest: snapshot({
        issues: complete([issue(229), issue(231)]),
        pullRequests: complete([pull({ linkedIssues: complete([229]) })]),
      }),
    });

    const locatorPairs = projection.deliveries
      .map((lane) => `${lane.issueNumber ?? "none"}:${lane.pullRequestNumber ?? "none"}`)
      .sort();
    expect(locatorPairs).toEqual(["229:321", "231:none"]);
    expect(issueLane(projection, 231)).toMatchObject({
      issueNumber: 231,
      issueTitle: { availability: "complete", value: "Issue 231" },
      issueState: { availability: "complete", value: "OPEN" },
      dependencies: { availability: "complete", value: [] },
      pullRequestNumber: null,
    });
  });

  it("projects complete current-work graph nodes and dependency edges without inventing path semantics", () => {
    const projection = projectObservation({
      latest: snapshot({
        issues: complete([
          issue(229, complete([230])),
          issue(230, complete([])),
          issue(999, complete([])),
        ]),
        pullRequests: complete([pull({ linkedIssues: complete([229]) })]),
      }),
    });

    expect(projection.criticalPath).toEqual({
      availability: "complete",
      issueNumbers: [229, 230, 999],
    });
    expect(issueLane(projection, 229)?.dependencies).toEqual({
      availability: "complete",
      value: [230],
    });
    expect(issueLane(projection, 230)?.dependencies).toEqual({
      availability: "complete",
      value: [],
    });
    expect(issueLane(projection, 999)?.dependencies).toEqual({
      availability: "complete",
      value: [],
    });
  });

  it("clears a retained partial recent-activity payload", () => {
    const projection = projectObservation({
      latest: snapshot({
        recentActivity: partial([{ number: 320, title: "retained" }]),
      }),
    });

    expect(projection.recentActivity).toEqual({
      availability: "partial",
      value: null,
    });
  });
});
