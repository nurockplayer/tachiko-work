import { describe, expect, it } from "vitest";

import {
  projectObservation,
  type IssueObservation,
  type ObservationSnapshot,
  type PullObservation,
  type StewardWatchObservation,
} from "../../src/server/observation.js";

const MAIN = "1".repeat(40);
const HEAD = "2".repeat(40);

const ISSUE: IssueObservation = {
  number: 229,
  title: "Issue 229",
  state: "OPEN",
  dependencies: { availability: "complete", value: [] },
};

const PULL: PullObservation = {
  number: 321,
  title: "PR 321",
  state: "OPEN",
  draft: true,
  head: { availability: "complete", value: HEAD },
  base: { availability: "complete", value: MAIN },
  linkedIssues: { availability: "complete", value: [229] },
};

const WATCH: StewardWatchObservation = {
  prNumber: 321,
  availability: "complete",
  verdict: "GREEN",
  humanAction: "none",
  head: HEAD,
  main: MAIN,
  sourceUrl: "https://github.example/pull/321#watch",
};

const SNAPSHOT: ObservationSnapshot = {
  main: { availability: "complete", value: MAIN },
  issues: { availability: "complete", value: [ISSUE] },
  pullRequests: { availability: "complete", value: [PULL] },
  stewardWatches: { availability: "complete", value: [WATCH] },
  recentActivity: { availability: "complete", value: [] },
};

describe("#229 second-round acceptance review gaps", () => {
  it("constrains the complete delivery collection to exactly the observed linked lane", () => {
    const projection = projectObservation({ latest: SNAPSHOT });

    expect(projection.deliveries).toEqual([
      {
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
      },
    ]);
  });

  it("preserves exactly the independently complete Issue lane when PR discovery is incomplete", () => {
    for (const availability of ["partial", "unavailable"] as const) {
      const projection = projectObservation({
        latest: {
          ...SNAPSHOT,
          pullRequests: { availability, value: [PULL] },
        },
      });

      expect(projection.deliveries).toHaveLength(1);
      expect(projection.deliveries[0]).toMatchObject({
        issueNumber: 229,
        issueTitle: { availability: "complete", value: "Issue 229" },
        issueState: { availability: "complete", value: "OPEN" },
        dependencies: { availability: "complete", value: [] },
        pullRequestNumber: null,
      });
      expect(
        projection.deliveries.some((lane) => lane.pullRequestNumber !== null),
      ).toBe(false);
    }
  });

  it("preserves the Issue-only lane when complete PR core has unavailable native linkage", () => {
    const projection = projectObservation({
      latest: {
        ...SNAPSHOT,
        pullRequests: {
          availability: "complete",
          value: [
            {
              ...PULL,
              linkedIssues: { availability: "unavailable", value: [229] },
            },
          ],
        },
      },
    });

    const locatorPairs = projection.deliveries.map((lane) => [
      lane.issueNumber,
      lane.pullRequestNumber,
    ]);
    expect(locatorPairs).toHaveLength(2);
    expect(locatorPairs).toEqual(
      expect.arrayContaining([
        [229, null],
        [null, 321],
      ]),
    );

    const prLane = projection.deliveries.find(
      (lane) => lane.pullRequestNumber === 321,
    );
    expect(prLane).toMatchObject({
      issueNumber: null,
      linkedIssues: { availability: "unavailable", value: null },
    });
  });

  it("clears critical path across a failed refresh instead of carrying previous success", () => {
    const previous = projectObservation({ latest: SNAPSHOT });
    const failed = projectObservation({
      latest: {
        main: { availability: "unavailable", value: MAIN },
        issues: { availability: "unavailable", value: [ISSUE] },
        pullRequests: { availability: "unavailable", value: [PULL] },
        stewardWatches: { availability: "unavailable", value: [WATCH] },
        recentActivity: { availability: "unavailable", value: [] },
      },
      previous,
    });

    expect(failed.criticalPath).toEqual({
      availability: "unavailable",
      issueNumbers: [],
    });
  });

  it("rejects an individually unavailable retained watch candidate", () => {
    const projection = projectObservation({
      latest: {
        ...SNAPSHOT,
        stewardWatches: {
          availability: "complete",
          value: [
            {
              ...WATCH,
              availability: "unavailable",
              verdict: "HOLD",
              humanAction: "required",
            },
          ],
        },
      },
    });

    expect(projection.attention.items).toEqual([]);
  });

  it("keeps critical path partial and empty when Issue discovery is partial", () => {
    const projection = projectObservation({
      latest: {
        ...SNAPSHOT,
        issues: { availability: "partial", value: [ISSUE] },
      },
    });

    expect(projection.criticalPath).toEqual({
      availability: "partial",
      issueNumbers: [],
    });
  });

  it("requires the watched PR to exist in a complete current PR observation", () => {
    const projection = projectObservation({
      latest: {
        ...SNAPSHOT,
        pullRequests: { availability: "complete", value: [] },
        stewardWatches: {
          availability: "complete",
          value: [
            {
              ...WATCH,
              verdict: "HOLD",
              humanAction: "required",
            },
          ],
        },
      },
    });

    expect(projection.attention.items).toEqual([]);
  });
});
