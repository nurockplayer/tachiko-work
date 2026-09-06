export type Availability = "complete" | "partial" | "unavailable";

/**
 * Raw observation.
 * `complete`: the payload is the whole current set.
 * `partial`: discovery is incomplete; retained members may support only
 * monotone-positive evidence whose own identity/currentness is independently
 * established. Set membership, emptiness, and exhaustive conclusions remain
 * incomplete.
 * `unavailable`: retained payload is historical/stale input and must not be
 * consumed as current evidence.
 */
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
  head: Observed<string>;
  base: Observed<string>;
  linkedIssues: Observed<readonly number[]>;
}

export interface StewardWatchObservation {
  prNumber: number;
  availability: Availability;
  verdict: "GREEN" | "AMBER" | "HOLD";
  humanAction: "none" | "required";
  head: string;
  main: string;
  sourceUrl: string;
}

export interface RecentActivityObservation {
  number: number;
  title: string;
}

export interface ObservationSnapshot {
  main: Observed<string>;
  issues: Observed<readonly IssueObservation[]>;
  pullRequests: Observed<readonly PullObservation[]>;
  stewardWatches: Observed<readonly StewardWatchObservation[]>;
  recentActivity: Observed<readonly RecentActivityObservation[]>;
}

export interface ProjectObservationInput {
  latest: ObservationSnapshot;
  previous?: DashboardProjection | null;
}

/** Output current facts cannot retain a payload when the fact is non-complete. */
export type CurrentFact<T> =
  | { availability: "complete"; value: T }
  | { availability: "partial" | "unavailable"; value: null };

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
}

export type PositiveAttention =
  | {
      kind: "steward-hold";
      prNumber: number;
      sourceUrl: string;
    }
  | {
      kind: "human-action-required";
      prNumber: number;
      sourceUrl: string;
    };

/**
 * Current-work graph node ordering only. When complete, `issueNumbers` preserves
 * the latest complete Issue-observation order. Dependency edges live in each
 * delivery lane's `dependencies` fact; v0 does not compute or claim a longest,
 * prioritized, or merge-authoritative path.
 */
export type CriticalPathProjection =
  | {
      availability: "complete";
      issueNumbers: readonly number[];
    }
  | {
      availability: "partial" | "unavailable";
      issueNumbers: readonly [];
    };

export interface DashboardProjection {
  executive: {
    mainSha: CurrentFact<string>;
  };
  deliveries: readonly DeliveryProjection[];
  criticalPath: CriticalPathProjection;
  recentActivity: CurrentFact<readonly RecentActivityObservation[]>;
  attention: {
    items: readonly PositiveAttention[];
  };
}

function incompleteFact<T>(availability: Availability): CurrentFact<T> {
  return {
    availability: availability === "complete" ? "unavailable" : availability,
    value: null,
  };
}

function currentFact<T>(observation: Observed<T>): CurrentFact<T> {
  if (observation.availability === "complete" && observation.value !== null) {
    return { availability: "complete", value: observation.value };
  }

  return incompleteFact(observation.availability);
}

function completeValue<T>(observation: Observed<T>): T | null {
  return observation.availability === "complete" ? observation.value : null;
}

function criticalPath(
  issues: readonly IssueObservation[] | null,
  issueAvailability: Availability,
): CriticalPathProjection {
  if (issues === null) {
    return {
      availability:
        issueAvailability === "complete" ? "unavailable" : issueAvailability,
      issueNumbers: [],
    };
  }

  let incompleteAvailability: Exclude<Availability, "complete"> | null = null;
  for (const issue of issues) {
    if (issue.dependencies.availability !== "complete" || issue.dependencies.value === null) {
      if (
        issue.dependencies.availability === "unavailable" ||
        issue.dependencies.value === null
      ) {
        incompleteAvailability = "unavailable";
      } else if (incompleteAvailability === null) {
        incompleteAvailability = "partial";
      }
    }
  }

  if (incompleteAvailability !== null) {
    return { availability: incompleteAvailability, issueNumbers: [] };
  }

  return {
    availability: "complete",
    issueNumbers: issues.map((issue) => issue.number),
  };
}

/**
 * Projects only facts observed in the latest snapshot. `previous` is deliberately
 * ignored so a prior success cannot re-enter the current surface after failure.
 */
export function projectObservation({ latest }: ProjectObservationInput): DashboardProjection {
  const issues = completeValue(latest.issues);
  const pullRequests = completeValue(latest.pullRequests);
  const issueByNumber = new Map(issues?.map((issue) => [issue.number, issue]));
  const linkedIssueNumbers = new Set<number>();

  const issueFacts = (issueNumber: number | null) => {
    const issue = issueNumber === null ? undefined : issueByNumber.get(issueNumber);
    if (issue !== undefined && issues !== null) {
      return {
        issueTitle: { availability: "complete", value: issue.title } as CurrentFact<string>,
        issueState: { availability: "complete", value: issue.state } as CurrentFact<
          "OPEN" | "CLOSED"
        >,
        dependencies: currentFact(issue.dependencies),
      };
    }

    const availability =
      issues === null && latest.issues.availability !== "complete"
        ? latest.issues.availability
        : "unavailable";
    return {
      issueTitle: incompleteFact<string>(availability),
      issueState: incompleteFact<"OPEN" | "CLOSED">(availability),
      dependencies: incompleteFact<readonly number[]>(availability),
    };
  };

  const pullFacts = (pullRequest: PullObservation | null) => {
    if (pullRequest !== null) {
      return {
        pullRequestTitle: {
          availability: "complete",
          value: pullRequest.title,
        } as CurrentFact<string>,
        pullRequestState: {
          availability: "complete",
          value: pullRequest.state,
        } as CurrentFact<"OPEN" | "CLOSED" | "MERGED">,
        pullRequestDraft: {
          availability: "complete",
          value: pullRequest.draft,
        } as CurrentFact<boolean>,
        pullRequestHead: currentFact(pullRequest.head),
        pullRequestBase: currentFact(pullRequest.base),
        linkedIssues: currentFact(pullRequest.linkedIssues),
      };
    }

    const availability =
      pullRequests === null && latest.pullRequests.availability !== "complete"
        ? latest.pullRequests.availability
        : "unavailable";
    return {
      pullRequestTitle: incompleteFact<string>(availability),
      pullRequestState: incompleteFact<"OPEN" | "CLOSED" | "MERGED">(availability),
      pullRequestDraft: incompleteFact<boolean>(availability),
      pullRequestHead: incompleteFact<string>(availability),
      pullRequestBase: incompleteFact<string>(availability),
      linkedIssues: incompleteFact<readonly number[]>(availability),
    };
  };

  const deliveries: DeliveryProjection[] = [];
  if (pullRequests !== null) {
    for (const pullRequest of pullRequests) {
      const linkage = currentFact(pullRequest.linkedIssues);
      const issueNumbers =
        linkage.availability === "complete" ? linkage.value : [];

      if (issueNumbers.length === 0) {
        deliveries.push({
          issueNumber: null,
          ...issueFacts(null),
          pullRequestNumber: pullRequest.number,
          ...pullFacts(pullRequest),
        });
        continue;
      }

      for (const issueNumber of issueNumbers) {
        linkedIssueNumbers.add(issueNumber);
        deliveries.push({
          issueNumber,
          ...issueFacts(issueNumber),
          pullRequestNumber: pullRequest.number,
          ...pullFacts(pullRequest),
        });
      }
    }
  }

  if (issues !== null) {
    for (const issue of issues) {
      if (!linkedIssueNumbers.has(issue.number)) {
        deliveries.push({
          issueNumber: issue.number,
          ...issueFacts(issue.number),
          pullRequestNumber: null,
          ...pullFacts(null),
        });
      }
    }
  }

  const main = currentFact(latest.main);
  const observedPullRequests =
    latest.pullRequests.availability === "unavailable"
      ? []
      : latest.pullRequests.value ?? [];
  const watchedCandidates =
    latest.stewardWatches.availability === "unavailable"
      ? []
      : latest.stewardWatches.value ?? [];
  const attention: PositiveAttention[] = [];
  const attentionKeys = new Set<string>();

  for (const watch of watchedCandidates) {
    const pullRequest = observedPullRequests.find(
      (candidate) => candidate.number === watch.prNumber,
    );
    const isCurrent =
      watch.availability === "complete" &&
      main.availability === "complete" &&
      main.value === watch.main &&
      pullRequest?.head.availability === "complete" &&
      pullRequest.head.value !== null &&
      pullRequest.head.value === watch.head;

    if (!isCurrent) {
      continue;
    }

    const addAttention = (kind: PositiveAttention["kind"]) => {
      const key = `${kind}:${watch.prNumber}:${watch.sourceUrl}`;
      if (!attentionKeys.has(key)) {
        attentionKeys.add(key);
        attention.push({ kind, prNumber: watch.prNumber, sourceUrl: watch.sourceUrl });
      }
    };

    if (watch.verdict === "HOLD") {
      addAttention("steward-hold");
    }
    if (watch.humanAction === "required") {
      addAttention("human-action-required");
    }
  }

  return {
    executive: { mainSha: main },
    deliveries,
    criticalPath: criticalPath(issues, latest.issues.availability),
    recentActivity: currentFact(latest.recentActivity),
    attention: { items: attention },
  };
}
