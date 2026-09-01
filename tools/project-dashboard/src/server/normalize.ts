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
  RawCheck,
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
const AUTOMATED_BROWSER_CHECK = "project-dashboard-browser";
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
  if (availability === "unavailable") {
    return { availability, facts, source: { id, url } };
  }
  return { availability, facts, source: { id, url } };
}

function checkSignal(pull: RawPullRequest): DisplaySignal {
  const sources = pull.checks.map((check) =>
    evidenceSource(`Check · ${check.name}`, check.url),
  );
  if (pull.checksAvailability !== "complete" || pull.checks.length === 0) {
    return directSignal(
      "unknown",
      pull.checksAvailability === "unavailable"
        ? "observation-unavailable"
        : "observation-incomplete",
      "Exact-head checks Unknown",
      sources,
    );
  }
  if (pull.checks.some((check) => check.headSha !== pull.headSha)) {
    return directSignal("unknown", "validation-stale", "Check identity mismatch", sources);
  }
  if (pull.checks.some((check) => check.status === "failure")) {
    return directSignal("blocked", "native-check-failed", "Exact-head check failed", sources);
  }
  if (pull.checks.some((check) => check.status === "pending")) {
    return directSignal("waiting", "native-check-pending", "Exact-head checks pending", sources);
  }
  return directSignal("satisfied", "native-check-succeeded", "Exact-head checks passed", sources);
}

function evidenceState(
  checks: RawCheck[],
  availability: RawPullRequest["checksAvailability"],
  headSha: string,
  name: string,
): DisplayState {
  if (availability !== "complete") return "unknown";
  const matches = checks.filter((check) => check.name === name);
  if (matches.length === 0) return "unknown";
  if (matches.some((check) => check.headSha !== headSha)) return "unknown";
  if (matches.some((check) => check.status === "failure")) return "blocked";
  if (matches.some((check) => check.status === "pending")) return "waiting";
  return "satisfied";
}

function ownerFor(issue: RawIssue): string {
  const owners = issue.labels.filter((label) => label.startsWith("agent:"));
  return owners.length === 1 ? owners.at(0) ?? "unknown" : "unknown";
}

function readinessFor(issue: RawIssue): DisplaySignal {
  const source = evidenceSource("Issue labels", issue.url);
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
  if (!issue.labels.includes("state:ready")) {
    return directSignal("blocked", "issue-not-ready", "Issue is not Ready", [source]);
  }
  return directSignal("satisfied", "all-required-conditions-satisfied", "Issue Ready", [source]);
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
      repository.main === null || issue.dependencyAvailability !== "complete"
        ? "incomplete"
        : "complete",
      [
        {
          issueReady: issue.labels.includes("state:ready"),
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
  const readiness = readinessFor(issue);
  const checks = checkSignal(pull);
  const review = fromCondition(reconciliation.review, "Exact-head review Unknown");
  const mergeGate = fromCondition(reconciliation.mergeGate, "Merge gate Unknown");
  const authority =
    pull.relationToMain === "current"
      ? fromCondition(reconciliation.authority, "Authority state Unknown")
      : directSignal(
          "unknown",
          "authority-reconciliation-needed",
          "Live-main reconciliation needed",
          [
            evidenceSource("Pull request relation", pull.url, "derived"),
            ...(repository.main === null
              ? []
              : [evidenceSource("Live main", repository.main.url)]),
          ],
        );
  const phase =
    mergeGate.state === "satisfied"
      ? "merge_gate"
      : review.state === "blocked"
        ? "review_fix"
        : checks.state === "waiting"
          ? "validating"
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
    handoff: fromCondition(reconciliation.handoff, "Handoff Unknown"),
    stewardWatch: fromCondition(reconciliation.watch, "Steward watch Unknown"),
    authority,
    humanAction: fromCondition(reconciliation.humanAction, "Human action Unknown"),
    mergeGate,
    evidence: {
      automatedBrowser: evidenceState(
        pull.checks,
        pull.checksAvailability,
        pull.headSha,
        AUTOMATED_BROWSER_CHECK,
      ),
      perceptualReview: evidenceState(
        pull.checks,
        pull.checksAvailability,
        pull.headSha,
        PERCEPTUAL_REVIEW_CHECK,
      ),
      deliveryIntegrity: reconciliation.validations[0]?.state ?? "unknown",
    },
    sources: [
      evidenceSource(`Issue #${String(issue.number)}`, issue.url),
      evidenceSource(`PR #${String(pull.number)}`, pull.url),
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
      owner === "agent:human"
        ? directSignal(
            "blocked",
            "human-action-required",
            "Human-owned issue requires attention",
            [evidenceSource("Issue owner label", issue.url)],
          )
        : directSignal("satisfied", "not-required", "No human action required", []),
    mergeGate: unavailable,
    evidence: {
      automatedBrowser: "unknown",
      perceptualReview: "unknown",
      deliveryIntegrity: "unknown",
    },
    sources: [evidenceSource(`Issue #${String(issue.number)}`, issue.url)],
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

  const laneIssues = observation.issues
    .filter((issue) => issue.labels.some((label) => label.startsWith("agent:")))
    .sort((left, right) => left.number - right.number);
  const deliveries = laneIssues.map((issue) => {
    const matches = observation.pullRequests.filter((pull) =>
      pull.closingIssueNumbers.includes(issue.number),
    );
    const pull = matches.at(0);
    return matches.length === 1 && pull !== undefined
      ? pullLane(observation, issue, pull)
      : issueLane(observation, issue);
  });
  const attention: AttentionItem[] = [...errorAttention(observation)];
  for (const lane of deliveries) {
    for (const signal of [
      lane.authority,
      lane.handoff,
      lane.stewardWatch,
      lane.review,
      lane.humanAction,
    ]) {
      if (
        signal.reason !== "not-required" &&
        (signal.state === "blocked" || signal.state === "unknown")
      ) {
        attention.push({ ...signal, issueNumber: lane.issue.number });
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
      nodes: laneIssues.map((issue) => ({
        issueNumber: issue.number,
        label: issue.title,
        state: issue.blockedBy.some((dependency) => dependency.state === "OPEN")
          ? "waiting"
          : issue.labels.includes("state:ready")
            ? "ready"
            : "unknown",
        url: issue.url,
      })),
      edges: laneIssues.flatMap((issue) =>
        issue.blockedBy.map((dependency) => ({
          from: dependency.number,
          to: issue.number,
          state: dependency.state === "CLOSED" ? "satisfied" as const : "waiting" as const,
        })),
      ),
    },
    recentActivity: observation.recentActivity.slice(0, 8),
    attention,
    humanAction,
    sources: [mainSource, roadmap.source],
  };
}
