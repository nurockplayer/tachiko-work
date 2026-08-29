import { describe, expect, it } from "vitest";

import { loadGithubSnapshot, type ReadonlyGithubApi } from "../src/server/github.ts";

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
            milestone: null,
            blockedBy: { nodes: [], pageInfo: { hasNextPage: false } },
            comments: { nodes: [], pageInfo: { hasPreviousPage: false } },
          },
        ],
        pageInfo: { hasNextPage: false, endCursor: null },
      },
      pullRequests: {
        nodes: [
          {
            number: 200,
            title: "Dashboard v0",
            url: "https://github.com/nurockplayer/tachiko-work/pull/200",
            body: "Closes #169",
            headRefOid: headSha,
            baseRefOid: mainSha,
            baseRefName: "main",
            updatedAt: "2026-08-30T00:00:00Z",
            closingIssuesReferences: { nodes: [{ number: 169 }] },
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
                            app: { databaseId: 42 },
                          },
                        ],
                      },
                    },
                  },
                },
              ],
            },
            reviewDecision: "REVIEW_REQUIRED",
            reviews: { nodes: [] },
            reviewThreads: { nodes: [] },
          },
        ],
        pageInfo: { hasNextPage: false, endCursor: null },
      },
      mergedPullRequests: {
        nodes: [
          {
            number: 186,
            title: "Ship Designer slice",
            url: "https://github.com/nurockplayer/tachiko-work/pull/186",
            mergedAt: "2026-08-29T19:00:00Z",
            mergeCommit: { oid: "c".repeat(40) },
            author: { login: "nurockplayer" },
          },
        ],
        pageInfo: { hasNextPage: false, endCursor: null },
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
      productHorizon: "05 · Designer MVP",
      issues: [{ number: 169, blockedBy: [] }],
      pullRequests: [{
        number: 200,
        headSha,
        baseSha: mainSha,
        mergeBaseSha: mainSha,
        relationToMain: "current",
        authorityPathsChangedOnMain: [],
        requiredChecks: [{ name: "test", integrationId: 42 }],
        checksObservedHeadSha: headSha,
      }],
      recentCompletions: [{ number: 186 }],
    });
    expect(result.productHorizonUrl).toContain(mainSha);
  });

  it("preserves the PR base tip and observes changed authority paths from merge-base to live main", async () => {
    const oldBase = "d".repeat(40);
    const page = githubPage();
    page.repository.pullRequests.nodes[0]!.baseRefOid = mainSha;
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
      baseSha: mainSha,
      mergeBaseSha: oldBase,
      relationToMain: "diverged",
      authorityPathsChangedOnMain: ["docs/decisions/ADR-0029-dashboard-boundary.md"],
    });
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
