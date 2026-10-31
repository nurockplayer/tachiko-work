//! Closed classification of retained spreadsheet number-format strings.
//! This does not render formats or select a sign/conditional section.

use tachiko_workspace_engine::{NumberFormat, NumberFormatKind as StoredNumberFormatKind};

/// Convert only the exact numeric vocabulary emitted by Tachiko's native
/// XLSX writer. Other Excel patterns remain source-only and are disclosed.
pub(crate) fn from_excel(pattern: &str) -> Option<NumberFormat> {
    let (kind, numeric) = if let Some(rest) = pattern.strip_suffix('%') {
        (StoredNumberFormatKind::Percent, rest)
    } else if let Some(rest) = pattern.strip_prefix('"') {
        let end = rest.find("\" ")?;
        let code = &rest[..end];
        if code.len() != 3 || !code.bytes().all(|byte| byte.is_ascii_uppercase()) {
            return None;
        }
        (
            StoredNumberFormatKind::Currency {
                code: code.to_owned(),
            },
            &rest[end + 2..],
        )
    } else {
        (StoredNumberFormatKind::Plain, pattern)
    };
    let (thousands_separator, numeric) = if let Some(rest) = numeric.strip_prefix("#,##0") {
        (true, rest)
    } else {
        (false, numeric.strip_prefix('0')?)
    };
    let decimals = if numeric.is_empty() {
        0
    } else {
        let places = numeric.strip_prefix('.')?;
        if places.is_empty() || places.len() > 10 || !places.bytes().all(|byte| byte == b'0') {
            return None;
        }
        u8::try_from(places.len()).ok()?
    };
    Some(NumberFormat {
        decimals,
        kind,
        thousands_separator,
    })
}

pub(crate) fn to_excel(format: &NumberFormat) -> String {
    let grouped = if format.thousands_separator {
        "#,##0"
    } else {
        "0"
    };
    let places = if format.decimals == 0 {
        String::new()
    } else {
        format!(".{}", "0".repeat(usize::from(format.decimals)))
    };
    let numeric = format!("{grouped}{places}");
    match &format.kind {
        StoredNumberFormatKind::Plain => numeric,
        StoredNumberFormatKind::Percent => format!("{numeric}%"),
        StoredNumberFormatKind::Currency { code } => format!("\"{code}\" {numeric}"),
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum NumberFormatKind {
    Number,
    Date,
    Time,
}

#[derive(Clone, Copy, PartialEq)]
enum Token {
    Number,
    Calendar,
    Month,
    ShortMonth,
    Hour,
    Second,
    Clock,
    Elapsed,
}

pub(crate) fn classify(pattern: Option<&str>) -> Result<NumberFormatKind, String> {
    inspect(pattern).map(|(kind, _)| kind)
}

/// One tokenizer supplies both classification and numeric condition inventory.
/// Digits in quoted display text, locale IDs and color IDs are not conditions.
pub(crate) fn inspect(pattern: Option<&str>) -> Result<(NumberFormatKind, Vec<String>), String> {
    let mut conditions = Vec::new();
    let pattern = pattern.unwrap_or("");
    if pattern.len() > 4096 {
        return Err("Number format exceeds retained-style bound".into());
    }
    let chars: Vec<char> = pattern.chars().collect();
    let mut sections = vec![Vec::new()];
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let token = match c {
            '"' => {
                i = literal_end(&chars, i + 1)?;
                None
            }
            '\\' | '_' | '*' => {
                i += 1;
                if i >= chars.len() {
                    return Err("Incomplete number-format escape/padding".into());
                }
                None
            }
            '[' => {
                let start = i + 1;
                i = start;
                while i < chars.len() && chars[i] != ']' {
                    if chars[i] == '[' {
                        return Err("Nested number-format bracket".into());
                    }
                    i += 1;
                }
                if i == chars.len() {
                    return Err("Incomplete number-format bracket".into());
                }
                bracket(&chars[start..i].iter().collect::<String>(), &mut conditions)?
            }
            ';' => {
                if sections.len() == 4 {
                    return Err("Number format exceeds four sections".into());
                }
                sections.push(Vec::new());
                None
            }
            '0' | '#' | '?' => Some(Token::Number),
            '@' | '.' | ',' | '/' | ':' | '%' | '+' | '-' | '(' | ')' | ' ' | '$' | '¥' | '￥'
            | '€' | '£' | '!' | '^' | '&' | '\'' | '~' | '{' | '}' | '=' | '<' | '>' => None,
            _ if c.is_ascii_alphabetic() => {
                let rest: String = chars[i..].iter().collect::<String>().to_ascii_lowercase();
                if rest.starts_with("general") {
                    i += 6;
                    Some(Token::Number)
                } else if rest.starts_with("am/pm") {
                    i += 4;
                    Some(Token::Clock)
                } else if rest.starts_with("a/p") {
                    i += 2;
                    Some(Token::Clock)
                } else if matches!(c, 'e' | 'E') {
                    if !matches!(chars.get(i + 1), Some('+' | '-'))
                        || !matches!(chars.get(i + 2), Some('0' | '#' | '?'))
                        || !matches!(sections.last().and_then(|s| s.last()), Some(Token::Number))
                    {
                        return Err("Unsupported scientific number-format token".into());
                    }
                    i += 1;
                    Some(Token::Number)
                } else {
                    let letter = c.to_ascii_lowercase();
                    let start = i;
                    while chars
                        .get(i + 1)
                        .is_some_and(|next| next.to_ascii_lowercase() == letter)
                    {
                        i += 1;
                    }
                    let count = i - start + 1;
                    Some(match (letter, count) {
                        ('y' | 'd', 1..=4) => Token::Calendar,
                        ('m', 1..=2) => Token::ShortMonth,
                        ('m', 3..=5) => Token::Month,
                        ('h', 1..=2) => Token::Hour,
                        ('s', 1..=2) => Token::Second,
                        _ => return Err("Unknown number-format token".into()),
                    })
                }
            }
            _ => return Err("Unknown number-format token".into()),
        };
        if let Some(token) = token {
            sections.last_mut().expect("one section").push(token);
        }
        i += 1;
    }
    uniform_kind(&sections).map(|kind| (kind, conditions))
}

fn literal_end(chars: &[char], mut i: usize) -> Result<usize, String> {
    while i < chars.len() && chars[i] != '"' {
        if chars[i] == '\\' {
            i += 1;
        }
        i += 1;
    }
    if i >= chars.len() {
        return Err("Incomplete number-format literal".into());
    }
    Ok(i)
}

fn uniform_kind(sections: &[Vec<Token>]) -> Result<NumberFormatKind, String> {
    let kinds: Vec<_> = sections.iter().take(3).map(|s| section_kind(s)).collect();
    let first = kinds[0];
    if kinds.iter().any(|kind| *kind != first) {
        return Err("Mixed numeric number-format section types are unsupported".into());
    }
    Ok(first)
}

fn section_kind(tokens: &[Token]) -> NumberFormatKind {
    if tokens.contains(&Token::Elapsed) {
        return NumberFormatKind::Time;
    }
    let calendar = tokens.iter().enumerate().any(|(i, token)| {
        matches!(token, Token::Calendar | Token::Month)
            || (*token == Token::ShortMonth
                && i.checked_sub(1).and_then(|p| tokens.get(p)) != Some(&Token::Hour)
                && tokens.get(i + 1) != Some(&Token::Second))
    });
    if calendar {
        NumberFormatKind::Date
    } else if tokens.iter().any(|t| {
        matches!(
            t,
            Token::Hour | Token::Second | Token::Clock | Token::ShortMonth
        )
    }) {
        NumberFormatKind::Time
    } else {
        NumberFormatKind::Number
    }
}

fn bracket(content: &str, conditions: &mut Vec<String>) -> Result<Option<Token>, String> {
    let lower = content.to_ascii_lowercase();
    if matches!(lower.as_str(), "h" | "hh" | "m" | "mm" | "s" | "ss") {
        return Ok(Some(Token::Elapsed));
    }
    if matches!(
        lower.as_str(),
        "black" | "blue" | "cyan" | "green" | "magenta" | "red" | "white" | "yellow"
    ) || lower
        .strip_prefix("color")
        .and_then(|v| v.parse::<u8>().ok())
        .is_some_and(|n| (1..=56).contains(&n))
    {
        return Ok(None);
    }
    if let Some(value) = ["<=", ">=", "<>", "=", "<", ">"]
        .iter()
        .find_map(|op| lower.strip_prefix(op))
    {
        if !value.is_empty()
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.' | b'e'))
        {
            if let Ok(number) = value.parse::<f64>() {
                if number.is_finite() {
                    conditions.push(value.to_owned());
                    return Ok(None);
                }
            }
        }
    }
    if let Some(currency) = content.strip_prefix('$') {
        let (symbol, locale) = currency
            .rsplit_once('-')
            .map_or((currency, None), |(a, b)| (a, Some(b)));
        // Currency text is retained presentation, not a currency identity.
        // The enclosing style bound also bounds this uninterpreted component.
        let literal_symbol = symbol.chars().all(|c| {
            !c.is_control() && !matches!(c, '[' | ']' | '"' | '\\' | ';' | '_' | '*' | '-')
        });
        let known_locale = locale.is_none_or(|v| {
            !v.is_empty() && v.len() <= 8 && v.bytes().all(|b| b.is_ascii_hexdigit())
        });
        if literal_symbol && known_locale && (!symbol.is_empty() || locale.is_some()) {
            return Ok(None);
        }
    }
    Err("Unknown number-format bracket".into())
}

#[cfg(test)]
mod tests {
    use super::{NumberFormatKind::*, classify};

    #[test]
    fn stored_formats_round_trip_only_exact_xlsx_codes() {
        use tachiko_workspace_engine::NumberFormat;

        for format in [
            NumberFormat {
                decimals: 2,
                kind: tachiko_workspace_engine::NumberFormatKind::Plain,
                thousands_separator: true,
            },
            NumberFormat {
                decimals: 0,
                kind: tachiko_workspace_engine::NumberFormatKind::Percent,
                thousands_separator: false,
            },
            NumberFormat {
                decimals: 3,
                kind: tachiko_workspace_engine::NumberFormatKind::Currency { code: "JPY".into() },
                thousands_separator: true,
            },
        ] {
            let code = super::to_excel(&format);
            assert_eq!(super::from_excel(&code), Some(format));
        }
        for unsupported in [
            "General",
            "0.##",
            "0.00;[Red]-0.00",
            "$#,##0.00",
            "\"usd\" 0.00",
        ] {
            assert_eq!(super::from_excel(unsupported), None, "{unsupported}");
        }
    }

    #[test]
    fn xlsx_column_format_uses_display_code_and_csv_keeps_raw_number() {
        use crate::interop_adapter::{
            SourceCell, SourceColumn, SourceLayout, SourceSheet, SourceValue, SourceWorkbook,
            export_csv, export_xlsx, import_xlsx,
        };
        use tachiko_workspace_engine::NumberFormat;

        let format = NumberFormat {
            decimals: 2,
            kind: tachiko_workspace_engine::NumberFormatKind::Percent,
            thousands_separator: true,
        };
        let workbook = SourceWorkbook {
            sheets: vec![SourceSheet {
                name: "Numbers".into(),
                has_header: true,
                columns: vec![SourceColumn {
                    name: "Rate".into(),
                    width: None,
                    number_format: Some(format.clone()),
                }],
                rows: vec![vec![SourceCell {
                    value: SourceValue::Number { value: 0.125 },
                    ..SourceCell::default()
                }]],
                layout: SourceLayout::default(),
            }],
            ledger: Vec::new(),
        };
        let csv = String::from_utf8(export_csv(&workbook.sheets[0]).unwrap()).unwrap();
        assert_eq!(csv, "Rate\r\n0.125\r\n");
        let imported = import_xlsx(&export_xlsx(&workbook).unwrap()).unwrap();
        assert_eq!(imported.sheets[0].columns[0].number_format, Some(format));
        assert_eq!(
            imported.sheets[0].rows[0][0].style.number_format.as_deref(),
            Some("#,##0.00%")
        );
    }

    #[test]
    fn calendar_months_and_contextual_minutes() {
        for pattern in [
            "m",
            "mm",
            "mmm",
            "mmmm",
            "mmmmm",
            "[$-411]mm",
            "m/d/yy",
            "d-mmm-yy",
            "d-mmm",
            "mmm-yy",
            "yyyy-mm-dd",
            "m/d/yy h:mm",
            "hh mmm",
            "h:mm m",
        ] {
            assert_eq!(classify(Some(pattern)), Ok(Date), "{pattern}");
        }
        for pattern in [
            "h:mm",
            "h:mm:ss",
            "h:mm AM/PM",
            "h:mm:ss AM/PM",
            "mm:ss",
            "h\"hours\" mm",
            "mm\\:ss",
            "[h]:mm",
            "[m]",
            "[ss]",
            "yyyy-mm-dd [h]",
            "[m] yyyy-mm-dd",
        ] {
            assert_eq!(classify(Some(pattern)), Ok(Time), "{pattern}");
        }
    }

    #[test]
    fn supported_numeric_literals_and_brackets() {
        for pattern in [
            "General",
            "0.00E+00",
            "##0.0E-0",
            "# ?/?",
            "#,##0 ;(#,##0)",
            "#,##0 ;[Red](#,##0)",
            "#,##0.00;(#,##0.00)",
            "#,##0.00;[Red](#,##0.00)",
            "@",
            "0;0%",
            "[$¥-411]0;[$$-409]0",
            "[$USD-411]0",
            "[$-411]0",
            "[$€-407]0",
            "[>=1]0;[Red]0",
            "[Color56]0",
            "0_);(0)",
            "0* ;0",
            "0\\;;0",
            "0\";date yyyy h\"",
            "0\\m",
            "0\"a\\\"%\\\"b\"",
        ] {
            assert_eq!(classify(Some(pattern)), Ok(Number), "{pattern}");
        }
        assert_eq!(classify(None), Ok(Number));
    }

    #[test]
    fn sections_are_uniform_and_text_section_is_not_numeric() {
        for pattern in ["yyyy;mm;dd;@", "mm;yyyy;dd;\"text\"@"] {
            assert_eq!(classify(Some(pattern)), Ok(Date));
        }
        assert_eq!(classify(Some("0;0;0;@%")), Ok(Number));
        for pattern in [
            "0;yyyy",
            "yyyy;0",
            "yyyy;yyyy;0",
            "0;0;mm",
            "h;yyyy",
            "yyyy;h",
            "0;;;;",
            "0;0;0;@;@",
        ] {
            assert!(classify(Some(pattern)).is_err(), "{pattern}");
        }
    }

    #[test]
    fn currency_text_is_retained_without_inventing_identity() {
        for pattern in [
            "[$CAD-409]0",
            "[$AUD-C09]0",
            "[$CNY-804]0",
            "[$R$-416]0",
            "[$CAD]0",
            "[$CAD-409]0;[$JPY-411]0",
            "[$-411]0",
        ] {
            assert_eq!(classify(Some(pattern)), Ok(Number), "{pattern}");
        }
    }

    #[test]
    fn malformed_unknown_and_oversized_formats_fail_closed() {
        for pattern in [
            "banana",
            "ggge",
            "rrrr",
            "unsupported_builtin_23",
            "[DBNum1]0",
            "[unknown]0",
            "[Color57]0",
            "[>=oops]0",
            "[$CAD-xyz]0",
            "[$CAD;USD-409]0",
            "[$CAD_USD-409]0",
            "[$CAD\\USD-409]0",
            "[$CAD\n-409]0",
            "[$-xyz]0",
            "[Red",
            "[[Red]]0",
            "0]",
            "0\\",
            "0_",
            "0*",
            "0\"no end",
            "0E",
            "0E+",
            "eeee",
            "hhhh",
            "mmmmmm",
        ] {
            assert!(classify(Some(pattern)).is_err(), "{pattern}");
        }
        assert!(classify(Some(&"0".repeat(4097))).is_err());
        assert_eq!(classify(Some(&"0".repeat(4096))), Ok(Number));
    }
}
