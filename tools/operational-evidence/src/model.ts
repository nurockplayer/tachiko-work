/** GitHub's finite author-association vocabulary. */
export type GitHubAuthorAssociation =
  | "COLLABORATOR"
  | "CONTRIBUTOR"
  | "FIRST_TIMER"
  | "FIRST_TIME_CONTRIBUTOR"
  | "MANNEQUIN"
  | "MEMBER"
  | "NONE"
  | "OWNER";

export type GitHubCommentKind =
  | "issue-comment"
  | "pull-request-review-comment"
  | "pull-request-review";

/**
 * Metadata obtained from GitHub and trust policy, never from the comment body.
 * `trustedProducer` is the caller's repository-policy decision.
 */
export interface CommentSourceMetadata {
  readonly repository: string;
  readonly id: string;
  readonly kind: GitHubCommentKind;
  readonly authorLogin: string;
  readonly authorAssociation: GitHubAuthorAssociation;
  readonly url: string;
  readonly createdAt: string;
  readonly updatedAt: string | null;
  readonly edited: boolean;
  readonly topLevel: boolean;
  readonly trustedProducer: boolean;
}

export interface StructuredCommentSource {
  readonly body: string;
  readonly metadata: CommentSourceMetadata;
}

/** Exact live identities against which a candidate comment is parsed. */
export interface EvidenceParseContext {
  readonly repository: string;
  readonly issueNumber: number;
  readonly pullRequestNumber: number;
  readonly owner: string;
  readonly headSha: string;
  readonly mainSha: string;
}

/** Stable provenance carried by every parsed value and parse failure. */
export interface SourceRef {
  readonly repository: string;
  readonly sourceId: string;
  readonly sourceKind: GitHubCommentKind;
  readonly url: string;
  readonly authorLogin: string;
  readonly authorAssociation: GitHubAuthorAssociation;
  readonly createdAt: string;
  readonly updatedAt: string | null;
  readonly observedIssue: number;
  readonly observedPullRequest: number;
  readonly observedHead: string;
  readonly observedMain: string;
}

export type ParseField =
  | "ISSUE"
  | "PR"
  | "OWNER"
  | "STATE"
  | "HEAD"
  | "MAIN"
  | "VERDICT"
  | "HUMAN_ACTION"
  | "KIND"
  | "RUN"
  | "NAME"
  | "RESULT"
  | "SUPERSEDES"
  | "SEVERITY"
  | "RESOLVES";

/** Finite machine reasons; prose diagnostics are deliberately not authority. */
export type ParseFailureReason =
  | "context-invalid"
  | "marker-missing"
  | "marker-not-first-line"
  | "marker-duplicate"
  | "source-not-top-level"
  | "source-edited"
  | "producer-untrusted"
  | "envelope-unclosed"
  | "header-empty"
  | "malformed-line"
  | "unknown-field"
  | "duplicate-field"
  | "missing-field"
  | "invalid-positive-integer"
  | "invalid-canonical-token"
  | "invalid-reference-token"
  | "invalid-sha"
  | "invalid-enum"
  | "incompatible-result"
  | "identity-mismatch"
  | "head-mismatch"
  | "main-mismatch"
  | "multiple-envelopes";

export interface ParseSuccess<T> {
  readonly ok: true;
  readonly value: T;
  readonly source: SourceRef;
}

export interface ParseFailure {
  readonly ok: false;
  readonly reason: ParseFailureReason;
  readonly field?: ParseField;
  readonly source: SourceRef;
}

export type ParseResult<T> = ParseSuccess<T> | ParseFailure;

export interface AgentHandoff {
  readonly kind: "agent-handoff";
  readonly issue: number;
  readonly pullRequest: number;
  readonly owner: string;
  readonly state: string;
  readonly head: string;
  readonly main: string;
  readonly source: SourceRef;
}

export type StewardVerdict = "GREEN" | "AMBER" | "HOLD";
export type HumanAction = "none" | "required";

export interface StewardWatch {
  readonly kind: "project-steward-watch";
  readonly verdict: StewardVerdict;
  readonly head: string;
  readonly main: string;
  readonly humanAction: HumanAction;
  readonly source: SourceRef;
}

interface OperationalEvidenceBase {
  readonly pullRequest: number;
  readonly head: string;
  readonly source: SourceRef;
}

export interface ValidationEvidence extends OperationalEvidenceBase {
  readonly kind: "validation";
  readonly run: string;
  readonly name: string;
  readonly result: "pass" | "fail" | "unknown";
  readonly supersedes?: string;
}

export interface ReviewEvidence extends OperationalEvidenceBase {
  readonly kind: "review";
  readonly run: string;
  readonly name: string;
  readonly result: "clean" | "findings" | "unknown";
  readonly supersedes?: string;
}

export type ReviewSeverity = "P0" | "P1" | "P2" | "P3";

export interface ReviewFindingEvidence extends OperationalEvidenceBase {
  readonly kind: "review-finding";
  readonly run: string;
  readonly severity: ReviewSeverity;
}

export interface ReviewResolutionEvidence extends OperationalEvidenceBase {
  readonly kind: "review-resolution";
  readonly resolves: string;
}

export type OperationalEvidence =
  | ValidationEvidence
  | ReviewEvidence
  | ReviewFindingEvidence
  | ReviewResolutionEvidence;
