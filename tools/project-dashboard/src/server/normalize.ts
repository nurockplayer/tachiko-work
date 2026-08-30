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
const explicitlySubstantiveFinding = /(?:\[|\b)(?:p[0-2]|sev(?:erity)?[ -]?[0-2]|blocking|security|correctness)(?:\]|\b)/i;
const explicitlyNonSubstantiveFinding = /^(?:[_*]+\s*)?(?:\[(?:p3|sev(?:erity)?[ -]?3)\]|(?:p3|sev(?:erity)?[ -]?3|nit(?:pick)?|trivial)\b)/i;
const explicitlyNonSubstantiveBadge = /^(?:<sub>\s*)+!\[(?:p3|sev(?:erity)?[ -]?3)\s+badge\]\([^)]*\)(?:<\/sub>\s*)+/i;
const authorityOnlyIssue = /^\s*\[(?:decision|research)\](?:\s|\[|$)/i;

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
  if (match?.[1] === undefined) return null;
  const value = stripMarkdown(match[1]);
  return value === "" ? null : value;
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

function isSubstantiveFinding(body: string): boolean {
  if (explicitlySubstantiveFinding.test(body)) return true;
  const normalized = stripMarkdown(body);
  if (explicitlyNonSubstantiveBadge.test(normalized)) return false;
  return !explicitlyNonSubstantiveFinding.test(normalized);
}

function claimedIssueNumber(body: string): number | null {
  const claim = labeledValue(body, "ISSUE");
  if (claim !== null) {
    const match = claim.match(/^(?:Issue\s*)?#?(\d+)(?:\s*\/\s*PR\s*#?\d+)?$/i);
    const number = Number.parseInt(match?.[1] ?? "", 10);
    return Number.isSafeInteger(number) ? number : null;
  }

  const candidates = new Set<number>();
  for (const match of body.matchAll(/\bIssue\s*#(\d+)\b/gi)) {
    const number = Number.parseInt(match[1] ?? "", 10);
    if (Number.isSafeInteger(number)) candidates.add(number);
  }
  for (const match of body.matchAll(/#(\d+)\s*\/\s*PR\s*#\d+/gi)) {
    const number = Number.parseInt(match[1] ?? "", 10);
    if (Number.isSafeInteger(number)) candidates.add(number);
  }
  return candidates.size === 1 ? [...candidates][0] ?? null : null;
}

function markdownSectionValue(body: string, headingPattern: RegExp): string | null {
  const lines = body.split(/\r?\n/);
  for (const [index, line] of lines.entries()) {
    const heading = line.match(/^#{1,6}\s+(.+?)\s*$/)?.[1];
    if (heading === undefined || !headingPattern.test(heading)) continue;

    const content: string[] = [];
    for (const following of lines.slice(index + 1)) {
      if (/^#{1,6}\s+/.test(following)) break;
      content.push(following);
    }
    const value = stripMarkdown(content.join("\n"));
    if (value !== "") return value;
  }
  return null;
}

function hasNonemptyMarkdownSection(body: string, headingPattern: RegExp): boolean {
  return markdownSectionValue(body, headingPattern) !== null;
}

function hasRequiredPrHandoffRecords(body: string): boolean {
  const hasIssue = claimedIssueNumber(body) !== null;
  const hasStatus = labeledValue(body, "STATUS") !== null || labeledValue(body, "STATE") !== null;
  const hasScope = labeledValue(body, "SCOPE BOUNDARY") !== null || hasNonemptyMarkdownSection(body, /\bscope\b/i);
  const hasValidation = labeledValue(body, "VALIDATION EVIDENCE") !== null || hasNonemptyMarkdownSection(body, /\b(?:validation|evidence)\b/i);
  const hasReviewState = labeledValue(body, "UNRESOLVED REVIEW STATE") !== null || hasNonemptyMarkdownSection(body, /\b(?:unresolved review|review state)\b/i);
  const hasNextAction = labeledValue(body, "NEXT ACTION") !== null || hasNonemptyMarkdownSection(body, /^next\b/i);
  const hasEscalation = ["HUMAN ACTION", "FOUNDER / STEWARD ACTION", "STEWARD ACTION", "ESCALATION"]
    .some((label) => labeledValue(body, label) !== null) || hasNonemptyMarkdownSection(body, /\bescalation\b/i);
  return hasIssue && hasStatus && hasScope && hasValidation && hasReviewState && hasNextAction && hasEscalation;
}

function projectHandoff(
  comments: RawComment[],
  commentsComplete: boolean,
  observedAt: string,
  currentHeadSha: string | null,
  liveMainSha: string | null,
): HandoffProjection {
  if (!commentsComplete) {
    return {
      condition: "unknown",
      claimedOwner: null,
      claimedState: null,
      claimedIssueNumber: null,
      claimedHeadSha: null,
      lastCheckedMainSha: null,
      updatedAt: null,
      sourceRefs: [],
    };
  }
  const canonical = canonicalComments(comments);
  if (canonical.length === 0) {
    return {
      condition: "missing",
      claimedOwner: null,
      claimedState: null,
      claimedIssueNumber: null,
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
  } else if (currentHeadSha !== null && !hasRequiredPrHandoffRecords(latest.body)) {
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
    claimedIssueNumber: canonical.length === 1 ? claimedIssueNumber(latest.body) : null,
    claimedHeadSha,
    lastCheckedMainSha,
    updatedAt: latest.updatedAt,
    sourceRefs: [source("direct", "Canonical handoff", latest.url, observedAt, latest.id)],
  };
}

function issueOwner(issue: RawIssue, handoff: HandoffProjection): string {
  if (handoff.condition === "current" && handoff.claimedOwner !== null) return handoff.claimedOwner;
  return labeledValue(issue.body, "Owner") ?? "unknown";
}

function hasUsableIssueClaim(
  handoff: HandoffProjection,
): handoff is HandoffProjection & { claimedIssueNumber: number } {
  return (handoff.condition === "current" || handoff.condition === "stale") && handoff.claimedIssueNumber !== null;
}

function issueStatusText(issue: RawIssue): string {
  return (issue.body.match(/## Status\s*([\s\S]*?)(?=\n## |$)/i)?.[1] ?? issue.body.slice(0, 600)).toLowerCase();
}

function statusClaimsBlocked(statusText: string): boolean {
  return /\bblock(?:ed)?\b/.test(statusText) && !statusText.includes("not blocked");
}

function issueReadiness(issue: RawIssue, handoff: HandoffProjection): DeliveryLane["issue"]["readiness"] {
  if (issue.blockedBy === null) return "unknown";
  if (issue.blockedBy.length > 0) return "blocked";
  if (authorityOnlyIssue.test(issue.title)) return "unknown";
  const issueStatus = issueStatusText(issue);
  if (/\bdecision[_ -]?ready\b/.test(issueStatus) || /\bnot[_ -]+ready\b/.test(issueStatus)) return "unknown";
  if (/\bpark(?:ed)?\b/.test(issueStatus)) return "parked";
  if (statusClaimsBlocked(issueStatus)) return "blocked";
  const handoffState = handoff.claimedState?.toLowerCase() ?? "";
  const staleHandoffClaimsBlocked = handoff.condition === "stale" &&
    statusClaimsBlocked(handoffState);
  const currentHandoffState = handoff.condition === "current" || staleHandoffClaimsBlocked
    ? handoff.claimedState
    : null;
  const statusText = (currentHandoffState ?? issueStatus).toLowerCase();
  if (/\bdecision[_ -]?ready\b/.test(statusText)) return "unknown";
  if (/\bnot[_ -]+ready\b/.test(statusText)) return "unknown";
  if (/\bpark(?:ed)?\b/.test(statusText)) return "parked";
  if (/\bblock(?:ed)?\b/.test(statusText) && !statusText.includes("not blocked")) return "blocked";
  if (/\bactive\b|\bimplementing\b|\bin progress\b|\bvalidating\b|\breview[_ -]?fix\b|\bhuman[_ -]?required\b/.test(statusText)) return "active";
  if (/\bready\b/.test(statusText) && !/not ready for (?:production )?implementation/.test(statusText)) return "ready";
  return "unknown";
}

function humanActionRequested(comments: RawComment[], handoff: HandoffProjection): boolean {
  if (handoff.condition !== "current" && handoff.condition !== "stale") return false;
  if (/human[_ -]?required/i.test(handoff.claimedState ?? "")) return true;
  const latest = canonicalComments(comments).toSorted((left, right) => right.updatedAt.localeCompare(left.updatedAt))[0];
  if (latest === undefined) return false;
  const claim =
    labeledValue(latest.body, "HUMAN ACTION") ??
    labeledValue(latest.body, "FOUNDER / STEWARD ACTION") ??
    labeledValue(latest.body, "STEWARD ACTION") ??
    labeledValue(latest.body, "ESCALATION") ??
    markdownSectionValue(latest.body, /\bescalation\b/i);
  if (
    claim === null ||
    /^(?:none|no|not (?:required|needed)|n\/a)\b/i.test(claim) ||
    /\b(?:(?:human|steward)\s+)?(?:action|decision|escalation)\s+(?:is\s+)?not\s+(?:required|needed)\b/i.test(claim)
  ) return false;
  return /\b(?:required|needed|decision|action)\b/i.test(claim);
}

function deliveryActionOwner(owner: string): DeliveryLane["action"]["owner"] {
  const normalized = owner.trim().toLowerCase();
  if (normalized === "agent:codex" || normalized === "codex") return "codex";
  if (normalized === "agent:chatgpt" || normalized === "chatgpt") return "chatgpt";
  return "unknown";
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
  const reviewedHeads = new Set(relevantReviews.map((review) => review.headSha));
  const reviewedHeadSha = reviewedHeads.size === 1 ? relevantReviews[0]?.headSha ?? null : null;
  const unresolved = pr.reviewThreads.filter((thread) => !thread.resolved);
  const allReviewsCoverHead = relevantReviews.length > 0 && relevantReviews.every(
    (review) => review.headSha !== null && shaMatches(review.headSha, pr.headSha),
  );
  const status = relevantReviews.length === 0
    ? (pr.reviewDecision === "review_required" ? "current" : "unknown")
    : allReviewsCoverHead
      ? "current"
      : "stale";

  return {
    decision: pr.reviewDecision,
    status,
    reviewedHeadSha,
    unresolvedThreadCount: unresolved.length,
    substantiveUnresolvedCount: unresolved.filter(
      (thread) => thread.comments.some((comment) => isSubstantiveFinding(comment)),
    ).length,
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

function horizonClass(
  milestone: string | null,
  productHorizon: string | null,
): "current" | "independent" | "other" | "unknown" {
  if (productHorizon === null) return "unknown";
  if (milestone === null) return "independent";
  return milestone === productHorizon ? "current" : "other";
}

function derivePhase(
  readiness: DeliveryLane["issue"]["readiness"],
  pr: RawPullRequest | null,
  checks: CheckProjection,
  reviews: ReviewProjection,
  handoff: HandoffProjection,
  drift: DeliveryLane["authorityDrift"],
  ownershipConflict: boolean,
  ownershipObservationComplete: boolean,
  githubMergeReady: boolean,
  issueScopeReconciled: boolean,
  horizonObserved: boolean,
  humanActionRequested: boolean,
): DeliveryPhase {
  const claimedState = handoff.condition === "current" ? handoff.claimedState?.toLowerCase() ?? "" : "";
  if (readiness === "parked" || claimedState.includes("parked")) return "parked";
  if (humanActionRequested || claimedState.includes("human_required") || claimedState.includes("human required")) return "human_required";
  if (ownershipConflict) return "blocked";
  if (readiness === "blocked") return "blocked";
  if (!ownershipObservationComplete) return "validating";
  if (!issueScopeReconciled || !horizonObserved) return "validating";
  if (pr === null) return readiness === "ready" ? "ready" : readiness === "active" ? "implementing" : "unknown";
  if (readiness === "unknown" || pr.isDraft) return "validating";
  if ((reviews.substantiveUnresolvedCount ?? 0) > 0) return "review_fix";
  if (reviews.status === "stale") return "rereview";
  if (reviews.decision === "changes_requested") return "review_fix";
  if (checks.status !== "success" || checks.requiredStatus !== "satisfied") return "validating";
  if (reviews.status !== "current") return "rereview";
  if (reviews.decision !== "approved") return "rereview";
  if (handoff.condition !== "current") return "validating";
  if (drift !== "none") return "validating";
  if (!githubMergeReady) return "validating";
  return "merge_gate";
}

function projectLane(
  issue: RawIssue,
  pr: RawPullRequest | null,
  snapshot: RawRepositorySnapshot,
  ownershipConflicts: Array<{ issueNumber: number; prNumbers: number[] }> = [],
  ownershipObservationComplete = true,
): DeliveryLane {
  const comments = pr === null ? issue.comments : pr.comments;
  const commentsComplete = pr === null ? issue.commentsComplete : pr.commentsComplete;
  const handoff = projectHandoff(comments, commentsComplete, snapshot.observedAt, pr?.headSha ?? null, snapshot.mainSha);
  const observedReadiness = issueReadiness(issue, handoff);
  const readinessBeforeHandoff = pr === null || !["ready", "active"].includes(observedReadiness) ? observedReadiness : "active";
  const readiness = !commentsComplete && !["blocked", "parked"].includes(readinessBeforeHandoff)
    ? "unknown"
    : readinessBeforeHandoff;
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
  const issueHorizonClass = horizonClass(issue.milestone, snapshot.productHorizon);
  const outsideCurrentHorizon = issueHorizonClass === "other";
  const horizonObserved = issueHorizonClass !== "unknown";
  const targetsDefaultBranch = pr === null || (
    snapshot.defaultBranchName !== null && pr.baseRefName === snapshot.defaultBranchName
  );
  const githubMergeReady = pr === null || (
    targetsDefaultBranch && pr.mergeable === "mergeable" && pr.mergeStateStatus === "clean"
  );
  const issueScopeReconciled = issue.lastEditedAt === null || handoff.condition === "missing" || (
    handoff.updatedAt !== null && handoff.updatedAt.localeCompare(issue.lastEditedAt) >= 0
  );
  const multipleIssueClaim = (pr?.issueNumbers.length ?? 0) > 1;
  const handoffIssueMismatch = pr !== null && pr.issueNumbers.length === 1 && hasUsableIssueClaim(handoff) &&
    handoff.claimedIssueNumber !== pr.issueNumbers[0];
  const ownershipConflict = ownershipConflicts.length > 0 || multipleIssueClaim || handoffIssueMismatch;
  const blockers: string[] = [];
  const issueStatusBlocked = statusClaimsBlocked(issueStatusText(issue));

  if ((issue.blockedBy?.length ?? 0) > 0) {
    blockers.push(`Live Issue dependencies block this lane: ${issue.blockedBy?.map((dependency) => `#${dependency.number}`).join(", ") ?? "Unknown"}.`);
  }
  if (issue.blockedBy === null) blockers.push("Issue dependency state could not be fully observed.");
  if (issueStatusBlocked && (issue.blockedBy?.length ?? 0) === 0) {
    blockers.push("The authoritative Issue status reports this lane blocked.");
  } else if (handoff.condition === "current" && observedReadiness === "blocked" && (issue.blockedBy?.length ?? 0) === 0) {
    blockers.push("The current canonical handoff reports this lane blocked.");
  } else if (handoff.condition === "stale" && observedReadiness === "blocked" && (issue.blockedBy?.length ?? 0) === 0) {
    blockers.push("The stale canonical handoff reports this lane blocked pending reconciliation.");
  }
  if (pr !== null && checks.status === "unknown") blockers.push("Checks were not observed for the current PR head.");
  if (pr !== null && checks.requiredStatus !== "satisfied") blockers.push(checks.requiredSummary);
  if (checks.status === "failure") blockers.push(checks.summary);
  if ((reviews.substantiveUnresolvedCount ?? 0) > 0) {
    blockers.push(`${reviews.substantiveUnresolvedCount ?? 0} substantive review finding(s) remain unresolved.`);
  }
  if (reviews.status === "stale") blockers.push("The latest substantive review does not describe the current PR head.");
  if (pr !== null && handoff.condition === "missing") {
    blockers.push(`Canonical handoff is missing for pull request #${pr.number}.`);
  }
  if (handoff.condition === "inconsistent") blockers.push("Canonical handoff conflicts with live PR identity or is duplicated.");
  if (handoff.condition === "stale") blockers.push("Canonical handoff has not reconciled the observed live main.");
  if (pr !== null && snapshot.defaultBranchName === null) {
    blockers.push("Default branch identity could not be observed.");
  } else if (pr !== null && !targetsDefaultBranch) {
    blockers.push(`Pull request #${pr.number} targets ${pr.baseRefName} instead of the live default branch ${snapshot.defaultBranchName ?? "Unknown"}.`);
  }
  if (!ownershipObservationComplete) blockers.push("Pull-request Issue ownership could not be fully observed.");
  const changedAuthorityPaths = pr?.authorityPathsChangedOnMain ?? [];
  if (drift === "suspected" && changedAuthorityPaths.length > 0) {
    blockers.push(`Accepted-authority candidates changed on main: ${changedAuthorityPaths.join(", ")}.`);
  }
  if (drift === "suspected") blockers.push("Authority or live-main drift requires explicit reconciliation.");
  if (drift === "unknown") blockers.push("Live-main and authority-drift reconciliation could not be observed.");
  for (const conflict of ownershipConflicts) {
    blockers.push(`Multiple open pull requests claim Issue #${conflict.issueNumber}: ${conflict.prNumbers.map((number) => `#${number}`).join(", ")}.`);
  }
  if (multipleIssueClaim && pr !== null) {
    blockers.push(`Pull request #${pr.number} claims multiple Issues (${pr.issueNumbers.map((number) => `#${number}`).join(", ")}), violating the one-Issue delivery boundary.`);
  }
  if (handoffIssueMismatch) {
    blockers.push(`Canonical handoff claims Issue #${handoff.claimedIssueNumber}, but pull request #${pr.number} closes Issue #${pr.issueNumbers[0] ?? "Unknown"}.`);
  }
  if (pr?.isDraft === true) blockers.push(`Pull request #${pr.number} is still a draft.`);
  if (pr !== null && (pr.mergeable === "unknown" || pr.mergeStateStatus === "unknown")) {
    blockers.push(`GitHub mergeability could not be fully observed for pull request #${pr.number}.`);
  } else if (pr !== null && (pr.mergeable === "conflicting" || pr.mergeStateStatus === "dirty")) {
    blockers.push(`GitHub reports that pull request #${pr.number} has merge conflicts.`);
  } else if (pr !== null && pr.mergeStateStatus !== "clean") {
    blockers.push(`GitHub reports that pull request #${pr.number} is blocked from merging.`);
  }
  if (!issueScopeReconciled) {
    blockers.push(`Issue #${issue.number} scope was edited after the canonical handoff; explicit reconciliation is required.`);
  }
  if (!horizonObserved) blockers.push("Product Roadmap horizon could not be observed.");
  if (outsideCurrentHorizon) {
    blockers.push(`Issue #${issue.number} belongs to non-current product milestone ${issue.milestone ?? "Unknown"}; the live current horizon is ${snapshot.productHorizon ?? "Unknown"}.`);
  }

  const requiresHuman = humanActionRequested(comments, handoff);
  const owner = issueOwner(issue, handoff);
  const phase = outsideCurrentHorizon
    ? "parked"
    : derivePhase(
        readiness,
        pr,
        checks,
        reviews,
        handoff,
        drift,
        ownershipConflict,
        ownershipObservationComplete,
        githubMergeReady,
        issueScopeReconciled,
        horizonObserved,
        requiresHuman,
      );
  const action: DeliveryLane["action"] = requiresHuman || phase === "human_required"
    ? { owner: "human", reason: "The canonical coordination state requests human or Steward action." }
    : phase === "review_fix" || phase === "rereview" || checks.status === "failure" ||
        (pr !== null && checks.requiredStatus !== "satisfied") || ownershipConflict ||
        pr?.mergeable === "conflicting" || pr?.mergeStateStatus === "dirty" ||
        drift === "suspected" || handoff.condition === "inconsistent" || handoff.condition === "stale" ||
        (pr !== null && handoff.condition === "missing") || !issueScopeReconciled ||
        !targetsDefaultBranch || !ownershipObservationComplete
      ? { owner: deliveryActionOwner(owner), reason: blockers[0] ?? "Delivery-agent action is required." }
      : { owner: "none", reason: "No human action is currently evidenced." };
  const issueRef = source(
    "direct",
    `Issue #${issue.number}`,
    issue.url,
    snapshot.observedAt,
    issue.lastEditedAt ?? issue.updatedAt,
  );
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
    id: pr === null ? `issue-${issue.number}` : `issue-${issue.number}-pr-${pr.number}`,
    issue: {
      number: issue.number,
      title: issue.title,
      url: issue.url,
      readiness,
      milestone: issue.milestone,
      lastEditedAt: issue.lastEditedAt,
      blockedBy: issue.blockedBy,
    },
    owner,
    phase,
    pr:
      pr === null
        ? null
        : {
            number: pr.number,
            title: pr.title,
            url: pr.url,
            isDraft: pr.isDraft,
            headSha: pr.headSha,
            baseRefName: pr.baseRefName,
            baseSha: pr.baseSha,
            mergeBaseSha: pr.mergeBaseSha,
            liveMainSha: snapshot.mainSha,
            relationToMain: pr.relationToMain,
            authorityPathsChangedOnMain: pr.authorityPathsChangedOnMain,
            mergeable: pr.mergeable,
            mergeStateStatus: pr.mergeStateStatus,
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
    lastEditedAt: null,
    milestone: null,
    blockedBy: null,
    commentsComplete: true,
    comments: [],
  };
}

export function normalizeRepositorySnapshot(snapshot: RawRepositorySnapshot): RepositoryProjection {
  const issues = snapshot.issues ?? [];
  const pullRequests = (snapshot.pullRequests ?? []).map((pr) => {
    if (pr.issueNumbers.length > 0) return pr;
    const handoff = projectHandoff(
      pr.comments,
      pr.commentsComplete,
      snapshot.observedAt,
      pr.headSha,
      snapshot.mainSha,
    );
    return hasUsableIssueClaim(handoff)
      ? { ...pr, issueNumbers: [handoff.claimedIssueNumber] }
      : pr;
  });
  const issuesByNumber = new Map(issues.map((issue) => [issue.number, issue]));
  const ownershipObservationComplete = snapshot.pullRequests !== null &&
    pullRequests.every((pr) =>
      pr.issueNumbersComplete && (pr.issueNumbers.length > 0 || pr.commentsComplete)
    );
  const pullRequestsByIssue = new Map<number, number[]>();
  for (const pr of pullRequests) {
    for (const issueNumber of pr.issueNumbers.length === 0 ? [pr.number] : pr.issueNumbers) {
      const numbers = pullRequestsByIssue.get(issueNumber) ?? [];
      numbers.push(pr.number);
      pullRequestsByIssue.set(issueNumber, numbers);
    }
  }
  for (const numbers of pullRequestsByIssue.values()) numbers.sort((left, right) => left - right);
  const ownedIssueNumbers = new Set<number>();
  const deliveries: DeliveryLane[] = [];

  for (const pr of pullRequests) {
    const number = pr.issueNumbers[0];
    const issue = number === undefined ? placeholderIssue(pr, snapshot.observedAt) : issuesByNumber.get(number) ?? placeholderIssue(pr, snapshot.observedAt);
    const claimedIssueNumbers = pr.issueNumbers.length === 0 ? [issue.number] : pr.issueNumbers;
    for (const issueNumber of claimedIssueNumbers) ownedIssueNumbers.add(issueNumber);
    const ownershipConflicts = claimedIssueNumbers.flatMap((issueNumber) => {
      const prNumbers = pullRequestsByIssue.get(issueNumber) ?? [];
      return prNumbers.length > 1 ? [{ issueNumber, prNumbers }] : [];
    });
    deliveries.push(projectLane(issue, pr, snapshot, ownershipConflicts, ownershipObservationComplete));
  }

  for (const issue of issues) {
    if (ownedIssueNumbers.has(issue.number)) continue;
    const handoff = projectHandoff(
      issue.comments,
      issue.commentsComplete,
      snapshot.observedAt,
      null,
      snapshot.mainSha,
    );
    const hasCanonicalHandoff = canonicalComments(issue.comments).length > 0;
    const owner = issueOwner(issue, handoff).toLowerCase();
    if (!hasCanonicalHandoff && !owner.includes("agent:")) continue;
    if (issueReadiness(issue, handoff) === "unknown" && !humanActionRequested(issue.comments, handoff)) continue;
    deliveries.push(projectLane(issue, null, snapshot, [], ownershipObservationComplete));
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
      defaultBranchName: snapshot.defaultBranchName,
      productHorizon: snapshot.productHorizon,
      sourceRefs: [repoRef, horizonRef],
    },
    deliveries,
    currentWork: {
      currentHorizon: deliveries.filter((lane) => horizonClass(lane.issue.milestone, snapshot.productHorizon) === "current").map((lane) => lane.id),
      independent: deliveries.filter((lane) => horizonClass(lane.issue.milestone, snapshot.productHorizon) === "independent").map((lane) => lane.id),
      otherHorizon: deliveries.filter((lane) => horizonClass(lane.issue.milestone, snapshot.productHorizon) === "other").map((lane) => lane.id),
      unclassified: deliveries.filter((lane) => horizonClass(lane.issue.milestone, snapshot.productHorizon) === "unknown").map((lane) => lane.id),
      horizonStatus: horizonKnown ? "current" : "unknown",
      dependencyHealth: snapshot.issues === null || !horizonKnown
        ? "unknown"
        : issues.some((issue) => issue.blockedBy === null)
          ? "partial"
          : "healthy",
      sourceRefs: [currentWorkRef],
    },
    recentCompletions: (snapshot.recentCompletions ?? []).map((completion) => ({
      ...completion,
      sourceRefs: [source("historical", `Merged PR #${completion.number}`, completion.url, snapshot.observedAt)],
    })),
    attention: {
      humanActionRequired: humanActions.length > 0 ? true : observationIncomplete ? null : false,
      reasons: humanActions.length > 0
        ? humanActions.map((lane) => `#${lane.issue.number}: ${lane.action.reason}`)
        : observationIncomplete
        ? snapshot.failures.length > 0
          ? snapshot.failures
          : ["One or more authoritative sources are unavailable."]
        : [],
      sourceRefs: [source("derived", "Attention classification", snapshot.repoUrl, snapshot.observedAt, snapshot.mainSha)],
    },
  };
}
