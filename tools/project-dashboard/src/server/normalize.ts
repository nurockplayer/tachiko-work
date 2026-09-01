import {
  parseAgentHandoff,
  reconcile,
  type Condition,
  type Observation,
  type StructuredCommentSource,
} from "@tachiko-work/operational-evidence";

import type {
  AttentionItem,
  DeliveryLane,
  DisplaySignal,
  DisplayState,
  RawIssue,
  RawPullRequest,
  RepositoryObservation,
  RepositoryProjection,
  SourceLink,
} from "../shared/model.js";
import { reasonLabels } from "../shared/model.js";
import { parseProductHorizon } from "./roadmap.js";

const ROADMAP_PATH = "docs/product/product-roadmap.md";
const RELEASE_CHECK = "release-check";
const PROJECT_REVIEW = "project-review";
const AUTOMATED_BROWSER_CHECK = "Live Project Dashboard browser journey";
const PERCEPTUAL_REVIEW_CHECK = "perceptual-review";

type ImplementationOverlap = "none" | "unknown" | "conflict";

function attentionKey(item: AttentionItem): string {
  const sourceUrls = [...new Set(item.sources.map((source) => source.url))].sort().join("|");
  return `${String(item.issueNumber ?? "repository")}:${item.reason}:${item.label}:${sourceUrls}`;
}

function evidenceSource(
  label: string,
  url: string,
  evidenceClass: SourceLink["evidenceClass"] = "direct",
): SourceLink {
  return { label, url, evidenceClass };
}

function directSignal(
  state: DisplayState,
  reason: string,
  label: string,
  sources: SourceLink[],
): DisplaySignal {
  return { state, reason, label, sources };
}

function fromCondition(condition: Condition, fallbackLabel: string): DisplaySignal {
  const sources: SourceLink[] = condition.provenance.flatMap((value) => {
    if (value.kind === "comment") {
      return [evidenceSource("Structured comment", value.source.url)];
    }
    if ("url" in value) {
      return [evidenceSource(value.kind, value.url)];
    }
    return [];
  });
  return directSignal(
    condition.state,
    condition.reason,
    reasonLabels[condition.reason] ?? stateFallbackLabel(fallbackLabel, condition.state),
    sources,
  );
}

function stateFallbackLabel(
  fallbackLabel: string,
  state: Condition["state"],
): string {
  if (state === "unknown") return fallbackLabel;
  const subject = fallbackLabel.replace(/\s+Unknown$/, "");
  return `${subject} ${state}`;
}

function observation<T>(
  availability: "complete" | "incomplete" | "unavailable",
  facts: T[],
  id: string,
  url: string,
): Observation<T> {
  return { availability, facts, source: { id, url } };
}

function checkSignal(pull: RawPullRequest, name?: string): DisplaySignal {
  const checks =
    name === undefined
      ? pull.checks
      : pull.checks.filter((check) => check.name === name);
  const sources = checks.map((check) =>
    evidenceSource(`Check · ${check.name}`, check.url),
  );
  if (
    checks.some(
      (check) => check.headSha === pull.headSha && check.status === "failure",
    )
  ) {
    return directSignal("blocked", "native-check-failed", "Exact-head check failed", sources);
  }
  if (pull.checksAvailability !== "complete" || checks.length === 0) {
    return directSignal(
      "unknown",
      pull.checksAvailability === "unavailable"
        ? "observation-unavailable"
        : "observation-incomplete",
      "Exact-head checks Unknown",
      sources,
    );
  }
  if (checks.some((check) => check.headSha !== pull.headSha)) {
    return directSignal("unknown", "validation-stale", "Check identity mismatch", sources);
  }
  if (checks.some((check) => check.status === "pending")) {
    return directSignal("waiting", "native-check-pending", "Exact-head checks pending", sources);
  }
  return directSignal("satisfied", "native-check-succeeded", "Exact-head checks passed", sources);
}

function ownerFor(issue: RawIssue): string {
  if (issue.labelsAvailability !== "complete") return "unknown";
  const owners = issue.labels.filter((label) => label.startsWith("agent:"));
  return owners.length === 1 ? owners.at(0) ?? "unknown" : "unknown";
}

function ownershipFor(issue: RawIssue): DisplaySignal {
  const source = evidenceSource("Issue owner labels", issue.url);
  if (issue.labelsAvailability !== "complete") {
    return directSignal("unknown", "observation-incomplete", "Issue ownership Unknown", [source]);
  }
  const owners = issue.labels.filter((label) => label.startsWith("agent:"));
  if (owners.length !== 1) {
    return directSignal(
      "unknown",
      owners.length === 0 ? "required-evidence-unknown" : "source-identity-conflict",
      "Issue ownership Unknown",
      [source],
    );
  }
  return directSignal(
    "satisfied",
    "all-required-conditions-satisfied",
    `Issue owner · ${owners[0] ?? "unknown"}`,
    [source],
  );
}

function humanOwnershipFor(issue: RawIssue): DisplaySignal {
  const ownership = ownershipFor(issue);
  if (ownership.state !== "satisfied") return ownership;
  return ownerFor(issue) === "agent:human"
    ? directSignal(
        "blocked",
        "human-action-required",
        "Human-owned issue requires attention",
        [evidenceSource("Issue owner label", issue.url)],
      )
    : directSignal("satisfied", "not-required", "No human owner action required", []);
}

function labelReadinessFor(issue: RawIssue): DisplaySignal {
  const source = evidenceSource("Issue labels", issue.url);
  if (issue.labelsAvailability !== "complete") {
    return directSignal("unknown", "observation-incomplete", "Issue labels Unknown", [source]);
  }
  const ready = issue.labels.includes("state:ready");
  const explicitlyNotReady =
    issue.labels.includes("state:blocked") || issue.labels.includes("state:parked");
  if (ready && explicitlyNotReady) {
    return directSignal(
      "unknown",
      "source-identity-conflict",
      "Issue readiness labels conflict",
      [source],
    );
  }
  if (explicitlyNotReady) {
    return directSignal("blocked", "issue-not-ready", "Issue is not Ready", [source]);
  }
  if (ready) {
    return directSignal("satisfied", "all-required-conditions-satisfied", "Issue Ready", [source]);
  }
  return directSignal(
    "unknown",
    "required-evidence-unknown",
    "Issue readiness Unknown",
    [source],
  );
}

function readinessFor(issue: RawIssue): DisplaySignal {
  const labelReadiness = labelReadinessFor(issue);
  if (labelReadiness.state !== "satisfied") return labelReadiness;
  if (issue.dependencyAvailability !== "complete") {
    return directSignal(
      "unknown",
      "observation-incomplete",
      "Dependency state Unknown",
      [evidenceSource("Issue dependencies", issue.url)],
    );
  }
  if (issue.blockedBy.some((dependency) => dependency.state === "OPEN")) {
    return directSignal("waiting", "dependency-waiting", "Waiting on dependency", [
      evidenceSource("Issue labels", issue.url),
      ...issue.blockedBy.map((dependency) =>
        evidenceSource(`Issue #${String(dependency.number)}`, dependency.url),
      ),
    ]);
  }
  return labelReadiness;
}

function roadmapAlignmentFor(
  issue: RawIssue,
  currentHorizon: string | null,
): DisplaySignal {
  const source = evidenceSource("Issue milestone", issue.url);
  if (issue.milestone.state === "unknown") {
    return directSignal(
      "unknown",
      issue.milestone.availability === "unavailable"
        ? "observation-unavailable"
        : "observation-incomplete",
      "Issue milestone alignment Unknown",
      [source],
    );
  }
  if (issue.milestone.state === "null") {
    return directSignal(
      "satisfied",
      "not-required",
      "Unmilestoned Issue is permitted",
      [source],
    );
  }
  if (currentHorizon === null) {
    return directSignal(
      "unknown",
      "observation-incomplete",
      "Issue milestone alignment Unknown",
      [source],
    );
  }
  return issue.milestone.value === currentHorizon
    ? directSignal(
        "satisfied",
        "all-required-conditions-satisfied",
        "Issue belongs to the current Product Roadmap horizon",
        [source],
      )
    : directSignal(
        "blocked",
        "issue-not-ready",
        "Issue milestone is outside the current Product Roadmap horizon",
        [source],
      );
}

function aggregateSignals(signals: DisplaySignal[], fallback: string): DisplaySignal {
  const state = signals.some((signal) => signal.state === "blocked")
    ? "blocked"
    : signals.some((signal) => signal.state === "unknown")
        ? "unknown"
        : signals.some((signal) => signal.state === "waiting")
          ? "waiting"
          : signals.every((signal) => signal.state === "satisfied")
            ? "satisfied"
            : "advisory";
  const selected = signals.find((signal) => signal.state === state) ?? signals[0];
  return directSignal(
    state,
    selected?.reason ?? "required-evidence-unknown",
    selected?.label ?? fallback,
    signals.flatMap((signal) => signal.sources),
  );
}

function commentCompleteness(pull: RawPullRequest): DisplaySignal {
  return pull.commentsAvailability === "complete"
    ? directSignal("satisfied", "all-required-conditions-satisfied", "Structured comments complete", [
        evidenceSource("Pull request comments", pull.url),
      ])
    : directSignal(
        "unknown",
        pull.commentsAvailability === "unavailable"
          ? "observation-unavailable"
          : "observation-incomplete",
        "Structured comment evidence incomplete",
        [evidenceSource("Pull request comments", pull.url)],
      );
}

function commentSources(pull: RawPullRequest, repository: string): StructuredCommentSource[] {
  return pull.comments.map((comment) => ({
    body: comment.body,
    metadata: {
      repository,
      id: comment.id,
      kind: comment.kind,
      authorLogin: comment.authorLogin,
      authorAssociation: comment.authorAssociation,
      url: comment.url,
      createdAt: comment.createdAt,
      updatedAt: comment.updatedAt,
      edited: comment.lastEditedAt.state !== "null",
      topLevel: comment.topLevel,
      trustedProducer: comment.trustedProducer,
    },
  }));
}

function trustedHandoffIssueClaims(
  repository: RepositoryObservation,
  pull: RawPullRequest,
  issues: readonly RawIssue[],
): { current: number[]; stale: number[]; ambiguous: number[]; unscoped: boolean } {
  if (repository.main === null) {
    return { current: [], stale: [], ambiguous: [], unscoped: false };
  }
  const current = new Set<number>();
  const stale = new Set<number>();
  const ambiguous = new Set<number>();
  let unscoped = false;
  for (const source of commentSources(pull, repository.repository)) {
    if (!source.metadata.trustedProducer || !source.metadata.topLevel) continue;
    const identity = boundedHandoffIdentity(source.body);
    if (identity === null) {
      if (hasExactHandoffMarker(source.body)) unscoped = true;
      continue;
    }
    for (const issueNumber of identity.issues) {
      const issue = issues.find((candidate) => candidate.number === issueNumber);
      if (issue === undefined) continue;
      if (
        identity.issues.length !== 1 ||
        identity.pullRequests.length !== 1 ||
        identity.pullRequests[0] !== pull.number
      ) {
        ambiguous.add(issue.number);
        continue;
      }
      const result = parseAgentHandoff(source, {
        repository: repository.repository,
        issueNumber: issue.number,
        pullRequestNumber: pull.number,
        owner: ownerFor(issue),
        headSha: pull.headSha,
        mainSha: repository.main.sha,
      });
      if (result.ok) current.add(result.value.issue);
      else if (result.reason === "head-mismatch" || result.reason === "main-mismatch") {
        stale.add(issue.number);
      } else {
        ambiguous.add(issue.number);
      }
    }
  }
  return {
    current: [...current],
    stale: [...stale],
    ambiguous: [...ambiguous],
    unscoped,
  };
}

function hasExactHandoffMarker(body: string): boolean {
  return body.replaceAll("\r\n", "\n").split("\n").includes("<!-- agent-handoff:v1 -->");
}

function boundedHandoffIdentity(
  body: string,
): { issues: number[]; pullRequests: number[] } | null {
  const lines = body.replaceAll("\r\n", "\n").split("\n");
  const header: string[] = [];
  const markerIndexes = lines.flatMap((line, index) =>
    line === "<!-- agent-handoff:v1 -->" ? [index] : [],
  );
  if (markerIndexes.length === 0) return null;
  for (const markerIndex of markerIndexes) {
    for (let index = markerIndex + 1; index < lines.length; index += 1) {
      const line = lines[index];
      if (line === undefined || line === "") break;
      header.push(line);
    }
  }
  const values = (field: "ISSUE" | "PR") =>
    header.flatMap((line) => {
      const match = new RegExp(`^${field}: ([0-9]+)$`).exec(line);
      return match?.[1] === undefined ? [] : [match[1]];
    });
  const positiveSafeIntegers = (rawValues: string[]) =>
    [...new Set(rawValues.map(Number))].filter(
      (value) => Number.isSafeInteger(value) && value > 0,
    );
  const issues = positiveSafeIntegers(values("ISSUE"));
  const pullRequests = positiveSafeIntegers(values("PR"));
  return issues.length > 0 ? { issues, pullRequests } : null;
}

function mergeabilityFor(pull: RawPullRequest): DisplaySignal {
  const source = [evidenceSource(`PR #${String(pull.number)}`, pull.url)];
  const decision = pull.reviewDecision;
  const nativePolicy =
    pull.mergeStateStatus === "DIRTY" ||
    pull.mergeStateStatus === "BLOCKED" ||
    pull.mergeStateStatus === "BEHIND" ||
    (decision.state === "value" && decision.value === "CHANGES_REQUESTED")
      ? "blocked"
      : pull.mergeStateStatus === "UNKNOWN" || decision.state === "unknown"
        ? "unknown"
        : decision.state === "value" && decision.value === "REVIEW_REQUIRED"
          ? "waiting"
          : "satisfied";
  if (pull.mergeability === "conflicting" || nativePolicy === "blocked") {
    return directSignal(
      "blocked",
      pull.mergeability === "conflicting" ? "native-merge-conflict" : "native-merge-policy-blocked",
      pull.mergeability === "conflicting"
        ? "Pull request has a native merge conflict"
        : "GitHub native merge policy blocks this pull request",
      source,
    );
  }
  if (pull.mergeability === "unknown" || nativePolicy === "unknown") {
    return directSignal(
      "unknown",
      "observation-incomplete",
      "Native mergeability Unknown",
      source,
    );
  }
  if (nativePolicy === "waiting") {
    return directSignal(
      "waiting",
      "native-review-required",
      "GitHub native review policy is waiting",
      source,
    );
  }
  return directSignal(
    "satisfied",
    "all-required-conditions-satisfied",
    "Pull request is natively mergeable",
    source,
  );
}

function implementationConflictSignal(
  repository: RepositoryObservation,
  pull: RawPullRequest,
  implementationOverlap: ImplementationOverlap,
): DisplaySignal {
  const source = evidenceSource(`PR #${String(pull.number)}`, pull.url);
  if (implementationOverlap === "conflict") {
    return directSignal(
      "blocked",
      "source-identity-conflict",
      "Competing implementation pull requests",
      [source],
    );
  }
  if (implementationOverlap === "unknown") {
    return directSignal(
      "unknown",
      "source-identity-conflict",
      "Implementation ownership needs reconciliation",
      [source],
    );
  }
  if (repository.implementationLinkageAvailability !== "complete") {
    return directSignal(
      "unknown",
      repository.implementationLinkageAvailability === "unavailable"
        ? "observation-unavailable"
        : "observation-incomplete",
      "Implementation overlap observation Unknown",
      [source],
    );
  }
  return directSignal(
    "satisfied",
    "not-required",
    "No competing implementation pull request",
    [source],
  );
}

function currentReviewDisposition(
  reviews: RawPullRequest["reviews"],
): { ids: Set<string>; complete: boolean } {
  const decisiveByAuthor = new Map<string, RawPullRequest["reviews"]>();
  const ids = new Set(
    reviews.filter((review) => review.state === "PENDING").map((review) => review.id),
  );
  for (const review of reviews) {
    if (review.state !== "APPROVED" && review.state !== "CHANGES_REQUESTED") continue;
    const authorReviews = decisiveByAuthor.get(review.authorLogin) ?? [];
    authorReviews.push(review);
    decisiveByAuthor.set(review.authorLogin, authorReviews);
  }
  let complete = true;
  for (const authorReviews of decisiveByAuthor.values()) {
    if (authorReviews.some((review) => review.submittedAt === null)) {
      complete = false;
      for (const review of authorReviews) ids.add(review.id);
      continue;
    }
    const latestTimestamp = authorReviews.reduce(
      (latest, review) =>
        review.submittedAt !== null && review.submittedAt > latest
          ? review.submittedAt
          : latest,
      "",
    );
    for (const review of authorReviews) {
      if (review.submittedAt === latestTimestamp) ids.add(review.id);
    }
  }
  return { ids, complete };
}

function isTrustedNativeReviewer(review: RawPullRequest["reviews"][number]): boolean {
  return review.authorAssociation === "OWNER" ||
    review.authorAssociation === "MEMBER" ||
    review.authorAssociation === "COLLABORATOR";
}

function pullLane(
  repository: RepositoryObservation,
  issue: RawIssue,
  pull: RawPullRequest,
  implementationOverlap: ImplementationOverlap,
  roadmapCurrent: boolean,
  currentHorizon: string | null,
): DeliveryLane {
  const mainSha = repository.main?.sha ?? "";
  const githubUrl = pull.url;
  const implementationState = implementationConflictSignal(
    repository,
    pull,
    implementationOverlap,
  );
  const ownership = ownershipFor(issue);
  const humanOwnership = humanOwnershipFor(issue);
  const labelReadiness = labelReadinessFor(issue);
  const readiness = aggregateSignals(
    [readinessFor(issue), roadmapAlignmentFor(issue, currentHorizon)],
    "Issue readiness Unknown",
  );
  const mergeability = mergeabilityFor(pull);
  const trustedReviews = pull.reviews.filter(isTrustedNativeReviewer);
  const currentReviews = currentReviewDisposition(trustedReviews);
  const nativeReviewAvailability =
    pull.reviewsAvailability === "complete" && !currentReviews.complete
      ? "incomplete"
      : pull.reviewsAvailability;
  const reconciliation = reconcile({
    context: {
      repository: repository.repository,
      issueNumber: issue.number,
      pullRequestNumber: pull.number,
      owner: ownerFor(issue),
      headSha: pull.headSha,
      mainSha,
    },
    comments: commentSources(pull, repository.repository),
    nativeChecks: observation(
      pull.checksAvailability,
      pull.checks.map((check) => ({
        name: check.name,
        head: check.headSha,
        status: check.status,
        source: { id: `check:${check.name}`, url: check.url },
      })),
      `checks:${String(pull.number)}`,
      githubUrl,
    ),
    nativeReviews: observation(
      nativeReviewAvailability,
      trustedReviews.map((review) => ({
        current: currentReviews.ids.has(review.id),
        head: review.commitSha,
        state: review.state,
        source: { id: review.id, url: review.url },
      })),
      `reviews:${String(pull.number)}`,
      githubUrl,
    ),
    nativeThreads: observation(
      pull.threadsAvailability,
      pull.threads.map((thread) => ({
        resolved: thread.resolved,
        outdated: thread.outdated,
        severity: "unknown" as const,
        source: { id: thread.id, url: thread.url },
      })),
      `threads:${String(pull.number)}`,
      githubUrl,
    ),
    nativeRepository: observation(
      repository.main === null ||
        repository.roadmap === null ||
        !roadmapCurrent ||
        repository.implementationLinkageAvailability !== "complete" ||
        ownership.state !== "satisfied" ||
        issue.labelsAvailability !== "complete" ||
        issue.dependencyAvailability !== "complete" ||
        readiness.state === "unknown"
        ? "incomplete"
        : "complete",
      [
        {
          issueReady: labelReadiness.state === "satisfied",
          dependencies: issue.blockedBy.some((dependency) => dependency.state === "OPEN")
            ? "waiting"
            : "satisfied",
          pullRequestState: pull.state,
          pullRequestDraft: pull.draft,
          baseRef: pull.baseRef,
          authorityConflict: implementationOverlap === "conflict",
        },
      ],
      `repository:${String(pull.number)}`,
      githubUrl,
    ),
    requirements: {
      requiredValidations: [{ name: RELEASE_CHECK, evidence: "manual" }],
      requiredReview: { name: PROJECT_REVIEW, evidence: "manual" },
      currentStewardWatch: true,
      expectedBaseRef: "main",
    },
    advisories: [
      {
        kind: "prose",
        source: { id: `narrative:${String(pull.number)}`, url: pull.url },
      },
    ],
  });
  const checks = checkSignal(pull);
  const commentState = commentCompleteness(pull);
  const readinessReconciliation = labelReadiness.state === "unknown"
    ? labelReadiness
    : directSignal(
        "satisfied",
        "not-required",
        "No readiness-label reconciliation required",
        labelReadiness.sources,
      );
  const labelState = issue.labelsAvailability === "complete"
    ? directSignal(
        "satisfied",
        "all-required-conditions-satisfied",
        "Issue labels complete",
        [evidenceSource("Issue labels", issue.url)],
      )
    : directSignal(
        "unknown",
        "observation-incomplete",
        "Issue labels incomplete",
        [evidenceSource("Issue labels", issue.url)],
      );
  const review = aggregateSignals(
    [fromCondition(reconciliation.review, "Exact-head review Unknown"), commentState],
    "Exact-head review Unknown",
  );
  const handoff = aggregateSignals(
    [fromCondition(reconciliation.handoff, "Handoff Unknown"), commentState],
    "Handoff Unknown",
  );
  const stewardWatch = aggregateSignals(
    [fromCondition(reconciliation.watch, "Steward watch Unknown"), commentState],
    "Steward watch Unknown",
  );
  const humanAction = aggregateSignals(
    [
      fromCondition(reconciliation.humanAction, "Human action Unknown"),
      commentState,
      labelState,
      ownership,
      humanOwnership,
      readinessReconciliation,
    ],
    "Human action Unknown",
  );
  const deliveryIntegrity = aggregateSignals(
    [
      reconciliation.validations[0] === undefined
        ? directSignal("unknown", "validation-missing", "Delivery integrity Unknown", [])
        : fromCondition(reconciliation.validations[0], "Delivery integrity Unknown"),
      commentState,
    ],
    "Delivery integrity Unknown",
  );
  const observedAuthority =
    pull.authorityAvailability !== "complete"
      ? directSignal(
          "unknown",
          pull.authorityAvailability === "unavailable"
            ? "observation-unavailable"
            : "observation-incomplete",
          "Authority-path observation Unknown",
          [evidenceSource("Pull request", pull.url)],
        )
      : pull.authorityChanges.length === 0
        ? fromCondition(reconciliation.authority, "Authority state Unknown")
      : directSignal(
          "unknown",
          "authority-drift-suspected",
          "Changed authority paths need reconciliation",
          pull.authorityChanges.map((change) =>
            evidenceSource(`Authority · ${change.path}`, change.url, "derived"),
          ),
        );
  const authority = aggregateSignals(
    [observedAuthority, implementationState],
    "Authority state Unknown",
  );
  const mergeGate = aggregateSignals(
    [
      fromCondition(reconciliation.mergeGate, "Merge gate Unknown"),
      checks,
      commentState,
      authority,
      humanAction,
      readiness,
      mergeability,
    ],
    "Merge gate Unknown",
  );
  const phase = humanAction.state === "blocked"
    ? "human_required"
    : review.state === "blocked"
      ? "review_fix"
      : [readiness, checks, stewardWatch, authority, mergeGate].some(
            (signal) => signal.state === "blocked",
          )
        ? "blocked"
        : [commentState, checks, handoff, stewardWatch, humanAction].some(
              (signal) => signal.state === "unknown",
            )
          ? "unknown"
        : readiness.state === "waiting"
          ? mergeGate.state === "waiting" ? "waiting" : "unknown"
          : commentState.state === "unknown"
            ? "unknown"
          : checks.state === "unknown"
            ? "unknown"
          : review.state === "waiting"
            ? mergeGate.state === "waiting" ? "review_wait" : "unknown"
          : mergeGate.state === "satisfied"
            ? "merge_gate"
            : mergeability.state === "waiting"
              ? mergeGate.state === "waiting" ? "review_wait" : "unknown"
            : mergeability.state === "unknown"
              ? "unknown"
            : readiness.state === "unknown"
              ? "unknown"
              : authority.reason === "authority-drift-suspected"
                ? "rereview"
                : authority.state === "unknown"
                  ? "unknown"
                  : mergeGate.state === "unknown" && checks.state === "waiting"
                    ? "unknown"
                  : checks.state === "waiting"
                    ? "validating"
                    : checks.state === "satisfied" && review.state !== "satisfied"
                      ? "rereview"
                      : mergeGate.state === "unknown"
                        ? "unknown"
                        : mergeGate.state === "waiting"
                          ? "waiting"
                          : "implementing";
  return {
    issue: { number: issue.number, title: issue.title, url: issue.url },
    owner: ownerFor(issue),
    phase,
    pullRequest: {
      number: pull.number,
      title: pull.title,
      url: pull.url,
      headSha: pull.headSha,
      baseSha: pull.baseSha,
      liveMainSha: mainSha,
      mergeBaseSha: pull.mergeBaseSha,
      baseRef: pull.baseRef,
      relationToMain: pull.relationToMain,
      draft: pull.draft,
    },
    readiness,
    checks,
    review,
    handoff,
    stewardWatch,
    authority,
    humanAction,
    mergeGate,
    evidence: {
      automatedBrowser: checkSignal(pull, AUTOMATED_BROWSER_CHECK),
      perceptualReview: checkSignal(pull, PERCEPTUAL_REVIEW_CHECK),
      deliveryIntegrity,
    },
    sources: [
      evidenceSource(`Issue #${String(issue.number)}`, issue.url),
      evidenceSource(`PR #${String(pull.number)}`, pull.url),
      ...[
        readiness,
        checks,
        review,
        handoff,
        stewardWatch,
        authority,
        humanAction,
        mergeGate,
        deliveryIntegrity,
      ].flatMap((signal) => signal.sources),
      ...reconciliation.advisories.flatMap((advisory) =>
        advisory.provenance.flatMap((value) =>
          "url" in value
            ? [evidenceSource("Advisory evidence", value.url, "advisory")]
            : [],
        ),
      ),
    ],
  };
}

function issueLane(
  repository: RepositoryObservation,
  issue: RawIssue,
  roadmapCurrent: boolean,
  currentHorizon: string | null,
  implementationUncertain: boolean,
): DeliveryLane {
  const labelReadiness = labelReadinessFor(issue);
  const readiness = aggregateSignals(
    [readinessFor(issue), roadmapAlignmentFor(issue, currentHorizon)],
    "Issue readiness Unknown",
  );
  const owner = ownerFor(issue);
  const humanOwnership = humanOwnershipFor(issue);
  const linkageComplete = repository.implementationLinkageAvailability === "complete";
  const unavailable = implementationUncertain
    ? directSignal(
        "unknown",
        "source-identity-conflict",
        "Implementation ownership Unknown",
        [evidenceSource(`Issue #${String(issue.number)}`, issue.url)],
      )
    : linkageComplete
    ? directSignal(
        "unknown",
        "not-required",
        "No implementation PR",
        [evidenceSource(`Issue #${String(issue.number)}`, issue.url)],
      )
    : directSignal(
        "unknown",
        repository.implementationLinkageAvailability === "unavailable"
          ? "observation-unavailable"
          : "observation-incomplete",
        "Implementation PR linkage Unknown",
        [evidenceSource(`Issue #${String(issue.number)}`, issue.url)],
      );
  return {
    issue: { number: issue.number, title: issue.title, url: issue.url },
    owner,
    phase: humanOwnership.state === "blocked"
      ? "human_required"
      : readiness.state === "blocked"
      ? "blocked"
      : implementationUncertain
        ? "unknown"
      : linkageComplete && roadmapCurrent
        ? readiness.state === "satisfied" ? "ready" : readiness.state
        : "unknown",
    pullRequest: null,
    readiness,
    checks: unavailable,
    review: unavailable,
    handoff: unavailable,
    stewardWatch: unavailable,
    authority: repository.main === null
      ? directSignal("unknown", "observation-unavailable", "Authority Unknown", [])
      : implementationUncertain
        ? directSignal(
            "unknown",
            "source-identity-conflict",
            "Implementation ownership Unknown",
            [evidenceSource(`Issue #${String(issue.number)}`, issue.url)],
          )
      : !roadmapCurrent
        ? directSignal(
            "unknown",
            "observation-incomplete",
            "Product Roadmap authority Unknown",
            [
              evidenceSource(
                "Product Roadmap",
                repository.roadmap?.url ??
                  `https://github.com/${repository.repository}/blob/main/${ROADMAP_PATH}`,
              ),
            ],
          )
      : directSignal("satisfied", "repository-authority-current", "Authority observed", [
          evidenceSource("Live main", repository.main.url),
        ]),
    humanAction:
      humanOwnership.state !== "satisfied"
        ? humanOwnership
        : labelReadiness.state === "unknown"
          ? labelReadiness
        : directSignal("satisfied", "not-required", "No human action required", []),
    mergeGate: unavailable,
    evidence: {
      automatedBrowser: unavailable,
      perceptualReview: unavailable,
      deliveryIntegrity: unavailable,
    },
    sources: [evidenceSource(`Issue #${String(issue.number)}`, issue.url)],
  };
}

function unlinkedPullLane(
  repository: RepositoryObservation,
  pull: RawPullRequest,
  implementationOverlap: ImplementationOverlap,
  roadmapCurrent: boolean,
  definiteIssues: readonly RawIssue[],
): DeliveryLane {
  const pullSource = evidenceSource(`PR #${String(pull.number)}`, pull.url);
  const unknownIssue = directSignal(
    "unknown",
    "source-identity-conflict",
    "Native Issue linkage Unknown",
    [pullSource],
  );
  const checks = checkSignal(pull);
  const trustedReviews = pull.reviews.filter(isTrustedNativeReviewer);
  const reviewSources = [
    ...trustedReviews.map((item) => evidenceSource("Native review", item.url)),
    ...pull.threads.map((item) => evidenceSource("Native review thread", item.url)),
  ];
  const currentReviews = currentReviewDisposition(trustedReviews);
  const review =
    trustedReviews.some(
      (item) => currentReviews.ids.has(item.id) && item.state === "CHANGES_REQUESTED",
    )
      ? directSignal(
          "blocked",
          "native-changes-requested",
          "Current review requests changes",
          reviewSources,
        )
      : pull.reviewsAvailability !== "complete" || pull.threadsAvailability !== "complete"
      ? directSignal(
          "unknown",
          pull.reviewsAvailability === "unavailable" ||
            pull.threadsAvailability === "unavailable"
            ? "observation-unavailable"
            : "observation-incomplete",
          "Exact-head review observation incomplete",
          reviewSources,
        )
      : pull.threads.some((item) => !item.resolved)
          ? directSignal(
              "unknown",
              "native-thread-unknown",
              "Unresolved thread severity Unknown",
              reviewSources,
            )
          : trustedReviews.some(
                (item) => currentReviews.ids.has(item.id) && item.state === "PENDING",
              )
            ? directSignal(
                "waiting",
                "native-review-pending",
                "Exact-head review pending",
                reviewSources,
              )
            : directSignal(
                "unknown",
                "review-missing",
                "Exact-head review Unknown",
                reviewSources,
              );
  const observedAuthority =
    !roadmapCurrent
      ? directSignal(
          "unknown",
          "observation-incomplete",
          "Product Roadmap authority Unknown",
          [
            evidenceSource(
              "Product Roadmap",
              repository.roadmap?.url ??
                `https://github.com/${repository.repository}/blob/main/${ROADMAP_PATH}`,
            ),
          ],
        )
      : pull.authorityAvailability !== "complete"
      ? directSignal(
          "unknown",
          "observation-incomplete",
          "Authority-path observation Unknown",
          [pullSource],
        )
      : pull.authorityChanges.length === 0
        ? directSignal(
            "satisfied",
            "repository-authority-current",
            "No authority-path drift observed",
            [pullSource],
          )
        : directSignal(
            "unknown",
            "authority-drift-suspected",
            "Changed authority paths need reconciliation",
            pull.authorityChanges.map((change) =>
              evidenceSource(`Authority · ${change.path}`, change.url, "derived"),
            ),
          );
  const implementationState = implementationConflictSignal(
    repository,
    pull,
    implementationOverlap,
  );
  const authority = aggregateSignals(
    [observedAuthority, implementationState],
    "Authority state Unknown",
  );
  const unavailable = directSignal(
    "unknown",
    "required-evidence-unknown",
    "Structured coordination Unknown",
    [pullSource],
  );
  const automatedBrowser = checkSignal(pull, AUTOMATED_BROWSER_CHECK);
  const perceptualReview = checkSignal(pull, PERCEPTUAL_REVIEW_CHECK);
  const mergeability = mergeabilityFor(pull);
  const humanIssueSignals = definiteIssues.map(humanOwnershipFor);
  const humanAction = humanIssueSignals.some((signal) => signal.state === "blocked")
    ? directSignal(
        "blocked",
        "human-action-required",
        "Linked human-owned issue requires attention",
        definiteIssues
          .filter((issue) => ownerFor(issue) === "agent:human")
          .map((issue) => evidenceSource(`Issue #${String(issue.number)}`, issue.url)),
      )
    : unavailable;
  return {
    issue: null,
    owner: "unknown",
    phase: humanAction.state === "blocked"
      ? "human_required"
      : implementationOverlap === "conflict" ||
      [checks, review, authority, mergeability].some(
        (signal) => signal.state === "blocked",
      )
        ? "blocked"
        : "unknown",
    pullRequest: {
      number: pull.number,
      title: pull.title,
      url: pull.url,
      headSha: pull.headSha,
      baseSha: pull.baseSha,
      liveMainSha: repository.main?.sha ?? "",
      mergeBaseSha: pull.mergeBaseSha,
      baseRef: pull.baseRef,
      relationToMain: pull.relationToMain,
      draft: pull.draft,
    },
    readiness: unknownIssue,
    checks,
    review,
    handoff: unavailable,
    stewardWatch: unavailable,
    authority,
    humanAction,
    mergeGate: aggregateSignals(
      [unknownIssue, checks, review, authority, humanAction, mergeability],
      "Merge gate Unknown",
    ),
    evidence: {
      automatedBrowser,
      perceptualReview,
      deliveryIntegrity: unavailable,
    },
    sources: [
      pullSource,
      ...[checks, review, authority, humanAction, mergeability, automatedBrowser, perceptualReview].flatMap(
        (signal) => signal.sources,
      ),
    ],
  };
}

function errorAttention(repository: RepositoryObservation): AttentionItem[] {
  return repository.errors.map((error) => ({
    state: "unknown",
    reason: error.reason,
    label: `${error.source} unavailable`,
    sources: [evidenceSource(error.source, error.url)],
  }));
}

export function normalizeRepository(
  observationInput: RepositoryObservation,
): RepositoryProjection {
  const observation = { ...observationInput };
  delete observation.serverCredential;
  const mainSource = evidenceSource(
    "Live main",
    observation.main?.url ?? `https://github.com/${observation.repository}`,
  );
  const mainSha = observation.main === null
    ? { state: "unknown" as const, value: "Unknown", source: mainSource }
    : { state: "satisfied" as const, value: observation.main.sha, source: mainSource };
  const roadmap = observation.roadmap === null
    ? {
        state: "unknown" as const,
        value: "Unknown",
        source: evidenceSource(
          "Product Roadmap",
          `https://github.com/${observation.repository}/blob/main/${ROADMAP_PATH}`,
        ),
      }
    : parseProductHorizon(observation.roadmap.markdown, observation.roadmap.url);

  const issuesByNumber = new Map(observation.issues.map((issue) => [issue.number, issue]));
  const handoffClaims = observation.pullRequests.map((pull) =>
    trustedHandoffIssueClaims(observation, pull, observation.issues),
  );
  const singlePullIssueNumbers = observation.pullRequests.map((pull, index) =>
    pull.closingIssueNumbers.length === 1
      ? pull.closingIssueNumbers[0]
      : pull.closingIssueNumbers.length === 0 && handoffClaims[index]?.current.length === 1
        ? handoffClaims[index].current[0]
        : undefined,
  );
  const effectiveObservation = handoffClaims.some((claim) => claim.unscoped)
    ? { ...observation, implementationLinkageAvailability: "incomplete" as const }
    : observation;
  const acceptedCurrentHandoffClaims = observation.pullRequests.map((pull, index) =>
    (handoffClaims[index]?.current ?? []).filter(
      (issueNumber) =>
        pull.closingIssueNumbers.length === 0 || pull.closingIssueNumbers.includes(issueNumber),
    ),
  );
  const contradictoryCurrentHandoffClaims = observation.pullRequests.map((pull, index) =>
    (handoffClaims[index]?.current ?? []).filter(
      (issueNumber) =>
        pull.closingIssueNumbers.length > 0 && !pull.closingIssueNumbers.includes(issueNumber),
    ),
  );
  const implementationDefiniteIssueNumbers = observation.pullRequests.map(
    (pull, index) =>
      new Set([
        ...pull.closingIssueNumbers,
        ...(acceptedCurrentHandoffClaims[index] ?? []),
      ]),
  );
  const implementationIssueNumbers = observation.pullRequests.map(
    (_pull, index) =>
      new Set([
        ...(implementationDefiniteIssueNumbers[index] ?? []),
        ...(contradictoryCurrentHandoffClaims[index] ?? []),
        ...(handoffClaims[index]?.stale ?? []),
        ...(handoffClaims[index]?.ambiguous ?? []),
      ]),
  );
  const uncertainIssueNumbers = new Set(
    handoffClaims.flatMap((claim, index) => [
      ...(contradictoryCurrentHandoffClaims[index] ?? []),
      ...claim.stale,
      ...claim.ambiguous,
    ]),
  );
  const implementationCounts = new Map<number, number>();
  for (const issueNumbers of implementationDefiniteIssueNumbers) {
    for (const issueNumber of issueNumbers) {
      implementationCounts.set(issueNumber, (implementationCounts.get(issueNumber) ?? 0) + 1);
    }
  }
  const linkedIssueNumbers = new Set<number>();
  const deliveries: DeliveryLane[] = observation.pullRequests.map((pull, index) => {
    for (const issueNumber of implementationDefiniteIssueNumbers[index] ?? []) {
      linkedIssueNumbers.add(issueNumber);
    }
    const implementationOverlap: ImplementationOverlap = [
      ...(implementationDefiniteIssueNumbers[index] ?? []),
    ].some((issueNumber) => (implementationCounts.get(issueNumber) ?? 0) > 1)
      ? "conflict"
      : [...(implementationIssueNumbers[index] ?? [])].some((issueNumber) =>
            uncertainIssueNumbers.has(issueNumber),
          )
        ? "unknown"
        : "none";
    const issueNumber = singlePullIssueNumbers[index];
    const issue = issueNumber === undefined ? undefined : issuesByNumber.get(issueNumber);
    if (issue === undefined) {
      const definiteIssues = [...(implementationDefiniteIssueNumbers[index] ?? [])].flatMap(
        (number) => {
          const candidate = issuesByNumber.get(number);
          return candidate === undefined ? [] : [candidate];
        },
      );
      return unlinkedPullLane(
        effectiveObservation,
        pull,
        implementationOverlap,
        roadmap.state === "satisfied",
        definiteIssues,
      );
    }
    return pullLane(
      effectiveObservation,
      issue,
      pull,
      implementationOverlap,
      roadmap.state === "satisfied",
      roadmap.state === "satisfied" ? roadmap.value : null,
    );
  });
  const readyOrOwnedIssues = observation.issues.filter(
    (issue) =>
      !linkedIssueNumbers.has(issue.number) &&
      (issue.labels.some((label) => label.startsWith("agent:")) ||
        issue.labels.includes("state:ready")),
  );
  deliveries.push(
    ...readyOrOwnedIssues.map((issue) =>
      issueLane(
        effectiveObservation,
        issue,
        roadmap.state === "satisfied",
        roadmap.state === "satisfied" ? roadmap.value : null,
        uncertainIssueNumbers.has(issue.number),
      ),
    ),
  );
  deliveries.sort((left, right) => {
    const leftIdentity = left.issue?.number ?? Number.MAX_SAFE_INTEGER;
    const rightIdentity = right.issue?.number ?? Number.MAX_SAFE_INTEGER;
    if (leftIdentity !== rightIdentity) return leftIdentity - rightIdentity;
    return (left.pullRequest?.number ?? 0) - (right.pullRequest?.number ?? 0);
  });
  const laneIssues = observation.issues
    .filter(
      (issue) =>
        linkedIssueNumbers.has(issue.number) ||
        issue.labels.some((label) => label.startsWith("agent:")) ||
        issue.labels.includes("state:ready"),
    )
    .sort((left, right) => left.number - right.number);
  const attention: AttentionItem[] = [];
  const attentionKeys = new Set<string>();
  for (const item of errorAttention(observation)) {
    const key = attentionKey(item);
    if (attentionKeys.has(key)) continue;
    attentionKeys.add(key);
    attention.push(item);
  }
  for (const lane of deliveries) {
    for (const signal of [
      lane.readiness,
      lane.checks,
      lane.authority,
      lane.handoff,
      lane.stewardWatch,
      lane.review,
      lane.mergeGate,
      lane.humanAction,
    ]) {
      if (
        signal.reason !== "not-required" &&
        (signal.state === "blocked" || signal.state === "waiting" || signal.state === "unknown")
      ) {
        const item: AttentionItem = {
          ...signal,
          ...(lane.issue === null ? {} : { issueNumber: lane.issue.number }),
        };
        const key = attentionKey(item);
        if (!attentionKeys.has(key)) {
          attentionKeys.add(key);
          attention.push(item);
        }
      }
    }
  }
  const humanSignals = deliveries.map((lane) => lane.humanAction);
  const humanAction =
    humanSignals.some((signal) => signal.state === "blocked")
      ? directSignal("blocked", "human-action-required", "Human action required", [])
      : observation.availability !== "complete" || observation.issuesAvailability !== "complete"
        ? directSignal("unknown", "observation-incomplete", "Human action state Unknown", [mainSource])
        : humanSignals.some((signal) => signal.state === "unknown")
          ? directSignal("unknown", "required-evidence-unknown", "Human action state Unknown", [])
          : directSignal("satisfied", "human-action-none", "No human action required", []);
  const sourceAvailability = [
    observation.availability,
    observation.issuesAvailability,
    observation.pullsAvailability,
    observation.recentActivityAvailability,
  ];
  const fetchHealth = sourceAvailability.every((value) => value === "complete")
    ? "healthy"
    : sourceAvailability.every((value) => value === "unavailable")
      ? "unavailable"
      : "partial";
  const linkageSource = evidenceSource(
    "Open pull request linkage",
    `https://github.com/${observation.repository}/pulls`,
  );
  const issuesSource = evidenceSource(
    "Open Issue readiness",
    `https://github.com/${observation.repository}/issues`,
  );
  const countValue = (value: number, complete: boolean, source: SourceLink) =>
    complete
      ? { state: "satisfied" as const, value, source }
      : { state: "unknown" as const, value: "Unknown" as const, source };
  const linkageComplete = effectiveObservation.implementationLinkageAvailability === "complete";
  const issuesComplete =
    observation.issuesAvailability === "complete" &&
    observation.issues.every(
      (issue) =>
        issue.labelsAvailability === "complete" &&
        issue.dependencyAvailability === "complete",
    );

  return {
    repository: observation.repository,
    observedAt: observation.observedAt,
    fetchHealth,
    executive: {
      mainSha,
      productHorizon: roadmap,
      activeCount: countValue(
        deliveries.filter((lane) => lane.pullRequest !== null).length,
        linkageComplete,
        linkageSource,
      ),
      readyCount: countValue(
        deliveries.filter(
          (lane) =>
            lane.pullRequest === null &&
            lane.readiness.state === "satisfied" &&
            (lane.issue === null || !uncertainIssueNumbers.has(lane.issue.number)),
        ).length,
        linkageComplete && issuesComplete && roadmap.state === "satisfied",
        issuesSource,
      ),
    },
    deliveries,
    criticalPath: {
      nodes: laneIssues.map((issue) => {
        const readiness = aggregateSignals(
          [
            readinessFor(issue),
            roadmapAlignmentFor(
              issue,
              roadmap.state === "satisfied" ? roadmap.value : null,
            ),
          ],
          "Issue readiness Unknown",
        );
        return {
          issueNumber: issue.number,
          label: issue.title,
          state: readiness.state === "blocked"
            ? "blocked"
            : uncertainIssueNumbers.has(issue.number)
              ? "unknown"
            : readiness.state === "satisfied"
            ? "ready"
            : readiness.state === "waiting"
              ? "waiting"
              : "unknown",
          url: issue.url,
        };
      }),
      edges: laneIssues.flatMap((issue) =>
        issue.blockedBy.map((dependency) => ({
          from: dependency.number,
          to: issue.number,
          state: dependency.state === "CLOSED" ? "satisfied" as const : "waiting" as const,
        })),
      ),
    },
    recentActivity: [...observation.recentActivity]
      .sort((left, right) => right.mergedAt.localeCompare(left.mergedAt))
      .slice(0, 8),
    attention,
    humanAction,
    sources: [mainSource, roadmap.source],
  };
}
