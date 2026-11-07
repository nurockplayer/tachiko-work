// Generates `display_text_corpus.json`: the golden text every Runtime
// `format_number` result must match. Run with Node from this directory:
//
//   node display_text_corpus.mjs
//
// The two functions below are copied verbatim from Tachiko Sheet's
// `src/ui/field-display.ts` (`formatNumberFor` and `formatNumber`); Node's
// `Intl` is then the authority for every expected string. Commit both files.

import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const numberFormatterCache = new Map();

export function formatNumberFor(value, format) {
  if (!format) return formatNumber(value);
  const key = JSON.stringify(format);
  let formatter = numberFormatterCache.get(key);
  if (!formatter) {
    try {
      formatter = new Intl.NumberFormat("en-US", {
        minimumFractionDigits: format.decimals,
        maximumFractionDigits: format.decimals,
        useGrouping: format.thousands_separator,
        style: format.kind.type === "currency" ? "currency" : format.kind.type === "percent" ? "percent" : "decimal",
        ...(format.kind.type === "currency" ? { currency: format.kind.code } : {}),
      });
    } catch {
      return formatNumber(value);
    }
    numberFormatterCache.set(key, formatter);
  }
  return formatter.format(value);
}

/**
 * A number without a format, shown to at most 15 significant digits as other
 * spreadsheets do: a binary sum such as 1455.7999999999997 reads 1455.8. Work's
 * value is unchanged; stored values seed the cell editor exactly.
 */
export function formatNumber(value) {
  if (!Number.isFinite(value)) return String(value);
  // Rounding near the float limit can overflow; then show the exact value.
  const rounded = Number(value.toPrecision(15));
  return String(Number.isFinite(rounded) ? rounded : value);
}

// The 23 codes whose en-US CLDR currency symbol is not the code itself, as
// Node's `Intl` reports them. Every other three-letter code renders as the
// code plus U+00A0.
const CURRENCY_SYMBOLS = [
  "AUD", "BRL", "CAD", "CNY", "EUR", "GBP", "HKD", "ILS", "INR", "JPY",
  "KRW", "MXN", "NZD", "PHP", "TWD", "USD", "VND", "XAF", "XCD", "XCG",
  "XOF", "XPF", "XXX",
];
// One valid code with no en-US symbol: the code plus U+00A0.
const NO_SYMBOL_CODE = "CHF";

const VALUES = [
  0, -0, 1, -1, 0.5, 1.005, 0.125, 2.5, 1234.5, 1234.505, -1234.5, -0.001,
  1455.7999999999997, 999.995, 1e15, 1e21, 1e-7, Number.MAX_VALUE,
  Number.MIN_VALUE, 1e23,
  // Two equally short spellings: JavaScript takes the even one.
  100000000000000.125, 100000000000000.375, 100000000000000.625,
  100000000000000.875, 1125899906842624.25, 1125899906842624.75,
  1125899906842625.25, 1125899906842625.75,
];

const plain = (decimals, thousands_separator) => ({
  decimals,
  kind: { type: "plain" },
  thousands_separator,
});
const percent = (decimals, thousands_separator) => ({
  decimals,
  kind: { type: "percent" },
  thousands_separator,
});
const currency = (code, decimals, thousands_separator) => ({
  decimals,
  kind: { type: "currency", code },
  thousands_separator,
});

/** mulberry32: a deterministic 32-bit PRNG, so the random cases are stable. */
function seededRandom(seed) {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) | 0;
    let value = Math.imul(state ^ (state >>> 15), 1 | state);
    value = (value + Math.imul(value ^ (value >>> 7), 61 | value)) ^ value;
    return ((value ^ (value >>> 14)) >>> 0) / 4294967296;
  };
}

/** `String(value)` plus a spelling that keeps the negative zero's sign. */
function spelling(value) {
  return Object.is(value, -0) ? "-0" : String(value);
}

const cases = [];
const seen = new Set();

function add(value, format) {
  const caseOf = {
    value: spelling(value),
    format,
    expected: formatNumberFor(value, format ?? undefined),
  };
  const key = JSON.stringify(caseOf);
  if (seen.has(key)) return;
  seen.add(key);
  cases.push(caseOf);
}

// Every value against one representative of each format kind.
for (const value of VALUES) {
  add(value, null);
  add(value, plain(2, true));
  add(value, plain(2, false));
  add(value, percent(2, true));
  add(value, percent(2, false));
  add(value, currency("USD", 2, true));
  add(value, currency("USD", 2, false));
  add(value, currency(NO_SYMBOL_CODE, 2, true));
}

// Every admitted fraction-digit count, with grouping both ways, on values
// where rounding and the tie rule are visible.
for (const value of [0.125, 1.005, 1234.505, 999.995]) {
  for (const group of [true, false]) {
    for (let decimals = 0; decimals <= 10; decimals += 1) {
      add(value, plain(decimals, group));
      add(value, percent(decimals, group));
    }
  }
}

// Every code whose en-US symbol is not the code, plus one that has none.
for (const code of [...CURRENCY_SYMBOLS, NO_SYMBOL_CODE]) {
  for (const group of [true, false]) {
    add(1234.5, currency(code, 2, group));
    add(-0, currency(code, 2, group));
  }
}
// Zero decimals keeps the integer-only currency path covered.
for (const code of ["JPY", "TWD", "USD", NO_SYMBOL_CODE]) {
  add(1234.5, currency(code, 0, true));
  add(-1234.5, currency(code, 0, true));
}

// 200 doubles from a fixed seed, half in a practical range and half across
// the whole exponent range, each without a format and with 2-decimal USD and
// percent formatting. The seed makes the corpus reproducible.
const random = seededRandom(0x0686_0001);
for (let index = 0; index < 200; index += 1) {
  const exponent =
    index % 2 === 1
      ? Math.floor(random() * 601) - 300
      : Math.floor(random() * 25) - 12;
  const mantissa = 1 + random() * 9;
  const sign = random() < 0.5 ? -1 : 1;
  const value = sign * mantissa * 10 ** exponent;
  add(value, null);
  add(value, currency("USD", 2, true));
  add(value, percent(2, true));
}

const target = fileURLToPath(new URL("./display_text_corpus.json", import.meta.url));
writeFileSync(target, `${JSON.stringify({ cases }, null, 1)}\n`);
console.log(`wrote ${cases.length} cases to ${target}`);
