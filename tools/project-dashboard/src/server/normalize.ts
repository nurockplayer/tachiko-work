import type {
  CheckProjection,
  DeliveryLane,
  DeliveryPhase,
  HandoffProjection,
  RawComment,
  RawIssue,
  RawPullRequest,
  RawRepositorySnapshot,
  RepositoryProjection,
  ReviewProjection,
  SourceClass,
  SourceRef,
} from "../shared/types.ts";

const handoffMarker = "<!-- agent-handoff:v1 -->";
const substantiveFinding = /(?:\[|\b)(?:p[0-2]|sev(?:erity)?[ -]?[0-2]|blocking|security|correctness)(?:\]|\b)/i;

function source(
  className: SourceClass,
  label: string,
  url: string,
  observedAt: string,
  observedIdentity: string | null = null,
): SourceRef {
  return { class: className, label, url, observedAt, observedIdentity };
}

function stripMarkdown(value: string): string {
  return value
    .replaceAll("**", "")
    .replaceAll("`", "")
    .replace(/^[-#>\s]+/, "")
    .trim();
}

function labeledValue(body: string, label: string): string | null {
  const escaped = label.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = body.match(new RegExp(`^\\s*(?:[-*]\\s*)?(?:\\*\\*)?${escaped}(?:\\*\\*)?\\s*:\\s*(.+?)\\s*$`, "im"));
  return match?.[1] === undefined ? null : stripMarkdown(match[1]);
}

function normalizedSha(value: string | null): string | null {
  if (value === null) return null;
  const match = value.match(/\b[0-9a-f]{7,40}\b/i);
  return match?.[0]?.toLowerCase() ?? null;
}

function shaMatches(claimed: string | null, actual: string | null): boolean {
  if (claimed === null || actual === null) return false;
  return actual.startsWith(claimed) || claimed.startsWith(actual);
}

function canonicalComments(comments: RawComment[]): RawComment[] {
  return comments.filter((comment) => comment.body.includes(handoffMarker));
}

function projectHandoff(
  comments: RawComment[],
  observedAt: string,
  currentHeadSha: string | null,
  liveMainSha: string | null,
): HandoffProjection {
  const canonical = canonicalComments(comments);
  if (canonical.length === 0) {
    return {
      condition: "missing",
      claimedOwner: null,
      claimedState: null,
      claimedHeadSha: null,
      lastCheckedMainSha: null,
      updatedAt: null,
      sourceRefs: [],
    };
  }

  const latest = canonical.toSorted((left, right) => right.updatedAt.localeCompare(left.updatedAt))[0];
  if (latest === undefined) {
    throw new Error("Canonical handoff selection unexpectedly failed");
  }

  const claimedHeadSha = normalizedSha(labeledValue(latest.body, "HEAD") ?? labeledValue(latest.body, "EXACT HEAD"));
  const lastCheckedMainSha = normalizedSha(
    labeledValue(latest.body, "LAST CHECKED MAIN") ?? labeledValue(latest.body, "MAIN"),
  );
  let condition: HandoffProjection["condition"] = "current";

  if (canonical.length !== 1) {
    condition = "inconsistent";
  } else if (currentHeadSha !== null && !shaMatches(claimedHeadSha, currentHeadSha)) {
    condition = "inconsistent";
  } else if (
    liveMainSha !== null &&
    lastCheckedMainSha !== null &&
    !shaMatches(lastCheckedMainSha, liveMainSha)
  ) {
    condition = "stale";
  } else if (liveMainSha !== null && lastCheckedMainSha === null) {
    condition = "unknown";
  }

  return {
    condition,
    claimedOwner: labeledValue(latest.body, "OWNER"),
    claimedState: labeledValue(latest.body, "STATE") ?? labeledValue(latest.body, "STATUS"),
    claimedHeadSha,
    lastCheckedMainSha,
    updatedAt: latest.updatedAt,
    sourceRefs: [source("direct", "Canonical handoff", latest.url, observedAt, latest.id)],
  };
}

function issueOwner(issue: RawIssue, handoff: HandoffProjection): string {
  if (handoff.claimedOwner !== null) return handoff.claimedOwner;
  return labeledValue(issue.body, "Owner") ?? "unknown";
}

function issueReadiness(issue: RawIssue, handoff: HandoffProjection): DeliveryLane["issue"]["readiness"] {
  if ((issue.blockedBy?.length ?? 0) > 0) return "blocked";
  const statusSection = issue.body.match(/## Status\s*([\s\S]*?)(?=\n## |$)/i)?.[1] ?? issue.body.slice(0, 600);
  const currentHandoffState = handoff.condition === "current" ? handoff.claimedState : null;
  const statusText = (currentHandoffState ?? statusSection).toLowerCase();
  if (/\bpark(?:ed)?\b/.test(statusText)) return "parked";
  if (/\bblock(?:ed)?\b/.test(statusText) && !statusText.includes("not blocked")) return "blocked";
  if (/\bactive\b|\bimplementing\b|\bin progress\b|\bvalidating\b|\breview[_ -]?fix\b|\bhuman[_ -]?required\b/.test(statusText)) return "active";
  if (/\bready\b/.test(statusText) && !/not ready for (?:production )?implementation/.test(statusText)) return "ready";
  return "unknown";
}

function humanActionRequested(comments: RawComment[], handoff: HandoffProjection): boolean {
  if (handoff.condition !== "current") return false;
  if (/human[_ -]?required/i.test(handoff.claimedState ?? "")) return true;
  const latest = canonicalComments(comments).toSorted((left, right) => right.updatedAt.localeCompare(left.updatedAt))[0];
  if (latest === undefined) return false;
  const claim =
    labeledValue(latest.body, "HUMAN ACTION") ??
    labeledValue(latest.body, "FOUNDER / STEWARD ACTION") ??
    labeledValue(latest.body, "STEWARD ACTION") ??
    labeledValue(latest.body, "ESCALATION");
  if (claim === null || /^(?:none|no|not required|n\/a)\b/i.test(claim)) return false;
  return /\b(?:required|needed|decision|action)\b/i.test(claim);
}

function projectChecks(pr: RawPullRequest, observedAt: string): CheckProjection {
  const refs = [
    source("direct", `PR #${pr.number} exact-head checks`, `${pr.url}/checks`, observedAt, pr.checksObservedHeadSha),
    source("direct", `Required checks for ${pr.baseRefName}`, `${pr.url.split("/pull/")[0] ?? pr.url}/rules`, observedAt, pr.baseSha),
  ];
  if (pr.checks === null || !shaMatches(pr.checksObservedHeadSha, pr.headSha)) {
    return {
      status: "unknown",
      requiredStatus: "unknown",
      observedHeadSha: pr.checksObservedHeadSha,
      summary: "Checks were not observed for the current PR head.",
      requiredSummary: "Required checks were not observed for the current PR head.",
      sourceRefs: refs,
    };
  }
  const required = projectRequiredChecks(pr);
  if (pr.checks.length === 0) {
    return {
      status: "unknown",
      ...required,
      observedHeadSha: pr.checksObservedHeadSha,
      summary: "No check data was returned.",
      sourceRefs: refs,
    };
  }

  const failure = pr.checks.find(
    (check) => check.status === "completed" && check.conclusion !== null && !["success", "neutral", "skipped"].includes(check.conclusion),
  );
  if (failure !== undefined) {
    return {
      status: "failure",
      ...required,
      observedHeadSha: pr.checksObservedHeadSha,
      summary: `${failure.name} concluded ${failure.conclusion ?? "unknown"}.`,
      sourceRefs: refs,
    };
  }

  const pending = pr.checks.find((check) => check.status !== "completed" || check.conclusion === null);
  if (pending !== undefined) {
    return {
      status: "pending",
      ...required,
      observedHeadSha: pr.checksObservedHeadSha,
      summary: `${pending.name} is ${pending.status.replaceAll("_", " ")}.`,
      sourceRefs: refs,
    };
  }

  return {
    status: "success",
    ...required,
    observedHeadSha: pr.checksObservedHeadSha,
    summary: `${pr.checks.length} exact-head check${pr.checks.length === 1 ? "" : "s"} passed.`,
    sourceRefs: refs,
  };
}

function projectRequiredChecks(
  pr: RawPullRequest,
): Pick<CheckProjection, "requiredStatus" | "requiredSummary"> {
  if (pr.requiredChecks === null) {
    return { requiredStatus: "unknown", requiredSummary: "The required-check set could not be observed." };
  }
  if (pr.requiredChecks.length === 0) {
    return { requiredStatus: "satisfied", requiredSummary: `No required status checks apply to ${pr.baseRefName}.` };
  }

  for (const required of pr.requiredChecks) {
    const observed = pr.checks?.find(
      (check) => check.name === required.name &&
        (required.integrationId === null || check.integrationId === required.integrationId),
    );
    if (observed === undefined) {
      return {
        requiredStatus: "unsatisfied",
        requiredSummary: `Required check ${required.name} was not observed for the current PR head.`,
      };
    }
    if (
      observed.status !== "completed" ||
      observed.conclusion === null ||
      !["success", "neutral", "skipped"].includes(observed.conclusion)
    ) {
      return {
        requiredStatus: "unsatisfied",
        requiredSummary: `Required check ${required.name} has not completed successfully for the current PR head.`,
      };
    }
  }

  return {
    requiredStatus: "satisfied",
    requiredSummary: `${pr.requiredChecks.length} required check${pr.requiredChecks.length === 1 ? "" : "s"} satisfied for the current PR head.`,
  };
}

function projectReviews(pr: RawPullRequest, observedAt: string): ReviewProjection {
  const refs = [source("direct", `PR #${pr.number} reviews`, `${pr.url}/reviews`, observedAt, pr.headSha)];
  if (pr.reviews === null || pr.reviewThreads === null) {
    return {
      decision: "unknown",
      status: "unknown",
      reviewedHeadSha: null,
      unresolvedThreadCount: null,
      substantiveUnresolvedCount: null,
      sourceRefs: refs,
    };
  }

  const relevantReviews = pr.reviews
    .filter((review) => review.state === "approved" || review.state === "changes_requested")
    .toSorted((left, right) => right.submittedAt.localeCompare(left.submittedAt));
  const reviewedHeadSha = relevantReviews[0]?.headSha ?? null;
  const unresolved = pr.reviewThreads.filter((thread) => !thread.resolved && !thread.outdated);
  const status = reviewedHeadSha === null ? (pr.reviewDecision === "review_required" ? "current" : "unknown") : shaMatches(reviewedHeadSha, pr.headSha) ? "current" : "stale";

  return {
    decision: pr.reviewDecision,
    status,
    reviewedHeadSha,
    unresolvedThreadCount: unresolved.length,
    substantiveUnresolvedCount: unresolved.filter((thread) => substantiveFinding.test(thread.body)).length,
    sourceRefs: refs,
  };
}

function authorityDrift(
  pr: RawPullRequest,
  handoff: HandoffProjection,
  mainSha: string | null,
): DeliveryLane["authorityDrift"] {
  if (mainSha === null) return "unknown";
  if (pr.authorityPathsChangedOnMain === null) return "unknown";
  if (handoff.lastCheckedMainSha !== null && !shaMatches(handoff.lastCheckedMainSha, mainSha)) return "suspected";
  if (pr.relationToMain === "diverged" || pr.relationToMain === "behind") return "suspected";
  if (pr.relationToMain === "unknown") return "unknown";
  return "none";
}

function derivePhase(
  readiness: DeliveryLane["issue"]["readiness"],
  pr: RawPullRequest | null,
  checks: CheckProjection,
  reviews: ReviewProjection,
  handoff: HandoffProjection,
): DeliveryPhase {
  const claimedState = handoff.condition === "current" ? handoff.claimedState?.toLowerCase() ?? "" : "";
  if (readiness === "parked" || claimedState.includes("parked")) return "parked";
  if (claimedState.includes("human_required") || claimedState.includes("human required")) return "human_required";
  if (readiness === "blocked") return "blocked";
  if (pr === null) return readiness === "ready" ? "ready" : readiness === "active" ? "implementing" : "unknown";
  if ((reviews.substantiveUnresolvedCount ?? 0) > 0) return "review_fix";
  if (reviews.status === "stale") return "rereview";
  if (reviews.decision === "changes_requested") return "review_fix";
  if (checks.status !== "success" || checks.requiredStatus !== "satisfied") return "validating";
  if (reviews.decision !== "approved") return "rereview";
  if (handoff.condition !== "current") return "validating";
  return "merge_gate";
}

function projectLane(
  issue: RawIssue,
  pr: RawPullRequest | null,
  snapshot: RawRepositorySnapshot,
): DeliveryLane {
  const comments = pr?.comments ?? issue.comments;
  const handoff = projectHandoff(comments, snapshot.observedAt, pr?.headSha ?? null, snapshot.mainSha);
  const observedReadiness = issueReadiness(issue, handoff);
  const readiness = pr === null || observedReadiness === "blocked" || observedReadiness === "parked"
    ? observedReadiness
    : "active";
  const checks =
    pr === null
      ? {
          status: "unknown" as const,
          requiredStatus: "unknown" as const,
          observedHeadSha: null,
          summary: "No pull request exists for this lane.",
          requiredSummary: "No pull request exists for this lane.",
          sourceRefs: [],
        }
      : projectChecks(pr, snapshot.observedAt);
  const reviews =
    pr === null
      ? {
          decision: "unknown" as const,
          status: "unknown" as const,
          reviewedHeadSha: null,
          unresolvedThreadCount: null,
          substantiveUnresolvedCount: null,
          sourceRefs: [],
        }
      : projectReviews(pr, snapshot.observedAt);
  const drift = pr === null ? "none" : authorityDrift(pr, handoff, snapshot.mainSha);
  const blockers: string[] = [];

  if ((issue.blockedBy?.length ?? 0) > 0) {
    blockers.push(`Live Issue dependencies block this lane: ${issue.blockedBy?.map((dependency) => `#${dependency.number}`).join(", ") ?? "Unknown"}.`);
  }
  if (handoff.condition === "current" && observedReadiness === "blocked" && (issue.blockedBy?.length ?? 0) === 0) {
    blockers.push("The current canonical handoff reports this lane blocked.");
  }
  if (pr !== null && checks.status === "unknown") blockers.push("Checks were not observed for the current PR head.");
  if (pr !== null && checks.requiredStatus !== "satisfied") blockers.push(checks.requiredSummary);
  if (checks.status === "failure") blockers.push(checks.summary);
  if ((reviews.substantiveUnresolvedCount ?? 0) > 0) {
    blockers.push(`${reviews.substantiveUnresolvedCount ?? 0} substantive review finding(s) remain unresolved.`);
  }
  if (reviews.status === "stale") blockers.push("The latest substantive review does not describe the current PR head.");
  if (handoff.condition === "inconsistent") blockers.push("Canonical handoff conflicts with live PR identity or is duplicated.");
  if (handoff.condition === "stale") blockers.push("Canonical handoff has not reconciled the observed live main.");
  const changedAuthorityPaths = pr?.authorityPathsChangedOnMain ?? [];
  if (drift === "suspected" && changedAuthorityPaths.length > 0) {
    blockers.push(`Accepted-authority candidates changed on main: ${changedAuthorityPaths.join(", ")}.`);
  }
  if (drift === "suspected") blockers.push("Authority or live-main drift requires explicit reconciliation.");

  const phase = derivePhase(readiness, pr, checks, reviews, handoff);
  const requiresHuman = humanActionRequested(comments, handoff);
  const action: DeliveryLane["action"] = requiresHuman || phase === "human_required"
    ? { owner: "human", reason: "The canonical coordination state requests human or Steward action." }
    : phase === "review_fix" || checks.status === "failure"
      ? { owner: "codex", reason: blockers[0] ?? "Delivery-agent action is required." }
      : { owner: "none", reason: "No human action is currently evidenced." };
  const issueRef = source("direct", `Issue #${issue.number}`, issue.url, snapshot.observedAt, `issue-${issue.number}`);
  const sourceRefs = [issueRef];
  if (pr !== null) sourceRefs.push(source("direct", `Pull request #${pr.number}`, pr.url, snapshot.observedAt, pr.headSha));
  sourceRefs.push(
    source(
      "derived",
      "Delivery phase and gate reconciliation",
      pr?.url ?? issue.url,
      snapshot.observedAt,
      pr?.headSha ?? snapshot.mainSha,
    ),
  );
  if ((reviews.unresolvedThreadCount ?? 0) > 0) {
    sourceRefs.push(
      source(
        "heuristic",
        "Review severity classification",
        pr?.url ?? issue.url,
        snapshot.observedAt,
        pr?.headSha ?? null,
      ),
    );
  }

  return {
    id: `issue-${issue.number}`,
    issue: {
      number: issue.number,
      title: issue.title,
      url: issue.url,
      readiness,
      milestone: issue.milestone,
      blockedBy: issue.blockedBy,
    },
    owner: issueOwner(issue, handoff),
    phase,
    pr:
      pr === null
        ? null
        : {
            number: pr.number,
            title: pr.title,
            url: pr.url,
            headSha: pr.headSha,
            baseRefName: pr.baseRefName,
            baseSha: pr.baseSha,
            mergeBaseSha: pr.mergeBaseSha,
            liveMainSha: snapshot.mainSha,
            relationToMain: pr.relationToMain,
            authorityPathsChangedOnMain: pr.authorityPathsChangedOnMain,
          },
    checks,
    reviews,
    handoff,
    authorityDrift: drift,
    blockers,
    action,
    sourceRefs,
  };
}

function placeholderIssue(pr: RawPullRequest, observedAt: string): RawIssue {
  const number = pr.issueNumbers[0] ?? pr.number;
  return {
    number,
    title: pr.issueNumbers.length === 0 ? `${pr.title} (Issue association unknown)` : `Issue #${number}`,
    url: pr.issueNumbers.length === 0 ? pr.url : `${pr.url.split("/pull/")[0] ?? pr.url}/issues/${number}`,
    body: "",
    updatedAt: observedAt,
    milestone: null,
    blockedBy: null,
    comments: [],
  };
}

export function normalizeRepositorySnapshot(snapshot: RawRepositorySnapshot): RepositoryProjection {
  const issues = snapshot.issues ?? [];
  const pullRequests = snapshot.pullRequests ?? [];
  const issuesByNumber = new Map(issues.map((issue) => [issue.number, issue]));
  const ownedIssueNumbers = new Set<number>();
  const deliveries: DeliveryLane[] = [];

  for (const pr of pullRequests) {
    const number = pr.issueNumbers[0];
    const issue = number === undefined ? placeholderIssue(pr, snapshot.observedAt) : issuesByNumber.get(number) ?? placeholderIssue(pr, snapshot.observedAt);
    ownedIssueNumbers.add(issue.number);
    deliveries.push(projectLane(issue, pr, snapshot));
  }

  for (const issue of issues) {
    if (ownedIssueNumbers.has(issue.number)) continue;
    const handoff = projectHandoff(issue.comments, snapshot.observedAt, null, snapshot.mainSha);
    const hasCanonicalHandoff = canonicalComments(issue.comments).length > 0;
    const owner = issueOwner(issue, handoff).toLowerCase();
    if (!hasCanonicalHandoff && !owner.includes("agent:")) continue;
    if (issueReadiness(issue, handoff) === "unknown") continue;
    deliveries.push(projectLane(issue, null, snapshot));
  }

  deliveries.sort((left, right) => {
    const leftCurrent = left.issue.milestone === snapshot.productHorizon ? 0 : 1;
    const rightCurrent = right.issue.milestone === snapshot.productHorizon ? 0 : 1;
    if (leftCurrent !== rightCurrent) return leftCurrent - rightCurrent;
    if ((left.pr === null) !== (right.pr === null)) return left.pr === null ? 1 : -1;
    return right.issue.number - left.issue.number;
  });

  const repoRef = source("direct", "Live GitHub repository", snapshot.repoUrl, snapshot.observedAt, snapshot.mainSha);
  const horizonRef = source("direct", "Product Roadmap at observed main", snapshot.productHorizonUrl, snapshot.observedAt, snapshot.mainSha);
  const currentWorkRef = source("derived", "Current-horizon lane projection", snapshot.productHorizonUrl, snapshot.observedAt, snapshot.mainSha);
  const humanActions = deliveries.filter((lane) => lane.action.owner === "human");
  const observationIncomplete = snapshot.fetchHealth !== "healthy";
  const horizonKnown = snapshot.productHorizon !== null;

  return {
    repo: {
      name: snapshot.repoName,
      observedAt: snapshot.observedAt,
      fetchHealth: snapshot.fetchHealth,
      failures: snapshot.failures,
      mainSha: snapshot.mainSha,
      productHorizon: snapshot.productHorizon,
      sourceRefs: [repoRef, horizonRef],
    },
    deliveries,
    currentWork: {
      currentHorizon: horizonKnown ? deliveries.filter((lane) => lane.issue.milestone === snapshot.productHorizon).map((lane) => lane.id) : [],
      independent: horizonKnown ? deliveries.filter((lane) => lane.issue.milestone !== snapshot.productHorizon).map((lane) => lane.id) : [],
      unclassified: horizonKnown ? [] : deliveries.map((lane) => lane.id),
      horizonStatus: horizonKnown ? "current" : "unknown",
      dependencyHealth: snapshot.issues === null || !horizonKnown
        ? "unknown"
        : deliveries.some((lane) => lane.issue.blockedBy === null)
          ? "partial"
          : "healthy",
      sourceRefs: [currentWorkRef],
    },
    recentCompletions: (snapshot.recentCompletions ?? []).map((completion) => ({
      ...completion,
      sourceRefs: [source("historical", `Merged PR #${completion.number}`, completion.url, snapshot.observedAt)],
    })),
    attention: {
      humanActionRequired: observationIncomplete ? null : humanActions.length > 0,
      reasons: observationIncomplete
        ? snapshot.failures.length > 0
          ? snapshot.failures
          : ["One or more authoritative sources are unavailable."]
        : humanActions.map((lane) => `#${lane.issue.number}: ${lane.action.reason}`),
      sourceRefs: [source("derived", "Attention classification", snapshot.repoUrl, snapshot.observedAt, snapshot.mainSha)],
    },
  };
}
