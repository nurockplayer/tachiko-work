import type {
  GitHubAuthorAssociation,
  GitHubCommentKind,
} from "@tachiko-work/operational-evidence";

import type {
  ObservationAvailability,
  RawCheck,
  RawComment,
  RawPullRequest,
  RepositoryObservation,
} from "../shared/model.js";

const REPOSITORY = "nurockplayer/tachiko-work";
const OWNER = "nurockplayer";
const NAME = "tachiko-work";
const API_URL = "https://api.github.com";
const GRAPHQL_URL = `${API_URL}/graphql`;
const ROADMAP_PATH = "docs/product/product-roadmap.md";
const REQUEST_TIMEOUT_MS = 15_000;

const QUERY = `
  query DashboardRepository($owner: String!, $name: String!) {
    repository(owner: $owner, name: $name) {
      url
      defaultBranchRef {
        name
        target {
          ... on Commit { oid url }
        }
      }
      roadmap: object(expression: "main:${ROADMAP_PATH}") {
        ... on Blob { text oid }
      }
      issues(first: 100, states: OPEN, orderBy: {field: UPDATED_AT, direction: DESC}) {
        pageInfo { hasNextPage }
        nodes {
          number title url state
          labels(first: 30) { pageInfo { hasNextPage } nodes { name } }
          milestone { title }
          blockedBy(first: 100) {
            pageInfo { hasNextPage }
            nodes { number state url }
          }
        }
      }
      pullRequests(first: 40, states: OPEN, orderBy: {field: UPDATED_AT, direction: DESC}) {
        pageInfo { hasNextPage }
        nodes {
          number title url state isDraft headRefOid baseRefOid baseRefName mergeable
          mergeStateStatus reviewDecision
          closingIssuesReferences(first: 20) {
            pageInfo { hasNextPage }
            nodes { number }
          }
          comments(first: 100) {
            pageInfo { hasNextPage }
            nodes {
              id databaseId body url createdAt updatedAt lastEditedAt
              author { login }
              authorAssociation
            }
          }
          reviews(first: 100) {
            pageInfo { hasNextPage }
            nodes {
              id fullDatabaseId body url createdAt updatedAt lastEditedAt state submittedAt
              author { login }
              authorAssociation
              commit { oid }
            }
          }
          reviewThreads(first: 100) {
            pageInfo { hasNextPage }
            nodes {
              id isResolved isOutdated
              comments(first: 100) {
                pageInfo { hasNextPage }
                nodes {
                  id databaseId body url createdAt updatedAt lastEditedAt
                  author { login }
                  authorAssociation
                }
              }
            }
          }
          statusCheckRollup {
            contexts(first: 100) {
              pageInfo { hasNextPage }
              nodes {
                __typename
                ... on CheckRun { id name status conclusion url detailsUrl }
                ... on StatusContext { id context state targetUrl commit { oid } }
              }
            }
          }
        }
      }
      recent: pullRequests(first: 8, states: MERGED, orderBy: {field: UPDATED_AT, direction: DESC}) {
        pageInfo { hasNextPage }
        nodes { number title url mergedAt mergeCommit { oid } }
      }
    }
  }
`;

interface PageInfo {
  hasNextPage: boolean;
}

interface Actor {
  login: string;
}

interface GraphComment {
  id: string;
  databaseId?: number | null;
  body: string;
  url: string;
  createdAt: string;
  updatedAt: string;
  lastEditedAt?: string | null;
  author: Actor | null;
  authorAssociation: GitHubAuthorAssociation;
}

interface GraphReview extends GraphComment {
  fullDatabaseId: string | null;
  submittedAt: string | null;
  state: "APPROVED" | "CHANGES_REQUESTED" | "COMMENTED" | "DISMISSED" | "PENDING";
  commit: { oid: string } | null;
}

interface GraphThread {
  id: string;
  isResolved: boolean;
  isOutdated: boolean;
  comments: { pageInfo: PageInfo; nodes: (GraphComment | null)[] | null };
}

interface GraphCheckRun {
  __typename: "CheckRun";
  id: string;
  name: string;
  status: string;
  conclusion: string | null;
  url: string;
  detailsUrl: string | null;
}

interface GraphStatusContext {
  __typename: "StatusContext";
  id: string;
  context: string;
  state: string;
  targetUrl: string | null;
  commit: { oid: string };
}

type GraphCheck = GraphCheckRun | GraphStatusContext;

interface GraphPull {
  number: number;
  title: string;
  url: string;
  state: "OPEN" | "CLOSED" | "MERGED";
  isDraft: boolean;
  headRefOid: string;
  baseRefOid: string;
  baseRefName: string;
  mergeable: "MERGEABLE" | "CONFLICTING" | "UNKNOWN";
  mergeStateStatus:
    | "DIRTY"
    | "UNKNOWN"
    | "BLOCKED"
    | "BEHIND"
    | "UNSTABLE"
    | "HAS_HOOKS"
    | "CLEAN";
  reviewDecision: "CHANGES_REQUESTED" | "APPROVED" | "REVIEW_REQUIRED" | null;
  closingIssuesReferences: {
    pageInfo: PageInfo;
    nodes: ({ number: number } | null)[] | null;
  } | null;
  comments: { pageInfo: PageInfo; nodes: (GraphComment | null)[] | null };
  reviews: { pageInfo: PageInfo; nodes: (GraphReview | null)[] | null } | null;
  reviewThreads: { pageInfo: PageInfo; nodes: (GraphThread | null)[] | null };
  statusCheckRollup: {
    contexts: { pageInfo: PageInfo; nodes: (GraphCheck | null)[] | null };
  } | null;
}

interface GraphIssue {
  number: number;
  title: string;
  url: string;
  state: "OPEN" | "CLOSED";
  labels: { pageInfo: PageInfo; nodes: ({ name: string } | null)[] | null } | null;
  milestone: { title: string } | null;
  blockedBy: {
    pageInfo: PageInfo;
    nodes: ({ number: number; state: "OPEN" | "CLOSED"; url: string } | null)[] | null;
  };
}

interface GraphRecentPull {
  number: number;
  title: string;
  url: string;
  mergedAt: string | null;
  mergeCommit: { oid: string } | null;
}

interface GraphRepository {
  url: string;
  defaultBranchRef: {
    name: string;
    target: { oid: string; url: string };
  } | null;
  roadmap: unknown;
  issues: {
    pageInfo: PageInfo;
    nodes: (GraphIssue | null)[] | null;
  };
  pullRequests: { pageInfo: PageInfo; nodes: (GraphPull | null)[] | null };
  recent: {
    pageInfo: PageInfo;
    nodes: (GraphRecentPull | null)[] | null;
  };
}

interface GraphResponse {
  data?: { repository: GraphRepository | null };
  errors?: { message: string }[];
}

interface CompareResponse {
  status: "ahead" | "behind" | "diverged" | "identical";
  merge_base_commit: { sha: string };
  files?: { filename: string; previous_filename?: string }[];
}

interface RequestOptions {
  token?: string;
  fetchImpl?: typeof fetch;
}

function requestHeaders(token?: string): Record<string, string> {
  return {
    Accept: "application/vnd.github+json",
    "Content-Type": "application/json",
    "X-GitHub-Api-Version": "2022-11-28",
    ...(token === undefined ? {} : { Authorization: `Bearer ${token}` }),
  };
}

async function githubRequest<T>(
  url: string,
  init: RequestInit,
  options: RequestOptions,
): Promise<T> {
  const fetchImpl = options.fetchImpl ?? fetch;
  const response = await fetchImpl(url, {
    ...init,
    headers: requestHeaders(options.token),
    signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
  });
  if (!response.ok) {
    throw new Error(`GitHub observation failed with HTTP ${String(response.status)}`);
  }
  return (await response.json()) as T;
}

function stableSourceId(comment: GraphComment): string {
  return comment.id;
}

function trustedProducer(comment: GraphComment): boolean {
  return comment.author?.login === OWNER && comment.authorAssociation === "OWNER";
}

function rawComment(
  comment: GraphComment,
  kind: GitHubCommentKind,
  topLevel: boolean,
): RawComment {
  return {
    body: comment.body,
    id: stableSourceId(comment),
    kind,
    authorLogin: comment.author?.login ?? "unknown",
    authorAssociation: comment.authorAssociation,
    url: comment.url,
    createdAt: comment.createdAt,
    updatedAt: comment.updatedAt,
    edited: comment.lastEditedAt !== null && comment.lastEditedAt !== undefined,
    topLevel,
    trustedProducer: trustedProducer(comment),
  };
}

function checkState(check: GraphCheck): RawCheck["status"] {
  if (check.__typename === "StatusContext") {
    if (check.state === "SUCCESS") return "success";
    if (check.state === "PENDING" || check.state === "EXPECTED") return "pending";
    return "failure";
  }
  if (check.status !== "COMPLETED") return "pending";
  return check.conclusion === "SUCCESS" ? "success" : "failure";
}

function rawCheck(check: GraphCheck, headSha: string): RawCheck {
  if (check.__typename === "StatusContext") {
    return {
      name: check.context,
      headSha: check.commit.oid,
      status: checkState(check),
      url: check.targetUrl ?? `https://github.com/${REPOSITORY}/commit/${headSha}/checks`,
    };
  }
  return {
    name: check.name,
    headSha,
    status: checkState(check),
    url: check.detailsUrl ?? check.url,
  };
}

function presentNodes<T>(nodes: readonly (T | null)[] | null): T[] {
  return nodes?.filter((node): node is T => node !== null) ?? [];
}

function hasMissingNode(nodes: readonly unknown[] | null): boolean {
  return nodes === null || nodes.some((node) => node === null);
}

function roadmapText(value: unknown): string | null {
  if (typeof value !== "object" || value === null || !("text" in value)) return null;
  const text = value.text;
  return typeof text === "string" && text.trim().length > 0 ? text : null;
}

function hasNestedTruncation(pull: GraphPull): boolean {
  const threads = presentNodes(pull.reviewThreads.nodes);
  return (
    pull.closingIssuesReferences === null ||
    pull.closingIssuesReferences.pageInfo.hasNextPage ||
    hasMissingNode(pull.closingIssuesReferences.nodes) ||
    pull.comments.pageInfo.hasNextPage ||
    hasMissingNode(pull.comments.nodes) ||
    pull.reviews === null ||
    pull.reviews.pageInfo.hasNextPage ||
    hasMissingNode(pull.reviews.nodes) ||
    pull.reviewThreads.pageInfo.hasNextPage ||
    hasMissingNode(pull.reviewThreads.nodes) ||
    threads.some(
      (thread) =>
        thread.comments.pageInfo.hasNextPage || hasMissingNode(thread.comments.nodes),
    ) ||
    (pull.statusCheckRollup?.contexts.pageInfo.hasNextPage ?? false) ||
    (pull.statusCheckRollup === null
      ? false
      : hasMissingNode(pull.statusCheckRollup.contexts.nodes))
  );
}

function commentsTruncated(pull: GraphPull): boolean {
  const threads = presentNodes(pull.reviewThreads.nodes);
  return (
    pull.comments.pageInfo.hasNextPage ||
    hasMissingNode(pull.comments.nodes) ||
    pull.reviews === null ||
    pull.reviews.pageInfo.hasNextPage ||
    hasMissingNode(pull.reviews.nodes) ||
    pull.reviewThreads.pageInfo.hasNextPage ||
    hasMissingNode(pull.reviewThreads.nodes) ||
    threads.some(
      (thread) =>
        thread.comments.pageInfo.hasNextPage || hasMissingNode(thread.comments.nodes),
    )
  );
}

function topLevelCommentsTruncated(pull: GraphPull): boolean {
  return (
    pull.comments.pageInfo.hasNextPage ||
    hasMissingNode(pull.comments.nodes) ||
    pull.reviews === null ||
    pull.reviews.pageInfo.hasNextPage ||
    hasMissingNode(pull.reviews.nodes)
  );
}

function isAuthorityPath(path: string): boolean {
  return (
    (path === "AGENTS.md" || path.endsWith("/AGENTS.md")) ||
    path === "Cargo.toml" ||
    path === "Cargo.lock" ||
    path === "CONTRIBUTING.md" ||
    path === "SECURITY.md" ||
    path === ROADMAP_PATH ||
    path.startsWith(".github/workflows/") ||
    path.startsWith("scripts/") ||
    path.startsWith("docs/architecture/") ||
    path.startsWith("docs/decisions/") ||
    path.startsWith("docs/governance/") ||
    path.startsWith("docs/product/") ||
    path.startsWith("docs/security/") ||
    path.startsWith("docs/specs/") ||
    path.startsWith("docs/vision/")
  );
}

function nativeMergePolicy(pull: GraphPull): RawPullRequest["nativeMergePolicy"] {
  if (pull.reviewDecision === "CHANGES_REQUESTED") return "blocked";
  if (
    pull.mergeStateStatus === "DIRTY" ||
    pull.mergeStateStatus === "BLOCKED" ||
    pull.mergeStateStatus === "BEHIND"
  ) {
    return "blocked";
  }
  if (pull.mergeStateStatus === "UNKNOWN") return "unknown";
  if (pull.reviewDecision === "REVIEW_REQUIRED") return "waiting";
  return "satisfied";
}

async function comparePull(
  mainSha: string,
  headSha: string,
  options: RequestOptions,
): Promise<{
  mergeBaseSha: string;
  relation: RawPullRequest["relationToMain"];
  authorityChanges: RawPullRequest["authorityChanges"];
  authorityAvailability: ObservationAvailability;
}> {
  const comparison = await githubRequest<CompareResponse>(
    `${API_URL}/repos/${REPOSITORY}/compare/${mainSha}...${headSha}`,
    { method: "GET" },
    options,
  );
  const relation =
    comparison.status === "diverged"
      ? "diverged"
      : comparison.status === "behind"
        ? "behind"
        : comparison.merge_base_commit.sha === mainSha
          ? "current"
          : "unknown";
  const mergeBaseSha = comparison.merge_base_commit.sha;
  if (mergeBaseSha === mainSha) {
    return {
      mergeBaseSha,
      relation,
      authorityChanges: [],
      authorityAvailability: "complete" as const,
    };
  }
  try {
    const mainAdvance = await githubRequest<CompareResponse>(
      `${API_URL}/repos/${REPOSITORY}/compare/${mergeBaseSha}...${mainSha}`,
      { method: "GET" },
      options,
    );
    const compareUrl = `https://github.com/${REPOSITORY}/compare/${mergeBaseSha}...${mainSha}`;
    return {
      mergeBaseSha,
      relation,
      authorityChanges: [
        ...new Set(
          (mainAdvance.files ?? []).flatMap((file) =>
            [file.filename, file.previous_filename].filter(
              (path): path is string => path !== undefined && isAuthorityPath(path),
            ),
          ),
        ),
      ].map((path) => ({ path, url: compareUrl })),
      authorityAvailability:
        mainAdvance.files === undefined || mainAdvance.files.length >= 300
          ? "incomplete" as const
          : "complete" as const,
    };
  } catch {
    return {
      mergeBaseSha,
      relation,
      authorityChanges: [],
      authorityAvailability: "unavailable" as const,
    };
  }
}

function unavailableObservation(reason: string): RepositoryObservation {
  return {
    repository: REPOSITORY,
    ownerToken: "agent:codex",
    observedAt: new Date().toISOString(),
    availability: "unavailable",
    main: null,
    roadmap: null,
    issues: [],
    issuesAvailability: "unavailable",
    pullRequests: [],
    pullsAvailability: "unavailable",
    implementationLinkageAvailability: "unavailable",
    recentActivity: [],
    recentActivityAvailability: "unavailable",
    errors: [{ source: "GitHub", url: GRAPHQL_URL, reason }],
  };
}

export async function observeRepository(
  options: RequestOptions = {},
): Promise<RepositoryObservation> {
  let response: GraphResponse;
  try {
    response = await githubRequest<GraphResponse>(
      GRAPHQL_URL,
      {
        method: "POST",
        body: JSON.stringify({ query: QUERY, variables: { owner: OWNER, name: NAME } }),
      },
      options,
    );
  } catch {
    return unavailableObservation("observation-unavailable");
  }
  const repository = response.data?.repository;
  const main = repository?.defaultBranchRef?.target;
  if (repository === null || repository === undefined || main === undefined) {
    return unavailableObservation("observation-incomplete");
  }

  const issueNodes = presentNodes(repository.issues.nodes);
  const pullNodes = presentNodes(repository.pullRequests.nodes);
  const recentNodes = presentNodes(repository.recent.nodes);
  const issueNodeMissing = hasMissingNode(repository.issues.nodes);
  const pullNodeMissing = hasMissingNode(repository.pullRequests.nodes);
  const recentNodeMissing = hasMissingNode(repository.recent.nodes);
  const nullNodeObserved = issueNodeMissing || pullNodeMissing || recentNodeMissing;

  const errors: RepositoryObservation["errors"] = [];
  if ((response.errors?.length ?? 0) > 0) {
    errors.push({ source: "GitHub GraphQL", url: GRAPHQL_URL, reason: "observation-incomplete" });
  }
  const roadmapMarkdown = roadmapText(repository.roadmap);
  if (roadmapMarkdown === null) {
    errors.push({ source: "Product Roadmap", url: repository.url, reason: "observation-incomplete" });
  }
  const topLevelTruncated =
    repository.issues.pageInfo.hasNextPage ||
    repository.pullRequests.pageInfo.hasNextPage;
  const issueTruncated = issueNodes.some(
    (issue) =>
      issue.labels === null ||
      issue.labels.pageInfo.hasNextPage ||
      hasMissingNode(issue.labels.nodes) ||
      issue.blockedBy.pageInfo.hasNextPage ||
      hasMissingNode(issue.blockedBy.nodes),
  );
  const pullTruncated = pullNodes.some(hasNestedTruncation);
  if (topLevelTruncated || issueTruncated || pullTruncated || nullNodeObserved) {
    errors.push({ source: "GitHub GraphQL", url: GRAPHQL_URL, reason: "observation-incomplete" });
  }

  const comparisons = await Promise.all(
    pullNodes.map(async (pull) => {
      try {
        return await comparePull(main.oid, pull.headRefOid, options);
      } catch {
        return {
          mergeBaseSha: null,
          relation: "unknown" as const,
          authorityChanges: [],
          authorityAvailability: "unavailable" as const,
        };
      }
    }),
  );
  comparisons.forEach((comparison, index) => {
    if (comparison.authorityAvailability !== "complete") {
      const pull = pullNodes[index];
      if (pull !== undefined) {
        errors.push({
          source: `PR #${String(pull.number)} authority comparison`,
          url: pull.url,
          reason:
            comparison.authorityAvailability === "unavailable"
              ? "observation-unavailable"
              : "observation-incomplete",
        });
      }
    }
  });
  const pullRequests = pullNodes.map((pull, index): RawPullRequest => {
    const comparison = comparisons[index] ?? {
      mergeBaseSha: null,
      relation: "unknown" as const,
      authorityChanges: [],
      authorityAvailability: "unavailable" as const,
    };
    const comments = presentNodes(pull.comments.nodes);
    const reviews = presentNodes(pull.reviews?.nodes ?? null);
    const threads = presentNodes(pull.reviewThreads.nodes);
    const reviewComments = reviews
      .filter((review) => review.body.length > 0)
      .map((review) => rawComment(review, "pull-request-review", true));
    const threadComments = threads.flatMap((thread) =>
      presentNodes(thread.comments.nodes).map((comment) =>
        rawComment(comment, "pull-request-review-comment", false),
      ),
    );
    const checkNodes = presentNodes(pull.statusCheckRollup?.contexts.nodes ?? []);
    return {
      number: pull.number,
      title: pull.title,
      url: pull.url,
      state: pull.state,
      draft: pull.isDraft,
      headSha: pull.headRefOid,
      baseSha: pull.baseRefOid,
      baseRef: pull.baseRefName,
      mergeBaseSha: comparison.mergeBaseSha,
      relationToMain: comparison.relation,
      mergeability:
        pull.mergeable === "MERGEABLE"
          ? "mergeable"
          : pull.mergeable === "CONFLICTING"
            ? "conflicting"
            : "unknown",
      nativeMergePolicy: nativeMergePolicy(pull),
      authorityChanges: comparison.authorityChanges,
      authorityAvailability: comparison.authorityAvailability,
      closingIssueNumbers: presentNodes(pull.closingIssuesReferences?.nodes ?? null).map(
        (issue) => issue.number,
      ),
      comments: [
        ...comments.map((comment) => rawComment(comment, "issue-comment", true)),
        ...reviewComments,
        ...threadComments,
      ],
      commentsAvailability: commentsTruncated(pull) ? "incomplete" : "complete",
      checks: checkNodes.map((check) => rawCheck(check, pull.headRefOid)),
      checksAvailability:
        pull.statusCheckRollup === null
          ? "complete"
          : pull.statusCheckRollup.contexts.pageInfo.hasNextPage ||
              hasMissingNode(pull.statusCheckRollup.contexts.nodes)
            ? "incomplete"
            : "complete",
      reviews: reviews.map((review) => ({
        id: stableSourceId(review),
        authorLogin: review.author?.login ?? `unknown:${review.id}`,
        authorAssociation: review.authorAssociation,
        submittedAt: review.submittedAt,
        commitSha: review.commit?.oid ?? "",
        state: review.state,
        url: review.url,
      })),
      reviewsAvailability:
        pull.reviews === null ||
        pull.reviews.pageInfo.hasNextPage ||
        hasMissingNode(pull.reviews.nodes)
          ? "incomplete"
          : "complete",
      threads: threads.map((thread) => ({
        id: thread.id,
        resolved: thread.isResolved,
        outdated: thread.isOutdated,
        url: presentNodes(thread.comments.nodes)[0]?.url ?? pull.url,
      })),
      threadsAvailability:
        pull.reviewThreads.pageInfo.hasNextPage ||
        hasMissingNode(pull.reviewThreads.nodes) ||
        threads.some(
          (thread) =>
            thread.comments.pageInfo.hasNextPage || hasMissingNode(thread.comments.nodes),
        )
          ? "incomplete"
          : "complete",
    };
  });

  const availability: ObservationAvailability =
    errors.length === 0 ? "complete" : "incomplete";
  const token = options.token?.trim();
  return {
    repository: REPOSITORY,
    ownerToken: "agent:codex",
    observedAt: new Date().toISOString(),
    availability,
    main: { sha: main.oid, url: main.url },
    roadmap: roadmapMarkdown === null
      ? null
      : {
          markdown: roadmapMarkdown,
          url: `https://github.com/${REPOSITORY}/blob/${main.oid}/${ROADMAP_PATH}`,
        },
    issues: issueNodes.map((issue) => ({
      number: issue.number,
      title: issue.title,
      url: issue.url,
      state: issue.state,
      labels: presentNodes(issue.labels?.nodes ?? null).map((label) => label.name),
      labelsAvailability:
        issue.labels === null ||
        issue.labels.pageInfo.hasNextPage ||
        hasMissingNode(issue.labels.nodes)
          ? "incomplete"
          : "complete",
      milestone: issue.milestone?.title ?? null,
      blockedBy: presentNodes(issue.blockedBy.nodes),
      dependencyAvailability:
        issue.blockedBy.pageInfo.hasNextPage || hasMissingNode(issue.blockedBy.nodes)
          ? "incomplete"
          : "complete",
    })),
    issuesAvailability:
      repository.issues.pageInfo.hasNextPage || issueTruncated || issueNodeMissing
        ? "incomplete"
        : "complete",
    pullRequests,
    pullsAvailability:
      repository.pullRequests.pageInfo.hasNextPage || pullTruncated || pullNodeMissing
        ? "incomplete"
        : "complete",
    implementationLinkageAvailability:
      (response.errors?.length ?? 0) > 0 ||
      repository.pullRequests.pageInfo.hasNextPage ||
      pullNodeMissing ||
      pullNodes.some(
        (pull) =>
          pull.closingIssuesReferences === null ||
          pull.closingIssuesReferences.pageInfo.hasNextPage ||
          hasMissingNode(pull.closingIssuesReferences.nodes) ||
          topLevelCommentsTruncated(pull),
      )
        ? "incomplete"
        : "complete",
    recentActivity: recentNodes.flatMap((pull) =>
      pull.mergedAt === null || pull.mergeCommit === null
        ? []
        : [{
            number: pull.number,
            title: pull.title,
            url: pull.url,
            mergedAt: pull.mergedAt,
            mergeSha: pull.mergeCommit.oid,
          }],
    ),
    // Recent activity is intentionally a bounded context window, not a complete history query.
    recentActivityAvailability: recentNodeMissing ? "incomplete" : "complete",
    errors,
    ...(token === undefined || token.length === 0 ? {} : { serverCredential: "present" }),
  };
}

export function readServerCredential(environment: NodeJS.ProcessEnv = process.env): string | undefined {
  for (const value of [environment.GITHUB_TOKEN, environment.GH_TOKEN]) {
    const trimmed = value?.trim();
    if (trimmed !== undefined && trimmed.length > 0) return trimmed;
  }
  return undefined;
}
