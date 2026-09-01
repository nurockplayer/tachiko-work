import type { DisplayValue, SourceLink } from "../shared/model.js";

const CURRENT_HORIZON_HEADING = /^## Current horizon[ \t]*$/gm;
const NEXT_SECTION = /^## /m;
const HORIZON_LINE = /^> \*\*([^*\n]+)\*\*$/gm;

function withoutNonAuthorityBlocks(markdown: string): string {
  let fence: { kind: "`" | "~"; length: number } | null = null;
  let htmlComment = false;
  return markdown
    .split("\n")
    .map((line) => {
      if (fence === null) {
        if (htmlComment) {
          if (line.includes("-->")) htmlComment = false;
          return "";
        }
        const commentStart = line.indexOf("<!--");
        if (commentStart >= 0) {
          if (line.indexOf("-->", commentStart + 4) < 0) htmlComment = true;
          return "";
        }
        const marker = /^ {0,3}(`{3,}|~{3,})/.exec(line)?.[1];
        if (marker === undefined) return line;
        fence = { kind: marker[0] as "`" | "~", length: marker.length };
        return "";
      }
      const closing = /^ {0,3}(`+|~+)[ \t]*$/.exec(line)?.[1];
      if (
        closing !== undefined &&
        closing[0] === fence.kind &&
        closing.length >= fence.length
      ) {
        fence = null;
      }
      return "";
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
  const authorityMarkdown = withoutNonAuthorityBlocks(markdown);
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
