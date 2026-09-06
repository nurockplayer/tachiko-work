import { describe, expect, it } from "vitest";

import {
  projectObservation,
  type ObservationSnapshot,
  type Observed,
  type StewardWatchObservation,
} from "../../src/server/observation.js";

const MAIN = "1".repeat(40);
const HEAD = "2".repeat(40);
const OTHER = "9".repeat(40);

function complete<T>(value: T): Observed<T> {
  return { availability: "complete", value };
}

function snapshot(watch: StewardWatchObservation): ObservationSnapshot {
  return {
    main: complete(MAIN),
    issues: complete([
      {
        number: 229,
        title: "Dashboard v0.1",
        state: "OPEN",
        dependencies: complete([]),
      },
    ]),
    pullRequests: complete([
      {
        number: 400,
        title: "Fresh Dashboard v0.1",
        state: "OPEN",
        draft: true,
        head: complete(HEAD),
        base: complete(MAIN),
        linkedIssues: complete([229, 304]),
      },
    ]),
    stewardWatches: complete([watch]),
    recentActivity: complete([]),
  };
}

function requiredHold(overrides: Partial<StewardWatchObservation>): StewardWatchObservation {
  return {
    prNumber: 400,
    availability: "complete",
    verdict: "HOLD",
    humanAction: "required",
    head: HEAD,
    main: MAIN,
    ...overrides,
  };
}

function assertStaleWatchDoesNotCreatePositiveAttention(
  projection: ReturnType<typeof projectObservation>,
) {
  const delivery = projection.deliveries.find(
    (candidate) => candidate.pullRequestNumber === 400,
  );
  expect(delivery?.stewardWatch).toEqual({
    status: "unknown",
    verdict: null,
    humanAction: null,
  });
  expect(projection.attention.hold.value).not.toBe(true);
  expect(projection.attention.humanAction.value).not.toBe("required");
}

describe("#229 acceptance: Steward watch exact identity", () => {
  it("rejects a fully observed HOLD/required watch for a different PR head", () => {
    const projection = projectObservation({
      latest: snapshot(requiredHold({ head: OTHER })),
    });

    assertStaleWatchDoesNotCreatePositiveAttention(projection);
  });

  it("rejects a fully observed HOLD/required watch for a different live main", () => {
    const projection = projectObservation({
      latest: snapshot(requiredHold({ main: OTHER })),
    });

    assertStaleWatchDoesNotCreatePositiveAttention(projection);
  });
});
