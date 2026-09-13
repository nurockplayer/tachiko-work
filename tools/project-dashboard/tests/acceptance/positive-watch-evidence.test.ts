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

describe("#229 positive watch evidence acceptance", () => {

  it("does not expose removed merge, handoff, check/review, or negative-health surfaces", () => {
    const serialized = JSON.stringify(projectObservation({ latest: recentSnapshot() }));

    expect(serialized).not.toMatch(/agent.?handoff/i);
    expect(serialized).not.toMatch(/merge.?ready|can.?merge|mergeable/i);
    expect(serialized).not.toMatch(/reviewDecision|statusCheck|checks|reviews/i);
    expect(serialized).not.toMatch(/stewardWatch/i);
    expect(serialized).not.toContain('"hold":false');
    expect(serialized).not.toContain('"humanAction":"none"');
    expect(serialized).not.toMatch(/healthy|no blockers|clean bill/i);
  });

  it("does not use a retained unavailable Steward-watch payload as positive current evidence", () => {
    const projection = projectObservation({
      latest: recentSnapshot({
        stewardWatches: unavailable([
          watch({ verdict: "HOLD", humanAction: "required" }),
        ]),
      }),
    });

    expect(projection.attention.items).toEqual([]);
  });

  it("preserves a directly observed positive watch from a partial watch-discovery set", () => {
    const projection = projectObservation({
      latest: recentSnapshot({
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
      latest: recentSnapshot({
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
      latest: recentSnapshot({
        pullRequests: unavailable([pull()]),
        stewardWatches: complete([watch({ verdict: "HOLD" })]),
      }),
    });

    expect(projection.attention.items).toEqual([]);
  });

  it("requires exact PR HEAD for positive watch currentness", () => {
    const projection = projectObservation({
      latest: recentSnapshot({
        stewardWatches: complete([
          watch({ verdict: "HOLD", humanAction: "required", head: OTHER_HEAD }),
        ]),
      }),
    });

    expect(projection.attention.items).toEqual([]);
  });

  it("requires exact live MAIN for positive watch currentness", () => {
    const projection = projectObservation({
      latest: recentSnapshot({
        stewardWatches: complete([
          watch({ verdict: "HOLD", humanAction: "required", main: OTHER_MAIN }),
        ]),
      }),
    });

    expect(projection.attention.items).toEqual([]);
  });

  it("does not promote a non-complete individual watch candidate", () => {
    const projection = projectObservation({
      latest: recentSnapshot({
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
        latest: recentSnapshot({
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
        latest: recentSnapshot({
          pullRequests: complete([pull(321, complete([229]), head)]),
          stewardWatches: complete([watch({ verdict: "HOLD" })]),
        }),
      });

      expect(projection.attention.items).toEqual([]);
      expect(prLane(projection)?.pullRequestHead.value).toBeNull();
    }
  });

  it("represents complete GREEN/none evidence as absence of positive items, not as a negative health claim", () => {
    const projection = projectObservation({
      latest: recentSnapshot({
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
      latest: recentSnapshot({
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
      latest: recentSnapshot({
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

  it("rejects an individually unavailable retained watch candidate", () => {
    const projection = projectObservation({
      latest: {
        ...emptySnapshot(),
        stewardWatches: {
          availability: "complete",
          value: [
            {
              ...watch(),
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

  it("requires the watched PR to exist in a complete current PR observation", () => {
    const projection = projectObservation({
      latest: {
        ...emptySnapshot(),
        pullRequests: { availability: "complete", value: [] },
        stewardWatches: {
          availability: "complete",
          value: [
            {
              ...watch(),
              verdict: "HOLD",
              humanAction: "required",
            },
          ],
        },
      },
    });

    expect(projection.attention.items).toEqual([]);
  });

  it("does not promote an exact-current AMBER watch to HOLD attention", () => {
    const projection = projectObservation({
      latest: recentSnapshot({
        stewardWatches: complete([watch({ verdict: "AMBER" })]),
      }),
    });

    expect(projection.attention.items).toEqual([]);
  });

  it("keeps AMBER required-action evidence positive without inventing HOLD", () => {
    const projection = projectObservation({
      latest: recentSnapshot({
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
});

