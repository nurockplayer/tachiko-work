import type { DisplayValue, SourceLink } from "../shared/model.js";

const CURRENT_HORIZON_HEADING = /^## Current horizon[ \t]*$/gm;
const NEXT_ATX_SECTION = /^ {0,3}#{1,2}(?:[ \t]|$)/m;
const SETEXT_UNDERLINE = /^ {0,3}(?:=+|-+)[ \t]*$/;
const HORIZON_LINE = /^> \*\*([^*\n]+)\*\*$/gm;
const RAW_HTML_TAG =
  /^(?:address|article|aside|base|basefont|blockquote|body|caption|center|col|colgroup|dd|details|dialog|dir|div|dl|dt|fieldset|figcaption|figure|footer|form|frame|frameset|h[1-6]|head|header|hr|html|iframe|legend|li|link|main|menu|menuitem|nav|noframes|ol|optgroup|option|p|param|search|section|summary|table|tbody|td|tfoot|th|thead|title|tr|track|ul)$/i;
const COMPLETE_HTML_OPEN_TAG =
  /^ {0,3}<([A-Za-z][A-Za-z0-9-]*)(?:[ \t]+[A-Za-z_:][A-Za-z0-9_.:-]*(?:[ \t]*=[ \t]*(?:[^ "'=<>`]+|'[^']*'|"[^"]*"))?)*[ \t]*\/?>[ \t]*$/;
const COMPLETE_HTML_CLOSING_TAG =
  /^ {0,3}<\/([A-Za-z][A-Za-z0-9-]*)[ \t]*>[ \t]*$/;
const TYPE_ONE_HTML_TAG = /^(?:script|pre|style|textarea)$/i;

type HtmlBlock = { end: RegExp | "blank" };
type Fence = { kind: "`" | "~"; length: number };

function isTypeSevenHtmlLine(line: string): boolean {
  const openTag = COMPLETE_HTML_OPEN_TAG.exec(line)?.[1];
  if (
    openTag !== undefined &&
    !TYPE_ONE_HTML_TAG.test(openTag) &&
    !RAW_HTML_TAG.test(openTag)
  ) {
    return true;
  }
  const closingTag = COMPLETE_HTML_CLOSING_TAG.exec(line)?.[1];
  return closingTag !== undefined && !RAW_HTML_TAG.test(closingTag);
}

function blankLine(line: string): string {
  return " ".repeat(line.length);
}

function fenceStartFrom(line: string): Fence | null {
  const match = /^ {0,3}(`{3,}|~{3,})(.*)$/.exec(line);
  const marker = match?.[1];
  const suffix = match?.[2] ?? "";
  return marker !== undefined && (marker[0] === "~" || !suffix.includes("`"))
    ? { kind: marker[0] as "`" | "~", length: marker.length }
    : null;
}

function htmlBlockFrom(line: string): HtmlBlock | null {
  const typeOne = /^ {0,3}<(script|pre|style|textarea)(?:[ \t]|>|$)/i.exec(line)?.[1];
  if (typeOne !== undefined) {
    return { end: new RegExp(`</${typeOne}>`, "i") };
  }
  if (/^ {0,3}<\?/.test(line)) return { end: /\?>/ };
  if (/^ {0,3}<!\[CDATA\[/.test(line)) return { end: /\]\]>/ };
  if (/^ {0,3}<![A-Z]/.test(line)) return { end: />/ };

  const typeSix = /^ {0,3}<\/?([A-Za-z][A-Za-z0-9-]*)(?=[ \t]|\/?>|$)/.exec(
    line,
  )?.[1];
  if (typeSix !== undefined && RAW_HTML_TAG.test(typeSix)) return { end: "blank" };

  return isTypeSevenHtmlLine(line) ? { end: "blank" } : null;
}

function withoutNonAuthorityBlocks(markdown: string): string {
  let fence: Fence | null = null;
  let htmlComment = false;
  let htmlBlock: HtmlBlock | null = null;
  return markdown
    .replace(/\r\n?/g, "\n")
    .split("\n")
    .map((line) => {
      if (fence === null) {
        if (htmlBlock !== null) {
          if (
            (htmlBlock.end === "blank" && /^[ \t]*$/.test(line)) ||
            (htmlBlock.end !== "blank" && htmlBlock.end.test(line))
          ) {
            htmlBlock = null;
          }
          return blankLine(line);
        }
        if (htmlComment) {
          if (line.includes("-->")) htmlComment = false;
          return blankLine(line);
        }
        const fenceStart = fenceStartFrom(line);
        if (fenceStart !== null) {
          fence = fenceStart;
          return blankLine(line);
        }
        const rawHtml = htmlBlockFrom(line);
        if (rawHtml !== null) {
          if (rawHtml.end === "blank" || !rawHtml.end.test(line)) htmlBlock = rawHtml;
          return blankLine(line);
        }
        const commentStart = /^ {0,3}<!--/.exec(line)?.index;
        if (commentStart !== undefined) {
          if (line.indexOf("-->", commentStart + 2) < 0) htmlComment = true;
          return blankLine(line);
        }
        return line;
      }
      const closing = /^ {0,3}(`+|~+)[ \t]*$/.exec(line)?.[1];
      if (
        closing !== undefined &&
        closing[0] === fence.kind &&
        closing.length >= fence.length
      ) {
        fence = null;
      }
      return blankLine(line);
    })
    .join("\n");
}

function horizonSectionBounds(
  markdown: string,
  headingIndex: number,
  headingLength: number,
): { start: number; end: number } {
  const start = headingIndex + headingLength;
  const nextSectionIndex = markdown.slice(start).search(NEXT_ATX_SECTION);
  return { start, end: nextSectionIndex < 0 ? markdown.length : start + nextSectionIndex };
}

function hasPotentialSetextBoundary(section: string): boolean {
  const lines = section.split("\n");
  const paragraphEligible = (line: string) =>
    !/^[ \t]*$/.test(line) &&
    !/^ {0,3}(?:>|#{1,6}(?:[ \t]|$)|(?:[-+*]|\d{1,9}[.)])(?:[ \t]|$))/.test(
      line,
    ) &&
    !/^ {0,3}(?:(?:\*[ \t]*){3,}|(?:_[ \t]*){3,}|(?:-[ \t]*){3,})$/.test(line) &&
    htmlBlockFrom(line) === null &&
    fenceStartFrom(line) === null;
  return lines.some((line, index) => {
    if (index === 0 || !SETEXT_UNDERLINE.test(line)) return false;
    let contentIndex = index - 1;
    while (/^ {4}/.test(lines[contentIndex] ?? "")) contentIndex -= 1;
    const content = lines[contentIndex] ?? "";
    if (paragraphEligible(content)) return true;
    if (!isTypeSevenHtmlLine(content)) return false;
    let precedingIndex = contentIndex - 1;
    while (isTypeSevenHtmlLine(lines[precedingIndex] ?? "")) precedingIndex -= 1;
    return paragraphEligible(lines[precedingIndex] ?? "");
  });
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
  if (/[\u2028\u2029]/.test(markdown)) {
    return { state: "unknown", value: "Unknown", source };
  }
  const normalizedMarkdown = markdown.replace(/\r\n?/g, "\n");
  const rawHeadings = [...normalizedMarkdown.matchAll(CURRENT_HORIZON_HEADING)];
  const rawHeading = rawHeadings.length === 1 ? rawHeadings[0] : undefined;
  if (rawHeading?.index === undefined) {
    return { state: "unknown", value: "Unknown", source };
  }
  const rawBounds = horizonSectionBounds(
    normalizedMarkdown,
    rawHeading.index,
    rawHeading[0].length,
  );
  const rawSection = normalizedMarkdown.slice(rawBounds.start, rawBounds.end);
  const rawMatches = [...rawSection.matchAll(HORIZON_LINE)];
  const rawMatch = rawMatches.length === 1 ? rawMatches[0] : undefined;
  if (rawMatch?.index === undefined || hasPotentialSetextBoundary(rawSection)) {
    return { state: "unknown", value: "Unknown", source };
  }
  const rawHorizonIndex = rawBounds.start + rawMatch.index;

  const authorityMarkdown = withoutNonAuthorityBlocks(normalizedMarkdown);
  const headings = [...authorityMarkdown.matchAll(CURRENT_HORIZON_HEADING)];
  const heading = headings.length === 1 ? headings[0] : undefined;
  if (heading?.index === undefined || heading.index !== rawHeading.index) {
    return { state: "unknown", value: "Unknown", source };
  }

  const filteredBounds = horizonSectionBounds(
    authorityMarkdown,
    heading.index,
    heading[0].length,
  );
  const section = authorityMarkdown.slice(filteredBounds.start, filteredBounds.end);
  const matches = [...section.matchAll(HORIZON_LINE)];
  const match = matches.length === 1 ? matches[0] : undefined;
  const horizon = match?.[1]?.trim();
  return match?.index === undefined ||
    filteredBounds.start + match.index !== rawHorizonIndex ||
    hasPotentialSetextBoundary(section) ||
    horizon === undefined ||
    horizon.length === 0
    ? { state: "unknown", value: "Unknown", source }
    : { state: "satisfied", value: horizon, source };
}
