export type Availability = "complete" | "partial" | "unavailable";

export interface SourceLink {
  label: string;
  url: string;
  kind: "github" | "repository" | "structured";
}

export interface ObservedValue<T> {
  value: T | null;
  availability: Availability;
  source: SourceLink;
  additionalSources?: SourceLink[];
}

export interface IssueFact {
  number: number;
  title: string;
  url: string;
  state: string;
  labels: string[];
  labelsAvailability: Availability;
  milestone: string | null;
  milestoneAvailability: Availability;
  blockedBy: { number: number; state: string; url: string }[];
  dependenciesAvailability: Availability;
  identityAvailability: Availability;
  availability: Availability;
}

export interface CheckFact {
  name: string;
  status: string | null;
  conclusion: string | null;
  url: string;
  headSha: string | null;
}

export interface ReviewFact {
  author: string;
  state: string;
  commitSha: string | null;
  exactHead: boolean | null;
  url: string;
}

export interface StructuredFact {
  status: "current" | "missing" | "unknown";
  value: string | null;
  reason: string;
  source: SourceLink | null;
}

export interface PullRequestFact {
  number: number;
  title: string;
  url: string;
  state: string;
  draft: boolean | null;
  headSha: string;
  baseSha: string;
  baseRef: string;
  mergeable: string | null;
  mergeStateStatus: string | null;
  reviewDecision: string | null;
  linkedIssueNumbers: number[];
  linkageAvailability: Availability;
  identityAvailability: Availability;
  nativeAvailability: Availability;
  checks: { availability: Availability; items: CheckFact[] };
  reviews: { availability: Availability; items: ReviewFact[] };
  handoff: StructuredFact;
  stewardWatch: StructuredFact;
  availability: Availability;
}

export interface DeliveryLane {
  issue: IssueFact | null;
  pullRequest: PullRequestFact | null;
  linkageAvailability: Availability;
}

export interface RecentActivity {
  number: number;
  title: string;
  url: string;
  mergedAt: string;
  mergeSha: string;
}

export interface AttentionItem {
  level: "attention" | "unknown" | "info";
  label: string;
  detail: string;
  sources: SourceLink[];
}

export interface DashboardProjection {
  repository: string;
  observedAt: string;
  fetchHealth: "healthy" | "partial" | "unavailable";
  executive: {
    mainSha: ObservedValue<string>;
    productHorizon: ObservedValue<string>;
    activeCount: ObservedValue<number>;
    readyCount: ObservedValue<number>;
    humanAction: ObservedValue<string>;
  };
  deliveries: DeliveryLane[];
  deliveriesAvailability: Availability;
  criticalPath: {
    availability: Availability;
    nodes: { issueNumber: number; label: string; state: string; url: string }[];
    edges: { from: number; to: number; state: string }[];
    source: SourceLink;
  };
  recentActivity: {
    availability: Availability;
    items: RecentActivity[];
    source: SourceLink;
  };
  attention: AttentionItem[];
  sources: SourceLink[];
}
