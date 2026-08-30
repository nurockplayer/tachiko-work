import { describe, expect, it, vi } from "vitest";

import { GithubApiClient, loadGithubSnapshot, type ReadonlyGithubApi } from "../src/server/github.ts";

const mainSha = "a".repeat(40);
const headSha = "b".repeat(40);

function githubPage() {
  return {
    repository: {
      url: "https://github.com/nurockplayer/tachiko-work",
      defaultBranchRef: { name: "main", target: { oid: mainSha } },
      issues: {
        nodes: [
          {
            number: 169,
            title: "Dashboard v0",
            url: "https://github.com/nurockplayer/tachiko-work/issues/169",
            body: "**READY FOR BOUNDED IMPLEMENTATION**\nOwner: agent:codex",
            updatedAt: "2026-08-30T00:00:00Z",
            lastEditedAt: null,
            milestone: null,
            blockedBy: {
              nodes: [
                {
                  number: 170,
                  title: "Open dependency",
                  url: "https://github.com/nurockplayer/tachiko-work/issues/170",
                  state: "OPEN",
                },
                {
                  number: 171,
                  title: "Closed dependency",
                  url: "https://github.com/nurockplayer/tachiko-work/issues/171",
                  state: "CLOSED",
                },
              ],
              pageInfo: { hasNextPage: false },
            },
            comments: { nodes: [], pageInfo: { hasPreviousPage: false } },
          },
        ],
        pageInfo: { hasNextPage: false, endCursor: null as string | null },
      },
      pullRequests: {
        nodes: [
          {
            number: 200,
            title: "Dashboard v0",
            url: "https://github.com/nurockplayer/tachiko-work/pull/200",
            body: "Closes #169",
            isDraft: false,
            headRefOid: headSha,
            baseRefOid: mainSha,
            baseRefName: "main",
            mergeable: "MERGEABLE",
            mergeStateStatus: "CLEAN",
            updatedAt: "2026-08-30T00:00:00Z",
            closingIssuesReferences: { nodes: [{ number: 169 }], pageInfo: { hasNextPage: false } },
            comments: { nodes: [], pageInfo: { hasPreviousPage: false } },
            commits: {
              nodes: [
                {
                  commit: {
                    oid: headSha,
                    statusCheckRollup: {
                      contexts: {
                        nodes: [
                          {
                            __typename: "CheckRun",
                            name: "test",
                            status: "COMPLETED",
                            conclusion: "SUCCESS",
                            detailsUrl: null,
                            checkSuite: { app: { databaseId: 42 } },
                          },
                        ],
                        pageInfo: { hasNextPage: false },
                      },
                    },
                  },
                },
              ],
            },
            reviewDecision: "REVIEW_REQUIRED",
            reviews: { nodes: [], pageInfo: { hasNextPage: false } },
            reviewThreads: {
              nodes: [{
                isResolved: false,
                isOutdated: false,
                comments: {
                  nodes: [
                    { body: "[P3] Initial suggestion", url: "https://github.com/thread#comment-1" },
                    { body: "[P1] Follow-up correctness finding", url: "https://github.com/thread#comment-2" },
                  ],
                  pageInfo: { hasNextPage: false },
                },
              }],
              pageInfo: { hasNextPage: false },
            },
          },
        ],
        pageInfo: { hasNextPage: false, endCursor: null as string | null },
      },
      mergedPullRequests: {
        nodes: [
          {
            number: 186,
            title: "Ship Designer slice",
            url: "https://github.com/nurockplayer/tachiko-work/pull/186",
            mergedAt: "2026-08-29T19:00:00Z",
            mergeCommit: { oid: "c".repeat(40) },
            mergedBy: { login: "maintainer" },
          },
        ],
        pageInfo: { hasNextPage: false, endCursor: null as string | null },
      },
    },
  };
}

describe("loadGithubSnapshot", () => {
  it("binds roadmap authority and checks to the exact observed main/head identities", async () => {
    const api: ReadonlyGithubApi = {
      graphql: async () => githubPage(),
      rawText: async () => "## Current horizon\n\n> **05 · Designer MVP**",
      requiredStatusChecks: async () => [{ name: "test", integrationId: 42 }],
      compare: async () => ({ status: "ahead", mergeBaseSha: mainSha, files: [] }),
    };

    const result = await loadGithubSnapshot(api, {
      owner: "nurockplayer",
      repo: "tachiko-work",
      observedAt: "2026-08-30T00:00:00.000Z",
    });

    expect(result).toMatchObject({
      fetchHealth: "healthy",
      mainSha,
      defaultBranchName: "main",
      productHorizon: "05 · Designer MVP",
      issues: [{
        number: 169,
        blockedBy: [{
          number: 170,
          title: "Open dependency",
          url: "https://github.com/nurockplayer/tachiko-work/issues/170",
        }],
        commentsComplete: true,
      }],
      pullRequests: [{
        number: 200,
        isDraft: false,
        headSha,
        baseSha: mainSha,
        mergeBaseSha: mainSha,
        relationToMain: "current",
        authorityPathsChangedOnMain: [],
        mergeable: "mergeable",
        mergeStateStatus: "clean",
        issueNumbersComplete: true,
        commentsComplete: true,
        requiredChecks: [{ name: "test", integrationId: 42 }],
        checksObservedHeadSha: headSha,
        checks: [{ name: "test", integrationId: 42, status: "completed", conclusion: "success" }],
        reviewThreads: [{
          resolved: false,
          outdated: false,
          comments: ["[P3] Initial suggestion", "[P1] Follow-up correctness finding"],
          url: "https://github.com/thread#comment-1",
        }],
      }],
      recentCompletions: [{ number: 186, mergedBy: "maintainer" }],
    });
    expect(result.productHorizonUrl).toContain(mainSha);
  });

  it("preserves the PR base tip and observes changed authority paths from merge-base to live main", async () => {
    const oldBase = "d".repeat(40);
    const page = githubPage();
    page.repository.pullRequests.nodes[0]!.baseRefOid = oldBase;
    const comparisons: string[] = [];
    const api: ReadonlyGithubApi = {
      graphql: async () => page,
      rawText: async () => "## Current horizon\n\n> **05 · Designer MVP**",
      requiredStatusChecks: async () => [],
      compare: async (_owner, _repo, base, head) => {
        comparisons.push(`${base}...${head}`);
        return base === mainSha
          ? { status: "diverged", mergeBaseSha: oldBase, files: [] }
          : {
              status: "ahead",
              mergeBaseSha: oldBase,
              files: ["src/unrelated.rs", "docs/decisions/ADR-0029-dashboard-boundary.md"],
            };
      },
    };

    const result = await loadGithubSnapshot(api, {
      owner: "nurockplayer",
      repo: "tachiko-work",
      observedAt: "2026-08-30T00:00:00.000Z",
    });

    expect(comparisons).toEqual([`${mainSha}...${headSha}`, `${oldBase}...${mainSha}`]);
    expect(result.pullRequests?.[0]).toMatchObject({
      baseSha: oldBase,
      mergeBaseSha: oldBase,
      relationToMain: "diverged",
      authorityPathsChangedOnMain: ["docs/decisions/ADR-0029-dashboard-boundary.md"],
    });
  });

  it("keeps draft state and truncated exact-head checks explicit", async () => {
    const page = githubPage();
    page.repository.pullRequests.nodes[0]!.isDraft = true;
    page.repository.pullRequests.nodes[0]!.commits.nodes[0]!.commit.statusCheckRollup.contexts.pageInfo.hasNextPage = true;
    const api: ReadonlyGithubApi = {
      graphql: async () => page,
      rawText: async () => "## Current horizon\n\n> **05 · Designer MVP**",
      requiredStatusChecks: async () => [],
      compare: async () => ({ status: "ahead", mergeBaseSha: mainSha, files: [] }),
    };

    const result = await loadGithubSnapshot(api, {
      owner: "nurockplayer",
      repo: "tachiko-work",
      observedAt: "2026-08-30T00:00:00.000Z",
    });

    expect(result.fetchHealth).toBe("partial");
    expect(result.pullRequests?.[0]).toMatchObject({ isDraft: true, checks: null });
    expect(result.failures).toContain("PR #200 exact-head check observation was truncated.");
  });

  it("fails closed when closing-Issue ownership is truncated", async () => {
    const page = githubPage();
    page.repository.pullRequests.nodes[0]!.closingIssuesReferences.pageInfo.hasNextPage = true;
    const api: ReadonlyGithubApi = {
      graphql: async () => page,
      rawText: async () => "## Current horizon\n\n> **05 · Designer MVP**",
      requiredStatusChecks: async () => [],
      compare: async () => ({ status: "ahead", mergeBaseSha: mainSha, files: [] }),
    };

    const result = await loadGithubSnapshot(api, {
      owner: "nurockplayer",
      repo: "tachiko-work",
      observedAt: "2026-08-30T00:00:00.000Z",
    });

    expect(result.fetchHealth).toBe("partial");
    expect(result.pullRequests?.[0]).toMatchObject({
      issueNumbers: [169],
      issueNumbersComplete: false,
    });
    expect(result.failures).toContain("PR #200 closing-Issue observation was truncated.");
  });

  it("bounds the recent-completion window and sorts its sample by merge time", async () => {
    let recentQuery = "";
    let recentCalls = 0;
    const api: ReadonlyGithubApi = {
      graphql: async (query) => {
        if (!query.includes("RecentCompletions")) return githubPage();
        recentCalls += 1;
        recentQuery = query;
        return {
          repository: {
            mergedPullRequests: {
              nodes: Array.from({ length: 9 }, (_, index) => {
                const number = 100 + index;
                return {
                  number,
                  title: `Merge ${number}`,
                  url: `https://github.com/nurockplayer/tachiko-work/pull/${number}`,
                  mergedAt: `2026-08-30T00:0${index}:00Z`,
                  mergeCommit: { oid: index.toString(16).repeat(40) },
                  mergedBy: { login: "maintainer" },
                };
              }),
            },
          },
        };
      },
      rawText: async () => "## Current horizon\n\n> **05 · Designer MVP**",
      requiredStatusChecks: async () => [],
      compare: async () => ({ status: "ahead", mergeBaseSha: mainSha, files: [] }),
    };

    const result = await loadGithubSnapshot(api, {
      owner: "nurockplayer",
      repo: "tachiko-work",
      observedAt: "2026-08-30T00:00:00.000Z",
    });

    expect(recentCalls).toBe(1);
    expect(recentQuery).toMatch(/pullRequests\(\s*first:\s*100/);
    expect(recentQuery).toMatch(/states:\s*MERGED/);
    expect(recentQuery).toMatch(/orderBy:\s*\{\s*field:\s*UPDATED_AT,\s*direction:\s*DESC\s*\}/);
    expect(result.recentCompletions?.map((completion) => completion.number)).toEqual([108, 107, 106, 105, 104, 103, 102, 101]);
  });

  it("stops querying a repository connection after that connection is exhausted", async () => {
    const dashboardVariables: Array<Record<string, string | boolean | null>> = [];
    const api: ReadonlyGithubApi = {
      graphql: async (query, variables) => {
        if (query.includes("RecentCompletions")) return githubPage();
        dashboardVariables.push(variables);
        const page = githubPage();
        const secondPage = dashboardVariables.length === 2;
        page.repository.issues.pageInfo = { hasNextPage: false, endCursor: null };
        page.repository.pullRequests.pageInfo = {
          hasNextPage: !secondPage,
          endCursor: secondPage ? null : "pr-page-2",
        };
        if (secondPage) page.repository.pullRequests.nodes = [];
        return page;
      },
      rawText: async () => "## Current horizon\n\n> **05 · Designer MVP**",
      requiredStatusChecks: async () => [],
      compare: async () => ({ status: "ahead", mergeBaseSha: mainSha, files: [] }),
    };

    await loadGithubSnapshot(api, {
      owner: "nurockplayer",
      repo: "tachiko-work",
      observedAt: "2026-08-30T00:00:00.000Z",
    });

    expect(dashboardVariables).toHaveLength(2);
    expect(dashboardVariables[0]).toMatchObject({ includeIssues: true, includePullRequests: true });
    expect(dashboardVariables[1]).toMatchObject({
      includeIssues: false,
      includePullRequests: true,
      issueCursor: null,
      prCursor: "pr-page-2",
    });
  });

  it("preserves incomplete handoff and review observation with exact merger attribution", async () => {
    const page = githubPage();
    page.repository.issues.nodes[0]!.comments.pageInfo.hasPreviousPage = true;
    page.repository.pullRequests.nodes[0]!.comments.pageInfo.hasPreviousPage = true;
    page.repository.pullRequests.nodes[0]!.reviews.pageInfo.hasNextPage = true;
    page.repository.pullRequests.nodes[0]!.reviewThreads.nodes[0]!.comments.pageInfo.hasNextPage = true;
    page.repository.mergedPullRequests.nodes[0]!.mergedBy = { login: "release-maintainer" };
    const api: ReadonlyGithubApi = {
      graphql: async () => page,
      rawText: async () => "## Current horizon\n\n> **05 · Designer MVP**",
      requiredStatusChecks: async () => [],
      compare: async () => ({ status: "ahead", mergeBaseSha: mainSha, files: [] }),
    };

    const result = await loadGithubSnapshot(api, {
      owner: "nurockplayer",
      repo: "tachiko-work",
      observedAt: "2026-08-30T00:00:00.000Z",
    });

    expect(result.issues?.[0]?.commentsComplete).toBe(false);
    expect(result.pullRequests?.[0]?.commentsComplete).toBe(false);
    expect(result.pullRequests?.[0]?.reviews).toBeNull();
    expect(result.pullRequests?.[0]?.reviewThreads).toBeNull();
    expect(result.recentCompletions?.[0]?.mergedBy).toBe("release-maintainer");
    expect(result.failures).toEqual(expect.arrayContaining([
      "Issue #169 handoff observation was truncated.",
      "PR #200 handoff observation was truncated.",
      "PR #200 review observation was truncated.",
      "PR #200 review-thread observation was truncated.",
    ]));
  });

  it("keeps authority drift unknown when GitHub caps a compare at 300 files", async () => {
    const oldBase = "d".repeat(40);
    const api: ReadonlyGithubApi = {
      graphql: async () => githubPage(),
      rawText: async () => "## Current horizon\n\n> **05 · Designer MVP**",
      requiredStatusChecks: async () => [],
      compare: async (_owner, _repo, base) => base === mainSha
        ? { status: "diverged", mergeBaseSha: oldBase, files: [] }
        : { status: "ahead", mergeBaseSha: oldBase, files: Array.from({ length: 300 }, (_, index) => `src/file-${index}.rs`) },
    };

    const result = await loadGithubSnapshot(api, {
      owner: "nurockplayer",
      repo: "tachiko-work",
      observedAt: "2026-08-30T00:00:00.000Z",
    });

    expect(result.fetchHealth).toBe("partial");
    expect(result.pullRequests?.[0]?.authorityPathsChangedOnMain).toBeNull();
    expect(result.failures).toContain("PR #200 authority-change observation failed.");
  });

  it("combines required checks from active rulesets and classic branch protection", async () => {
    const fetchImplementation = vi.fn(async (input: string | URL | Request) => {
      const url = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
      if (url.includes("/rules/branches/main")) {
        return new Response(JSON.stringify([
          {
            type: "required_status_checks",
            parameters: { required_status_checks: [{ context: "ruleset-check", integration_id: 42 }] },
          },
        ]), { status: 200 });
      }
      if (url.includes("/branches/main/protection/required_status_checks")) {
        return new Response(JSON.stringify({
          checks: [{ context: "classic-check", app_id: 7 }],
          contexts: ["legacy-context"],
        }), { status: 200 });
      }
      throw new Error(`Unexpected request: ${url}`);
    });
    const client = new GithubApiClient("test-token", fetchImplementation);

    await expect(client.requiredStatusChecks("nurockplayer", "tachiko-work", "main")).resolves.toEqual([
      { name: "classic-check", integrationId: 7 },
      { name: "legacy-context", integrationId: null },
      { name: "ruleset-check", integrationId: 42 },
    ]);
    expect(fetchImplementation).toHaveBeenCalledTimes(2);
  });

  it("paginates active rules before declaring the required-check set complete", async () => {
    const fetchImplementation = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(typeof input === "string" ? input : input instanceof URL ? input.href : input.url);
      if (url.pathname.endsWith("/rules/branches/main")) {
        const page = url.searchParams.get("page");
        const count = page === "1" ? 100 : 1;
        return new Response(JSON.stringify(Array.from({ length: count }, (_, index) => ({
          type: "required_status_checks",
          parameters: {
            required_status_checks: [{ context: `ruleset-${page}-${index}`, integration_id: 42 }],
          },
        }))), { status: 200 });
      }
      if (url.pathname.endsWith("/branches/main/protection/required_status_checks")) {
        return new Response("{}", { status: 404 });
      }
      throw new Error(`Unexpected request: ${url.href}`);
    });
    const client = new GithubApiClient("test-token", fetchImplementation);

    const checks = await client.requiredStatusChecks("nurockplayer", "tachiko-work", "main");

    expect(checks).toHaveLength(101);
    expect(checks).toContainEqual({ name: "ruleset-2-0", integrationId: 42 });
    expect(fetchImplementation).toHaveBeenCalledTimes(3);
  });

  it("treats classic branch-protection app_id -1 as any app", async () => {
    const fetchImplementation = vi.fn(async (input: string | URL | Request) => {
      const url = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
      if (url.includes("/rules/branches/main")) return new Response("[]", { status: 200 });
      if (url.includes("/branches/main/protection/required_status_checks")) {
        return new Response(JSON.stringify({ checks: [{ context: "build", app_id: -1 }] }), { status: 200 });
      }
      throw new Error(`Unexpected request: ${url}`);
    });
    const client = new GithubApiClient("test-token", fetchImplementation);

    await expect(client.requiredStatusChecks("nurockplayer", "tachiko-work", "main")).resolves.toEqual([
      { name: "build", integrationId: null },
    ]);
  });

  it("degrades a failed compare observation to partial/unknown rather than healthy", async () => {
    const api: ReadonlyGithubApi = {
      graphql: async () => githubPage(),
      rawText: async () => "## Current horizon\n\n> **05 · Designer MVP**",
      requiredStatusChecks: async () => [],
      compare: async () => {
        throw new Error("rate limited");
      },
    };

    const result = await loadGithubSnapshot(api, {
      owner: "nurockplayer",
      repo: "tachiko-work",
      observedAt: "2026-08-30T00:00:00.000Z",
    });

    expect(result.fetchHealth).toBe("partial");
    expect(result.pullRequests?.[0]?.relationToMain).toBe("unknown");
    expect(result.failures).toContain("PR #200 relation-to-main observation failed.");
  });
});
