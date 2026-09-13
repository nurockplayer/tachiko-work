import { describe, expect, it } from "vitest";

import { projectObservation } from "../../src/server/observation.js";
import {
  complete,
  emptySnapshot,
  HEAD,
  issue,
  issueLane,
  issueOnlyLane,
  MAIN,
  OTHER_HEAD,
  OTHER_MAIN,
  partial,
  prLane,
  pull,
  recentSnapshot,
  unavailable,
  watch,
  WATCH_URL,
} from "./observation-fixtures.js";

describe("#229 collection and linkage acceptance", () => {

  it("ignores retained Issue and PR payloads when their outer observations are unavailable", () => {
    const projection = projectObservation({
      latest: recentSnapshot({
        issues: unavailable([issue()]),
        pullRequests: unavailable([pull()]),
      }),
    });

    expect(projection.deliveries).toEqual([]);
  });

  it("treats complete-empty native linkage as known unlinked and never falls back to the sole Issue", () => {
    const projection = projectObservation({
      latest: recentSnapshot({
        pullRequests: complete([pull(321, complete([]))]),
      }),
    });

    const lane = prLane(projection);
    expect(lane).toMatchObject({
      issueNumber: null,
      linkedIssues: { availability: "complete", value: [] },
    });
    expect(issueOnlyLane(projection)).toBeDefined();
  });

  it("does not associate a PR from a partial linkage even when the partial payload contains one Issue", () => {
    const projection = projectObservation({
      latest: recentSnapshot({
        pullRequests: complete([pull(321, partial([229]))]),
      }),
    });

    const lane = prLane(projection);
    expect(lane).toMatchObject({
      issueNumber: null,
      linkedIssues: { availability: "partial", value: null },
    });
    expect(issueOnlyLane(projection)).toBeDefined();
  });

  it("does not associate a PR from an unavailable linkage with a retained payload", () => {
    const projection = projectObservation({
      latest: recentSnapshot({
        pullRequests: complete([pull(321, unavailable([229]))]),
      }),
    });

    expect(prLane(projection)).toMatchObject({
      issueNumber: null,
      linkedIssues: { availability: "unavailable", value: null },
    });
  });

  it("supports complete multi-Issue linkage without changing PR-scoped positive watch currentness", () => {
    const projection = projectObservation({
      latest: recentSnapshot({
        issues: complete([issue(229), issue(231)]),
        pullRequests: complete([pull(321, complete([229, 231]))]),
        stewardWatches: complete([watch({ verdict: "HOLD" })]),
      }),
    });

    const linked = projection.deliveries
      .filter((lane) => lane.pullRequestNumber === 321)
      .map((lane) => lane.issueNumber)
      .sort();
    expect(linked).toEqual([229, 231]);
    expect(projection.attention.items).toEqual([
      {
        kind: "steward-hold",
        prNumber: 321,
        sourceUrl: "https://github.example/pull/321#watch",
      },
    ]);
  });

  it("can keep a PR locator/linkage current without consuming retained unavailable Issue core values", () => {
    const projection = projectObservation({
      latest: recentSnapshot({
        issues: unavailable([issue()]),
      }),
    });

    expect(prLane(projection)).toMatchObject({
      issueNumber: 229,
      issueTitle: { availability: "unavailable", value: null },
      issueState: { availability: "unavailable", value: null },
      pullRequestNumber: 321,
      pullRequestTitle: { availability: "complete", value: "PR 321" },
      linkedIssues: { availability: "complete", value: [229] },
    });
  });

  it("constrains the complete delivery collection to exactly the observed linked lane", () => {
    const projection = projectObservation({ latest: emptySnapshot() });

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
          ...emptySnapshot(),
          pullRequests: { availability, value: [pull()] },
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
        ...emptySnapshot(),
        pullRequests: {
          availability: "complete",
          value: [
            {
              ...pull(),
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

  it("preserves unlinked complete Issues alongside linked PR lanes", () => {
    const projection = projectObservation({
      latest: recentSnapshot({
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

  it("asserts every exposed delivery fact on complete evidence", () => {
    const projection = projectObservation({ latest: emptySnapshot() });

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
      latest: emptySnapshot({
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

  it("preserves independently complete Issue data when PR discovery is partial", () => {
    const projection = projectObservation({
      latest: emptySnapshot({ pullRequests: partial([pull()]) }),
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
      latest: emptySnapshot({ pullRequests: unavailable([pull()]) }),
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

