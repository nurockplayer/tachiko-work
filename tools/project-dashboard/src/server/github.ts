import type {
  GitHubAuthorAssociation,
  GitHubCommentKind,
} from "@tachiko-work/operational-evidence";

import type {
  CheckResult,
  FieldObservation,
  MergeableState,
  MergeStateStatus,
  NativeMergePolicy,
  ObservationAvailability,
  RawCheck,
  RawComment,
  RawPullRequest,
  RepositoryObservation,
  ReviewDecision,
} from "../shared/model.js";

const REPOSITORY = "nurockplayer/tachiko-work";
const OWNER = "nurockplayer";
const NAME = "tachiko-work";
const API_URL = "https://api.github.com";
const GRAPHQL_URL = `${API_URL}/graphql`;
const ROADMAP_PATH = "docs/product/product-roadmap.md";
const REQUEST_TIMEOUT_MS = 15_000;
const CARGO_AUTHORITY_PATH = /(?:^|\/)Cargo\.(?:toml|lock)$/;

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
  lastEditedAt: string | null;
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
  status: unknown;
  conclusion: unknown;
  url: string;
  detailsUrl: string | null;
}

interface GraphStatusContext {
  __typename: "StatusContext";
  id: string;
  context: string;
  state: unknown;
  targetUrl: string | null;
  commit?: { oid: unknown } | null;
}

type GraphCheck = GraphCheckRun | GraphStatusContext;

interface GraphPull {
  number: number;
  title: string;
  url: string;
  state: "OPEN" | "CLOSED" | "MERGED";
  isDraft: unknown;
  headRefOid: string;
  baseRefOid: string;
  baseRefName: string;
  mergeable: unknown;
  mergeStateStatus: unknown;
  reviewDecision: unknown;
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
  milestone?: { title: string } | null;
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

interface GraphError {
  message: string;
  path?: readonly (string | number)[];
}

interface GraphResponse {
  data?: { repository: GraphRepository | null };
  errors?: GraphError[];
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

const CHECK_STATUSES = [
  "REQUESTED",
  "QUEUED",
  "IN_PROGRESS",
  "COMPLETED",
  "WAITING",
  "PENDING",
] as const;
const CHECK_CONCLUSIONS = [
  "ACTION_REQUIRED",
  "TIMED_OUT",
  "CANCELLED",
  "FAILURE",
  "SUCCESS",
  "NEUTRAL",
  "SKIPPED",
  "STARTUP_FAILURE",
  "STALE",
] as const;
const STATUS_STATES = ["EXPECTED", "ERROR", "FAILURE", "PENDING", "SUCCESS"] as const;
const REVIEW_DECISIONS = ["CHANGES_REQUESTED", "APPROVED", "REVIEW_REQUIRED"] as const;
const MERGEABLE_STATES = ["MERGEABLE", "CONFLICTING", "UNKNOWN"] as const;
const MERGE_STATE_STATUSES = [
  "DIRTY",
  "UNKNOWN",
  "BLOCKED",
  "BEHIND",
  "UNSTABLE",
  "HAS_HOOKS",
  "CLEAN",
] as const;

function isString(value: unknown): value is string {
  return typeof value === "string";
}

function isBoolean(value: unknown): value is boolean {
  return typeof value === "boolean";
}

function enumGuard<const Values extends readonly string[]>(values: Values) {
  return (value: unknown): value is Values[number] =>
    typeof value === "string" && values.includes(value);
}

const isCheckStatus = enumGuard(CHECK_STATUSES);
const isCheckConclusion = enumGuard(CHECK_CONCLUSIONS);
const isStatusState = enumGuard(STATUS_STATES);
const isReviewDecision = enumGuard(REVIEW_DECISIONS);
const isMergeableState = enumGuard(MERGEABLE_STATES);
const isMergeStateStatus = enumGuard(MERGE_STATE_STATUSES);

function fieldError(
  errors: GraphResponse["errors"],
  fieldPath: readonly (string | number)[],
): GraphError | undefined {
  return errors?.find((error) => {
    if (error.path === undefined) return true;
    const sharedLength = Math.min(error.path.length, fieldPath.length);
    return Array.from(
      { length: sharedLength },
      (_, index) => error.path?.[index] === fieldPath[index],
    ).every(Boolean);
  });
}

function sourceError(
  errors: GraphResponse["errors"],
  sourcePaths: readonly (readonly (string | number)[])[],
): boolean {
  return sourcePaths.some((path) => fieldError(errors, path) !== undefined);
}

function observeField<T>(
  value: unknown,
  isValue: (candidate: unknown) => candidate is T,
  errors: GraphResponse["errors"],
  fieldPath: readonly (string | number)[],
): FieldObservation<T> {
  const error = fieldError(errors, fieldPath);
  if (error !== undefined || value === undefined || (value !== null && !isValue(value))) {
    return {
      state: "unknown",
      availability: "incomplete",
      path: error?.path ?? fieldPath,
    };
  }
  return value === null ? { state: "null" } : { state: "value", value };
}

function unknownOrNull<T>(
  observation: Exclude<FieldObservation<unknown>, { state: "value" }>,
): FieldObservation<T> {
  return observation.state === "null"
    ? { state: "null" }
    : {
        state: "unknown",
        availability: observation.availability,
        path: observation.path,
      };
}

type PolicyCandidate = NativeMergePolicy["state"];

function requiredPolicy<T>(
  observation: FieldObservation<T>,
  decode: (value: T) => PolicyCandidate,
): PolicyCandidate {
  return observation.state === "value" ? decode(observation.value) : "unknown";
}

function decodeNativeMergePolicy(
  draft: FieldObservation<boolean>,
  mergeable: FieldObservation<MergeableState>,
  mergeState: FieldObservation<MergeStateStatus>,
  reviewDecision: FieldObservation<ReviewDecision>,
): NativeMergePolicy {
  const draftPolicy = requiredPolicy(draft, (value) => value ? "blocked" : "satisfied");
  const mergeabilityPolicy = requiredPolicy(mergeable, (value) =>
    value === "CONFLICTING" ? "blocked" : value === "UNKNOWN" ? "unknown" : "satisfied",
  );
  const mergeStatePolicy = requiredPolicy(mergeState, (value) => {
    switch (value) {
      case "DIRTY":
      case "BLOCKED":
      case "BEHIND":
        return "blocked";
      case "UNKNOWN":
        return "unknown";
      case "UNSTABLE":
      case "HAS_HOOKS":
      case "CLEAN":
        return "satisfied";
    }
  });
  const reviewPolicy = reviewDecision.state === "null"
    ? "satisfied"
    : requiredPolicy(reviewDecision, (value) =>
        value === "CHANGES_REQUESTED"
          ? "blocked"
          : value === "REVIEW_REQUIRED"
            ? "waiting"
            : "satisfied",
      );
  const candidates = [draftPolicy, mergeabilityPolicy, mergeStatePolicy, reviewPolicy];
  if (mergeabilityPolicy === "blocked") return { state: "blocked", reason: "conflict" };
  if (candidates.includes("blocked")) return { state: "blocked", reason: "policy" };
  if (candidates.includes("unknown")) return { state: "unknown" };
  if (candidates.includes("waiting")) return { state: "waiting" };
  return candidates.every((candidate) => candidate === "satisfied")
    ? { state: "satisfied" }
    : { state: "unknown" };
}

function rawComment(
  comment: GraphComment,
  kind: GitHubCommentKind,
  topLevel: boolean,
  errors: GraphResponse["errors"],
  commentPath: readonly (string | number)[],
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
    lastEditedAt: observeField(
      comment.lastEditedAt,
      isString,
      errors,
      [...commentPath, "lastEditedAt"],
    ),
    topLevel,
    trustedProducer: trustedProducer(comment),
  };
}

function checkRunResult(
  status: FieldObservation<(typeof CHECK_STATUSES)[number]>,
  conclusion: FieldObservation<(typeof CHECK_CONCLUSIONS)[number]>,
): FieldObservation<CheckResult> {
  if (status.state !== "value") return unknownOrNull(status);
  if (status.value !== "COMPLETED") return { state: "value", value: "pending" };
  if (conclusion.state !== "value") return unknownOrNull(conclusion);
  return {
    state: "value",
    value: conclusion.value === "SUCCESS" ? "success" : "failure",
  };
}

function statusContextResult(
  state: FieldObservation<(typeof STATUS_STATES)[number]>,
): FieldObservation<CheckResult> {
  if (state.state !== "value") return unknownOrNull(state);
  if (state.value === "SUCCESS") return { state: "value", value: "success" };
  if (state.value === "PENDING" || state.value === "EXPECTED") {
    return { state: "value", value: "pending" };
  }
  return { state: "value", value: "failure" };
}

function rawCheck(
  check: GraphCheck,
  headSha: string,
  errors: GraphResponse["errors"],
  checkPath: readonly (string | number)[],
): RawCheck {
  if (check.__typename === "StatusContext") {
    const state = observeField(check.state, isStatusState, errors, [...checkPath, "state"]);
    const commitOid = check.commit === null ? null : check.commit?.oid;
    return {
      name: check.context,
      headSha: observeField(commitOid, isString, errors, [...checkPath, "commit"]),
      result: statusContextResult(state),
      url: check.targetUrl ?? `https://github.com/${REPOSITORY}/commit/${headSha}/checks`,
    };
  }
  const status = observeField(check.status, isCheckStatus, errors, [...checkPath, "status"]);
  const conclusion = observeField(
    check.conclusion,
    isCheckConclusion,
    errors,
    [...checkPath, "conclusion"],
  );
  return {
    name: check.name,
    headSha: { state: "value", value: headSha },
    result: checkRunResult(status, conclusion),
    url: check.detailsUrl ?? check.url,
  };
}

function presentNodes<T>(nodes: readonly (T | null)[] | null): T[] {
  return nodes?.filter((node): node is T => node !== null) ?? [];
}

function hasMissingNode(nodes: readonly unknown[] | null): boolean {
  return nodes === null || nodes.some((node) => node === null);
}

function graphNodeIndex<T>(nodes: readonly (T | null)[] | null, node: T): number {
  return nodes?.indexOf(node) ?? -1;
}

function roadmapText(value: unknown): string | null {
  if (typeof value !== "object" || value === null || !("text" in value)) return null;
  const text = value.text;
  return typeof text === "string" && text.trim().length > 0 ? text : null;
}

function hasNestedTruncation(
  pull: GraphPull,
  errors: GraphResponse["errors"],
  pullPath: readonly (string | number)[],
): boolean {
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
      : hasMissingNode(pull.statusCheckRollup.contexts.nodes)) ||
    fieldError(errors, [...pullPath, "closingIssuesReferences"]) !== undefined ||
    fieldError(errors, [...pullPath, "comments"]) !== undefined ||
    fieldError(errors, [...pullPath, "reviews"]) !== undefined ||
    fieldError(errors, [...pullPath, "reviewThreads"]) !== undefined ||
    fieldError(errors, [...pullPath, "statusCheckRollup"]) !== undefined
  );
}

function commentsTruncated(
  pull: GraphPull,
  errors: GraphResponse["errors"],
  pullPath: readonly (string | number)[],
): boolean {
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
    ) ||
    fieldError(errors, [...pullPath, "comments"]) !== undefined ||
    fieldError(errors, [...pullPath, "reviews"]) !== undefined ||
    fieldError(errors, [...pullPath, "reviewThreads"]) !== undefined
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
    CARGO_AUTHORITY_PATH.test(path) ||
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
  for (const error of response.errors ?? []) {
    errors.push({
      source: "GitHub GraphQL",
      url: GRAPHQL_URL,
      reason: "observation-incomplete",
      ...(error.path === undefined ? {} : { path: error.path }),
    });
  }
  const roadmapMarkdown = roadmapText(repository.roadmap);
  if (roadmapMarkdown === null) {
    errors.push({ source: "Product Roadmap", url: repository.url, reason: "observation-incomplete" });
  }
  const topLevelTruncated =
    repository.issues.pageInfo.hasNextPage ||
    repository.pullRequests.pageInfo.hasNextPage;
  const issueTruncated = issueNodes.some((issue) => {
    const issuePath = [
      "repository",
      "issues",
      "nodes",
      graphNodeIndex(repository.issues.nodes, issue),
    ] as const;
    return (
      issue.labels === null ||
      issue.labels.pageInfo.hasNextPage ||
      hasMissingNode(issue.labels.nodes) ||
      issue.blockedBy.pageInfo.hasNextPage ||
      hasMissingNode(issue.blockedBy.nodes) ||
      fieldError(response.errors, [...issuePath, "labels"]) !== undefined ||
      fieldError(response.errors, [...issuePath, "blockedBy"]) !== undefined
    );
  });
  const pullTruncated = pullNodes.some((pull) => {
    const pullPath = [
      "repository",
      "pullRequests",
      "nodes",
      graphNodeIndex(repository.pullRequests.nodes, pull),
    ] as const;
    return hasNestedTruncation(pull, response.errors, pullPath);
  });
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
    const pullPath = [
      "repository",
      "pullRequests",
      "nodes",
      graphNodeIndex(repository.pullRequests.nodes, pull),
    ] as const;
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
      .map((review) =>
        rawComment(review, "pull-request-review", true, response.errors, [
          ...pullPath,
          "reviews",
          "nodes",
          graphNodeIndex(pull.reviews?.nodes ?? null, review),
        ]),
      );
    const threadComments = threads.flatMap((thread) => {
      const threadPath = [
        ...pullPath,
        "reviewThreads",
        "nodes",
        graphNodeIndex(pull.reviewThreads.nodes, thread),
      ] as const;
      return presentNodes(thread.comments.nodes).map((comment) =>
        rawComment(comment, "pull-request-review-comment", false, response.errors, [
          ...threadPath,
          "comments",
          "nodes",
          graphNodeIndex(thread.comments.nodes, comment),
        ]),
      );
    });
    const checkNodes = presentNodes(pull.statusCheckRollup?.contexts.nodes ?? []);
    const draft = observeField(pull.isDraft, isBoolean, response.errors, [...pullPath, "isDraft"]);
    const mergeable = observeField(
      pull.mergeable,
      isMergeableState,
      response.errors,
      [...pullPath, "mergeable"],
    );
    const mergeState = observeField(
      pull.mergeStateStatus,
      isMergeStateStatus,
      response.errors,
      [...pullPath, "mergeStateStatus"],
    );
    const reviewDecision = observeField(
      pull.reviewDecision,
      isReviewDecision,
      response.errors,
      [...pullPath, "reviewDecision"],
    );
    return {
      number: pull.number,
      title: pull.title,
      url: pull.url,
      state: pull.state,
      draft: draft.state === "value" ? draft.value : false,
      headSha: pull.headRefOid,
      baseSha: pull.baseRefOid,
      baseRef: pull.baseRefName,
      mergeBaseSha: comparison.mergeBaseSha,
      relationToMain: comparison.relation,
      nativeMergePolicy: decodeNativeMergePolicy(
        draft,
        mergeable,
        mergeState,
        reviewDecision,
      ),
      authorityChanges: comparison.authorityChanges,
      authorityAvailability: comparison.authorityAvailability,
      closingIssueNumbers: presentNodes(pull.closingIssuesReferences?.nodes ?? null).map(
        (issue) => issue.number,
      ),
      comments: [
        ...comments.map((comment) =>
          rawComment(comment, "issue-comment", true, response.errors, [
            ...pullPath,
            "comments",
            "nodes",
            graphNodeIndex(pull.comments.nodes, comment),
          ]),
        ),
        ...reviewComments,
        ...threadComments,
      ],
      commentsAvailability: commentsTruncated(pull, response.errors, pullPath)
        ? "incomplete"
        : "complete",
      checks: checkNodes.map((check) =>
        rawCheck(check, pull.headRefOid, response.errors, [
          ...pullPath,
          "statusCheckRollup",
          "contexts",
          "nodes",
          graphNodeIndex(pull.statusCheckRollup?.contexts.nodes ?? null, check),
        ]),
      ),
      checksAvailability:
        fieldError(response.errors, [...pullPath, "statusCheckRollup"]) !== undefined ||
        (pull.statusCheckRollup !== null &&
          (pull.statusCheckRollup.contexts.pageInfo.hasNextPage ||
              hasMissingNode(pull.statusCheckRollup.contexts.nodes)
          ))
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
        hasMissingNode(pull.reviews.nodes) ||
        fieldError(response.errors, [...pullPath, "reviews"]) !== undefined
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
        ) ||
        fieldError(response.errors, [...pullPath, "reviewThreads"]) !== undefined
          ? "incomplete"
          : "complete",
    };
  });

  const availability: ObservationAvailability =
    errors.length === 0 ? "complete" : "incomplete";
  const implementationLinkagePaths = [
    ["repository", "pullRequests", "pageInfo"],
    ...pullNodes.flatMap((pull) => {
      const pullPath = [
        "repository",
        "pullRequests",
        "nodes",
        graphNodeIndex(repository.pullRequests.nodes, pull),
      ] as const;
      return [
        [...pullPath, "closingIssuesReferences"],
        [...pullPath, "comments"],
        [...pullPath, "reviews"],
      ];
    }),
  ] as const;
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
    issues: issueNodes.map((issue) => {
      const issuePath = [
        "repository",
        "issues",
        "nodes",
        graphNodeIndex(repository.issues.nodes, issue),
      ] as const;
      return {
        number: issue.number,
        title: issue.title,
        url: issue.url,
        state: issue.state,
        labels: presentNodes(issue.labels?.nodes ?? null).map((label) => label.name),
        labelsAvailability:
          issue.labels === null ||
          issue.labels.pageInfo.hasNextPage ||
          hasMissingNode(issue.labels.nodes) ||
          fieldError(response.errors, [...issuePath, "labels"]) !== undefined
            ? "incomplete"
            : "complete",
        milestone: observeField(
          issue.milestone === null ? null : issue.milestone?.title,
          isString,
          response.errors,
          [...issuePath, "milestone"],
        ),
        blockedBy: presentNodes(issue.blockedBy.nodes),
        dependencyAvailability:
          issue.blockedBy.pageInfo.hasNextPage ||
          hasMissingNode(issue.blockedBy.nodes) ||
          fieldError(response.errors, [...issuePath, "blockedBy"]) !== undefined
            ? "incomplete"
            : "complete",
      };
    }),
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
      sourceError(response.errors, implementationLinkagePaths) ||
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
    recentActivityAvailability:
      recentNodeMissing || sourceError(response.errors, [["repository", "recent"]])
        ? "incomplete"
        : "complete",
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
