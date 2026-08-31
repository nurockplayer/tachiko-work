import { describe, expect, it } from "vitest";

import type {
  CommentSourceMetadata,
  EvidenceParseContext,
  StructuredCommentSource,
} from "../src/model.js";
import {
  parseAgentHandoff,
  parseOperationalEvidence,
  parseStewardWatch,
} from "../src/parse.js";

const HEAD = "a".repeat(40);
const MAIN = "b".repeat(40);
const OLD_HEAD = "c".repeat(40);

const context: EvidenceParseContext = {
  repository: "nurockplayer/tachiko-work",
  issueNumber: 200,
  pullRequestNumber: 202,
  owner: "agent:codex",
  headSha: HEAD,
  mainSha: MAIN,
};

const baseMetadata: CommentSourceMetadata = {
  repository: "nurockplayer/tachiko-work",
  id: "IC_kwDOExample",
  kind: "issue-comment",
  authorLogin: "trusted-agent",
  authorAssociation: "MEMBER",
  url: "https://github.com/nurockplayer/tachiko-work/pull/202#issuecomment-1",
  createdAt: "2026-08-31T10:00:00Z",
  updatedAt: null,
  edited: false,
  topLevel: true,
  trustedProducer: true,
};

function comment(
  body: string,
  overrides: Partial<CommentSourceMetadata> = {},
): StructuredCommentSource {
  return { body, metadata: { ...baseMetadata, ...overrides } };
}

function handoff(overrides: Record<string, string> = {}): string {
  const values = {
    ISSUE: "200",
    PR: "202",
    OWNER: "agent:codex",
    STATE: "merge-ready",
    HEAD,
    MAIN,
    ...overrides,
  };
  return [
    "<!-- agent-handoff:v1 -->",
    `ISSUE: ${values.ISSUE}`,
    `PR: ${values.PR}`,
    `OWNER: ${values.OWNER}`,
    `STATE: ${values.STATE}`,
    `HEAD: ${values.HEAD}`,
    `MAIN: ${values.MAIN}`,
    "",
    "Narrative says failed, HOLD, P1, resolved, and merge-ready.",
  ].join("\n");
}

function watch(overrides: Record<string, string> = {}): string {
  const values = {
    VERDICT: "GREEN",
    HEAD,
    MAIN,
    HUMAN_ACTION: "none",
    ...overrides,
  };
  return [
    "<!-- project-steward-watch:v1 -->",
    `VERDICT: ${values.VERDICT}`,
    `HEAD: ${values.HEAD}`,
    `MAIN: ${values.MAIN}`,
    `HUMAN_ACTION: ${values.HUMAN_ACTION}`,
    "",
    "Advisory prose: HOLD and human action required.",
  ].join("\n");
}

function envelope(lines: readonly string[], narrative = "Ready; passed; P1 resolved."): string {
  return ["<!-- operational-evidence:v1", ...lines, "-->", narrative].join("\n");
}

describe("parseAgentHandoff", () => {
  it("parses the canonical contiguous header and preserves provenance", () => {
    const result = parseAgentHandoff(comment(handoff()), context);

    expect(result).toMatchObject({
      ok: true,
      value: {
        kind: "agent-handoff",
        issue: 200,
        pullRequest: 202,
        owner: "agent:codex",
        state: "merge-ready",
        head: HEAD,
        main: MAIN,
        source: {
          sourceId: "IC_kwDOExample",
          observedPullRequest: 202,
          observedHead: HEAD,
          observedMain: MAIN,
        },
      },
    });
  });

  it("accepts CRLF and a PATCH-edited canonical handoff", () => {
    const result = parseAgentHandoff(
      comment(handoff().replaceAll("\n", "\r\n"), {
        edited: true,
        updatedAt: "2026-08-31T10:10:00Z",
      }),
      context,
    );
    expect(result.ok).toBe(true);
  });

  it.each([
    ["duplicate", `${handoff()}\n<!-- agent-handoff:v1 -->`, "marker-duplicate"],
    ["not first", `preface\n${handoff()}`, "marker-not-first-line"],
    [
      "duplicate field",
      handoff().replace(`PR: 202`, `PR: 202\nPR: 202`),
      "duplicate-field",
    ],
    [
      "unknown field",
      handoff().replace(`STATE: merge-ready`, `PHASE: review\nSTATE: merge-ready`),
      "unknown-field",
    ],
    [
      "alias",
      handoff().replace(`STATE: merge-ready`, `STATUS: merge-ready`),
      "unknown-field",
    ],
    ["missing field", handoff().replace(`STATE: merge-ready\n`, ""), "missing-field"],
    ["malformed field", handoff().replace(`PR: 202`, `PR:202`), "malformed-line"],
    ["leading-zero integer", handoff({ ISSUE: "0200" }), "invalid-positive-integer"],
    ["zero integer", handoff({ PR: "0" }), "invalid-positive-integer"],
    [
      "unsafe integer",
      handoff({ PR: "9007199254740992" }),
      "invalid-positive-integer",
    ],
    ["abbreviated head", handoff({ HEAD: HEAD.slice(0, 12) }), "invalid-sha"],
    ["uppercase head", handoff({ HEAD: HEAD.toUpperCase() }), "invalid-sha"],
    ["issue mismatch", handoff({ ISSUE: "201" }), "identity-mismatch"],
    ["PR mismatch", handoff({ PR: "203" }), "identity-mismatch"],
    ["owner mismatch", handoff({ OWNER: "agent:other" }), "identity-mismatch"],
    ["head mismatch", handoff({ HEAD: OLD_HEAD }), "head-mismatch"],
    ["main mismatch", handoff({ MAIN: OLD_HEAD }), "main-mismatch"],
    ["non-token state", handoff({ STATE: "ready / merge" }), "invalid-canonical-token"],
  ])("rejects %s", (_name, body, reason) => {
    expect(parseAgentHandoff(comment(body), context)).toMatchObject({ ok: false, reason });
  });

  it("requires an authorized top-level producer only after finding the marker", () => {
    expect(
      parseAgentHandoff(comment(handoff(), { trustedProducer: false }), context),
    ).toMatchObject({ ok: false, reason: "producer-untrusted" });
    expect(parseAgentHandoff(comment(handoff(), { topLevel: false }), context)).toMatchObject({
      ok: false,
      reason: "source-not-top-level",
    });
    expect(
      parseAgentHandoff(
        comment("Quoted prose says agent-handoff:v1 and merge-ready.", {
          trustedProducer: false,
        }),
        context,
      ),
    ).toMatchObject({ ok: false, reason: "marker-missing" });
  });

  it("binds a trusted source to the enclosing repository identity", () => {
    expect(
      parseAgentHandoff(
        comment(handoff(), { repository: "someone-else/tachiko-work" }),
        context,
      ),
    ).toMatchObject({ ok: false, reason: "identity-mismatch" });
  });

  it("requires the header to start immediately and treats narrative after a blank as advisory", () => {
    expect(
      parseAgentHandoff(
        comment(`<!-- agent-handoff:v1 -->\n\n${handoff().split("\n").slice(1).join("\n")}`),
        context,
      ),
    ).toMatchObject({ ok: false, reason: "header-empty" });

    const result = parseAgentHandoff(
      comment(handoff().replace("Narrative says", "STATUS: failed\nNarrative says")),
      context,
    );
    expect(result.ok).toBe(true);
  });
});

describe("parseStewardWatch", () => {
  it.each([
    ["GREEN", "none"],
    ["AMBER", "required"],
    ["HOLD", "required"],
  ] as const)("parses %s with HUMAN_ACTION %s", (verdict, humanAction) => {
    const result = parseStewardWatch(
      comment(watch({ VERDICT: verdict, HUMAN_ACTION: humanAction }), { edited: true }),
      context,
    );
    expect(result).toMatchObject({
      ok: true,
      value: { verdict, humanAction, head: HEAD, main: MAIN },
    });
  });

  it.each([
    ["unknown field", watch().replace("HEAD:", "SCOPE: ok\nHEAD:"), "unknown-field"],
    [
      "duplicate",
      watch().replace(`MAIN: ${MAIN}`, `MAIN: ${MAIN}\nMAIN: ${MAIN}`),
      "duplicate-field",
    ],
    ["bad verdict", watch({ VERDICT: "green" }), "invalid-enum"],
    ["bad action", watch({ HUMAN_ACTION: "yes" }), "invalid-enum"],
    ["old head", watch({ HEAD: OLD_HEAD }), "head-mismatch"],
    ["old main", watch({ MAIN: OLD_HEAD }), "main-mismatch"],
  ])("rejects %s", (_name, body, reason) => {
    expect(parseStewardWatch(comment(body), context)).toMatchObject({ ok: false, reason });
  });
});

describe("parseOperationalEvidence", () => {
  it.each([
    [
      "validation",
      [
        "KIND: validation",
        "PR: 202",
        `HEAD: ${HEAD}`,
        "RUN: local-20260831-1",
        "NAME: release-check",
        "RESULT: pass",
        "SUPERSEDES: IC_prior",
      ],
      { kind: "validation", result: "pass", supersedes: "IC_prior" },
    ],
    [
      "review",
      [
        "KIND: review",
        "PR: 202",
        `HEAD: ${HEAD}`,
        "RUN: review-7",
        "NAME: exact-head-review",
        "RESULT: clean",
      ],
      { kind: "review", result: "clean" },
    ],
    [
      "review finding",
      [
        "KIND: review-finding",
        "PR: 202",
        `HEAD: ${HEAD}`,
        "RUN: review-7",
        "SEVERITY: P2",
      ],
      { kind: "review-finding", severity: "P2" },
    ],
    [
      "review resolution",
      [
        "KIND: review-resolution",
        "PR: 202",
        `HEAD: ${HEAD}`,
        "RESOLVES: IC_finding",
      ],
      { kind: "review-resolution", resolves: "IC_finding" },
    ],
  ] as const)("parses a closed %s envelope", (_name, lines, expected) => {
    expect(parseOperationalEvidence(comment(envelope(lines)), context)).toMatchObject({
      ok: true,
      value: expected,
    });
  });

  it("accepts an old-head envelope so reconciliation can mark it stale", () => {
    const result = parseOperationalEvidence(
      comment(
        envelope([
          "KIND: validation",
          "PR: 202",
          `HEAD: ${OLD_HEAD}`,
          "RUN: old-run",
          "NAME: release-check",
          "RESULT: pass",
        ]),
      ),
      context,
    );
    expect(result).toMatchObject({ ok: true, value: { head: OLD_HEAD, result: "pass" } });
  });

  it.each([
    [
      "edited source",
      envelope([
        "KIND: review-finding",
        "PR: 202",
        `HEAD: ${HEAD}`,
        "RUN: review-7",
        "SEVERITY: P1",
      ]),
      { edited: true },
      "source-edited",
    ],
    [
      "unauthorized source",
      envelope([
        "KIND: review-finding",
        "PR: 202",
        `HEAD: ${HEAD}`,
        "RUN: review-7",
        "SEVERITY: P1",
      ]),
      { trustedProducer: false },
      "producer-untrusted",
    ],
    [
      "wrong PR",
      envelope([
        "KIND: review-resolution",
        "PR: 203",
        `HEAD: ${HEAD}`,
        "RESOLVES: IC_finding",
      ]),
      {},
      "identity-mismatch",
    ],
    [
      "unknown kind field",
      envelope([
        "KIND: review-finding",
        "PR: 202",
        `HEAD: ${HEAD}`,
        "RUN: review-7",
        "SEVERITY: P1",
        "NAME: forbidden-here",
      ]),
      {},
      "unknown-field",
    ],
    [
      "incompatible validation result",
      envelope([
        "KIND: validation",
        "PR: 202",
        `HEAD: ${HEAD}`,
        "RUN: run-1",
        "NAME: release-check",
        "RESULT: clean",
      ]),
      {},
      "incompatible-result",
    ],
    [
      "abbreviated SHA",
      envelope([
        "KIND: review",
        "PR: 202",
        `HEAD: ${HEAD.slice(0, 12)}`,
        "RUN: run-1",
        "NAME: review",
        "RESULT: clean",
      ]),
      {},
      "invalid-sha",
    ],
    [
      "duplicate field",
      envelope([
        "KIND: review-resolution",
        "PR: 202",
        "PR: 202",
        `HEAD: ${HEAD}`,
        "RESOLVES: IC_finding",
      ]),
      {},
      "duplicate-field",
    ],
  ] as const)("rejects %s", (_name, body, metadata, reason) => {
    expect(parseOperationalEvidence(comment(body, metadata), context)).toMatchObject({
      ok: false,
      reason,
    });
  });

  it("requires exactly one first-line, exactly closed envelope", () => {
    const valid = envelope([
      "KIND: review-resolution",
      "PR: 202",
      `HEAD: ${HEAD}`,
      "RESOLVES: IC_finding",
    ]);
    expect(parseOperationalEvidence(comment(`preface\n${valid}`), context)).toMatchObject({
      ok: false,
      reason: "marker-not-first-line",
    });
    expect(
      parseOperationalEvidence(comment(`${valid}\n${valid}`), context),
    ).toMatchObject({ ok: false, reason: "multiple-envelopes" });
    expect(
      parseOperationalEvidence(comment(valid.replace("\n-->", "\n-- >")), context),
    ).toMatchObject({ ok: false, reason: "envelope-unclosed" });
  });

  it("does not treat marker words in ordinary untrusted prose as evidence", () => {
    expect(
      parseOperationalEvidence(
        comment("Operational evidence passed; review P1 resolved and merge-ready.", {
          trustedProducer: false,
        }),
        context,
      ),
    ).toMatchObject({ ok: false, reason: "marker-missing" });
  });
});
