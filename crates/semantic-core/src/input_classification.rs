//! Classifies raw cell input for Auto columns (ADR-0047 §2).
//!
//! The function is pure: it reads no clock and no locale. Matching runs on a
//! normalized copy of the input (full-width digits and marks read as ASCII);
//! input that matches no row keeps its exact original spelling.

use crate::{Date, Number, NumberFormat, NumberFormatKind};

/// The host's date order for `M/D/YYYY` and `D/M/YYYY` input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DateOrder {
    /// Read the ambiguous form as `M/D/YYYY`.
    MonthFirst,
    /// Read the ambiguous form as `D/M/YYYY`.
    DayFirst,
}

/// The date spelling a classified Date was typed in.
///
/// Canonical `YYYY-MM-DD` has no display. ADR-0049's date format maps from
/// this later.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassifiedDateDisplay {
    /// `YYYY/M/D`.
    YmdSlash,
    /// `YYYY年M月D日`.
    YmdCjk,
    /// `M/D/YYYY`, read by a [`DateOrder::MonthFirst`] host.
    MonthFirst,
    /// `D/M/YYYY`, read by a [`DateOrder::DayFirst`] host.
    DayFirst,
}

/// What one raw input classifies to, before anything is bound or stored.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClassifiedInput {
    /// The empty string.
    Blank,
    /// The input starts with `=`; the caller binds it.
    FormulaSource(String),
    /// `TRUE` or `FALSE`, any ASCII case.
    Boolean(bool),
    /// A Number, with the display the input implied, if any.
    Number {
        value: Number,
        display: Option<NumberFormat>,
    },
    /// A Date, with the spelling it was typed in, if it was not canonical.
    Date {
        value: Date,
        display: Option<ClassifiedDateDisplay>,
    },
    /// Anything else, exactly as typed.
    Text(String),
}

/// A valid number spelling with what its display implies.
struct ParsedNumber {
    value: Number,
    decimals_typed: usize,
    grouped: bool,
}

/// R3's limit on implied display decimals. The value stays exact.
const MAX_DISPLAY_DECIMALS: u8 = 10;

/// At most 15 significant digits, so a Number keeps every typed digit.
const MAX_SIGNIFICANT_DIGITS: usize = 15;

/// Currency marks, longest first, with the code each one displays as.
const CURRENCY_MARKS: &[(&str, &str)] = &[
    ("NT$", "TWD"),
    ("$", "USD"),
    ("¥", "JPY"),
    ("€", "EUR"),
    ("£", "GBP"),
];

/// Classify one raw input for an Auto cell.
///
/// The first row of ADR-0047 §2's table that matches decides. `date_order`
/// comes from the host and is used only for `M/D/YYYY` and `D/M/YYYY`.
#[must_use]
pub fn classify_input(input: &str, date_order: Option<DateOrder>) -> ClassifiedInput {
    if input.is_empty() {
        return ClassifiedInput::Blank;
    }
    if input.starts_with('=') {
        return ClassifiedInput::FormulaSource(input.to_owned());
    }
    let normalized = normalize_width(input);
    let trimmed = trim_ascii_spaces(&normalized);
    if trimmed.eq_ignore_ascii_case("true") {
        return ClassifiedInput::Boolean(true);
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return ClassifiedInput::Boolean(false);
    }
    if let Some(parsed) = parse_plain(trimmed) {
        return ClassifiedInput::Number {
            value: parsed.value,
            display: None,
        };
    }
    if let Some(parsed) = parse_grouped(trimmed) {
        return ClassifiedInput::Number {
            display: Some(format_for(&parsed, NumberFormatKind::Plain, parsed.grouped)),
            value: parsed.value,
        };
    }
    if let Some((parsed, code)) = parse_currency(trimmed) {
        let thousands = parsed.grouped || parsed.value.get().abs() >= 1_000.0;
        return ClassifiedInput::Number {
            display: Some(format_for(
                &parsed,
                NumberFormatKind::Currency {
                    code: code.to_owned(),
                },
                thousands,
            )),
            value: parsed.value,
        };
    }
    if let Some(parsed) = parse_percent(trimmed) {
        return ClassifiedInput::Number {
            display: Some(format_for(&parsed, NumberFormatKind::Percent, false)),
            value: parsed.value,
        };
    }
    if let Some((value, display)) = parse_date(&normalized, date_order) {
        return ClassifiedInput::Date { value, display };
    }
    ClassifiedInput::Text(input.to_owned())
}

/// Map the characters an IME may produce to the ASCII ones matching expects.
fn normalize_width(input: &str) -> String {
    let mut normalized = String::with_capacity(input.len());
    for character in input.chars() {
        let ascii = match character {
            '\u{FF10}'..='\u{FF19}' => {
                let digit = u8::try_from(u32::from(character) - 0xFF10).unwrap_or_default();
                char::from(b'0' + digit)
            }
            '．' => '.',
            '，' => ',',
            '％' => '%',
            '＋' => '+',
            '－' => '-',
            '￥' => '¥',
            other => other,
        };
        normalized.push(ascii);
    }
    normalized
}

fn trim_ascii_spaces(text: &str) -> &str {
    text.trim_matches(' ')
}

fn format_for(
    parsed: &ParsedNumber,
    kind: NumberFormatKind,
    thousands_separator: bool,
) -> NumberFormat {
    NumberFormat {
        decimals: display_decimals(parsed.decimals_typed),
        kind,
        thousands_separator,
    }
}

fn display_decimals(decimals_typed: usize) -> u8 {
    u8::try_from(decimals_typed.min(usize::from(MAX_DISPLAY_DECIMALS)))
        .unwrap_or(MAX_DISPLAY_DECIMALS)
}

/// Parse the locale-free numeric grammar (ADR-0047 §5), without comma groups.
fn parse_plain(text: &str) -> Option<ParsedNumber> {
    let (negative, rest) = split_sign(text);
    let (mantissa, exponent) = split_exponent(rest)?;
    let (int_part, frac_part) = match mantissa.split_once('.') {
        Some((int_part, frac_part)) => (int_part, frac_part),
        None => (mantissa, ""),
    };
    if !is_ascii_digits(int_part) || !is_ascii_digits(frac_part) {
        return None;
    }
    if int_part.is_empty() && frac_part.is_empty() {
        return None;
    }
    if int_part.len() > 1 && int_part.starts_with('0') {
        return None;
    }
    build_number(negative, int_part, frac_part, exponent, false)
}

/// Parse the numeric grammar with comma groups of three.
fn parse_grouped(text: &str) -> Option<ParsedNumber> {
    let (negative, rest) = split_sign(text);
    let (mantissa, exponent) = split_exponent(rest)?;
    let (integer, frac_part) = match mantissa.split_once('.') {
        Some((integer, frac_part)) => (integer, frac_part),
        None => (mantissa, ""),
    };
    if !is_ascii_digits(frac_part) {
        return None;
    }
    let mut groups = integer.split(',');
    let first = groups.next()?;
    if first.is_empty() || first.len() > 3 || !is_ascii_digits(first) {
        return None;
    }
    let mut digits = String::with_capacity(integer.len());
    digits.push_str(first);
    let mut grouped = false;
    for group in groups {
        if group.len() != 3 || !is_ascii_digits(group) {
            return None;
        }
        digits.push_str(group);
        grouped = true;
    }
    if !grouped {
        return None;
    }
    if digits.len() > 1 && digits.starts_with('0') {
        return None;
    }
    build_number(negative, &digits, frac_part, exponent, true)
}

/// Parse a number with an optional leading sign and optional comma groups.
fn parse_number(text: &str) -> Option<ParsedNumber> {
    parse_plain(text).or_else(|| parse_grouped(text))
}

/// Parse a leading currency mark with an optional sign before or after it.
fn parse_currency(text: &str) -> Option<(ParsedNumber, &'static str)> {
    let mut body = text;
    let mut negative = false;
    let mut signed = false;
    if let Some(rest) = body.strip_prefix('-') {
        negative = true;
        signed = true;
        body = rest;
    } else if let Some(rest) = body.strip_prefix('+') {
        signed = true;
        body = rest;
    }
    let (code, rest) = CURRENCY_MARKS
        .iter()
        .find_map(|(mark, code)| body.strip_prefix(*mark).map(|rest| (*code, rest)))?;
    body = rest;
    if let Some(rest) = body.strip_prefix('-') {
        if signed {
            return None;
        }
        negative = true;
        body = rest;
    } else if let Some(rest) = body.strip_prefix('+') {
        if signed {
            return None;
        }
        body = rest;
    }
    let mut parsed = parse_number(body)?;
    if negative {
        parsed.value = Number::new(-parsed.value.get()).ok()?;
    }
    Some((parsed, code))
}

/// Parse a trailing percent sign; the value is the number over 100.
fn parse_percent(text: &str) -> Option<ParsedNumber> {
    let body = text.strip_suffix('%')?;
    let mut parsed = parse_number(trim_ascii_spaces(body))?;
    parsed.value = Number::new(parsed.value.get() / 100.0).ok()?;
    Some(parsed)
}

fn split_sign(text: &str) -> (bool, &str) {
    if let Some(rest) = text.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = text.strip_prefix('+') {
        (false, rest)
    } else {
        (false, text)
    }
}

/// Split an optional exponent off the mantissa, keeping its own sign.
fn split_exponent(text: &str) -> Option<(&str, Option<&str>)> {
    match text.find(|character| character == 'e' || character == 'E') {
        Some(index) => {
            let mantissa = &text[..index];
            let exponent = &text[index + 1..];
            if !is_valid_exponent(exponent) {
                return None;
            }
            Some((mantissa, Some(exponent)))
        }
        None => Some((text, None)),
    }
}

fn is_valid_exponent(text: &str) -> bool {
    let (_, digits) = split_sign(text);
    !digits.is_empty() && is_ascii_digits(digits)
}

fn is_ascii_digits(text: &str) -> bool {
    text.bytes().all(|byte| byte.is_ascii_digit())
}

/// Count the digits left after stripping leading zeros.
fn significant_digit_count(int_part: &str, frac_part: &str) -> usize {
    let mut count = 0;
    let mut leading = true;
    for byte in int_part.bytes().chain(frac_part.bytes()) {
        if leading && byte == b'0' {
            continue;
        }
        leading = false;
        count += 1;
    }
    count
}

fn build_number(
    negative: bool,
    int_part: &str,
    frac_part: &str,
    exponent: Option<&str>,
    grouped: bool,
) -> Option<ParsedNumber> {
    if significant_digit_count(int_part, frac_part) > MAX_SIGNIFICANT_DIGITS {
        return None;
    }
    let mut canonical = String::new();
    if negative {
        canonical.push('-');
    }
    if int_part.is_empty() {
        canonical.push('0');
    } else {
        canonical.push_str(int_part);
    }
    canonical.push('.');
    if frac_part.is_empty() {
        canonical.push('0');
    } else {
        canonical.push_str(frac_part);
    }
    if let Some(exponent) = exponent {
        canonical.push('e');
        canonical.push_str(exponent);
    }
    let parsed = canonical.parse::<f64>().ok()?;
    Some(ParsedNumber {
        value: Number::new(parsed).ok()?,
        decimals_typed: frac_part.len(),
        grouped,
    })
}

fn parse_date(
    text: &str,
    date_order: Option<DateOrder>,
) -> Option<(Date, Option<ClassifiedDateDisplay>)> {
    if let Ok(value) = Date::parse(text) {
        return Some((value, None));
    }
    if let Some(value) = parse_ymd_slash(text) {
        return Some((value, Some(ClassifiedDateDisplay::YmdSlash)));
    }
    if let Some(value) = parse_ymd_cjk(text) {
        return Some((value, Some(ClassifiedDateDisplay::YmdCjk)));
    }
    let order = date_order?;
    let value = parse_order_first(text, order)?;
    let display = match order {
        DateOrder::MonthFirst => ClassifiedDateDisplay::MonthFirst,
        DateOrder::DayFirst => ClassifiedDateDisplay::DayFirst,
    };
    Some((value, Some(display)))
}

/// Parse `YYYY/M/D` with a four-digit year first.
fn parse_ymd_slash(text: &str) -> Option<Date> {
    let mut parts = text.split('/');
    let year = parse_four_digit_year(parts.next()?)?;
    let month = parse_one_or_two_digits(parts.next()?)?;
    let day = parse_one_or_two_digits(parts.next()?)?;
    if parts.next().is_some() {
        return None;
    }
    Date::new(year, month, day).ok()
}

/// Parse `YYYY年M月D日` with a four-digit year first.
fn parse_ymd_cjk(text: &str) -> Option<Date> {
    let text = text.strip_suffix('日')?;
    let (year, rest) = text.split_once('年')?;
    let (month, day) = rest.split_once('月')?;
    let year = parse_four_digit_year(year)?;
    let month = parse_one_or_two_digits(month)?;
    let day = parse_one_or_two_digits(day)?;
    Date::new(year, month, day).ok()
}

/// Parse the ambiguous `M/D/YYYY` or `D/M/YYYY` form under the host's order.
fn parse_order_first(text: &str, order: DateOrder) -> Option<Date> {
    let mut parts = text.split('/');
    let first = parse_one_or_two_digits(parts.next()?)?;
    let second = parse_one_or_two_digits(parts.next()?)?;
    let year = parse_four_digit_year(parts.next()?)?;
    if parts.next().is_some() {
        return None;
    }
    let (month, day) = match order {
        DateOrder::MonthFirst => (first, second),
        DateOrder::DayFirst => (second, first),
    };
    Date::new(year, month, day).ok()
}

fn parse_four_digit_year(text: &str) -> Option<u16> {
    if text.len() != 4 || !is_ascii_digits(text) {
        return None;
    }
    text.parse::<u16>().ok()
}

fn parse_one_or_two_digits(text: &str) -> Option<u8> {
    if text.is_empty() || text.len() > 2 || !is_ascii_digits(text) {
        return None;
    }
    text.parse::<u8>().ok()
}
