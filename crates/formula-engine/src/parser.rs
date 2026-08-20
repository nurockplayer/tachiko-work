use tachiko_semantic_core::{Expression, FieldRef, is_valid_identifier};
use thiserror::Error;

const MAX_INPUT_BYTES: usize = 4_096;
const MAX_AST_NODES: usize = 256;
const MAX_NESTING: usize = 64;

/// A deterministic formula-language failure at a UTF-8 byte position.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("formula parse error at byte {position}: {message}")]
pub struct FormulaParseError {
    /// Zero-based byte position at which parsing became impossible.
    pub position: usize,
    /// Stable, user-actionable description of the expected input.
    pub message: String,
}

impl FormulaParseError {
    fn new(position: usize, message: impl Into<String>) -> Self {
        Self {
            position,
            message: message.into(),
        }
    }
}

/// Parse the bounded Tachiko formula language into its semantic expression AST.
///
/// # Errors
///
/// Returns [`FormulaParseError`] for invalid syntax, identifiers, non-finite
/// numeric literals, trailing content, or a breached input, node, or nesting
/// limit.
pub fn parse_expression(input: &str) -> Result<Expression, FormulaParseError> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(FormulaParseError::new(
            MAX_INPUT_BYTES,
            "expression exceeds 4096-byte limit",
        ));
    }
    Parser::new(input).parse()
}

/// Render an expression in deterministic, copy/paste-safe canonical syntax.
#[must_use]
pub fn format_expression(expression: &Expression) -> String {
    match expression {
        Expression::Number(number) => format_number(*number),
        Expression::Reference(reference) => {
            format!("[{}.{}]", reference.entity, reference.field)
        }
        Expression::Add { left, right } => format_binary(left, "+", right),
        Expression::Subtract { left, right } => format_binary(left, "-", right),
        Expression::Multiply { left, right } => format_binary(left, "*", right),
        Expression::Divide { left, right } => format_binary(left, "/", right),
        Expression::Minimum { left, right } => format!(
            "min({}, {})",
            format_expression(left),
            format_expression(right)
        ),
        Expression::Maximum { left, right } => format!(
            "max({}, {})",
            format_expression(left),
            format_expression(right)
        ),
    }
}

fn format_binary(left: &Expression, operator: &str, right: &Expression) -> String {
    format!(
        "({} {operator} {})",
        format_expression(left),
        format_expression(right)
    )
}

fn format_number(number: f64) -> String {
    if number.fract() == 0.0 {
        format!("{number:.0}")
    } else {
        number.to_string()
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

    fn parse(mut self) -> Result<Expression, FormulaParseError> {
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

    fn parse_additive(&mut self) -> Result<Expression, FormulaParseError> {
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
                Expression::Add {
                    left: Box::new(expression),
                    right: Box::new(right),
                }
            } else {
                Expression::Subtract {
                    left: Box::new(expression),
                    right: Box::new(right),
                }
            };
        }
    }

    fn parse_multiplicative(&mut self) -> Result<Expression, FormulaParseError> {
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
                Expression::Multiply {
                    left: Box::new(expression),
                    right: Box::new(right),
                }
            } else {
                Expression::Divide {
                    left: Box::new(expression),
                    right: Box::new(right),
                }
            };
        }
    }

    fn parse_unary(&mut self) -> Result<Expression, FormulaParseError> {
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
        if let Expression::Number(number) = operand {
            return Ok(Expression::Number(-number));
        }

        self.record_node(operator_position)?;
        self.record_node(operator_position)?;
        Ok(Expression::Subtract {
            left: Box::new(Expression::Number(0.0)),
            right: Box::new(operand),
        })
    }

    fn parse_primary(&mut self) -> Result<Expression, FormulaParseError> {
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

    fn parse_number(&mut self) -> Result<Expression, FormulaParseError> {
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
        if !number.is_finite() {
            return Err(FormulaParseError::new(
                start,
                "numeric literal must be finite",
            ));
        }
        self.record_node(start)?;
        Ok(Expression::Number(number))
    }

    fn parse_reference(&mut self) -> Result<Expression, FormulaParseError> {
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
            return Err(self.error("invalid reference entity identifier"));
        }
        if !is_valid_identifier(field) {
            self.position = invalid_identifier_position(field, inner_start + dot + 1);
            return Err(self.error("invalid reference field identifier"));
        }

        self.position = close + 1;
        self.record_node(start)?;
        Ok(Expression::Reference(FieldRef::new(entity, field)))
    }

    fn parse_parenthesized(&mut self) -> Result<Expression, FormulaParseError> {
        let open = self.position;
        self.position += 1;
        self.with_nesting(open, |parser| {
            let expression = parser.parse_additive()?;
            parser.expect(b')', "expected ')' after expression")?;
            Ok(expression)
        })
    }

    fn parse_function(&mut self) -> Result<Expression, FormulaParseError> {
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
                Ok(Expression::Minimum {
                    left: Box::new(left),
                    right: Box::new(right),
                })
            } else {
                Ok(Expression::Maximum {
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
