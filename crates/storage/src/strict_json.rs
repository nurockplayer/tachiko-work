use std::collections::BTreeSet;

use serde::de::{self, DeserializeOwned, IgnoredAny};
use tachiko_semantic_core::MAX_EXPRESSION_DEPTH;

// The supported direct-ro representations can reach 132 JSON containers at
// the accepted 64-node formula depth. Keep a bounded safety margin above every
// valid current representation before disabling serde_json's lower fixed
// recursion limit for version-specific DTO materialization.
const MAX_SUPPORTED_JSON_NESTING_DEPTH: usize = MAX_EXPRESSION_DEPTH * 3;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Inspection {
    pub(crate) version: Option<VersionToken>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VersionToken {
    Unsigned(u64),
    Other,
}

#[derive(Debug)]
pub(crate) enum FrontendError {
    InvalidJson(serde_json::Error),
    DuplicateMember(String),
}

/// Parse a single JSON value, rejecting duplicate object members.
///
/// The front-end uses an explicit container stack so syntactically valid future
/// representations cannot trip `serde_json`'s recursive depth guard before
/// version dispatch. It records a duplicate but completes syntax validation so
/// malformed JSON retains precedence over an earlier duplicate.
pub(crate) fn inspect(source: &str) -> Result<Inspection, FrontendError> {
    Scanner::new(source).inspect()
}

/// Deserialize already-inspected, supported JSON while retaining a bounded
/// nesting admission above every valid current representation.
pub(crate) fn deserialize<T>(source: &str) -> Result<T, serde_json::Error>
where
    T: DeserializeOwned,
{
    enforce_supported_nesting_limit(source)?;
    let mut deserializer = serde_json::Deserializer::from_str(source);
    deserializer.disable_recursion_limit();
    let value = T::deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(value)
}

fn enforce_supported_nesting_limit(source: &str) -> Result<(), serde_json::Error> {
    let mut depth = 0_usize;
    let mut in_string = false;
    let mut escaped = false;

    for byte in source.bytes() {
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
                depth += 1;
                if depth > MAX_SUPPORTED_JSON_NESTING_DEPTH {
                    return Err(<serde_json::Error as de::Error>::custom(format!(
                        "supported representation nesting exceeds {MAX_SUPPORTED_JSON_NESTING_DEPTH} containers"
                    )));
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }

    Ok(())
}

#[derive(Clone, Copy)]
enum ArrayState {
    FirstValueOrEnd,
    ValueRequired,
    CommaOrEnd,
}

#[derive(Clone, Copy)]
enum ObjectState {
    FirstKeyOrEnd,
    KeyRequired,
    Colon,
    Value,
    CommaOrEnd,
}

enum Frame {
    Array { state: ArrayState },
    Object(Box<ObjectFrame>),
}

struct ObjectFrame {
    state: ObjectState,
    members: BTreeSet<String>,
    current_key: Option<String>,
    root: bool,
}

#[derive(Clone, Copy)]
enum Step {
    Array(ArrayState),
    Object(ObjectState),
}

struct Scanner<'a> {
    source: &'a str,
    bytes: &'a [u8],
    index: usize,
    frames: Vec<Frame>,
    root_started: bool,
    root_finished: bool,
    version: Option<VersionToken>,
    duplicate: Option<String>,
}

impl<'a> Scanner<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            index: 0,
            frames: Vec::new(),
            root_started: false,
            root_finished: false,
            version: None,
            duplicate: None,
        }
    }

    fn inspect(mut self) -> Result<Inspection, FrontendError> {
        loop {
            self.skip_whitespace();

            if self.frames.is_empty() {
                if self.root_finished {
                    break;
                }
                if self.root_started {
                    return Err(FrontendError::InvalidJson(
                        self.syntax_error("incomplete JSON value"),
                    ));
                }
                self.root_started = true;
                self.parse_value(true).map_err(FrontendError::InvalidJson)?;
                continue;
            }

            let step = match self.frames.last() {
                Some(Frame::Array { state }) => Step::Array(*state),
                Some(Frame::Object(frame)) => Step::Object(frame.state),
                None => {
                    return Err(FrontendError::InvalidJson(
                        self.syntax_error("container parser state is inconsistent"),
                    ));
                }
            };
            match step {
                Step::Array(state) => self.step_array(state).map_err(FrontendError::InvalidJson)?,
                Step::Object(state) => self
                    .step_object(state)
                    .map_err(FrontendError::InvalidJson)?,
            }
        }

        self.skip_whitespace();
        if self.index != self.bytes.len() {
            return Err(FrontendError::InvalidJson(
                self.syntax_error("trailing characters after the JSON value"),
            ));
        }
        if let Some(member) = self.duplicate {
            return Err(FrontendError::DuplicateMember(member));
        }

        Ok(Inspection {
            version: self.version,
        })
    }

    fn step_array(&mut self, state: ArrayState) -> Result<(), serde_json::Error> {
        match state {
            ArrayState::FirstValueOrEnd => {
                if self.consume_if(b']') {
                    self.close_container()?;
                } else {
                    self.parse_value(false)?;
                }
            }
            ArrayState::ValueRequired => {
                if self.peek() == Some(b']') {
                    return Err(self.syntax_error("array value required after comma"));
                }
                self.parse_value(false)?;
            }
            ArrayState::CommaOrEnd => match self.peek() {
                Some(b',') => {
                    self.index += 1;
                    self.set_array_state(ArrayState::ValueRequired)?;
                }
                Some(b']') => {
                    self.index += 1;
                    self.close_container()?;
                }
                _ => return Err(self.syntax_error("expected ',' or ']' after array value")),
            },
        }
        Ok(())
    }

    fn step_object(&mut self, state: ObjectState) -> Result<(), serde_json::Error> {
        match state {
            ObjectState::FirstKeyOrEnd => {
                if self.consume_if(b'}') {
                    self.close_container()?;
                } else {
                    self.parse_object_key()?;
                }
            }
            ObjectState::KeyRequired => {
                if self.peek() == Some(b'}') {
                    return Err(self.syntax_error("object member required after comma"));
                }
                self.parse_object_key()?;
            }
            ObjectState::Colon => {
                if !self.consume_if(b':') {
                    return Err(self.syntax_error("expected ':' after object member name"));
                }
                self.set_object_state(ObjectState::Value)?;
            }
            ObjectState::Value => self.parse_value(false)?,
            ObjectState::CommaOrEnd => match self.peek() {
                Some(b',') => {
                    self.index += 1;
                    self.set_object_state(ObjectState::KeyRequired)?;
                }
                Some(b'}') => {
                    self.index += 1;
                    self.close_container()?;
                }
                _ => return Err(self.syntax_error("expected ',' or '}' after object value")),
            },
        }
        Ok(())
    }

    fn parse_object_key(&mut self) -> Result<(), serde_json::Error> {
        if self.peek() != Some(b'"') {
            return Err(self.syntax_error("object member name must be a string"));
        }
        let key = self
            .consume_string(true)?
            .ok_or_else(|| self.syntax_error("object member name was not decoded"))?;

        let duplicate = {
            let Some(Frame::Object(frame)) = self.frames.last_mut() else {
                return Err(self.syntax_error("object parser state is inconsistent"));
            };
            let duplicate = !frame.members.insert(key.clone());
            frame.current_key = Some(key.clone());
            frame.state = ObjectState::Colon;
            duplicate
        };
        if duplicate && self.duplicate.is_none() {
            self.duplicate = Some(key);
        }
        Ok(())
    }

    fn parse_value(&mut self, root: bool) -> Result<(), serde_json::Error> {
        self.skip_whitespace();
        let Some(byte) = self.peek() else {
            return Err(self.syntax_error("expected a JSON value"));
        };

        match byte {
            b'{' => {
                self.index += 1;
                self.frames.push(Frame::Object(Box::new(ObjectFrame {
                    state: ObjectState::FirstKeyOrEnd,
                    members: BTreeSet::new(),
                    current_key: None,
                    root,
                })));
            }
            b'[' => {
                self.index += 1;
                self.frames.push(Frame::Array {
                    state: ArrayState::FirstValueOrEnd,
                });
            }
            b'"' => {
                self.consume_string(false)?;
                self.finish_value(VersionToken::Other)?;
            }
            b't' => {
                self.consume_literal(b"true")?;
                self.finish_value(VersionToken::Other)?;
            }
            b'f' => {
                self.consume_literal(b"false")?;
                self.finish_value(VersionToken::Other)?;
            }
            b'n' => {
                self.consume_literal(b"null")?;
                self.finish_value(VersionToken::Other)?;
            }
            b'-' | b'0'..=b'9' => {
                let version = self.consume_number()?;
                self.finish_value(version)?;
            }
            _ => return Err(self.syntax_error("expected a JSON value")),
        }
        Ok(())
    }

    fn consume_string(&mut self, decode: bool) -> Result<Option<String>, serde_json::Error> {
        let start = self.index;
        self.index += 1;

        while let Some(byte) = self.peek() {
            match byte {
                b'"' => {
                    self.index += 1;
                    let token = &self.source[start..self.index];
                    return if decode {
                        serde_json::from_str::<String>(token).map(Some)
                    } else {
                        serde_json::from_str::<IgnoredAny>(token).map(|_| None)
                    };
                }
                b'\\' => {
                    self.index += 1;
                    if self.peek().is_none() {
                        return Err(self.syntax_error("unterminated JSON string escape"));
                    }
                    self.index += 1;
                }
                0x00..=0x1f => {
                    return Err(self.syntax_error("unescaped control character in JSON string"));
                }
                _ => self.index += 1,
            }
        }

        Err(self.syntax_error("unterminated JSON string"))
    }

    fn consume_literal(&mut self, literal: &[u8]) -> Result<(), serde_json::Error> {
        if !self.bytes[self.index..].starts_with(literal) {
            return Err(self.syntax_error("invalid JSON literal"));
        }
        self.index += literal.len();
        self.require_value_delimiter()
    }

    fn consume_number(&mut self) -> Result<VersionToken, serde_json::Error> {
        let start = self.index;
        let negative = self.consume_if(b'-');
        let Some(first_digit) = self.peek() else {
            return Err(self.syntax_error("number requires an integer component"));
        };

        match first_digit {
            b'0' => {
                self.index += 1;
                if matches!(self.peek(), Some(b'0'..=b'9')) {
                    return Err(
                        self.syntax_error("leading zeros are not permitted in JSON numbers")
                    );
                }
            }
            b'1'..=b'9' => {
                self.index += 1;
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.index += 1;
                }
            }
            _ => return Err(self.syntax_error("number requires an integer component")),
        }

        let fraction = if self.consume_if(b'.') {
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.syntax_error("number fraction requires at least one digit"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.index += 1;
            }
            true
        } else {
            false
        };

        let exponent = if matches!(self.peek(), Some(b'e' | b'E')) {
            self.index += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.index += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.syntax_error("number exponent requires at least one digit"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.index += 1;
            }
            true
        } else {
            false
        };

        self.require_value_delimiter()?;
        if negative || fraction || exponent {
            return Ok(VersionToken::Other);
        }

        Ok(self.source[start..self.index]
            .parse::<u64>()
            .map_or(VersionToken::Other, VersionToken::Unsigned))
    }

    fn require_value_delimiter(&self) -> Result<(), serde_json::Error> {
        if matches!(
            self.peek(),
            None | Some(b' ' | b'\t' | b'\r' | b'\n' | b',' | b']' | b'}')
        ) {
            Ok(())
        } else {
            Err(self.syntax_error("invalid character after JSON scalar"))
        }
    }

    fn close_container(&mut self) -> Result<(), serde_json::Error> {
        self.frames
            .pop()
            .ok_or_else(|| self.syntax_error("unexpected container terminator"))?;
        self.finish_value(VersionToken::Other)
    }

    fn finish_value(&mut self, token: VersionToken) -> Result<(), serde_json::Error> {
        let mut root_version = None;
        if let Some(frame) = self.frames.last_mut() {
            match frame {
                Frame::Array { state } => match state {
                    ArrayState::FirstValueOrEnd | ArrayState::ValueRequired => {
                        *state = ArrayState::CommaOrEnd;
                    }
                    ArrayState::CommaOrEnd => {
                        return Err(self.syntax_error("array parser state is inconsistent"));
                    }
                },
                Frame::Object(frame) => {
                    if !matches!(frame.state, ObjectState::Value) {
                        return Err(self.syntax_error("object parser state is inconsistent"));
                    }
                    if frame.root && frame.current_key.as_deref() == Some("format_version") {
                        root_version = Some(token);
                    }
                    frame.current_key = None;
                    frame.state = ObjectState::CommaOrEnd;
                }
            }
        } else {
            self.root_finished = true;
        }
        if let Some(version) = root_version {
            self.version = Some(version);
        }
        Ok(())
    }

    fn set_array_state(&mut self, next: ArrayState) -> Result<(), serde_json::Error> {
        let Some(Frame::Array { state }) = self.frames.last_mut() else {
            return Err(self.syntax_error("array parser state is inconsistent"));
        };
        *state = next;
        Ok(())
    }

    fn set_object_state(&mut self, next: ObjectState) -> Result<(), serde_json::Error> {
        let Some(Frame::Object(frame)) = self.frames.last_mut() else {
            return Err(self.syntax_error("object parser state is inconsistent"));
        };
        frame.state = next;
        Ok(())
    }

    fn consume_if(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.index += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.index).copied()
    }

    fn syntax_error(&self, message: &str) -> serde_json::Error {
        <serde_json::Error as de::Error>::custom(format!("{message} at byte offset {}", self.index))
    }
}

#[cfg(test)]
mod tests {
    use super::{FrontendError, VersionToken, inspect};

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
    fn validates_deep_arrays_and_objects_without_recursive_descent() {
        let depth = 10_000;
        let array = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
        let object = format!("{}0{}", "{\"a\":".repeat(depth), "}".repeat(depth));

        assert!(inspect(&array).unwrap().version.is_none());
        assert!(inspect(&object).unwrap().version.is_none());
    }

    #[test]
    fn scanner_syntax_matches_serde_json_across_deterministic_mutations() {
        const CORPUS: &[&str] = &[
            "null",
            "true",
            "false",
            "0",
            "-0",
            "1",
            "1.0",
            "1e0",
            r#"""#,
            r#""a\\n\\u0062""#,
            "[]",
            "{}",
            r#"[0,true,null,"x"]"#,
            r#"{"format_version":2,"a":[1,{"b":"c"}]}"#,
            r#"{"a":1,"a":2}"#,
        ];
        const ALPHABET: &[u8] = br#"{}[]:,"\\tfn01-.e+a \n"#;

        fn bounded_index(value: u64, bound: usize) -> usize {
            let bound = u64::try_from(bound).unwrap();
            usize::try_from(value % bound).unwrap()
        }

        let mut state = 0x6a09_e667_f3bc_c909_u64;
        for case in 0..100_000 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let mut candidate = CORPUS[bounded_index(state, CORPUS.len())]
                .as_bytes()
                .to_vec();
            let mutation_count = 1 + bounded_index(state >> 8, 3);
            for _ in 0..mutation_count {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let operation = bounded_index(state >> 16, 3);
                let position = if candidate.is_empty() {
                    0
                } else {
                    bounded_index(state >> 24, candidate.len() + 1)
                };
                let byte = ALPHABET[bounded_index(state >> 40, ALPHABET.len())];
                match operation {
                    0 => candidate.insert(position, byte),
                    1 if !candidate.is_empty() => {
                        candidate.remove(position.min(candidate.len() - 1));
                    }
                    2 if !candidate.is_empty() => {
                        let index = position.min(candidate.len() - 1);
                        candidate[index] = byte;
                    }
                    _ => candidate.push(byte),
                }
            }

            let candidate = String::from_utf8(candidate).unwrap();
            let oracle_valid = serde_json::from_str::<serde_json::Value>(&candidate).is_ok();
            let scanner_valid = !matches!(inspect(&candidate), Err(FrontendError::InvalidJson(_)));
            assert_eq!(
                scanner_valid,
                oracle_valid,
                "case {case} disagreed for {candidate:?}: scanner={:?}",
                inspect(&candidate)
            );
        }
    }

    #[test]
    fn deep_invalid_json_still_beats_an_earlier_duplicate() {
        let depth = 10_000;
        let source = format!(
            "{{\"a\":1,\"a\":2,\"future\":{}0{}}}",
            "[".repeat(depth),
            "]".repeat(depth - 1)
        );

        assert!(matches!(
            inspect(&source),
            Err(FrontendError::InvalidJson(_))
        ));
    }
}
