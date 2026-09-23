//! Closed classification of retained spreadsheet number-format strings.
//! This does not render formats or select a sign/conditional section.

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
                bracket(&chars[start..i].iter().collect::<String>())?
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
    uniform_kind(&sections)
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

fn bracket(content: &str) -> Result<Option<Token>, String> {
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
            && value.parse::<f64>().is_ok_and(f64::is_finite)
        {
            return Ok(None);
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
