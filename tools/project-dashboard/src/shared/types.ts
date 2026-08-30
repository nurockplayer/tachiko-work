export type SourceClass = "direct" | "derived" | "heuristic" | "historical";

export interface SourceRef {
  class: SourceClass;
  label: string;
  url: string;
  observedAt: string;
  observedIdentity: string | null;
}

export type FetchHealth = "healthy" | "partial" | "unavailable";
export type DeliveryPhase =
  | "ready"
  | "implementing"
  | "validating"
  | "review_fix"
  | "rereview"
  | "merge_gate"
  | "human_required"
  | "blocked"
  | "parked"
  | "completed"
  | "unknown";

export interface RawComment {
  id: string;
  body: string;
  url: string;
  authorAssociation: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface RawIssue {
  number: number;
  title: string;
  url: string;
  body: string;
  authorAssociation: string | null;
  updatedAt: string;
  lastEditedAt: string | null;
  milestone: string | null;
  blockedBy: Array<{ number: number; title: string; url: string }> | null;
  commentsComplete: boolean;
  comments: RawComment[];
}

export interface RawCheck {
  name: string;
  integrationId: number | null;
  attemptAt: string | null;
  status: "queued" | "in_progress" | "completed" | "unknown";
  conclusion: "success" | "failure" | "cancelled" | "neutral" | "skipped" | "stale" | null;
  url: string | null;
}

export interface RawRequiredCheck {
  name: string;
  integrationId: number | null;
}

export interface RawReview {
  state: "approved" | "changes_requested" | "commented" | "dismissed" | "pending" | "unknown";
  author: string | null;
  body: string;
  headSha: string | null;
  url: string;
  submittedAt: string;
}

export interface RawReviewThread {
  resolved: boolean;
  outdated: boolean;
  comments: string[];
  url: string;
}

export interface RawPullRequest {
  number: number;
  title: string;
  url: string;
  body: string;
  author: { login: string; type: "user" | "bot" | "organization" | "unknown" } | null;
  isDraft: boolean;
  headSha: string;
  baseRefName: string;
  baseSha: string | null;
  mergeBaseSha: string | null;
  relationToMain: "current" | "behind" | "diverged" | "unknown";
  changedPaths: string[] | null;
  authorityPathsChangedOnMain: string[] | null;
  mergeable: "mergeable" | "conflicting" | "unknown";
  mergeStateStatus: "clean" | "blocked" | "behind" | "dirty" | "draft" | "unstable" | "unknown";
  issueNumbers: number[];
  issueNumbersComplete: boolean;
  commentsComplete: boolean;
  comments: RawComment[];
  checksObservedHeadSha: string | null;
  checks: RawCheck[] | null;
  requiredChecks: RawRequiredCheck[] | null;
  reviewDecision: "approved" | "changes_requested" | "review_required" | "unknown";
  reviews: RawReview[] | null;
  reviewThreads: RawReviewThread[] | null;
  updatedAt: string;
}

export interface RawCompletion {
  number: number;
  title: string;
  url: string;
  mergedAt: string;
  mergeSha: string | null;
  mergedBy: string | null;
}

export interface RawRepositorySnapshot {
  repoName: string;
  repoUrl: string;
  observedAt: string;
  mainSha: string | null;
  defaultBranchName: string | null;
  productHorizon: string | null;
  productHorizonUrl: string;
  fetchHealth: FetchHealth;
  failures: string[];
  issues: RawIssue[] | null;
  pullRequests: RawPullRequest[] | null;
  recentCompletions: RawCompletion[] | null;
}

export interface HandoffProjection {
  condition: "current" | "stale" | "inconsistent" | "missing" | "unknown";
  claimedOwner: string | null;
  claimedState: string | null;
  claimedIssueNumber: number | null;
  observedIssueNumbers: number[];
  claimedHeadSha: string | null;
  lastCheckedMainSha: string | null;
  updatedAt: string | null;
  sourceRefs: SourceRef[];
}

export interface CheckProjection {
  status: "success" | "failure" | "pending" | "unknown";
  requiredStatus: "satisfied" | "unsatisfied" | "unknown";
  observedHeadSha: string | null;
  summary: string;
  requiredSummary: string;
  sourceRefs: SourceRef[];
}

export interface ReviewProjection {
  decision: RawPullRequest["reviewDecision"];
  status: "current" | "stale" | "unknown";
  reviewedHeadSha: string | null;
  unresolvedThreadCount: number | null;
  substantiveUnresolvedCount: number | null;
  sourceRefs: SourceRef[];
}

export interface DeliveryLane {
  id: string;
  issue: {
    number: number;
    title: string;
    url: string;
    readiness: "ready" | "active" | "blocked" | "parked" | "unknown";
    milestone: string | null;
    lastEditedAt: string | null;
    blockedBy: Array<{ number: number; title: string; url: string }> | null;
  };
  owner: string;
  phase: DeliveryPhase;
  pr: null | {
    number: number;
    title: string;
    url: string;
    isDraft: boolean;
    headSha: string;
    baseRefName: string;
    baseSha: string | null;
    mergeBaseSha: string | null;
    liveMainSha: string | null;
    relationToMain: RawPullRequest["relationToMain"];
    authorityPathsChangedOnMain: string[] | null;
    mergeable: RawPullRequest["mergeable"];
    mergeStateStatus: RawPullRequest["mergeStateStatus"];
  };
  checks: CheckProjection;
  reviews: ReviewProjection;
  handoff: HandoffProjection;
  authorityDrift: "none" | "suspected" | "confirmed" | "unknown";
  blockers: string[];
  action: {
    owner: "none" | "codex" | "chatgpt" | "agent" | "human" | "unknown";
    reason: string;
  };
  sourceRefs: SourceRef[];
}

export interface RepositoryProjection {
  repo: {
    name: string;
    observedAt: string;
    fetchHealth: FetchHealth;
    failures: string[];
    mainSha: string | null;
    defaultBranchName: string | null;
    productHorizon: string | null;
    sourceRefs: SourceRef[];
  };
  deliveries: DeliveryLane[];
  currentWork: {
    currentHorizon: DeliveryLane["id"][];
    independent: DeliveryLane["id"][];
    otherHorizon: DeliveryLane["id"][];
    unclassified: DeliveryLane["id"][];
    horizonStatus: "current" | "unknown";
    dependencyHealth: "healthy" | "partial" | "unknown";
    sourceRefs: SourceRef[];
  };
  recentCompletions: Array<RawCompletion & { sourceRefs: SourceRef[] }>;
  attention: {
    humanActionRequired: boolean | null;
    reasons: string[];
    sourceRefs: SourceRef[];
  };
}
