import { normalizeRepositorySnapshot } from "./normalize.ts";
import type { RawIssue, RawPullRequest, RawRepositorySnapshot, RepositoryProjection } from "../shared/types.ts";

const observedAt = "2026-08-30T00:00:00.000Z";
const mainSha = "18044aa247ff886f4944c973104f2fc8494bd435";
const dashboardHead = "b".repeat(40);
const reviewHead = "c".repeat(40);

function issue(
  number: number,
  title: string,
  milestone: string | null,
  state: string,
): RawIssue {
  return {
    number,
    title,
    url: `https://github.com/nurockplayer/tachiko-work/issues/${number}`,
    body: `## Status\n\n**READY FOR BOUNDED IMPLEMENTATION**\n\nOwner: \`agent:codex\``,
    updatedAt: observedAt,
    lastEditedAt: null,
    milestone,
    blockedBy: [],
    commentsComplete: true,
    comments: [
      {
        id: `issue-${number}-handoff`,
        body: `<!-- agent-handoff:v1 -->\nOWNER: agent:codex\nSTATE: ${state}`,
        url: `https://github.com/nurockplayer/tachiko-work/issues/${number}#issuecomment-${number}`,
        createdAt: observedAt,
        updatedAt: observedAt,
      },
    ],
  };
}

function staleDashboardPr(): RawPullRequest {
  return {
    number: 200,
    title: "Build repository-local Dashboard v0",
    url: "https://github.com/nurockplayer/tachiko-work/pull/200",
    body: "Closes #169",
    isDraft: false,
    headSha: dashboardHead,
    baseRefName: "main",
    baseSha: mainSha,
    mergeBaseSha: "d".repeat(40),
    relationToMain: "diverged",
    authorityPathsChangedOnMain: ["docs/governance/project-governance.md"],
    mergeable: "mergeable",
    mergeStateStatus: "blocked",
    issueNumbers: [169],
    commentsComplete: true,
    comments: [
      {
        id: "pr-200-handoff",
        body: `<!-- agent-handoff:v1 -->\nOWNER: agent:codex\nSTATE: merge-ready\nHEAD: ${"a".repeat(40)}\nLAST CHECKED MAIN: ${mainSha}`,
        url: "https://github.com/nurockplayer/tachiko-work/pull/200#issuecomment-200",
        createdAt: observedAt,
        updatedAt: observedAt,
      },
    ],
    checksObservedHeadSha: "a".repeat(40),
    checks: [{ name: "project-dashboard", integrationId: null, status: "completed", conclusion: "success", url: null }],
    requiredChecks: [],
    reviewDecision: "review_required",
    reviews: [],
    reviewThreads: [],
    updatedAt: observedAt,
  };
}

function reviewFixPr(): RawPullRequest {
  return {
    number: 201,
    title: "PPTX projection research",
    url: "https://github.com/nurockplayer/tachiko-work/pull/201",
    body: "Closes #163",
    isDraft: false,
    headSha: reviewHead,
    baseRefName: "main",
    baseSha: mainSha,
    mergeBaseSha: mainSha,
    relationToMain: "current",
    authorityPathsChangedOnMain: [],
    mergeable: "mergeable",
    mergeStateStatus: "blocked",
    issueNumbers: [163],
    commentsComplete: true,
    comments: [
      {
        id: "pr-201-handoff",
        body: `<!-- agent-handoff:v1 -->\nOWNER: agent:codex\nSTATE: review_fix\nHEAD: ${reviewHead}\nLAST CHECKED MAIN: ${mainSha}`,
        url: "https://github.com/nurockplayer/tachiko-work/pull/201#issuecomment-201",
        createdAt: observedAt,
        updatedAt: observedAt,
      },
    ],
    checksObservedHeadSha: reviewHead,
    checks: [{ name: "research-probe", integrationId: null, status: "completed", conclusion: "success", url: null }],
    requiredChecks: [],
    reviewDecision: "changes_requested",
    reviews: [
      {
        state: "changes_requested",
        headSha: reviewHead,
        url: "https://github.com/nurockplayer/tachiko-work/pull/201#pullrequestreview-1",
        submittedAt: observedAt,
      },
    ],
    reviewThreads: [
      {
        resolved: false,
        outdated: false,
        comments: ["[P2] Keep target identity out of Tachiko semantics"],
        url: "https://github.com/nurockplayer/tachiko-work/pull/201#discussion_r1",
      },
    ],
    updatedAt: observedAt,
  };
}

export function fixtureProjection(): RepositoryProjection {
  const raw: RawRepositorySnapshot = {
    repoName: "nurockplayer/tachiko-work",
    repoUrl: "https://github.com/nurockplayer/tachiko-work",
    observedAt,
    mainSha,
    productHorizon: "05 · Designer MVP",
    productHorizonUrl: `https://github.com/nurockplayer/tachiko-work/blob/${mainSha}/docs/product/product-roadmap.md`,
    fetchHealth: "healthy",
    failures: [],
    issues: [
      issue(187, "Open canonical project, atomic Save As, and reopen", "05 · Designer MVP", "ready"),
      issue(169, "Build read-only repository-local live project control room", null, "implementing"),
      issue(163, "Dogfood adapter-only PPTX projection", null, "review_fix"),
    ],
    pullRequests: [staleDashboardPr(), reviewFixPr()],
    recentCompletions: [
      {
        number: 186,
        title: "Ship the first Rust-authoritative web slice",
        url: "https://github.com/nurockplayer/tachiko-work/pull/186",
        mergedAt: "2026-08-29T18:45:00.000Z",
        mergeSha: mainSha,
        mergedBy: "nurockplayer",
      },
    ],
  };
  return normalizeRepositorySnapshot(raw);
}
