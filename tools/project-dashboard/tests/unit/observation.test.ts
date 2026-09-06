import { describe, expect, it } from "vitest";

import {
  projectObservation,
  type ObservationSnapshot,
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
});
