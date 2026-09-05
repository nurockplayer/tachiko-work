import type { NumberFormat } from "./tracker-model.ts";

/** Bounded display classification; retain the complete source pattern for export. */
export function importedNumberFormat(pattern: string | null | undefined): NumberFormat {
  if (!pattern) return "number";
  const plain = pattern.replace(/"([^"]*)"/g, (_match: string, value: string) => /^[¥￥$]$/.test(value) ? value : "")
    .replace(/\\(.)/g, (_match: string, value: string) => /^[¥￥$]$/.test(value) ? value : "")
    .replace(/[_*]./g, "");
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
