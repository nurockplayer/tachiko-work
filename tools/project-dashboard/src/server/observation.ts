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
export function projectObservation(_input: ProjectObservationInput): DashboardProjection {
  throw new Error("Not implemented: #229 acceptance seed");
}
