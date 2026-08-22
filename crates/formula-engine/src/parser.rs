use tachiko_semantic_core::{FieldAddress, Number, is_valid_identifier};
use thiserror::Error;

pub(crate) const MAX_INPUT_BYTES: usize = 4_096;
const MAX_AST_NODES: usize = 256;
const MAX_NESTING: usize = 64;

/// Parsed formula structure whose references are still human addresses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnboundExpression {
    Number(Number),
    Reference(FieldAddress),
    Add { left: Box<Self>, right: Box<Self> },
    Subtract { left: Box<Self>, right: Box<Self> },
    Multiply { left: Box<Self>, right: Box<Self> },
    Divide { left: Box<Self>, right: Box<Self> },
    Minimum { left: Box<Self>, right: Box<Self> },
    Maximum { left: Box<Self>, right: Box<Self> },
}

/// A deterministic formula-language failure at a UTF-8 byte position.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("formula parse error at byte {position}: {message}")]
pub struct FormulaParseError {
    /// Zero-based byte position at which parsing became impossible.
    pub position: usize,
    /// Stable, user-actionable description of the expected input.
    pub message: String,
}

/// A post-desugaring expression complexity limit violation.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ExpressionComplexityError {
    /// The AST contains more than 256 expression nodes.
    #[error("expression exceeds 256-node limit")]
    NodeLimit,
    /// The AST contains a root-to-leaf path deeper than 64 nodes.
    #[error("expression exceeds 64-depth limit")]
    DepthLimit,
    /// Canonical rendering would exceed the 4,096-byte parser input limit.
    #[error("canonical expression exceeds 4096-byte limit")]
    CanonicalLengthLimit,
}

impl FormulaParseError {
    fn new(position: usize, message: impl Into<String>) -> Self {
        Self {
            position,
            message: message.into(),
        }
    }
}

/// Parse the bounded Tachiko formula language into an unbound address AST.
///
/// # Errors
///
/// Returns [`FormulaParseError`] for invalid syntax, keys, non-finite numeric
/// literals, trailing content, or a breached input, node, AST-depth,
/// canonical-byte, or syntactic-nesting limit.
pub fn parse_expression(input: &str) -> Result<UnboundExpression, FormulaParseError> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(FormulaParseError::new(
            MAX_INPUT_BYTES,
            "expression exceeds 4096-byte limit",
        ));
    }
    let expression = Parser::new(input).parse()?;
    validate_unbound_expression_structure(&expression)
        .map_err(|error| FormulaParseError::new(input.len(), error.to_string()))?;
    Ok(expression)
}

/// Validate the node, depth, and canonical-byte bounds of an unbound formula.
///
/// # Errors
///
/// Returns the first breached structural limit.
pub fn validate_unbound_expression_structure(
    expression: &UnboundExpression,
) -> Result<(), ExpressionComplexityError> {
    let mut nodes = 0_usize;
    let mut stack = vec![(expression, 1_usize)];
    while let Some((node, depth)) = stack.pop() {
        if depth > MAX_NESTING {
            return Err(ExpressionComplexityError::DepthLimit);
        }
        nodes += 1;
        if nodes > MAX_AST_NODES {
            return Err(ExpressionComplexityError::NodeLimit);
        }
        push_children(&mut stack, node, depth + 1);
    }

    if format_unbound_expression(expression).len() > MAX_INPUT_BYTES {
        return Err(ExpressionComplexityError::CanonicalLengthLimit);
    }
    Ok(())
}

/// Render an unbound expression in deterministic copy/paste-safe syntax.
#[must_use]
pub fn format_unbound_expression(expression: &UnboundExpression) -> String {
    match expression {
        UnboundExpression::Number(number) => format_number(*number),
        UnboundExpression::Reference(reference) => {
            format!("[{}.{}]", reference.entity, reference.field)
        }
        UnboundExpression::Add { left, right } => format_binary(left, "+", right),
        UnboundExpression::Subtract { left, right } => format_binary(left, "-", right),
        UnboundExpression::Multiply { left, right } => format_binary(left, "*", right),
        UnboundExpression::Divide { left, right } => format_binary(left, "/", right),
        UnboundExpression::Minimum { left, right } => format!(
            "min({}, {})",
            format_unbound_expression(left),
            format_unbound_expression(right)
        ),
        UnboundExpression::Maximum { left, right } => format!(
            "max({}, {})",
            format_unbound_expression(left),
            format_unbound_expression(right)
        ),
    }
}

fn format_binary(left: &UnboundExpression, operator: &str, right: &UnboundExpression) -> String {
    format!(
        "({} {operator} {})",
        format_unbound_expression(left),
        format_unbound_expression(right)
    )
}

pub(crate) fn format_number(number: Number) -> String {
    let value = number.get();
    let display = value.to_string();
    let scientific = format!("{value:e}");
    let scientific_round_trips = scientific.parse::<f64>().is_ok_and(|parsed| {
        Number::new(parsed).is_ok_and(|parsed| parsed.to_bits() == number.to_bits())
    });

    if scientific.len() < display.len() && scientific_round_trips {
        scientific
    } else {
        display
    }
}

fn push_children<'expression>(
    stack: &mut Vec<(&'expression UnboundExpression, usize)>,
    expression: &'expression UnboundExpression,
    child_depth: usize,
) {
    if let Some((left, right)) = binary_children(expression) {
        stack.push((right, child_depth));
        stack.push((left, child_depth));
    }
}

fn binary_children(
    expression: &UnboundExpression,
) -> Option<(&UnboundExpression, &UnboundExpression)> {
    match expression {
        UnboundExpression::Add { left, right }
        | UnboundExpression::Subtract { left, right }
        | UnboundExpression::Multiply { left, right }
        | UnboundExpression::Divide { left, right }
        | UnboundExpression::Minimum { left, right }
        | UnboundExpression::Maximum { left, right } => Some((left, right)),
        UnboundExpression::Number(_) | UnboundExpression::Reference(_) => None,
    }
}

struct Parser<'input> {
    input: &'input str,
    bytes: &'input [u8],
    position: usize,
    nodes: usize,
    nesting: usize,
}

impl<'input> Parser<'input> {
    fn new(input: &'input str) -> Self {
        Self {
            input,
            bytes: input.as_bytes(),
            position: 0,
            nodes: 0,
            nesting: 0,
        }
    }

    fn parse(mut self) -> Result<UnboundExpression, FormulaParseError> {
        self.skip_whitespace();
        if self.is_finished() {
            return Err(self.error("expected expression"));
        }
        let expression = self.parse_additive()?;
        self.skip_whitespace();
        if !self.is_finished() {
            return Err(self.error("unexpected trailing content"));
        }
        Ok(expression)
    }

    fn parse_additive(&mut self) -> Result<UnboundExpression, FormulaParseError> {
        let mut expression = self.parse_multiplicative()?;
        loop {
            self.skip_whitespace();
            let Some(operator @ (b'+' | b'-')) = self.peek() else {
                return Ok(expression);
            };
            let operator_position = self.position;
            self.position += 1;
            self.record_node(operator_position)?;
            let right = self.parse_multiplicative()?;
            expression = if operator == b'+' {
                UnboundExpression::Add {
                    left: Box::new(expression),
                    right: Box::new(right),
                }
            } else {
                UnboundExpression::Subtract {
                    left: Box::new(expression),
                    right: Box::new(right),
                }
            };
        }
    }

    fn parse_multiplicative(&mut self) -> Result<UnboundExpression, FormulaParseError> {
        let mut expression = self.parse_unary()?;
        loop {
            self.skip_whitespace();
            let Some(operator @ (b'*' | b'/')) = self.peek() else {
                return Ok(expression);
            };
            let operator_position = self.position;
            self.position += 1;
            self.record_node(operator_position)?;
            let right = self.parse_unary()?;
            expression = if operator == b'*' {
                UnboundExpression::Multiply {
                    left: Box::new(expression),
                    right: Box::new(right),
                }
            } else {
                UnboundExpression::Divide {
                    left: Box::new(expression),
                    right: Box::new(right),
                }
            };
        }
    }

    fn parse_unary(&mut self) -> Result<UnboundExpression, FormulaParseError> {
        self.skip_whitespace();
        let Some(operator @ (b'+' | b'-')) = self.peek() else {
            return self.parse_primary();
        };
        let operator_position = self.position;
        self.position += 1;
        let operand = self.with_nesting(operator_position, Self::parse_unary)?;
        if operator == b'+' {
            return Ok(operand);
        }
        if let UnboundExpression::Number(number) = operand {
            return Ok(UnboundExpression::Number(
                Number::new(-number.get()).expect("negating a finite number stays finite"),
            ));
        }

        self.record_node(operator_position)?;
        self.record_node(operator_position)?;
        Ok(UnboundExpression::Subtract {
            left: Box::new(UnboundExpression::Number(Number::default())),
            right: Box::new(operand),
        })
    }

    fn parse_primary(&mut self) -> Result<UnboundExpression, FormulaParseError> {
        self.skip_whitespace();
        let Some(next) = self.peek() else {
            return Err(self.error("expected expression"));
        };
        match next {
            b'0'..=b'9' | b'.' => self.parse_number(),
            b'[' => self.parse_reference(),
            b'(' => self.parse_parenthesized(),
            byte if byte.is_ascii_alphabetic() => self.parse_function(),
            _ => Err(self.error("expected number, reference, function, or '('")),
        }
    }

    fn parse_number(&mut self) -> Result<UnboundExpression, FormulaParseError> {
        let start = self.position;
        let mut has_digits = self.consume_digits();
        if self.peek() == Some(b'.') {
            self.position += 1;
            has_digits |= self.consume_digits();
        }
        if !has_digits {
            return Err(FormulaParseError::new(
                start,
                "expected digits in numeric literal",
            ));
        }

        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.position += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.position += 1;
            }
            let exponent_start = self.position;
            if !self.consume_digits() {
                return Err(FormulaParseError::new(
                    exponent_start,
                    "expected exponent digits",
                ));
            }
        }

        let literal = &self.input[start..self.position];
        let number = literal
            .parse::<f64>()
            .map_err(|_| FormulaParseError::new(start, "invalid numeric literal"))?;
        let number = Number::new(number).map_err(|_| {
            FormulaParseError::new(start, "numeric literal must convert to a finite Number")
        })?;
        self.record_node(start)?;
        Ok(UnboundExpression::Number(number))
    }

    fn parse_reference(&mut self) -> Result<UnboundExpression, FormulaParseError> {
        let start = self.position;
        self.position += 1;
        let inner_start = self.position;
        let Some(relative_close) = self.bytes[inner_start..]
            .iter()
            .position(|byte| *byte == b']')
        else {
            self.position = self.bytes.len();
            return Err(self.error("expected ']' after reference"));
        };
        let close = inner_start + relative_close;
        let inner = &self.input[inner_start..close];

        let Some(dot) = inner.find('.') else {
            self.position = close;
            return Err(self.error("reference must contain exactly one '.'"));
        };
        if let Some(second_dot) = inner[dot + 1..].find('.') {
            self.position = inner_start + dot + 1 + second_dot;
            return Err(self.error("reference must contain exactly one '.'"));
        }

        let entity = &inner[..dot];
        let field = &inner[dot + 1..];
        if !is_valid_identifier(entity) {
            self.position = invalid_identifier_position(entity, inner_start);
            return Err(self.error("invalid reference entity key"));
        }
        if !is_valid_identifier(field) {
            self.position = invalid_identifier_position(field, inner_start + dot + 1);
            return Err(self.error("invalid reference field key"));
        }

        self.position = close + 1;
        self.record_node(start)?;
        Ok(UnboundExpression::Reference(FieldAddress::new(
            entity, field,
        )))
    }

    fn parse_parenthesized(&mut self) -> Result<UnboundExpression, FormulaParseError> {
        let open = self.position;
        self.position += 1;
        self.with_nesting(open, |parser| {
            let expression = parser.parse_additive()?;
            parser.expect(b')', "expected ')' after expression")?;
            Ok(expression)
        })
    }

    fn parse_function(&mut self) -> Result<UnboundExpression, FormulaParseError> {
        let start = self.position;
        self.position += 1;
        while self
            .peek()
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            self.position += 1;
        }
        let name = &self.input[start..self.position];
        let is_minimum = match name {
            "min" => true,
            "max" => false,
            _ => {
                return Err(FormulaParseError::new(
                    start,
                    format!("unknown function '{name}'; expected min or max"),
                ));
            }
        };

        self.skip_whitespace();
        let open = self.position;
        self.expect(b'(', "expected '(' after function name")?;
        self.record_node(start)?;
        self.with_nesting(open, |parser| {
            let left = parser.parse_additive()?;
            parser.expect(b',', "expected ',' between function arguments")?;
            let right = parser.parse_additive()?;
            parser.expect(b')', "expected ')' after function arguments")?;
            if is_minimum {
                Ok(UnboundExpression::Minimum {
                    left: Box::new(left),
                    right: Box::new(right),
                })
            } else {
                Ok(UnboundExpression::Maximum {
                    left: Box::new(left),
                    right: Box::new(right),
                })
            }
        })
    }

    fn consume_digits(&mut self) -> bool {
        let start = self.position;
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.position += 1;
        }
        self.position > start
    }

    fn expect(&mut self, expected: u8, message: &str) -> Result<(), FormulaParseError> {
        self.skip_whitespace();
        if self.peek() == Some(expected) {
            self.position += 1;
            Ok(())
        } else {
            Err(self.error(message))
        }
    }

    fn with_nesting<T>(
        &mut self,
        position: usize,
        parse: impl FnOnce(&mut Self) -> Result<T, FormulaParseError>,
    ) -> Result<T, FormulaParseError> {
        if self.nesting >= MAX_NESTING {
            return Err(FormulaParseError::new(
                position,
                "expression exceeds 64-nesting limit",
            ));
        }
        self.nesting += 1;
        let result = parse(self);
        self.nesting -= 1;
        result
    }

    fn record_node(&mut self, position: usize) -> Result<(), FormulaParseError> {
        if self.nodes >= MAX_AST_NODES {
            return Err(FormulaParseError::new(
                position,
                "expression exceeds 256-node limit",
            ));
        }
        self.nodes += 1;
        Ok(())
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.position += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    fn is_finished(&self) -> bool {
        self.position == self.bytes.len()
    }

    fn error(&self, message: impl Into<String>) -> FormulaParseError {
        FormulaParseError::new(self.position, message)
    }
}

fn invalid_identifier_position(identifier: &str, start: usize) -> usize {
    let mut characters = identifier.char_indices();
    let Some((_, first)) = characters.next() else {
        return start;
    };
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return start;
    }
    characters
        .find(|(_, character)| {
            !character.is_ascii_lowercase()
                && !character.is_ascii_digit()
                && !matches!(character, '_' | '-')
        })
        .map_or(start, |(offset, _)| start + offset)
}
