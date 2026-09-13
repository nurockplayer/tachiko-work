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

describe("#229 availability and currentness acceptance", () => {

  it("clears retained main and recent-activity payloads when their families are unavailable", () => {
    const projection = projectObservation({
      latest: recentSnapshot({
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

  it("scopes incomplete dependencies to the dependency/current-path family", () => {
    const projection = projectObservation({
      latest: recentSnapshot({
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

  it("never carries a previous successful current value into a failed latest refresh", () => {
    const previous = projectObservation({ latest: recentSnapshot() });
    const failed = projectObservation({
      latest: recentSnapshot({
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

  it("preserves partial availability while clearing a retained live-main payload", () => {
    const projection = projectObservation({
      latest: emptySnapshot({ main: partial(MAIN) }),
    });

    expect(projection.executive.mainSha).toEqual({
      availability: "partial",
      value: null,
    });
  });

  it("clears Issue dependencies with the outer unavailable Issue family", () => {
    const projection = projectObservation({
      latest: emptySnapshot({ issues: unavailable([issue(complete([200]))]) }),
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
      latest: emptySnapshot({
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
        latest: emptySnapshot({ pullRequests: complete([pull(head)]) }),
      });

      expect(prLane(projection)?.pullRequestHead).toEqual({
        availability: head.availability,
        value: null,
      });
    }
  });

  it("keeps a complete-empty recent-activity observation known empty", () => {
    const projection = projectObservation({
      latest: emptySnapshot({ recentActivity: complete([]) }),
    });

    expect(projection.recentActivity).toEqual({
      availability: "complete",
      value: [],
    });
  });

  it("clears critical path across a failed refresh instead of carrying previous success", () => {
    const previous = projectObservation({ latest: emptySnapshot() });
    const failed = projectObservation({
      latest: {
        main: { availability: "unavailable", value: MAIN },
        issues: { availability: "unavailable", value: [issue()] },
        pullRequests: { availability: "unavailable", value: [pull()] },
        stewardWatches: { availability: "unavailable", value: [watch()] },
        recentActivity: { availability: "unavailable", value: [] },
      },
      previous,
    });

    expect(failed.criticalPath).toEqual({
      availability: "unavailable",
      issueNumbers: [],
    });
  });

  it("fails closed Issue facts and critical path when Issue discovery is partial", () => {
    const projection = projectObservation({
      latest: {
        ...emptySnapshot(),
        issues: { availability: "partial", value: [issue()] },
      },
    });

    const linkedLane = projection.deliveries.find(
      (lane) => lane.pullRequestNumber === 321,
    );
    expect(linkedLane).toMatchObject({
      issueNumber: 229,
      issueTitle: { availability: "partial", value: null },
      issueState: { availability: "partial", value: null },
      dependencies: { availability: "partial", value: null },
      pullRequestNumber: 321,
      pullRequestTitle: { availability: "complete", value: "PR 321" },
      linkedIssues: { availability: "complete", value: [229] },
    });
    expect(projection.criticalPath).toEqual({
      availability: "partial",
      issueNumbers: [],
    });
  });

  it("clears retained PR-base payloads when the base family is non-complete", () => {
    for (const base of [partial(MAIN), unavailable(MAIN)] as const) {
      const projection = projectObservation({
        latest: recentSnapshot({
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
});

