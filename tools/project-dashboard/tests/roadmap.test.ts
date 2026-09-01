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
    "Foo\n    bar\n---",
    "``` invalid`info\n---",
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

  it.each(["\r\n", "\r"])(
    "bounds Setext sections after normalizing %j line endings",
    (ending) => {
      const markdown = [
        "## Current horizon",
        "",
        "Later section",
        "---",
        "",
        "> **NOT CURRENT**",
      ].join(ending);
      expect(parseProductHorizon(markdown, "https://github.example/roadmap")).toMatchObject({
        state: "unknown",
        value: "Unknown",
      });
    },
  );

  it.each(["\u2028", "\u2029"])(
    "rejects JavaScript-only %j line separators as authority boundaries",
    (separator) => {
      expect(
        parseProductHorizon(
          `prose${separator}## Current horizon${separator}${separator}> **FAKE**`,
          "https://github.example/roadmap",
        ),
      ).toMatchObject({ state: "unknown", value: "Unknown" });
    },
  );

  it("does not treat a thematic break as a section boundary", () => {
    expect(
      parseProductHorizon(
        "## Current horizon\n\n> **A**\n\n---\n\n> **B**",
        "https://github.example/roadmap",
      ),
    ).toMatchObject({ state: "unknown", value: "Unknown" });
  });

  it.each(["<custom>", "</custom>", "<span>", "</span>"])(
    "recognizes a Type-7 HTML paragraph continuation before a Setext boundary: %s",
    (html) => {
      expect(
        parseProductHorizon(
          `## Current horizon\n\nNext section\n${html}\n---\n\n> **NOT CURRENT**`,
          "https://github.example/roadmap",
        ),
      ).toMatchObject({ state: "unknown", value: "Unknown" });
    },
  );

  it.each([
    "2. continuation",
    "2) continuation",
    "14. continuation",
    "02. continuation",
    "0. continuation",
    "*",
    "+   ",
    "1.",
    "1)   ",
  ])(
    "recognizes a non-interrupting list continuation before a Setext boundary: %j",
    (continuation) => {
      expect(
        parseProductHorizon(
          `## Current horizon\n\nNext section\n${continuation}\n---\n\n> **NOT CURRENT**`,
          "https://github.example/roadmap",
        ),
      ).toMatchObject({ state: "unknown", value: "Unknown" });
    },
  );

  it.each(["1. item", "1) item", "* item", "+ item"])(
    "allows an interrupting list item before a thematic break: %j",
    (item) => {
      expect(
        parseProductHorizon(
          `## Current horizon\n\n> **A**\n\nParagraph\n${item}\n---`,
          "https://github.example/roadmap",
        ),
      ).toMatchObject({ state: "satisfied", value: "A" });
    },
  );

  it.each(["> **A**", "- list item"])(
    "does not treat a thematic break after block content %j as Setext",
    (content) => {
      expect(
        parseProductHorizon(
          `## Current horizon\n\n> **A**\n${content === "> **A**" ? "" : `${content}\n`}---`,
          "https://github.example/roadmap",
        ),
      ).toMatchObject({ state: "satisfied", value: "A" });
    },
  );

  it("requires the raw horizon candidate to survive filtering at the same position", () => {
    expect(
      parseProductHorizon(
        [
          "## Current horizon",
          "",
          "<script>",
          "> **HIDDEN**",
          "## Hidden section",
          "</script>",
          "> **VISIBLE**",
        ].join("\n"),
        "https://github.example/roadmap",
      ),
    ).toMatchObject({ state: "unknown", value: "Unknown" });
  });

  it.each([
    "<script>\nconst example = true;\n</script>",
    "<details>\nexample\n\n",
  ])("resumes authority parsing after a bounded raw HTML block", (rawBlock) => {
    expect(
      parseProductHorizon(
        `${rawBlock}\n## Current horizon\n\n> **06 · Team Workspace Beta**`,
        "https://github.example/roadmap",
      ),
    ).toMatchObject({ state: "satisfied", value: "06 · Team Workspace Beta" });
  });

  it.each([
    "<script/>",
    "</script >\n",
    "<![cdata[\n",
    "``` invalid`info",
    "`<!--`",
    "\\<!--",
    "<!-->",
    "<!--->",
    "paragraph continuation\n<roadmap-example>",
    "<div/foo>",
    "- item\n  ```\n  code",
    "- item\n  <script>",
    "- item\n  <!--",
  ])("does not hide conflicting authority after ordinary content %j", (content) => {
    expect(
      parseProductHorizon(
        `## Current horizon\n\n> **A**\n${content}\n## Current horizon\n\n> **B**\n-->`,
        "https://github.example/roadmap",
      ),
    ).toMatchObject({ state: "unknown", value: "Unknown" });
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
      "block-tag example with non-ASCII whitespace",
      "<details>\n\u00a0\n## Current horizon\n\n> **06 · Team Workspace Beta**\n</details>",
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
