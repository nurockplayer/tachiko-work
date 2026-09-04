import {
  parseAgentHandoff,
  parseStewardWatch,
  type EvidenceParseContext,
  type GitHubAuthorAssociation,
  type ParseResult,
  type StructuredCommentSource,
} from "@tachiko-work/operational-evidence";

import type {
  Availability,
  AttentionItem,
  DashboardProjection,
  DeliveryLane,
  IssueFact,
  PullRequestFact,
  SourceLink,
  StructuredFact,
} from "../shared/model.js";

const REPOSITORY = "nurockplayer/tachiko-work";
const OWNER = "nurockplayer";
const OWNER_TOKEN = "agent:codex";
const GRAPHQL_URL = "https://api.github.com/graphql";
const ROADMAP_PATH = "docs/product/product-roadmap.md";
const REQUEST_TIMEOUT_MS = 15_000;

export const DASHBOARD_QUERY = `
  query DashboardObservation($owner: String!, $name: String!) {
    repository(owner: $owner, name: $name) {
      url
      defaultBranchRef { target { ... on Commit { oid url } } }
      roadmap: object(expression: "main:${ROADMAP_PATH}") { ... on Blob { text } }
      issues(first: 100, states: OPEN, orderBy: {field: UPDATED_AT, direction: DESC}) {
        pageInfo { hasNextPage }
        nodes {
          number title url state
          labels(first: 30) { pageInfo { hasNextPage } nodes { name } }
          milestone { title }
          blockedBy(first: 100) { pageInfo { hasNextPage } nodes { number state url } }
        }
      }
      pullRequests(first: 40, states: OPEN, orderBy: {field: UPDATED_AT, direction: DESC}) {
        pageInfo { hasNextPage }
        nodes {
          number title url state isDraft headRefOid baseRefOid baseRefName
          mergeable mergeStateStatus reviewDecision
          closingIssuesReferences(first: 20) { pageInfo { hasNextPage } nodes { number } }
          comments(first: 100) {
            pageInfo { hasNextPage }
            nodes {
              id body url createdAt updatedAt lastEditedAt
              author { login }
              authorAssociation
            }
          }
          reviews(first: 100) {
            pageInfo { hasNextPage }
            nodes { id url state author { login } commit { oid } }
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

interface Page<T> {
  pageInfo: { hasNextPage: boolean };
  nodes: (T | null)[] | null;
}

interface GraphIssue {
  number: number;
  title: string;
  url: string;
  state: string;
  labels: Page<{ name: string }> | null;
  milestone: { title: string } | null;
  blockedBy: Page<{ number: number; state: string; url: string }> | null;
}

interface GraphComment {
  id: string;
  body: string;
  url: string;
  createdAt: string;
  updatedAt: string;
  lastEditedAt: string | null;
  author: { login: string } | null;
  authorAssociation: GitHubAuthorAssociation;
}

interface GraphReview {
  id: string;
  url: string;
  state: string;
  author: { login: string } | null;
  commit: { oid: string } | null;
}

interface GraphCheckRun {
  __typename: "CheckRun";
  id: string;
  name: string;
  status: string | null;
  conclusion: string | null;
  url: string | null;
  detailsUrl: string | null;
}

interface GraphStatusContext {
  __typename: "StatusContext";
  id: string;
  context: string;
  state: string | null;
  targetUrl: string | null;
  commit: { oid: string } | null;
}

interface GraphPullRequest {
  number: number;
  title: string;
  url: string;
  state: string;
  isDraft: boolean | null;
  headRefOid: string;
  baseRefOid: string;
  baseRefName: string;
  mergeable: string | null;
  mergeStateStatus: string | null;
  reviewDecision: string | null;
  closingIssuesReferences: Page<{ number: number }> | null;
  comments: Page<GraphComment> | null;
  reviews: Page<GraphReview> | null;
  statusCheckRollup: { contexts: Page<GraphCheckRun | GraphStatusContext> } | null;
}

interface GraphRecentPullRequest {
  number: number;
  title: string;
  url: string;
  mergedAt: string | null;
  mergeCommit: { oid: string } | null;
}

export interface DashboardGraphResponse {
  data?: {
    repository: {
      url: string;
      defaultBranchRef: { target: { oid: string; url: string } | null } | null;
      roadmap: { text: string | null } | null;
      issues: Page<GraphIssue>;
      pullRequests: Page<GraphPullRequest>;
      recent: Page<GraphRecentPullRequest>;
    } | null;
  };
  errors?: { message?: string; path?: readonly (string | number)[] }[];
}

interface ObserveOptions {
  token?: string;
  fetchImpl?: typeof fetch;
  observedAt?: string;
}

function present<T>(page: Page<T> | null): T[] {
  return (page?.nodes ?? []).filter((node): node is T => node !== null);
}

function completeNodes<T>(page: Page<T> | null): T[] {
  return page === null || pagePartial(page) ? [] : present(page);
}

function pagePartial<T>(page: Page<T> | null): boolean {
  return page === null || page.nodes === null || page.pageInfo.hasNextPage || page.nodes.some((node) => node === null);
}

function sectionAvailability(partial: boolean): Availability {
  return partial ? "partial" : "complete";
}

type GraphError = NonNullable<DashboardGraphResponse["errors"]>[number];
type GraphPath = readonly (string | number)[];

function pathStartsWith(path: GraphPath, prefix: GraphPath): boolean {
  return prefix.every((segment, index) => path[index] === segment);
}

function pathAffected(errors: readonly GraphError[], target: GraphPath): boolean {
  return errors.some((error) =>
    error.path === undefined ||
    pathStartsWith(error.path, target) ||
    pathStartsWith(target, error.path));
}

function connectionAffected(errors: readonly GraphError[], target: GraphPath): boolean {
  return errors.some((error) => {
    if (error.path === undefined || pathStartsWith(target, error.path)) return true;
    if (!pathStartsWith(error.path, target)) return false;
    const child = error.path[target.length];
    return child === undefined || child === "pageInfo" ||
      (child === "nodes" && error.path.length === target.length + 1);
  });
}

function source(label: string, url: string, kind: SourceLink["kind"] = "github"): SourceLink {
  return { label, url, kind };
}

export function parseProductHorizon(markdown: string): string | null {
  const lines = markdown.replaceAll("\r\n", "\n").split("\n");
  const headings = lines.flatMap((line, index) => line === "## Current horizon" ? [index] : []);
  if (headings.length !== 1) return null;
  const start = (headings[0] ?? 0) + 1;
  const relativeEnd = lines.slice(start).findIndex((line) => /^##(?:\s|$)/.test(line));
  const end = relativeEnd < 0 ? lines.length : start + relativeEnd;
  const values = lines.slice(start, end).flatMap((line) => {
    const match = /^> \*\*([^*\n]+)\*\*$/.exec(line);
    return match?.[1] === undefined ? [] : [match[1].trim()];
  });
  return values.length === 1 && values[0] !== "" ? values[0] ?? null : null;
}

function candidateComments(comments: GraphComment[], marker: string): GraphComment[] {
  return comments.filter((comment) =>
    comment.body.replaceAll("\r\n", "\n").split("\n").includes(marker));
}

function trustedProducer(comment: GraphComment): boolean {
  return comment.author?.login === OWNER && comment.authorAssociation === "OWNER";
}

function hasTrustedEvidenceMarker(comments: GraphComment[]): boolean {
  return ["<!-- agent-handoff:v1 -->", "<!-- project-steward-watch:v1 -->"]
    .some((marker) => candidateComments(comments, marker).some(trustedProducer));
}

function structuredSource(comment: GraphComment): StructuredCommentSource {
  return {
    body: comment.body,
    metadata: {
      repository: REPOSITORY,
      id: comment.id,
      kind: "issue-comment",
      authorLogin: comment.author?.login ?? "unknown",
      authorAssociation: comment.authorAssociation,
      url: comment.url,
      createdAt: comment.createdAt,
      updatedAt: comment.updatedAt === comment.createdAt ? null : comment.updatedAt,
      edited: comment.lastEditedAt !== null,
      topLevel: true,
      trustedProducer: trustedProducer(comment),
    },
  };
}

function structuredFact<T>(
  comments: GraphComment[],
  commentsPartial: boolean,
  marker: string,
  context: EvidenceParseContext | null,
  unavailableReason: string,
  parser: (input: StructuredCommentSource, context: EvidenceParseContext) => ParseResult<T>,
  value: (input: T) => string,
  label: string,
): StructuredFact {
  if (context === null) {
    return { status: "unknown", value: null, reason: unavailableReason, source: null };
  }
  const candidates = candidateComments(comments, marker);
  const trusted = candidates.filter(trustedProducer);
  if (commentsPartial) {
    const candidate = trusted.length === 1 ? trusted[0] : trusted.length === 0 ? candidates[0] : undefined;
    return {
      status: "unknown",
      value: null,
      reason: `${label} comment observation incomplete`,
      source: candidate === undefined ? null : source(label, candidate.url, "structured"),
    };
  }
  if (trusted.length > 1) {
    return { status: "unknown", value: null, reason: `Multiple trusted ${label} comments observed`, source: null };
  }
  const candidate = trusted[0] ?? candidates[0];
  if (candidate === undefined) {
    return { status: "missing", value: null, reason: `${label} not observed`, source: null };
  }
  const result = parser(structuredSource(candidate), context);
  return result.ok
    ? {
        status: "current",
        value: value(result.value),
        reason: "Exact head and live main match",
        source: source(label, result.source.url, "structured"),
      }
    : {
        status: "unknown",
        value: null,
        reason: `${label} ${result.reason}`,
        source: source(label, result.source.url, "structured"),
      };
}

interface IssuePartial {
  core: boolean;
  labels: boolean;
  milestone: boolean;
  dependencies: boolean;
}

function issueFact(issue: GraphIssue, observation: IssuePartial): IssueFact {
  const identityAvailability = sectionAvailability(observation.core);
  const labelsAvailability = sectionAvailability(observation.labels || pagePartial(issue.labels));
  const dependenciesAvailability = sectionAvailability(
    observation.dependencies || pagePartial(issue.blockedBy),
  );
  const partial =
    observation.core ||
    labelsAvailability !== "complete" ||
    observation.milestone ||
    dependenciesAvailability !== "complete";
  return {
    number: issue.number,
    title: observation.core ? null : issue.title,
    url: issue.url,
    state: observation.core ? null : issue.state,
    labels: labelsAvailability === "complete" ? completeNodes(issue.labels).map((label) => label.name) : [],
    labelsAvailability,
    milestone: observation.milestone ? null : issue.milestone?.title ?? null,
    milestoneAvailability: sectionAvailability(observation.milestone),
    blockedBy: dependenciesAvailability === "complete" ? completeNodes(issue.blockedBy) : [],
    dependenciesAvailability,
    identityAvailability,
    availability: sectionAvailability(partial),
  };
}

function pullRequestFact(
  pull: GraphPullRequest,
  mainSha: string | null,
  observation: {
    core: boolean;
    head: boolean;
    base: boolean;
    native: boolean;
    linkage: boolean;
    comments: boolean;
    reviews: boolean;
    checks: boolean;
  },
): PullRequestFact {
  const linkagePartial = observation.linkage || pagePartial(pull.closingIssuesReferences);
  const linkedIssueNumbers = linkagePartial
    ? []
    : completeNodes(pull.closingIssuesReferences).map((issue) => issue.number);
  const comments = present(pull.comments);
  const commentsPartial = observation.comments || pagePartial(pull.comments);
  const headSha = observation.head ? null : pull.headRefOid;
  const baseSha = observation.base ? null : pull.baseRefOid;
  const baseRef = observation.base ? null : pull.baseRefName;
  const context = !linkagePartial && linkedIssueNumbers.length === 1 && headSha !== null && mainSha !== null
    ? {
        repository: REPOSITORY,
        issueNumber: linkedIssueNumbers[0] ?? 0,
        pullRequestNumber: pull.number,
        owner: OWNER_TOKEN,
        headSha,
        mainSha,
      }
    : null;
  const contextUnavailableReason = linkagePartial
    ? "Issue linkage Unknown"
    : headSha === null
      ? "PR head identity Unknown"
      : mainSha === null
        ? "Live main identity Unknown"
        : "Structured evidence context Unknown";
  const checkPage = pull.statusCheckRollup?.contexts ?? null;
  const checksPartial = observation.checks || observation.head || pagePartial(checkPage);
  const reviewsPartial = observation.reviews || observation.head || pagePartial(pull.reviews);
  const identityAvailability = sectionAvailability(observation.core);
  const headAvailability = sectionAvailability(observation.head);
  const baseAvailability = sectionAvailability(observation.base);
  const nativeAvailability = sectionAvailability(observation.native || observation.head);
  const partial = observation.core ||
    observation.head ||
    observation.base ||
    observation.native ||
    linkagePartial ||
    commentsPartial ||
    reviewsPartial ||
    checksPartial;
  return {
    number: pull.number,
    title: observation.core ? null : pull.title,
    url: pull.url,
    state: observation.core ? null : pull.state,
    draft: observation.core ? null : pull.isDraft,
    headSha,
    baseSha,
    baseRef,
    mergeable: observation.native || observation.head ? null : pull.mergeable,
    mergeStateStatus: observation.native || observation.head ? null : pull.mergeStateStatus,
    reviewDecision: observation.native || observation.head ? null : pull.reviewDecision,
    linkedIssueNumbers,
    linkageAvailability: sectionAvailability(linkagePartial),
    identityAvailability,
    headAvailability,
    baseAvailability,
    nativeAvailability,
    checks: {
      availability: sectionAvailability(checksPartial),
      items: checksPartial ? [] : present(checkPage).map((check) => check.__typename === "CheckRun"
        ? {
            name: check.name,
            status: check.status,
            conclusion: check.conclusion,
            url: check.detailsUrl ?? check.url ?? pull.url,
            headSha: null,
          }
        : {
            name: check.context,
            status: check.state,
            conclusion: null,
            url: check.targetUrl ?? pull.url,
            headSha: check.commit?.oid ?? null,
          }),
    },
    reviews: {
      availability: sectionAvailability(reviewsPartial),
      items: reviewsPartial ? [] : present(pull.reviews).map((review) => ({
        author: review.author?.login ?? "Unknown",
        state: review.state,
        commitSha: review.commit?.oid ?? null,
        exactHead: review.commit === null ? null : review.commit.oid === pull.headRefOid,
        url: review.url,
      })),
    },
    handoff: structuredFact(
      comments,
      commentsPartial,
      "<!-- agent-handoff:v1 -->",
      context,
      contextUnavailableReason,
      parseAgentHandoff,
      (handoff) => `${handoff.state} · ${handoff.owner}`,
      "Agent handoff",
    ),
    stewardWatch: structuredFact(
      comments,
      commentsPartial,
      "<!-- project-steward-watch:v1 -->",
      context,
      contextUnavailableReason,
      parseStewardWatch,
      (watch) => `${watch.verdict} · human action ${watch.humanAction}`,
      "Steward watch",
    ),
    availability: sectionAvailability(partial),
  };
}

function unavailableProjection(observedAt: string): DashboardProjection {
  const repositorySource = source("GitHub repository", `https://github.com/${REPOSITORY}`);
  return {
    repository: REPOSITORY,
    observedAt,
    fetchHealth: "unavailable",
    executive: {
      mainSha: { value: null, availability: "unavailable", source: repositorySource },
      productHorizon: {
        value: null,
        availability: "unavailable",
        source: source(
          "Product Roadmap",
          `https://github.com/${REPOSITORY}/blob/main/${ROADMAP_PATH}`,
          "repository",
        ),
      },
      activeCount: { value: null, availability: "unavailable", source: repositorySource },
      readyCount: { value: null, availability: "unavailable", source: repositorySource },
      humanAction: { value: null, availability: "unavailable", source: repositorySource },
    },
    deliveries: [],
    deliveriesAvailability: "unavailable",
    criticalPath: { availability: "unavailable", nodes: [], edges: [], source: repositorySource },
    recentActivity: { availability: "unavailable", items: [], source: repositorySource },
    attention: [{
      level: "unknown",
      label: "GitHub observation unavailable",
      detail: "Live repository facts are Unknown. No healthy state or final merge verdict is inferred.",
      sources: [repositorySource],
    }],
    sources: [repositorySource],
  };
}

function evidenceRelevant(
  pull: PullRequestFact,
  issuesByNumber: ReadonlyMap<number, IssueFact>,
  trustedEvidencePullNumbers: ReadonlySet<number>,
): boolean {
  return pull.linkedIssueNumbers.some((number) =>
    issuesByNumber.get(number)?.labels.includes(OWNER_TOKEN) === true) ||
    trustedEvidencePullNumbers.has(pull.number);
}

export function projectGraphResponse(
  response: DashboardGraphResponse,
  observedAt = new Date().toISOString(),
): DashboardProjection {
  const repository = response.data?.repository;
  const main = repository?.defaultBranchRef?.target;
  if (repository === null || repository === undefined) {
    return unavailableProjection(observedAt);
  }

  const errors = response.errors ?? [];
  const hasResponseErrors = errors.length > 0;
  const repositorySource = source("GitHub repository", repository.url);
  const mainUnavailable = main === null || main === undefined || pathAffected(errors, ["repository", "defaultBranchRef"]);
  const mainSha = mainUnavailable ? null : main.oid;
  const issuePath = ["repository", "issues"] as const;
  const pullPath = ["repository", "pullRequests"] as const;
  const issuePagePartial = pagePartial(repository.issues) || connectionAffected(errors, issuePath);
  const pullPagePartial = pagePartial(repository.pullRequests) || connectionAffected(errors, pullPath);
  const issueNodes = issuePagePartial ? [] : repository.issues.nodes ?? [];
  const pullNodes = repository.pullRequests.nodes ?? [];
  const issues = issueNodes.flatMap((issue, index) => issue === null
    ? []
    : [issueFact(issue, {
        core: ["number", "title", "url", "state"].some((field) =>
          pathAffected(errors, [...issuePath, "nodes", index, field])),
        labels: pathAffected(errors, [...issuePath, "nodes", index, "labels"]),
        milestone: pathAffected(errors, [...issuePath, "nodes", index, "milestone"]),
        dependencies: pathAffected(errors, [...issuePath, "nodes", index, "blockedBy"]),
      })]);
  const pullRequests = pullNodes.flatMap((pull, index) => pull === null
    ? []
    : [pullRequestFact(pull, mainSha, {
        core: ["number", "title", "url", "state", "isDraft"].some((field) =>
          pathAffected(errors, [...pullPath, "nodes", index, field])),
        head: pathAffected(errors, [...pullPath, "nodes", index, "headRefOid"]),
        base: ["baseRefOid", "baseRefName"].some((field) =>
          pathAffected(errors, [...pullPath, "nodes", index, field])),
        native: ["mergeable", "mergeStateStatus", "reviewDecision"].some((field) =>
          pathAffected(errors, [...pullPath, "nodes", index, field])),
        linkage: pathAffected(errors, [...pullPath, "nodes", index, "closingIssuesReferences"]),
        comments: pathAffected(errors, [...pullPath, "nodes", index, "comments"]),
        reviews: pathAffected(errors, [...pullPath, "nodes", index, "reviews"]),
        checks: pathAffected(errors, [...pullPath, "nodes", index, "statusCheckRollup"]),
      })]);
  const issuesAvailability = sectionAvailability(
    issuePagePartial || issues.some((issue) => issue.availability !== "complete"),
  );
  const pullsAvailability = sectionAvailability(
    pullPagePartial || pullRequests.some((pull) => pull.availability !== "complete"),
  );
  const linkageAvailability = sectionAvailability(
    pullPagePartial || pullRequests.some((pull) => pull.linkageAvailability !== "complete"),
  );
  const issueDiscoveryAvailability = sectionAvailability(
    issuePagePartial || issues.some((issue) => issue.labelsAvailability !== "complete"),
  );
  const watchDiscoveryAvailability = sectionAvailability(
    pullPagePartial || pullNodes.some((pull, index) =>
      pull === null ||
      pagePartial(pull.comments) ||
      pathAffected(errors, [...pullPath, "nodes", index, "comments"])),
  );
  const deliveriesAvailability = sectionAvailability(
    issueDiscoveryAvailability !== "complete" ||
    linkageAvailability !== "complete",
  );
  const issuesByNumber = new Map(issues.map((issue) => [issue.number, issue]));
  const usedIssues = new Set<number>();
  const deliveries = pullRequests.flatMap<DeliveryLane>((pullRequest) => {
    const linkedIssues = pullRequest.linkedIssueNumbers.flatMap((number) => {
      const issue = issuesByNumber.get(number);
      return issue === undefined ? [] : [issue];
    });
    if (linkedIssues.length === 0) {
      return [{ issue: null, pullRequest, linkageAvailability: pullRequest.linkageAvailability }];
    }
    return linkedIssues.map((issue) => {
      usedIssues.add(issue.number);
      return { issue, pullRequest, linkageAvailability: pullRequest.linkageAvailability };
    });
  });
  for (const issue of issues) {
    if (!usedIssues.has(issue.number) && issue.labels.includes(OWNER_TOKEN)) {
      deliveries.push({ issue, pullRequest: null, linkageAvailability });
    }
  }
  deliveries.sort((left, right) =>
    (left.issue?.number ?? left.pullRequest?.number ?? 0) -
    (right.issue?.number ?? right.pullRequest?.number ?? 0));
  const currentDeliveries = issuePagePartial
    ? []
    : pullPagePartial
      ? deliveries.filter((lane) => lane.pullRequest === null)
      : deliveries;

  const roadmapSource = source(
    "Product Roadmap",
    `https://github.com/${REPOSITORY}/blob/${mainSha ?? "main"}/${ROADMAP_PATH}`,
    "repository",
  );
  const roadmapAffected = pathAffected(errors, ["repository", "roadmap"]);
  const roadmapText = repository.roadmap?.text;
  const horizon = typeof roadmapText === "string" ? parseProductHorizon(roadmapText) : null;
  const activeIssues = issues.filter((issue) => issue.labels.includes(OWNER_TOKEN));
  const criticalPathAvailability = sectionAvailability(
    issueDiscoveryAvailability !== "complete" ||
    activeIssues.some((issue) =>
      issue.identityAvailability !== "complete" ||
      issue.dependenciesAvailability !== "complete"),
  );
  const countAvailability = issueDiscoveryAvailability;
  const trustedEvidencePullNumbers = new Set(pullNodes.flatMap((pull, index) =>
    pull !== null &&
      !pagePartial(pull.comments) &&
      !pathAffected(errors, [...pullPath, "nodes", index, "comments"]) &&
      hasTrustedEvidenceMarker(present(pull.comments))
      ? [pull.number]
      : []));
  const relevantPullRequests = pullRequests.filter((pull) =>
    evidenceRelevant(pull, issuesByNumber, trustedEvidencePullNumbers));
  const watchFacts = relevantPullRequests.map((pull) => pull.stewardWatch);
  const currentWatchSources = watchFacts.flatMap((watch) =>
    watch.status === "current" && watch.source !== null ? [watch.source] : []);
  const requiredWatch = watchFacts.find((watch) => watch.status === "current" && watch.value?.includes("human action required"));
  const allWatchesCurrent =
    issueDiscoveryAvailability === "complete" &&
    linkageAvailability === "complete" &&
    watchDiscoveryAvailability === "complete" &&
    watchFacts.length > 0 &&
    watchFacts.every((watch) => watch.status === "current");
  const humanAction = requiredWatch !== undefined
    ? "Required"
    : allWatchesCurrent
      ? "None in current watches"
      : null;

  const nodes = new Map<number, { issueNumber: number; label: string; state: string; url: string }>();
  const edges: { from: number; to: number; state: string }[] = [];
  for (const issue of activeIssues) {
    nodes.set(issue.number, {
      issueNumber: issue.number,
      label: `#${String(issue.number)} · ${issue.state ?? "Unknown"}`,
      state: issue.state ?? "Unknown",
      url: issue.url,
    });
    for (const blocker of issue.blockedBy) {
      nodes.set(blocker.number, {
        issueNumber: blocker.number,
        label: `#${String(blocker.number)} · ${blocker.state}`,
        state: blocker.state,
        url: blocker.url,
      });
      edges.push({ from: issue.number, to: blocker.number, state: blocker.state });
    }
  }

  const recentNodes = present(repository.recent);
  const recentNodeMissing = repository.recent.nodes === null ||
    repository.recent.nodes.some((pull) => pull === null);
  const recentDropped = recentNodes.some((pull) => pull.mergedAt === null || pull.mergeCommit === null);
  const recentAvailability = sectionAvailability(
    pathAffected(errors, ["repository", "recent"]) || recentNodeMissing || recentDropped,
  );
  const recentItems = recentNodes.flatMap((pull) =>
    recentAvailability !== "complete" || pull.mergedAt === null || pull.mergeCommit === null
      ? []
      : [{
          number: pull.number,
          title: pull.title,
          url: pull.url,
          mergedAt: pull.mergedAt,
          mergeSha: pull.mergeCommit.oid,
        }]);

  const attention: AttentionItem[] = [{
    level: "info" as const,
    label: "Observational boundary",
    detail: "Native GitHub and Steward values are displayed directly; the Dashboard does not compute a final merge verdict.",
    sources: [repositorySource],
  }];
  if (
    hasResponseErrors ||
    mainUnavailable ||
    horizon === null ||
    issuesAvailability !== "complete" ||
    pullsAvailability !== "complete"
  ) {
    attention.unshift({
      level: "unknown",
      label: "GitHub observation partial",
      detail: "At least one response path is incomplete; affected facts remain partial or Unknown.",
      sources: [repositorySource],
    });
  }
  for (const pull of pullRequests) {
    if (!evidenceRelevant(pull, issuesByNumber, trustedEvidencePullNumbers)) continue;
    for (const [label, fact] of [["Agent handoff", pull.handoff], ["Steward watch", pull.stewardWatch]] as const) {
      if (fact.status !== "current") {
        attention.push({
          level: "unknown",
          label: `PR #${String(pull.number)} · ${label} ${fact.status}`,
          detail: fact.reason,
          sources: fact.source === null ? [source(`PR #${String(pull.number)}`, pull.url)] : [fact.source],
        });
      }
    }
    if (pull.stewardWatch.status === "current" && pull.stewardWatch.value?.startsWith("HOLD")) {
      attention.push({
        level: "attention",
        label: `PR #${String(pull.number)} · Steward HOLD`,
        detail: pull.stewardWatch.value,
        sources: pull.stewardWatch.source === null ? [] : [pull.stewardWatch.source],
      });
    }
    if (pull.stewardWatch.status === "current" && pull.stewardWatch.value?.includes("human action required")) {
      attention.push({
        level: "attention",
        label: `PR #${String(pull.number)} · Human action required`,
        detail: pull.stewardWatch.value,
        sources: pull.stewardWatch.source === null ? [] : [pull.stewardWatch.source],
      });
    }
  }

  return {
    repository: REPOSITORY,
    observedAt,
    fetchHealth: hasResponseErrors || mainUnavailable || horizon === null || issuesAvailability !== "complete" || pullsAvailability !== "complete" || recentAvailability !== "complete"
      ? "partial"
      : "healthy",
    executive: {
      mainSha: {
        value: mainSha,
        availability: sectionAvailability(mainUnavailable),
        source: source("Live main", main?.url ?? repository.url),
      },
      productHorizon: {
        value: roadmapAffected ? null : horizon,
        availability: horizon === null || roadmapAffected
          ? "partial"
          : "complete",
        source: roadmapSource,
      },
      activeCount: {
        value: countAvailability === "complete" ? activeIssues.length : null,
        availability: countAvailability,
        source: repositorySource,
      },
      readyCount: {
        value: countAvailability === "complete"
          ? activeIssues.filter((issue) => issue.labels.includes("state:ready")).length
          : null,
        availability: countAvailability,
        source: repositorySource,
      },
      humanAction: {
        value: humanAction,
        availability: humanAction === null ? "partial" : "complete",
        source: requiredWatch?.source ?? currentWatchSources[0] ?? repositorySource,
        ...(requiredWatch === undefined && currentWatchSources.length > 1
          ? { additionalSources: currentWatchSources.slice(1) }
          : {}),
      },
    },
    deliveries: currentDeliveries,
    deliveriesAvailability,
    criticalPath: {
      availability: criticalPathAvailability,
      nodes: criticalPathAvailability === "complete"
        ? [...nodes.values()].sort((left, right) => left.issueNumber - right.issueNumber)
        : [],
      edges: criticalPathAvailability === "complete" ? edges : [],
      source: repositorySource,
    },
    recentActivity: {
      availability: recentAvailability,
      items: recentItems,
      source: repositorySource,
    },
    attention,
    sources: [repositorySource, roadmapSource],
  };
}

export async function observeRepository(options: ObserveOptions = {}): Promise<DashboardProjection> {
  const observedAt = options.observedAt ?? new Date().toISOString();
  try {
    const response = await (options.fetchImpl ?? fetch)(GRAPHQL_URL, {
      method: "POST",
      headers: {
        Accept: "application/vnd.github+json",
        "Content-Type": "application/json",
        "X-GitHub-Api-Version": "2022-11-28",
        ...(options.token === undefined ? {} : { Authorization: `Bearer ${options.token}` }),
      },
      body: JSON.stringify({ query: DASHBOARD_QUERY, variables: { owner: OWNER, name: "tachiko-work" } }),
      signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
    });
    if (!response.ok) return unavailableProjection(observedAt);
    return projectGraphResponse((await response.json()) as DashboardGraphResponse, observedAt);
  } catch {
    return unavailableProjection(observedAt);
  }
}

export function readServerCredential(environment: NodeJS.ProcessEnv = process.env): string | undefined {
  for (const value of [environment.GITHUB_TOKEN, environment.GH_TOKEN]) {
    const trimmed = value?.trim();
    if (trimmed !== undefined && trimmed.length > 0) return trimmed;
  }
  return undefined;
}
