import type { DashboardProjection, SourceLink } from "../shared/model.js";

const MAIN_SHA = "1111111111111111111111111111111111111111";
const HEAD_SHA = "2222222222222222222222222222222222222222";

const repositorySource: SourceLink = {
  label: "GitHub repository",
  url: "https://github.example/tachiko-work",
  kind: "github",
};

export function healthyProjection(): DashboardProjection {
  const issue = {
    number: 229,
    title: "Rebuild the control room as an observational projection",
    url: "https://github.example/issues/229",
    state: "OPEN",
    labels: ["agent:codex", "state:ready"],
    milestone: null,
    blockedBy: [{ number: 200, state: "CLOSED", url: "https://github.example/issues/200" }],
    dependenciesAvailability: "complete" as const,
    availability: "complete" as const,
  };
  const pullRequest = {
    number: 230,
    title: "tooling: add observational dashboard",
    url: "https://github.example/pulls/230",
    state: "OPEN",
    draft: false,
    headSha: HEAD_SHA,
    baseSha: MAIN_SHA,
    baseRef: "main",
    mergeable: "MERGEABLE",
    mergeStateStatus: "CLEAN",
    reviewDecision: null,
    linkedIssueNumbers: [229],
    checks: {
      availability: "complete" as const,
      items: [{
        name: "Live Project Dashboard browser journey",
        status: "COMPLETED",
        conclusion: "SUCCESS",
        url: "https://github.example/checks/1",
        headSha: HEAD_SHA,
      }],
    },
    reviews: {
      availability: "complete" as const,
      items: [{
        author: "reviewer",
        state: "COMMENTED",
        commitSha: HEAD_SHA,
        exactHead: true,
        url: "https://github.example/reviews/1",
      }],
    },
    handoff: {
      status: "current" as const,
      value: "active · agent:codex",
      reason: "Exact head and live main match",
      source: { label: "Agent handoff", url: "https://github.example/comments/1", kind: "structured" as const },
    },
    stewardWatch: {
      status: "current" as const,
      value: "GREEN · human action none",
      reason: "Exact head and live main match",
      source: { label: "Steward watch", url: "https://github.example/comments/2", kind: "structured" as const },
    },
    availability: "complete" as const,
  };
  return {
    repository: "nurockplayer/tachiko-work",
    observedAt: "2026-09-02T00:00:00.000Z",
    fetchHealth: "healthy",
    executive: {
      mainSha: { value: MAIN_SHA, availability: "complete", source: repositorySource },
      productHorizon: {
        value: "06 · Team Workspace Beta",
        availability: "complete",
        source: { label: "Product Roadmap", url: "https://github.example/roadmap", kind: "repository" },
      },
      activeCount: { value: 1, availability: "complete", source: repositorySource },
      readyCount: { value: 1, availability: "complete", source: repositorySource },
      humanAction: {
        value: "None in current watches",
        availability: "complete",
        source: { label: "Steward watch", url: "https://github.example/comments/2", kind: "structured" },
      },
    },
    deliveries: [{ issue, pullRequest, linkageAvailability: "complete" }],
    criticalPath: {
      availability: "complete",
      nodes: [
        { issueNumber: 200, label: "#200 · CLOSED", state: "CLOSED", url: "https://github.example/issues/200" },
        { issueNumber: 229, label: "#229 · OPEN", state: "OPEN", url: issue.url },
      ],
      edges: [{ from: 229, to: 200, state: "CLOSED" }],
      source: repositorySource,
    },
    recentActivity: {
      availability: "complete",
      items: [{
        number: 227,
        title: "define semantic execution taxonomy",
        url: "https://github.example/pulls/227",
        mergedAt: "2026-09-01T12:00:00.000Z",
        mergeSha: "3333333333333333333333333333333333333333",
      }],
      source: repositorySource,
    },
    attention: [{
      level: "info",
      label: "Observed native policy facts",
      detail: "GitHub values are shown directly; the Dashboard does not compute a final merge verdict.",
      sources: [repositorySource],
    }],
    sources: [repositorySource],
  };
}

export function partialProjection(): DashboardProjection {
  const projection = healthyProjection();
  return {
    ...projection,
    fetchHealth: "partial",
    executive: {
      ...projection.executive,
      mainSha: { ...projection.executive.mainSha, value: null, availability: "partial" },
      humanAction: { ...projection.executive.humanAction, value: null, availability: "partial" },
    },
    deliveries: projection.deliveries.map((lane) => ({
      ...lane,
      linkageAvailability: "partial",
      issue: lane.issue === null ? null : {
        ...lane.issue,
        dependenciesAvailability: "partial",
      },
      pullRequest: lane.pullRequest === null ? null : {
        ...lane.pullRequest,
        mergeable: null,
        availability: "partial",
        handoff: { status: "unknown", value: null, reason: "Observation incomplete", source: null },
      },
    })),
    attention: [{
      level: "unknown",
      label: "GitHub observation partial",
      detail: "Some fields are Unknown; displayed values are not a completeness claim.",
      sources: projection.sources,
    }],
  };
}
