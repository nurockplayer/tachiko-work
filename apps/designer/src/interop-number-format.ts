import type { NumberFormat } from "./tracker-model.ts";

/** Bounded display classification; retain the complete source pattern for export. */
export function importedNumberFormat(pattern: string | null | undefined): NumberFormat {
  if (!pattern) return "number";
  const sections = numericSections(pattern);
  if (!sections) return "number";
  const kinds = sections.map(sectionFormat);
  return kinds.every(kind => kind === kinds[0]) ? kinds[0] ?? "number" : "number";
}

// Scan before classification: semicolons consumed by literals, brackets, escape,
// spacing and repetition directives do not delimit format sections.
function numericSections(pattern: string): string[] | null {
  const sections = [""];
  for (let i = 0; i < pattern.length; i++) {
    const token = pattern.charAt(i);
    let value = token;
    if (token === '"' || token === "[") {
      let end = i + 1;
      if (token === '"') {
        // Match the runtime literal boundary: escaped quotes remain literal.
        while (end < pattern.length && pattern[end] !== '"') {
          end += pattern[end] === "\\" ? 2 : 1;
        }
      } else end = pattern.indexOf("]", end);
      if (end < 0 || end >= pattern.length) return null;
      const content = pattern.slice(i + 1, end);
      if (token === "[") {
        // Conditional active-section selection is outside this display helper.
        if (/^[<=>]/.test(content.trim())) return null;
        value = `[${content}]`;
      } else value = /^[¥￥$]$/.test(content) ? content : "";
      i = end;
    } else if (token === "\\" || token === "_" || token === "*") {
      const next = pattern[++i];
      if (next === undefined) return null;
      value = token === "\\" && /^[¥￥$]$/.test(next) ? next : "";
    } else if (token === ";") {
      if (sections.length === 3) return sections; // Fourth section is Text only.
      sections.push("");
      continue;
    }
    const index = sections.length - 1;
    sections[index] = (sections[index] ?? "") + value;
  }
  return sections;
}

function sectionFormat(plain: string): NumberFormat {
  const tokens = plain.replace(/\[[^\]]*\]/g, "");
  const currencies = [...plain.matchAll(/\[\$([^\]]*)\]/g)].filter(match => {
    const token = match[1] ?? "";
    const split = token.lastIndexOf("-");
    return (split < 0 ? token : token.slice(0, split)) !== "";
  });
  if (currencies.length) {
    if (tokens.includes("%")) return "number";
    const kinds = currencies.map(match => {
      const token = match[1] ?? "";
      const split = token.lastIndexOf("-");
      const symbol = (split < 0 ? token : token.slice(0, split)).toUpperCase();
      const locale = split < 0 ? "" : token.slice(split + 1).toUpperCase();
      if (symbol === "JPY" || (["¥", "￥"].includes(symbol) && ["", "411"].includes(locale))) return "currency-jpy";
      if (symbol === "USD" || (symbol === "$" && ["", "409"].includes(locale))) return "currency-usd";
      return "number";
    });
    if (tokens.includes("¥") || tokens.includes("￥")) kinds.push("currency-jpy");
    if (tokens.includes("$")) kinds.push("currency-usd");
    return kinds.every(kind => kind === kinds[0]) ? kinds[0] ?? "number" : "number";
  }
  // Locale/color/condition brackets are not currency symbols. Quoted or escaped
  // percent signs are literals; literal dollar/yen symbols remain presentation.
  const kinds: NumberFormat[] = [];
  if (tokens.includes("%")) kinds.push("percentage");
  if (tokens.includes("¥") || tokens.includes("￥")) kinds.push("currency-jpy");
  if (tokens.includes("$")) kinds.push("currency-usd");
  return kinds.length === 1 ? kinds[0] ?? "number" : "number";
}
