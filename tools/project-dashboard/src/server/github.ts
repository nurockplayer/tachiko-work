import type {
  RawCheck,
  RawComment,
  RawIssue,
  RawPullRequest,
  RawRepositorySnapshot,
  RawRequiredCheck,
  RawReview,
  RawReviewThread,
} from "../shared/types.ts";

export interface ReadonlyGithubApi {
  graphql(query: string, variables: Record<string, string | null>): Promise<unknown>;
  rawText(path: string): Promise<string>;
  requiredStatusChecks(owner: string, repo: string, branch: string): Promise<RawRequiredCheck[]>;
  compare(
    owner: string,
    repo: string,
    baseSha: string,
    headSha: string,
  ): Promise<{ status: string; mergeBaseSha: string | null; files: string[] }>;
}

export interface GithubSnapshotOptions {
  owner: string;
  repo: string;
  observedAt?: string;
}

interface PageInfo {
  hasNextPage: boolean;
  endCursor: string | null;
  hasPreviousPage?: boolean;
}

interface GithubComment {
  id: string;
  body: string;
  url: string;
  createdAt: string;
  updatedAt: string;
}

interface GithubIssue {
  number: number;
  title: string;
  url: string;
  body: string;
  updatedAt: string;
  milestone: { title: string } | null;
  blockedBy: { nodes: Array<{ number: number; title: string; url: string }>; pageInfo: PageInfo };
  comments: { nodes: GithubComment[]; pageInfo: PageInfo };
}

interface GithubCheckContext {
  __typename: "CheckRun" | "StatusContext";
  name?: string;
  context?: string;
  status?: string;
  conclusion?: string | null;
  state?: string;
  detailsUrl?: string | null;
  targetUrl?: string | null;
  app?: { databaseId: number } | null;
}

interface GithubReview {
  state: string;
  submittedAt: string | null;
  url: string;
  commit: { oid: string } | null;
}

interface GithubReviewThread {
  isResolved: boolean;
  isOutdated: boolean;
  comments: { nodes: Array<{ body: string; url: string }> };
}

interface GithubPullRequest {
  number: number;
  title: string;
  url: string;
  body: string;
  headRefOid: string;
  baseRefOid: string;
  baseRefName: string;
  updatedAt: string;
  closingIssuesReferences: { nodes: Array<{ number: number }> };
  comments: { nodes: GithubComment[]; pageInfo: PageInfo };
  commits: {
    nodes: Array<{
      commit: {
        oid: string;
        statusCheckRollup: null | { contexts: { nodes: GithubCheckContext[] } };
      };
    }>;
  };
  reviewDecision: string | null;
  reviews: { nodes: GithubReview[] };
  reviewThreads: { nodes: GithubReviewThread[]; pageInfo?: PageInfo };
}

interface GithubMergedPullRequest {
  number: number;
  title: string;
  url: string;
  mergedAt: string | null;
  mergeCommit: { oid: string } | null;
  author: { login: string } | null;
}

interface GithubPage {
  repository: null | {
    url: string;
    defaultBranchRef: null | { name: string; target: { oid: string } };
    issues: { nodes: GithubIssue[]; pageInfo: PageInfo };
    pullRequests: { nodes: GithubPullRequest[]; pageInfo: PageInfo };
    mergedPullRequests: { nodes: GithubMergedPullRequest[]; pageInfo: PageInfo };
  };
}

const dashboardQuery = `
  query DashboardProjection(
    $owner: String!
    $repo: String!
    $issueCursor: String
    $prCursor: String
  ) {
    repository(owner: $owner, name: $repo) {
      url
      defaultBranchRef {
        name
        target { ... on Commit { oid } }
      }
      issues(first: 50, after: $issueCursor, states: OPEN, orderBy: { field: UPDATED_AT, direction: DESC }) {
        nodes {
          number title url body updatedAt
          milestone { title }
          blockedBy(first: 25) {
            nodes { number title url }
            pageInfo { hasNextPage }
          }
          comments(last: 100) {
            nodes { id body url createdAt updatedAt }
            pageInfo { hasPreviousPage }
          }
        }
        pageInfo { hasNextPage endCursor }
      }
      pullRequests(first: 50, after: $prCursor, states: OPEN, orderBy: { field: UPDATED_AT, direction: DESC }) {
        nodes {
          number title url body headRefOid baseRefOid baseRefName updatedAt
          closingIssuesReferences(first: 20) { nodes { number } }
          comments(last: 100) {
            nodes { id body url createdAt updatedAt }
            pageInfo { hasPreviousPage }
          }
          commits(last: 1) {
            nodes {
              commit {
                oid
                statusCheckRollup {
                  contexts(first: 100) {
                    nodes {
                      __typename
                      ... on CheckRun { name status conclusion detailsUrl app { databaseId } }
                      ... on StatusContext { context state targetUrl }
                    }
                  }
                }
              }
            }
          }
          reviewDecision
          reviews(last: 100) {
            nodes { state submittedAt url commit { oid } }
          }
          reviewThreads(first: 100) {
            nodes {
              isResolved isOutdated
              comments(first: 1) { nodes { body url } }
            }
            pageInfo { hasNextPage }
          }
        }
        pageInfo { hasNextPage endCursor }
      }
      mergedPullRequests: pullRequests(first: 8, states: MERGED, orderBy: { field: UPDATED_AT, direction: DESC }) {
        nodes { number title url mergedAt mergeCommit { oid } author { login } }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
`;

function asComment(comment: GithubComment): RawComment {
  return {
    id: comment.id,
    body: comment.body,
    url: comment.url,
    createdAt: comment.createdAt,
    updatedAt: comment.updatedAt,
  };
}

function normalizeCheckStatus(status: string | undefined): RawCheck["status"] {
  if (status === "COMPLETED") return "completed";
  if (status === "IN_PROGRESS" || status === "WAITING" || status === "PENDING") return "in_progress";
  if (status === "QUEUED" || status === "REQUESTED") return "queued";
  return "unknown";
}

function normalizeConclusion(conclusion: string | null | undefined): RawCheck["conclusion"] {
  if (conclusion === null || conclusion === undefined) return null;
  if (conclusion === "SUCCESS") return "success";
  if (["FAILURE", "TIMED_OUT", "ACTION_REQUIRED", "STARTUP_FAILURE"].includes(conclusion)) return "failure";
  if (conclusion === "CANCELLED") return "cancelled";
  if (conclusion === "NEUTRAL") return "neutral";
  if (conclusion === "SKIPPED") return "skipped";
  if (conclusion === "STALE") return "stale";
  return "failure";
}

function asCheck(context: GithubCheckContext): RawCheck {
  if (context.__typename === "StatusContext") {
    const state = context.state ?? "UNKNOWN";
    return {
      name: context.context ?? "commit status",
      integrationId: null,
      status: state === "PENDING" || state === "EXPECTED" ? "in_progress" : "completed",
      conclusion: state === "SUCCESS" ? "success" : state === "PENDING" || state === "EXPECTED" ? null : "failure",
      url: context.targetUrl ?? null,
    };
  }
  return {
    name: context.name ?? "check run",
    integrationId: context.app?.databaseId ?? null,
    status: normalizeCheckStatus(context.status),
    conclusion: normalizeConclusion(context.conclusion),
    url: context.detailsUrl ?? null,
  };
}

function normalizeReviewState(state: string): RawReview["state"] {
  if (state === "APPROVED") return "approved";
  if (state === "CHANGES_REQUESTED") return "changes_requested";
  if (state === "COMMENTED") return "commented";
  if (state === "DISMISSED") return "dismissed";
  if (state === "PENDING") return "pending";
  return "unknown";
}

function normalizeReviewDecision(value: string | null): RawPullRequest["reviewDecision"] {
  if (value === "APPROVED") return "approved";
  if (value === "CHANGES_REQUESTED") return "changes_requested";
  if (value === "REVIEW_REQUIRED") return "review_required";
  return "unknown";
}

function asReview(review: GithubReview): RawReview {
  return {
    state: normalizeReviewState(review.state),
    headSha: review.commit?.oid ?? null,
    url: review.url,
    submittedAt: review.submittedAt ?? "1970-01-01T00:00:00.000Z",
  };
}

function asReviewThread(thread: GithubReviewThread): RawReviewThread {
  const first = thread.comments.nodes[0];
  return {
    resolved: thread.isResolved,
    outdated: thread.isOutdated,
    body: first?.body ?? "Unclassified unresolved review thread",
    url: first?.url ?? "",
  };
}

function extractHorizon(markdown: string): string | null {
  const currentSection = markdown.match(/## Current horizon([\s\S]*?)(?:\n## |$)/i)?.[1] ?? markdown;
  return currentSection.match(/>\s*\*\*(.+?)\*\*/)?.[1]?.trim() ?? null;
}

function relationFromCompare(status: string): RawPullRequest["relationToMain"] {
  if (status === "ahead" || status === "identical") return "current";
  if (status === "behind") return "behind";
  if (status === "diverged") return "diverged";
  return "unknown";
}

function isAuthorityPath(path: string): boolean {
  return /^(?:AGENTS\.md|CONTRIBUTING\.md|SECURITY\.md|docs\/(?:decisions|governance|specs|vision)\/|docs\/product\/product-roadmap\.md)/.test(path);
}

export async function loadGithubSnapshot(
  api: ReadonlyGithubApi,
  options: GithubSnapshotOptions,
): Promise<RawRepositorySnapshot> {
  const observedAt = options.observedAt ?? new Date().toISOString();
  const repoName = `${options.owner}/${options.repo}`;
  const fallbackRepoUrl = `https://github.com/${repoName}`;
  const failures: string[] = [];
  const issues = new Map<number, GithubIssue>();
  const pullRequests = new Map<number, GithubPullRequest>();
  const mergedPullRequests = new Map<number, GithubMergedPullRequest>();
  let issueCursor: string | null = null;
  let prCursor: string | null = null;
  let repository: NonNullable<GithubPage["repository"]> | undefined;

  try {
    for (;;) {
      const response = (await api.graphql(dashboardQuery, {
        owner: options.owner,
        repo: options.repo,
        issueCursor,
        prCursor,
      })) as GithubPage;
      if (response.repository === null) throw new Error("repository not found");
      repository = response.repository;
      for (const issue of repository.issues.nodes) issues.set(issue.number, issue);
      for (const pr of repository.pullRequests.nodes) pullRequests.set(pr.number, pr);
      for (const pr of repository.mergedPullRequests.nodes) mergedPullRequests.set(pr.number, pr);

      const issueMore = repository.issues.pageInfo.hasNextPage;
      const prMore = repository.pullRequests.pageInfo.hasNextPage;
      if (!issueMore && !prMore) break;
      issueCursor = repository.issues.pageInfo.endCursor;
      prCursor = repository.pullRequests.pageInfo.endCursor;
    }
  } catch {
    return {
      repoName,
      repoUrl: fallbackRepoUrl,
      observedAt,
      mainSha: null,
      productHorizon: null,
      productHorizonUrl: `${fallbackRepoUrl}/blob/main/docs/product/product-roadmap.md`,
      fetchHealth: "unavailable",
      failures: ["GitHub repository observation failed."],
      issues: null,
      pullRequests: null,
      recentCompletions: null,
    };
  }

  const repoUrl = repository.url;
  const mainSha = repository.defaultBranchRef?.target.oid ?? null;
  let productHorizon: string | null = null;
  const productHorizonUrl = `${repoUrl}/blob/${mainSha ?? "main"}/docs/product/product-roadmap.md`;
  if (mainSha === null) {
    failures.push("Live main identity was unavailable.");
  } else {
    try {
      const roadmap = await api.rawText(
        `/repos/${options.owner}/${options.repo}/contents/docs/product/product-roadmap.md?ref=${mainSha}`,
      );
      productHorizon = extractHorizon(roadmap);
      if (productHorizon === null) failures.push("Product Roadmap current horizon could not be parsed.");
    } catch {
      failures.push("Product Roadmap observation failed.");
    }
  }

  const rawIssues: RawIssue[] = [];
  for (const issue of issues.values()) {
    const dependenciesComplete = !issue.blockedBy.pageInfo.hasNextPage;
    const commentsComplete = !issue.comments.pageInfo.hasPreviousPage;
    if (!dependenciesComplete) failures.push(`Issue #${issue.number} dependency observation was truncated.`);
    if (!commentsComplete) failures.push(`Issue #${issue.number} handoff observation was truncated.`);
    rawIssues.push({
      number: issue.number,
      title: issue.title,
      url: issue.url,
      body: issue.body,
      updatedAt: issue.updatedAt,
      milestone: issue.milestone?.title ?? null,
      blockedBy: dependenciesComplete ? issue.blockedBy.nodes : null,
      comments: commentsComplete ? issue.comments.nodes.map(asComment) : [],
    });
  }

  const rawPullRequests: RawPullRequest[] = [];
  const requiredChecksByBranch = new Map<string, RawRequiredCheck[] | null>();
  for (const pr of pullRequests.values()) {
    let relationToMain: RawPullRequest["relationToMain"] = "unknown";
    let mergeBaseSha: string | null = null;
    let authorityPathsChangedOnMain: string[] | null = null;
    if (mainSha === null) {
      failures.push(`PR #${pr.number} relation-to-main observation failed.`);
    } else {
      try {
        const comparison = await api.compare(options.owner, options.repo, mainSha, pr.headRefOid);
        relationToMain = relationFromCompare(comparison.status);
        mergeBaseSha = comparison.mergeBaseSha;
        if (mergeBaseSha === null) {
          failures.push(`PR #${pr.number} merge-base observation failed.`);
        } else if (mergeBaseSha === mainSha) {
          authorityPathsChangedOnMain = [];
        }
      } catch {
        failures.push(`PR #${pr.number} relation-to-main observation failed.`);
      }
      if (mergeBaseSha !== null && mergeBaseSha !== mainSha) {
        try {
          const mainChanges = await api.compare(options.owner, options.repo, mergeBaseSha, mainSha);
          authorityPathsChangedOnMain = mainChanges.files.filter(isAuthorityPath).toSorted();
        } catch {
          failures.push(`PR #${pr.number} authority-change observation failed.`);
        }
      }
    }

    const commit = pr.commits.nodes[0]?.commit;
    if (!requiredChecksByBranch.has(pr.baseRefName)) {
      try {
        requiredChecksByBranch.set(
          pr.baseRefName,
          await api.requiredStatusChecks(options.owner, options.repo, pr.baseRefName),
        );
      } catch {
        requiredChecksByBranch.set(pr.baseRefName, null);
        failures.push(`PR #${pr.number} required-check observation failed.`);
      }
    }
    const commentsComplete = !pr.comments.pageInfo.hasPreviousPage;
    const reviewThreadsComplete = !(pr.reviewThreads.pageInfo?.hasNextPage ?? false);
    if (!commentsComplete) failures.push(`PR #${pr.number} handoff observation was truncated.`);
    if (!reviewThreadsComplete) failures.push(`PR #${pr.number} review-thread observation was truncated.`);
    rawPullRequests.push({
      number: pr.number,
      title: pr.title,
      url: pr.url,
      body: pr.body,
      headSha: pr.headRefOid,
      baseRefName: pr.baseRefName,
      baseSha: pr.baseRefOid,
      mergeBaseSha,
      relationToMain,
      authorityPathsChangedOnMain,
      issueNumbers: pr.closingIssuesReferences.nodes.map((issue) => issue.number),
      comments: commentsComplete ? pr.comments.nodes.map(asComment) : [],
      checksObservedHeadSha: commit?.oid ?? null,
      checks: commit?.statusCheckRollup?.contexts.nodes.map(asCheck) ?? null,
      requiredChecks: requiredChecksByBranch.get(pr.baseRefName) ?? null,
      reviewDecision: normalizeReviewDecision(pr.reviewDecision),
      reviews: pr.reviews.nodes.map(asReview),
      reviewThreads: reviewThreadsComplete ? pr.reviewThreads.nodes.map(asReviewThread) : null,
      updatedAt: pr.updatedAt,
    });
  }

  const recentCompletions = [...mergedPullRequests.values()]
    .filter((pr): pr is GithubMergedPullRequest & { mergedAt: string } => pr.mergedAt !== null)
    .toSorted((left, right) => right.mergedAt.localeCompare(left.mergedAt))
    .slice(0, 8)
    .map((pr) => ({
      number: pr.number,
      title: pr.title,
      url: pr.url,
      mergedAt: pr.mergedAt,
      mergeSha: pr.mergeCommit?.oid ?? null,
      author: pr.author?.login ?? "unknown",
    }));

  return {
    repoName,
    repoUrl,
    observedAt,
    mainSha,
    productHorizon,
    productHorizonUrl,
    fetchHealth: failures.length === 0 ? "healthy" : "partial",
    failures,
    issues: rawIssues,
    pullRequests: rawPullRequests,
    recentCompletions,
  };
}

export class GithubApiClient implements ReadonlyGithubApi {
  readonly #token: string;
  readonly #fetch: typeof fetch;

  constructor(token: string, fetchImplementation: typeof fetch = fetch) {
    this.#token = token;
    this.#fetch = fetchImplementation;
  }

  async graphql(query: string, variables: Record<string, string | null>): Promise<unknown> {
    const response = await this.#fetch("https://api.github.com/graphql", {
      method: "POST",
      headers: this.#headers("application/vnd.github+json"),
      body: JSON.stringify({ query, variables }),
    });
    return this.#readJson(response);
  }

  async rawText(path: string): Promise<string> {
    const response = await this.#fetch(`https://api.github.com${path}`, {
      method: "GET",
      headers: this.#headers("application/vnd.github.raw+json"),
    });
    if (!response.ok) throw new Error(`GitHub content request failed with ${response.status}`);
    return response.text();
  }

  async requiredStatusChecks(owner: string, repo: string, branch: string): Promise<RawRequiredCheck[]> {
    const path = `/repos/${owner}/${repo}/rules/branches/${encodeURIComponent(branch)}`;
    const response = await this.#fetch(`https://api.github.com${path}`, {
      method: "GET",
      headers: this.#headers("application/vnd.github+json"),
    });
    const rules = (await this.#readJson(response)) as Array<{
      type?: string;
      parameters?: { required_status_checks?: Array<{ context?: string; integration_id?: number | null }> };
    }>;
    return rules
      .filter((rule) => rule.type === "required_status_checks")
      .flatMap((rule) => rule.parameters?.required_status_checks ?? [])
      .flatMap((check) => check.context === undefined
        ? []
        : [{ name: check.context, integrationId: check.integration_id ?? null }])
      .toSorted((left, right) => left.name.localeCompare(right.name) || (left.integrationId ?? 0) - (right.integrationId ?? 0));
  }

  async compare(
    owner: string,
    repo: string,
    baseSha: string,
    headSha: string,
  ): Promise<{ status: string; mergeBaseSha: string | null; files: string[] }> {
    const path = `/repos/${owner}/${repo}/compare/${encodeURIComponent(baseSha)}...${encodeURIComponent(headSha)}`;
    const response = await this.#fetch(`https://api.github.com${path}`, {
      method: "GET",
      headers: this.#headers("application/vnd.github+json"),
    });
    const value = (await this.#readJson(response)) as {
      status?: string;
      merge_base_commit?: { sha?: string };
      files?: Array<{ filename?: string }>;
    };
    return {
      status: value.status ?? "unknown",
      mergeBaseSha: value.merge_base_commit?.sha ?? null,
      files: value.files?.flatMap((file) => file.filename === undefined ? [] : [file.filename]) ?? [],
    };
  }

  #headers(accept: string): HeadersInit {
    return {
      Accept: accept,
      Authorization: `Bearer ${this.#token}`,
      "Content-Type": "application/json",
      "User-Agent": "tachiko-work-project-dashboard",
      "X-GitHub-Api-Version": "2022-11-28",
    };
  }

  async #readJson(response: Response): Promise<unknown> {
    if (!response.ok) throw new Error(`GitHub request failed with ${response.status}`);
    const value = (await response.json()) as { errors?: unknown[] };
    if (Array.isArray(value.errors) && value.errors.length > 0) throw new Error("GitHub GraphQL query returned errors");
    return "data" in value ? (value as { data: unknown }).data : value;
  }
}
