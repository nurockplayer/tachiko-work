//! Excel-style criteria text for conditional aggregates.

use std::fmt;

use crate::{ComparisonOperator, Date};

/// Maximum number of criteria admitted in one conditional aggregate node.
pub const MAX_CONDITIONAL_CRITERIA: usize = 16;

/// A criterion text that cannot be evaluated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidCriterion {
    message: String,
}

impl InvalidCriterion {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for InvalidCriterion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for InvalidCriterion {}

/// One cell read by a criterion test.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CriterionCell<'a> {
    Number(f64),
    Text(&'a str),
    Boolean(bool),
    Date(Date),
}

/// A parsed criterion text: an operator and the operand that follows it.
#[derive(Clone, Debug, PartialEq)]
pub struct CriterionTest {
    operator: ComparisonOperator,
    operand: Operand,
    // The text after the operator, kept for the wildcard match against Text
    // cells. Excel matches the literal operand text, not its classification.
    text: String,
}

#[derive(Clone, Debug, PartialEq)]
enum Operand {
    Number(f64),
    Text,
    Boolean(bool),
    Date(Date),
}

impl CriterionTest {
    /// Parse `OPERATOR operand`, defaulting to `=` when no operator is given.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidCriterion`] for a relational operator on text, a
    /// boolean, or an empty operand.
    pub fn parse(text: &str) -> Result<Self, InvalidCriterion> {
        let (operator, remainder) = if let Some(rest) = text.strip_prefix(">=") {
            (ComparisonOperator::GreaterOrEqual, rest)
        } else if let Some(rest) = text.strip_prefix("<=") {
            (ComparisonOperator::LessOrEqual, rest)
        } else if let Some(rest) = text.strip_prefix("<>") {
            (ComparisonOperator::NotEqual, rest)
        } else if let Some(rest) = text.strip_prefix('>') {
            (ComparisonOperator::Greater, rest)
        } else if let Some(rest) = text.strip_prefix('<') {
            (ComparisonOperator::Less, rest)
        } else if let Some(rest) = text.strip_prefix('=') {
            (ComparisonOperator::Equal, rest)
        } else {
            (ComparisonOperator::Equal, text)
        };
        let operand = classify(remainder);
        if is_relational(operator) && matches!(operand, Operand::Text | Operand::Boolean(_)) {
            return Err(InvalidCriterion::new(
                "a relational criterion requires a number or a date",
            ));
        }
        Ok(Self {
            operator,
            operand,
            text: remainder.to_owned(),
        })
    }

    /// The operator this criterion applies.
    #[must_use]
    pub fn operator(&self) -> ComparisonOperator {
        self.operator
    }

    /// The number operand, when the criterion text names one.
    #[must_use]
    pub fn number_operand(&self) -> Option<f64> {
        match self.operand {
            Operand::Number(value) => Some(value),
            Operand::Text | Operand::Boolean(_) | Operand::Date(_) => None,
        }
    }

    /// Whether the criterion tests for the blank cell text, `""`.
    #[must_use]
    pub fn is_blank(&self) -> bool {
        matches!(self.operand, Operand::Text) && self.text.is_empty()
    }

    /// A test for a blank cell: `""` with `Equal` and `"<>"` with `NotEqual`.
    #[must_use]
    pub fn blank(operator: ComparisonOperator) -> Self {
        Self {
            operator,
            operand: Operand::Text,
            text: String::new(),
        }
    }

    /// A test that compares a number with `operator`.
    #[must_use]
    pub fn number(operator: ComparisonOperator, value: f64) -> Self {
        Self {
            operator,
            operand: Operand::Number(value),
            text: value.to_string(),
        }
    }

    /// A test that a cell equals a number.
    #[must_use]
    pub fn equal_number(value: f64) -> Self {
        Self::equal(Operand::Number(value), value.to_string())
    }

    /// A test that a cell equals a boolean.
    #[must_use]
    pub fn equal_boolean(value: bool) -> Self {
        Self::equal(Operand::Boolean(value), value.to_string())
    }

    /// A test that a cell equals a date.
    #[must_use]
    pub fn equal_date(value: Date) -> Self {
        Self::equal(Operand::Date(value), value.to_canonical_string())
    }

    fn equal(operand: Operand, text: String) -> Self {
        Self {
            operator: ComparisonOperator::Equal,
            operand,
            text,
        }
    }

    /// Whether the cell satisfies the criterion.
    #[must_use]
    pub fn matches(&self, cell: Option<CriterionCell<'_>>) -> bool {
        if is_relational(self.operator) {
            return self.matches_relational(cell);
        }
        let equal = self.matches_equal(cell);
        if self.operator == ComparisonOperator::NotEqual {
            return !equal;
        }
        equal
    }

    #[allow(clippy::float_cmp)] // equality of finite values is the comparison
    fn matches_equal(&self, cell: Option<CriterionCell<'_>>) -> bool {
        match cell {
            None => self.text.is_empty() && matches!(&self.operand, Operand::Text),
            Some(CriterionCell::Number(number)) => match self.operand {
                Operand::Number(operand) => number == operand,
                _ => false,
            },
            Some(CriterionCell::Text(text)) => match self.operand {
                Operand::Number(operand) => {
                    parse_number_cell(text).is_some_and(|number| number == operand)
                }
                _ => wildcard_match(text, &self.text),
            },
            Some(CriterionCell::Boolean(boolean)) => {
                matches!(self.operand, Operand::Boolean(operand) if boolean == operand)
            }
            Some(CriterionCell::Date(date)) => {
                matches!(self.operand, Operand::Date(operand) if date == operand)
            }
        }
    }

    fn matches_relational(&self, cell: Option<CriterionCell<'_>>) -> bool {
        match (cell, &self.operand) {
            (Some(CriterionCell::Number(left)), Operand::Number(right)) => {
                compare_values(self.operator, left.partial_cmp(right))
            }
            (Some(CriterionCell::Date(left)), Operand::Date(right)) => {
                compare_values(self.operator, Some(left.cmp(right)))
            }
            _ => false,
        }
    }
}

fn is_relational(operator: ComparisonOperator) -> bool {
    matches!(
        operator,
        ComparisonOperator::Less
            | ComparisonOperator::LessOrEqual
            | ComparisonOperator::Greater
            | ComparisonOperator::GreaterOrEqual
    )
}

fn compare_values(operator: ComparisonOperator, ordering: Option<std::cmp::Ordering>) -> bool {
    use std::cmp::Ordering::{Equal, Greater, Less};

    let Some(ordering) = ordering else {
        return false;
    };
    match operator {
        ComparisonOperator::Less => ordering == Less,
        ComparisonOperator::LessOrEqual => ordering != Greater,
        ComparisonOperator::Greater => ordering == Greater,
        ComparisonOperator::GreaterOrEqual => ordering != Less,
        ComparisonOperator::Equal | ComparisonOperator::NotEqual => ordering == Equal,
    }
}

fn classify(text: &str) -> Operand {
    if let Some(number) = parse_number_cell(text) {
        return Operand::Number(number);
    }
    if let Ok(date) = Date::parse(text) {
        return Operand::Date(date);
    }
    if text.eq_ignore_ascii_case("true") {
        return Operand::Boolean(true);
    }
    if text.eq_ignore_ascii_case("false") {
        return Operand::Boolean(false);
    }
    Operand::Text
}

/// The order-preserving number rule shared by criterion operands and cells:
/// a leading digit, sign or dot, and a finite f64 parse.
fn parse_number_cell(text: &str) -> Option<f64> {
    let starts_like_a_number = text
        .as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_digit() || matches!(byte, b'-' | b'+' | b'.'));
    if !starts_like_a_number {
        return None;
    }
    text.parse::<f64>().ok().filter(|number| number.is_finite())
}

/// Match a whole cell against an Excel-style wildcard pattern.
///
/// `*` matches any sequence, `?` one character, and `~` escapes `*`, `?` or
/// `~`. Both sides fold with `char::to_lowercase`. The greedy backtracking is
/// linear in the product of the lengths and never exponential.
fn wildcard_match(cell: &str, pattern: &str) -> bool {
    let cell = lowercase(cell);
    let pattern = pattern_tokens(pattern);
    let mut cell_index = 0;
    let mut pattern_index = 0;
    let mut star: Option<usize> = None;
    let mut starred_cell = 0;
    while cell_index < cell.len() {
        match pattern.get(pattern_index) {
            Some(PatternToken::Any) => {
                cell_index += 1;
                pattern_index += 1;
            }
            Some(PatternToken::Literal(literal)) if *literal == cell[cell_index] => {
                cell_index += 1;
                pattern_index += 1;
            }
            Some(PatternToken::Star) => {
                star = Some(pattern_index);
                starred_cell = cell_index;
                pattern_index += 1;
            }
            _ => {
                let Some(star_index) = star else {
                    return false;
                };
                starred_cell += 1;
                cell_index = starred_cell;
                pattern_index = star_index + 1;
            }
        }
    }
    while matches!(pattern.get(pattern_index), Some(PatternToken::Star)) {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PatternToken {
    Any,
    Star,
    Literal(char),
}

fn pattern_tokens(pattern: &str) -> Vec<PatternToken> {
    let mut tokens = Vec::new();
    let mut characters = pattern.chars();
    while let Some(character) = characters.next() {
        match character {
            '*' => tokens.push(PatternToken::Star),
            '?' => tokens.push(PatternToken::Any),
            '~' => match characters.next() {
                Some(escaped @ ('*' | '?' | '~')) => push_literal(&mut tokens, escaped),
                Some(next) => {
                    push_literal(&mut tokens, '~');
                    push_literal(&mut tokens, next);
                }
                None => push_literal(&mut tokens, '~'),
            },
            literal => push_literal(&mut tokens, literal),
        }
    }
    tokens
}

fn push_literal(tokens: &mut Vec<PatternToken>, character: char) {
    tokens.extend(character.to_lowercase().map(PatternToken::Literal));
}

fn lowercase(text: &str) -> Vec<char> {
    text.chars().flat_map(char::to_lowercase).collect()
}

#[cfg(test)]
mod tests {
    use super::{CriterionCell, CriterionTest};
    use crate::Date;

    #[test]
    fn relational_criteria_refuse_text_and_booleans() {
        assert!(CriterionTest::parse(">abc").is_err());
        assert!(CriterionTest::parse(">=true").is_err());
        assert!(CriterionTest::parse(">").is_err());
        assert!(CriterionTest::parse(">=5").is_ok());
        assert!(CriterionTest::parse(">2026-01-01").is_ok());
    }

    #[test]
    fn wildcards_match_whole_cells_case_insensitively() {
        let test = CriterionTest::parse("A*").expect("valid criterion");
        assert!(test.matches(Some(CriterionCell::Text("abc"))));
        assert!(!test.matches(Some(CriterionCell::Text("ba"))));

        let test = CriterionTest::parse("?b").expect("valid criterion");
        assert!(test.matches(Some(CriterionCell::Text("AB"))));
        assert!(!test.matches(Some(CriterionCell::Text("aab"))));

        let test = CriterionTest::parse("~*").expect("valid criterion");
        assert!(test.matches(Some(CriterionCell::Text("*"))));
        assert!(!test.matches(Some(CriterionCell::Text("a"))));
    }

    #[test]
    fn blanks_match_empty_text_and_not_equal_includes_them() {
        let empty = CriterionTest::parse("").expect("valid criterion");
        assert!(empty.matches(None));
        assert!(empty.matches(Some(CriterionCell::Text(""))));
        assert!(!empty.matches(Some(CriterionCell::Text("x"))));

        let not_empty = CriterionTest::parse("<>").expect("valid criterion");
        assert!(!not_empty.matches(None));
        assert!(not_empty.matches(Some(CriterionCell::Text("x"))));

        let not_x = CriterionTest::parse("<>x").expect("valid criterion");
        assert!(not_x.matches(None));
        assert!(not_x.matches(Some(CriterionCell::Text("y"))));
        assert!(!not_x.matches(Some(CriterionCell::Text("X"))));
    }

    #[test]
    fn numeric_text_cells_compare_to_number_operands() {
        let test = CriterionTest::parse("5").expect("valid criterion");
        assert!(test.matches(Some(CriterionCell::Text("5"))));
        assert!(!test.matches(Some(CriterionCell::Text(" 5"))));
        assert!(!test.matches(Some(CriterionCell::Text("five"))));
    }

    #[test]
    fn dates_and_booleans_compare_by_value() {
        let test = CriterionTest::parse("2026-01-01").expect("valid criterion");
        assert!(test.matches(Some(CriterionCell::Date(
            Date::parse("2026-01-01").expect("valid date")
        ))));
        assert!(test.matches(Some(CriterionCell::Text("2026-01-01"))));

        let test = CriterionTest::parse("TRUE").expect("valid criterion");
        assert!(test.matches(Some(CriterionCell::Boolean(true))));
        assert!(!test.matches(Some(CriterionCell::Text("true"))));
    }
}
