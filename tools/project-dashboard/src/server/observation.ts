export type Availability = "complete" | "partial" | "unavailable";

export interface Observed<T> {
  availability: Availability;
  value: T | null;
}

export interface IssueObservation {
  number: number;
  title: string;
  state: "OPEN" | "CLOSED";
  dependencies: Observed<readonly number[]>;
}

export interface PullObservation {
  number: number;
  title: string;
  state: "OPEN" | "CLOSED" | "MERGED";
  draft: boolean;
  head: Observed<string | null>;
  base: Observed<string | null>;
  linkedIssues: Observed<readonly number[]>;
}

export interface StewardWatchObservation {
  prNumber: number;
  availability: Availability;
  verdict: "GREEN" | "AMBER" | "HOLD";
  humanAction: "none" | "required";
  head: string;
  main: string;
}

export interface RecentActivityObservation {
  number: number;
  title: string;
}

export interface ObservationSnapshot {
  main: Observed<string | null>;
  issues: Observed<readonly IssueObservation[]>;
  pullRequests: Observed<readonly PullObservation[]>;
  stewardWatches: Observed<readonly StewardWatchObservation[]>;
  recentActivity: Observed<readonly RecentActivityObservation[]>;
}

export interface ProjectObservationInput {
  latest: ObservationSnapshot;
  previous?: DashboardProjection | null;
}

export interface CurrentFact<T> {
  availability: Availability;
  value: T | null;
}

export interface WatchProjection {
  status: "current" | "unknown";
  verdict: "GREEN" | "AMBER" | "HOLD" | null;
  humanAction: "none" | "required" | null;
}

export interface DeliveryProjection {
  issueNumber: number | null;
  issueTitle: CurrentFact<string>;
  issueState: CurrentFact<"OPEN" | "CLOSED">;
  dependencies: CurrentFact<readonly number[]>;
  pullRequestNumber: number | null;
  pullRequestTitle: CurrentFact<string>;
  pullRequestState: CurrentFact<"OPEN" | "CLOSED" | "MERGED">;
  pullRequestDraft: CurrentFact<boolean>;
  pullRequestHead: CurrentFact<string>;
  pullRequestBase: CurrentFact<string>;
  linkedIssues: CurrentFact<readonly number[]>;
  stewardWatch: WatchProjection;
}

export interface DashboardProjection {
  executive: {
    mainSha: CurrentFact<string>;
  };
  deliveries: readonly DeliveryProjection[];
  criticalPath: {
    availability: Availability;
    issueNumbers: readonly number[];
  };
  recentActivity: CurrentFact<readonly RecentActivityObservation[]>;
  attention: {
    hold: CurrentFact<boolean>;
    humanAction: CurrentFact<"required">;
  };
}

/**
 * Acceptance-seed seam only. Production implementation belongs to the delivery
 * agent. The Steward acceptance tests define behavior; do not weaken them by
 * teaching this function old PR #188/#230 parser semantics.
 */
function currentFact<T>(observed: Observed<T>): CurrentFact<T> {
  return {
    availability: observed.availability,
    value: observed.availability === "unavailable" ? null : observed.value,
  };
}

function currentStringFact(observed: Observed<string | null>): CurrentFact<string> {
  return {
    availability: observed.availability,
    value: typeof observed.value === "string" ? observed.value : null,
  };
}

function knownFact<T>(value: T): CurrentFact<T> {
  return { availability: "complete", value };
}

function unknownFact<T>(availability: Availability = "unavailable"): CurrentFact<T> {
  return { availability, value: null };
}

function unknownWatch(): WatchProjection {
  return { status: "unknown", verdict: null, humanAction: null };
}

function projectStewardWatch(
  pullRequest: PullObservation,
  main: Observed<string | null>,
  watches: Observed<readonly StewardWatchObservation[]>,
): WatchProjection {
  const head = pullRequest.head;
  const headValue = typeof head.value === "string" ? head.value : null;
  const mainValue = typeof main.value === "string" ? main.value : null;
  if (
    head.availability !== "complete" ||
    headValue === null ||
    main.availability !== "complete" ||
    mainValue === null ||
    watches.value === null
  ) {
    return unknownWatch();
  }

  const watch = watches.value.find(
    (candidate) =>
      candidate.prNumber === pullRequest.number &&
      candidate.availability === "complete" &&
      candidate.head === headValue &&
      candidate.main === mainValue,
  );
  return watch === undefined
    ? unknownWatch()
    : {
        status: "current",
        verdict: watch.verdict,
        humanAction: watch.humanAction,
      };
}

function combinedAvailability(values: readonly Availability[]): Availability {
  if (values.includes("unavailable")) return "unavailable";
  if (values.includes("partial")) return "partial";
  return "complete";
}

function unresolvedAttentionAvailability(
  latest: ObservationSnapshot,
): Availability {
  return combinedAvailability([
    latest.main.availability,
    latest.pullRequests.availability,
    latest.stewardWatches.availability,
  ]);
}

export function projectObservation({ latest }: ProjectObservationInput): DashboardProjection {
  const issues = latest.issues.value ?? [];
  const pullRequests = latest.pullRequests.value ?? [];
  const issuesByNumber = new Map(issues.map((issue) => [issue.number, issue]));
  const usedIssueNumbers = new Set<number>();
  const deliveries: DeliveryProjection[] = [];
  const watchesByPull = new Map<number, WatchProjection>();

  for (const pullRequest of pullRequests) {
    const stewardWatch = projectStewardWatch(
      pullRequest,
      latest.main,
      latest.stewardWatches,
    );
    watchesByPull.set(pullRequest.number, stewardWatch);

    const linkedIssueNumbers = pullRequest.linkedIssues.value ?? [];
    const laneIssueNumbers: readonly (number | null)[] =
      linkedIssueNumbers.length > 0
        ? linkedIssueNumbers
        : issues.length === 1
          ? [issues[0].number]
          : [null];

    for (const issueNumber of laneIssueNumbers) {
      const issue = issueNumber === null ? undefined : issuesByNumber.get(issueNumber);
      if (issue !== undefined) usedIssueNumbers.add(issue.number);

      const issueAvailability =
        latest.issues.availability === "complete" ? "partial" : latest.issues.availability;
      deliveries.push({
        issueNumber,
        issueTitle: issue === undefined ? unknownFact(issueAvailability) : knownFact(issue.title),
        issueState: issue === undefined ? unknownFact(issueAvailability) : knownFact(issue.state),
        dependencies:
          issue === undefined ? unknownFact(issueAvailability) : currentFact(issue.dependencies),
        pullRequestNumber: pullRequest.number,
        pullRequestTitle: knownFact(pullRequest.title),
        pullRequestState: knownFact(pullRequest.state),
        pullRequestDraft: knownFact(pullRequest.draft),
        pullRequestHead: currentStringFact(pullRequest.head),
        pullRequestBase: currentStringFact(pullRequest.base),
        linkedIssues: currentFact(pullRequest.linkedIssues),
        stewardWatch,
      });
    }
  }

  for (const issue of issues) {
    if (usedIssueNumbers.has(issue.number)) continue;
    deliveries.push({
      issueNumber: issue.number,
      issueTitle: knownFact(issue.title),
      issueState: knownFact(issue.state),
      dependencies: currentFact(issue.dependencies),
      pullRequestNumber: null,
      pullRequestTitle: unknownFact(),
      pullRequestState: unknownFact(),
      pullRequestDraft: unknownFact(),
      pullRequestHead: unknownFact(),
      pullRequestBase: unknownFact(),
      linkedIssues: unknownFact(),
      stewardWatch: unknownWatch(),
    });
  }

  const watchProjections = [...watchesByPull.values()];
  const holdObserved = watchProjections.some(
    (watch) => watch.status === "current" && watch.verdict === "HOLD",
  );
  const humanActionObserved = watchProjections.some(
    (watch) => watch.status === "current" && watch.humanAction === "required",
  );
  const allRelevantWatchesCurrent =
    pullRequests.length > 0 &&
    latest.pullRequests.availability === "complete" &&
    latest.stewardWatches.availability === "complete" &&
    watchProjections.length === pullRequests.length &&
    watchProjections.every((watch) => watch.status === "current");
  const attentionAvailability = unresolvedAttentionAvailability(latest);

  return {
    executive: {
      mainSha: currentStringFact(latest.main),
    },
    deliveries,
    criticalPath: {
      availability: combinedAvailability([
        latest.issues.availability,
        ...issues.map((issue) => issue.dependencies.availability),
      ]),
      issueNumbers: issues.map((issue) => issue.number),
    },
    recentActivity: currentFact(latest.recentActivity),
    attention: {
      hold: holdObserved
        ? { availability: "complete", value: true }
        : allRelevantWatchesCurrent
          ? { availability: "complete", value: false }
          : { availability: attentionAvailability, value: null },
      humanAction: humanActionObserved
        ? { availability: "complete", value: "required" }
        : allRelevantWatchesCurrent
          ? { availability: "complete", value: null }
          : { availability: attentionAvailability, value: null },
    },
  };
}
