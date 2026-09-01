import {
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
    reasonLabels[condition.reason] ?? fallbackLabel,
    sources,
  );
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
  if (checks.some((check) => check.status === "failure")) {
    return directSignal("blocked", "native-check-failed", "Exact-head check failed", sources);
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

function readinessFor(issue: RawIssue): DisplaySignal {
  const source = evidenceSource("Issue labels", issue.url);
  if (issue.labelsAvailability !== "complete") {
    return directSignal("unknown", "observation-incomplete", "Issue labels Unknown", [source]);
  }
  if (issue.dependencyAvailability !== "complete") {
    return directSignal("unknown", "observation-incomplete", "Dependency state Unknown", [source]);
  }
  if (issue.blockedBy.some((dependency) => dependency.state === "OPEN")) {
    return directSignal("waiting", "dependency-waiting", "Waiting on dependency", [
      source,
      ...issue.blockedBy.map((dependency) =>
        evidenceSource(`Issue #${String(dependency.number)}`, dependency.url),
      ),
    ]);
  }
  if (
    issue.labels.includes("state:blocked") ||
    issue.labels.includes("state:parked")
  ) {
    return directSignal("blocked", "issue-not-ready", "Issue is not Ready", [source]);
  }
  if (issue.labels.includes("state:ready")) {
    return directSignal("satisfied", "all-required-conditions-satisfied", "Issue Ready", [source]);
  }
  return directSignal(
    "unknown",
    "required-evidence-unknown",
    "Issue readiness Unknown",
    [source],
  );
}

function aggregateSignals(signals: DisplaySignal[], fallback: string): DisplaySignal {
  const state = signals.some((signal) => signal.state === "blocked")
    ? "blocked"
    : signals.some((signal) => signal.state === "waiting")
      ? "waiting"
      : signals.some((signal) => signal.state === "unknown")
        ? "unknown"
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
      edited: comment.edited,
      topLevel: comment.topLevel,
      trustedProducer: comment.trustedProducer,
    },
  }));
}

function pullLane(
  repository: RepositoryObservation,
  issue: RawIssue,
  pull: RawPullRequest,
): DeliveryLane {
  const mainSha = repository.main?.sha ?? "";
  const githubUrl = pull.url;
  const readiness = readinessFor(issue);
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
      pull.reviewsAvailability,
      pull.reviews.map((review) => ({
        current: review.commitSha === pull.headSha,
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
        issue.labelsAvailability !== "complete" ||
        issue.dependencyAvailability !== "complete" ||
        readiness.state === "unknown"
        ? "incomplete"
        : "complete",
      [
        {
          issueReady: readiness.state === "satisfied",
          dependencies: issue.blockedBy.some((dependency) => dependency.state === "OPEN")
            ? "waiting"
            : "satisfied",
          pullRequestState: pull.state,
          pullRequestDraft: pull.draft,
          baseRef: pull.baseRef,
          authorityConflict: false,
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
    [fromCondition(reconciliation.humanAction, "Human action Unknown"), commentState, labelState],
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
  const mergeGate = aggregateSignals(
    [fromCondition(reconciliation.mergeGate, "Merge gate Unknown"), checks, commentState],
    "Merge gate Unknown",
  );
  const authority =
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
  const phase = humanAction.state === "blocked"
    ? "human_required"
    : review.state === "blocked"
      ? "review_fix"
      : [readiness, checks, stewardWatch].some((signal) => signal.state === "blocked")
        ? "blocked"
        : readiness.state === "waiting"
          ? "waiting"
          : mergeGate.state === "satisfied"
            ? "merge_gate"
            : readiness.state === "unknown"
              ? "unknown"
              : authority.reason === "authority-drift-suspected"
                ? "rereview"
                : checks.state === "waiting"
                  ? "validating"
                  : checks.state === "satisfied" && review.state !== "satisfied"
                    ? "rereview"
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

function issueLane(repository: RepositoryObservation, issue: RawIssue): DeliveryLane {
  const readiness = readinessFor(issue);
  const owner = ownerFor(issue);
  const unavailable = directSignal(
    "unknown",
    "not-required",
    "No pull request yet",
    [evidenceSource(`Issue #${String(issue.number)}`, issue.url)],
  );
  return {
    issue: { number: issue.number, title: issue.title, url: issue.url },
    owner,
    phase: readiness.state === "satisfied" ? "ready" : readiness.state,
    pullRequest: null,
    readiness,
    checks: unavailable,
    review: unavailable,
    handoff: unavailable,
    stewardWatch: unavailable,
    authority: repository.main === null
      ? directSignal("unknown", "observation-unavailable", "Authority Unknown", [])
      : directSignal("satisfied", "repository-authority-current", "Authority observed", [
          evidenceSource("Live main", repository.main.url),
        ]),
    humanAction:
      issue.labelsAvailability !== "complete"
        ? directSignal(
            "unknown",
            "observation-incomplete",
            "Issue ownership Unknown",
            [evidenceSource("Issue owner label", issue.url)],
          )
        : owner === "agent:human"
        ? directSignal(
            "blocked",
            "human-action-required",
            "Human-owned issue requires attention",
            [evidenceSource("Issue owner label", issue.url)],
          )
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
): DeliveryLane {
  const pullSource = evidenceSource(`PR #${String(pull.number)}`, pull.url);
  const unknownIssue = directSignal(
    "unknown",
    "source-identity-conflict",
    "Native Issue linkage Unknown",
    [pullSource],
  );
  const checks = checkSignal(pull);
  const reviewSources = [
    ...pull.reviews.map((item) => evidenceSource("Native review", item.url)),
    ...pull.threads.map((item) => evidenceSource("Native review thread", item.url)),
  ];
  const review =
    pull.reviewsAvailability !== "complete" || pull.threadsAvailability !== "complete"
      ? directSignal(
          "unknown",
          "observation-incomplete",
          "Exact-head review observation incomplete",
          reviewSources,
        )
      : pull.reviews.some(
            (item) =>
              item.commitSha === pull.headSha && item.state === "CHANGES_REQUESTED",
          )
        ? directSignal(
            "blocked",
            "native-changes-requested",
            "Changes requested on exact head",
            reviewSources,
          )
        : pull.threads.some((item) => !item.resolved)
          ? directSignal(
              "unknown",
              "native-thread-unknown",
              "Unresolved thread severity Unknown",
              reviewSources,
            )
          : pull.reviews.some(
                (item) => item.commitSha === pull.headSha && item.state === "PENDING",
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
  const authority =
    pull.authorityAvailability !== "complete"
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
  const unavailable = directSignal(
    "unknown",
    "required-evidence-unknown",
    "Structured coordination Unknown",
    [pullSource],
  );
  const automatedBrowser = checkSignal(pull, AUTOMATED_BROWSER_CHECK);
  const perceptualReview = checkSignal(pull, PERCEPTUAL_REVIEW_CHECK);
  return {
    issue: null,
    owner: "unknown",
    phase: "unknown",
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
    humanAction: unavailable,
    mergeGate: aggregateSignals([unknownIssue, checks, review, authority], "Merge gate Unknown"),
    evidence: {
      automatedBrowser,
      perceptualReview,
      deliveryIntegrity: unavailable,
    },
    sources: [
      pullSource,
      ...[checks, review, authority, automatedBrowser, perceptualReview].flatMap(
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
  const linkedIssueNumbers = new Set<number>();
  const deliveries: DeliveryLane[] = observation.pullRequests.map((pull) => {
    const issueNumber =
      pull.closingIssueNumbers.length === 1 ? pull.closingIssueNumbers[0] : undefined;
    const issue = issueNumber === undefined ? undefined : issuesByNumber.get(issueNumber);
    if (issue === undefined) return unlinkedPullLane(observation, pull);
    linkedIssueNumbers.add(issue.number);
    return pullLane(observation, issue, pull);
  });
  const readyOrOwnedIssues = observation.issues.filter(
    (issue) =>
      !linkedIssueNumbers.has(issue.number) &&
      (issue.labels.some((label) => label.startsWith("agent:")) ||
        issue.labels.includes("state:ready")),
  );
  deliveries.push(...readyOrOwnedIssues.map((issue) => issueLane(observation, issue)));
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
  const attention: AttentionItem[] = [...errorAttention(observation)];
  const attentionKeys = new Set(
    attention.map((item) => `${String(item.issueNumber ?? "repository")}:${item.reason}:${item.label}`),
  );
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
        const key = `${String(item.issueNumber ?? "repository")}:${item.reason}:${item.label}`;
        if (!attentionKeys.has(key)) {
          attentionKeys.add(key);
          attention.push(item);
        }
      }
    }
  }
  const humanSignals = deliveries.map((lane) => lane.humanAction);
  const humanAction =
    observation.availability !== "complete" || observation.issuesAvailability !== "complete"
      ? directSignal("unknown", "observation-incomplete", "Human action state Unknown", [mainSource])
      : humanSignals.some((signal) => signal.state === "blocked")
        ? directSignal("blocked", "human-action-required", "Human action required", [])
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

  return {
    repository: observation.repository,
    observedAt: observation.observedAt,
    fetchHealth,
    executive: {
      mainSha,
      productHorizon: roadmap,
      activeCount: deliveries.filter((lane) => lane.pullRequest !== null).length,
      readyCount: deliveries.filter(
        (lane) => lane.pullRequest === null && lane.readiness.state === "satisfied",
      ).length,
    },
    deliveries,
    criticalPath: {
      nodes: laneIssues.map((issue) => {
        const readiness = readinessFor(issue);
        return {
          issueNumber: issue.number,
          label: issue.title,
          state: readiness.state === "satisfied"
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
