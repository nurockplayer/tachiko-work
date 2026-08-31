import { describe, expect, it } from "vitest";

import type {
  CommentSourceMetadata,
  StructuredCommentSource,
} from "../src/model.js";
import {
  reconcile,
  type NativeCheck,
  type NativeReview,
  type NativeReviewThread,
  type Observation,
  type ReconcileInput,
} from "../src/reconcile.js";

const HEAD = "a".repeat(40);
const MAIN = "b".repeat(40);
const OLD_HEAD = "c".repeat(40);

function comment(
  id: string,
  body: string,
  metadata: Partial<CommentSourceMetadata> = {},
): StructuredCommentSource {
  return {
    body,
    metadata: {
      repository: "nurockplayer/tachiko-work",
      id,
      kind: "issue-comment",
      authorLogin: "steward",
      authorAssociation: "OWNER",
      url: `https://example.test/${id}`,
      createdAt: "2026-08-31T00:00:00Z",
      updatedAt: null,
      edited: false,
      topLevel: true,
      trustedProducer: true,
      ...metadata,
    },
  };
}

function handoff(state = "merge-ready", id = "handoff"): StructuredCommentSource {
  return comment(
    id,
    [
      "<!-- agent-handoff:v1 -->",
      "ISSUE: 200",
      "PR: 201",
      "OWNER: agent:codex",
      `STATE: ${state}`,
      `HEAD: ${HEAD}`,
      `MAIN: ${MAIN}`,
      "",
      "Narrative says HOLD, failed, P1, resolved, and Ready.",
    ].join("\n"),
    { edited: true },
  );
}

function watch(
  verdict: "GREEN" | "AMBER" | "HOLD" = "GREEN",
  humanAction: "none" | "required" = "none",
  head = HEAD,
  id = "watch",
): StructuredCommentSource {
  return comment(
    id,
    [
      "<!-- project-steward-watch:v1 -->",
      `VERDICT: ${verdict}`,
      `HEAD: ${head}`,
      `MAIN: ${MAIN}`,
      `HUMAN_ACTION: ${humanAction}`,
    ].join("\n"),
    { edited: true },
  );
}

function validation(
  id: string,
  result: "pass" | "fail" | "unknown",
  options: {
    head?: string;
    name?: string;
    run?: string;
    supersedes?: string;
  } = {},
): StructuredCommentSource {
  return comment(
    id,
    [
      "<!-- operational-evidence:v1",
      "KIND: validation",
      "PR: 201",
      `HEAD: ${options.head ?? HEAD}`,
      `RUN: ${options.run ?? `run-${id}`}`,
      `NAME: ${options.name ?? "manual-validation"}`,
      `RESULT: ${result}`,
      ...(options.supersedes === undefined
        ? []
        : [`SUPERSEDES: ${options.supersedes}`]),
      "-->",
    ].join("\n"),
  );
}

function review(
  id: string,
  result: "clean" | "findings" | "unknown",
  options: {
    head?: string;
    name?: string;
    run?: string;
    supersedes?: string;
  } = {},
): StructuredCommentSource {
  return comment(
    id,
    [
      "<!-- operational-evidence:v1",
      "KIND: review",
      "PR: 201",
      `HEAD: ${options.head ?? HEAD}`,
      `RUN: ${options.run ?? `run-${id}`}`,
      `NAME: ${options.name ?? "exact-head-review"}`,
      `RESULT: ${result}`,
      ...(options.supersedes === undefined
        ? []
        : [`SUPERSEDES: ${options.supersedes}`]),
      "-->",
    ].join("\n"),
  );
}

function finding(
  id: string,
  severity: "P0" | "P1" | "P2" | "P3",
  run: string,
  head = HEAD,
): StructuredCommentSource {
  return comment(
    id,
    [
      "<!-- operational-evidence:v1",
      "KIND: review-finding",
      "PR: 201",
      `HEAD: ${head}`,
      `RUN: ${run}`,
      `SEVERITY: ${severity}`,
      "-->",
    ].join("\n"),
  );
}

function resolution(
  id: string,
  resolves: string,
  head = HEAD,
): StructuredCommentSource {
  return comment(
    id,
    [
      "<!-- operational-evidence:v1",
      "KIND: review-resolution",
      "PR: 201",
      `HEAD: ${head}`,
      `RESOLVES: ${resolves}`,
      "-->",
    ].join("\n"),
  );
}

function complete<T>(facts: readonly T[]): Observation<T> {
  return { availability: "complete", facts, source: { id: "api" } };
}

function baseInput(overrides: Partial<ReconcileInput> = {}): ReconcileInput {
  return {
    context: {
      repository: "nurockplayer/tachiko-work",
      issueNumber: 200,
      pullRequestNumber: 201,
      owner: "agent:codex",
      headSha: HEAD,
      mainSha: MAIN,
    },
    comments: [
      handoff(),
      watch(),
      validation("validation-pass", "pass"),
      review("review-clean", "clean"),
    ],
    nativeChecks: complete<NativeCheck>([]),
    nativeReviews: complete<NativeReview>([]),
    nativeThreads: complete<NativeReviewThread>([]),
    nativeRepository: complete([
      {
        issueReady: true,
        dependencies: "satisfied",
        pullRequestState: "OPEN",
        pullRequestDraft: false,
        baseRef: "main",
        authorityConflict: false,
      },
    ]),
    requirements: {
      requiredValidations: [
        { name: "manual-validation", evidence: "manual" },
      ],
      requiredReview: { name: "exact-head-review", evidence: "manual" },
      currentStewardWatch: true,
      expectedBaseRef: "main",
    },
    ...overrides,
  };
}

describe("reconcile", () => {
  it("satisfies all gates from current structured evidence without reading prose", () => {
    const result = reconcile(baseInput());

    expect(result.handoff).toMatchObject({
      state: "satisfied",
      reason: "handoff-current",
    });
    expect(result.watch).toMatchObject({
      state: "satisfied",
      reason: "steward-watch-green",
    });
    expect(result.validations).toEqual([
      expect.objectContaining({
        name: "manual-validation",
        state: "satisfied",
        reason: "validation-passed",
      }),
    ]);
    expect(result.review).toMatchObject({
      state: "satisfied",
      reason: "review-clean-current",
    });
    expect(result.mutationGate.state).toBe("satisfied");
    expect(result.mergeGate.state).toBe("satisfied");
  });

  it("does not let handoff merge-ready or GREEN grant missing evidence", () => {
    const result = reconcile(
      baseInput({ comments: [handoff("merge-ready"), watch("GREEN")] }),
    );

    expect(result.handoff.state).toBe("satisfied");
    expect(result.validations[0]).toMatchObject({
      state: "unknown",
      reason: "validation-missing",
    });
    expect(result.review).toMatchObject({
      state: "unknown",
      reason: "review-missing",
    });
    expect(result.mergeGate.state).toBe("unknown");
  });

  it("requires complete native authority and pull-request facts for gate projections", () => {
    const unavailable = reconcile(
      baseInput({
        nativeRepository: {
          availability: "unavailable",
          source: { id: "repository-api" },
        },
      }),
    );
    const notReady = reconcile(
      baseInput({
        nativeRepository: complete([
          {
            issueReady: false,
            dependencies: "satisfied",
            pullRequestState: "OPEN",
            pullRequestDraft: false,
            baseRef: "main",
            authorityConflict: false,
          },
        ]),
      }),
    );

    expect(unavailable.authority).toMatchObject({
      state: "unknown",
      reason: "observation-unavailable",
    });
    expect(unavailable.mergeGate.state).toBe("unknown");
    expect(notReady.authority).toMatchObject({
      state: "blocked",
      reason: "issue-not-ready",
    });
    expect(notReady.mutationGate.state).toBe("blocked");
    expect(notReady.mergeGate.state).toBe("blocked");
  });

  it("keeps draft PR policy out of mutation while blocking merge", () => {
    const result = reconcile(
      baseInput({
        nativeRepository: complete([
          {
            issueReady: true,
            dependencies: "satisfied",
            pullRequestState: "OPEN",
            pullRequestDraft: true,
            baseRef: "main",
            authorityConflict: false,
          },
        ]),
      }),
    );

    expect(result.mutationGate.state).toBe("satisfied");
    expect(result.mergePolicy).toMatchObject({
      state: "blocked",
      reason: "pull-request-draft",
    });
    expect(result.mergeGate.state).toBe("blocked");
  });

  it("selects before parsing and fails closed on two trusted handoffs", () => {
    const malformed = comment(
      "malformed",
      ["<!-- agent-handoff:v1 -->", "STATUS: ready"].join("\n"),
    );
    const result = reconcile(
      baseInput({ comments: [handoff(), malformed, watch()] }),
    );

    expect(result.handoff).toMatchObject({
      state: "unknown",
      reason: "trusted-source-ambiguous",
    });
    expect(result.mutationGate.state).toBe("unknown");
  });

  it("ignores an untrusted imitation when exactly one trusted handoff exists", () => {
    const imitation = comment("fake", handoff().body, {
      trustedProducer: false,
      authorAssociation: "NONE",
    });
    const result = reconcile(
      baseInput({ comments: [handoff(), imitation, watch()] }),
    );

    expect(result.handoff.state).toBe("satisfied");
  });

  it("blocks mutation and merge for current HOLD or human action", () => {
    const hold = reconcile(
      baseInput({
        comments: [
          handoff(),
          watch("HOLD"),
          validation("pass", "pass"),
          review("clean", "clean"),
        ],
      }),
    );
    const human = reconcile(
      baseInput({
        comments: [
          handoff(),
          watch("GREEN", "required"),
          validation("pass", "pass"),
          review("clean", "clean"),
        ],
      }),
    );

    expect(hold.watch).toMatchObject({
      state: "blocked",
      reason: "steward-watch-hold",
    });
    expect(hold.mutationGate.state).toBe("blocked");
    expect(hold.mergeGate.state).toBe("blocked");
    expect(human.humanAction).toMatchObject({
      state: "blocked",
      reason: "human-action-required",
    });
    expect(human.mutationGate.state).toBe("blocked");
  });

  it("treats a stale HOLD as Unknown instead of a current blocker", () => {
    const result = reconcile(
      baseInput({ comments: [handoff(), watch("HOLD", "none", OLD_HEAD)] }),
    );

    expect(result.watch).toMatchObject({ state: "unknown", reason: "head-mismatch" });
    expect(result.mutationGate.state).toBe("unknown");
  });

  it("lets a current native check outrank contradictory custom evidence", () => {
    const nativeFailure: NativeCheck = {
      name: "manual-validation",
      head: HEAD,
      status: "failure",
      source: { id: "check-1" },
    };
    const result = reconcile(
      baseInput({ nativeChecks: complete([nativeFailure]) }),
    );

    expect(result.validations[0]).toMatchObject({
      state: "blocked",
      reason: "native-check-failed",
    });
    expect(result.mergeGate.state).toBe("blocked");
  });

  it("does not let custom evidence replace native-required validation or review", () => {
    const result = reconcile(
      baseInput({
        requirements: {
          requiredValidations: [
            { name: "manual-validation", evidence: "native-check" },
          ],
          requiredReview: {
            name: "exact-head-review",
            evidence: "native-review",
          },
          currentStewardWatch: true,
          expectedBaseRef: "main",
        },
      }),
    );

    expect(result.validations[0]).toMatchObject({
      state: "unknown",
      reason: "validation-missing",
    });
    expect(result.review).toMatchObject({
      state: "unknown",
      reason: "review-missing",
    });
    expect(result.advisories).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ reason: "custom-evidence-not-applicable" }),
      ]),
    );
    expect(result.mergeGate.state).toBe("unknown");
  });

  it("lets a current native success outrank a custom failure", () => {
    const nativeSuccess: NativeCheck = {
      name: "manual-validation",
      head: HEAD,
      status: "success",
      source: { id: "check-success" },
    };
    const result = reconcile(
      baseInput({
        comments: [
          handoff(),
          watch(),
          validation("custom-fail", "fail"),
          review("review-clean", "clean"),
        ],
        nativeChecks: complete([nativeSuccess]),
      }),
    );

    expect(result.validations[0]).toMatchObject({
      state: "satisfied",
      reason: "native-check-succeeded",
    });
    expect(result.advisories).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ reason: "custom-evidence-shadowed" }),
      ]),
    );
  });

  it("distinguishes native pending and unavailable from failure", () => {
    const pending: NativeCheck = {
      name: "manual-validation",
      head: HEAD,
      status: "pending",
      source: { id: "check-pending" },
    };
    const pendingResult = reconcile(
      baseInput({ nativeChecks: complete([pending]) }),
    );
    const unavailableResult = reconcile(
      baseInput({
        nativeChecks: {
          availability: "unavailable",
          source: { id: "checks-api" },
        },
      }),
    );

    expect(pendingResult.validations[0]?.state).toBe("waiting");
    expect(pendingResult.mergeGate.state).toBe("waiting");
    expect(unavailableResult.validations[0]).toMatchObject({
      state: "unknown",
      reason: "observation-unavailable",
    });
    expect(unavailableResult.mergeGate.state).toBe("unknown");
  });

  it("keeps old-head validation and review success stale", () => {
    const result = reconcile(
      baseInput({
        comments: [
          handoff(),
          watch(),
          validation("old-validation", "pass", { head: OLD_HEAD }),
          review("old-review", "clean", { head: OLD_HEAD }),
        ],
      }),
    );

    expect(result.validations[0]).toMatchObject({
      state: "unknown",
      reason: "validation-stale",
    });
    expect(result.review).toMatchObject({
      state: "unknown",
      reason: "review-stale",
    });
  });

  it("requires explicit valid supersession and reports conflicts and dangling links", () => {
    const conflict = reconcile(
      baseInput({
        comments: [
          handoff(),
          watch(),
          validation("pass", "pass"),
          validation("fail", "fail"),
          review("clean", "clean"),
        ],
      }),
    );
    const superseded = reconcile(
      baseInput({
        comments: [
          handoff(),
          watch(),
          validation("pass", "pass"),
          validation("fail", "fail", { supersedes: "pass" }),
          review("clean", "clean"),
        ],
      }),
    );
    const dangling = reconcile(
      baseInput({
        comments: [
          handoff(),
          watch(),
          validation("pass", "pass", { supersedes: "missing" }),
          review("clean", "clean"),
        ],
      }),
    );

    expect(conflict.validations[0]).toMatchObject({
      state: "unknown",
      reason: "evidence-conflict",
    });
    expect(superseded.validations[0]).toMatchObject({
      state: "blocked",
      reason: "validation-failed",
    });
    expect(dangling.validations[0]).toMatchObject({
      state: "unknown",
      reason: "reference-missing",
    });
  });

  it("rejects cyclic and wrong-slot supersession relationships", () => {
    const cyclic = reconcile(
      baseInput({
        comments: [
          handoff(),
          watch(),
          validation("cycle-a", "pass", { supersedes: "cycle-b" }),
          validation("cycle-b", "fail", { supersedes: "cycle-a" }),
          review("clean", "clean"),
        ],
      }),
    );
    const wrongSlot = reconcile(
      baseInput({
        comments: [
          handoff(),
          watch(),
          review("other-kind", "clean", { name: "manual-validation" }),
          validation("wrong-slot", "pass", { supersedes: "other-kind" }),
          review("clean", "clean"),
        ],
      }),
    );

    expect(cyclic.validations[0]).toMatchObject({
      state: "unknown",
      reason: "reference-cycle",
    });
    expect(wrongSlot.validations[0]).toMatchObject({
      state: "unknown",
      reason: "reference-mismatch",
    });
  });

  it("blocks on current CHANGES_REQUESTED despite a clean attestation", () => {
    const changes: NativeReview = {
      current: true,
      head: HEAD,
      state: "CHANGES_REQUESTED",
      source: { id: "review-native" },
    };
    const result = reconcile(
      baseInput({ nativeReviews: complete([changes]) }),
    );

    expect(result.review).toMatchObject({
      state: "blocked",
      reason: "native-changes-requested",
    });
    expect(result.advisories).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ reason: "custom-evidence-shadowed" }),
      ]),
    );
  });

  it("separates current review disposition from exact-head approval identity", () => {
    const currentOldHeadChanges: NativeReview = {
      current: true,
      head: OLD_HEAD,
      state: "CHANGES_REQUESTED",
      source: { id: "changes-current" },
    };
    const staleApproval: NativeReview = {
      current: false,
      head: HEAD,
      state: "APPROVED",
      source: { id: "approval-stale" },
    };
    const blocked = reconcile(
      baseInput({ nativeReviews: complete([currentOldHeadChanges]) }),
    );
    const stale = reconcile(
      baseInput({
        comments: [handoff(), watch(), validation("pass", "pass")],
        nativeReviews: complete([staleApproval]),
      }),
    );

    expect(blocked.review).toMatchObject({
      state: "blocked",
      reason: "native-changes-requested",
    });
    expect(stale.review).toMatchObject({
      state: "unknown",
      reason: "review-stale",
    });
  });

  it("accepts only a current exact-head native approval", () => {
    const approval: NativeReview = {
      current: true,
      head: HEAD,
      state: "APPROVED",
      source: { id: "approval-current" },
    };
    const result = reconcile(
      baseInput({
        comments: [handoff(), watch(), validation("pass", "pass")],
        nativeReviews: complete([approval]),
        advisories: [{ kind: "prose", source: { id: "positive-review-body" } }],
      }),
    );

    expect(result.review).toMatchObject({
      state: "satisfied",
      reason: "native-approval-current",
    });
    expect(result.mergeGate.state).toBe("satisfied");
  });

  it("keeps a current pending review waiting even with an exact-head approval", () => {
    const approval: NativeReview = {
      current: true,
      head: HEAD,
      state: "APPROVED",
      source: { id: "approval-current" },
    };
    const pending: NativeReview = {
      current: true,
      head: HEAD,
      state: "PENDING",
      source: { id: "review-pending" },
    };
    const result = reconcile(
      baseInput({
        comments: [handoff(), watch(), validation("pass", "pass")],
        nativeReviews: complete([approval, pending]),
      }),
    );

    expect(result.review).toMatchObject({
      state: "waiting",
      reason: "native-review-pending",
    });
    expect(result.mergeGate.state).toBe("waiting");
  });

  it("keeps an explicit structured unknown review Unknown", () => {
    const result = reconcile(
      baseInput({
        comments: [
          handoff(),
          watch(),
          validation("pass", "pass"),
          review("review-unknown", "unknown"),
        ],
      }),
    );

    expect(result.review).toMatchObject({
      state: "unknown",
      reason: "review-unknown",
    });
  });

  it("blocks unresolved P0-P2 native threads even when outdated", () => {
    const thread: NativeReviewThread = {
      resolved: false,
      outdated: true,
      severity: "P2",
      source: { id: "thread-1" },
    };
    const result = reconcile(
      baseInput({ nativeThreads: complete([thread]) }),
    );

    expect(result.review).toMatchObject({
      state: "blocked",
      reason: "native-thread-blocking",
    });
  });

  it("keeps unknown native severity Unknown and P3 advisory", () => {
    const unknown: NativeReviewThread = {
      resolved: false,
      outdated: false,
      severity: "unknown",
      source: { id: "thread-unknown" },
    };
    const p3: NativeReviewThread = {
      resolved: false,
      outdated: true,
      severity: "P3",
      source: { id: "thread-p3" },
    };
    const unknownResult = reconcile(
      baseInput({ nativeThreads: complete([unknown]) }),
    );
    const p3Result = reconcile(
      baseInput({ nativeThreads: complete([p3]) }),
    );

    expect(unknownResult.review).toMatchObject({
      state: "unknown",
      reason: "native-thread-unknown",
    });
    expect(p3Result.review.state).toBe("satisfied");
    expect(p3Result.advisories).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ reason: "native-thread-p3" }),
      ]),
    );
  });

  it("fails closed when native review-thread observation is unavailable", () => {
    const result = reconcile(
      baseInput({
        nativeThreads: {
          availability: "unavailable",
          source: { id: "threads-api" },
        },
      }),
    );

    expect(result.review).toMatchObject({
      state: "unknown",
      reason: "observation-unavailable",
    });
    expect(result.mergeGate.state).toBe("unknown");
  });

  it("persists structured blocking findings across heads until explicit resolution", () => {
    const oldFinding = finding("finding-p1", "P1", "review-run", OLD_HEAD);
    const blocked = reconcile(
      baseInput({
        comments: [
          handoff(),
          watch(),
          validation("pass", "pass"),
          review("clean", "clean"),
          oldFinding,
        ],
      }),
    );
    const resolved = reconcile(
      baseInput({
        comments: [
          handoff(),
          watch(),
          validation("pass", "pass"),
          review("clean", "clean"),
          oldFinding,
          resolution("resolution", "finding-p1"),
        ],
      }),
    );

    expect(blocked.review).toMatchObject({
      state: "blocked",
      reason: "review-finding-blocking",
    });
    expect(resolved.review.state).toBe("satisfied");
  });

  it("keeps a standalone P2 finding blocking and cannot resolve a native thread", () => {
    const result = reconcile(
      baseInput({
        comments: [
          handoff(),
          watch(),
          validation("pass", "pass"),
          review("clean", "clean"),
          finding("orphan", "P2", "no-review-run"),
          resolution("bad-resolution", "native-thread-42"),
        ],
      }),
    );

    expect(result.review).toMatchObject({
      state: "blocked",
      reason: "review-finding-blocking",
    });
  });

  it("fails closed when a trusted operational envelope is malformed", () => {
    const malformedFinding = comment(
      "malformed-finding",
      [
        "<!-- operational-evidence:v1",
        "KIND: review-finding",
        "PR: 201",
        `HEAD: ${HEAD}`,
        "RUN: malformed-review-run",
        "-->",
      ].join("\n"),
    );
    const result = reconcile(
      baseInput({
        comments: [...baseInput().comments, malformedFinding],
      }),
    );

    expect(result.review).toMatchObject({
      state: "unknown",
      reason: "missing-field",
      provenance: [
        expect.objectContaining({
          kind: "comment",
          source: expect.objectContaining({ sourceId: "malformed-finding" }),
        }),
      ],
    });
    expect(result.validations[0]?.state).toBe("satisfied");
    expect(result.mergeGate.state).toBe("unknown");
  });

  it("scopes malformed run evidence to its exact kind and requirement name", () => {
    const malformedReview = comment(
      "malformed-review",
      [
        "<!-- operational-evidence:v1",
        "KIND: review",
        "PR: 201",
        `HEAD: ${HEAD}`,
        "RUN: malformed-review-run",
        "NAME: exact-head-review",
        "-->",
      ].join("\n"),
    );
    const malformedValidation = comment(
      "malformed-validation",
      [
        "<!-- operational-evidence:v1",
        "KIND: validation",
        "PR: 201",
        `HEAD: ${HEAD}`,
        "RUN: malformed-validation-run",
        "NAME: manual-validation",
        "-->",
      ].join("\n"),
    );

    const reviewFailure = reconcile(
      baseInput({ comments: [...baseInput().comments, malformedReview] }),
    );
    const validationFailure = reconcile(
      baseInput({ comments: [...baseInput().comments, malformedValidation] }),
    );

    expect(reviewFailure.validations[0]?.state).toBe("satisfied");
    expect(reviewFailure.review).toMatchObject({
      state: "unknown",
      reason: "missing-field",
    });
    expect(validationFailure.validations[0]).toMatchObject({
      state: "unknown",
      reason: "missing-field",
    });
    expect(validationFailure.review.state).toBe("satisfied");
  });

  it("keeps stale-head and cross-PR malformed run evidence advisory", () => {
    const staleValidation = comment(
      "stale-validation",
      [
        "<!-- operational-evidence:v1",
        "KIND: validation",
        "PR: 201",
        `HEAD: ${OLD_HEAD}`,
        "RUN: stale-validation-run",
        "NAME: manual-validation",
        "-->",
      ].join("\n"),
    );
    const staleReview = comment(
      "stale-review",
      [
        "<!-- operational-evidence:v1",
        "KIND: review",
        "PR: 201",
        `HEAD: ${OLD_HEAD}`,
        "RUN: stale-review-run",
        "NAME: exact-head-review",
        "-->",
      ].join("\n"),
    );
    const crossPullRequest = comment(
      "cross-pr-validation",
      [
        "<!-- operational-evidence:v1",
        "KIND: validation",
        "PR: 202",
        `HEAD: ${HEAD}`,
        "RUN: cross-pr-run",
        "NAME: manual-validation",
        "-->",
      ].join("\n"),
    );
    const result = reconcile(
      baseInput({
        comments: [
          ...baseInput().comments,
          staleValidation,
          staleReview,
          crossPullRequest,
        ],
      }),
    );

    expect(result.validations[0]?.state).toBe("satisfied");
    expect(result.review.state).toBe("satisfied");
    expect(result.mergeGate.state).toBe("satisfied");
    expect(result.advisories).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ reason: "stale-structured-evidence" }),
      ]),
    );
  });

  it("keeps unscoped and unrelated malformed envelopes advisory", () => {
    const unrelated = comment(
      "unrelated-validation",
      [
        "<!-- operational-evidence:v1",
        "KIND: validation",
        "PR: 201",
        `HEAD: ${HEAD}`,
        "RUN: unrelated-run",
        "NAME: another-validation",
        "-->",
      ].join("\n"),
    );
    const unscoped = comment(
      "unscoped-review",
      [
        "<!-- operational-evidence:v1",
        "KIND: review",
        "PR: 201",
        `HEAD: ${HEAD}`,
        "RUN: unscoped-run",
        "-->",
      ].join("\n"),
    );
    const result = reconcile(
      baseInput({ comments: [...baseInput().comments, unrelated, unscoped] }),
    );

    expect(result.validations[0]?.state).toBe("satisfied");
    expect(result.review.state).toBe("satisfied");
    expect(result.mergeGate.state).toBe("satisfied");
    expect(result.advisories).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ reason: "invalid-structured-source" }),
      ]),
    );
  });

  it("keeps native approval authoritative over a malformed review attestation", () => {
    const malformedReview = comment(
      "malformed-review",
      [
        "<!-- operational-evidence:v1",
        "KIND: review",
        "PR: 201",
        `HEAD: ${HEAD}`,
        "RUN: malformed-review-run",
        "NAME: exact-head-review",
        "-->",
      ].join("\n"),
    );
    const approval: NativeReview = {
      current: true,
      head: HEAD,
      state: "APPROVED",
      source: { id: "approval-current" },
    };
    const result = reconcile(
      baseInput({
        comments: [...baseInput().comments, malformedReview],
        nativeReviews: complete([approval]),
      }),
    );

    expect(result.review).toMatchObject({
      state: "satisfied",
      reason: "native-approval-current",
    });
  });

  it("makes standalone P3 findings advisory and keeps unclassified runs Unknown", () => {
    const withP3 = reconcile(
      baseInput({
        comments: [
          handoff(),
          watch(),
          validation("pass", "pass"),
          review("clean", "clean"),
          finding("finding-p3", "P3", "standalone-review-run"),
        ],
      }),
    );
    const unclassified = reconcile(
      baseInput({
        comments: [
          handoff(),
          watch(),
          validation("pass", "pass"),
          review("findings", "findings", { run: "review-run" }),
        ],
      }),
    );

    expect(withP3.review).toMatchObject({
      state: "satisfied",
      reason: "review-clean-current",
    });
    expect(withP3.advisories).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ reason: "review-finding-p3" }),
      ]),
    );
    expect(unclassified.review).toMatchObject({
      state: "unknown",
      reason: "review-findings-unclassified",
    });
  });

  it("keeps prose and cache inputs advisory and is independent of source order", () => {
    const prose = comment(
      "prose",
      "Ready. HOLD. validation passed. validation failed. P1 resolved. merge-ready.",
      { trustedProducer: false },
    );
    const input = baseInput({
      comments: [...baseInput().comments, prose],
      advisories: [
        { kind: "cache", source: { id: "old-cache" } },
        { kind: "prose", source: { id: "status-prose" } },
      ],
    });
    const forward = reconcile(input);
    const reverse = reconcile({ ...input, comments: [...input.comments].reverse() });

    expect(reverse).toEqual(forward);
    expect(forward.mergeGate.state).toBe("satisfied");
    for (const gate of [
      forward.handoff,
      forward.watch,
      ...forward.validations,
      forward.review,
      forward.humanAction,
      forward.mutationGate,
      forward.mergeGate,
    ]) {
      expect(gate).toMatchObject({ observedHead: HEAD, observedMain: MAIN });
    }
    expect(forward.advisories).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ reason: "cached-observation-stale" }),
        expect.objectContaining({ reason: "unstructured-prose" }),
      ]),
    );
  });

  it("orders provenance by fixed ordinal code units instead of host collation", () => {
    const result = reconcile(
      baseInput({
        nativeChecks: complete([
          {
            name: "manual-validation",
            head: HEAD,
            status: "failure",
            source: { id: "a-check" },
          },
          {
            name: "manual-validation",
            head: HEAD,
            status: "failure",
            source: { id: "Z-check" },
          },
        ]),
      }),
    );

    expect(
      result.validations[0]?.provenance.map((value) =>
        value.kind === "native-check" ? value.id : "",
      ),
    ).toEqual(["Z-check", "a-check"]);
  });

  it("deduplicates an exact source repeat but fails closed on a reused ID", () => {
    const pass = validation("same-source", "pass");
    const exactRepeat = reconcile(
      baseInput({ comments: [...baseInput().comments, pass, pass] }),
    );
    const reused = reconcile(
      baseInput({
        comments: [
          handoff(),
          watch(),
          pass,
          validation("same-source", "fail"),
          review("clean", "clean"),
        ],
      }),
    );

    expect(exactRepeat.validations[0]?.state).toBe("satisfied");
    expect(reused.validations[0]).toMatchObject({
      state: "unknown",
      reason: "source-identity-conflict",
    });
  });
});
