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

/** Steward acceptance seam. Delivery implementation must satisfy the acceptance suite. */
export function projectObservation(_input: ProjectObservationInput): DashboardProjection {
  throw new Error("Not implemented: #229 positive-only acceptance seed");
}
