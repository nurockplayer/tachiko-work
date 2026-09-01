import type { DisplayValue, SourceLink } from "../shared/model.js";

const CURRENT_HORIZON_HEADING = "## Current horizon";
const NEXT_SECTION = /^## /m;
const HORIZON_LINE = /^> \*\*([^*\n]+)\*\*$/gm;

export function parseProductHorizon(
  markdown: string,
  url: string,
): DisplayValue<string> {
  const source: SourceLink = {
    label: "Product Roadmap",
    url,
    evidenceClass: "direct",
  };
  const headingIndex = markdown.indexOf(CURRENT_HORIZON_HEADING);
  if (
    headingIndex < 0 ||
    markdown.indexOf(CURRENT_HORIZON_HEADING, headingIndex + 1) >= 0
  ) {
    return { state: "unknown", value: "Unknown", source };
  }

  const remainder = markdown.slice(headingIndex + CURRENT_HORIZON_HEADING.length);
  const nextSectionIndex = remainder.search(NEXT_SECTION);
  const section = nextSectionIndex < 0 ? remainder : remainder.slice(0, nextSectionIndex);
  const matches = [...section.matchAll(HORIZON_LINE)].map((match) => match[1]?.trim());
  const horizon = matches.length === 1 ? matches[0] : undefined;
  return horizon === undefined || horizon.length === 0
    ? { state: "unknown", value: "Unknown", source }
    : { state: "satisfied", value: horizon, source };
}
