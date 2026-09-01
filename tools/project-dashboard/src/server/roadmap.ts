import type { DisplayValue, SourceLink } from "../shared/model.js";

const CURRENT_HORIZON_HEADING = /^## Current horizon[ \t]*$/gm;
const NEXT_SECTION =
  /^ {0,3}#{1,2}(?:[ \t]|$)|^ {0,3}\S.*\n {0,3}(?:=+|-+)[ \t]*$/m;
const HORIZON_LINE = /^> \*\*([^*\n]+)\*\*$/gm;
const RAW_HTML_TAG =
  /^(?:address|article|aside|base|basefont|blockquote|body|caption|center|col|colgroup|dd|details|dialog|dir|div|dl|dt|fieldset|figcaption|figure|footer|form|frame|frameset|h[1-6]|head|header|hr|html|iframe|legend|li|link|main|menu|menuitem|nav|noframes|ol|optgroup|option|p|param|search|section|summary|table|tbody|td|tfoot|th|thead|title|tr|track|ul)$/i;
const COMPLETE_HTML_TAG =
  /^ {0,3}(?:<[A-Za-z][A-Za-z0-9-]*(?:[ \t]+[A-Za-z_:][A-Za-z0-9_.:-]*(?:[ \t]*=[ \t]*(?:[^ "'=<>`]+|'[^']*'|"[^"]*"))?)*[ \t]*\/?>|<\/[A-Za-z][A-Za-z0-9-]*[ \t]*>)[ \t]*$/;

type HtmlBlock = { end: RegExp | "blank" };

function htmlBlockFrom(line: string): HtmlBlock | null {
  const start = /^ {0,3}<([^\s>]+)/.exec(line)?.[1];
  if (start === undefined) return null;
  const normalized = start.replace(/^\/|\/$/g, "").toLowerCase();
  if (["script", "pre", "style", "textarea"].includes(normalized)) {
    return { end: new RegExp(`</${normalized}>`, "i") };
  }
  if (start.startsWith("?")) return { end: /\?>/ };
  if (start.toUpperCase().startsWith("![CDATA[")) return { end: /\]\]>/ };
  if (/^![A-Z]/.test(start)) return { end: />/ };
  return RAW_HTML_TAG.test(normalized) || COMPLETE_HTML_TAG.test(line)
    ? { end: "blank" }
    : null;
}

function withoutNonAuthorityBlocks(markdown: string): string {
  let fence: { kind: "`" | "~"; length: number } | null = null;
  let htmlComment = false;
  let htmlBlock: HtmlBlock | null = null;
  return markdown
    .split("\n")
    .map((line) => {
      if (fence === null) {
        if (htmlBlock !== null) {
          if (
            (htmlBlock.end === "blank" && line.trim().length === 0) ||
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
        const marker = /^ {0,3}(`{3,}|~{3,})/.exec(line)?.[1];
        if (marker !== undefined) {
          fence = { kind: marker[0] as "`" | "~", length: marker.length };
          return "";
        }
        const rawHtml = htmlBlockFrom(line);
        if (rawHtml !== null) {
          if (rawHtml.end === "blank" || !rawHtml.end.test(line)) htmlBlock = rawHtml;
          return "";
        }
        const commentStart = line.indexOf("<!--");
        if (commentStart >= 0) {
          if (line.indexOf("-->", commentStart + 4) < 0) htmlComment = true;
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
