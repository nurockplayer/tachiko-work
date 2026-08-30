import type {
  CheckProjection,
  DeliveryLane,
  DeliveryPhase,
  HandoffProjection,
  RawCheck,
  RawComment,
  RawIssue,
  RawPullRequest,
  RawRepositorySnapshot,
  RawReview,
  RepositoryProjection,
  ReviewProjection,
  SourceClass,
  SourceRef,
} from "../shared/types.ts";
import { isDecisionAuthorityPath } from "../shared/authority.ts";

const handoffMarker = "<!-- agent-handoff:v1 -->";
const explicitlyPrioritizedFinding = /(?:\[|\b)(?:p[0-2]|sev(?:erity)?[ -]?[0-2])(?:\]|\b)/i;
const negatedReviewFinding = /(?:\b(?:no|none|without|zero|0)\b[^.!?;\n]{0,80}\b(?:p[0-2]|sev(?:erity)?[ -]?[0-2]|blocking|security|correctness|findings?|issues?|concerns?|problems?)\b|\bnot\s+(?:an?\s+)?\[?(?:p[0-2]|sev(?:erity)?[ -]?[0-2]|blocking|security|correctness)(?:\]|\b))/i;
const clearedReviewFinding = /\b(?:p[0-2]|sev(?:erity)?[ -]?[0-2]|security|correctness|blocking|data[- ]integrity)\b[^.!?;\n]{0,80}\b(?:checks?|review|findings?|issues?)?\s*(?:passed|complete(?:d)?|clean|resolved)\b(?:\s*\([^)]*\))?\s*$/i;
const negatedReviewResolution = /\b(?:p[0-2]|sev(?:erity)?[ -]?[0-2]|security|correctness|blocking|data[- ]integrity)\b[^.!?;\n]{0,80}(?:\b(?:not|never|cannot)\b|\b(?:is|are|was|were|has|have|had|do|does|did|can|could|would|should|will|wo)n['’]?t\b)(?:\s+\w+){0,3}\s+(?:passed|complete(?:d)?|clean|resolved)\b/i;
const postposedClearedReviewFinding = /\b(?:p[0-2]|sev(?:erity)?[ -]?[0-2]|blocking|security|correctness|data[- ]integrity)(?:\s+(?:findings?|issues?|concerns?|problems?)(?:\s+found)?)?\s*(?::|=|\bare\b)\s*(?:none|zero|0|absent|not\s+(?:found|present|observed|identified|detected))\b(?:\s*\([^)]*\))?\s*$/i;
const equivalentReviewFindingLabel = /(?:^|\s)(?:blocking|security|correctness|data[- ]integrity)\s*[:,]/i;
const equivalentReviewFindingContext = /\b(?:blocking|security|correctness|data[- ]integrity)\b[^.!?;\n]{0,80}\b(?:finding|issue|bug|risk|failure|regression|vulnerab\w*|flaw|problem|concern|break\w*|corrupt\w*|overwrit\w*|data[- ]loss)\b|\b(?:finding|issue|bug|risk|failure|regression|vulnerab\w*|flaw|problem|concern|break\w*|corrupt\w*|overwrit\w*|data[- ]loss)\b[^.!?;\n]{0,80}\b(?:blocking|security|correctness|data[- ]integrity)\b/i;
const explicitReviewClearingSignal = /(?:\[|\b)(?:p[0-2]|sev(?:erity)?[ -]?[0-2]|blocking|security|correctness|data[- ]integrity)(?:\]|\b)/i;
const reviewClauseBoundary = /[.!?;\n]+|\b(?:but|except|however|although|yet)\b|,\s*(?=(?:p[0-2]|sev(?:erity)?[ -]?[0-2]|blocking|security|correctness|data[- ]integrity)\b)/i;
const explicitlyNonSubstantiveFinding = /^(?:[_*]+\s*)?(?:\[(?:p3|sev(?:erity)?[ -]?3)\]|(?:p3|sev(?:erity)?[ -]?3|nit(?:pick)?|trivial)\b)/i;
const explicitlyNonSubstantiveBadge = /^(?:<sub>\s*)+!\[(?:p3|sev(?:erity)?[ -]?3)\s+badge\]\([^)]*\)(?:<\/sub>\s*)+/i;
const explicitlyNonSubstantiveAcknowledgment = /^(?:done|fixed(?:\s+in\s+(?:commit\s+)?[0-9a-f]{7,40})?|thanks,\s+applied this suggestion)[.!]?$/i;
const negatedUnlabeledSubstantiveImpact = /(?:\b(?:no|none|without|zero|0)\b[^.!?;\n]{0,80}\b(?:wrong|incorrect|stale|invalid|unsafe|unauthori[sz]ed|data[- ]loss|regression|race\s+condition|deadlock|vulnerab\w*|security\s+flaw|crash(?:es|ed|ing)?|panic(?:s|ked|king)?|corrupt\w*|overwrit\w*|bypass\w*|leak\w*|los(?:e|es|ing|t)\s+(?:user\s+)?data)\b|\b(?:does|do|did|can|could|would|should|will|is|are|was|were|has|have|had)\s+not\s+(?:\w+\s+){0,3}(?:return\s+(?:a\s+)?wrong|produc\w*\s+(?:an?\s+)?incorrect|los(?:e|es|ing|t)\s+(?:user\s+)?data|corrupt\w*|overwrit\w*|bypass\w*|leak\w*|crash\w*|panic\w*|break\w*|fail\w*)\b|\b(?:data[- ]loss|regression|race\s+condition|deadlock|vulnerab\w*|security\s+flaw|crash(?:es|ed|ing)?|panic(?:s|ked|king)?|corrupt\w*|overwrit\w*|bypass\w*|leak\w*)\b[^.!?;\n]{0,80}\b(?:not\s+(?:found|present|observed|identified|detected)|absent)\b)/i;
const unlabeledPureMaintainabilitySuggestion = /^(?:could|would|can|please|consider|maybe|perhaps)\b[^.!?;\n]{0,120}\b(?:rename|naming|clarity|readability|style|format(?:ting)?|wording|comments?|documentation|docs|simplif\w*|clean\s*up|refactor\w*)\b/i;
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

function hasTrustedAuthorAssociation(authorAssociation: string | null): boolean {
  return /^(?:owner|member|collaborator)$/i.test(authorAssociation ?? "");
}

function canonicalComments(comments: RawComment[]): RawComment[] {
  return comments.filter(
    (comment) => hasTrustedAuthorAssociation(comment.authorAssociation) && comment.body.includes(handoffMarker),
  );
}

function isSubstantiveFinding(body: string): boolean {
  const normalized = stripMarkdown(body).replace(explicitlyNonSubstantiveBadge, "[P3] ");
  return reviewBodyClauses(normalized).some((clause) => {
    if (explicitlyNonSubstantiveBadge.test(clause)) return false;
    if (explicitlyNonSubstantiveAcknowledgment.test(clause)) return false;
    if (explicitlyNonSubstantiveFinding.test(clause)) return false;
    if (isSubstantiveReviewClause(clause)) return true;
    if (isClearedReviewClause(clause) || negatedUnlabeledSubstantiveImpact.test(clause)) return false;
    return !unlabeledPureMaintainabilitySuggestion.test(clause);
  });
}

function reviewBodyClauses(body: string): string[] {
  return stripMarkdown(body).split(reviewClauseBoundary).map((segment) => segment.trim()).filter(Boolean);
}

function isClearedReviewClause(clause: string): boolean {
  return negatedReviewFinding.test(clause) ||
    (clearedReviewFinding.test(clause) && !negatedReviewResolution.test(clause)) ||
    postposedClearedReviewFinding.test(clause);
}

function isSubstantiveReviewClause(clause: string): boolean {
  if (isClearedReviewClause(clause)) return false;
  return explicitlyPrioritizedFinding.test(clause) || equivalentReviewFindingLabel.test(clause) ||
    equivalentReviewFindingContext.test(clause);
}

function isSubstantiveReviewBody(body: string): boolean {
  return reviewBodyClauses(body).some(isSubstantiveReviewClause);
}

function clearsSubstantiveReviewBody(body: string): boolean {
  const clauses = reviewBodyClauses(body);
  return clauses.length > 0 && !clauses.some(isSubstantiveReviewClause) &&
    clauses.some((clause) => explicitReviewClearingSignal.test(clause) && isClearedReviewClause(clause));
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
      observedIssueNumbers: [],
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
      observedIssueNumbers: [],
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
  const observedIssueNumbers = [...new Set(canonical.flatMap((comment) => {
    const number = claimedIssueNumber(comment.body);
    return number === null ? [] : [number];
  }))].toSorted((left, right) => left - right);
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
    observedIssueNumbers,
    claimedHeadSha,
    lastCheckedMainSha,
    updatedAt: latest.updatedAt,
    sourceRefs: [source("direct", "Canonical handoff", latest.url, observedAt, latest.id)],
  };
}

function issueOwner(issue: RawIssue, handoff: HandoffProjection): string {
  if ((handoff.condition === "current" || handoff.condition === "stale") && handoff.claimedOwner !== null) {
    return handoff.claimedOwner;
  }
  if (!hasTrustedAuthorAssociation(issue.authorAssociation)) return "unknown";
  return labeledValue(issue.body, "Owner") ?? "unknown";
}

function hasUsableIssueClaim(
  handoff: HandoffProjection,
): handoff is HandoffProjection & { claimedIssueNumber: number } {
  return (handoff.condition === "current" || handoff.condition === "stale") && handoff.claimedIssueNumber !== null;
}

function issueStatusText(issue: RawIssue): string {
  if (!hasTrustedAuthorAssociation(issue.authorAssociation)) return "";
  return (
    issue.body.match(/^##[ \t]+Status[ \t]*(?:\r?\n|$)([\s\S]*?)(?=^#{1,6}[ \t]+|$(?![\s\S]))/im)?.[1] ?? ""
  ).toLowerCase();
}

function statusClaimIsNegated(statusText: string, match: RegExpExecArray): boolean {
  const before = statusText.slice(0, match.index);
  const after = statusText.slice(match.index + match[0].length);
  return /(?:^|[^a-z0-9_])(?:not(?:[_ -]+(?:yet|currently|now|presently|quite))?|never|no[_ -]+longer|non)[_ -]*$/i.test(before) ||
    /^\s*(?:(?:is\s+)?not\b|[:=-]\s*(?:false|no)\b)/i.test(after);
}

function statusClaimIsConditional(
  statusText: string,
  match: RegExpExecArray,
  pendingIsConditional: boolean,
): boolean {
  const before = statusText.slice(0, match.index);
  const after = statusText.slice(match.index + match[0].length);
  const prefixClause = before
    .slice(Math.max(before.lastIndexOf("\n"), before.lastIndexOf("."), before.lastIndexOf(";")) + 1)
    .replace(/[-—–,:=]\s*$/, "")
    .trim();
  const prefixSegments = prefixClause.split(/\s*(?:,|\bbut\b)\s*/i).filter(Boolean);
  const unresolvedPrefix = prefixSegments.some((segment) => {
    if (/^(?:subject\s+to\b|(?:only\s+)?(?:once|when|if|after)\b)/i.test(segment)) return true;
    if (/^pending\b/i.test(segment)) {
      return pendingIsConditional &&
        !/\b(?:is|are|was|were|has|have)\s+(?:now\s+)?(?:been\s+)?(?:complete|completed|resolved|satisfied|approved|cleared|closed)\b/i.test(segment);
    }
    return pendingIsConditional && !/^(?:no|none|nothing)\b/i.test(segment) &&
      /\b(?:is|are|remain|remains)\s+pending\b/i.test(segment);
  });
  return /\b(?:(?:future|become|mark|set|move|declare|consider)(?:\s+(?:as|to))?|(?:will|would|should|can|could|may|might)(?:\s+be|\s+become)?)\s*$/i.test(before) ||
    unresolvedPrefix ||
    /^\s*(?:(?:[:=-])\s*)?(?:only\s+)?(?:(?:once|when|if|after)\b|subject\s+to\b)/i.test(after) ||
    (pendingIsConditional && /^\s*(?:(?:[:=-])\s*)?(?:only\s+)?pending\b/i.test(after));
}

function statusHasAffirmativeClaim(
  statusText: string,
  claim: RegExp,
  pendingIsConditional = false,
): boolean {
  const matcher = new RegExp(claim.source, `${claim.flags.replaceAll("g", "")}g`);
  return [...statusText.matchAll(matcher)].some(
    (match) => !statusClaimIsNegated(statusText, match) &&
      !statusClaimIsConditional(statusText, match, pendingIsConditional),
  );
}

function statusHasNegatedClaim(statusText: string, claim: RegExp): boolean {
  const matcher = new RegExp(claim.source, `${claim.flags.replaceAll("g", "")}g`);
  return [...statusText.matchAll(matcher)].some((match) => statusClaimIsNegated(statusText, match));
}

function statusClaimsDecisionReady(statusText: string): boolean {
  return statusHasAffirmativeClaim(statusText, /\bdecision[_ -]?ready\b/i, true);
}

function statusClaimsBlocked(statusText: string): boolean {
  return statusHasAffirmativeClaim(statusText, /\bblock(?:ed)?\b/i);
}

function statusClaimsParked(statusText: string): boolean {
  return statusHasAffirmativeClaim(statusText, /\bpark(?:ed)?\b/i);
}

function statusClaimsActive(statusText: string): boolean {
  return statusHasAffirmativeClaim(
    statusText,
    /\b(?:active|implementing|in progress|validating|review[_ -]?fix)\b/i,
    true,
  );
}

function statusClaimsHumanRequired(statusText: string): boolean {
  return statusHasAffirmativeClaim(statusText, /\bhuman[_ -]?required\b/i);
}

function statusClaimsReady(statusText: string): boolean {
  return !/\bdecision[_ -]?ready\b/i.test(statusText) && !statusClaimsNotReady(statusText) &&
    statusHasAffirmativeClaim(statusText, /\bready\b/i, true) &&
    !/not ready for (?:production )?implementation/i.test(statusText);
}

function statusClaimsNotReady(statusText: string): boolean {
  return statusHasNegatedClaim(statusText, /\b(?:decision[_ -]?)?ready\b/i);
}

function handoffClaimsMergeReady(handoff: HandoffProjection): boolean {
  return handoff.condition === "current" && /^merge[_ -]ready$/i.test(handoff.claimedState?.trim() ?? "");
}

function issueReadiness(issue: RawIssue, handoff: HandoffProjection): DeliveryLane["issue"]["readiness"] {
  if (issue.blockedBy === null) return "unknown";
  if (issue.blockedBy.length > 0) return "blocked";
  const issueStatus = issueStatusText(issue);
  if (authorityOnlyIssue.test(issue.title) && statusClaimsDecisionReady(issueStatus)) return "ready";
  if (authorityOnlyIssue.test(issue.title)) return "unknown";
  if (/\bdecision[_ -]?ready\b/i.test(issueStatus) || statusClaimsNotReady(issueStatus)) return "unknown";
  if (statusClaimsParked(issueStatus)) return "parked";
  if (statusClaimsBlocked(issueStatus)) return "blocked";
  const handoffState = handoff.claimedState?.toLowerCase() ?? "";
  const trustedHandoffAffirmsDelivery = !hasTrustedAuthorAssociation(issue.authorAssociation) &&
    handoff.condition === "current" &&
    (statusClaimsActive(handoffState) || statusClaimsReady(handoffState));
  if (!statusClaimsActive(issueStatus) && !statusClaimsReady(issueStatus) && !trustedHandoffAffirmsDelivery) {
    return "unknown";
  }
  const staleHandoffClaimsBlocked = handoff.condition === "stale" &&
    statusClaimsBlocked(handoffState);
  const currentHandoffState = handoff.condition === "current" || staleHandoffClaimsBlocked
    ? handoff.claimedState
    : null;
  const statusText = (currentHandoffState ?? issueStatus).toLowerCase().replaceAll("_", " ");
  if (/\bdecision[_ -]?ready\b/i.test(statusText)) return "unknown";
  if (statusClaimsNotReady(statusText)) return "unknown";
  if (statusClaimsParked(statusText)) return "parked";
  if (statusClaimsBlocked(statusText)) return "blocked";
  if (statusClaimsActive(statusText)) return "active";
  if (statusClaimsReady(statusText)) return "ready";
  return "unknown";
}

function isDecisionReadyAuthorityIssue(issue: RawIssue): boolean {
  return authorityOnlyIssue.test(issue.title) && statusClaimsDecisionReady(issueStatusText(issue));
}

function isFocusedAuthorityPullRequest(pr: RawPullRequest): boolean {
  return pr.changedPaths !== null && pr.changedPaths.length > 0 && pr.changedPaths.every(isDecisionAuthorityPath);
}

function humanActionRequested(comments: RawComment[], handoff: HandoffProjection): boolean {
  if (handoff.condition === "missing") return false;
  const isNegative = (claim: string) =>
    /^(?:none|no|false|unnecessary|not(?:\s+currently)?\s+(?:required|needed|necessary|applicable)|n\/a)\b/i.test(claim) ||
    /\b(?:(?:human|steward)\s+)?(?:action|decision|escalation|approval|review)\s+(?:(?:is|are)\s+not|isn['’]?t|aren['’]?t)(?:\s+currently)?\s+(?:required|needed|necessary|applicable)\b/i.test(claim) ||
    /\bdo not escalate\b/i.test(claim);
  return canonicalComments(comments).some((comment) => {
    const claimedState = labeledValue(comment.body, "STATE") ?? labeledValue(comment.body, "STATUS");
    if (/human[_ -]?required/i.test(claimedState ?? "")) return true;
    const labeledClaims = [
      labeledValue(comment.body, "HUMAN ACTION"),
      labeledValue(comment.body, "FOUNDER / STEWARD ACTION"),
      labeledValue(comment.body, "STEWARD ACTION"),
      labeledValue(comment.body, "ESCALATION"),
    ].filter((claim): claim is string => claim !== null);
    if (labeledClaims.some((claim) => !isNegative(claim))) return true;

    const escalationSection = markdownSectionValue(comment.body, /\bescalation\b/i);
    return escalationSection !== null && !isNegative(escalationSection) &&
      /\b(?:required|needed|decision|action|approval|review|escalat(?:e|ion))\b/i.test(escalationSection);
  });
}

function deliveryActionOwner(owner: string): DeliveryLane["action"]["owner"] {
  const normalized = owner.trim().toLowerCase();
  if (normalized === "agent:codex" || normalized === "codex") return "codex";
  if (normalized === "agent:chatgpt" || normalized === "chatgpt") return "chatgpt";
  return "unknown";
}

function latestCheckAttempts(checks: RawCheck[]): { checks: RawCheck[]; ambiguous: boolean } {
  const attemptsByIdentity = new Map<string, RawCheck[]>();
  for (const check of checks) {
    const identity = `${check.name}\u0000${check.integrationId ?? ""}`;
    const attempts = attemptsByIdentity.get(identity) ?? [];
    attempts.push(check);
    attemptsByIdentity.set(identity, attempts);
  }

  let ambiguous = false;
  const latest = [...attemptsByIdentity.values()].flatMap((attempts) => {
    if (attempts.length === 1) return attempts;
    if (attempts.some((attempt) => attempt.attemptAt === null)) {
      ambiguous = true;
      return attempts;
    }
    const latestAt = attempts.toSorted((left, right) =>
      (right.attemptAt ?? "").localeCompare(left.attemptAt ?? "")
    )[0]?.attemptAt;
    const latestAttempts = attempts.filter((attempt) => attempt.attemptAt === latestAt);
    if (latestAttempts.length > 1) ambiguous = true;
    return latestAttempts;
  });
  return { checks: latest, ambiguous };
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
  const latest = latestCheckAttempts(pr.checks);
  if (latest.ambiguous) {
    return {
      status: "unknown",
      requiredStatus: "unknown",
      observedHeadSha: pr.checksObservedHeadSha,
      summary: "The latest check attempt could not be identified from GitHub's per-run timestamps.",
      requiredSummary: "Required checks remain unknown because the latest check attempt could not be identified.",
      sourceRefs: refs,
    };
  }
  const checks = latest.checks;
  const required = projectRequiredChecks(pr, checks);
  if (checks.length === 0) {
    return {
      status: "unknown",
      ...required,
      observedHeadSha: pr.checksObservedHeadSha,
      summary: "No check data was returned.",
      sourceRefs: refs,
    };
  }

  const failure = checks.find(
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

  const pending = checks.find((check) => check.status !== "completed" || check.conclusion === null);
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
    summary: `${checks.length} exact-head check${checks.length === 1 ? "" : "s"} passed.`,
    sourceRefs: refs,
  };
}

function projectRequiredChecks(
  pr: RawPullRequest,
  checks: RawCheck[],
): Pick<CheckProjection, "requiredStatus" | "requiredSummary"> {
  if (pr.requiredChecks === null) {
    return { requiredStatus: "unknown", requiredSummary: "The required-check set could not be observed." };
  }
  if (pr.requiredChecks.length === 0) {
    return { requiredStatus: "satisfied", requiredSummary: `No required status checks apply to ${pr.baseRefName}.` };
  }

  for (const required of pr.requiredChecks) {
    const observed = checks.find(
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

function latestReviewsByReviewer(reviews: RawReview[]): RawReview[] {
  const latest = new Map<string, RawReview>();
  for (const review of reviews.toSorted((left, right) => right.submittedAt.localeCompare(left.submittedAt))) {
    const reviewer = review.author?.toLowerCase() ?? review.url;
    if (!latest.has(reviewer)) latest.set(reviewer, review);
  }
  return [...latest.values()];
}

function activeSubstantiveReviewBodies(reviews: RawReview[]): RawReview[] {
  const byReviewer = new Map<string, RawReview[]>();
  for (const review of reviews) {
    const reviewer = review.author?.toLowerCase() ?? review.url;
    const history = byReviewer.get(reviewer) ?? [];
    history.push(review);
    byReviewer.set(reviewer, history);
  }

  const active: RawReview[] = [];
  for (const history of byReviewer.values()) {
    for (const review of history.toSorted((left, right) => right.submittedAt.localeCompare(left.submittedAt))) {
      if (review.state === "dismissed") break;
      if (isSubstantiveReviewBody(review.body)) {
        active.push(review);
        break;
      }
      if (review.state === "approved" || clearsSubstantiveReviewBody(review.body)) break;
    }
  }
  return active;
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

  const relevantReviews = latestReviewsByReviewer(
    pr.reviews.filter((review) => review.state === "approved" || review.state === "changes_requested"),
  );
  const reviewedHeads = new Set(relevantReviews.map((review) => review.headSha));
  const reviewedHeadSha = reviewedHeads.size === 1 ? relevantReviews[0]?.headSha ?? null : null;
  const unresolved = pr.reviewThreads.filter((thread) => !thread.resolved);
  const substantiveReviewBodies = activeSubstantiveReviewBodies(
    pr.reviews.filter((review) =>
      review.state !== "pending" && review.headSha !== null && shaMatches(review.headSha, pr.headSha)
    ),
  );
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
    substantiveUnresolvedCount: substantiveReviewBodies.length + unresolved.filter(
      (thread) => thread.comments.some((comment) => isSubstantiveFinding(comment)),
    ).length,
    sourceRefs: [
      ...refs,
      ...substantiveReviewBodies.map((review) =>
        source("direct", "Current-head substantive review body", review.url, observedAt, review.headSha)
      ),
    ],
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
  handoffRequired: boolean,
  drift: DeliveryLane["authorityDrift"],
  ownershipConflict: boolean,
  ownershipObservationComplete: boolean,
  githubMergeReady: boolean,
  issueScopeReconciled: boolean,
  horizonObserved: boolean,
  humanActionRequested: boolean,
): DeliveryPhase {
  const claimedState = handoff.condition === "current" ? handoff.claimedState?.toLowerCase() ?? "" : "";
  if (readiness === "parked" || statusClaimsParked(claimedState)) return "parked";
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
  if (handoffRequired && handoff.condition !== "current") return "validating";
  if (handoffRequired && !handoffClaimsMergeReady(handoff)) return "validating";
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
  const owner = issueOwner(issue, handoff);
  const normalizedOwner = owner.trim().toLowerCase();
  const alignedHumanPr = pr !== null && pr.author?.type === "user" && normalizedOwner !== "unknown" &&
    !/^agent:/i.test(normalizedOwner) && pr.author.login.trim().toLowerCase() === normalizedOwner;
  const missingHandoffRequiresDelivery = pr !== null && !alignedHumanPr;
  const handoffRequired = pr !== null && (handoff.condition !== "missing" || missingHandoffRequiresDelivery);
  const decisionReadyAuthority = isDecisionReadyAuthorityIssue(issue);
  const decisionReadyScopeReconciled = !decisionReadyAuthority || (pr !== null && isFocusedAuthorityPullRequest(pr));
  const observedReadiness = decisionReadyScopeReconciled ? issueReadiness(issue, handoff) : "unknown";
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
  const crossPrOwnershipConflict = ownershipConflicts.length > 0;
  const ownershipConflict = crossPrOwnershipConflict || multipleIssueClaim || handoffIssueMismatch;
  const nonCurrentPrRequiresSteward = outsideCurrentHorizon && pr !== null;
  const blockers: string[] = [];
  const authoritativeIssueStatus = issueStatusText(issue);
  const issueStatusBlocked = statusClaimsBlocked(authoritativeIssueStatus);
  const issueStatusAffirmsDelivery = statusClaimsActive(authoritativeIssueStatus) || statusClaimsReady(authoritativeIssueStatus);
  const dependencyStateRequiresSteward = issue.blockedBy === null || issue.blockedBy.length > 0;
  const authorityReadinessRequiresSteward = pr !== null && authorityOnlyIssue.test(issue.title) &&
    !decisionReadyAuthority;
  const issueReadinessRequiresSteward = issue.blockedBy !== null && issue.blockedBy.length === 0 &&
    (issueStatusBlocked || authorityReadinessRequiresSteward || (
      !authorityOnlyIssue.test(issue.title) && pr !== null && observedReadiness === "unknown" && !issueStatusAffirmsDelivery
    ));
  const handoffMergeReadinessRequiresDelivery = handoffRequired && handoff.condition === "current" &&
    !handoffClaimsMergeReady(handoff);
  const currentPrePrHandoffBlocked = pr === null && handoff.condition === "current" && observedReadiness === "blocked";

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
  if (issueReadinessRequiresSteward) {
    blockers.push(authorityReadinessRequiresSteward
      ? "The linked Decision or Research Issue is not affirmatively Decision-Ready."
      : "The authoritative Issue status does not affirm that this lane is Ready or active.");
  }
  if (handoffMergeReadinessRequiresDelivery) {
    blockers.push("The current canonical handoff does not affirm merge-ready delivery state.");
  }
  if (pr !== null && checks.status === "unknown") blockers.push(checks.summary);
  if (checks.status === "pending") blockers.push(checks.summary);
  if (pr !== null && checks.requiredStatus !== "satisfied") blockers.push(checks.requiredSummary);
  if (checks.status === "failure") blockers.push(checks.summary);
  if ((reviews.substantiveUnresolvedCount ?? 0) > 0) {
    blockers.push(`${reviews.substantiveUnresolvedCount ?? 0} substantive review finding(s) remain unresolved.`);
  }
  if (pr !== null && reviews.status === "unknown") {
    blockers.push("Reviews were not fully observed for the current PR head.");
  }
  if (reviews.status === "stale") blockers.push("The latest substantive review does not describe the current PR head.");
  if (reviews.decision === "changes_requested") {
    blockers.push("GitHub reports changes requested for the current PR head.");
  } else if (reviews.decision === "review_required") {
    blockers.push("GitHub requires an approving review for the current PR head.");
  } else if (pr !== null && reviews.decision === "unknown") {
    blockers.push("GitHub review decision could not be observed.");
  }
  if (missingHandoffRequiresDelivery && handoff.condition === "missing") {
    blockers.push(`Canonical handoff is missing for pull request #${pr.number}.`);
  }
  if (pr !== null && handoff.condition === "unknown") {
    blockers.push("Canonical handoff could not be fully reconciled with the observed PR and live main.");
  }
  if (handoff.condition === "inconsistent") blockers.push("Canonical handoff conflicts with live PR identity or is duplicated.");
  if (handoff.condition === "stale") blockers.push("Canonical handoff has not reconciled the observed live main.");
  if (pr !== null && snapshot.defaultBranchName === null) {
    blockers.push("Default branch identity could not be observed.");
  } else if (pr !== null && !targetsDefaultBranch) {
    blockers.push(`Pull request #${pr.number} targets ${pr.baseRefName} instead of the live default branch ${snapshot.defaultBranchName ?? "Unknown"}.`);
  }
  if (!ownershipObservationComplete) blockers.push("Pull-request Issue ownership could not be fully observed.");
  if (pr !== null && decisionReadyAuthority && pr.changedPaths === null) {
    blockers.push("Pull-request changed paths could not be fully observed.");
  } else if (pr !== null && decisionReadyAuthority && !decisionReadyScopeReconciled) {
    blockers.push("Decision-Ready authorizes only a focused authority or specification pull request.");
  }
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

  const requiresHuman = statusClaimsHumanRequired(authoritativeIssueStatus) || humanActionRequested(comments, handoff);
  const phase = outsideCurrentHorizon
    ? "parked"
    : derivePhase(
        readiness,
        pr,
        checks,
        reviews,
        handoff,
        handoffRequired,
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
    : crossPrOwnershipConflict
      ? { owner: "human", reason: "Multiple open pull requests claim the same Issue; Project Steward reconciliation is required." }
    : nonCurrentPrRequiresSteward
      ? { owner: "human", reason: "A non-current milestone pull request requires Project Steward roadmap activation." }
    : dependencyStateRequiresSteward
      ? { owner: "human", reason: "Issue dependency state requires Project Steward reconciliation." }
    : !horizonObserved
      ? { owner: "human", reason: "Product Roadmap authority requires Project Steward reconciliation." }
    : issueReadinessRequiresSteward
      ? { owner: "human", reason: "The authoritative Issue status requires Steward readiness action." }
    : phase === "review_fix" || phase === "rereview" || checks.status === "failure" || checks.status === "pending" ||
        (pr !== null && checks.requiredStatus !== "satisfied") || ownershipConflict ||
        !githubMergeReady || drift !== "none" || handoff.condition === "inconsistent" || handoff.condition === "stale" ||
        (pr !== null && handoff.condition === "unknown") ||
        (missingHandoffRequiresDelivery && handoff.condition === "missing") || !issueScopeReconciled ||
        handoffMergeReadinessRequiresDelivery || currentPrePrHandoffBlocked || pr?.isDraft === true ||
        !targetsDefaultBranch || !ownershipObservationComplete
        || !decisionReadyScopeReconciled
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
    authorAssociation: null,
    updatedAt: observedAt,
    lastEditedAt: null,
    milestone: null,
    blockedBy: null,
    commentsComplete: true,
    comments: [],
  };
}

function pullRequestIssueClaims(pr: RawPullRequest, snapshot: RawRepositorySnapshot): number[] {
  const issueNumbers = pr.issueNumbers.length === 0 ? [pr.number] : pr.issueNumbers;
  const handoff = projectHandoff(
    pr.comments,
    pr.commentsComplete,
    snapshot.observedAt,
    pr.headSha,
    snapshot.mainSha,
  );
  return [...new Set([...issueNumbers, ...handoff.observedIssueNumbers])];
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
      pr.issueNumbersComplete && pr.commentsComplete
    );
  const pullRequestsByIssue = new Map<number, number[]>();
  for (const pr of pullRequests) {
    for (const issueNumber of pullRequestIssueClaims(pr, snapshot)) {
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
    const claimedIssueNumbers = pullRequestIssueClaims(pr, snapshot);
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
    const requiresHuman = statusClaimsHumanRequired(issueStatusText(issue)) || humanActionRequested(issue.comments, handoff);
    if (isDecisionReadyAuthorityIssue(issue) && !requiresHuman) continue;
    const hasCanonicalHandoff = canonicalComments(issue.comments).length > 0;
    const owner = issueOwner(issue, handoff).toLowerCase();
    if (!requiresHuman && !hasCanonicalHandoff && !owner.includes("agent:")) continue;
    if (issueReadiness(issue, handoff) === "unknown" && !requiresHuman) continue;
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
  const attentionFailures = snapshot.failures.filter(
    (failure) => failure !== "Recent completion observation failed.",
  );
  const observationIncomplete = snapshot.fetchHealth === "unavailable" ||
    (snapshot.fetchHealth !== "healthy" && (snapshot.failures.length === 0 || attentionFailures.length > 0));
  const horizonKnown = snapshot.productHorizon !== null;
  const deliveryQueueRequiresSteward = !observationIncomplete && deliveries.every(
    (lane) => lane.phase === "parked" && lane.action.owner === "none",
  );

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
      humanActionRequired: humanActions.length > 0 || deliveryQueueRequiresSteward ? true : observationIncomplete ? null : false,
      reasons: humanActions.length > 0
        ? humanActions.map((lane) => `#${lane.issue.number}: ${lane.action.reason}`)
        : deliveryQueueRequiresSteward
        ? ["No Ready delivery remains; the Project Steward must select or ready successor work."]
        : observationIncomplete
        ? attentionFailures.length > 0
          ? attentionFailures
          : ["One or more authoritative sources are unavailable."]
        : [],
      sourceRefs: [source("derived", "Attention classification", snapshot.repoUrl, snapshot.observedAt, snapshot.mainSha)],
    },
  };
}
