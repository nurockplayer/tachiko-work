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
                            checkSuite: { app: { databaseId: 42 } },
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
        checks: [{ name: "test", integrationId: 42, status: "completed", conclusion: "success" }],
      }],
      recentCompletions: [{ number: 186 }],
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
