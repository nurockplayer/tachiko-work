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
const OTHER_HEAD = "3".repeat(40);
const OTHER_MAIN = "4".repeat(40);

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
  number = 229,
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
  number = 321,
  linkedIssues: Observed<readonly number[]> = complete([229]),
  head: Observed<string> = complete(HEAD),
): PullObservation {
  return {
    number,
    title: `PR ${number}`,
    state: "OPEN",
    draft: true,
    head,
    base: complete(MAIN),
    linkedIssues,
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
    sourceUrl: "https://github.example/pull/321#watch",
    ...overrides,
  };
}

function snapshot(overrides: Partial<ObservationSnapshot> = {}): ObservationSnapshot {
  return {
    main: complete(MAIN),
    issues: complete([issue()]),
    pullRequests: complete([pull()]),
    stewardWatches: complete([watch()]),
    recentActivity: complete([{ number: 320, title: "Recent merge" }]),
    ...overrides,
  };
}

function prLane(projection: DashboardProjection, prNumber = 321) {
  return projection.deliveries.find(
    (lane) => lane.pullRequestNumber === prNumber,
  );
}

function issueOnlyLane(projection: DashboardProjection, issueNumber = 229) {
  return projection.deliveries.find(
    (lane) =>
      lane.issueNumber === issueNumber && lane.pullRequestNumber === null,
  );
}

describe("#229 positive-only observation acceptance", () => {
  it("projects the bounded five current surfaces on complete evidence", () => {
    const projection = projectObservation({ latest: snapshot() });

    expect(projection.executive.mainSha).toEqual({
      availability: "complete",
      value: MAIN,
    });
    expect(prLane(projection)).toMatchObject({
      issueNumber: 229,
      issueTitle: { availability: "complete", value: "Issue 229" },
      pullRequestNumber: 321,
      pullRequestTitle: { availability: "complete", value: "PR 321" },
      pullRequestHead: { availability: "complete", value: HEAD },
      linkedIssues: { availability: "complete", value: [229] },
    });
    expect(projection.criticalPath).toEqual({
      availability: "complete",
      issueNumbers: [229],
    });
    expect(projection.recentActivity).toEqual({
      availability: "complete",
      value: [{ number: 320, title: "Recent merge" }],
    });
    expect(projection.attention.items).toEqual([]);
  });

  it("does not expose removed merge, handoff, check/review, or negative-health surfaces", () => {
    const serialized = JSON.stringify(projectObservation({ latest: snapshot() }));

    expect(serialized).not.toMatch(/agent.?handoff/i);
    expect(serialized).not.toMatch(/merge.?ready|can.?merge|mergeable/i);
    expect(serialized).not.toMatch(/reviewDecision|statusCheck|checks|reviews/i);
    expect(serialized).not.toMatch(/stewardWatch/i);
    expect(serialized).not.toContain('"hold":false');
    expect(serialized).not.toContain('"humanAction":"none"');
    expect(serialized).not.toMatch(/healthy|no blockers|clean bill/i);
  });

  it("ignores retained Issue and PR payloads when their outer observations are unavailable", () => {
    const projection = projectObservation({
      latest: snapshot({
        issues: unavailable([issue()]),
        pullRequests: unavailable([pull()]),
      }),
    });

    expect(projection.deliveries).toEqual([]);
  });

  it("clears retained main and recent-activity payloads when their families are unavailable", () => {
    const projection = projectObservation({
      latest: snapshot({
        main: unavailable(MAIN),
        recentActivity: unavailable([{ number: 999, title: "stale" }]),
      }),
    });

    expect(projection.executive.mainSha).toEqual({
      availability: "unavailable",
      value: null,
    });
    expect(projection.recentActivity).toEqual({
      availability: "unavailable",
      value: null,
    });
  });

  it("does not use a retained unavailable Steward-watch payload as positive current evidence", () => {
    const projection = projectObservation({
      latest: snapshot({
        stewardWatches: unavailable([
          watch({ verdict: "HOLD", humanAction: "required" }),
        ]),
      }),
    });

    expect(projection.attention.items).toEqual([]);
  });

  it("preserves a directly observed positive watch from a partial watch-discovery set", () => {
    const projection = projectObservation({
      latest: snapshot({
        stewardWatches: partial([
          watch({ verdict: "HOLD", humanAction: "required" }),
        ]),
      }),
    });

    expect(projection.attention.items).toEqual([
      {
        kind: "steward-hold",
        prNumber: 321,
        sourceUrl: "https://github.example/pull/321#watch",
      },
      {
        kind: "human-action-required",
        prNumber: 321,
        sourceUrl: "https://github.example/pull/321#watch",
      },
    ]);
  });

  it("preserves positive watch evidence from an observed PR even when PR discovery is partial", () => {
    const projection = projectObservation({
      latest: snapshot({
        pullRequests: partial([pull()]),
        stewardWatches: partial([watch({ verdict: "HOLD" })]),
      }),
    });

    expect(projection.attention.items).toContainEqual({
      kind: "steward-hold",
      prNumber: 321,
      sourceUrl: "https://github.example/pull/321#watch",
    });
    expect(projection.deliveries.every((lane) => lane.pullRequestNumber !== 321)).toBe(true);
  });

  it("does not use a retained PR head from an unavailable PR collection to validate a watch", () => {
    const projection = projectObservation({
      latest: snapshot({
        pullRequests: unavailable([pull()]),
        stewardWatches: complete([watch({ verdict: "HOLD" })]),
      }),
    });

    expect(projection.attention.items).toEqual([]);
  });

  it("treats complete-empty native linkage as known unlinked and never falls back to the sole Issue", () => {
    const projection = projectObservation({
      latest: snapshot({
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
      latest: snapshot({
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
      latest: snapshot({
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
      latest: snapshot({
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

  it("requires exact PR HEAD for positive watch currentness", () => {
    const projection = projectObservation({
      latest: snapshot({
        stewardWatches: complete([
          watch({ verdict: "HOLD", humanAction: "required", head: OTHER_HEAD }),
        ]),
      }),
    });

    expect(projection.attention.items).toEqual([]);
  });

  it("requires exact live MAIN for positive watch currentness", () => {
    const projection = projectObservation({
      latest: snapshot({
        stewardWatches: complete([
          watch({ verdict: "HOLD", humanAction: "required", main: OTHER_MAIN }),
        ]),
      }),
    });

    expect(projection.attention.items).toEqual([]);
  });

  it("does not promote a non-complete individual watch candidate", () => {
    const projection = projectObservation({
      latest: snapshot({
        stewardWatches: partial([
          watch({
            availability: "partial",
            verdict: "HOLD",
            humanAction: "required",
          }),
        ]),
      }),
    });

    expect(projection.attention.items).toEqual([]);
  });

  it("invalidates positive watch evidence when live main is non-complete even if a retained SHA matches", () => {
    for (const main of [partial(MAIN), unavailable(MAIN)] as const) {
      const projection = projectObservation({
        latest: snapshot({
          main,
          stewardWatches: complete([
            watch({ verdict: "HOLD", humanAction: "required" }),
          ]),
        }),
      });

      expect(projection.executive.mainSha.value).toBeNull();
      expect(projection.attention.items).toEqual([]);
    }
  });

  it("invalidates positive watch evidence when PR head is non-complete even if a retained SHA matches", () => {
    for (const head of [partial(HEAD), unavailable(HEAD)] as const) {
      const projection = projectObservation({
        latest: snapshot({
          pullRequests: complete([pull(321, complete([229]), head)]),
          stewardWatches: complete([watch({ verdict: "HOLD" })]),
        }),
      });

      expect(projection.attention.items).toEqual([]);
      expect(prLane(projection)?.pullRequestHead.value).toBeNull();
    }
  });

  it("scopes incomplete dependencies to the dependency/current-path family", () => {
    const projection = projectObservation({
      latest: snapshot({
        issues: complete([issue(229, partial([200]))]),
      }),
    });

    expect(prLane(projection)).toMatchObject({
      issueTitle: { availability: "complete", value: "Issue 229" },
      pullRequestTitle: { availability: "complete", value: "PR 321" },
      dependencies: { availability: "partial", value: null },
    });
    expect(projection.criticalPath).toEqual({
      availability: "partial",
      issueNumbers: [],
    });
  });

  it("can keep a PR locator/linkage current without consuming retained unavailable Issue core values", () => {
    const projection = projectObservation({
      latest: snapshot({
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

  it("never carries a previous successful current value into a failed latest refresh", () => {
    const previous = projectObservation({ latest: snapshot() });
    const failed = projectObservation({
      latest: snapshot({
        main: unavailable(MAIN),
        issues: unavailable([issue()]),
        pullRequests: unavailable([pull()]),
        stewardWatches: unavailable([watch({ verdict: "HOLD" })]),
        recentActivity: unavailable([{ number: 320, title: "Recent merge" }]),
      }),
      previous,
    });

    expect(failed.executive.mainSha.value).toBeNull();
    expect(failed.deliveries).toEqual([]);
    expect(failed.recentActivity.value).toBeNull();
    expect(failed.attention.items).toEqual([]);
  });

  it("represents complete GREEN/none evidence as absence of positive items, not as a negative health claim", () => {
    const projection = projectObservation({
      latest: snapshot({
        stewardWatches: complete([watch({ verdict: "GREEN", humanAction: "none" })]),
      }),
    });

    expect(projection.attention.items).toEqual([]);
    const serialized = JSON.stringify(projection.attention);
    expect(serialized).not.toContain("false");
    expect(serialized).not.toContain("none");
    expect(serialized).not.toContain("GREEN");
  });

  it("preserves required action even when another observed watch cannot be validated", () => {
    const secondPull = pull(322, complete([]), complete(OTHER_HEAD));
    const projection = projectObservation({
      latest: snapshot({
        pullRequests: complete([pull(), secondPull]),
        stewardWatches: partial([
          watch({ humanAction: "required" }),
          watch({
            prNumber: 322,
            head: "f".repeat(40),
            sourceUrl: "https://github.example/pull/322#watch",
          }),
        ]),
      }),
    });

    expect(projection.attention.items).toContainEqual({
      kind: "human-action-required",
      prNumber: 321,
      sourceUrl: "https://github.example/pull/321#watch",
    });
  });

  it("deduplicates HOLD and required attention per PR/source without inventing any negative item", () => {
    const projection = projectObservation({
      latest: snapshot({
        stewardWatches: partial([
          watch({ verdict: "HOLD", humanAction: "required" }),
          watch({ verdict: "HOLD", humanAction: "required" }),
        ]),
      }),
    });

    expect(projection.attention.items).toEqual([
      {
        kind: "steward-hold",
        prNumber: 321,
        sourceUrl: "https://github.example/pull/321#watch",
      },
      {
        kind: "human-action-required",
        prNumber: 321,
        sourceUrl: "https://github.example/pull/321#watch",
      },
    ]);
  });
});
