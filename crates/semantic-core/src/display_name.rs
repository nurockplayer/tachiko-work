//! Frozen ADR-0043 spelling rules shared by authoring and admission.

pub const MAX_DISPLAY_NAME_BYTES: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayNameFailure {
    Empty,
    TooLong,
    ForbiddenScalar,
    Untrimmed,
}

impl DisplayNameFailure {
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::Empty => "empty_display_name",
            Self::TooLong => "display_name_too_long",
            Self::ForbiddenScalar => "forbidden_display_name_scalar",
            Self::Untrimmed => "untrimmed_display_name",
        }
    }
}

#[must_use]
pub fn is_display_name_whitespace(scalar: char) -> bool {
    matches!(scalar, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{0085}' | '\u{00a0}'
        | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}'
        | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

#[must_use]
pub fn trim_display_name(input: &str) -> &str {
    input.trim_matches(is_display_name_whitespace)
}

/// Check canonical name spelling, accumulating independent failures.
/// Empty/length checks use the frozen trim result; untrimmed input is still
/// rejected rather than repaired. Authoring adapters trim before calling this.
#[must_use]
pub fn display_name_failures(name: &str) -> Vec<DisplayNameFailure> {
    let mut failures = Vec::new();
    let trimmed = trim_display_name(name);
    if trimmed.is_empty() {
        failures.push(DisplayNameFailure::Empty);
    }
    if trimmed.len() > MAX_DISPLAY_NAME_BYTES {
        failures.push(DisplayNameFailure::TooLong);
    }
    if name.chars().any(|scalar| matches!(scalar, '\u{0000}'..='\u{001f}' | '\u{007f}'..='\u{009f}' | '\u{2028}' | '\u{2029}')) {
        failures.push(DisplayNameFailure::ForbiddenScalar);
    }
    if trimmed != name {
        failures.push(DisplayNameFailure::Untrimmed);
    }
    failures
}
