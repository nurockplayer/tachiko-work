//! The exact text Sheet renders for one value.
//!
//! Sheet renders Numbers with `formatNumberFor` in `src/ui/field-display.ts`.
//! This module reproduces that function without an ICU dependency, so a window
//! can carry the display text and the grid, filters and Find can never
//! disagree on any platform: en-US grouping and percent/currency symbols,
//! half-expand rounding on the shortest decimal spelling of a double, and a
//! Number without a format shown at most 15 significant digits as JavaScript
//! `String()` writes it.

use tachiko_workspace_engine::{NumberFormat, NumberFormatKind};

use crate::ViewValue;

/// The en-US CLDR symbol of every currency code that has one. A code with no
/// symbol renders as the code itself plus U+00A0, as `Intl.NumberFormat` does.
const CURRENCY_SYMBOLS: [(&str, &str); 23] = [
    ("AUD", "A$"),
    ("BRL", "R$"),
    ("CAD", "CA$"),
    ("CNY", "CN¥"),
    ("EUR", "€"),
    ("GBP", "£"),
    ("HKD", "HK$"),
    ("ILS", "₪"),
    ("INR", "₹"),
    ("JPY", "¥"),
    ("KRW", "₩"),
    ("MXN", "MX$"),
    ("NZD", "NZ$"),
    ("PHP", "₱"),
    ("TWD", "NT$"),
    ("USD", "$"),
    ("VND", "₫"),
    ("XAF", "FCFA\u{a0}"),
    ("XCD", "EC$"),
    ("XCG", "Cg.\u{a0}"),
    ("XOF", "F\u{202f}CFA\u{a0}"),
    ("XPF", "CFPF\u{a0}"),
    ("XXX", "¤"),
];

/// The text Sheet renders for one value, or `None` when the value has none.
///
/// Blank, unavailable, failed and Reference values have no display text; the
/// filters in the next Change use this.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn display_text(value: &ViewValue, format: Option<&NumberFormat>) -> Option<String> {
    match value {
        ViewValue::Number(number) => Some(format_number(*number, format)),
        ViewValue::Text(text) => Some(text.clone()),
        ViewValue::Boolean(flag) => Some(if *flag { "true" } else { "false" }.to_owned()),
        ViewValue::Date(date) => Some(date.to_string()),
        ViewValue::Reference => None,
    }
}

/// `Intl.NumberFormat("en-US", …).format(value)` with `decimals` as both
/// fraction-digit bounds, or JavaScript `String()` at 15 significant digits
/// when no format is stored.
pub(crate) fn format_number(value: f64, format: Option<&NumberFormat>) -> String {
    let Some(format) = format else {
        return format_unformatted(value);
    };
    if let NumberFormatKind::Currency { code } = &format.kind {
        // `Intl.NumberFormat` refuses a code that is not three uppercase
        // letters; Sheet's `formatNumberFor` catches that and falls back to
        // the unformatted spelling.
        if !is_currency_code(code) {
            return format_unformatted(value);
        }
    }
    format_formatted(value, format)
}

fn is_currency_code(code: &str) -> bool {
    code.len() == 3 && code.bytes().all(|byte| byte.is_ascii_uppercase())
}

fn format_formatted(value: f64, format: &NumberFormat) -> String {
    let percent = matches!(&format.kind, NumberFormatKind::Percent);
    let decimals = usize::from(format.decimals);
    let body = if value.is_nan() {
        "NaN".to_owned()
    } else if value.is_infinite() {
        "∞".to_owned()
    } else {
        let decimal = shortest_decimal(value.abs());
        let scaled = round_scaled(&decimal, decimals, percent);
        let (integer, fraction) = split_scaled(scaled, decimals);
        let mut body = if format.thousands_separator {
            group_digits(&integer)
        } else {
            integer.clone()
        };
        if !fraction.is_empty() {
            body.push('.');
            body.push_str(&fraction);
        }
        body
    };
    // `Intl` keeps the sign of -0 but never signs NaN.
    let sign = if value.is_sign_negative() && !value.is_nan() {
        "-"
    } else {
        ""
    };
    match &format.kind {
        NumberFormatKind::Plain => format!("{sign}{body}"),
        NumberFormatKind::Percent => format!("{sign}{body}%"),
        NumberFormatKind::Currency { code } => {
            // A non-finite amount joins the code or symbol with no space.
            let prefix = if value.is_finite() {
                currency_prefix(code)
            } else {
                currency_word(code)
            };
            format!("{sign}{prefix}{body}")
        }
    }
}

fn currency_prefix(code: &str) -> String {
    match CURRENCY_SYMBOLS.iter().find(|(known, _)| *known == code) {
        Some((_, symbol)) => (*symbol).to_owned(),
        None => format!("{code}\u{a0}"),
    }
}

fn currency_word(code: &str) -> String {
    match CURRENCY_SYMBOLS.iter().find(|(known, _)| *known == code) {
        Some((_, symbol)) => symbol.trim_end_matches('\u{a0}').to_owned(),
        None => code.to_owned(),
    }
}

fn format_unformatted(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-Infinity"
        } else {
            "Infinity"
        }
        .to_owned();
    }
    if is_zero(value) {
        // JavaScript `String(-0)` is "0".
        return "0".to_owned();
    }
    // `Number(value.toPrecision(15))`, falling back to the exact value when
    // rounding at the float limit overflows to infinity.
    let rounded = round_to_significant(value, 15);
    js_number_to_string(rounded)
}

const fn is_zero(value: f64) -> bool {
    // Positive zero has 64 zero bits; negative zero has 63.
    value.to_bits().trailing_zeros() >= 63
}

/// A finite, non-negative decimal spelling of a double: the value is
/// `digits × 10^(point − digits.len())`. `digits` has no leading or trailing
/// zeros; zero is `{ digits: "0", point: 1 }`.
struct Decimal {
    digits: String,
    point: i32,
}

/// The shortest round-trip spelling, the same significant digits JavaScript
/// `String()` uses before it chooses positional or exponential notation.
fn shortest_decimal(magnitude: f64) -> Decimal {
    let text = format!("{magnitude:e}");
    let (mantissa, exponent) = text
        .split_once('e')
        .expect("LowerExp always writes an exponent");
    let exponent = exponent
        .parse::<i32>()
        .expect("LowerExp writes a base-10 exponent");
    let digits = mantissa
        .chars()
        .filter(|character| *character != '.')
        .collect::<String>();
    Decimal {
        digits,
        point: exponent + 1,
    }
}

/// The exact decimal expansion of a finite, positive double. A double is
/// `odd × 2^t`, so its expansion has exactly `−t` fraction digits and ends
/// there; formatting with that precision is exact, not rounded.
fn exact_decimal(magnitude: f64) -> Decimal {
    let bits = magnitude.to_bits();
    let biased = i32::try_from((bits >> 52) & 0x7ff).expect("11 exponent bits fit in i32");
    let fraction = bits & ((1_u64 << 52) - 1);
    let (mantissa, exponent) = if biased == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1_u64 << 52), biased - 1075)
    };
    let trailing = i32::try_from(mantissa.trailing_zeros()).expect("at most 63 shifts fit");
    let exponent = exponent + trailing;
    let fraction_digits = usize::try_from(-exponent).unwrap_or(0);
    let text = format!("{magnitude:.fraction_digits$}");
    let (integer, fraction) = text.split_once('.').unwrap_or((text.as_str(), ""));
    let joined = format!("{integer}{fraction}");
    let leading = joined.bytes().take_while(|byte| *byte == b'0').count();
    if leading == joined.len() {
        return Decimal {
            digits: "0".to_owned(),
            point: 1,
        };
    }
    let digits = joined[leading..].trim_end_matches('0').to_owned();
    let point = i32::try_from(integer.len()).expect("an f64 has at most 309 integer digits")
        - i32::try_from(leading).expect("an f64 has at most 1074 fraction digits");
    Decimal { digits, point }
}

/// Round `value × 10^decimals` (shifted two places for percent) to an integer
/// with half-expand rounding, and return its digits.
fn round_scaled(decimal: &Decimal, decimals: usize, percent: bool) -> String {
    let decimals = i32::try_from(decimals).unwrap_or(i32::MAX);
    let shift = decimals.saturating_add(if percent { 2 } else { 0 });
    let scaled_point = decimal.point.saturating_add(shift);
    let length = i32::try_from(decimal.digits.len()).unwrap_or(i32::MAX);
    let mut integer;
    let round_up;
    if scaled_point >= length {
        integer = decimal.digits.clone();
        integer.push_str(&"0".repeat(usize::try_from(scaled_point - length).unwrap_or(0)));
        round_up = false;
    } else if scaled_point > 0 {
        let cut = usize::try_from(scaled_point).unwrap_or(0);
        integer = decimal.digits[..cut].to_owned();
        round_up = decimal.digits.as_bytes()[cut] >= b'5';
    } else {
        integer = "0".to_owned();
        round_up = scaled_point == 0 && decimal.digits.as_bytes()[0] >= b'5';
    }
    if round_up {
        increment(&mut integer);
    }
    integer
}

/// Split the scaled integer into the grouped integer part and exactly
/// `decimals` fraction digits.
fn split_scaled(mut scaled: String, decimals: usize) -> (String, String) {
    if decimals == 0 {
        return (strip_leading_zeros(&scaled), String::new());
    }
    while scaled.len() <= decimals {
        scaled.insert(0, '0');
    }
    let split = scaled.len() - decimals;
    let fraction = scaled[split..].to_owned();
    (strip_leading_zeros(&scaled[..split]), fraction)
}

fn strip_leading_zeros(digits: &str) -> String {
    let trimmed = digits.trim_start_matches('0');
    if trimmed.is_empty() {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn increment(decimal: &mut String) {
    let mut bytes = decimal.as_bytes().to_vec();
    for index in (0..bytes.len()).rev() {
        if bytes[index] == b'9' {
            bytes[index] = b'0';
        } else {
            bytes[index] += 1;
            *decimal = String::from_utf8(bytes).expect("decimal digits are ASCII");
            return;
        }
    }
    bytes.insert(0, b'1');
    *decimal = String::from_utf8(bytes).expect("decimal digits are ASCII");
}

fn group_digits(integer: &str) -> String {
    let bytes = integer.as_bytes();
    let mut grouped = String::with_capacity(integer.len() + integer.len() / 3);
    for (index, byte) in bytes.iter().enumerate() {
        if index > 0 && (bytes.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(char::from(*byte));
    }
    grouped
}

/// `Number(value.toPrecision(significant))`'s numeric result.
fn round_to_significant(value: f64, significant: usize) -> f64 {
    let decimal = exact_decimal(value.abs());
    if decimal.digits.len() <= significant {
        return value;
    }
    let bytes = decimal.digits.as_bytes();
    let mut kept = decimal.digits[..significant].to_owned();
    if bytes[significant] >= b'5' {
        increment(&mut kept);
    }
    let grown = i32::try_from(kept.len()).expect("at most 17 significant digits")
        - i32::try_from(significant).expect("15 significant digits fit");
    let point = decimal.point + grown;
    let mantissa = if kept.len() == 1 {
        kept
    } else {
        format!("{}.{}", &kept[..1], &kept[1..])
    };
    let text = format!("{mantissa}e{}", point - 1);
    let rounded = text.parse::<f64>().unwrap_or(f64::INFINITY);
    let rounded = if value.is_sign_negative() {
        -rounded
    } else {
        rounded
    };
    if rounded.is_finite() { rounded } else { value }
}

/// JavaScript `String(value)`: the shortest round-trip digits with the
/// ECMAScript positional/exponential thresholds.
fn js_number_to_string(value: f64) -> String {
    if is_zero(value) {
        return "0".to_owned();
    }
    let negative = value.is_sign_negative();
    let text = format!("{:e}", value.abs());
    let (mantissa, exponent) = text
        .split_once('e')
        .expect("LowerExp always writes an exponent");
    let exponent = exponent
        .parse::<i32>()
        .expect("LowerExp writes a base-10 exponent");
    let digits = mantissa
        .chars()
        .filter(|character| *character != '.')
        .collect::<String>();
    let length = i32::try_from(digits.len()).expect("shortest digits fit in i32");
    let point = exponent + 1;
    let magnitude = if length <= point && point <= 21 {
        format!(
            "{digits}{}",
            "0".repeat(usize::try_from(point - length).unwrap_or(0))
        )
    } else if point > 0 && point <= 21 {
        let split = usize::try_from(point).unwrap_or(0);
        format!("{}.{}", &digits[..split], &digits[split..])
    } else if point > -6 && point <= 0 {
        format!(
            "0.{}{digits}",
            "0".repeat(usize::try_from(-point).unwrap_or(0))
        )
    } else {
        let mantissa = if length == 1 {
            digits
        } else {
            format!("{}.{}", &digits[..1], &digits[1..])
        };
        let exponent = point - 1;
        if exponent >= 0 {
            format!("{mantissa}e+{exponent}")
        } else {
            format!("{mantissa}e{exponent}")
        }
    };
    if negative {
        format!("-{magnitude}")
    } else {
        magnitude
    }
}

#[cfg(test)]
mod tests {
    use super::{display_text, format_number};
    use crate::ViewValue;
    use serde::Deserialize;
    use tachiko_workspace_engine::{Date, NumberFormat, NumberFormatKind};

    #[derive(Deserialize)]
    struct Case {
        /// The shortest round-trip spelling; `-0` keeps the negative zero.
        value: String,
        format: Option<NumberFormat>,
        expected: String,
    }

    #[derive(Deserialize)]
    struct Corpus {
        cases: Vec<Case>,
    }

    #[test]
    fn display_text_maps_each_value_kind() {
        let plain = NumberFormat {
            decimals: 2,
            kind: NumberFormatKind::Plain,
            thousands_separator: true,
        };
        assert_eq!(
            display_text(&ViewValue::Number(1234.5), Some(&plain)),
            Some("1,234.50".to_owned())
        );
        assert_eq!(
            display_text(&ViewValue::Number(1234.5), None),
            Some("1234.5".to_owned())
        );
        assert_eq!(
            display_text(&ViewValue::Text("hello".to_owned()), None),
            Some("hello".to_owned())
        );
        assert_eq!(
            display_text(&ViewValue::Boolean(true), None),
            Some("true".to_owned())
        );
        assert_eq!(
            display_text(&ViewValue::Boolean(false), None),
            Some("false".to_owned())
        );
        let date = Date::parse("2026-01-02").expect("fixed Gregorian date");
        assert_eq!(
            display_text(&ViewValue::Date(date), Some(&plain)),
            Some(date.to_string())
        );
        assert_eq!(display_text(&ViewValue::Reference, Some(&plain)), None);
    }

    /// The committed corpus is generated with Node's `Intl` by the adjacent
    /// `.mjs`, so it is the authority for en-US behavior.
    #[test]
    fn golden_corpus_matches_sheet_display() {
        let corpus: Corpus =
            serde_json::from_str(include_str!("../tests/fixtures/display_text_corpus.json"))
                .expect("the committed corpus is valid JSON");
        assert!(!corpus.cases.is_empty(), "the corpus has cases");
        let mut mismatches = Vec::new();
        for case in &corpus.cases {
            let value = case
                .value
                .parse::<f64>()
                .expect("corpus values are f64 spellings");
            let actual = format_number(value, case.format.as_ref());
            if actual != case.expected {
                mismatches.push(format!(
                    "value {} with format {:?}: expected {:?}, got {:?}",
                    case.value, case.format, case.expected, actual
                ));
                if mismatches.len() == 20 {
                    break;
                }
            }
        }
        assert!(
            mismatches.is_empty(),
            "{} corpus mismatches:\n{}",
            mismatches.len(),
            mismatches.join("\n")
        );
    }
}
