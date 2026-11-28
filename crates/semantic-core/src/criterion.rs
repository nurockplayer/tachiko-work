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

/// The column-index key of one cell or one resolved test.
///
/// A test's keys are a superset filter: a cell the test matches always carries
/// one of them, so an index built from these keys may hold extra cells but
/// never misses a match.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CriterionKey {
    /// A number by bits, with `-0.0` stored as `0.0` because `==` treats the
    /// two signs as equal.
    Number(u64),
    /// The text under the same case fold the wildcard match compares with.
    Text(String),
    Boolean(bool),
    Date(Date),
}

impl CriterionKey {
    fn number(value: f64) -> Self {
        let value = if value == 0.0 { 0.0 } else { value };
        Self::Number(value.to_bits())
    }
}

impl CriterionCell<'_> {
    /// The keys a column index files this cell under: its own type's key, plus
    /// a number key for text a number operand reads as a number.
    #[must_use]
    pub fn keys(self) -> Vec<CriterionKey> {
        match self {
            Self::Number(number) => vec![CriterionKey::number(number)],
            Self::Text(text) => {
                let mut keys = vec![CriterionKey::Text(lowercase_text(text))];
                if let Some(number) = parse_number_cell(text) {
                    keys.push(CriterionKey::number(number));
                }
                keys
            }
            Self::Boolean(boolean) => vec![CriterionKey::Boolean(boolean)],
            Self::Date(date) => vec![CriterionKey::Date(date)],
        }
    }
}

/// One resolved test's identity: equal keys match exactly the same cells.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CriterionTestKey {
    operator: u8,
    operand: CriterionOperandKey,
    text: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CriterionOperandKey {
    /// The operand by bits, with no zero-sign normalising: only `==` compares
    /// operands, so both signs are safe either way.
    Number(u64),
    Text,
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

/// What a criterion compares against, for checks against a column's type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CriterionOperandKind {
    Number,
    Text,
    Boolean,
    Date,
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
        // Match longer prefixes before their single-character forms.
        const OPERATOR_PREFIXES: [(&str, ComparisonOperator); 6] = [
            (">=", ComparisonOperator::GreaterOrEqual),
            ("<=", ComparisonOperator::LessOrEqual),
            ("<>", ComparisonOperator::NotEqual),
            (">", ComparisonOperator::Greater),
            ("<", ComparisonOperator::Less),
            ("=", ComparisonOperator::Equal),
        ];
        let (operator, remainder) = OPERATOR_PREFIXES
            .into_iter()
            .find_map(|(prefix, operator)| text.strip_prefix(prefix).map(|rest| (operator, rest)))
            .unwrap_or((ComparisonOperator::Equal, text));
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

    /// What this criterion compares against.
    #[must_use]
    pub const fn operand_kind(&self) -> CriterionOperandKind {
        match self.operand {
            Operand::Number(_) => CriterionOperandKind::Number,
            Operand::Text => CriterionOperandKind::Text,
            Operand::Boolean(_) => CriterionOperandKind::Boolean,
            Operand::Date(_) => CriterionOperandKind::Date,
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

    /// The column-index keys a cell this test matches may be filed under, or
    /// `None` when the test has to read every cell.
    ///
    /// Every key is derived from a comparison this test actually makes: `=`
    /// only, a number operand, a literal text operand (blanks and wildcards
    /// have no key), and the text spelling a boolean or date operand also
    /// compares with.
    #[must_use]
    pub fn keys(&self) -> Option<Vec<CriterionKey>> {
        if self.operator != ComparisonOperator::Equal {
            return None;
        }
        match self.operand {
            Operand::Number(number) => Some(vec![CriterionKey::number(number)]),
            // The empty text matches blank cells, which carry no key.
            Operand::Text if self.text.is_empty() => None,
            Operand::Text => self.text_keys(),
            Operand::Boolean(boolean) => {
                let mut keys = self.text_keys()?;
                keys.insert(0, CriterionKey::Boolean(boolean));
                Some(keys)
            }
            Operand::Date(date) => {
                let mut keys = self.text_keys()?;
                keys.insert(0, CriterionKey::Date(date));
                Some(keys)
            }
        }
    }

    /// The identity of this resolved test: two equal keys agree on every cell.
    #[must_use]
    pub fn key(&self) -> CriterionTestKey {
        CriterionTestKey {
            operator: operator_order(self.operator),
            operand: match self.operand {
                Operand::Number(number) => CriterionOperandKey::Number(number.to_bits()),
                Operand::Text => CriterionOperandKey::Text,
                Operand::Boolean(boolean) => CriterionOperandKey::Boolean(boolean),
                Operand::Date(date) => CriterionOperandKey::Date(date),
            },
            text: self.text.clone(),
        }
    }

    /// The text-cell key of the operand text, `None` when the text holds a
    /// wildcard or an escape and so has to be matched cell by cell.
    fn text_keys(&self) -> Option<Vec<CriterionKey>> {
        if has_wildcard(&self.text) {
            return None;
        }
        Some(vec![CriterionKey::Text(lowercase_text(&self.text))])
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

/// A stable order over operators for a memo key; the values mean nothing else.
const fn operator_order(operator: ComparisonOperator) -> u8 {
    match operator {
        ComparisonOperator::Equal => 0,
        ComparisonOperator::NotEqual => 1,
        ComparisonOperator::Less => 2,
        ComparisonOperator::LessOrEqual => 3,
        ComparisonOperator::Greater => 4,
        ComparisonOperator::GreaterOrEqual => 5,
    }
}

fn has_wildcard(text: &str) -> bool {
    text.chars()
        .any(|character| matches!(character, '*' | '?' | '~'))
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

/// The same fold as [`lowercase`], kept as text for a key.
fn lowercase_text(text: &str) -> String {
    text.chars().flat_map(char::to_lowercase).collect()
}

#[cfg(test)]
mod tests;
