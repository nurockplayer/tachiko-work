import { describe, expect, it } from "vitest";

import {
  projectObservation,
  type IssueObservation,
  type ObservationSnapshot,
  type Observed,
} from "../../src/server/observation.js";

const MAIN = "1".repeat(40);
const HEAD = "2".repeat(40);

function snapshot(): ObservationSnapshot {
  return {
    main: { availability: "complete", value: null },
    issues: { availability: "complete", value: [] },
    pullRequests: {
      availability: "complete",
      value: [
        {
          number: 321,
          title: "PR 321",
          state: "OPEN",
          draft: true,
          head: { availability: "complete", value: null },
          base: { availability: "complete", value: MAIN },
          linkedIssues: { availability: "complete", value: null },
        },
      ],
    },
    stewardWatches: {
      availability: "complete",
      value: [
        {
          prNumber: 321,
          availability: "complete",
          verdict: "HOLD",
          humanAction: "required",
          head: HEAD,
          main: MAIN,
          sourceUrl: "https://github.example/pull/321#watch",
        },
      ],
    },
    recentActivity: { availability: "complete", value: [] },
  };
}

function issue(
  number: number,
  dependencies: Observed<readonly number[]>,
): IssueObservation {
  return {
    number,
    title: `Issue ${number}`,
    state: "OPEN",
    dependencies,
  };
}

describe("projectObservation unit regressions", () => {
  it("fails closed when a complete observation lacks a required payload", () => {
    const projection = projectObservation({ latest: snapshot() });

    expect(projection.executive.mainSha).toEqual({
      availability: "unavailable",
      value: null,
    });
    expect(projection.deliveries[0]).toMatchObject({
      pullRequestHead: { availability: "unavailable", value: null },
      linkedIssues: { availability: "unavailable", value: null },
    });
    expect(projection.attention.items).toEqual([]);
  });

  it("does not claim a complete current-work graph when an edge payload is absent", () => {
    const latest = snapshot();
    latest.issues = {
      availability: "complete",
      value: [
        {
          number: 229,
          title: "Issue 229",
          state: "OPEN",
          dependencies: { availability: "complete", value: null },
        },
      ],
    };

    expect(projectObservation({ latest }).criticalPath).toEqual({
      availability: "unavailable",
      issueNumbers: [],
    });
  });

  it("preserves partial status for a null dependency observation", () => {
    const latest = snapshot();
    latest.issues = {
      availability: "complete",
      value: [
        issue(229, { availability: "partial", value: null }),
      ],
    };

    const projection = projectObservation({ latest });
    expect(
      projection.deliveries.find((lane) => lane.issueNumber === 229)?.dependencies,
    ).toEqual({
      availability: "partial",
      value: null,
    });
    expect(projection.criticalPath).toEqual({
      availability: "partial",
      issueNumbers: [],
    });
  });

  it("keeps unavailable dominance while preserving partial/null sibling order", () => {
    const cases = [
      {
        dependencies: [
          { availability: "partial", value: null },
          { availability: "complete", value: [] },
        ],
        availability: "partial",
      },
      {
        dependencies: [
          { availability: "complete", value: [] },
          { availability: "partial", value: null },
        ],
        availability: "partial",
      },
      {
        dependencies: [
          { availability: "partial", value: null },
          { availability: "unavailable", value: null },
        ],
        availability: "unavailable",
      },
      {
        dependencies: [
          { availability: "unavailable", value: null },
          { availability: "partial", value: null },
        ],
        availability: "unavailable",
      },
    ] as const;

    for (const { dependencies, availability } of cases) {
      const latest = snapshot();
      latest.issues = {
        availability: "complete",
        value: dependencies.map((dependency, index) => issue(229 + index, dependency)),
      };

      expect(projectObservation({ latest }).criticalPath).toEqual({
        availability,
        issueNumbers: [],
      });
    }
  });
});
