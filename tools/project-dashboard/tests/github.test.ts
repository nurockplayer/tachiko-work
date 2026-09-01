import { describe, expect, it } from "vitest";

import { observeRepository } from "../src/server/github.js";
import { normalizeRepository } from "../src/server/normalize.js";

const MAIN = "1111111111111111111111111111111111111111";
const HEAD = "2222222222222222222222222222222222222222";
const MERGE_BASE = "0000000000000000000000000000000000000000";

type FixturePull = ReturnType<typeof graphResponse>["data"]["repository"]["pullRequests"]["nodes"][number];
type FixtureGraph = ReturnType<typeof graphResponse>;

function graphResponse(
  hasNextPage = false,
  labelsHaveNextPage = hasNextPage,
  hasRoadmap = true,
  pullsHaveNextPage = false,
  closingIssuesHaveNextPage = false,
  hasErrors = false,
  commentsHaveNextPage = false,
  mergeable: "MERGEABLE" | "CONFLICTING" | "UNKNOWN" = "MERGEABLE",
  reviewsHaveNextPage = false,
) {
  return {
    ...(hasErrors
      ? { errors: [{ message: "partial GraphQL response", path: ["repository", "pullRequests"] }] }
      : {}),
    data: {
      repository: {
        url: "https://github.example/repository",
        defaultBranchRef: {
          name: "main",
          target: { oid: MAIN, url: `https://github.example/commit/${MAIN}` },
        },
        roadmap: hasRoadmap
          ? {
              oid: "roadmap-oid",
              text: "## Current horizon\n\n> **06 · Team Workspace Beta**\n\n## Product stages",
            }
          : null,
        issues: {
          pageInfo: { hasNextPage },
          nodes: [
            {
              number: 169,
              title: "Dashboard",
              url: "https://github.example/issues/169",
              state: "OPEN",
              labels: {
                pageInfo: { hasNextPage: labelsHaveNextPage },
                nodes: [{ name: "agent:codex" }, { name: "state:ready" }],
              },
              milestone: null,
              blockedBy: { pageInfo: { hasNextPage: false }, nodes: [] },
            },
          ],
        },
        pullRequests: {
          pageInfo: { hasNextPage: pullsHaveNextPage },
          nodes: [
            {
              number: 225,
              title: "Dashboard",
              url: "https://github.example/pulls/225",
              state: "OPEN",
              isDraft: false,
              headRefOid: HEAD,
              baseRefOid: MAIN,
              baseRefName: "main",
              mergeable,
              mergeStateStatus: "CLEAN" as const,
              reviewDecision: null,
              closingIssuesReferences: {
                pageInfo: { hasNextPage: closingIssuesHaveNextPage },
                nodes: [{ number: 169 }],
              },
              comments: { pageInfo: { hasNextPage: commentsHaveNextPage }, nodes: [] },
              reviews: { pageInfo: { hasNextPage: reviewsHaveNextPage }, nodes: [] },
              reviewThreads: { pageInfo: { hasNextPage: false }, nodes: [] },
              statusCheckRollup: {
                contexts: {
                  pageInfo: { hasNextPage: false },
                  nodes: [
                    {
                      __typename: "CheckRun",
                      id: "check-1",
                      name: "Live Project Dashboard browser journey",
                      status: "COMPLETED",
                      conclusion: "SKIPPED",
                      url: "https://github.example/checks/1",
                      detailsUrl: null,
                    },
                  ],
                },
              },
            },
          ],
        },
        recent: { pageInfo: { hasNextPage: true }, nodes: [] },
      },
    },
  };
}

function fakeFetch(
  hasNextPage = false,
  labelsHaveNextPage = hasNextPage,
  hasRoadmap = true,
  pullsHaveNextPage = false,
  closingIssuesHaveNextPage = false,
  hasErrors = false,
  commentsHaveNextPage = false,
  mergeable: "MERGEABLE" | "CONFLICTING" | "UNKNOWN" = "MERGEABLE",
  reviewsHaveNextPage = false,
) {
  const requests: { url: string; init?: RequestInit }[] = [];
  const implementation = (async (input: string | URL | Request, init?: RequestInit) => {
    const url =
      typeof input === "string"
        ? input
        : input instanceof URL
          ? input.href
          : input.url;
    requests.push(init === undefined ? { url } : { url, init });
    if (url.endsWith("/graphql")) {
      return new Response(
        JSON.stringify(
          graphResponse(
            hasNextPage,
            labelsHaveNextPage,
            hasRoadmap,
            pullsHaveNextPage,
            closingIssuesHaveNextPage,
            hasErrors,
            commentsHaveNextPage,
            mergeable,
            reviewsHaveNextPage,
          ),
        ),
        {
        status: 200,
        headers: { "Content-Type": "application/json" },
        },
      );
    }
    return new Response(
      JSON.stringify({ status: "ahead", merge_base_commit: { sha: MAIN } }),
      { status: 200, headers: { "Content-Type": "application/json" } },
    );
  });
  return { implementation, requests };
}

function fetchForGraph(graph: ReturnType<typeof graphResponse>): typeof fetch {
  return (async (input: string | URL | Request) => {
    const url = typeof input === "string"
      ? input
      : input instanceof URL
        ? input.href
        : input.url;
    return url.endsWith("/graphql")
      ? new Response(JSON.stringify(graph), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        })
      : new Response(
          JSON.stringify({ status: "ahead", merge_base_commit: { sha: MAIN } }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        );
  });
}

function appendNull(nodes: unknown[]): void {
  nodes.push(null);
}

function addGraphError(graph: FixtureGraph, path: readonly (string | number)[]): void {
  Object.assign(graph, { errors: [{ message: "partial field observation", path }] });
}

function fakeBehindAuthorityFetch() {
  const requests: string[] = [];
  const implementation = (async (input: string | URL | Request) => {
    const url =
      typeof input === "string"
        ? input
        : input instanceof URL
          ? input.href
          : input.url;
    requests.push(url);
    if (url.endsWith("/graphql")) {
      return new Response(JSON.stringify(graphResponse()), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }
    const body = url.endsWith(`${MAIN}...${HEAD}`)
      ? { status: "diverged", merge_base_commit: { sha: MERGE_BASE } }
      : {
          status: "ahead",
          merge_base_commit: { sha: MERGE_BASE },
          files: [
            { filename: "docs/architecture/document-model.md" },
            { filename: "docs/decisions/ADR-0032-dashboard.md" },
            { filename: "docs/security/threat-model.md" },
            { filename: "docs/vision/product-constitution.md" },
            { filename: "docs/product/engine-integration-strategy.md" },
            { filename: ".github/workflows/ci.yml" },
            { filename: "scripts/release-check.sh" },
            { filename: "Cargo.toml" },
            { filename: "Cargo.lock" },
            { filename: "crates/cli/Cargo.toml" },
            { filename: "apps/designer/runtime/Cargo.lock" },
            { filename: "crates/cli/AGENTS.md" },
            {
              filename: "docs/discussions/renamed-authority.md",
              previous_filename: "docs/decisions/ADR-renamed.md",
            },
            {
              filename: "docs/discussions/renamed-workflow.md",
              previous_filename: ".github/workflows/renamed.yml",
            },
            {
              filename: "docs/governance/renamed-in.md",
              previous_filename: "docs/discussions/draft.md",
            },
            {
              filename: "docs/discussions/renamed-manifest.md",
              previous_filename: "crates/storage/Cargo.toml",
            },
            {
              filename: "docs/decisions/ADR-0032-dashboard.md",
              previous_filename: "docs/decisions/ADR-0032-dashboard.md",
            },
            { filename: "docs/discussions/history.md" },
          ],
        };
    return new Response(JSON.stringify(body), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  }) as typeof fetch;
  return { implementation, requests };
}

function fakeComparisonFailureFetch() {
  const graph = graphResponse();
  const first = graph.data.repository.pullRequests.nodes[0];
  if (first === undefined) throw new Error("fixture missing pull request");
  graph.data.repository.pullRequests.nodes.push({
    ...first,
    number: 226,
    url: "https://github.example/pulls/226",
    headRefOid: "3333333333333333333333333333333333333333",
    closingIssuesReferences: {
      pageInfo: { hasNextPage: false },
      nodes: [],
    },
  });
  const implementation = (async (input: string | URL | Request) => {
    const url =
      typeof input === "string"
        ? input
        : input instanceof URL
          ? input.href
          : input.url;
    if (url.endsWith("/graphql")) {
      return new Response(JSON.stringify(graph), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }
    if (url.includes(HEAD)) {
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
    return new Response("comparison unavailable", { status: 503 });
  }) as typeof fetch;
  return implementation;
}

describe("GitHub observation adapter", () => {
  it("uses only a bounded query/GET path and treats skipped browser evidence as failure", async () => {
    const fake = fakeFetch();
    const observation = await observeRepository({
      token: "server_secret",
      fetchImpl: fake.implementation,
    });
    const projection = normalizeRepository(observation);

    expect(observation.availability).toBe("complete");
    expect(projection.deliveries[0]?.checks.state).toBe("blocked");
    expect(projection.deliveries[0]?.evidence.automatedBrowser.state).toBe("blocked");
    expect(fake.requests.map((request) => request.init?.method)).toEqual(["POST", "GET"]);
    const graphBody = fake.requests[0]?.init?.body;
    if (typeof graphBody !== "string") throw new Error("GraphQL body must be JSON text");
    expect(graphBody).toContain("query DashboardRepository");
    expect(graphBody).toContain("state submittedAt");
    expect(graphBody).toContain("baseRefName mergeable");
    expect(graphBody).toContain("mergeStateStatus reviewDecision");
    expect(graphBody.match(/\blastEditedAt\b/g)).toHaveLength(3);
    expect(graphBody).not.toContain("mutation");
    expect(observation.serverCredential).toBe("present");
    expect(JSON.stringify(observation)).not.toContain("server_secret");
    expect(JSON.stringify(projection)).not.toContain("server_secret");
    expect(observation.pullRequests[0]?.nativeMergePolicy).toEqual({ state: "satisfied" });
  });

  it("preserves reviewer trust and globally unique GraphQL source IDs", async () => {
    const graph = graphResponse();
    const pull = graph.data.repository.pullRequests.nodes[0];
    if (pull === undefined) throw new Error("fixture missing pull request");
    const reviews = [
      {
        id: "review-member",
        fullDatabaseId: "101",
        body: "Structured review body",
        url: "https://github.example/reviews/member",
        createdAt: "2026-09-01T00:00:00.000Z",
        updatedAt: "2026-09-01T00:00:00.000Z",
        lastEditedAt: null,
        state: "APPROVED",
        submittedAt: "2026-09-01T00:00:00.000Z",
        author: { login: "member" },
        authorAssociation: "MEMBER",
        commit: { oid: HEAD },
      },
      {
        id: "review-outsider",
        fullDatabaseId: "102",
        body: "",
        url: "https://github.example/reviews/outsider",
        createdAt: "2026-09-01T00:01:00.000Z",
        updatedAt: "2026-09-01T00:01:00.000Z",
        lastEditedAt: null,
        state: "CHANGES_REQUESTED",
        submittedAt: "2026-09-01T00:01:00.000Z",
        author: { login: "outsider" },
        authorAssociation: "NONE",
        commit: { oid: HEAD },
      },
      {
        id: "review-without-database-id",
        fullDatabaseId: null,
        body: "Review without a database ID",
        url: "https://github.example/reviews/no-database-id",
        createdAt: "2026-09-01T00:02:00.000Z",
        updatedAt: "2026-09-01T00:02:00.000Z",
        lastEditedAt: null,
        state: "COMMENTED",
        submittedAt: "2026-09-01T00:02:00.000Z",
        author: { login: "member" },
        authorAssociation: "MEMBER",
        commit: { oid: HEAD },
      },
    ];
    (pull.reviews as unknown as { nodes: typeof reviews }).nodes = reviews;
    const issueComments = [
      {
        id: "issue-comment-node",
        databaseId: 101,
        body: "Unstructured issue comment",
        url: "https://github.example/comments/101",
        createdAt: "2026-09-01T00:00:00.000Z",
        updatedAt: "2026-09-01T00:00:00.000Z",
        lastEditedAt: "2026-09-01T00:03:00.000Z",
        author: { login: "member" },
        authorAssociation: "MEMBER",
      },
    ];
    (pull.comments as unknown as { nodes: typeof issueComments }).nodes = issueComments;
    const threadComments = [
      {
        id: "thread-comment-node",
        databaseId: 101,
        body: "Unstructured review-thread comment",
        url: "https://github.example/review-comments/101",
        createdAt: "2026-09-01T00:04:00.000Z",
        updatedAt: "2026-09-01T00:04:00.000Z",
        lastEditedAt: null,
        author: { login: "member" },
        authorAssociation: "MEMBER",
      },
    ];
    (pull.reviewThreads.nodes as unknown[]).push({
      id: "thread-with-shared-database-id",
      isResolved: true,
      isOutdated: false,
      comments: { pageInfo: { hasNextPage: false }, nodes: threadComments },
    });
    const fetchImpl = (async (input: string | URL | Request) => {
      const url = typeof input === "string"
        ? input
        : input instanceof URL
          ? input.href
          : input.url;
      return url.endsWith("/graphql")
        ? new Response(JSON.stringify(graph), {
            status: 200,
            headers: { "Content-Type": "application/json" },
          })
        : new Response(
            JSON.stringify({ status: "ahead", merge_base_commit: { sha: MAIN } }),
            { status: 200, headers: { "Content-Type": "application/json" } },
          );
    }) as typeof fetch;

    const observation = await observeRepository({ fetchImpl });
    expect(
      observation.pullRequests[0]?.reviews.map((review) => ({
        id: review.id,
        association: review.authorAssociation,
      })),
    ).toEqual([
      { id: "review-member", association: "MEMBER" },
      { id: "review-outsider", association: "NONE" },
      { id: "review-without-database-id", association: "MEMBER" },
    ]);
    expect(observation.pullRequests[0]?.comments).toContainEqual(
      expect.objectContaining({ id: "review-member", kind: "pull-request-review" }),
    );
    expect(observation.pullRequests[0]?.comments).toContainEqual(
      expect.objectContaining({
        id: "review-without-database-id",
        kind: "pull-request-review",
      }),
    );
    expect(observation.pullRequests[0]?.comments).toContainEqual(
      expect.objectContaining({
        id: "issue-comment-node",
        kind: "issue-comment",
        lastEditedAt: { state: "value", value: "2026-09-01T00:03:00.000Z" },
      }),
    );
    expect(observation.pullRequests[0]?.comments).toContainEqual(
      expect.objectContaining({
        id: "thread-comment-node",
        kind: "pull-request-review-comment",
      }),
    );
    expect(
      new Set(observation.pullRequests[0]?.comments.map((comment) => comment.id)).size,
    ).toBe(observation.pullRequests[0]?.comments.length);
    expect(normalizeRepository(observation).deliveries[0]?.review.reason).not.toBe(
      "source-identity-conflict",
    );
  });

  it.each(
    (["issue-comment", "pull-request-review", "pull-request-review-comment"] as const)
      .flatMap((kind) =>
        (["unedited", "edited", "missing", "partial-error-null"] as const).map(
          (editState) => [kind, editState] as const,
        ),
      ),
  )("fails structured %s evidence closed for %s edit metadata", async (kind, editState) => {
    const graph = graphResponse();
    const pull = graph.data.repository.pullRequests.nodes[0];
    if (pull === undefined) throw new Error("fixture missing pull request");
    const body = [
      "<!-- operational-evidence:v1",
      "KIND: validation",
      "PR: 225",
      `HEAD: ${HEAD}`,
      `RUN: ${kind}-${editState}`,
      "NAME: release-check",
      "RESULT: pass",
      "-->",
    ].join("\n");
    const comment: Record<string, unknown> = {
      id: `node-${kind}-${editState}`,
      databaseId: 101,
      body,
      url: `https://github.example/${kind}/${editState}`,
      createdAt: "2026-09-01T00:00:00.000Z",
      updatedAt: "2026-09-01T00:00:00.000Z",
      ...(editState === "missing"
        ? {}
        : {
            lastEditedAt:
              editState === "edited" ? "2026-09-01T00:01:00.000Z" : null,
          }),
      author: { login: "nurockplayer" },
      authorAssociation: "OWNER",
    };

    if (kind === "issue-comment") {
      Object.assign(pull.comments, { nodes: [comment] });
    } else if (kind === "pull-request-review") {
      Object.assign(pull.reviews, {
        nodes: [
          {
            ...comment,
            fullDatabaseId: "101",
            state: "COMMENTED",
            submittedAt: "2026-09-01T00:00:00.000Z",
            commit: { oid: HEAD },
          },
        ],
      });
    } else {
      Object.assign(pull.reviewThreads, {
        nodes: [
          {
            id: `thread-${editState}`,
            isResolved: true,
            isOutdated: false,
            comments: { pageInfo: { hasNextPage: false }, nodes: [comment] },
          },
        ],
      });
    }
    if (editState === "partial-error-null") {
      const sourcePath = kind === "issue-comment"
        ? ["repository", "pullRequests", "nodes", 0, "comments", "nodes", 0]
        : kind === "pull-request-review"
          ? ["repository", "pullRequests", "nodes", 0, "reviews", "nodes", 0]
          : [
              "repository",
              "pullRequests",
              "nodes",
              0,
              "reviewThreads",
              "nodes",
              0,
              "comments",
              "nodes",
              0,
            ];
      addGraphError(graph, [...sourcePath, "lastEditedAt"]);
    }

    const observation = await observeRepository({ fetchImpl: fetchForGraph(graph) });
    const observed = observation.pullRequests[0]?.comments.find(
      (candidate) => candidate.id === comment.id,
    );
    const integrity = normalizeRepository(observation).deliveries[0]?.evidence.deliveryIntegrity;

    expect(observed?.lastEditedAt).toMatchObject(
      editState === "unedited"
        ? { state: "null" }
        : editState === "edited"
          ? { state: "value", value: "2026-09-01T00:01:00.000Z" }
          : { state: "unknown", availability: "incomplete" },
    );
    expect(integrity).toMatchObject(
      editState === "unedited"
        ? { state: "satisfied", reason: "validation-passed" }
        : { state: "unknown", reason: "source-edited" },
    );
  });

  it.each([
    ["observed value", { title: "06 · Team Workspace Beta" }, null, "satisfied"],
    ["observed null", null, null, "satisfied"],
    [
      "partial-error null",
      null,
      ["repository", "issues", "nodes", 0, "milestone"],
      "unknown",
    ],
  ] as const)(
    "distinguishes %s milestone observation",
    async (_case, milestone, errorPath, expectedReadiness) => {
      const graph = graphResponse();
      const issue = graph.data.repository.issues.nodes[0];
      if (issue === undefined) throw new Error("fixture missing issue");
      Object.assign(issue, { milestone });
      if (errorPath !== null) addGraphError(graph, errorPath);

      const observation = await observeRepository({ fetchImpl: fetchForGraph(graph) });
      expect(normalizeRepository(observation).deliveries[0]?.readiness.state).toBe(
        expectedReadiness,
      );
      if (errorPath !== null) {
        expect(observation.errors).toContainEqual(
          expect.objectContaining({ path: errorPath }),
        );
      }
    },
  );

  it.each([
    ["observed value", "REVIEW_REQUIRED", null, "waiting"],
    ["observed null", null, null, "satisfied"],
    [
      "partial-error null",
      null,
      ["repository", "pullRequests", "nodes", 0, "reviewDecision"],
      "unknown",
    ],
  ] as const)(
    "decodes %s native review-decision observation",
    async (_case, reviewDecision, errorPath, expectedState) => {
      const graph = graphResponse();
      const pull = graph.data.repository.pullRequests.nodes[0];
      if (pull === undefined) throw new Error("fixture missing pull request");
      Object.assign(pull, { reviewDecision });
      if (errorPath !== null) addGraphError(graph, errorPath);

      const observation = await observeRepository({ fetchImpl: fetchForGraph(graph) });
      expect(observation.pullRequests[0]?.nativeMergePolicy).toMatchObject({
        state: expectedState,
      });
    },
  );

  it.each([
    ["observed value", false, "complete"],
    ["observed null", true, "complete"],
    ["partial-error null", true, "incomplete"],
  ] as const)(
    "distinguishes %s status-check rollup observation",
    async (_case, makeNull, expectedAvailability) => {
      const graph = graphResponse();
      const pull = graph.data.repository.pullRequests.nodes[0];
      if (pull === undefined) throw new Error("fixture missing pull request");
      if (makeNull) Object.assign(pull, { statusCheckRollup: null });
      if (_case === "partial-error null") {
        addGraphError(graph, [
          "repository",
          "pullRequests",
          "nodes",
          0,
          "statusCheckRollup",
        ]);
      }

      const observation = await observeRepository({ fetchImpl: fetchForGraph(graph) });
      expect(observation.pullRequests[0]?.checksAvailability).toBe(expectedAvailability);
    },
  );

  it.each([
    ["observed value", "SUCCESS", null, "value", "satisfied"],
    ["observed null", null, null, "null", "unknown"],
    [
      "partial-error null",
      null,
      [
        "repository",
        "pullRequests",
        "nodes",
        0,
        "statusCheckRollup",
        "contexts",
        "nodes",
        0,
        "conclusion",
      ],
      "unknown",
      "unknown",
    ],
  ] as const)(
    "distinguishes %s CheckRun conclusion",
    async (_case, conclusion, errorPath, expectedObservation, expectedSignal) => {
      const graph = graphResponse();
      const pull = graph.data.repository.pullRequests.nodes[0];
      if (pull === undefined) throw new Error("fixture missing pull request");
      const check = pull.statusCheckRollup.contexts.nodes[0];
      if (check === undefined) throw new Error("fixture missing check run");
      Object.assign(check, { conclusion });
      if (errorPath !== null) addGraphError(graph, errorPath);

      const observation = await observeRepository({ fetchImpl: fetchForGraph(graph) });
      expect(observation.pullRequests[0]?.checks[0]).toMatchObject({
        headSha: { state: "value", value: HEAD },
        result: { state: expectedObservation },
      });
      expect(normalizeRepository(observation).deliveries[0]?.checks.state).toBe(
        expectedSignal,
      );
    },
  );

  it.each([
    ["observed value", { oid: HEAD }, "SUCCESS", null, "value", "success", "satisfied"],
    ["observed null", null, "FAILURE", null, "null", "failure", "unknown"],
    [
      "partial-error null",
      null,
      "FAILURE",
      [
        "repository",
        "pullRequests",
        "nodes",
        0,
        "statusCheckRollup",
        "contexts",
        "nodes",
        0,
        "commit",
      ],
      "unknown",
      "failure",
      "unknown",
    ],
  ] as const)(
    "distinguishes %s StatusContext commit identity",
    async (
      _case,
      commit,
      state,
      errorPath,
      expectedObservation,
      expectedResult,
      expectedSignal,
    ) => {
      const graph = graphResponse();
      const pull = graph.data.repository.pullRequests.nodes[0];
      if (pull === undefined) throw new Error("fixture missing pull request");
      Object.assign(pull.statusCheckRollup.contexts, {
        nodes: [
          {
            __typename: "StatusContext",
            id: `status-${_case}`,
            context: "Live Project Dashboard browser journey",
            state,
            targetUrl: null,
            commit,
          },
        ],
      });
      if (errorPath !== null) addGraphError(graph, errorPath);

      const observation = await observeRepository({ fetchImpl: fetchForGraph(graph) });
      expect(observation.pullRequests[0]?.checks[0]).toMatchObject({
        headSha: { state: expectedObservation },
        result: { state: "value", value: expectedResult },
      });
      expect(normalizeRepository(observation).deliveries[0]?.checks.state).toBe(
        expectedSignal,
      );
    },
  );

  it.each([
    ["CheckRun status", "CheckRun", "status", "FUTURE"],
    ["CheckRun conclusion", "CheckRun", "conclusion", "FUTURE"],
    ["StatusContext state", "StatusContext", "state", "FUTURE"],
  ] as const)(
    "decodes an unrecognized %s as Unknown",
    async (_case, kind, field, value) => {
      const graph = graphResponse();
      const pull = graph.data.repository.pullRequests.nodes[0];
      if (pull === undefined) throw new Error("fixture missing pull request");
      if (kind === "CheckRun") {
        const check = pull.statusCheckRollup.contexts.nodes[0];
        if (check === undefined) throw new Error("fixture missing check run");
        Object.assign(check, { [field]: value });
      } else {
        Object.assign(pull.statusCheckRollup.contexts, {
          nodes: [
            {
              __typename: "StatusContext",
              id: "status-unknown",
              context: "Live Project Dashboard browser journey",
              state: value,
              targetUrl: null,
              commit: { oid: HEAD },
            },
          ],
        });
      }

      const observation = await observeRepository({ fetchImpl: fetchForGraph(graph) });
      expect(observation.pullRequests[0]?.checks[0]?.result).toMatchObject({
        state: "unknown",
      });
      expect(normalizeRepository(observation).deliveries[0]?.checks.state).toBe("unknown");
    },
  );

  it.each(["author", "commit", "submittedAt"] as const)(
    "marks a partial-error null review %s incomplete",
    async (field) => {
      const graph = graphResponse();
      const pull = graph.data.repository.pullRequests.nodes[0];
      if (pull === undefined) throw new Error("fixture missing pull request");
      const review = {
        id: `review-${field}`,
        fullDatabaseId: "101",
        body: "",
        url: `https://github.example/reviews/${field}`,
        createdAt: "2026-09-01T00:00:00.000Z",
        updatedAt: "2026-09-01T00:00:00.000Z",
        lastEditedAt: null,
        state: "APPROVED",
        submittedAt: "2026-09-01T00:00:00.000Z",
        author: { login: "nurockplayer" },
        authorAssociation: "OWNER",
        commit: { oid: HEAD },
      };
      Object.assign(review, { [field]: null });
      Object.assign(pull.reviews, { nodes: [review] });
      addGraphError(graph, [
        "repository",
        "pullRequests",
        "nodes",
        0,
        "reviews",
        "nodes",
        0,
        field,
      ]);

      const observation = await observeRepository({ fetchImpl: fetchForGraph(graph) });
      expect(observation.pullRequests[0]?.reviewsAvailability).toBe("incomplete");
    },
  );

  it("marks completeness-required pagination as partial but keeps bounded recent history valid", async () => {
    const fake = fakeFetch(true);
    const observation = await observeRepository({ fetchImpl: fake.implementation });

    expect(observation.availability).toBe("incomplete");
    expect(observation.issuesAvailability).toBe("incomplete");
    expect(observation.recentActivityAvailability).toBe("complete");
    expect(normalizeRepository(observation).fetchHealth).toBe("partial");
  });

  it("does not grant readiness, ownership, or merge state from truncated labels", async () => {
    const fake = fakeFetch(false, true);
    const observation = await observeRepository({ fetchImpl: fake.implementation });
    const lane = normalizeRepository(observation).deliveries[0];

    expect(observation.issues[0]?.labelsAvailability).toBe("incomplete");
    expect(lane?.owner).toBe("unknown");
    expect(lane?.readiness.state).toBe("unknown");
    expect(lane?.mergeGate.state).not.toBe("satisfied");
    expect(normalizeRepository(observation).executive.readyCount.state).toBe("unknown");
  });

  it("marks a missing Product Roadmap source partial and the horizon Unknown", async () => {
    const fake = fakeFetch(false, false, false);
    const observation = await observeRepository({ fetchImpl: fake.implementation });
    const projection = normalizeRepository(observation);

    expect(observation.availability).toBe("incomplete");
    expect(projection.fetchHealth).toBe("partial");
    expect(projection.executive.productHorizon.state).toBe("unknown");
    expect(projection.deliveries[0]?.mergeGate.state).not.toBe("satisfied");
  });

  it("keeps implementation linkage incomplete for a partial GraphQL response", async () => {
    const fake = fakeFetch(false, false, true, false, false, true);
    const observation = await observeRepository({ fetchImpl: fake.implementation });

    expect(observation.implementationLinkageAvailability).toBe("incomplete");
    expect(normalizeRepository(observation).deliveries[0]?.mergeGate.state).not.toBe(
      "satisfied",
    );
  });

  it.each([
    [
      "recent activity",
      ["repository", "recent", "nodes", 0, "mergeCommit"],
      "complete",
      "incomplete",
    ],
    [
      "Issue milestone",
      ["repository", "issues", "nodes", 0, "milestone"],
      "complete",
      "complete",
    ],
    [
      "status-check conclusion",
      [
        "repository",
        "pullRequests",
        "nodes",
        0,
        "statusCheckRollup",
        "contexts",
        "nodes",
        0,
        "conclusion",
      ],
      "complete",
      "complete",
    ],
    [
      "closing Issue references",
      ["repository", "pullRequests", "nodes", 0, "closingIssuesReferences"],
      "incomplete",
      "complete",
    ],
    [
      "top-level Issue comments",
      ["repository", "pullRequests", "nodes", 0, "comments"],
      "incomplete",
      "complete",
    ],
    [
      "top-level reviews",
      ["repository", "pullRequests", "nodes", 0, "reviews"],
      "incomplete",
      "complete",
    ],
  ] as const)(
    "scopes a %s GraphQL error to the affected source",
    async (_case, errorPath, expectedLinkage, expectedRecent) => {
      const graph = graphResponse();
      addGraphError(graph, errorPath);

      const observation = await observeRepository({ fetchImpl: fetchForGraph(graph) });
      expect(observation.implementationLinkageAvailability).toBe(expectedLinkage);
      expect(observation.recentActivityAvailability).toBe(expectedRecent);
    },
  );

  it("treats a pathless GraphQL error as globally incomplete", async () => {
    const graph = graphResponse();
    Object.assign(graph, { errors: [{ message: "unscoped partial response" }] });

    const observation = await observeRepository({ fetchImpl: fetchForGraph(graph) });
    expect(observation.implementationLinkageAvailability).toBe("incomplete");
    expect(observation.recentActivityAvailability).toBe("incomplete");
  });

  it.each([
    ["MERGEABLE", "satisfied"],
    ["CONFLICTING", "blocked"],
    ["UNKNOWN", "unknown"],
  ] as const)("decodes native mergeability %s", async (native, expected) => {
    const fake = fakeFetch(false, false, true, false, false, false, false, native);
    const observation = await observeRepository({ fetchImpl: fake.implementation });

    expect(observation.pullRequests[0]?.nativeMergePolicy.state).toBe(expected);
  });

  it.each([
    ["clean", {}, "satisfied", undefined],
    ["draft", { isDraft: true }, "blocked", "policy"],
    ["merge conflict", { mergeable: "CONFLICTING" }, "blocked", "conflict"],
    ["dirty", { mergeStateStatus: "DIRTY" }, "blocked", "policy"],
    ["blocked", { mergeStateStatus: "BLOCKED" }, "blocked", "policy"],
    ["behind", { mergeStateStatus: "BEHIND" }, "blocked", "policy"],
    ["merge state unknown", { mergeStateStatus: "UNKNOWN" }, "unknown", undefined],
    ["unstable", { mergeStateStatus: "UNSTABLE" }, "satisfied", undefined],
    ["hooks", { mergeStateStatus: "HAS_HOOKS" }, "satisfied", undefined],
    ["review required", { reviewDecision: "REVIEW_REQUIRED" }, "waiting", undefined],
    ["changes requested", { reviewDecision: "CHANGES_REQUESTED" }, "blocked", "policy"],
    ["approved", { reviewDecision: "APPROVED" }, "satisfied", undefined],
    ["unknown draft input", { isDraft: "false" }, "unknown", undefined],
    ["unknown mergeability input", { mergeable: "FUTURE" }, "unknown", undefined],
    ["unknown merge-state input", { mergeStateStatus: "FUTURE" }, "unknown", undefined],
    ["unknown review input", { reviewDecision: "FUTURE" }, "unknown", undefined],
  ] as const)(
    "decodes native policy for %s",
    async (_case, inputs, expectedState, expectedReason) => {
      const graph = graphResponse();
      const pull = graph.data.repository.pullRequests.nodes[0];
      if (pull === undefined) throw new Error("fixture missing pull request");
      Object.assign(pull, inputs);

      const observation = await observeRepository({ fetchImpl: fetchForGraph(graph) });
      expect(observation.pullRequests[0]?.nativeMergePolicy).toMatchObject({
        state: expectedState,
        ...(expectedReason === undefined ? {} : { reason: expectedReason }),
      });
    },
  );

  it.each([
    ["isDraft", ["isDraft"]],
    ["mergeable", ["mergeable"]],
    ["mergeStateStatus", ["mergeStateStatus"]],
    ["reviewDecision", ["reviewDecision"]],
  ] as const)(
    "decodes a partial-error null %s as native policy Unknown",
    async (field, suffix) => {
      const graph = graphResponse();
      const pull = graph.data.repository.pullRequests.nodes[0];
      if (pull === undefined) throw new Error("fixture missing pull request");
      Object.assign(pull, { [field]: null });
      addGraphError(graph, ["repository", "pullRequests", "nodes", 0, ...suffix]);

      const observation = await observeRepository({ fetchImpl: fetchForGraph(graph) });
      expect(observation.pullRequests[0]?.nativeMergePolicy).toMatchObject({
        state: "unknown",
      });
    },
  );

  it.each([
    [
      "Blocked over Unknown",
      { isDraft: true, mergeStateStatus: "UNKNOWN" },
      "blocked",
    ],
    [
      "Unknown over Waiting",
      { mergeable: "UNKNOWN", reviewDecision: "REVIEW_REQUIRED" },
      "unknown",
    ],
    ["Waiting", { reviewDecision: "REVIEW_REQUIRED" }, "waiting"],
  ] as const)("preserves %s native-policy precedence", async (_case, inputs, expected) => {
    const graph = graphResponse();
    const pull = graph.data.repository.pullRequests.nodes[0];
    if (pull === undefined) throw new Error("fixture missing pull request");
    Object.assign(pull, inputs);

    const observation = await observeRepository({ fetchImpl: fetchForGraph(graph) });
    expect(observation.pullRequests[0]?.nativeMergePolicy).toMatchObject({ state: expected });
  });

  it("keeps handoff ownership overlap incomplete when top-level comments are truncated", async () => {
    const fake = fakeFetch(false, false, true, false, false, false, true);
    const observation = await observeRepository({ fetchImpl: fake.implementation });

    expect(observation.pullRequests[0]?.commentsAvailability).toBe("incomplete");
    expect(observation.implementationLinkageAvailability).toBe("incomplete");
    expect(normalizeRepository(observation).deliveries[0]?.mergeGate.state).not.toBe(
      "satisfied",
    );
  });

  it("keeps handoff ownership overlap incomplete when top-level reviews are truncated", async () => {
    const fake = fakeFetch(
      false,
      false,
      true,
      false,
      false,
      false,
      false,
      "MERGEABLE",
      true,
    );
    const observation = await observeRepository({ fetchImpl: fake.implementation });
    const lane = normalizeRepository(observation).deliveries[0];

    expect(observation.pullRequests[0]?.reviewsAvailability).toBe("incomplete");
    expect(observation.implementationLinkageAvailability).toBe("incomplete");
    expect(lane?.authority).toMatchObject({
      state: "unknown",
      reason: "observation-incomplete",
    });
  });

  it("scopes truncated review threads to lane review evidence", async () => {
    const graph = graphResponse();
    const pull = graph.data.repository.pullRequests.nodes[0];
    if (pull === undefined) throw new Error("fixture missing pull request");
    pull.reviewThreads.pageInfo.hasNextPage = true;

    const observation = await observeRepository({ fetchImpl: fetchForGraph(graph) });
    expect(observation.pullRequests[0]?.commentsAvailability).toBe("incomplete");
    expect(observation.implementationLinkageAvailability).toBe("complete");
    expect(normalizeRepository(observation).deliveries[0]?.authority.state).toBe("satisfied");
  });

  it("scopes truncated thread comments to lane review evidence", async () => {
    const graph = graphResponse();
    const pull = graph.data.repository.pullRequests.nodes[0];
    if (pull === undefined) throw new Error("fixture missing pull request");
    (pull.reviewThreads.nodes as unknown[]).push({
      id: "thread-1",
      isResolved: false,
      isOutdated: false,
      comments: { pageInfo: { hasNextPage: true }, nodes: [] },
    });

    const observation = await observeRepository({ fetchImpl: fetchForGraph(graph) });
    const observedPull = observation.pullRequests[0];
    expect(observedPull).toMatchObject({
      commentsAvailability: "incomplete",
      threadsAvailability: "incomplete",
    });
    expect(observation.implementationLinkageAvailability).toBe("complete");
    expect(normalizeRepository(observation).deliveries[0]?.mergeGate.state).not.toBe(
      "satisfied",
    );
  });

  it("retains usable partial data when GraphQL nulls top-level nodes", async () => {
    const graph = graphResponse(false, false, true, false, false, true);
    const issues = graph.data.repository.issues.nodes;
    const pulls = graph.data.repository.pullRequests.nodes;
    const recent = graph.data.repository.recent.nodes;
    appendNull(issues);
    appendNull(pulls);
    appendNull(recent);

    const observation = await observeRepository({ fetchImpl: fetchForGraph(graph) });
    expect(observation).toMatchObject({
      availability: "incomplete",
      issuesAvailability: "incomplete",
      pullsAvailability: "incomplete",
      implementationLinkageAvailability: "incomplete",
      recentActivityAvailability: "incomplete",
    });
    expect(observation.issues).toHaveLength(1);
    expect(observation.pullRequests).toHaveLength(1);
    expect(normalizeRepository(observation).fetchHealth).toBe("partial");
  });

  it("fails linkage and executive counts closed for a null PR node without errors", async () => {
    const graph = graphResponse();
    appendNull(graph.data.repository.pullRequests.nodes);

    const projection = normalizeRepository(
      await observeRepository({ fetchImpl: fetchForGraph(graph) }),
    );
    expect(projection.fetchHealth).toBe("partial");
    expect(projection.executive.activeCount.state).toBe("unknown");
    expect(projection.executive.readyCount.state).toBe("unknown");
  });

  it("compacts nullable Issue evidence nodes", async () => {
    const graph = graphResponse();
    const issue = graph.data.repository.issues.nodes[0];
    if (issue === undefined) throw new Error("fixture missing issue");
    appendNull(issue.labels.nodes);
    appendNull(issue.blockedBy.nodes);

    const observation = await observeRepository({ fetchImpl: fetchForGraph(graph) });
    expect(observation.issues[0]).toMatchObject({
      labelsAvailability: "incomplete",
      dependencyAvailability: "incomplete",
    });
    expect(normalizeRepository(observation).fetchHealth).toBe("partial");
  });

  it.each([
    [
      "closing reference element",
      (pull: FixturePull) => {
        appendNull(pull.closingIssuesReferences.nodes);
      },
      {},
      "incomplete",
    ],
    [
      "top-level comment element",
      (pull: FixturePull) => {
        appendNull(pull.comments.nodes);
      },
      { commentsAvailability: "incomplete" },
      "incomplete",
    ],
    [
      "review element",
      (pull: FixturePull) => {
        appendNull(pull.reviews.nodes);
      },
      { commentsAvailability: "incomplete", reviewsAvailability: "incomplete" },
      "incomplete",
    ],
    [
      "review-thread element",
      (pull: FixturePull) => {
        appendNull(pull.reviewThreads.nodes);
      },
      { commentsAvailability: "incomplete", threadsAvailability: "incomplete" },
      "complete",
    ],
    [
      "thread-comment element",
      (pull: FixturePull) => {
        const thread = {
          id: "thread-null-comment",
          isResolved: false,
          isOutdated: false,
          comments: { pageInfo: { hasNextPage: false }, nodes: [] },
        };
        (pull.reviewThreads.nodes as unknown[]).push(thread);
        appendNull(thread.comments.nodes);
      },
      { commentsAvailability: "incomplete", threadsAvailability: "incomplete" },
      "complete",
    ],
    [
      "check element",
      (pull: FixturePull) => {
        appendNull(pull.statusCheckRollup.contexts.nodes);
      },
      { checksAvailability: "incomplete" },
      "complete",
    ],
    [
      "null comment node list",
      (pull: FixturePull) => {
        Object.assign(pull.comments, { nodes: null });
      },
      { commentsAvailability: "incomplete" },
      "incomplete",
    ],
  ] as const)(
    "isolates nullable nested %s",
    async (_case, mutate, expectedPull, expectedLinkage) => {
      const graph = graphResponse();
      const pull = graph.data.repository.pullRequests.nodes[0];
      if (pull === undefined) throw new Error("fixture missing pull request");
      mutate(pull);

      const observation = await observeRepository({ fetchImpl: fetchForGraph(graph) });
      expect(observation.pullsAvailability).toBe("incomplete");
      expect(observation.pullRequests[0]).toMatchObject(expectedPull);
      expect(observation.implementationLinkageAvailability).toBe(expectedLinkage);
    },
  );

  it.each([
    [
      "Issue labels",
      (graph: FixtureGraph) => {
        const issue = graph.data.repository.issues.nodes[0];
        if (issue === undefined) throw new Error("fixture missing issue");
        Object.assign(issue, { labels: null });
      },
      { issue: { labelsAvailability: "incomplete" }, pull: {}, linkage: "complete" },
    ],
    [
      "closing Issue references",
      (graph: FixtureGraph) => {
        const pull = graph.data.repository.pullRequests.nodes[0];
        if (pull === undefined) throw new Error("fixture missing pull request");
        Object.assign(pull, { closingIssuesReferences: null });
      },
      { issue: {}, pull: { closingIssueNumbers: [] }, linkage: "incomplete" },
    ],
    [
      "reviews",
      (graph: FixtureGraph) => {
        const pull = graph.data.repository.pullRequests.nodes[0];
        if (pull === undefined) throw new Error("fixture missing pull request");
        Object.assign(pull, { reviews: null });
      },
      {
        issue: {},
        pull: { commentsAvailability: "incomplete", reviewsAvailability: "incomplete" },
        linkage: "incomplete",
      },
    ],
  ] as const)("fails a nullable %s connection closed", async (_case, mutate, expected) => {
    const graph = graphResponse();
    mutate(graph);

    const observation = await observeRepository({ fetchImpl: fetchForGraph(graph) });
    expect(observation.availability).toBe("incomplete");
    expect(observation.issues[0]).toMatchObject(expected.issue);
    expect(observation.pullRequests[0]).toMatchObject(expected.pull);
    expect(observation.implementationLinkageAvailability).toBe(expected.linkage);
  });

  it.each([
    ["non-Blob", {}],
    ["nullable Blob text", { oid: "roadmap-oid", text: null }],
    ["empty Blob text", { oid: "roadmap-oid", text: "" }],
  ] as const)("keeps a %s Roadmap object Unknown", async (_case, roadmap) => {
    const graph = graphResponse();
    Object.assign(graph.data.repository, { roadmap });

    const observation = await observeRepository({ fetchImpl: fetchForGraph(graph) });
    expect(observation.roadmap).toBeNull();
    expect(observation.availability).toBe("incomplete");
    expect(normalizeRepository(observation).executive.productHorizon.state).toBe("unknown");
  });

  it("keeps concurrent comparison errors in pull-request order", async () => {
    const observation = await observeRepository({ fetchImpl: fakeComparisonFailureFetch() });

    expect(
      observation.errors
        .filter((error) => error.source.endsWith("authority comparison"))
        .map((error) => error.source),
    ).toEqual(["PR #225 authority comparison", "PR #226 authority comparison"]);
  });

  it.each([
    ["top-level pull pagination", true, false],
    ["closing-Issue pagination", false, true],
  ])("keeps merge state Unknown for incomplete %s", async (_name, pullPage, closingPage) => {
    const fake = fakeFetch(false, false, true, pullPage, closingPage);
    const observation = await observeRepository({ fetchImpl: fake.implementation });
    const projection = normalizeRepository(observation);

    expect(observation.implementationLinkageAvailability).toBe("incomplete");
    expect(observation.pullRequests[0]?.closingIssueNumbers).toEqual([169]);
    expect(projection.deliveries[0]?.mergeGate.state).not.toBe("satisfied");
    expect(projection.executive.activeCount.state).toBe("unknown");
    expect(projection.executive.readyCount.state).toBe("unknown");
  });

  it("fails closed when Accepted ADR or Principle authority changed after the merge base", async () => {
    const fake = fakeBehindAuthorityFetch();
    const observation = await observeRepository({ fetchImpl: fake.implementation });
    const pull = observation.pullRequests[0];

    expect(fake.requests).toHaveLength(3);
    expect(pull?.authorityAvailability).toBe("complete");
    expect(pull?.authorityChanges.map((change) => change.path)).toEqual([
      "docs/architecture/document-model.md",
      "docs/decisions/ADR-0032-dashboard.md",
      "docs/security/threat-model.md",
      "docs/vision/product-constitution.md",
      "docs/product/engine-integration-strategy.md",
      ".github/workflows/ci.yml",
      "scripts/release-check.sh",
      "Cargo.toml",
      "Cargo.lock",
      "crates/cli/Cargo.toml",
      "apps/designer/runtime/Cargo.lock",
      "crates/cli/AGENTS.md",
      "docs/decisions/ADR-renamed.md",
      ".github/workflows/renamed.yml",
      "docs/governance/renamed-in.md",
      "crates/storage/Cargo.toml",
    ]);
    expect(normalizeRepository(observation).deliveries[0]?.authority.state).toBe("unknown");
  });
});
