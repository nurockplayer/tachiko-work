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

describe("#229 graph and activity acceptance", () => {

  it("projects the bounded five current surfaces on complete evidence", () => {
    const projection = projectObservation({ latest: recentSnapshot() });

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

  it("projects complete current-work graph nodes and dependency edges without inventing path semantics", () => {
    const projection = projectObservation({
      latest: recentSnapshot({
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

  it("keeps an unobserved dependency target as an edge rather than a graph node", () => {
    const projection = projectObservation({
      latest: recentSnapshot({
        issues: complete([issue(229, complete([230]))]),
        pullRequests: complete([pull({ linkedIssues: complete([229]) })]),
      }),
    });

    expect(projection.criticalPath).toEqual({
      availability: "complete",
      issueNumbers: [229],
    });
    expect(issueLane(projection, 229)?.dependencies).toEqual({
      availability: "complete",
      value: [230],
    });
    expect(issueLane(projection, 230)).toBeUndefined();
  });

  it("clears a retained partial recent-activity payload", () => {
    const projection = projectObservation({
      latest: recentSnapshot({
        recentActivity: partial([{ number: 320, title: "retained" }]),
      }),
    });

    expect(projection.recentActivity).toEqual({
      availability: "partial",
      value: null,
    });
  });

  it("makes critical path unavailable and empty when Issue discovery is unavailable even with retained payload", () => {
    const projection = projectObservation({
      latest: emptySnapshot({ issues: unavailable([issue()]) }),
    });

    expect(projection.criticalPath).toEqual({
      availability: "unavailable",
      issueNumbers: [],
    });
  });

  it("makes critical path unavailable and empty when a dependency family is unavailable with retained payload", () => {
    const projection = projectObservation({
      latest: emptySnapshot({
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
});

