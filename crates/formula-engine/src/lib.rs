//! Deterministic formula parsing, binding, projection, and evaluation.

mod parser;

use std::collections::{BTreeMap, BTreeSet};

use tachiko_semantic_core::{
    AddressIndex, AddressIndexError, Document, Expression, FieldAddress, FieldRef, FieldType,
    MAX_EXPRESSION_DEPTH, MAX_EXPRESSION_NODES, Number, Value,
};
use thiserror::Error;

pub use parser::{
    ExpressionComplexityError, FormulaParseError, UnboundExpression, format_unbound_expression,
    parse_expression, validate_unbound_expression_structure,
};

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FormulaBindError {
    #[error(transparent)]
    Complexity(#[from] ExpressionComplexityError),
    #[error("formula address index is invalid: {source}")]
    Index {
        #[source]
        source: Box<AddressIndexError>,
    },
    #[error("formula address '{address}' cannot be resolved: {source}")]
    Address {
        address: FieldAddress,
        #[source]
        source: Box<AddressIndexError>,
    },
    #[error("formula address '{address}' resolves to non-numeric field '{reference}'")]
    NonNumericTarget {
        address: FieldAddress,
        reference: FieldRef,
    },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CanonicalAuthoringProjectionError {
    #[error("bound formula references cannot be projected through current human addresses")]
    UnresolvableBoundReferences { targets: BTreeSet<FieldRef> },
    #[error(transparent)]
    Complexity(#[from] ExpressionComplexityError),
}

/// Bind every parsed human address to stable IDs in one document snapshot.
///
/// # Errors
///
/// Returns a typed address or numeric-target error without producing a partial
/// bound expression.
pub fn bind_expression(
    document: &Document,
    expression: &UnboundExpression,
) -> Result<Expression, FormulaBindError> {
    validate_unbound_expression_structure(expression)?;
    let index = AddressIndex::build(document).map_err(|source| FormulaBindError::Index {
        source: Box::new(source),
    })?;
    bind_node(document, &index, expression)
}

fn bind_node(
    document: &Document,
    index: &AddressIndex,
    expression: &UnboundExpression,
) -> Result<Expression, FormulaBindError> {
    Ok(match expression {
        UnboundExpression::Number(number) => Expression::Number(*number),
        UnboundExpression::Reference(address) => {
            let reference = index.resolve_field(document, address).map_err(|source| {
                FormulaBindError::Address {
                    address: address.clone(),
                    source: Box::new(source),
                }
            })?;
            let entity = document.entities.get(&reference.entity).ok_or_else(|| {
                FormulaBindError::Index {
                    source: Box::new(AddressIndexError::MissingBoundEntity {
                        entity: reference.entity.clone(),
                    }),
                }
            })?;
            let schema =
                document
                    .schemas
                    .get(&entity.schema)
                    .ok_or_else(|| FormulaBindError::Index {
                        source: Box::new(AddressIndexError::MissingBoundSchema {
                            schema: entity.schema.clone(),
                        }),
                    })?;
            let definition =
                schema
                    .fields
                    .get(&reference.field)
                    .ok_or_else(|| FormulaBindError::Index {
                        source: Box::new(AddressIndexError::MissingBoundField {
                            entity: reference.entity.clone(),
                            field: reference.field.clone(),
                        }),
                    })?;
            if definition.field_type != FieldType::Number {
                return Err(FormulaBindError::NonNumericTarget {
                    address: address.clone(),
                    reference,
                });
            }
            Expression::Reference(reference)
        }
        UnboundExpression::Add { left, right } => Expression::Add {
            left: Box::new(bind_node(document, index, left)?),
            right: Box::new(bind_node(document, index, right)?),
        },
        UnboundExpression::Subtract { left, right } => Expression::Subtract {
            left: Box::new(bind_node(document, index, left)?),
            right: Box::new(bind_node(document, index, right)?),
        },
        UnboundExpression::Multiply { left, right } => Expression::Multiply {
            left: Box::new(bind_node(document, index, left)?),
            right: Box::new(bind_node(document, index, right)?),
        },
        UnboundExpression::Divide { left, right } => Expression::Divide {
            left: Box::new(bind_node(document, index, left)?),
            right: Box::new(bind_node(document, index, right)?),
        },
        UnboundExpression::Minimum { left, right } => Expression::Minimum {
            left: Box::new(bind_node(document, index, left)?),
            right: Box::new(bind_node(document, index, right)?),
        },
        UnboundExpression::Maximum { left, right } => Expression::Maximum {
            left: Box::new(bind_node(document, index, left)?),
            right: Box::new(bind_node(document, index, right)?),
        },
    })
}

/// Project a bound formula through the document's current human keys.
///
/// Every reference is round-trip checked against the same snapshot. Failure
/// returns all unresolved stable targets and no source text.
///
/// # Errors
///
/// Returns a typed stable-target set or structural/canonical-length failure.
pub fn project_expression(
    document: &Document,
    expression: &Expression,
) -> Result<String, CanonicalAuthoringProjectionError> {
    validate_expression_structure(expression)?;
    let dependencies = extract_dependencies(expression);
    let Ok(index) = AddressIndex::build(document) else {
        return Err(
            CanonicalAuthoringProjectionError::UnresolvableBoundReferences {
                targets: dependencies,
            },
        );
    };
    let mut addresses = BTreeMap::new();
    let mut unresolved = BTreeSet::new();
    for target in dependencies {
        match index.field_address(document, &target) {
            Ok(address) => {
                addresses.insert(target, address);
            }
            Err(_) => {
                unresolved.insert(target);
            }
        }
    }
    if !unresolved.is_empty() {
        return Err(
            CanonicalAuthoringProjectionError::UnresolvableBoundReferences {
                targets: unresolved,
            },
        );
    }

    let projected = render_bound(expression, &addresses);
    if projected.len() > parser::MAX_INPUT_BYTES {
        return Err(ExpressionComplexityError::CanonicalLengthLimit.into());
    }
    Ok(projected)
}

fn render_bound(expression: &Expression, addresses: &BTreeMap<FieldRef, FieldAddress>) -> String {
    match expression {
        Expression::Number(number) => parser::format_number(*number),
        Expression::Reference(reference) => {
            let address = &addresses[reference];
            format!("[{}.{}]", address.entity, address.field)
        }
        Expression::Add { left, right } => render_bound_binary(left, "+", right, addresses),
        Expression::Subtract { left, right } => render_bound_binary(left, "-", right, addresses),
        Expression::Multiply { left, right } => render_bound_binary(left, "*", right, addresses),
        Expression::Divide { left, right } => render_bound_binary(left, "/", right, addresses),
        Expression::Minimum { left, right } => format!(
            "min({}, {})",
            render_bound(left, addresses),
            render_bound(right, addresses)
        ),
        Expression::Maximum { left, right } => format!(
            "max({}, {})",
            render_bound(left, addresses),
            render_bound(right, addresses)
        ),
    }
}

fn render_bound_binary(
    left: &Expression,
    operator: &str,
    right: &Expression,
    addresses: &BTreeMap<FieldRef, FieldAddress>,
) -> String {
    format!(
        "({} {operator} {})",
        render_bound(left, addresses),
        render_bound(right, addresses)
    )
}

/// Validate node and depth limits for a directly supplied bound expression.
/// Canonical byte length is checked by [`project_expression`] because it
/// depends on the document's current human keys.
///
/// # Errors
///
/// Returns a typed node/depth limit error.
pub fn validate_expression_structure(
    expression: &Expression,
) -> Result<(), ExpressionComplexityError> {
    let mut nodes = 0_usize;
    let mut stack = vec![(expression, 1_usize)];
    while let Some((node, depth)) = stack.pop() {
        if depth > MAX_EXPRESSION_DEPTH {
            return Err(ExpressionComplexityError::DepthLimit);
        }
        nodes += 1;
        if nodes > MAX_EXPRESSION_NODES {
            return Err(ExpressionComplexityError::NodeLimit);
        }
        match node {
            Expression::Add { left, right }
            | Expression::Subtract { left, right }
            | Expression::Multiply { left, right }
            | Expression::Divide { left, right }
            | Expression::Minimum { left, right }
            | Expression::Maximum { left, right } => {
                stack.push((right, depth + 1));
                stack.push((left, depth + 1));
            }
            Expression::Number(_) | Expression::Reference(_) => {}
        }
    }
    Ok(())
}

/// Extract the deterministic static dependency set from a bound expression.
#[must_use]
pub fn extract_dependencies(expression: &Expression) -> BTreeSet<FieldRef> {
    let mut dependencies = BTreeSet::new();
    let mut stack = vec![expression];
    while let Some(node) = stack.pop() {
        match node {
            Expression::Reference(reference) => {
                dependencies.insert(reference.clone());
            }
            Expression::Add { left, right }
            | Expression::Subtract { left, right }
            | Expression::Multiply { left, right }
            | Expression::Divide { left, right }
            | Expression::Minimum { left, right }
            | Expression::Maximum { left, right } => {
                stack.push(right);
                stack.push(left);
            }
            Expression::Number(_) => {}
        }
    }
    dependencies
}

#[derive(Clone, Debug, PartialEq)]
pub struct Calculation {
    values: BTreeMap<FieldRef, Number>,
    dependencies: BTreeMap<FieldRef, BTreeSet<FieldRef>>,
}

impl Calculation {
    #[must_use]
    pub fn value(&self, field: &FieldRef) -> Option<Number> {
        self.values.get(field).copied()
    }

    #[must_use]
    pub fn values(&self) -> &BTreeMap<FieldRef, Number> {
        &self.values
    }

    #[must_use]
    pub fn dependencies(&self) -> &BTreeMap<FieldRef, BTreeSet<FieldRef>> {
        &self.dependencies
    }

    #[must_use]
    pub fn dependencies_of(&self, field: &FieldRef) -> Option<&BTreeSet<FieldRef>> {
        self.dependencies.get(field)
    }

    #[must_use]
    pub fn affected_by(&self, changed: &FieldRef) -> Vec<FieldRef> {
        let mut frontier = BTreeSet::from([changed.clone()]);
        let mut affected = BTreeSet::new();

        loop {
            let newly_affected: BTreeSet<_> = self
                .dependencies
                .iter()
                .filter(|(formula, dependencies)| {
                    !affected.contains(*formula) && !dependencies.is_disjoint(&frontier)
                })
                .map(|(formula, _)| formula.clone())
                .collect();
            if newly_affected.is_empty() {
                break;
            }
            frontier.clone_from(&newly_affected);
            affected.extend(newly_affected);
        }

        affected.into_iter().collect()
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum CalculationError {
    #[error("formula reference '{reference}' does not exist")]
    MissingReference { reference: FieldRef },
    #[error("formula reference '{reference}' is not numeric")]
    NonNumericReference { reference: FieldRef },
    #[error("formula '{formula}' divided by zero")]
    DivisionByZero { formula: FieldRef },
    #[error("formula dependency cycle: {path:?}")]
    Cycle { path: Vec<FieldRef> },
    #[error("calculation for '{field}' produced a non-finite result")]
    NonFiniteResult { field: FieldRef },
    #[error("formula '{formula}' violates the bound expression contract: {message}")]
    InvalidExpression { formula: FieldRef, message: String },
}

/// Calculate every numeric literal and formula in deterministic stable-ID order.
///
/// # Errors
///
/// Returns a [`CalculationError`] for invalid bound expressions, missing or
/// non-numeric references, cycles, division by zero, or non-finite results.
pub fn calculate(document: &Document) -> Result<Calculation, CalculationError> {
    let mut dependencies = BTreeMap::new();
    for (entity_id, entity) in &document.entities {
        for (field_id, value) in &entity.fields {
            if let Value::Formula(expression) = value {
                let formula = FieldRef::new(entity_id.clone(), field_id.clone());
                validate_expression_structure(expression).map_err(|error| {
                    CalculationError::InvalidExpression {
                        formula: formula.clone(),
                        message: error.to_string(),
                    }
                })?;
                dependencies.insert(formula, extract_dependencies(expression));
            }
        }
    }

    let mut evaluator = Evaluator::new(document, dependencies);
    for (entity_id, entity) in &document.entities {
        for (field_id, value) in &entity.fields {
            if matches!(value, Value::Number(_) | Value::Formula(_)) {
                evaluator.value_for(&FieldRef::new(entity_id.clone(), field_id.clone()))?;
            }
        }
    }

    Ok(Calculation {
        values: evaluator.values,
        dependencies: evaluator.dependencies,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VisitState {
    Visiting,
    Complete,
}

#[derive(Clone, Copy, Debug)]
enum BinaryOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Minimum,
    Maximum,
}

enum EvaluationFrame<'expression> {
    Field(FieldRef),
    CompleteField(FieldRef),
    Expression {
        formula: FieldRef,
        expression: &'expression Expression,
    },
    Apply {
        formula: FieldRef,
        operation: BinaryOperation,
    },
}

struct Evaluator<'document> {
    document: &'document Document,
    values: BTreeMap<FieldRef, Number>,
    dependencies: BTreeMap<FieldRef, BTreeSet<FieldRef>>,
    states: BTreeMap<FieldRef, VisitState>,
    stack: Vec<FieldRef>,
}

impl<'document> Evaluator<'document> {
    fn new(
        document: &'document Document,
        dependencies: BTreeMap<FieldRef, BTreeSet<FieldRef>>,
    ) -> Self {
        Self {
            document,
            values: BTreeMap::new(),
            dependencies,
            states: BTreeMap::new(),
            stack: Vec::new(),
        }
    }

    fn value_for(&mut self, field: &FieldRef) -> Result<Number, CalculationError> {
        let mut frames = vec![EvaluationFrame::Field(field.clone())];
        let mut results = Vec::new();

        while let Some(frame) = frames.pop() {
            self.evaluate_frame(frame, &mut frames, &mut results)?;
        }

        if results.len() != 1 {
            return Err(Self::invalid_evaluation_state(
                field,
                "evaluation did not produce exactly one result",
            ));
        }
        Self::pop_result(&mut results, field)
    }

    fn evaluate_frame(
        &mut self,
        frame: EvaluationFrame<'document>,
        frames: &mut Vec<EvaluationFrame<'document>>,
        results: &mut Vec<Number>,
    ) -> Result<(), CalculationError> {
        match frame {
            EvaluationFrame::Field(field) => self.evaluate_field(field, frames, results),
            EvaluationFrame::CompleteField(field) => self.complete_field(field, results),
            EvaluationFrame::Expression {
                formula,
                expression,
            } => {
                Self::evaluate_expression(formula, expression, frames, results);
                Ok(())
            }
            EvaluationFrame::Apply { formula, operation } => {
                let result = Self::apply_operation(&formula, operation, results)?;
                results.push(result);
                Ok(())
            }
        }
    }

    fn evaluate_field(
        &mut self,
        field: FieldRef,
        frames: &mut Vec<EvaluationFrame<'document>>,
        results: &mut Vec<Number>,
    ) -> Result<(), CalculationError> {
        match self.states.get(&field) {
            Some(VisitState::Complete) => {
                let value = self.values.get(&field).copied().ok_or_else(|| {
                    Self::invalid_evaluation_state(&field, "completed field has no cached value")
                })?;
                results.push(value);
            }
            Some(VisitState::Visiting) => {
                let cycle_start = self
                    .stack
                    .iter()
                    .position(|candidate| candidate == &field)
                    .unwrap_or(0);
                let mut path = self.stack[cycle_start..].to_vec();
                path.push(field);
                return Err(CalculationError::Cycle { path });
            }
            None => match self.lookup_value(&field)? {
                Value::Number(number) => {
                    self.states.insert(field.clone(), VisitState::Complete);
                    self.values.insert(field, *number);
                    results.push(*number);
                }
                Value::Formula(expression) => {
                    self.states.insert(field.clone(), VisitState::Visiting);
                    self.stack.push(field.clone());
                    frames.push(EvaluationFrame::CompleteField(field.clone()));
                    frames.push(EvaluationFrame::Expression {
                        formula: field,
                        expression,
                    });
                }
                Value::Text(_) | Value::Boolean(_) | Value::Reference(_) => {
                    return Err(CalculationError::NonNumericReference { reference: field });
                }
            },
        }
        Ok(())
    }

    fn complete_field(
        &mut self,
        field: FieldRef,
        results: &mut Vec<Number>,
    ) -> Result<(), CalculationError> {
        let result = Self::pop_result(results, &field)?;
        if self.stack.pop().as_ref() != Some(&field) {
            return Err(Self::invalid_evaluation_state(
                &field,
                "active formula stack is inconsistent",
            ));
        }
        self.states.insert(field.clone(), VisitState::Complete);
        self.values.insert(field, result);
        results.push(result);
        Ok(())
    }

    fn evaluate_expression(
        formula: FieldRef,
        expression: &'document Expression,
        frames: &mut Vec<EvaluationFrame<'document>>,
        results: &mut Vec<Number>,
    ) {
        match expression {
            Expression::Number(number) => results.push(*number),
            Expression::Reference(reference) => {
                frames.push(EvaluationFrame::Field(reference.clone()));
            }
            Expression::Add { left, right } => {
                Self::push_binary(frames, formula, BinaryOperation::Add, left, right);
            }
            Expression::Subtract { left, right } => {
                Self::push_binary(frames, formula, BinaryOperation::Subtract, left, right);
            }
            Expression::Multiply { left, right } => {
                Self::push_binary(frames, formula, BinaryOperation::Multiply, left, right);
            }
            Expression::Divide { left, right } => {
                Self::push_binary(frames, formula, BinaryOperation::Divide, left, right);
            }
            Expression::Minimum { left, right } => {
                Self::push_binary(frames, formula, BinaryOperation::Minimum, left, right);
            }
            Expression::Maximum { left, right } => {
                Self::push_binary(frames, formula, BinaryOperation::Maximum, left, right);
            }
        }
    }

    fn apply_operation(
        formula: &FieldRef,
        operation: BinaryOperation,
        results: &mut Vec<Number>,
    ) -> Result<Number, CalculationError> {
        let right = Self::pop_result(results, formula)?;
        let left = Self::pop_result(results, formula)?;
        match operation {
            BinaryOperation::Add => Self::number_result(formula, left.get() + right.get()),
            BinaryOperation::Subtract => Self::number_result(formula, left.get() - right.get()),
            BinaryOperation::Multiply => Self::number_result(formula, left.get() * right.get()),
            BinaryOperation::Divide => {
                if right.get() == 0.0 {
                    return Err(CalculationError::DivisionByZero {
                        formula: formula.clone(),
                    });
                }
                Self::number_result(formula, left.get() / right.get())
            }
            BinaryOperation::Minimum => Ok(if left <= right { left } else { right }),
            BinaryOperation::Maximum => Ok(if left >= right { left } else { right }),
        }
    }

    fn lookup_value(&self, field: &FieldRef) -> Result<&'document Value, CalculationError> {
        let entity = self.document.entities.get(&field.entity).ok_or_else(|| {
            CalculationError::MissingReference {
                reference: field.clone(),
            }
        })?;
        let definition = self
            .document
            .schemas
            .get(&entity.schema)
            .and_then(|schema| schema.fields.get(&field.field))
            .ok_or_else(|| CalculationError::MissingReference {
                reference: field.clone(),
            })?;
        if definition.field_type != FieldType::Number {
            return Err(CalculationError::NonNumericReference {
                reference: field.clone(),
            });
        }
        entity
            .fields
            .get(&field.field)
            .ok_or_else(|| CalculationError::MissingReference {
                reference: field.clone(),
            })
    }

    fn push_binary(
        frames: &mut Vec<EvaluationFrame<'document>>,
        formula: FieldRef,
        operation: BinaryOperation,
        left: &'document Expression,
        right: &'document Expression,
    ) {
        frames.push(EvaluationFrame::Apply {
            formula: formula.clone(),
            operation,
        });
        frames.push(EvaluationFrame::Expression {
            formula: formula.clone(),
            expression: right,
        });
        frames.push(EvaluationFrame::Expression {
            formula,
            expression: left,
        });
    }

    fn pop_result(
        results: &mut Vec<Number>,
        formula: &FieldRef,
    ) -> Result<Number, CalculationError> {
        results.pop().ok_or_else(|| {
            Self::invalid_evaluation_state(formula, "evaluation result stack is empty")
        })
    }

    fn invalid_evaluation_state(field: &FieldRef, message: &str) -> CalculationError {
        CalculationError::InvalidExpression {
            formula: field.clone(),
            message: message.to_owned(),
        }
    }

    fn number_result(field: &FieldRef, value: f64) -> Result<Number, CalculationError> {
        Number::new(value).map_err(|_| CalculationError::NonFiniteResult {
            field: field.clone(),
        })
    }
}
