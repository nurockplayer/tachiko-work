import type {
  ConditionReason,
  GitHubAuthorAssociation,
  GitHubCommentKind,
} from "@tachiko-work/operational-evidence";

export type EvidenceClass = "direct" | "derived" | "advisory" | "historical";
export type DisplayState =
  | "satisfied"
  | "waiting"
  | "blocked"
  | "unknown"
  | "advisory";

export interface SourceLink {
  label: string;
  url: string;
  evidenceClass: EvidenceClass;
}

export interface DisplaySignal {
  state: DisplayState;
  reason: string;
  label: string;
  sources: SourceLink[];
}

export interface DisplayValue<T> {
  state: "satisfied" | "unknown";
  value: T;
  source: SourceLink;
}

export type ObservationAvailability = "complete" | "incomplete" | "unavailable";

export type FieldObservation<T> =
  | { state: "value"; value: T }
  | { state: "null" }
  | {
      state: "unknown";
      availability: Exclude<ObservationAvailability, "complete">;
      path: readonly (string | number)[] | null;
    };

export type ReviewDecision = "CHANGES_REQUESTED" | "APPROVED" | "REVIEW_REQUIRED";

export type MergeStateStatus =
  | "DIRTY"
  | "UNKNOWN"
  | "BLOCKED"
  | "BEHIND"
  | "UNSTABLE"
  | "HAS_HOOKS"
  | "CLEAN";

export type MergeableState = "MERGEABLE" | "CONFLICTING" | "UNKNOWN";

export type NativeMergePolicy =
  | { state: "blocked"; reason: "conflict" | "policy" }
  | { state: "unknown" }
  | { state: "waiting" }
  | { state: "satisfied" };

export type CheckResult = "success" | "pending" | "failure";

export interface RawSource {
  id: string;
  url: string;
}

export interface RawComment {
  body: string;
  id: string;
  kind: GitHubCommentKind;
  authorLogin: string;
  authorAssociation: GitHubAuthorAssociation;
  url: string;
  createdAt: string;
  updatedAt: string | null;
  lastEditedAt: FieldObservation<string>;
  topLevel: boolean;
  trustedProducer: boolean;
}

export interface RawCheck {
  name: string;
  headSha: FieldObservation<string>;
  result: FieldObservation<CheckResult>;
  url: string;
}

export interface RawReview {
  id: string;
  authorLogin: string;
  authorAssociation: GitHubAuthorAssociation;
  submittedAt: string | null;
  commitSha: string;
  state: "APPROVED" | "CHANGES_REQUESTED" | "COMMENTED" | "DISMISSED" | "PENDING";
  url: string;
}

export interface RawReviewThread {
  id: string;
  resolved: boolean;
  outdated: boolean;
  url: string;
}

export interface RawIssue {
  number: number;
  title: string;
  url: string;
  state: "OPEN" | "CLOSED";
  labels: string[];
  labelsAvailability: ObservationAvailability;
  milestone: FieldObservation<string>;
  blockedBy: { number: number; state: "OPEN" | "CLOSED"; url: string }[];
  dependencyAvailability: ObservationAvailability;
}

export interface RawPullRequest {
  number: number;
  title: string;
  url: string;
  state: "OPEN" | "CLOSED" | "MERGED";
  draft: boolean;
  headSha: string;
  baseSha: string;
  baseRef: string;
  mergeBaseSha: string | null;
  relationToMain: "current" | "behind" | "diverged" | "unknown";
  nativeMergePolicy: NativeMergePolicy;
  authorityChanges: { path: string; url: string }[];
  authorityAvailability: ObservationAvailability;
  closingIssueNumbers: number[];
  comments: RawComment[];
  commentsAvailability: ObservationAvailability;
  checks: RawCheck[];
  checksAvailability: ObservationAvailability;
  reviews: RawReview[];
  reviewsAvailability: ObservationAvailability;
  threads: RawReviewThread[];
  threadsAvailability: ObservationAvailability;
}

export interface RawRecentActivity {
  number: number;
  title: string;
  url: string;
  mergedAt: string;
  mergeSha: string;
}

export interface RepositoryObservation {
  repository: string;
  ownerToken: string;
  observedAt: string;
  availability: ObservationAvailability;
  main: { sha: string; url: string } | null;
  roadmap: { markdown: string; url: string } | null;
  issues: RawIssue[];
  issuesAvailability: ObservationAvailability;
  pullRequests: RawPullRequest[];
  pullsAvailability: ObservationAvailability;
  implementationLinkageAvailability: ObservationAvailability;
  recentActivity: RawRecentActivity[];
  recentActivityAvailability: ObservationAvailability;
  errors: {
    source: string;
    url: string;
    reason: string;
    path?: readonly (string | number)[];
  }[];
  /** Non-secret server-side credential-presence marker used by serialization tests. */
  serverCredential?: string;
}

export interface PullRequestProjection {
  number: number;
  title: string;
  url: string;
  headSha: string;
  baseSha: string;
  liveMainSha: string;
  mergeBaseSha: string | null;
  baseRef: string;
  relationToMain: RawPullRequest["relationToMain"];
  draft: boolean;
}

export interface DeliveryLane {
  issue: Pick<RawIssue, "number" | "title" | "url"> | null;
  owner: string;
  phase: string;
  pullRequest: PullRequestProjection | null;
  readiness: DisplaySignal;
  checks: DisplaySignal;
  review: DisplaySignal;
  handoff: DisplaySignal;
  stewardWatch: DisplaySignal;
  authority: DisplaySignal;
  humanAction: DisplaySignal;
  mergeGate: DisplaySignal;
  evidence: {
    automatedBrowser: DisplaySignal;
    perceptualReview: DisplaySignal;
    deliveryIntegrity: DisplaySignal;
  };
  sources: SourceLink[];
}

export interface AttentionItem extends DisplaySignal {
  issueNumber?: number;
}

export interface RepositoryProjection {
  repository: string;
  observedAt: string;
  fetchHealth: "healthy" | "partial" | "unavailable";
  executive: {
    mainSha: DisplayValue<string>;
    productHorizon: DisplayValue<string>;
    activeCount: DisplayValue<number | "Unknown">;
    readyCount: DisplayValue<number | "Unknown">;
  };
  deliveries: DeliveryLane[];
  criticalPath: {
    nodes: { issueNumber: number; label: string; state: string; url: string }[];
    edges: { from: number; to: number; state: "waiting" | "satisfied" }[];
  };
  recentActivity: RawRecentActivity[];
  attention: AttentionItem[];
  humanAction: DisplaySignal;
  sources: SourceLink[];
}

export const reasonLabels: Partial<Record<ConditionReason, string>> = {
  "handoff-current": "Current for exact head",
  "steward-watch-green": "Steward watch · GREEN",
  "steward-watch-amber": "Steward watch · AMBER",
  "steward-watch-hold": "Steward watch · HOLD",
  "human-action-none": "No human action required",
  "human-action-required": "Human action required",
  "native-check-succeeded": "Exact-head checks passed",
  "native-check-pending": "Exact-head checks pending",
  "native-check-failed": "Exact-head checks failed",
  "review-clean-current": "Exact-head review clean",
  "review-missing": "Exact-head review missing",
  "review-stale": "Review belongs to an older head",
  "native-thread-unknown": "Unresolved thread severity unknown",
  "main-mismatch": "Handoff does not match live main",
  "head-mismatch": "Handoff does not match PR head",
  "all-required-conditions-satisfied": "All declared conditions satisfied",
};
