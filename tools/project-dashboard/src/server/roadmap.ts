import type { DisplayValue, SourceLink } from "../shared/model.js";

const CURRENT_HORIZON_HEADING = /^## Current horizon[ \t]*$/gm;
const NEXT_SECTION = /^## /m;
const HORIZON_LINE = /^> \*\*([^*\n]+)\*\*$/gm;

function withoutFencedBlocks(markdown: string): string {
  let fence: "`" | "~" | null = null;
  return markdown
    .split("\n")
    .map((line) => {
      const marker = /^\s*(`{3,}|~{3,})/.exec(line)?.[1];
      if (marker !== undefined) {
        const kind = marker[0] as "`" | "~";
        if (fence === null) fence = kind;
        else if (fence === kind) fence = null;
        return "";
      }
      return fence === null ? line : "";
    })
    .join("\n");
}

export function parseProductHorizon(
  markdown: string,
  url: string,
): DisplayValue<string> {
  const source: SourceLink = {
    label: "Product Roadmap",
    url,
    evidenceClass: "direct",
  };
  const authorityMarkdown = withoutFencedBlocks(markdown);
  const headings = [...authorityMarkdown.matchAll(CURRENT_HORIZON_HEADING)];
  const heading = headings.length === 1 ? headings[0] : undefined;
  if (heading?.index === undefined) {
    return { state: "unknown", value: "Unknown", source };
  }

  const remainder = authorityMarkdown.slice(heading.index + heading[0].length);
  const nextSectionIndex = remainder.search(NEXT_SECTION);
  const section = nextSectionIndex < 0 ? remainder : remainder.slice(0, nextSectionIndex);
  const matches = [...section.matchAll(HORIZON_LINE)].map((match) => match[1]?.trim());
  const horizon = matches.length === 1 ? matches[0] : undefined;
  return horizon === undefined || horizon.length === 0
    ? { state: "unknown", value: "Unknown", source }
    : { state: "satisfied", value: horizon, source };
}
