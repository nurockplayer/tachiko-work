use std::{collections::HashSet, fmt};

use serde::{
    Deserialize,
    de::{self, DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor},
};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Inspection {
    pub(crate) version: Option<VersionToken>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum VersionToken {
    Unsigned(u64),
    Other,
}

#[derive(Debug)]
pub(crate) enum FrontendError {
    InvalidJson(serde_json::Error),
    DuplicateMember(String),
    NestingLimit { limit: usize },
}

/// Parse a single JSON value, rejecting duplicate object members.
///
/// A validation pass is deliberately completed before duplicate detection so a
/// JSON syntax error takes precedence over any duplicate found earlier in the
/// input. Both passes stream through the source and discard values as they go.
pub(crate) fn inspect(source: &str) -> Result<Inspection, FrontendError> {
    inspect_with_recursion(source, false)
}

/// Inspect `.roproj` JSON with a representation-derived nesting bound.
///
/// The byte preflight ignores structural characters inside JSON strings, so
/// serde's recursion limit can be disabled without admitting unbounded parser
/// recursion. The normal direct-`.ro` frontend continues to use [`inspect`]
/// and `serde_json`'s default recursion limit.
pub(crate) fn inspect_roproj(
    source: &str,
    maximum_nesting: usize,
) -> Result<Inspection, FrontendError> {
    if exceeds_json_nesting(source.as_bytes(), maximum_nesting) {
        return Err(FrontendError::NestingLimit {
            limit: maximum_nesting,
        });
    }
    inspect_with_recursion(source, true)
}

fn inspect_with_recursion(
    source: &str,
    disable_recursion_limit: bool,
) -> Result<Inspection, FrontendError> {
    let mut syntax_deserializer = serde_json::Deserializer::from_str(source);
    if disable_recursion_limit {
        syntax_deserializer.disable_recursion_limit();
    }
    IgnoredAny::deserialize(&mut syntax_deserializer).map_err(FrontendError::InvalidJson)?;
    syntax_deserializer
        .end()
        .map_err(FrontendError::InvalidJson)?;

    let mut duplicate = None;
    let mut deserializer = serde_json::Deserializer::from_str(source);
    if disable_recursion_limit {
        deserializer.disable_recursion_limit();
    }
    let inspection = RootSeed {
        duplicate: &mut duplicate,
    }
    .deserialize(&mut deserializer)
    .map_err(|error| match duplicate {
        Some(member) => FrontendError::DuplicateMember(member),
        None => FrontendError::InvalidJson(error),
    })?;
    deserializer.end().map_err(FrontendError::InvalidJson)?;

    Ok(inspection)
}

fn exceeds_json_nesting(source: &[u8], maximum: usize) -> bool {
    let mut nesting = 0_usize;
    let mut in_string = false;
    let mut escaped = false;
    for &byte in source {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                nesting += 1;
                if nesting > maximum {
                    return true;
                }
            }
            b'}' | b']' => nesting = nesting.saturating_sub(1),
            _ => {}
        }
    }
    false
}

struct RootSeed<'a> {
    duplicate: &'a mut Option<String>,
}

impl<'de> DeserializeSeed<'de> for RootSeed<'_> {
    type Value = Inspection;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_any(ValueVisitor {
            duplicate: self.duplicate,
            root: true,
        })
    }
}

struct ValueSeed<'a> {
    duplicate: &'a mut Option<String>,
}

impl<'de> DeserializeSeed<'de> for ValueSeed<'_> {
    type Value = Inspection;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_any(ValueVisitor {
            duplicate: self.duplicate,
            root: false,
        })
    }
}

struct VersionSeed<'a> {
    duplicate: &'a mut Option<String>,
}

impl<'de> DeserializeSeed<'de> for VersionSeed<'_> {
    type Value = VersionToken;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_any(VersionVisitor {
            duplicate: self.duplicate,
        })
    }
}

struct ValueVisitor<'a> {
    duplicate: &'a mut Option<String>,
    root: bool,
}

impl<'de> Visitor<'de> for ValueVisitor<'_> {
    type Value = Inspection;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Inspection { version: None })
    }

    fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Inspection { version: None })
    }

    fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Inspection { version: None })
    }

    fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Inspection { version: None })
    }

    fn visit_str<E>(self, _: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Inspection { version: None })
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Inspection { version: None })
    }

    fn visit_seq<A>(self, sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        inspect_array(sequence, self.duplicate)?;
        Ok(Inspection { version: None })
    }

    fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        inspect_object(map, self.duplicate, self.root)
    }
}

struct VersionVisitor<'a> {
    duplicate: &'a mut Option<String>,
}

impl<'de> Visitor<'de> for VersionVisitor<'_> {
    type Value = VersionToken;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(VersionToken::Other)
    }

    fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(VersionToken::Other)
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(VersionToken::Unsigned(value))
    }

    fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(VersionToken::Other)
    }

    fn visit_str<E>(self, _: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(VersionToken::Other)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(VersionToken::Other)
    }

    fn visit_seq<A>(self, sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        inspect_array(sequence, self.duplicate)?;
        Ok(VersionToken::Other)
    }

    fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        inspect_object(map, self.duplicate, false)?;
        Ok(VersionToken::Other)
    }
}

fn inspect_array<'de, A>(mut sequence: A, duplicate: &mut Option<String>) -> Result<(), A::Error>
where
    A: SeqAccess<'de>,
{
    while sequence
        .next_element_seed(ValueSeed { duplicate })?
        .is_some()
    {}
    Ok(())
}

fn inspect_object<'de, A>(
    mut map: A,
    duplicate: &mut Option<String>,
    root: bool,
) -> Result<Inspection, A::Error>
where
    A: MapAccess<'de>,
{
    let mut members = HashSet::new();
    let mut version = None;

    while let Some(member) = map.next_key::<String>()? {
        if !members.insert(member.clone()) {
            *duplicate = Some(member);
            return Err(de::Error::custom("duplicate JSON object member"));
        }

        if root && member == "format_version" {
            version = Some(map.next_value_seed(VersionSeed { duplicate })?);
        } else {
            map.next_value_seed(ValueSeed { duplicate })?;
        }
    }

    Ok(Inspection {
        version: root.then_some(version).flatten(),
    })
}

#[cfg(test)]
mod tests {
    use super::{FrontendError, VersionToken, inspect, inspect_roproj};

    #[test]
    fn records_a_root_unsigned_format_version() {
        let inspection = inspect(r#"{"format_version":1,"future":{"nested":true}}"#).unwrap();

        assert!(matches!(
            inspection.version,
            Some(VersionToken::Unsigned(1))
        ));
    }

    #[test]
    fn reports_duplicates_after_decoding_escapes_at_any_depth() {
        let error = inspect(r#"{"format_version":2,"future":{"a":1,"\u0061":2}}"#).unwrap_err();

        assert!(matches!(error, FrontendError::DuplicateMember(member) if member == "a"));
    }

    #[test]
    fn invalid_json_wins_over_an_earlier_duplicate() {
        let error = inspect(r#"{"format_version":1,"format_version":1,"future":[}"#).unwrap_err();

        assert!(matches!(error, FrontendError::InvalidJson(_)));
    }

    #[test]
    fn distinguishes_unsigned_versions_from_other_json_representations() {
        for source in [
            r#"{"format_version":"1"}"#,
            r#"{"format_version":1.0}"#,
            r#"{"format_version":1e0}"#,
            r#"{"format_version":-0}"#,
        ] {
            let inspection = inspect(source).unwrap();
            assert!(matches!(inspection.version, Some(VersionToken::Other)));
        }

        assert!(matches!(
            inspect(r#"{"format_version":0}"#).unwrap().version,
            Some(VersionToken::Unsigned(0))
        ));
    }

    #[test]
    fn ignores_versions_outside_the_root_object_and_accepts_only_one_value() {
        assert!(
            inspect(r#"[{"format_version":1}]"#)
                .unwrap()
                .version
                .is_none()
        );
        assert!(matches!(
            inspect(r#"{"format_version":1} trailing"#),
            Err(FrontendError::InvalidJson(_))
        ));
    }

    #[test]
    fn roproj_inspection_bounds_nesting_but_ignores_string_contents() {
        assert!(inspect_roproj(r#"{"text":"[[{{\\\"}}]]"}"#, 1).is_ok());
        assert!(matches!(
            inspect_roproj(r#"{"nested":[{}]}"#, 2),
            Err(FrontendError::NestingLimit { limit: 2 })
        ));
    }

    #[test]
    fn roproj_inspection_keeps_syntax_precedence_and_deep_escape_duplicates() {
        let malformed = r#"{"a":1,"a":2,"nested":[}"#;
        assert!(matches!(
            inspect_roproj(malformed, 132),
            Err(FrontendError::InvalidJson(_))
        ));

        let mut source = String::new();
        for _ in 0..64 {
            source.push_str("{\"nested\":");
        }
        source.push_str(r#"{"a":1,"\u0061":2}"#);
        source.push_str(&"}".repeat(64));
        assert!(matches!(
            inspect_roproj(&source, 65),
            Err(FrontendError::DuplicateMember(member)) if member == "a"
        ));
    }
}
