import { describe, expect, it } from "vitest";

import { healthyObservation, partialObservation } from "../src/server/fixtures.js";
import { normalizeRepository } from "../src/server/normalize.js";

const HEAD = "2222222222222222222222222222222222222222";

function addGreenOperationalEvidence(observation: ReturnType<typeof healthyObservation>): void {
  const pull = observation.pullRequests[0];
  if (pull === undefined) throw new Error("fixture missing pull request");
  const comment = (id: string, body: string) => ({
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
  });
  pull.comments.push(
    comment(
      "release-evidence",
      [
        "<!-- operational-evidence:v1",
        "KIND: validation",
        "PR: 225",
        `HEAD: ${HEAD}`,
        "RUN: release-225",
        "NAME: release-check",
        "RESULT: pass",
        "-->",
      ].join("\n"),
    ),
    comment(
      "review-evidence",
      [
        "<!-- operational-evidence:v1",
        "KIND: review",
        "PR: 225",
        `HEAD: ${HEAD}`,
        "RUN: review-225",
        "NAME: project-review",
        "RESULT: clean",
        "-->",
      ].join("\n"),
    ),
  );
}

describe("normalizeRepository", () => {
  it("keeps independent lanes and exact evidence classes separate", () => {
    const projection = normalizeRepository(healthyObservation());

    expect(projection.fetchHealth).toBe("healthy");
    expect(projection.deliveries.map((lane) => lane.issue?.number ?? null)).toEqual([
      169, 223,
    ]);
    expect(projection.deliveries[0]?.pullRequest?.headSha).toBe(
      "2222222222222222222222222222222222222222",
    );
    expect(projection.deliveries[0]?.checks.state).toBe("satisfied");
    expect(projection.deliveries[0]?.review.state).toBe("unknown");
    expect(projection.deliveries[0]?.evidence.automatedBrowser.state).toBe("satisfied");
    expect(projection.deliveries[0]?.evidence.perceptualReview.state).toBe("unknown");
    expect(projection.deliveries[0]?.mergeGate.state).not.toBe("satisfied");
    expect(projection.deliveries[0]?.phase).toBe("rereview");
  });

  it("makes a moving-head handoff stale instead of trusting narrative claims", () => {
    const observation = healthyObservation();
    const pull = observation.pullRequests[0];
    if (pull === undefined) throw new Error("fixture missing pull request");
    pull.comments[0] = {
      ...pull.comments[0]!,
      body: pull.comments[0]!.body.replace(
        "2222222222222222222222222222222222222222",
        "9999999999999999999999999999999999999999",
      ),
    };

    const lane = normalizeRepository(observation).deliveries[0];
    expect(lane?.handoff.state).toBe("unknown");
    expect(lane?.handoff.reason).toBe("head-mismatch");
    expect(lane?.mergeGate.state).toBe("unknown");
  });

  it("degrades unavailable sources to explicit partial and Unknown", () => {
    const projection = normalizeRepository(partialObservation());

    expect(projection.fetchHealth).toBe("partial");
    expect(projection.executive.mainSha.state).toBe("unknown");
    expect(projection.executive.productHorizon.state).toBe("unknown");
    expect(projection.attention.some((item) => item.state === "unknown")).toBe(true);
    expect(projection.humanAction.state).toBe("unknown");
  });

  it("never serializes the server credential", () => {
    const observation = healthyObservation();
    observation.serverCredential = "github_pat_never_reaches_the_browser";

    expect(JSON.stringify(normalizeRepository(observation))).not.toContain(
      observation.serverCredential,
    );
  });

  it("keeps structured human ownership visible as an attention condition", () => {
    const observation = healthyObservation();
    const issue = observation.issues[1];
    if (issue === undefined) throw new Error("fixture missing issue");
    issue.labels = ["agent:human", "state:ready"];

    const projection = normalizeRepository(observation);
    expect(projection.humanAction.state).toBe("blocked");
    expect(
      projection.attention.some(
        (item) => item.issueNumber === issue.number && item.reason === "human-action-required",
      ),
    ).toBe(true);
    expect(
      projection.deliveries.find((lane) => lane.issue?.number === issue.number)?.phase,
    ).toBe("human_required");
  });

  it("blocks a fully evidenced linked lane owned by a human", () => {
    const observation = healthyObservation();
    addGreenOperationalEvidence(observation);
    const issue = observation.issues[0];
    const pull = observation.pullRequests[0];
    if (issue === undefined || pull === undefined) throw new Error("fixture missing lane");
    issue.labels = ["agent:human", "state:ready"];
    pull.comments = pull.comments.map((comment) => ({
      ...comment,
      body: comment.body.replace("OWNER: agent:codex", "OWNER: agent:human"),
    }));

    const projection = normalizeRepository(observation);
    expect(projection.deliveries[0]).toMatchObject({
      humanAction: { state: "blocked", reason: "human-action-required" },
      mergeGate: { state: "blocked" },
      phase: "human_required",
    });
    expect(projection.humanAction.state).toBe("blocked");
  });

  it("fails strict coordination closed when comment pagination is incomplete", () => {
    const observation = healthyObservation();
    const pull = observation.pullRequests[0];
    if (pull === undefined) throw new Error("fixture missing pull request");
    pull.commentsAvailability = "incomplete";

    const lane = normalizeRepository(observation).deliveries[0];
    expect(lane?.handoff.state).toBe("unknown");
    expect(lane?.stewardWatch.state).toBe("unknown");
    expect(lane?.mergeGate.state).toBe("unknown");
  });

  it("preserves Unknown over pending checks when required evidence is incomplete", () => {
    const observation = healthyObservation();
    const pull = observation.pullRequests[0];
    if (pull === undefined) throw new Error("fixture missing pull request");
    pull.commentsAvailability = "incomplete";
    pull.checks = pull.checks.map((check) => ({ ...check, status: "pending" }));

    const lane = normalizeRepository(observation).deliveries[0];
    expect(lane?.checks.state).toBe("waiting");
    expect(lane?.handoff.state).toBe("unknown");
    expect(lane?.mergeGate.state).toBe("unknown");
    expect(lane?.phase).toBe("unknown");
  });

  it("distinguishes missing readiness from an explicit negative label", () => {
    const observation = healthyObservation();
    const issue = observation.issues[1];
    if (issue === undefined) throw new Error("fixture missing issue");
    issue.labels = ["agent:codex"];
    expect(normalizeRepository(observation).deliveries[1]?.readiness.state).toBe("unknown");

    issue.labels = ["agent:codex", "state:parked"];
    expect(normalizeRepository(observation).deliveries[1]?.readiness.state).toBe("blocked");
  });

  it("projects changed authority paths as reconciliation-needed Unknown", () => {
    const observation = healthyObservation();
    const pull = observation.pullRequests[0];
    if (pull === undefined) throw new Error("fixture missing pull request");
    pull.authorityChanges = [
      { path: "docs/governance/project-governance.md", url: "https://github.example/compare" },
    ];

    const authority = normalizeRepository(observation).deliveries[0]?.authority;
    expect(authority?.state).toBe("unknown");
    expect(authority?.reason).toBe("authority-drift-suspected");
    expect(authority?.sources[0]?.url).toBe("https://github.example/compare");
    expect(normalizeRepository(observation).deliveries[0]?.phase).toBe("rereview");
  });

  it("never lets changed or unavailable authority pass an otherwise-green merge gate", () => {
    const changed = healthyObservation();
    addGreenOperationalEvidence(changed);
    expect(normalizeRepository(changed).deliveries[0]?.mergeGate.state).toBe("satisfied");
    const changedPull = changed.pullRequests[0];
    if (changedPull === undefined) throw new Error("fixture missing pull request");
    changedPull.authorityChanges = [
      { path: "docs/vision/product-constitution.md", url: "https://github.example/compare" },
    ];
    expect(normalizeRepository(changed).deliveries[0]).toMatchObject({
      authority: { state: "unknown" },
      mergeGate: { state: "unknown" },
      phase: "rereview",
    });

    const unavailable = healthyObservation();
    addGreenOperationalEvidence(unavailable);
    const unavailablePull = unavailable.pullRequests[0];
    if (unavailablePull === undefined) throw new Error("fixture missing pull request");
    unavailablePull.authorityAvailability = "unavailable";
    expect(normalizeRepository(unavailable).deliveries[0]).toMatchObject({
      authority: { state: "unknown" },
      mergeGate: { state: "unknown" },
      phase: "unknown",
    });
  });

  it("keeps truncated dependencies Unknown in the critical-path projection", () => {
    const observation = healthyObservation();
    const issue = observation.issues[1];
    if (issue === undefined) throw new Error("fixture missing issue");
    issue.dependencyAvailability = "incomplete";

    const node = normalizeRepository(observation).criticalPath.nodes.find(
      (item) => item.issueNumber === issue.number,
    );
    expect(node?.state).toBe("unknown");
  });

  it("preserves explicit Blocked and dependency Waiting as distinct conditions", () => {
    const blocked = healthyObservation();
    const blockedIssue = blocked.issues[1];
    if (blockedIssue === undefined) throw new Error("fixture missing issue");
    blockedIssue.labels = ["agent:codex", "state:blocked"];
    expect(
      normalizeRepository(blocked).criticalPath.nodes.find(
        (item) => item.issueNumber === blockedIssue.number,
      )?.state,
    ).toBe("blocked");

    const waiting = healthyObservation();
    addGreenOperationalEvidence(waiting);
    const waitingIssue = waiting.issues[0];
    if (waitingIssue === undefined) throw new Error("fixture missing issue");
    waitingIssue.blockedBy = [
      { number: 200, state: "OPEN", url: "https://github.example/issues/200" },
    ];
    const lane = normalizeRepository(waiting).deliveries[0];
    expect(lane?.readiness.state).toBe("waiting");
    expect(lane?.mergeGate.state).toBe("waiting");
    expect(lane?.phase).toBe("waiting");
  });

  it("routes failed exact-head checks to blocked phase and attention", () => {
    const observation = healthyObservation();
    const pull = observation.pullRequests[0];
    if (pull === undefined) throw new Error("fixture missing pull request");
    pull.checks[0] = { ...pull.checks[0]!, status: "failure" };

    const projection = normalizeRepository(observation);
    expect(projection.deliveries[0]?.phase).toBe("blocked");
    expect(
      projection.attention.some(
        (item) => item.issueNumber === 169 && item.reason === "native-check-failed",
      ),
    ).toBe(true);
  });

  it("preserves a known exact-head failure when check pagination is incomplete", () => {
    const observation = healthyObservation();
    const pull = observation.pullRequests[0];
    if (pull === undefined) throw new Error("fixture missing pull request");
    pull.checksAvailability = "incomplete";
    pull.checks[0] = { ...pull.checks[0]!, status: "failure" };

    const lane = normalizeRepository(observation).deliveries[0];
    expect(lane?.checks.state).toBe("blocked");
    expect(lane?.phase).toBe("blocked");
  });

  it("keeps current review disposition separate from exact-head identity", () => {
    const observation = healthyObservation();
    addGreenOperationalEvidence(observation);
    const pull = observation.pullRequests[0];
    if (pull === undefined) throw new Error("fixture missing pull request");
    pull.reviews = [
      {
        id: "review-changes",
        authorLogin: "reviewer",
        submittedAt: "2026-09-01T00:00:00.000Z",
        commitSha: "1111111111111111111111111111111111111111",
        state: "CHANGES_REQUESTED",
        url: "https://github.example/reviews/changes",
      },
    ];
    expect(normalizeRepository(observation).deliveries[0]?.review.state).toBe("blocked");

    pull.reviews.push({
      id: "review-approval",
      authorLogin: "reviewer",
      submittedAt: "2026-09-01T00:01:00.000Z",
      commitSha: HEAD,
      state: "APPROVED",
      url: "https://github.example/reviews/approval",
    });
    expect(normalizeRepository(observation).deliveries[0]?.review.state).toBe("satisfied");
  });

  it("keeps current pending and ambiguous decisive reviews fail-closed", () => {
    const pending = healthyObservation();
    addGreenOperationalEvidence(pending);
    const pendingPull = pending.pullRequests[0];
    if (pendingPull === undefined) throw new Error("fixture missing pull request");
    pendingPull.reviews = [
      {
        id: "pending-review",
        authorLogin: "reviewer",
        submittedAt: null,
        commitSha: pendingPull.headSha,
        state: "PENDING",
        url: "https://github.example/reviews/pending",
      },
    ];
    expect(normalizeRepository(pending).deliveries[0]).toMatchObject({
      review: { state: "waiting" },
      phase: "review_wait",
    });

    if (pending.roadmap === null) throw new Error("fixture missing roadmap");
    pending.roadmap.markdown = "# Roadmap\n\n## Future";
    expect(normalizeRepository(pending).deliveries[0]).toMatchObject({
      review: { state: "waiting" },
      mergeGate: { state: "unknown" },
      phase: "unknown",
    });

    const incompleteComments = healthyObservation();
    addGreenOperationalEvidence(incompleteComments);
    const incompleteCommentsPull = incompleteComments.pullRequests[0];
    if (incompleteCommentsPull === undefined) throw new Error("fixture missing pull request");
    incompleteCommentsPull.commentsAvailability = "incomplete";
    incompleteCommentsPull.reviews = [
      {
        id: "pending-review",
        authorLogin: "reviewer",
        submittedAt: null,
        commitSha: incompleteCommentsPull.headSha,
        state: "PENDING",
        url: "https://github.example/reviews/pending",
      },
    ];
    expect(normalizeRepository(incompleteComments).deliveries[0]).toMatchObject({
      review: { state: "unknown" },
      mergeGate: { state: "unknown" },
      phase: "unknown",
    });

    const tied = healthyObservation();
    addGreenOperationalEvidence(tied);
    const tiedPull = tied.pullRequests[0];
    if (tiedPull === undefined) throw new Error("fixture missing pull request");
    tiedPull.reviews = [
      {
        id: "9",
        authorLogin: "reviewer",
        submittedAt: "2026-09-01T00:00:00.000Z",
        commitSha: tiedPull.headSha,
        state: "APPROVED",
        url: "https://github.example/reviews/approval",
      },
      {
        id: "10",
        authorLogin: "reviewer",
        submittedAt: "2026-09-01T00:00:00.000Z",
        commitSha: tiedPull.headSha,
        state: "CHANGES_REQUESTED",
        url: "https://github.example/reviews/changes",
      },
    ];
    expect(normalizeRepository(tied).deliveries[0]?.review.state).toBe("blocked");

    const missingTimestamp = healthyObservation();
    addGreenOperationalEvidence(missingTimestamp);
    const missingTimestampPull = missingTimestamp.pullRequests[0];
    if (missingTimestampPull === undefined) throw new Error("fixture missing pull request");
    missingTimestampPull.reviews = [
      {
        id: "approval-without-time",
        authorLogin: "reviewer",
        submittedAt: null,
        commitSha: missingTimestampPull.headSha,
        state: "APPROVED",
        url: "https://github.example/reviews/approval-without-time",
      },
    ];
    expect(normalizeRepository(missingTimestamp).deliveries[0]?.review.state).toBe(
      "unknown",
    );
  });

  it.each([
    ["missing", "# Roadmap\n\n## Future"],
    ["wrong-depth", "### Current horizon\n\n> **06 · Team Workspace Beta**"],
    [
      "fenced-example",
      "```md\n## Current horizon\n\n> **06 · Team Workspace Beta**\n```",
    ],
    [
      "fenced-pseudo-close",
      "````md\n```not-a-close\n## Current horizon\n\n> **06 · Team Workspace Beta**\n````",
    ],
    [
      "HTML-commented-example",
      "<!--\n## Current horizon\n\n> **06 · Team Workspace Beta**\n-->",
    ],
    [
      "ambiguous",
      "## Current horizon\n\n> **06 · Team Workspace Beta**\n> **07 · Migration**",
    ],
  ])("keeps merge authority Unknown for a %s current-horizon block", (_name, markdown) => {
    const observation = healthyObservation();
    addGreenOperationalEvidence(observation);
    if (observation.roadmap === null) throw new Error("fixture missing roadmap");
    observation.roadmap.markdown = markdown;

    const lane = normalizeRepository(observation).deliveries[0];
    expect(lane?.authority.state).toBe("unknown");
    expect(lane?.mergeGate.state).toBe("unknown");
    expect(lane?.phase).not.toBe("merge_gate");
  });

  it.each(["missing", "malformed"])(
    "keeps issue-only Ready authority and count Unknown when the Roadmap is %s",
    (kind) => {
      const observation = healthyObservation();
      if (kind === "missing") {
        observation.roadmap = null;
      } else if (observation.roadmap !== null) {
        observation.roadmap.markdown = "# Roadmap\n\n## Future";
      }

      const projection = normalizeRepository(observation);
      const lane = projection.deliveries.find(
        (item) => item.issue?.number === 223 && item.pullRequest === null,
      );
      expect(lane).toMatchObject({
        readiness: { state: "unknown" },
        authority: { state: "unknown" },
        phase: "unknown",
      });
      expect(projection.executive.readyCount).toMatchObject({
        state: "unknown",
        value: "Unknown",
      });
    },
  );

  it("preserves a known unlinked review blocker under partial pagination", () => {
    const observation = healthyObservation();
    const pull = observation.pullRequests[0];
    if (pull === undefined) throw new Error("fixture missing pull request");
    pull.closingIssueNumbers = [];
    pull.reviewsAvailability = "incomplete";
    pull.reviews = [
      {
        id: "unlinked-changes",
        authorLogin: "reviewer",
        submittedAt: "2026-09-01T00:00:00.000Z",
        commitSha: pull.headSha,
        state: "CHANGES_REQUESTED",
        url: "https://github.example/reviews/unlinked-changes",
      },
    ];

    const lane = normalizeRepository(observation).deliveries.find(
      (item) => item.pullRequest?.number === pull.number,
    );
    expect(lane?.issue).toBeNull();
    expect(lane?.review.state).toBe("blocked");
    expect(lane?.phase).toBe("blocked");
  });

  it.each([
    ["reviews", "unavailable", "observation-unavailable"],
    ["threads", "unavailable", "observation-unavailable"],
    ["reviews", "incomplete", "observation-incomplete"],
  ] as const)(
    "reports unlinked %s availability %s precisely",
    (kind, availability, reason) => {
      const observation = healthyObservation();
      const pull = observation.pullRequests[0];
      if (pull === undefined) throw new Error("fixture missing pull request");
      pull.closingIssueNumbers = [];
      if (kind === "reviews") pull.reviewsAvailability = availability;
      else pull.threadsAvailability = availability;

      const lane = normalizeRepository(observation).deliveries.find(
        (item) => item.pullRequest?.number === pull.number,
      );
      expect(lane?.review).toMatchObject({ state: "unknown", reason });
    },
  );

  it("preserves current pending disposition for an unlinked old-head review", () => {
    const observation = healthyObservation();
    const pull = observation.pullRequests[0];
    if (pull === undefined) throw new Error("fixture missing pull request");
    pull.closingIssueNumbers = [];
    pull.reviews = [
      {
        id: "unlinked-pending",
        authorLogin: "reviewer",
        submittedAt: null,
        commitSha: "1111111111111111111111111111111111111111",
        state: "PENDING",
        url: "https://github.example/reviews/unlinked-pending",
      },
    ];

    const lane = normalizeRepository(observation).deliveries.find(
      (item) => item.pullRequest?.number === pull.number,
    );
    expect(lane?.issue).toBeNull();
    expect(lane?.review.state).toBe("waiting");
    expect(lane?.phase).toBe("unknown");
  });

  it("uses a blocked reconciled merge gate for lane phase selection", () => {
    const observation = healthyObservation();
    addGreenOperationalEvidence(observation);
    const pull = observation.pullRequests[0];
    if (pull === undefined) throw new Error("fixture missing pull request");
    const validation = pull.comments.find((comment) =>
      comment.body.includes("KIND: validation"),
    );
    if (validation === undefined) throw new Error("fixture missing validation evidence");
    validation.body = validation.body.replace("RESULT: pass", "RESULT: fail");

    const lane = normalizeRepository(observation).deliveries[0];
    expect(lane?.checks.state).toBe("satisfied");
    expect(lane?.mergeGate.state).toBe("blocked");
    expect(lane?.phase).toBe("blocked");
  });

  it("deduplicates identical repository observation attention", () => {
    const observation = healthyObservation();
    observation.errors = [
      { source: "GitHub GraphQL", url: "https://api.github.example/graphql", reason: "observation-incomplete" },
      { source: "GitHub GraphQL", url: "https://api.github.example/graphql", reason: "observation-incomplete" },
    ];

    expect(
      normalizeRepository(observation).attention.filter(
        (item) => item.label === "GitHub GraphQL unavailable",
      ),
    ).toHaveLength(1);
  });

  it("keeps partial label evidence Unknown throughout a pull-request lane", () => {
    const observation = healthyObservation();
    const issue = observation.issues[0];
    if (issue === undefined) throw new Error("fixture missing issue");
    issue.labelsAvailability = "incomplete";

    const lane = normalizeRepository(observation).deliveries[0];
    expect(lane?.owner).toBe("unknown");
    expect(lane?.readiness.state).toBe("unknown");
    expect(lane?.humanAction.state).toBe("unknown");
    expect(lane?.mergeGate.state).not.toBe("satisfied");
    expect(lane?.phase).toBe("unknown");
  });

  it.each([
    ["missing", ["state:ready"]],
    ["conflicting", ["agent:codex", "agent:human", "state:ready"]],
  ])("keeps %s ownership Unknown for both pull-request and no-PR lanes", (_name, labels) => {
    const observation = healthyObservation();
    const pullIssue = observation.issues[0];
    const issueOnly = observation.issues[1];
    if (pullIssue === undefined || issueOnly === undefined) {
      throw new Error("fixture missing issue");
    }
    pullIssue.labels = [...labels];
    issueOnly.labels = [...labels];

    const projection = normalizeRepository(observation);
    for (const issueNumber of [169, 223]) {
      const lane = projection.deliveries.find((item) => item.issue?.number === issueNumber);
      expect(lane?.owner).toBe("unknown");
      expect(lane?.humanAction.state).toBe("unknown");
      expect(
        projection.attention.some(
          (item) => item.issueNumber === issueNumber && item.label === "Issue ownership Unknown",
        ),
      ).toBe(true);
    }
    expect(projection.deliveries[0]?.mergeGate.state).not.toBe("satisfied");
    expect(projection.humanAction.state).toBe("unknown");
  });

  it.each(["state:blocked", "state:parked"])(
    "keeps Ready plus %s as conflicting Unknown evidence",
    (negativeLabel) => {
      const observation = healthyObservation();
      const pullIssue = observation.issues[0];
      const issueOnly = observation.issues[1];
      if (pullIssue === undefined || issueOnly === undefined) {
        throw new Error("fixture missing issue");
      }
      pullIssue.labels = ["agent:codex", "state:ready", negativeLabel];
      issueOnly.labels = ["agent:codex", "state:ready", negativeLabel];

      const projection = normalizeRepository(observation);
      for (const issueNumber of [169, 223]) {
        const lane = projection.deliveries.find((item) => item.issue?.number === issueNumber);
        expect(lane?.readiness).toMatchObject({
          state: "unknown",
          reason: "source-identity-conflict",
        });
        expect(lane?.humanAction.state).toBe("unknown");
      }
      expect(projection.deliveries[0]?.mergeGate.state).not.toBe("satisfied");
      expect(projection.deliveries[0]?.phase).toBe("unknown");
      expect(projection.humanAction.state).toBe("unknown");
    },
  );

  it("keeps open pull requests visible when native Issue linkage is missing", () => {
    const observation = healthyObservation();
    const source = observation.pullRequests[0];
    if (source === undefined) throw new Error("fixture missing pull request");
    observation.pullRequests.push({
      ...source,
      number: 226,
      title: "Unlinked docs change",
      url: "https://github.example/pulls/226",
      closingIssueNumbers: [],
      comments: [],
      checks: [],
      reviews: [],
      threads: [],
    });

    const lane = normalizeRepository(observation).deliveries.find(
      (item) => item.pullRequest?.number === 226,
    );
    expect(lane?.issue).toBeNull();
    expect(lane?.readiness.state).toBe("unknown");
  });

  it("blocks every lane when multiple open implementation PRs close the same Issue", () => {
    const observation = healthyObservation();
    addGreenOperationalEvidence(observation);
    const source = observation.pullRequests[0];
    if (source === undefined) throw new Error("fixture missing pull request");
    observation.pullRequests.push({
      ...source,
      number: 226,
      title: "competing implementation",
      url: "https://github.example/pulls/226",
      comments: source.comments.map((comment) => ({
        ...comment,
        id: `duplicate-${comment.id}`,
        url: `https://github.example/comments/duplicate-${comment.id}`,
        body: comment.body.replace("PR: 225", "PR: 226"),
      })),
      checks: source.checks.map((check) => ({ ...check })),
      reviews: source.reviews.map((review) => ({ ...review })),
      threads: source.threads.map((thread) => ({ ...thread })),
    });

    const expectBothBlocked = () => {
      const lanes = normalizeRepository(observation).deliveries.filter(
        (lane) => lane.pullRequest?.number === 225 || lane.pullRequest?.number === 226,
      );
      expect(lanes).toHaveLength(2);
      for (const lane of lanes) {
        expect(lane.authority.state).toBe("blocked");
        expect(lane.mergeGate.state).toBe("blocked");
        expect(lane.phase).toBe("blocked");
      }
    };
    expectBothBlocked();

    for (const pull of observation.pullRequests) {
      pull.authorityChanges = [
        { path: "docs/vision/product-constitution.md", url: "https://github.example/compare" },
      ];
    }
    expectBothBlocked();

    for (const pull of observation.pullRequests) {
      pull.authorityChanges = [];
      pull.authorityAvailability = "unavailable";
    }
    expectBothBlocked();

    const issue = observation.issues[0];
    if (issue === undefined) throw new Error("fixture missing issue");
    issue.labelsAvailability = "incomplete";
    expectBothBlocked();
  });

  it("blocks mixed-cardinality PR linkage when any native Issue overlaps", () => {
    const observation = healthyObservation();
    const source = observation.pullRequests[0];
    if (source === undefined) throw new Error("fixture missing pull request");
    observation.pullRequests.push({
      ...source,
      number: 226,
      title: "multi-Issue competing implementation",
      url: "https://github.example/pulls/226",
      closingIssueNumbers: [169, 223],
      comments: [],
      checks: source.checks.map((check) => ({ ...check })),
      reviews: [],
      threads: [],
    });

    const projection = normalizeRepository(observation);
    const lanes = projection.deliveries.filter(
      (lane) => lane.pullRequest?.number === 225 || lane.pullRequest?.number === 226,
    );
    expect(lanes).toHaveLength(2);
    expect(lanes.find((lane) => lane.pullRequest?.number === 226)?.issue).toBeNull();
    expect(
      projection.deliveries.some(
        (lane) => lane.issue?.number === 223 && lane.pullRequest === null,
      ),
    ).toBe(false);
    expect(projection.executive.readyCount).toMatchObject({
      state: "satisfied",
      value: 0,
    });
    for (const lane of lanes) {
      expect(lane.authority.state).toBe("blocked");
      expect(lane.mergeGate.state).toBe("blocked");
      expect(lane.phase).toBe("blocked");
    }
  });

  it("blocks native and handoff-owned lanes that claim the same Issue", () => {
    const observation = healthyObservation();
    addGreenOperationalEvidence(observation);
    const source = observation.pullRequests[0];
    if (source === undefined) throw new Error("fixture missing pull request");
    observation.pullRequests.push({
      ...source,
      number: 226,
      title: "handoff-owned competing implementation",
      url: "https://github.example/pulls/226",
      closingIssueNumbers: [],
      comments: source.comments.map((comment) => ({
        ...comment,
        id: `handoff-duplicate-${comment.id}`,
        url: `https://github.example/comments/handoff-duplicate-${comment.id}`,
        body: comment.body.replace("PR: 225", "PR: 226"),
      })),
    });

    const lanes = normalizeRepository(observation).deliveries.filter(
      (lane) => lane.pullRequest?.number === 225 || lane.pullRequest?.number === 226,
    );
    expect(lanes).toHaveLength(2);
    for (const lane of lanes) {
      expect(lane.authority.state).toBe("blocked");
      expect(lane.phase).toBe("blocked");
    }
  });

  it("retains a visible handoff overlap blocker under partial comment pagination", () => {
    const observation = healthyObservation();
    const source = observation.pullRequests[0];
    if (source === undefined) throw new Error("fixture missing pull request");
    observation.pullRequests.push({
      ...source,
      number: 226,
      title: "partially observed handoff owner",
      url: "https://github.example/pulls/226",
      closingIssueNumbers: [],
      commentsAvailability: "incomplete",
      comments: source.comments.map((comment) => ({
        ...comment,
        id: `partial-${comment.id}`,
        body: comment.body.replace("PR: 225", "PR: 226"),
      })),
    });
    observation.implementationLinkageAvailability = "incomplete";

    const lanes = normalizeRepository(observation).deliveries.filter(
      (lane) => lane.pullRequest?.number === 225 || lane.pullRequest?.number === 226,
    );
    expect(lanes).toHaveLength(2);
    for (const lane of lanes) {
      expect(lane).toMatchObject({
        authority: { state: "blocked" },
        mergeGate: { state: "blocked" },
        phase: "blocked",
      });
    }
  });

  it("keeps duplicate trusted handoff claims in overlap accounting", () => {
    const observation = healthyObservation();
    const source = observation.pullRequests[0];
    const handoff = source?.comments.find((comment) =>
      comment.body.startsWith("<!-- agent-handoff:v1 -->"),
    );
    if (source === undefined || handoff === undefined) throw new Error("fixture missing handoff");
    const competingHandoff = {
      ...handoff,
      id: "competing-handoff",
      body: handoff.body.replace("PR: 225", "PR: 226"),
    };
    observation.pullRequests.push({
      ...source,
      number: 226,
      title: "duplicate handoff owner",
      url: "https://github.example/pulls/226",
      closingIssueNumbers: [],
      comments: [
        competingHandoff,
        { ...competingHandoff, id: "competing-handoff-duplicate" },
      ],
    });

    expect(
      normalizeRepository(observation).deliveries.find(
        (lane) => lane.pullRequest?.number === 225,
      )?.authority.state,
    ).toBe("blocked");
  });

  it("suppresses issue-only Ready when a trusted handoff owns the Issue", () => {
    const observation = healthyObservation();
    const source = observation.pullRequests[0];
    if (source === undefined) throw new Error("fixture missing pull request");
    source.closingIssueNumbers = [];

    const projection = normalizeRepository(observation);
    expect(
      projection.deliveries.some((lane) => lane.issue?.number === 169),
    ).toBe(false);
    expect(projection.executive.readyCount).toMatchObject({ state: "satisfied", value: 1 });
  });

  it("does not treat untrusted handoff prose as implementation ownership", () => {
    const observation = healthyObservation();
    addGreenOperationalEvidence(observation);
    const source = observation.pullRequests[0];
    if (source === undefined) throw new Error("fixture missing pull request");
    observation.pullRequests.push({
      ...source,
      number: 226,
      title: "untrusted handoff claim",
      url: "https://github.example/pulls/226",
      closingIssueNumbers: [],
      comments: source.comments.map((comment) => ({
        ...comment,
        id: `untrusted-${comment.id}`,
        trustedProducer: false,
        body: comment.body.replace("PR: 225", "PR: 226"),
      })),
    });

    expect(
      normalizeRepository(observation).deliveries.find(
        (lane) => lane.pullRequest?.number === 225,
      )?.authority.state,
    ).toBe("satisfied");
  });

  it("keeps future-milestone Ready Issues out of the current horizon", () => {
    const observation = healthyObservation();
    const issue = observation.issues[1];
    if (issue === undefined) throw new Error("fixture missing issue");
    issue.milestone = "07 · Future horizon";

    const projection = normalizeRepository(observation);
    const lane = projection.deliveries.find((item) => item.issue?.number === issue.number);
    expect(lane).toMatchObject({
      readiness: { state: "blocked", reason: "issue-not-ready" },
      phase: "blocked",
    });
    expect(
      projection.criticalPath.nodes.find((item) => item.issueNumber === issue.number)?.state,
    ).toBe("blocked");
    expect(projection.executive.readyCount).toMatchObject({ state: "satisfied", value: 0 });
  });

  it("fails an otherwise-green linked future-milestone lane closed", () => {
    const observation = healthyObservation();
    addGreenOperationalEvidence(observation);
    const issue = observation.issues[0];
    if (issue === undefined) throw new Error("fixture missing issue");
    issue.milestone = "07 · Future horizon";

    expect(normalizeRepository(observation).deliveries[0]).toMatchObject({
      readiness: { state: "blocked", reason: "issue-not-ready" },
      mergeGate: { state: "blocked" },
      phase: "blocked",
    });
  });

  it("preserves the unmilestoned no-PR Ready exception across projections", () => {
    const observation = healthyObservation();
    const issue = observation.issues[1];
    if (issue === undefined) throw new Error("fixture missing issue");
    issue.milestone = null;

    const projection = normalizeRepository(observation);
    expect(
      projection.deliveries.find((lane) => lane.issue?.number === issue.number),
    ).toMatchObject({ readiness: { state: "satisfied" }, phase: "ready" });
    expect(projection.executive.readyCount).toMatchObject({ state: "satisfied", value: 1 });
    expect(
      projection.criticalPath.nodes.find((item) => item.issueNumber === issue.number)?.state,
    ).toBe("ready");
  });

  it.each(["missing", "malformed"])(
    "keeps milestone alignment Unknown across linked and critical-path projections when Roadmap is %s",
    (kind) => {
      const observation = healthyObservation();
      addGreenOperationalEvidence(observation);
      const linkedIssue = observation.issues[0];
      if (linkedIssue === undefined) throw new Error("fixture missing issue");
      linkedIssue.milestone = "06 · Team Workspace Beta";
      if (kind === "missing") observation.roadmap = null;
      else if (observation.roadmap !== null) observation.roadmap.markdown = "# Roadmap\n\n## Future";

      const projection = normalizeRepository(observation);
      expect(projection.deliveries[0]).toMatchObject({
        readiness: { state: "unknown" },
        mergeGate: { state: "unknown" },
        phase: "unknown",
      });
      expect(
        projection.criticalPath.nodes.find(
          (item) => item.issueNumber === linkedIssue.number,
        )?.state,
      ).toBe("unknown");
    },
  );

  it.each([
    ["conflicting", "blocked"],
    ["unknown", "unknown"],
  ] as const)("fails merge closed for native mergeability %s", (mergeability, state) => {
    const observation = healthyObservation();
    addGreenOperationalEvidence(observation);
    const pull = observation.pullRequests[0];
    if (pull === undefined) throw new Error("fixture missing pull request");
    pull.mergeability = mergeability;

    expect(normalizeRepository(observation).deliveries[0]).toMatchObject({
      mergeGate: { state },
      phase: state === "blocked" ? "blocked" : "unknown",
    });
  });

  it("keeps implementation overlap Unknown when the PR/linkage set is incomplete", () => {
    const observation = healthyObservation();
    addGreenOperationalEvidence(observation);
    observation.implementationLinkageAvailability = "incomplete";

    const projection = normalizeRepository(observation);
    const lane = projection.deliveries[0];
    const issueOnlyLane = projection.deliveries.find(
      (item) => item.issue?.number === 223 && item.pullRequest === null,
    );
    expect(lane?.authority.state).toBe("unknown");
    expect(lane?.mergeGate.state).toBe("unknown");
    expect(lane?.phase).toBe("unknown");
    expect(issueOnlyLane?.phase).toBe("unknown");
    expect(issueOnlyLane?.mergeGate.label).toBe("Implementation PR linkage Unknown");
    expect(projection.executive.activeCount).toMatchObject({
      state: "unknown",
      value: "Unknown",
    });
    expect(projection.executive.readyCount).toMatchObject({
      state: "unknown",
      value: "Unknown",
    });
  });

  it("keeps other lanes Unknown when a hidden paginated handoff may claim ownership", () => {
    const observation = healthyObservation();
    addGreenOperationalEvidence(observation);
    const source = observation.pullRequests[0];
    if (source === undefined) throw new Error("fixture missing pull request");
    observation.pullRequests.push({
      ...source,
      number: 226,
      title: "unobserved handoff owner",
      url: "https://github.example/pulls/226",
      closingIssueNumbers: [],
      comments: [],
      commentsAvailability: "incomplete",
    });
    observation.implementationLinkageAvailability = "incomplete";

    expect(
      normalizeRepository(observation).deliveries.find(
        (lane) => lane.pullRequest?.number === 225,
      ),
    ).toMatchObject({
      authority: { state: "unknown", reason: "observation-incomplete" },
      mergeGate: { state: "unknown" },
      phase: "unknown",
    });
  });

  it("keeps Ready counts Unknown when Issue or label observation is incomplete", () => {
    const incompleteIssues = healthyObservation();
    incompleteIssues.issuesAvailability = "incomplete";
    expect(normalizeRepository(incompleteIssues).executive.readyCount).toMatchObject({
      state: "unknown",
      value: "Unknown",
    });

    const incompleteLabels = healthyObservation();
    const issue = incompleteLabels.issues[1];
    if (issue === undefined) throw new Error("fixture missing issue");
    issue.labelsAvailability = "incomplete";
    expect(normalizeRepository(incompleteLabels).executive.readyCount).toMatchObject({
      state: "unknown",
      value: "Unknown",
    });

    for (const availability of ["incomplete", "unavailable"] as const) {
      const incompleteDependencies = healthyObservation();
      const dependencyIssue = incompleteDependencies.issues[1];
      if (dependencyIssue === undefined) throw new Error("fixture missing issue");
      dependencyIssue.dependencyAvailability = availability;
      expect(
        normalizeRepository(incompleteDependencies).executive.readyCount,
      ).toMatchObject({ state: "unknown", value: "Unknown" });
    }
  });

  it("scopes incomplete pull evidence to its own linked lane", () => {
    const observation = healthyObservation();
    addGreenOperationalEvidence(observation);
    const source = observation.pullRequests[0];
    if (source === undefined) throw new Error("fixture missing pull request");
    const secondHead = "3333333333333333333333333333333333333333";
    observation.pullRequests.push({
      ...source,
      number: 226,
      title: "independent incomplete lane",
      url: "https://github.example/pulls/226",
      headSha: secondHead,
      closingIssueNumbers: [223],
      comments: [],
      reviewsAvailability: "incomplete",
      checks: source.checks.map((check) => ({ ...check, headSha: secondHead })),
      reviews: [],
      threads: [],
    });
    observation.availability = "incomplete";
    observation.pullsAvailability = "incomplete";
    observation.errors.push({
      source: "PR #226 reviews",
      url: "https://github.example/pulls/226",
      reason: "observation-incomplete",
    });

    const projection = normalizeRepository(observation);
    expect(
      projection.deliveries.find((lane) => lane.pullRequest?.number === 225)?.mergeGate,
    ).toMatchObject({ state: "satisfied" });
    expect(
      projection.deliveries.find((lane) => lane.pullRequest?.number === 226)?.mergeGate,
    ).toMatchObject({ state: "unknown" });
    expect(projection.fetchHealth).toBe("partial");
  });

  it("preserves explicit Blocked over incomplete implementation linkage", () => {
    const observation = healthyObservation();
    observation.implementationLinkageAvailability = "incomplete";
    const issue = observation.issues[1];
    if (issue === undefined) throw new Error("fixture missing issue");
    issue.labels = ["agent:codex", "state:blocked"];

    const lane = normalizeRepository(observation).deliveries.find(
      (item) => item.issue?.number === issue.number && item.pullRequest === null,
    );
    expect(lane?.readiness.state).toBe("blocked");
    expect(lane?.phase).toBe("blocked");
    expect(lane?.mergeGate.state).toBe("unknown");
  });

  it.each([
    ["conflicting", "blocked"],
    ["unknown", "unknown"],
  ] as const)(
    "preserves native mergeability %s for an unlinked lane",
    (mergeability, state) => {
      const observation = healthyObservation();
      const pull = observation.pullRequests[0];
      if (pull === undefined) throw new Error("fixture missing pull request");
      pull.closingIssueNumbers = [];
      pull.mergeability = mergeability;

      expect(
        normalizeRepository(observation).deliveries.find(
          (lane) => lane.pullRequest?.number === pull.number,
        ),
      ).toMatchObject({ mergeGate: { state }, phase: state });
    },
  );

  it("exposes exact check provenance on the delivery lane", () => {
    const lane = normalizeRepository(healthyObservation()).deliveries[0];
    expect(lane?.sources.some((source) => source.url.endsWith("/checks/browser"))).toBe(true);
  });
});
