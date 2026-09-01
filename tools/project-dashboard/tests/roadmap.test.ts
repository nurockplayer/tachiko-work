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
    "##\tLater section",
    "##",
    " ## Later section",
    "# Replacement document",
    "Later section\n---",
    "Later section\n===",
  ])(
    "does not read a horizon value beyond the next CommonMark H2 form %j",
    (nextHeading) => {
      expect(
        parseProductHorizon(
          `## Current horizon\n\n${nextHeading}\n\n> **NOT CURRENT**`,
          "https://github.example/roadmap",
        ),
      ).toMatchObject({ state: "unknown", value: "Unknown" });
    },
  );

  it.each([
    "<script>\n## Current horizon\n\n> **FAKE**\n</script>",
    "<details>\n## Current horizon\n> **FAKE**\n\n",
  ])("resumes authority parsing after a bounded raw HTML block", (rawBlock) => {
    expect(
      parseProductHorizon(
        `${rawBlock}\n## Current horizon\n\n> **06 · Team Workspace Beta**`,
        "https://github.example/roadmap",
      ),
    ).toMatchObject({ state: "satisfied", value: "06 · Team Workspace Beta" });
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
      "backtick fence with a comment-like info string",
      "```<!-- -->\n## Current horizon\n\n> **06 · Team Workspace Beta**\n```",
    ],
    [
      "tilde fence with a comment-like info string",
      "~~~<!-- -->\n## Current horizon\n\n> **06 · Team Workspace Beta**\n~~~",
    ],
    [
      "HTML-commented example",
      "<!--\n## Current horizon\n\n> **06 · Team Workspace Beta**\n-->",
    ],
    [
      "script-block example",
      "<script>\n## Current horizon\n\n> **06 · Team Workspace Beta**\n</script>",
    ],
    [
      "script-block opener with a comment",
      "<script><!-- -->\n## Current horizon\n\n> **06 · Team Workspace Beta**\n</script>",
    ],
    [
      "script-block pseudo-close with whitespace",
      "<script>\n</script >\n## Current horizon\n\n> **06 · Team Workspace Beta**\n</script>",
    ],
    [
      "pre-block example",
      "<PRE class=example>\n## Current horizon\n\n> **06 · Team Workspace Beta**\n</PRE>",
    ],
    [
      "block-tag example",
      "<details>\n## Current horizon\n\n> **06 · Team Workspace Beta**\n</details>",
    ],
    [
      "self-closing block-tag example with trailing content",
      "<div/> trailing\n## Current horizon\n\n> **06 · Team Workspace Beta**\n\n",
    ],
    [
      "custom-tag example",
      "<roadmap-example>\n## Current horizon\n\n> **06 · Team Workspace Beta**\n\n",
    ],
    [
      "custom-tag example with a quoted greater-than attribute",
      "<roadmap-example data=\">\">\n## Current horizon\n\n> **06 · Team Workspace Beta**\n\n",
    ],
    [
      "processing-instruction example",
      "<?example\n## Current horizon\n\n> **06 · Team Workspace Beta**\n?>",
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
