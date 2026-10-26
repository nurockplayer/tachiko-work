//! Storage-owned optional number-format declaration shared by the current codecs.
use serde::{Deserialize, Deserializer, Serialize};
use tachiko_semantic_core::{NumberFormat, NumberFormatKind};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NumberFormatDto {
    decimals: u8,
    kind: Kind,
    thousands_separator: bool,
}

// Unit variants of an internally tagged enum ignore extra keys, so each kind wraps
// a `deny_unknown_fields` struct to keep the stored wire shape closed.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Kind {
    Plain(EmptyKind),
    Currency(CurrencyKind),
    Percent(EmptyKind),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EmptyKind {}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CurrencyKind {
    code: String,
}

impl From<&NumberFormat> for NumberFormatDto {
    fn from(format: &NumberFormat) -> Self {
        Self {
            decimals: format.decimals,
            kind: match &format.kind {
                NumberFormatKind::Plain => Kind::Plain(EmptyKind {}),
                NumberFormatKind::Currency { code } => {
                    Kind::Currency(CurrencyKind { code: code.clone() })
                }
                NumberFormatKind::Percent => Kind::Percent(EmptyKind {}),
            },
            thousands_separator: format.thousands_separator,
        }
    }
}

impl From<NumberFormatDto> for NumberFormat {
    fn from(format: NumberFormatDto) -> Self {
        Self {
            decimals: format.decimals,
            kind: match format.kind {
                Kind::Plain(_) => NumberFormatKind::Plain,
                Kind::Currency(CurrencyKind { code }) => NumberFormatKind::Currency { code },
                Kind::Percent(_) => NumberFormatKind::Percent,
            },
            thousands_separator: format.thousands_separator,
        }
    }
}

pub(crate) fn optional<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<NumberFormatDto>, D::Error> {
    NumberFormatDto::deserialize(deserializer).map(Some)
}

/// Direct `.ro/v2` normalizes every JSON number to binary64 before decoding, so an
/// integer `decimals` arrives as `2.0`. Only an exactly integral value that fits `u8`
/// is rewritten to an integer; everything else still goes through the strict DTO.
pub(crate) fn optional_v2<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<NumberFormatDto>, D::Error> {
    let mut value = serde_json::Value::deserialize(deserializer)?;
    if let Some(decimals) = value.get("decimals").and_then(serde_json::Value::as_f64)
        && let Ok(integer) = decimals.to_string().parse::<u8>()
    {
        value["decimals"] = integer.into();
    }
    serde_json::from_value(value)
        .map(Some)
        .map_err(serde::de::Error::custom)
}

pub(crate) fn write(
    output: &mut String,
    format: &NumberFormatDto,
    indent: usize,
) -> Result<(), serde_json::Error> {
    let json = serde_json::to_string_pretty(format)?;
    for (index, line) in json.lines().enumerate() {
        if index != 0 {
            output.push('\n');
            output.push_str(&"  ".repeat(indent));
        }
        output.push_str(line);
    }
    Ok(())
}
