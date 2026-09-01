import type { DisplayValue, SourceLink } from "../shared/model.js";

const CURRENT_HORIZON_HEADING = /^## Current horizon[ \t]*$/gm;
const NEXT_SECTION =
  /^ {0,3}#{1,2}(?:[ \t]|$)|^ {0,3}(?:=+|-+)[ \t]*$/m;
const HORIZON_LINE = /^> \*\*([^*\n]+)\*\*$/gm;
const RAW_HTML_TAG =
  /^(?:address|article|aside|base|basefont|blockquote|body|caption|center|col|colgroup|dd|details|dialog|dir|div|dl|dt|fieldset|figcaption|figure|footer|form|frame|frameset|h[1-6]|head|header|hr|html|iframe|legend|li|link|main|menu|menuitem|nav|noframes|ol|optgroup|option|p|param|search|section|summary|table|tbody|td|tfoot|th|thead|title|tr|track|ul)$/i;
const COMPLETE_HTML_OPEN_TAG =
  /^ {0,3}<([A-Za-z][A-Za-z0-9-]*)(?:[ \t]+[A-Za-z_:][A-Za-z0-9_.:-]*(?:[ \t]*=[ \t]*(?:[^ "'=<>`]+|'[^']*'|"[^"]*"))?)*[ \t]*\/?>[ \t]*$/;
const COMPLETE_HTML_CLOSING_TAG =
  /^ {0,3}<\/[A-Za-z][A-Za-z0-9-]*[ \t]*>[ \t]*$/;
const TYPE_ONE_HTML_TAG = /^(?:script|pre|style|textarea)$/i;

type HtmlBlock = { end: RegExp | "blank" };

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

  const typeSevenOpen = COMPLETE_HTML_OPEN_TAG.exec(line)?.[1];
  return (typeSevenOpen !== undefined && !TYPE_ONE_HTML_TAG.test(typeSevenOpen)) ||
    COMPLETE_HTML_CLOSING_TAG.test(line)
    ? { end: "blank" }
    : null;
}

function withoutNonAuthorityBlocks(markdown: string): string {
  let fence: { kind: "`" | "~"; length: number } | null = null;
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
          return "";
        }
        if (htmlComment) {
          if (line.includes("-->")) htmlComment = false;
          return "";
        }
        const fenceStart = /^ {0,3}(`{3,}|~{3,})(.*)$/.exec(line);
        const marker = fenceStart?.[1];
        const suffix = fenceStart?.[2] ?? "";
        if (marker !== undefined && (marker[0] === "~" || !suffix.includes("`"))) {
          fence = { kind: marker[0] as "`" | "~", length: marker.length };
          return "";
        }
        const rawHtml = htmlBlockFrom(line);
        if (rawHtml !== null) {
          if (rawHtml.end === "blank" || !rawHtml.end.test(line)) htmlBlock = rawHtml;
          return "";
        }
        const commentStart = /^ {0,3}<!--/.exec(line)?.index;
        if (commentStart !== undefined) {
          if (line.indexOf("-->", commentStart + 2) < 0) htmlComment = true;
          return "";
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
      return "";
    })
    .join("\n");
}

function horizonSection(markdown: string, headingIndex: number, headingLength: number): string {
  const remainder = markdown.slice(headingIndex + headingLength);
  const nextSectionIndex = remainder.search(NEXT_SECTION);
  return nextSectionIndex < 0 ? remainder : remainder.slice(0, nextSectionIndex);
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
  const rawSection = horizonSection(
    normalizedMarkdown,
    rawHeading.index,
    rawHeading[0].length,
  );
  if ([...rawSection.matchAll(HORIZON_LINE)].length !== 1) {
    return { state: "unknown", value: "Unknown", source };
  }

  const authorityMarkdown = withoutNonAuthorityBlocks(normalizedMarkdown);
  const headings = [...authorityMarkdown.matchAll(CURRENT_HORIZON_HEADING)];
  const heading = headings.length === 1 ? headings[0] : undefined;
  if (heading?.index === undefined) {
    return { state: "unknown", value: "Unknown", source };
  }

  const section = horizonSection(authorityMarkdown, heading.index, heading[0].length);
  const matches = [...section.matchAll(HORIZON_LINE)].map((match) => match[1]?.trim());
  const horizon = matches.length === 1 ? matches[0] : undefined;
  return horizon === undefined || horizon.length === 0
    ? { state: "unknown", value: "Unknown", source }
    : { state: "satisfied", value: horizon, source };
}
