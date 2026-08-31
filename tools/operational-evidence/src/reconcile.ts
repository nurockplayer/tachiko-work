import type {
  EvidenceParseContext,
  OperationalEvidence,
  ParseFailureReason,
  ReviewEvidence,
  ReviewFindingEvidence,
  ReviewResolutionEvidence,
  ReviewSeverity,
  SourceRef,
  StructuredCommentSource,
  ValidationEvidence,
} from "./model.js";
import {
  parseAgentHandoff,
  parseOperationalEvidence,
  parseStewardWatch,
} from "./parse.js";

export type ConditionState = "satisfied" | "waiting" | "blocked" | "unknown";

export type ConditionReason =
  | ParseFailureReason
  | "trusted-source-missing"
  | "trusted-source-ambiguous"
  | "handoff-current"
  | "watch-not-required"
  | "steward-watch-green"
  | "steward-watch-amber"
  | "steward-watch-hold"
  | "human-action-none"
  | "human-action-required"
  | "native-check-succeeded"
  | "native-check-pending"
  | "native-check-failed"
  | "validation-passed"
  | "validation-failed"
  | "validation-unknown"
  | "validation-stale"
  | "validation-missing"
  | "evidence-conflict"
  | "reference-missing"
  | "reference-mismatch"
  | "reference-cycle"
  | "review-run-missing"
  | "native-approval-current"
  | "native-approval-stale"
  | "native-changes-requested"
  | "native-review-pending"
  | "native-thread-blocking"
  | "native-thread-unknown"
  | "review-clean-current"
  | "review-findings-current"
  | "review-findings-unclassified"
  | "review-unknown"
  | "review-stale"
  | "review-missing"
  | "review-finding-blocking"
  | "source-identity-conflict"
  | "observation-incomplete"
  | "observation-unavailable"
  | "not-required"
  | "explicit-blocker"
  | "required-evidence-unknown"
  | "native-evidence-waiting"
  | "all-required-conditions-satisfied";

export interface NativeSourceRef {
  readonly id: string;
  readonly url?: string;
}

export type Provenance =
  | { readonly kind: "authority"; readonly id: string }
  | { readonly kind: "comment"; readonly source: SourceRef }
  | {
      readonly kind:
        | "native-check"
        | "native-review"
        | "native-thread"
        | "native-observation"
        | "cache"
        | "prose";
      readonly id: string;
      readonly url?: string;
    };

export interface Condition {
  readonly state: ConditionState;
  readonly reason: ConditionReason;
  readonly provenance: readonly Provenance[];
  readonly observedHead: string;
  readonly observedMain: string;
}

export type Observation<T> =
  | {
      readonly availability: "complete";
      readonly facts: readonly T[];
      readonly source: NativeSourceRef;
    }
  | {
      readonly availability: "incomplete";
      readonly facts: readonly T[];
      readonly source: NativeSourceRef;
    }
  | {
      readonly availability: "unavailable";
      readonly facts?: readonly T[];
      readonly source: NativeSourceRef;
    };

export interface NativeCheck {
  readonly name: string;
  readonly head: string;
  readonly status: "success" | "pending" | "failure";
  readonly source: NativeSourceRef;
}

export interface NativeReview {
  /** GitHub/repository policy's current disposition, separate from commit identity. */
  readonly current: boolean;
  readonly head: string;
  readonly state:
    | "APPROVED"
    | "CHANGES_REQUESTED"
    | "COMMENTED"
    | "DISMISSED"
    | "PENDING";
  readonly source: NativeSourceRef;
}

export interface NativeReviewThread {
  readonly resolved: boolean;
  readonly outdated: boolean;
  readonly severity: ReviewSeverity | "unknown";
  readonly source: NativeSourceRef;
}

export interface ReconcileRequirements {
  readonly requiredValidations: readonly string[];
  /** A canonical structured review NAME, or null when no exact-head review is required. */
  readonly requiredReview: string | null;
  readonly currentStewardWatch: boolean;
}

export interface AdvisoryInput {
  readonly kind: "prose" | "cache";
  readonly source: NativeSourceRef;
}

export interface ReconcileInput {
  readonly context: EvidenceParseContext;
  readonly comments: readonly StructuredCommentSource[];
  readonly nativeChecks: Observation<NativeCheck>;
  readonly nativeReviews: Observation<NativeReview>;
  readonly nativeThreads: Observation<NativeReviewThread>;
  readonly requirements: ReconcileRequirements;
  readonly advisories?: readonly AdvisoryInput[];
}

export type AdvisoryReason =
  | "unstructured-prose"
  | "cached-observation-stale"
  | "untrusted-structured-source"
  | "invalid-structured-source"
  | "stale-structured-evidence"
  | "stale-native-evidence"
  | "native-thread-p3"
  | "review-finding-p3"
  | "custom-evidence-shadowed";

export interface Advisory {
  readonly reason: AdvisoryReason;
  readonly provenance: readonly Provenance[];
  readonly detail?: ConditionReason;
}

export interface ValidationCondition extends Condition {
  readonly name: string;
}

export interface Reconciliation {
  readonly handoff: Condition;
  readonly watch: Condition;
  readonly validations: readonly ValidationCondition[];
  readonly review: Condition;
  readonly humanAction: Condition;
  readonly mutationGate: Condition;
  readonly mergeGate: Condition;
  readonly advisories: readonly Advisory[];
}

interface EvidenceSet {
  readonly values: readonly OperationalEvidence[];
  readonly relationshipFailures: ReadonlyMap<string, ConditionReason>;
  readonly activeRunSourceIds: ReadonlySet<string>;
  readonly sourceConflicts: ReadonlySet<string>;
  readonly advisories: readonly Advisory[];
}

const HANDOFF_MARKER = "<!-- agent-handoff:v1 -->";
const WATCH_MARKER = "<!-- project-steward-watch:v1 -->";
const EVIDENCE_MARKER = "<!-- operational-evidence:v1";

function nativeProvenance(
  kind: Exclude<Provenance["kind"], "authority" | "comment">,
  source: NativeSourceRef,
): Provenance {
  return source.url === undefined
    ? { kind, id: source.id }
    : { kind, id: source.id, url: source.url };
}

function commentProvenance(source: SourceRef): Provenance {
  return { kind: "comment", source };
}

function provenanceKey(value: Provenance): string {
  if (value.kind === "comment") return `comment:${value.source.sourceId}`;
  return `${value.kind}:${value.id}`;
}

function stableProvenance(values: readonly Provenance[]): readonly Provenance[] {
  const byKey = new Map<string, Provenance>();
  for (const value of values) byKey.set(provenanceKey(value), value);
  return [...byKey.values()].sort((left, right) =>
    provenanceKey(left).localeCompare(provenanceKey(right)),
  );
}

function condition(
  state: ConditionState,
  reason: ConditionReason,
  provenance: readonly Provenance[],
): Condition {
  return {
    state,
    reason,
    provenance: stableProvenance(provenance),
    observedHead: "",
    observedMain: "",
  };
}

function sourceFingerprint(source: StructuredCommentSource): string {
  const metadata = source.metadata;
  return [
    metadata.id,
    source.body,
    metadata.repository,
    metadata.kind,
    metadata.authorLogin,
    metadata.authorAssociation,
    metadata.url,
    metadata.createdAt,
    metadata.updatedAt ?? "",
    String(metadata.edited),
    String(metadata.topLevel),
    String(metadata.trustedProducer),
  ].join("\u0000");
}

interface StableComments {
  readonly values: readonly StructuredCommentSource[];
  readonly conflicts: ReadonlySet<string>;
}

function stableComments(
  comments: readonly StructuredCommentSource[],
): StableComments {
  const sorted = [...comments].sort((left, right) =>
    sourceFingerprint(left).localeCompare(sourceFingerprint(right)),
  );
  const seenFingerprints = new Set<string>();
  const fingerprintsById = new Map<string, Set<string>>();
  const values = sorted.filter((source) => {
    const fingerprint = sourceFingerprint(source);
    const sourceFingerprints = fingerprintsById.get(source.metadata.id) ?? new Set();
    sourceFingerprints.add(fingerprint);
    fingerprintsById.set(source.metadata.id, sourceFingerprints);
    if (seenFingerprints.has(fingerprint)) return false;
    seenFingerprints.add(fingerprint);
    return true;
  });
  return {
    values,
    conflicts: new Set(
      [...fingerprintsById]
        .filter(([, fingerprints]) => fingerprints.size > 1)
        .map(([sourceId]) => sourceId),
    ),
  };
}

function exactMarkerPresent(body: string, marker: string): boolean {
  return body.replaceAll("\r\n", "\n").split("\n").includes(marker);
}

function selectSingleton<T>(
  comments: readonly StructuredCommentSource[],
  sourceConflicts: ReadonlySet<string>,
  marker: string,
  parse: (
    source: StructuredCommentSource,
    context: EvidenceParseContext,
  ) =>
    | { readonly ok: true; readonly value: T; readonly source: SourceRef }
    | {
        readonly ok: false;
        readonly reason: ParseFailureReason;
        readonly source: SourceRef;
      },
  context: EvidenceParseContext,
): { readonly condition: Condition; readonly value?: T } {
  const candidates = comments.filter(
    (source) =>
      source.metadata.topLevel && exactMarkerPresent(source.body, marker),
  );
  const trusted = candidates.filter(
    (source) => source.metadata.trustedProducer,
  );
  if (trusted.length > 1) {
    return {
      condition: condition(
        "unknown",
        "trusted-source-ambiguous",
        trusted.map((source) => ({
          kind: "comment" as const,
          source: {
            repository: source.metadata.repository,
            sourceId: source.metadata.id,
            sourceKind: source.metadata.kind,
            url: source.metadata.url,
            authorLogin: source.metadata.authorLogin,
            authorAssociation: source.metadata.authorAssociation,
            createdAt: source.metadata.createdAt,
            updatedAt: source.metadata.updatedAt,
            observedIssue: context.issueNumber,
            observedPullRequest: context.pullRequestNumber,
            observedHead: context.headSha,
            observedMain: context.mainSha,
          },
        })),
      ),
    };
  }
  const selected = trusted[0];
  if (selected === undefined) {
    const untrusted = candidates[0];
    if (untrusted !== undefined) {
      const parsed = parse(untrusted, context);
      return {
        condition: condition("unknown", "producer-untrusted", [
          commentProvenance(parsed.source),
        ]),
      };
    }
    return {
      condition: condition("unknown", "trusted-source-missing", [
        { kind: "authority", id: "structured-source-required" },
      ]),
    };
  }
  if (sourceConflicts.has(selected.metadata.id)) {
    const parsed = parse(selected, context);
    return {
      condition: condition("unknown", "source-identity-conflict", [
        commentProvenance(parsed.source),
      ]),
    };
  }
  const parsed = parse(selected, context);
  if (!parsed.ok) {
    return {
      condition: condition("unknown", parsed.reason, [
        commentProvenance(parsed.source),
      ]),
    };
  }
  return {
    condition: condition("satisfied", "handoff-current", [
      commentProvenance(parsed.source),
    ]),
    value: parsed.value,
  };
}

function severityBlocks(severity: ReviewSeverity): boolean {
  return severity === "P0" || severity === "P1" || severity === "P2";
}

function isRunEvidence(
  value: OperationalEvidence,
): value is ValidationEvidence | ReviewEvidence {
  return value.kind === "validation" || value.kind === "review";
}

function buildEvidenceSet(
  comments: readonly StructuredCommentSource[],
  context: EvidenceParseContext,
  sourceConflicts: ReadonlySet<string>,
): EvidenceSet {
  const values: OperationalEvidence[] = [];
  const advisories: Advisory[] = [];
  for (const source of comments) {
    if (!exactMarkerPresent(source.body, EVIDENCE_MARKER)) continue;
    const parsed = parseOperationalEvidence(source, context);
    if (parsed.ok) {
      values.push(parsed.value);
    } else {
      advisories.push({
        reason:
          parsed.reason === "producer-untrusted"
            ? "untrusted-structured-source"
            : "invalid-structured-source",
        detail: parsed.reason,
        provenance: [commentProvenance(parsed.source)],
      });
    }
  }
  values.sort((left, right) =>
    left.source.sourceId.localeCompare(right.source.sourceId),
  );

  const runBySource = new Map<string, ValidationEvidence | ReviewEvidence>();
  for (const value of values) {
    if (isRunEvidence(value)) runBySource.set(value.source.sourceId, value);
  }

  const relationshipFailures = new Map<string, ConditionReason>();
  for (const value of values) {
    if (sourceConflicts.has(value.source.sourceId)) {
      relationshipFailures.set(value.source.sourceId, "source-identity-conflict");
    }
  }
  const edge = new Map<string, string>();
  for (const value of runBySource.values()) {
    if (value.supersedes === undefined) continue;
    const target = runBySource.get(value.supersedes);
    if (target === undefined) {
      relationshipFailures.set(value.source.sourceId, "reference-missing");
      continue;
    }
    if (
      target.kind !== value.kind ||
      target.pullRequest !== value.pullRequest ||
      target.head !== value.head ||
      target.name !== value.name
    ) {
      relationshipFailures.set(value.source.sourceId, "reference-mismatch");
      continue;
    }
    edge.set(value.source.sourceId, target.source.sourceId);
  }

  for (const start of edge.keys()) {
    const path: string[] = [];
    const positions = new Map<string, number>();
    let current: string | undefined = start;
    while (current !== undefined && !relationshipFailures.has(current)) {
      const prior = positions.get(current);
      if (prior !== undefined) {
        for (const sourceId of path.slice(prior)) {
          relationshipFailures.set(sourceId, "reference-cycle");
        }
        break;
      }
      positions.set(current, path.length);
      path.push(current);
      current = edge.get(current);
    }
  }

  const superseded = new Set<string>();
  for (const [sourceId, target] of edge) {
    if (!relationshipFailures.has(sourceId)) superseded.add(target);
  }
  const activeRunSourceIds = new Set(
    [...runBySource.keys()].filter(
      (sourceId) =>
        !relationshipFailures.has(sourceId) && !superseded.has(sourceId),
    ),
  );
  return {
    values,
    relationshipFailures,
    activeRunSourceIds,
    sourceConflicts,
    advisories,
  };
}

function observationProblem<T>(observation: Observation<T>): Condition | undefined {
  if (observation.availability === "complete") return undefined;
  return condition(
    "unknown",
    observation.availability === "incomplete"
      ? "observation-incomplete"
      : "observation-unavailable",
    [nativeProvenance("native-observation", observation.source)],
  );
}

function reconcileValidation(
  name: string,
  input: ReconcileInput,
  evidence: EvidenceSet,
  advisories: Advisory[],
): ValidationCondition {
  const checks = [...(input.nativeChecks.facts ?? [])]
    .filter((check) => check.name === name)
    .sort((left, right) => left.source.id.localeCompare(right.source.id));
  const currentChecks = checks.filter(
    (check) => check.head === input.context.headSha,
  );
  const currentCustom = evidence.values.filter(
    (value): value is ValidationEvidence =>
      value.kind === "validation" &&
      value.name === name &&
      value.head === input.context.headSha,
  );
  if (currentChecks.length > 0 && currentCustom.length > 0) {
    advisories.push({
      reason: "custom-evidence-shadowed",
      provenance: [
        ...currentChecks.map((check) =>
          nativeProvenance("native-check", check.source),
        ),
        ...currentCustom.map((value) => commentProvenance(value.source)),
      ],
    });
  }
  const oldChecks = checks.filter(
    (check) => check.head !== input.context.headSha,
  );
  for (const check of oldChecks) {
    advisories.push({
      reason: "stale-native-evidence",
      provenance: [nativeProvenance("native-check", check.source)],
    });
  }
  const nativeSources = currentChecks.map((check) =>
    nativeProvenance("native-check", check.source),
  );
  if (currentChecks.some((check) => check.status === "failure")) {
    return {
      name,
      ...condition("blocked", "native-check-failed", nativeSources),
    };
  }
  const availability = observationProblem(input.nativeChecks);
  if (availability !== undefined) {
    return { name, ...availability };
  }
  if (currentChecks.some((check) => check.status === "pending")) {
    return {
      name,
      ...condition("waiting", "native-check-pending", nativeSources),
    };
  }
  if (currentChecks.some((check) => check.status === "success")) {
    return {
      name,
      ...condition("satisfied", "native-check-succeeded", nativeSources),
    };
  }

  const all = evidence.values.filter(
    (value): value is ValidationEvidence =>
      value.kind === "validation" && value.name === name,
  );
  const current = all.filter((value) => value.head === input.context.headSha);
  const stale = all.filter((value) => value.head !== input.context.headSha);
  for (const value of stale) {
    advisories.push({
      reason: "stale-structured-evidence",
      provenance: [commentProvenance(value.source)],
    });
  }
  const relationshipFailure = current.find((value) =>
    evidence.relationshipFailures.has(value.source.sourceId),
  );
  if (relationshipFailure !== undefined) {
    return {
      name,
      ...condition(
        "unknown",
        evidence.relationshipFailures.get(
          relationshipFailure.source.sourceId,
        ) ?? "reference-mismatch",
        [commentProvenance(relationshipFailure.source)],
      ),
    };
  }
  const active = current.filter((value) =>
    evidence.activeRunSourceIds.has(value.source.sourceId),
  );
  const sources = active.map((value) => commentProvenance(value.source));
  const results = new Set(active.map((value) => value.result));
  if (results.size > 1) {
    return { name, ...condition("unknown", "evidence-conflict", sources) };
  }
  const result = active[0]?.result;
  if (result === "pass") {
    return { name, ...condition("satisfied", "validation-passed", sources) };
  }
  if (result === "fail") {
    return { name, ...condition("blocked", "validation-failed", sources) };
  }
  if (result === "unknown") {
    return { name, ...condition("unknown", "validation-unknown", sources) };
  }
  if (stale.length > 0 || oldChecks.length > 0) {
    return {
      name,
      ...condition("unknown", "validation-stale", [
        ...stale.map((value) => commentProvenance(value.source)),
        ...oldChecks.map((check) =>
          nativeProvenance("native-check", check.source),
        ),
      ]),
    };
  }
  return {
    name,
    ...condition("unknown", "validation-missing", [
      { kind: "authority", id: `required-validation:${name}` },
    ]),
  };
}

interface FindingState {
  readonly blocking: readonly ReviewFindingEvidence[];
  readonly p3: readonly ReviewFindingEvidence[];
  readonly invalid: readonly {
    readonly value: ReviewFindingEvidence | ReviewResolutionEvidence;
    readonly reason: ConditionReason;
  }[];
}

function reconcileFindings(
  evidence: EvidenceSet,
  advisories: Advisory[],
): FindingState {
  const reviewRuns = evidence.values.filter(
    (value): value is ReviewEvidence => value.kind === "review",
  );
  const findings = evidence.values.filter(
    (value): value is ReviewFindingEvidence => value.kind === "review-finding",
  );
  const resolutions = evidence.values.filter(
    (value): value is ReviewResolutionEvidence =>
      value.kind === "review-resolution",
  );
  const findingBySource = new Map(
    findings.map((value) => [value.source.sourceId, value] as const),
  );
  const invalid: {
    value: ReviewFindingEvidence | ReviewResolutionEvidence;
    reason: ConditionReason;
  }[] = [];
  const validFindingIds = new Set<string>();
  for (const finding of findings) {
    if (evidence.sourceConflicts.has(finding.source.sourceId)) {
      invalid.push({ value: finding, reason: "source-identity-conflict" });
      continue;
    }
    const matchingRun = reviewRuns.some(
      (run) =>
        !evidence.relationshipFailures.has(run.source.sourceId) &&
        run.pullRequest === finding.pullRequest &&
        run.head === finding.head &&
        run.run === finding.run &&
        run.result === "findings",
    );
    if (matchingRun) validFindingIds.add(finding.source.sourceId);
    else invalid.push({ value: finding, reason: "review-run-missing" });
  }
  const resolved = new Set<string>();
  for (const resolution of resolutions) {
    if (evidence.sourceConflicts.has(resolution.source.sourceId)) {
      invalid.push({ value: resolution, reason: "source-identity-conflict" });
      continue;
    }
    const target = findingBySource.get(resolution.resolves);
    if (
      target === undefined ||
      target.pullRequest !== resolution.pullRequest ||
      !validFindingIds.has(target.source.sourceId)
    ) {
      invalid.push({ value: resolution, reason: "reference-mismatch" });
    } else {
      resolved.add(target.source.sourceId);
    }
  }
  const active = findings.filter(
    (finding) =>
      validFindingIds.has(finding.source.sourceId) &&
      !resolved.has(finding.source.sourceId),
  );
  const p3 = active.filter((finding) => finding.severity === "P3");
  for (const finding of p3) {
    advisories.push({
      reason: "review-finding-p3",
      provenance: [commentProvenance(finding.source)],
    });
  }
  return {
    blocking: active.filter((finding) => severityBlocks(finding.severity)),
    p3,
    invalid,
  };
}

function reconcileReview(
  input: ReconcileInput,
  evidence: EvidenceSet,
  advisories: Advisory[],
): Condition {
  const reviews = [...(input.nativeReviews.facts ?? [])].sort((left, right) =>
    left.source.id.localeCompare(right.source.id),
  );
  const currentReviews = reviews.filter(
    (review) => review.current,
  );
  const oldApprovals = reviews.filter(
    (review) =>
      (!review.current || review.head !== input.context.headSha) &&
      review.state === "APPROVED",
  );
  for (const review of reviews.filter(
    (value) => !value.current || value.head !== input.context.headSha,
  )) {
    advisories.push({
      reason: "stale-native-evidence",
      provenance: [nativeProvenance("native-review", review.source)],
    });
  }

  const threads = [...(input.nativeThreads.facts ?? [])].sort((left, right) =>
    left.source.id.localeCompare(right.source.id),
  );
  const openThreads = threads.filter((thread) => !thread.resolved);
  const blockingThreads = openThreads.filter(
    (thread) => thread.severity !== "unknown" && severityBlocks(thread.severity),
  );
  const unknownThreads = openThreads.filter(
    (thread) => thread.severity === "unknown",
  );
  for (const thread of openThreads.filter(
    (value) => value.severity === "P3",
  )) {
    advisories.push({
      reason: "native-thread-p3",
      provenance: [nativeProvenance("native-thread", thread.source)],
    });
  }

  const findingState = reconcileFindings(evidence, advisories);
  const requiredName = input.requirements.requiredReview;
  const currentCustomReviews = evidence.values.filter(
    (value): value is ReviewEvidence =>
      value.kind === "review" &&
      value.head === input.context.headSha &&
      (requiredName === null || value.name === requiredName),
  );
  if (
    currentReviews.some(
      (review) =>
        review.state === "CHANGES_REQUESTED" ||
        review.state === "APPROVED" ||
        review.state === "PENDING",
    ) &&
    currentCustomReviews.length > 0
  ) {
    advisories.push({
      reason: "custom-evidence-shadowed",
      provenance: [
        ...currentReviews
          .filter(
            (review) =>
              review.state === "CHANGES_REQUESTED" ||
              review.state === "APPROVED" ||
              review.state === "PENDING",
          )
          .map((review) => nativeProvenance("native-review", review.source)),
        ...currentCustomReviews.map((value) =>
          commentProvenance(value.source),
        ),
      ],
    });
  }
  const changesRequested = currentReviews.filter(
    (review) => review.state === "CHANGES_REQUESTED",
  );
  if (changesRequested.length > 0) {
    return condition(
      "blocked",
      "native-changes-requested",
      changesRequested.map((review) =>
        nativeProvenance("native-review", review.source),
      ),
    );
  }
  if (blockingThreads.length > 0) {
    return condition(
      "blocked",
      "native-thread-blocking",
      blockingThreads.map((thread) =>
        nativeProvenance("native-thread", thread.source),
      ),
    );
  }
  if (findingState.blocking.length > 0) {
    return condition(
      "blocked",
      "review-finding-blocking",
      findingState.blocking.map((finding) =>
        commentProvenance(finding.source),
      ),
    );
  }
  const reviewAvailability = observationProblem(input.nativeReviews);
  if (reviewAvailability !== undefined) return reviewAvailability;
  const threadAvailability = observationProblem(input.nativeThreads);
  if (threadAvailability !== undefined) return threadAvailability;
  if (unknownThreads.length > 0) {
    return condition(
      "unknown",
      "native-thread-unknown",
      unknownThreads.map((thread) =>
        nativeProvenance("native-thread", thread.source),
      ),
    );
  }
  if (findingState.invalid.length > 0) {
    const first = findingState.invalid[0];
    if (first !== undefined) {
      return condition("unknown", first.reason, [
        commentProvenance(first.value.source),
      ]);
    }
  }

  if (requiredName === null) {
    return condition("satisfied", "not-required", [
      { kind: "authority", id: "exact-head-review-not-required" },
    ]);
  }
  const nativeApproval = currentReviews.filter(
    (review) =>
      review.state === "APPROVED" && review.head === input.context.headSha,
  );
  if (nativeApproval.length > 0) {
    return condition(
      "satisfied",
      "native-approval-current",
      nativeApproval.map((review) =>
        nativeProvenance("native-review", review.source),
      ),
    );
  }
  const pendingReview = currentReviews.filter(
    (review) => review.state === "PENDING",
  );
  if (pendingReview.length > 0) {
    return condition(
      "waiting",
      "native-review-pending",
      pendingReview.map((review) =>
        nativeProvenance("native-review", review.source),
      ),
    );
  }

  const all = evidence.values.filter(
    (value): value is ReviewEvidence =>
      value.kind === "review" && value.name === requiredName,
  );
  const current = all.filter((value) => value.head === input.context.headSha);
  const stale = all.filter((value) => value.head !== input.context.headSha);
  for (const value of stale) {
    advisories.push({
      reason: "stale-structured-evidence",
      provenance: [commentProvenance(value.source)],
    });
  }
  const relationshipFailure = current.find((value) =>
    evidence.relationshipFailures.has(value.source.sourceId),
  );
  if (relationshipFailure !== undefined) {
    return condition(
      "unknown",
      evidence.relationshipFailures.get(relationshipFailure.source.sourceId) ??
        "reference-mismatch",
      [commentProvenance(relationshipFailure.source)],
    );
  }
  const active = current.filter((value) =>
    evidence.activeRunSourceIds.has(value.source.sourceId),
  );
  const results = new Set(active.map((value) => value.result));
  const sources = active.map((value) => commentProvenance(value.source));
  if (results.size > 1) {
    return condition("unknown", "evidence-conflict", sources);
  }
  const result = active[0]?.result;
  if (result === "clean") {
    return condition("satisfied", "review-clean-current", sources);
  }
  if (result === "unknown") {
    return condition("unknown", "review-unknown", sources);
  }
  if (result === "findings") {
    const matchingFindings = evidence.values.filter(
      (value): value is ReviewFindingEvidence =>
        value.kind === "review-finding" &&
        active.some(
          (run) => run.run === value.run && run.head === value.head,
        ),
    );
    if (matchingFindings.length === 0) {
      return condition("unknown", "review-findings-unclassified", sources);
    }
    return condition("satisfied", "review-findings-current", [
      ...sources,
      ...matchingFindings.map((value) => commentProvenance(value.source)),
    ]);
  }
  if (stale.length > 0 || oldApprovals.length > 0) {
    return condition("unknown", "review-stale", [
      ...stale.map((value) => commentProvenance(value.source)),
      ...oldApprovals.map((review) =>
        nativeProvenance("native-review", review.source),
      ),
    ]);
  }
  return condition("unknown", "review-missing", [
    { kind: "authority", id: `required-review:${requiredName}` },
  ]);
}

function aggregate(conditions: readonly Condition[]): Condition {
  const provenance = conditions.flatMap((value) => value.provenance);
  if (conditions.some((value) => value.state === "blocked")) {
    return condition("blocked", "explicit-blocker", provenance);
  }
  if (conditions.some((value) => value.state === "unknown")) {
    return condition("unknown", "required-evidence-unknown", provenance);
  }
  if (conditions.some((value) => value.state === "waiting")) {
    return condition("waiting", "native-evidence-waiting", provenance);
  }
  return condition("satisfied", "all-required-conditions-satisfied", provenance);
}

function stableAdvisories(values: readonly Advisory[]): readonly Advisory[] {
  const keyed = new Map<string, Advisory>();
  for (const value of values) {
    const key = `${value.reason}:${value.detail ?? ""}:${value.provenance
      .map(provenanceKey)
      .sort()
      .join(",")}`;
    keyed.set(key, {
      ...value,
      provenance: stableProvenance(value.provenance),
    });
  }
  return [...keyed.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([, value]) => value);
}

function withObservedContext(
  value: Condition,
  context: EvidenceParseContext,
): Condition {
  return {
    ...value,
    observedHead: context.headSha,
    observedMain: context.mainSha,
  };
}

export function reconcile(input: ReconcileInput): Reconciliation {
  const stable = stableComments(input.comments);
  const comments = stable.values;
  const advisories: Advisory[] = (input.advisories ?? []).map((value) => ({
    reason:
      value.kind === "cache"
        ? "cached-observation-stale"
        : "unstructured-prose",
    provenance: [nativeProvenance(value.kind, value.source)],
  }));
  for (const source of comments) {
    const isHandoff = exactMarkerPresent(source.body, HANDOFF_MARKER);
    const isWatch = exactMarkerPresent(source.body, WATCH_MARKER);
    const isEvidence = exactMarkerPresent(source.body, EVIDENCE_MARKER);
    if (!isHandoff && !isWatch && !isEvidence) {
      advisories.push({
        reason: "unstructured-prose",
        provenance: [
          nativeProvenance("prose", {
            id: source.metadata.id,
            url: source.metadata.url,
          }),
        ],
      });
      continue;
    }
    if (isEvidence || (source.metadata.trustedProducer && source.metadata.topLevel)) {
      continue;
    }
    const parsed = isHandoff
      ? parseAgentHandoff(source, input.context)
      : parseStewardWatch(source, input.context);
    if (!parsed.ok) {
      advisories.push({
        reason:
          parsed.reason === "producer-untrusted"
            ? "untrusted-structured-source"
            : "invalid-structured-source",
        detail: parsed.reason,
        provenance: [commentProvenance(parsed.source)],
      });
    }
  }

  const handoffSelection = selectSingleton(
    comments,
    stable.conflicts,
    HANDOFF_MARKER,
    parseAgentHandoff,
    input.context,
  );
  const watchSelection = selectSingleton(
    comments,
    stable.conflicts,
    WATCH_MARKER,
    parseStewardWatch,
    input.context,
  );
  const evidence = buildEvidenceSet(comments, input.context, stable.conflicts);
  advisories.push(...evidence.advisories);

  const handoff = handoffSelection.condition;
  let watch = watchSelection.condition;
  let humanAction: Condition;
  const trustedWatchCandidatePresent = comments.some(
    (source) =>
      source.metadata.topLevel &&
      source.metadata.trustedProducer &&
      exactMarkerPresent(source.body, WATCH_MARKER),
  );
  if (watchSelection.value !== undefined) {
    const value = watchSelection.value;
    if (!("verdict" in value) || !("humanAction" in value)) {
      throw new Error("internal watch selection type mismatch");
    }
    watch =
      value.verdict === "HOLD"
        ? condition("blocked", "steward-watch-hold", [
            commentProvenance(value.source),
          ])
        : condition(
            "satisfied",
            value.verdict === "GREEN"
              ? "steward-watch-green"
              : "steward-watch-amber",
            [commentProvenance(value.source)],
          );
    humanAction = condition(
      value.humanAction === "required" ? "blocked" : "satisfied",
      value.humanAction === "required"
        ? "human-action-required"
        : "human-action-none",
      [commentProvenance(value.source)],
    );
  } else if (
    input.requirements.currentStewardWatch ||
    trustedWatchCandidatePresent
  ) {
    humanAction = condition("unknown", watch.reason, watch.provenance);
  } else {
    watch = condition("satisfied", "watch-not-required", [
      { kind: "authority", id: "current-steward-watch-not-required" },
    ]);
    humanAction = condition("satisfied", "not-required", [
      { kind: "authority", id: "current-steward-watch-not-required" },
    ]);
  }

  const validations = [...new Set(input.requirements.requiredValidations)]
    .sort((left, right) => left.localeCompare(right))
    .map((name) => reconcileValidation(name, input, evidence, advisories));
  const review = reconcileReview(input, evidence, advisories);
  const watchParticipates =
    input.requirements.currentStewardWatch || watchSelection.value !== undefined;
  const mutationGate = aggregate([
    handoff,
    ...(watchParticipates ? [watch, humanAction] : []),
  ]);
  const mergeGate = aggregate([
    mutationGate,
    ...validations,
    review,
  ]);
  return {
    handoff: withObservedContext(handoff, input.context),
    watch: withObservedContext(watch, input.context),
    validations: validations.map((value) => ({
      ...withObservedContext(value, input.context),
      name: value.name,
    })),
    review: withObservedContext(review, input.context),
    humanAction: withObservedContext(humanAction, input.context),
    mutationGate: withObservedContext(mutationGate, input.context),
    mergeGate: withObservedContext(mergeGate, input.context),
    advisories: stableAdvisories(advisories),
  };
}
