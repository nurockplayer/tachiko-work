import { describe, expect, it } from "vitest";

import { parseProductHorizon } from "../src/server/roadmap.js";

describe("parseProductHorizon", () => {
  it("derives the one bounded current-horizon block with provenance", () => {
    const result = parseProductHorizon(
      [
        "# Roadmap",
        "",
        "## Current horizon",
        "",
        "The current repository planning horizon is:",
        "",
        "> **06 · Team Workspace Beta**",
        "",
        "## Product stages",
      ].join("\n"),
      "https://github.example/roadmap",
    );

    expect(result).toEqual({
      state: "satisfied",
      value: "06 · Team Workspace Beta",
      source: {
        label: "Product Roadmap",
        url: "https://github.example/roadmap",
        evidenceClass: "direct",
      },
    });
  });

  it.each([
    ["missing", "# Roadmap\n\n## Future"],
    ["wrong heading depth", "### Current horizon\n\n> **06 · Team Workspace Beta**"],
    ["inline prose", "Prefix ## Current horizon\n\n> **06 · Team Workspace Beta**"],
    [
      "fenced example",
      "```md\n## Current horizon\n\n> **06 · Team Workspace Beta**\n```",
    ],
    [
      "shorter backtick pseudo-close",
      "````md\n```\n## Current horizon\n\n> **06 · Team Workspace Beta**\n````",
    ],
    [
      "suffixed backtick pseudo-close",
      "````md\n````not-a-close\n## Current horizon\n\n> **06 · Team Workspace Beta**\n````",
    ],
    [
      "shorter tilde pseudo-close",
      "~~~~md\n~~~\n## Current horizon\n\n> **06 · Team Workspace Beta**\n~~~~",
    ],
    [
      "suffixed tilde pseudo-close",
      "~~~~md\n~~~~not-a-close\n## Current horizon\n\n> **06 · Team Workspace Beta**\n~~~~",
    ],
    [
      "HTML-commented example",
      "<!--\n## Current horizon\n\n> **06 · Team Workspace Beta**\n-->",
    ],
    [
      "ambiguous",
      "## Current horizon\n\n> **06 · Team Workspace Beta**\n> **07 · Migration**",
    ],
  ])("fails closed when the authority block is %s", (_name, markdown) => {
    expect(parseProductHorizon(markdown, "https://github.example/roadmap")).toEqual({
      state: "unknown",
      value: "Unknown",
      source: {
        label: "Product Roadmap",
        url: "https://github.example/roadmap",
        evidenceClass: "direct",
      },
    });
  });
});
