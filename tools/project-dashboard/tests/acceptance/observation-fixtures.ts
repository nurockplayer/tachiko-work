import type { DashboardProjection, IssueObservation, ObservationSnapshot, Observed, PullObservation, StewardWatchObservation } from "../../src/server/observation.js";

export const MAIN = "1".repeat(40);
export const HEAD = "2".repeat(40);
export const OTHER_HEAD = "3".repeat(40);
export const OTHER_MAIN = "4".repeat(40);
export const WATCH_URL = "https://github.example/pull/321#watch";

export function complete<T>(value: T): Observed<T> {
  return { availability: "complete", value };
}

export function partial<T>(value: T | null): Observed<T> {
  return { availability: "partial", value };
}

export function unavailable<T>(value: T | null): Observed<T> {
  return { availability: "unavailable", value };
}

export function issue(
  numberOrOverrides:
    | number
    | Observed<readonly number[]>
    | Partial<IssueObservation> = 229,
  dependencies: Observed<readonly number[]> = complete([]),
): IssueObservation {
  const defaults: IssueObservation = {
    number: 229,
    title: "Issue 229",
    state: "OPEN",
    dependencies: complete([]),
  };
  if (typeof numberOrOverrides === "number") {
    return {
      ...defaults,
      number: numberOrOverrides,
      title: `Issue ${numberOrOverrides}`,
      dependencies,
    };
  }
  if ("availability" in numberOrOverrides) {
    return { ...defaults, dependencies: numberOrOverrides };
  }
  return { ...defaults, ...numberOrOverrides };
}

export function pull(
  first: number | Observed<string> | Partial<PullObservation> = 321,
  linkedIssues: Observed<readonly number[]> = complete([229]),
  head: Observed<string> = complete(HEAD),
): PullObservation {
  const defaults: PullObservation = {
    number: 321,
    title: "PR 321",
    state: "OPEN",
    draft: true,
    head: complete(HEAD),
    base: complete(MAIN),
    linkedIssues: complete([229]),
  };
  if (typeof first === "number") {
    return {
      ...defaults,
      number: first,
      title: `PR ${first}`,
      linkedIssues,
      head,
    };
  }
  if ("availability" in first) {
    return { ...defaults, head: first };
  }
  return { ...defaults, ...first };
}

export function watch(
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

export function snapshot(
  overrides: Partial<ObservationSnapshot> = {},
  recentActivity: readonly { number: number; title: string }[] = [],
): ObservationSnapshot {
  return {
    main: complete(MAIN),
    issues: complete([issue()]),
    pullRequests: complete([pull()]),
    stewardWatches: complete([watch()]),
    recentActivity: complete(recentActivity),
    ...overrides,
  };
}

export function emptySnapshot(
  overrides: Partial<ObservationSnapshot> = {},
): ObservationSnapshot {
  return snapshot(overrides);
}

export function recentSnapshot(
  overrides: Partial<ObservationSnapshot> = {},
): ObservationSnapshot {
  return snapshot(overrides, [{ number: 320, title: "Recent merge" }]);
}

export function prLane(projection: DashboardProjection, number = 321) {
  return projection.deliveries.find((lane) => lane.pullRequestNumber === number);
}

export function issueOnlyLane(projection: DashboardProjection, number = 229) {
  return projection.deliveries.find(
    (lane) => lane.issueNumber === number && lane.pullRequestNumber === null,
  );
}

export function issueLane(projection: DashboardProjection, number: number) {
  return projection.deliveries.find((lane) => lane.issueNumber === number);
}
