import type { RepositoryObservation } from "../shared/model.js";

const MAIN_SHA = "1111111111111111111111111111111111111111";
const HEAD_SHA = "2222222222222222222222222222222222222222";

function sourceComment(body: string, id: string) {
  return {
    body,
    id,
    kind: "issue-comment" as const,
    authorLogin: "nurockplayer",
    authorAssociation: "OWNER" as const,
    url: `https://github.example/comments/${id}`,
    createdAt: "2026-09-01T00:00:00.000Z",
    updatedAt: null,
    edited: false,
    topLevel: true,
    trustedProducer: true,
  };
}

export function healthyObservation(): RepositoryObservation {
  return {
    repository: "nurockplayer/tachiko-work",
    ownerToken: "agent:codex",
    observedAt: "2026-09-01T03:00:00.000Z",
    availability: "complete",
    main: {
      sha: MAIN_SHA,
      url: `https://github.example/commit/${MAIN_SHA}`,
    },
    roadmap: {
      markdown: [
        "# Tachiko Work Product Roadmap",
        "",
        "## Current horizon",
        "",
        "The current repository planning horizon is:",
        "",
        "> **06 · Team Workspace Beta**",
        "",
        "## Product stages",
      ].join("\n"),
      url: "https://github.example/roadmap",
    },
    issues: [
      {
        number: 169,
        title: "Build read-only project control room",
        url: "https://github.example/issues/169",
        state: "OPEN",
        labels: ["agent:codex", "state:ready"],
        labelsAvailability: "complete",
        milestone: null,
        blockedBy: [{ number: 200, state: "CLOSED", url: "https://github.example/issues/200" }],
        dependencyAvailability: "complete",
      },
      {
        number: 223,
        title: "Independent production lane",
        url: "https://github.example/issues/223",
        state: "OPEN",
        labels: ["agent:codex", "state:ready"],
        labelsAvailability: "complete",
        milestone: "06 · Team Workspace Beta",
        blockedBy: [],
        dependencyAvailability: "complete",
      },
    ],
    issuesAvailability: "complete",
    pullRequests: [
      {
        number: 225,
        title: "tooling: add project dashboard",
        url: "https://github.example/pulls/225",
        state: "OPEN",
        draft: false,
        headSha: HEAD_SHA,
        baseSha: MAIN_SHA,
        baseRef: "main",
        mergeBaseSha: MAIN_SHA,
        relationToMain: "current",
        authorityChanges: [],
        authorityAvailability: "complete",
        closingIssueNumbers: [169],
        comments: [
          sourceComment(
            [
              "<!-- agent-handoff:v1 -->",
              "ISSUE: 169",
              "PR: 225",
              "OWNER: agent:codex",
              "STATE: active",
              `HEAD: ${HEAD_SHA}`,
              `MAIN: ${MAIN_SHA}`,
              "",
              "Narrative says merge-ready, passed, and visually approved.",
            ].join("\n"),
            "handoff-1",
          ),
          sourceComment(
            [
              "<!-- project-steward-watch:v1 -->",
              "VERDICT: GREEN",
              `HEAD: ${HEAD_SHA}`,
              `MAIN: ${MAIN_SHA}`,
              "HUMAN_ACTION: none",
            ].join("\n"),
            "watch-1",
          ),
        ],
        commentsAvailability: "complete",
        checks: [
          {
            name: "Live Project Dashboard browser journey",
            headSha: HEAD_SHA,
            status: "success",
            url: "https://github.example/checks/browser",
          },
          {
            name: "build",
            headSha: HEAD_SHA,
            status: "success",
            url: "https://github.example/checks/build",
          },
        ],
        checksAvailability: "complete",
        reviews: [],
        reviewsAvailability: "complete",
        threads: [],
        threadsAvailability: "complete",
      },
    ],
    pullsAvailability: "complete",
    recentActivity: [
      {
        number: 207,
        title: "tooling: add strict operational evidence",
        url: "https://github.example/pulls/207",
        mergedAt: "2026-08-31T12:00:00.000Z",
        mergeSha: "3333333333333333333333333333333333333333",
      },
    ],
    recentActivityAvailability: "complete",
    errors: [],
  };
}

export function partialObservation(): RepositoryObservation {
  return {
    repository: "nurockplayer/tachiko-work",
    ownerToken: "agent:codex",
    observedAt: "2026-09-01T03:00:00.000Z",
    availability: "incomplete",
    main: null,
    roadmap: null,
    issues: [],
    issuesAvailability: "unavailable",
    pullRequests: [],
    pullsAvailability: "unavailable",
    recentActivity: [],
    recentActivityAvailability: "unavailable",
    errors: [
      {
        source: "GitHub GraphQL",
        url: "https://github.example/api",
        reason: "observation-unavailable",
      },
    ],
  };
}
