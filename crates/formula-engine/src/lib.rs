//! Deterministic formula parsing, binding, projection, and evaluation.

#[cfg(test)]
mod column_graph_differential;
mod evaluation;
mod parser;

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

use tachiko_semantic_core::{
    AddressIndex, AddressIndexError, ColumnAddress, ColumnCriterion, ColumnRef, ComparisonOperator,
    ConditionalAggregate, Document, Expression, FieldAddress, FieldRef, FieldType,
    FormulaExpressionType, MAX_EXPRESSION_DEPTH, MAX_EXPRESSION_NODES, Number, Value,
};
use thiserror::Error;

pub use parser::{
    ExpressionComplexityError, FormulaParseError, UnboundColumnCriterion,
    UnboundConditionalAggregate, UnboundExpression, format_unbound_expression, parse_expression,
    validate_unbound_expression_structure,
};

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FormulaBindError {
    #[error("formula type mismatch: {message}")]
    TypeMismatch { message: String },
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
    #[error("formula column address '{address}' cannot be resolved: {source}")]
    Column {
        address: ColumnAddress,
        #[source]
        source: Box<AddressIndexError>,
    },
    #[error("formula column address '{address}' does not resolve to a numeric field")]
    NonNumericColumn { address: ColumnAddress },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CanonicalAuthoringProjectionError {
    #[error("bound formula references cannot be projected through current human addresses")]
    UnresolvableBoundReferences { targets: BTreeSet<FieldRef> },
    #[error("bound formula columns cannot be projected through current human addresses")]
    UnresolvableBoundColumns { columns: BTreeSet<ColumnRef> },
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
    owner: &FieldRef,
    expression: &UnboundExpression,
) -> Result<Expression, FormulaBindError> {
    let index = AddressIndex::build(document).map_err(|source| FormulaBindError::Index {
        source: Box::new(source),
    })?;
    bind_expression_with_index(document, &index, owner, expression)
}

/// Bind a formula through an [`AddressIndex`] the caller built once.
///
/// Returns the same value as [`bind_expression`]; callers that bind every
/// formula of one document reuse a single index instead of rebuilding it.
///
/// # Errors
///
/// Returns an error when a reference does not resolve to a valid target or
/// the bound expression is not a Number.
pub fn bind_expression_with_index(
    document: &Document,
    index: &AddressIndex,
    owner: &FieldRef,
    expression: &UnboundExpression,
) -> Result<Expression, FormulaBindError> {
    validate_unbound_expression_structure(expression)?;
    let bound = bind_node(document, index, owner, expression)?;
    if bound.expression_type() != Some(FormulaExpressionType::Number) {
        return Err(FormulaBindError::TypeMismatch {
            message:
                "conditions are only valid in IF, AND, OR, or NOT; ROUND requires a numeric value"
                    .into(),
        });
    }
    Ok(bound)
}

/// Resolve one human address to a numeric stable target.
fn resolve_reference_target(
    document: &Document,
    index: &AddressIndex,
    address: &FieldAddress,
) -> Result<(FieldRef, FieldType), FormulaBindError> {
    let reference =
        index
            .resolve_field(document, address)
            .map_err(|source| FormulaBindError::Address {
                address: address.clone(),
                source: Box::new(source),
            })?;
    let entity =
        document
            .entities
            .get(&reference.entity)
            .ok_or_else(|| FormulaBindError::Index {
                source: Box::new(AddressIndexError::MissingBoundEntity {
                    entity: reference.entity.clone(),
                }),
            })?;
    let schema = document
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
    Ok((reference, definition.field_type.clone()))
}

fn bind_numeric_reference(
    document: &Document,
    index: &AddressIndex,
    address: &FieldAddress,
) -> Result<FieldRef, FormulaBindError> {
    let (reference, field_type) = resolve_reference_target(document, index, address)?;
    if field_type != FieldType::Number {
        return Err(FormulaBindError::NonNumericTarget {
            address: address.clone(),
            reference,
        });
    }
    Ok(reference)
}

/// Resolve one criterion value reference without requiring a Number column: a
/// criterion cell may hold text, a boolean or a date. Reference-typed columns
/// stay unreadable.
fn bind_scalar_reference(
    document: &Document,
    index: &AddressIndex,
    address: &FieldAddress,
) -> Result<FieldRef, FormulaBindError> {
    let (reference, field_type) = resolve_reference_target(document, index, address)?;
    if matches!(field_type, FieldType::Reference { .. }) {
        return Err(FormulaBindError::TypeMismatch {
            message: format!("criterion reference '{address}' targets a reference field"),
        });
    }
    Ok(reference)
}

/// Resolve one human column address to a stable schema/field identity and
/// require that the column is numeric.
fn bind_column_reference(
    document: &Document,
    index: &AddressIndex,
    address: &ColumnAddress,
) -> Result<ColumnRef, FormulaBindError> {
    let unresolved = |source: AddressIndexError| FormulaBindError::Column {
        address: address.clone(),
        source: Box::new(source),
    };
    let schema = index
        .schema_id(&address.schema)
        .map_err(unresolved)?
        .clone();
    let field = index
        .schema_field_id(&schema, &address.field)
        .map_err(unresolved)?
        .clone();
    let definition = document
        .schemas
        .get(&schema)
        .and_then(|schema| schema.fields.get(&field))
        .ok_or_else(|| {
            unresolved(AddressIndexError::MissingSchemaFieldKey {
                schema: schema.clone(),
                field: address.field.clone(),
            })
        })?;
    if definition.field_type != FieldType::Number {
        return Err(FormulaBindError::NonNumericColumn {
            address: address.clone(),
        });
    }
    Ok(ColumnRef::new(schema, field))
}

/// Resolve one criterion column without requiring a Number type; reference
/// fields cannot hold a criterion cell.
fn bind_criterion_column(
    document: &Document,
    index: &AddressIndex,
    address: &ColumnAddress,
) -> Result<ColumnRef, FormulaBindError> {
    let unresolved = |source: AddressIndexError| FormulaBindError::Column {
        address: address.clone(),
        source: Box::new(source),
    };
    let schema = index
        .schema_id(&address.schema)
        .map_err(unresolved)?
        .clone();
    let field = index
        .schema_field_id(&schema, &address.field)
        .map_err(unresolved)?
        .clone();
    let definition = document
        .schemas
        .get(&schema)
        .and_then(|schema| schema.fields.get(&field))
        .ok_or_else(|| {
            unresolved(AddressIndexError::MissingSchemaFieldKey {
                schema: schema.clone(),
                field: address.field.clone(),
            })
        })?;
    if matches!(definition.field_type, FieldType::Reference { .. }) {
        return Err(FormulaBindError::TypeMismatch {
            message: format!("criterion column '{address}' targets a reference field"),
        });
    }
    Ok(ColumnRef::new(schema, field))
}

fn bind_conditional(
    document: &Document,
    index: &AddressIndex,
    owner: &FieldRef,
    aggregate: &UnboundConditionalAggregate,
    criteria: &[UnboundColumnCriterion],
) -> Result<Expression, FormulaBindError> {
    let mut schema: Option<tachiko_semantic_core::SchemaId> = None;
    let mut bound_criteria = Vec::with_capacity(criteria.len());
    for criterion in criteria {
        let column = bind_criterion_column(document, index, &criterion.column)?;
        match &schema {
            None => schema = Some(column.schema.clone()),
            Some(schema) if schema != &column.schema => {
                return Err(FormulaBindError::TypeMismatch {
                    message: "conditional aggregate columns must all belong to one schema".into(),
                });
            }
            Some(_) => {}
        }
        let value = criterion
            .value
            .as_deref()
            .map(|expression| {
                bind_criterion_value(document, index, owner, expression).map(Box::new)
            })
            .transpose()?;
        bound_criteria.push(ColumnCriterion {
            column,
            operator: criterion.operator,
            value,
        });
    }
    let aggregate = match aggregate {
        UnboundConditionalAggregate::Sum(address) => {
            let column = bind_column_reference(document, index, address)?;
            schema.get_or_insert_with(|| column.schema.clone());
            if schema.as_ref() != Some(&column.schema) {
                return Err(FormulaBindError::TypeMismatch {
                    message: "conditional aggregate columns must all belong to one schema".into(),
                });
            }
            ConditionalAggregate::Sum(column)
        }
        UnboundConditionalAggregate::Average(address) => {
            let column = bind_column_reference(document, index, address)?;
            schema.get_or_insert_with(|| column.schema.clone());
            if schema.as_ref() != Some(&column.schema) {
                return Err(FormulaBindError::TypeMismatch {
                    message: "conditional aggregate columns must all belong to one schema".into(),
                });
            }
            ConditionalAggregate::Average(column)
        }
        UnboundConditionalAggregate::Count => ConditionalAggregate::Count,
    };
    Ok(Expression::ColumnConditional {
        aggregate: Box::new(aggregate),
        criteria: bound_criteria,
    })
}

/// A criterion value is read as a whole cell: a bare reference may name any
/// scalar column. Every other expression is arithmetic and binds numerically.
fn bind_criterion_value(
    document: &Document,
    index: &AddressIndex,
    owner: &FieldRef,
    expression: &UnboundExpression,
) -> Result<Expression, FormulaBindError> {
    match expression {
        UnboundExpression::Reference(address) => Ok(Expression::Reference(bind_scalar_reference(
            document, index, address,
        )?)),
        UnboundExpression::SameRowReference(field) => {
            let entity =
                document
                    .entities
                    .get(&owner.entity)
                    .ok_or_else(|| FormulaBindError::Index {
                        source: Box::new(AddressIndexError::MissingBoundEntity {
                            entity: owner.entity.clone(),
                        }),
                    })?;
            let address = FieldAddress::new(entity.key.clone(), field.clone());
            Ok(Expression::SameRowReference(
                bind_scalar_reference(document, index, &address)?.field,
            ))
        }
        other => bind_node(document, index, owner, other),
    }
}

fn bind_node(
    document: &Document,
    index: &AddressIndex,
    owner: &FieldRef,
    expression: &UnboundExpression,
) -> Result<Expression, FormulaBindError> {
    Ok(match expression {
        UnboundExpression::Number(number) => Expression::Number(*number),
        UnboundExpression::Reference(address) => {
            Expression::Reference(bind_numeric_reference(document, index, address)?)
        }
        UnboundExpression::SameRowReference(field) => {
            let entity =
                document
                    .entities
                    .get(&owner.entity)
                    .ok_or_else(|| FormulaBindError::Index {
                        source: Box::new(AddressIndexError::MissingBoundEntity {
                            entity: owner.entity.clone(),
                        }),
                    })?;
            let address = FieldAddress::new(entity.key.clone(), field.clone());
            Expression::SameRowReference(bind_numeric_reference(document, index, &address)?.field)
        }
        UnboundExpression::ColumnTotal(address) => {
            Expression::ColumnTotal(bind_column_reference(document, index, address)?)
        }
        UnboundExpression::ColumnAggregate { function, column } => Expression::ColumnAggregate {
            function: *function,
            column: bind_column_reference(document, index, column)?,
        },
        UnboundExpression::ColumnConditional {
            aggregate,
            criteria,
        } => bind_conditional(document, index, owner, aggregate, criteria)?,
        UnboundExpression::Add { left, right } => Expression::Add {
            left: Box::new(bind_node(document, index, owner, left)?),
            right: Box::new(bind_node(document, index, owner, right)?),
        },
        UnboundExpression::Subtract { left, right } => Expression::Subtract {
            left: Box::new(bind_node(document, index, owner, left)?),
            right: Box::new(bind_node(document, index, owner, right)?),
        },
        UnboundExpression::Multiply { left, right } => Expression::Multiply {
            left: Box::new(bind_node(document, index, owner, left)?),
            right: Box::new(bind_node(document, index, owner, right)?),
        },
        UnboundExpression::Divide { left, right } => Expression::Divide {
            left: Box::new(bind_node(document, index, owner, left)?),
            right: Box::new(bind_node(document, index, owner, right)?),
        },
        UnboundExpression::Minimum { left, right } => Expression::Minimum {
            left: Box::new(bind_node(document, index, owner, left)?),
            right: Box::new(bind_node(document, index, owner, right)?),
        },
        UnboundExpression::Maximum { left, right } => Expression::Maximum {
            left: Box::new(bind_node(document, index, owner, left)?),
            right: Box::new(bind_node(document, index, owner, right)?),
        },
        UnboundExpression::Compare {
            operator,
            left,
            right,
        } => Expression::Compare {
            operator: *operator,
            left: Box::new(bind_node(document, index, owner, left)?),
            right: Box::new(bind_node(document, index, owner, right)?),
        },
        UnboundExpression::If {
            condition,
            then_value,
            else_value,
        } => Expression::If {
            condition: Box::new(bind_node(document, index, owner, condition)?),
            then_value: Box::new(bind_node(document, index, owner, then_value)?),
            else_value: Box::new(bind_node(document, index, owner, else_value)?),
        },
        UnboundExpression::And(arguments) => {
            Expression::And(bind_arguments(document, index, owner, arguments)?)
        }
        UnboundExpression::Or(arguments) => {
            Expression::Or(bind_arguments(document, index, owner, arguments)?)
        }
        UnboundExpression::Not(value) => {
            Expression::Not(Box::new(bind_node(document, index, owner, value)?))
        }
        UnboundExpression::Round { value, digits } => Expression::Round {
            value: Box::new(bind_node(document, index, owner, value)?),
            digits: bind_round_digits(digits)?,
        },
        UnboundExpression::IsBlank(reference) => Expression::IsBlank(Box::new(
            bind_blank_reference(document, index, owner, reference)?,
        )),
    })
}

fn bind_round_digits(expression: &UnboundExpression) -> Result<u8, FormulaBindError> {
    let UnboundExpression::Number(digits) = expression else {
        return Err(round_digits_error());
    };
    let digits = digits.get();
    if !(0.0..=10.0).contains(&digits) || digits.fract() != 0.0 {
        return Err(round_digits_error());
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    // The literal was checked to be an integer in 0..=10.
    Ok(digits as u8)
}

fn round_digits_error() -> FormulaBindError {
    FormulaBindError::TypeMismatch {
        message: "ROUND digits must be an integer literal in 0..=10".into(),
    }
}

fn bind_arguments(
    document: &Document,
    index: &AddressIndex,
    owner: &FieldRef,
    arguments: &[UnboundExpression],
) -> Result<Vec<Expression>, FormulaBindError> {
    arguments
        .iter()
        .map(|argument| bind_node(document, index, owner, argument))
        .collect()
}

fn bind_blank_reference(
    document: &Document,
    index: &AddressIndex,
    owner: &FieldRef,
    expression: &UnboundExpression,
) -> Result<Expression, FormulaBindError> {
    let (address, same_row) =
        match expression {
            UnboundExpression::Reference(address) => (address.clone(), false),
            UnboundExpression::SameRowReference(field) => {
                let entity = document.entities.get(&owner.entity).ok_or_else(|| {
                    FormulaBindError::Index {
                        source: Box::new(AddressIndexError::MissingBoundEntity {
                            entity: owner.entity.clone(),
                        }),
                    }
                })?;
                (FieldAddress::new(entity.key.clone(), field.clone()), true)
            }
            _ => {
                return Err(FormulaBindError::TypeMismatch {
                    message: "ISBLANK requires a field reference".into(),
                });
            }
        };
    let reference =
        index
            .resolve_field(document, &address)
            .map_err(|source| FormulaBindError::Address {
                address,
                source: Box::new(source),
            })?;
    Ok(if same_row {
        Expression::SameRowReference(reference.field)
    } else {
        Expression::Reference(reference)
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
    owner: &FieldRef,
    expression: &Expression,
) -> Result<String, CanonicalAuthoringProjectionError> {
    validate_expression_structure(expression)?;
    // Column totals are projected through their own identity, so only the
    // direct references need a human-address round-trip proof here.
    let references = direct_references(owner, expression);
    let Ok(index) = AddressIndex::build(document) else {
        return Err(
            CanonicalAuthoringProjectionError::UnresolvableBoundReferences {
                targets: references,
            },
        );
    };
    project_through_index(document, &index, owner, expression, references)
}

/// Project a bound formula through an [`AddressIndex`] the caller built once.
///
/// Returns the same value as [`project_expression`]; callers that project every
/// formula of one document reuse a single index instead of rebuilding it.
///
/// # Errors
///
/// Returns a typed stable-target set or structural/canonical-length failure.
pub fn project_expression_with_index(
    document: &Document,
    index: &AddressIndex,
    owner: &FieldRef,
    expression: &Expression,
) -> Result<String, CanonicalAuthoringProjectionError> {
    validate_expression_structure(expression)?;
    // Column totals are projected through their own identity, so only the
    // direct references need a human-address round-trip proof here.
    let references = direct_references(owner, expression);
    project_through_index(document, index, owner, expression, references)
}

fn project_through_index(
    document: &Document,
    index: &AddressIndex,
    owner: &FieldRef,
    expression: &Expression,
    references: BTreeSet<FieldRef>,
) -> Result<String, CanonicalAuthoringProjectionError> {
    let mut addresses = BTreeMap::new();
    let mut unresolved = BTreeSet::new();
    for target in references {
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

    let mut columns = BTreeMap::new();
    let mut unresolved_columns = BTreeSet::new();
    let numeric_columns = extract_value_columns(expression);
    for column in extract_column_totals(expression) {
        match column_address(document, &column, numeric_columns.contains(&column)) {
            Some(address) => {
                columns.insert(column, address);
            }
            None => {
                unresolved_columns.insert(column);
            }
        }
    }
    if !unresolved_columns.is_empty() {
        return Err(
            CanonicalAuthoringProjectionError::UnresolvableBoundColumns {
                columns: unresolved_columns,
            },
        );
    }

    let projected = render_bound(expression, owner, &addresses, &columns);
    if projected.len() > parser::MAX_INPUT_BYTES {
        return Err(ExpressionComplexityError::CanonicalLengthLimit.into());
    }
    Ok(projected)
}

fn render_bound(
    expression: &Expression,
    owner: &FieldRef,
    addresses: &BTreeMap<FieldRef, FieldAddress>,
    columns: &BTreeMap<ColumnRef, ColumnAddress>,
) -> String {
    match expression {
        Expression::Number(number) => parser::format_number(*number),
        Expression::Reference(reference) => {
            let address = &addresses[reference];
            format!("[{}.{}]", address.entity, address.field)
        }
        Expression::SameRowReference(field) => {
            let reference = FieldRef::new(owner.entity.clone(), field.clone());
            format!("[.{}]", addresses[&reference].field)
        }
        Expression::ColumnTotal(column) => {
            let address = &columns[column];
            format!("[{}.{}:total]", address.schema, address.field)
        }
        Expression::ColumnAggregate { function, column } => {
            let address = &columns[column];
            format!(
                "[{}.{}:{}]",
                address.schema,
                address.field,
                function.suffix()
            )
        }
        Expression::ColumnConditional {
            aggregate,
            criteria,
        } => render_conditional(aggregate, criteria, owner, addresses, columns),
        Expression::Add { left, right } => {
            render_bound_binary(left, "+", right, owner, addresses, columns)
        }
        Expression::Subtract { left, right } => {
            render_bound_binary(left, "-", right, owner, addresses, columns)
        }
        Expression::Multiply { left, right } => {
            render_bound_binary(left, "*", right, owner, addresses, columns)
        }
        Expression::Divide { left, right } => {
            render_bound_binary(left, "/", right, owner, addresses, columns)
        }
        Expression::Minimum { left, right } => format!(
            "min({}, {})",
            render_bound(left, owner, addresses, columns),
            render_bound(right, owner, addresses, columns)
        ),
        Expression::Maximum { left, right } => format!(
            "max({}, {})",
            render_bound(left, owner, addresses, columns),
            render_bound(right, owner, addresses, columns)
        ),
        Expression::Compare {
            operator,
            left,
            right,
        } => render_bound_binary(left, operator.as_str(), right, owner, addresses, columns),
        Expression::If {
            condition,
            then_value,
            else_value,
        } => format!(
            "IF({}, {}, {})",
            render_bound(condition, owner, addresses, columns),
            render_bound(then_value, owner, addresses, columns),
            render_bound(else_value, owner, addresses, columns)
        ),
        Expression::And(arguments) => {
            render_bound_function("AND", arguments, owner, addresses, columns)
        }
        Expression::Or(arguments) => {
            render_bound_function("OR", arguments, owner, addresses, columns)
        }
        Expression::Not(value) => {
            format!("NOT({})", render_bound(value, owner, addresses, columns))
        }
        Expression::Round { value, digits } => {
            format!(
                "ROUND({}, {digits})",
                render_bound(value, owner, addresses, columns)
            )
        }
        Expression::IsBlank(value) => format!(
            "ISBLANK({})",
            render_bound(value, owner, addresses, columns)
        ),
    }
}

fn render_bound_function(
    name: &str,
    arguments: &[Expression],
    owner: &FieldRef,
    addresses: &BTreeMap<FieldRef, FieldAddress>,
    columns: &BTreeMap<ColumnRef, ColumnAddress>,
) -> String {
    format!(
        "{name}({})",
        arguments
            .iter()
            .map(|argument| render_bound(argument, owner, addresses, columns))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn render_bound_binary(
    left: &Expression,
    operator: &str,
    right: &Expression,
    owner: &FieldRef,
    addresses: &BTreeMap<FieldRef, FieldAddress>,
    columns: &BTreeMap<ColumnRef, ColumnAddress>,
) -> String {
    format!(
        "({} {operator} {})",
        render_bound(left, owner, addresses, columns),
        render_bound(right, owner, addresses, columns)
    )
}

fn render_conditional(
    aggregate: &ConditionalAggregate,
    criteria: &[ColumnCriterion],
    owner: &FieldRef,
    addresses: &BTreeMap<FieldRef, FieldAddress>,
    columns: &BTreeMap<ColumnRef, ColumnAddress>,
) -> String {
    let mut arguments = Vec::new();
    let name = match aggregate {
        ConditionalAggregate::Sum(values) => {
            arguments.push(render_column(&columns[values]));
            "SUMIFS"
        }
        ConditionalAggregate::Average(values) => {
            arguments.push(render_column(&columns[values]));
            "AVERAGEIFS"
        }
        ConditionalAggregate::Count => "COUNTIFS",
    };
    for criterion in criteria {
        arguments.push(render_column(&columns[&criterion.column]));
        arguments.push(render_criterion(criterion, owner, addresses, columns));
    }
    format!("{name}({})", arguments.join(", "))
}

fn render_criterion(
    criterion: &ColumnCriterion,
    owner: &FieldRef,
    addresses: &BTreeMap<FieldRef, FieldAddress>,
    columns: &BTreeMap<ColumnRef, ColumnAddress>,
) -> String {
    match criterion.value.as_deref() {
        None => {
            let text = if criterion.operator == ComparisonOperator::Equal {
                ""
            } else {
                "<>"
            };
            format!("\"{text}\"")
        }
        Some(expression) if criterion.operator == ComparisonOperator::Equal => {
            render_bound(expression, owner, addresses, columns)
        }
        Some(Expression::Number(number)) => format!(
            "\"{}{}\"",
            criterion.operator.as_str(),
            crate::parser::format_number(*number)
        ),
        Some(expression) => render_bound(expression, owner, addresses, columns),
    }
}

fn render_column(address: &ColumnAddress) -> String {
    format!("[{}.{}:column]", address.schema, address.field)
}

/// Resolve a column identity to the document's current human keys.
///
/// A value column must still be a Number column, because canonical text only
/// round-trips if the projected address binds back to the same column total.
/// A criterion column may hold any scalar.
fn column_address(
    document: &Document,
    column: &ColumnRef,
    require_number: bool,
) -> Option<ColumnAddress> {
    let schema = document.schemas.get(&column.schema)?;
    let field = schema.fields.get(&column.field)?;
    if matches!(field.field_type, FieldType::Reference { .. }) {
        return None;
    }
    if require_number && field.field_type != FieldType::Number {
        return None;
    }
    Some(ColumnAddress::new(schema.key.clone(), field.key.clone()))
}

/// The columns whose cells the aggregate sums: the values column of a total or
/// aggregate, and a conditional node's values column. Criterion columns feed
/// row membership but are read whatever their scalar type.
fn extract_value_columns(expression: &Expression) -> BTreeSet<ColumnRef> {
    expression.numeric_columns().into_iter().cloned().collect()
}

/// Collect every column identity named by a bound expression.
fn extract_column_totals(expression: &Expression) -> BTreeSet<ColumnRef> {
    expression.column_totals().into_iter().cloned().collect()
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
        stack.extend(
            node.children()
                .into_iter()
                .rev()
                .map(|child| (child, depth + 1)),
        );
    }
    Ok(())
}

/// Collect the value nodes named by direct references (`[e.f]` and `[.f]`).
fn direct_references(owner: &FieldRef, expression: &Expression) -> BTreeSet<FieldRef> {
    expression
        .reference_uses(owner)
        .into_iter()
        .map(|(reference, _)| reference)
        .collect()
}

/// Extract the deterministic static dependency set from a bound expression.
///
/// Same-row references resolve against `owner`, and a column total expands to
/// that column on every entity of the document's current schema, so the set is
/// exactly the stable value nodes the formula reads. Because a column total's
/// expansion follows current row membership, retained incremental state
/// re-extracts it whenever a row of that schema changes.
#[must_use]
pub fn extract_dependencies(
    document: &Document,
    owner: &FieldRef,
    expression: &Expression,
) -> BTreeSet<FieldRef> {
    let mut dependencies = expression
        .reference_uses(owner)
        .into_iter()
        .map(|(reference, _)| reference)
        .collect::<BTreeSet<_>>();
    for column in expression.column_totals() {
        for (entity_id, entity) in &document.entities {
            if entity.schema == column.schema {
                dependencies.insert(FieldRef::new(entity_id.clone(), column.field.clone()));
            }
        }
    }
    dependencies
}

#[derive(Clone, Debug, PartialEq)]
pub struct Calculation {
    values: BTreeMap<FieldRef, Number>,
    graph: DependencyGraph,
}

/// Complete atomic result of a fresh full-document recomputation.
///
/// A failed outcome deliberately contains no successful values. Static
/// dependencies remain available because they are extracted before graph and
/// evaluation failure classification.
#[derive(Clone, Debug, PartialEq)]
pub enum CalculationOutcome {
    Complete(Calculation),
    Failed(CalculationFailures),
}

/// Runtime-only formula state retained by a resident semantic workspace.
///
/// Successful values may remain present internally while another independent
/// component is failed so a later local edit can reuse unaffected work. They
/// are never exposed through [`Self::outcome`] while any failure exists.
/// This state is derived entirely from one [`Document`] and is not canonical
/// semantic state, a cache key contract, or a serialization surface.
#[derive(Clone, Debug, PartialEq)]
pub struct RetainedCalculationState {
    nodes: BTreeSet<FieldRef>,
    values: BTreeMap<FieldRef, Number>,
    failures: FailureMap,
    graph: DependencyGraph,
}

/// Scratch measurement (Plan W1): the retained map sizes behind one
/// [`RetainedCalculationState`]. Not semantic identity or a stable contract.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RetainedCalculationSizes {
    pub nodes: usize,
    pub values: usize,
    pub dependency_keys: usize,
    pub dependency_edges: usize,
    pub reverse_keys: usize,
    pub reverse_edges: usize,
}

/// Deterministic work evidence for one retained calculation transition.
///
/// These counters describe the current provisional algorithm. They are
/// runtime/benchmark evidence, not semantic identity or a performance SLA.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IncrementalCalculationWork {
    pub full_rebuilds: usize,
    pub incremental_updates: usize,
    pub nodes_recomputed: usize,
    pub nodes_reused: usize,
    pub reverse_edges_traversed: usize,
}

/// Runtime-only transition evidence consumed by resident projection
/// invalidation. Both field lists are deterministic and exclude dirty roots
/// from `affected_calculations` while retaining changed root projections in
/// `changed_calculation_projections` when applicable.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IncrementalCalculationTransition {
    pub work: IncrementalCalculationWork,
    pub affected_calculations: Vec<FieldRef>,
    pub changed_calculation_projections: Vec<FieldRef>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RetainedNodeProjection {
    Value(Number),
    Failure(CalculationFailure),
    Unavailable,
}

struct DirtyCalculationImpact {
    dirty: BTreeSet<FieldRef>,
    old_dependents: BTreeSet<FieldRef>,
    new_dependents: BTreeSet<FieldRef>,
    reverse_edges_traversed: usize,
}

struct FullCalculationParts {
    nodes: BTreeSet<FieldRef>,
    values: BTreeMap<FieldRef, Number>,
    failures: FailureMap,
    graph: DependencyGraph,
}

impl RetainedCalculationState {
    /// Rebuild all retained formula state through the full ADR-0018 oracle
    /// phases.
    #[must_use]
    pub fn rebuild(document: &Document) -> (Self, IncrementalCalculationWork) {
        let FullCalculationParts {
            nodes,
            values,
            failures,
            graph,
        } = full_calculation_parts(document);
        let work = IncrementalCalculationWork {
            full_rebuilds: 1,
            nodes_recomputed: nodes.len(),
            ..IncrementalCalculationWork::default()
        };
        (
            Self {
                nodes,
                values,
                failures,
                graph,
            },
            work,
        )
    }

    /// Sizes of the retained maps (scratch Plan W1 measurement).
    #[must_use]
    pub fn sizes(&self) -> RetainedCalculationSizes {
        RetainedCalculationSizes {
            nodes: self.nodes.len(),
            values: self.values.len(),
            dependency_keys: self.graph.reference_uses.len(),
            dependency_edges: self.graph.entry_count(),
            reverse_keys: 0,
            reverse_edges: 0,
        }
    }

    /// Recompute dirty roots and their old/new-graph reverse dependent closure.
    ///
    /// The caller owns conservative impact classification. When that cannot be
    /// proved, discard this state and call [`Self::rebuild`] instead. Column
    /// totals make row membership part of a formula's dependencies; the
    /// caller only lists changed rows, and this update re-extracts every
    /// column total those rows can affect.
    #[must_use]
    pub fn update(
        &mut self,
        document: &Document,
        dirty_roots: &BTreeSet<FieldRef>,
    ) -> IncrementalCalculationTransition {
        if dirty_roots.is_empty() {
            return IncrementalCalculationTransition {
                work: IncrementalCalculationWork {
                    incremental_updates: 1,
                    nodes_reused: self.nodes.len(),
                    ..IncrementalCalculationWork::default()
                },
                ..IncrementalCalculationTransition::default()
            };
        }

        let before_failed = self.is_failed();
        let before_formulas = self.graph.formulas().cloned().collect::<BTreeSet<_>>();
        let dirty_roots = &Self::with_column_total_roots(document, &self.graph, dirty_roots);
        let impact = self.update_dependency_impact(document, dirty_roots);
        let before_projections = impact
            .dirty
            .iter()
            .map(|field| (field.clone(), self.node_projection(field)))
            .collect::<BTreeMap<_, _>>();
        self.recompute_dirty(document, dirty_roots, &impact.dirty);

        let nodes_recomputed = impact
            .dirty
            .iter()
            .filter(|field| self.nodes.contains(*field))
            .count();
        let work = IncrementalCalculationWork {
            incremental_updates: 1,
            nodes_recomputed,
            nodes_reused: self.nodes.len().saturating_sub(nodes_recomputed),
            reverse_edges_traversed: impact.reverse_edges_traversed,
            ..IncrementalCalculationWork::default()
        };
        let after_failed = self.is_failed();
        let formula_fields = before_formulas
            .into_iter()
            .chain(self.graph.formulas().cloned())
            .collect::<BTreeSet<_>>();
        let changed_calculation_projections = if before_failed == after_failed {
            formula_fields
                .intersection(&impact.dirty)
                .filter(|field| {
                    before_projections.get(*field) != Some(&self.node_projection(field))
                })
                .cloned()
                .collect()
        } else {
            formula_fields.into_iter().collect()
        };
        IncrementalCalculationTransition {
            work,
            affected_calculations: impact
                .old_dependents
                .into_iter()
                .chain(impact.new_dependents)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            changed_calculation_projections,
        }
    }

    /// A column total's dependencies are the schema's current rows, so a
    /// changed row can change the dependency set of a formula that is not
    /// itself dirty. Promote every column-total formula that read a root
    /// before (old graph) or totals a root's schema now, so its dependencies
    /// are re-extracted rather than left stale.
    fn with_column_total_roots(
        document: &Document,
        graph: &DependencyGraph,
        dirty_roots: &BTreeSet<FieldRef>,
    ) -> BTreeSet<FieldRef> {
        let mut roots = dirty_roots.clone();
        // A read column's member set changes whenever a row of its schema is
        // added, removed or moved. Promotion is schema-wide rather than
        // slot-exact so those cases cannot be missed; the extra readers it
        // dirties are recomputed, which is safe.
        let root_schemas = dirty_roots
            .iter()
            .flat_map(|root| graph.schemas_of(document, &root.entity))
            .collect::<BTreeSet<_>>();
        for column in graph.tracked_columns() {
            if root_schemas.contains(&column.schema) {
                for dependent in graph.column_readers(column) {
                    roots.insert(dependent.clone());
                }
            }
        }
        roots
    }

    fn update_dependency_impact(
        &mut self,
        document: &Document,
        dirty_roots: &BTreeSet<FieldRef>,
    ) -> DirtyCalculationImpact {
        let (old_dependents, old_edges) = self.graph.reverse_closure(dirty_roots, false);
        let mut touched_schemas = BTreeSet::new();
        for root in dirty_roots {
            touched_schemas.extend(self.graph.schemas_of(document, &root.entity));
            self.graph.remember_entity(document, &root.entity);
            self.graph.replace_formula(document, root);
        }
        // Inserting or removing a row changes the member slots of every read
        // column of that row's schema, whether or not the row was blank.
        let touched_columns = self
            .graph
            .tracked_columns()
            .filter(|column| touched_schemas.contains(&column.schema))
            .cloned()
            .collect::<BTreeSet<_>>();
        self.graph.refresh_column_slots(document, &touched_columns);
        for root in dirty_roots {
            if !document.entities.contains_key(&root.entity) {
                self.graph.forget_entity(&root.entity);
            }
        }
        let (new_dependents, new_edges) = self.graph.reverse_closure(dirty_roots, false);
        let dirty = dirty_roots
            .iter()
            .chain(old_dependents.iter())
            .chain(new_dependents.iter())
            .cloned()
            .collect();
        DirtyCalculationImpact {
            dirty,
            old_dependents,
            new_dependents,
            reverse_edges_traversed: old_edges.saturating_add(new_edges),
        }
    }

    fn recompute_dirty(
        &mut self,
        document: &Document,
        dirty_roots: &BTreeSet<FieldRef>,
        dirty: &BTreeSet<FieldRef>,
    ) {
        for root in dirty_roots {
            if calculation_value(document, root).is_some() {
                self.nodes.insert(root.clone());
            } else {
                self.nodes.remove(root);
            }
        }
        let dirty_nodes = dirty
            .iter()
            .filter_map(|field| {
                calculation_value(document, field).map(|value| (field.clone(), value))
            })
            .collect::<ValueNodes<'_>>();
        let dirty_formulas = dirty_nodes
            .iter()
            .filter_map(|(field, value)| match value {
                Value::Formula(expression) => Some((field.clone(), expression)),
                Value::Number(_)
                | Value::Text(_)
                | Value::Boolean(_)
                | Value::Date(_)
                | Value::Reference(_) => None,
            })
            .collect::<FormulaNodes<'_>>();

        for field in dirty {
            self.failures.remove(field);
            self.values.remove(field);
        }
        self.failures
            .extend(pregraph_failures(document, &dirty_nodes, &dirty_formulas));
        assign_cycle_failures(&dirty_formulas, &self.graph, &mut self.failures);
        self.values
            .extend(initial_values(&dirty_nodes, &self.failures));
        evaluate_remaining_formulas(
            document,
            &dirty_formulas,
            &self.graph,
            &mut self.failures,
            &mut self.values,
        );
    }

    /// Project the Accepted atomic calculation outcome for this revision.
    #[must_use]
    pub fn outcome(&self) -> CalculationOutcome {
        if self.failures.is_empty() {
            CalculationOutcome::Complete(Calculation {
                values: self.values.clone(),
                graph: self.graph.clone(),
            })
        } else {
            CalculationOutcome::Failed(CalculationFailures {
                failures: self.failures.clone(),
                graph: self.graph.clone(),
            })
        }
    }

    /// Return whether the atomic calculation outcome is failed.
    #[must_use]
    pub fn is_failed(&self) -> bool {
        !self.failures.is_empty()
    }

    /// Return a published value only when the complete calculation is atomic.
    #[must_use]
    pub fn value(&self, field: &FieldRef) -> Option<Number> {
        self.failures
            .is_empty()
            .then(|| self.values.get(field).copied())
            .flatten()
    }

    /// Return the complete published value map only for an atomic successful
    /// outcome.
    #[must_use]
    pub fn complete_values(&self) -> Option<&BTreeMap<FieldRef, Number>> {
        self.failures.is_empty().then_some(&self.values)
    }

    /// Return one primary failure only while the atomic outcome is failed.
    #[must_use]
    pub fn failure(&self, field: &FieldRef) -> Option<&CalculationFailure> {
        self.failures.get(field)
    }

    /// Return all primary failures retained for the current revision.
    #[must_use]
    pub fn failures(&self) -> &BTreeMap<FieldRef, CalculationFailure> {
        &self.failures
    }

    /// Return every current static formula dependency set, expanded per cell.
    ///
    /// The expansion costs O(formulas x rows); use [`Self::dependencies_of`]
    /// and [`Self::formulas`] on edit paths.
    #[must_use]
    pub fn dependencies(&self) -> BTreeMap<FieldRef, BTreeSet<FieldRef>> {
        self.graph.expanded_dependencies()
    }

    /// Return the expanded static dependency set of one formula.
    #[must_use]
    pub fn dependencies_of(&self, field: &FieldRef) -> Option<BTreeSet<FieldRef>> {
        self.graph.dependencies_of(field)
    }

    /// Return the formulas whose expanded dependencies contain `field`.
    #[must_use]
    pub fn direct_dependents(&self, field: &FieldRef) -> BTreeSet<FieldRef> {
        self.graph.direct_dependents(field)
    }

    /// Iterate the formula keys of the current dependency graph.
    pub fn formulas(&self) -> impl Iterator<Item = &FieldRef> {
        self.graph.formulas()
    }

    /// Total entry count of the dependency graph, for memory-size assertions.
    #[must_use]
    pub fn graph_entry_count(&self) -> usize {
        self.graph.entry_count()
    }

    fn node_projection(&self, field: &FieldRef) -> RetainedNodeProjection {
        if self.is_failed() {
            self.failures
                .get(field)
                .map_or(RetainedNodeProjection::Unavailable, |failure| {
                    RetainedNodeProjection::Failure(failure.clone())
                })
        } else {
            self.values
                .get(field)
                .map_or(RetainedNodeProjection::Unavailable, |value| {
                    RetainedNodeProjection::Value(*value)
                })
        }
    }

    /// Return the deterministic reverse transitive closure, excluding the
    /// supplied dirty roots themselves.
    #[must_use]
    pub fn affected_by_all(&self, dirty_roots: &BTreeSet<FieldRef>) -> Vec<FieldRef> {
        self.graph
            .reverse_closure(dirty_roots, false)
            .0
            .into_iter()
            .collect()
    }
}

/// Every primary semantic failure from one failed full recomputation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CalculationFailures {
    failures: BTreeMap<FieldRef, CalculationFailure>,
    graph: DependencyGraph,
}

impl CalculationFailures {
    /// Return the stable value-node-keyed primary failure map.
    #[must_use]
    pub fn failures(&self) -> &BTreeMap<FieldRef, CalculationFailure> {
        &self.failures
    }

    /// Return every statically extracted formula dependency set, including
    /// edges incident to failed nodes, expanded per cell.
    ///
    /// The expansion costs O(formulas x rows); use [`Self::dependencies_of`]
    /// and [`Self::formulas`] on edit paths.
    #[must_use]
    pub fn dependencies(&self) -> BTreeMap<FieldRef, BTreeSet<FieldRef>> {
        self.graph.expanded_dependencies()
    }

    /// Return the expanded static dependency set of one formula.
    #[must_use]
    pub fn dependencies_of(&self, field: &FieldRef) -> Option<BTreeSet<FieldRef>> {
        self.graph.dependencies_of(field)
    }

    /// Iterate the formula keys of the current dependency graph.
    pub fn formulas(&self) -> impl Iterator<Item = &FieldRef> {
        self.graph.formulas()
    }

    /// Return the formulas whose expanded dependencies contain `field`.
    #[must_use]
    pub fn direct_dependents(&self, field: &FieldRef) -> BTreeSet<FieldRef> {
        self.graph.direct_dependents(field)
    }

    /// Return the reverse transitive closure of `changed`, including a changed
    /// formula that reaches itself.
    #[must_use]
    pub fn affected_by(&self, changed: &FieldRef) -> Vec<FieldRef> {
        self.graph.affected_by(changed)
    }

    fn compatibility_error(&self) -> CalculationError {
        let (mut field, mut failure) = self
            .failures
            .first_key_value()
            .expect("failed calculation must contain a primary failure");
        let mut visited = BTreeSet::new();
        loop {
            assert!(
                visited.insert(field.clone()),
                "failed-dependency outcomes form an acyclic graph"
            );
            match failure {
                CalculationFailure::TypeMismatch => {
                    return CalculationError::InvalidExpression {
                        formula: field.clone(),
                        message: "formula type mismatch".into(),
                    };
                }
                CalculationFailure::InvalidExpression { error } => {
                    return CalculationError::InvalidExpression {
                        formula: field.clone(),
                        message: error.to_string(),
                    };
                }
                CalculationFailure::InvalidReferences { targets } => {
                    let (reference, reference_failure) = targets
                        .first_key_value()
                        .expect("invalid-reference failure must contain a target");
                    return match reference_failure {
                        ReferenceFailure::Missing => CalculationError::MissingReference {
                            reference: reference.clone(),
                        },
                        ReferenceFailure::NonNumeric => CalculationError::NonNumericReference {
                            reference: reference.clone(),
                        },
                    };
                }
                CalculationFailure::Cycle { members } => {
                    return CalculationError::Cycle {
                        path: self.compatibility_cycle_witness(field, members.as_ref()),
                    };
                }
                CalculationFailure::FailedDependencies { dependencies } => {
                    let Some((dependency, dependency_failure)) =
                        dependencies.iter().find_map(|dependency| {
                            self.failures
                                .get(dependency)
                                .map(|failure| (dependency, failure))
                        })
                    else {
                        return CalculationError::MissingReference {
                            reference: dependencies
                                .first()
                                .cloned()
                                .unwrap_or_else(|| field.clone()),
                        };
                    };
                    field = dependency;
                    failure = dependency_failure;
                }
                CalculationFailure::EmptyAggregate => {
                    return CalculationError::EmptyAggregate {
                        formula: field.clone(),
                    };
                }
                CalculationFailure::DivisionByZero => {
                    return CalculationError::DivisionByZero {
                        formula: field.clone(),
                    };
                }
                CalculationFailure::NonFiniteResult => {
                    return CalculationError::NonFiniteResult {
                        field: field.clone(),
                    };
                }
                CalculationFailure::InvalidColumn { columns } => {
                    let (column, column_failure) = columns
                        .first_key_value()
                        .expect("invalid-column failure must contain a column");
                    return CalculationError::InvalidColumn {
                        column: column.clone(),
                        message: match column_failure {
                            ColumnFailure::MissingSchema => "column schema does not exist",
                            ColumnFailure::MissingField => "column field does not exist",
                            ColumnFailure::NonNumeric => "column field is not numeric",
                        }
                        .to_owned(),
                    };
                }
            }
        }
    }

    /// Derive a deterministic legacy witness from semantic SCC membership.
    /// The witness is presentation-only; the complete member set above is the
    /// graph-failure authority.
    fn compatibility_cycle_witness(
        &self,
        selected: &FieldRef,
        members: &BTreeSet<FieldRef>,
    ) -> Vec<FieldRef> {
        let start = if members.contains(selected) {
            selected
        } else {
            members
                .first()
                .expect("cycle failure must contain an SCC member")
        };
        let outgoing = self.graph.formula_dependencies_within(start, members);

        for next in outgoing {
            if &next == start {
                return vec![start.clone(), start.clone()];
            }

            let mut queue = VecDeque::from([next.clone()]);
            let mut visited = BTreeSet::from([next.clone()]);
            let mut parents: BTreeMap<FieldRef, FieldRef> = BTreeMap::new();
            while let Some(node) = queue.pop_front() {
                for dependency in self.graph.formula_dependencies_within(&node, members) {
                    if &dependency == start {
                        let mut reverse_path = vec![node.clone()];
                        while reverse_path.last() != Some(&next) {
                            let parent = parents
                                .get(reverse_path.last().expect("path is non-empty"))
                                .expect("visited SCC node has a parent")
                                .clone();
                            reverse_path.push(parent);
                        }
                        reverse_path.reverse();
                        let mut path = vec![start.clone()];
                        path.extend(reverse_path);
                        path.push(start.clone());
                        return path;
                    }
                    if visited.insert(dependency.clone()) {
                        parents.insert(dependency.clone(), node.clone());
                        queue.push_back(dependency.clone());
                    }
                }
            }
        }

        debug_assert!(false, "SCC member must have a cycle witness");
        let mut fallback = members.iter().cloned().collect::<Vec<_>>();
        fallback.push(start.clone());
        fallback
    }
}

/// One primary semantic failure for a stable value node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CalculationFailure {
    TypeMismatch,
    InvalidExpression {
        error: ExpressionComplexityError,
    },
    InvalidReferences {
        targets: BTreeMap<FieldRef, ReferenceFailure>,
    },
    Cycle {
        /// One immutable SCC set shared by its members: O(n) rather than O(n²) memory.
        /// `Arc` preserves `Send`/`Sync` for calculation results used by native hosts.
        members: Arc<BTreeSet<FieldRef>>,
    },
    FailedDependencies {
        dependencies: BTreeSet<FieldRef>,
    },
    DivisionByZero,
    EmptyAggregate,
    NonFiniteResult,
    InvalidColumn {
        columns: BTreeMap<ColumnRef, ColumnFailure>,
    },
}

/// Binding/type classification for one directly referenced stable value node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceFailure {
    Missing,
    NonNumeric,
}

/// Binding/type classification for one column total named by a formula.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColumnFailure {
    MissingSchema,
    MissingField,
    NonNumeric,
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

    /// Return every static formula dependency set, expanded per cell.
    ///
    /// The expansion costs O(formulas x rows); use [`Self::dependencies_of`]
    /// and [`Self::formulas`] on edit paths.
    #[must_use]
    pub fn dependencies(&self) -> BTreeMap<FieldRef, BTreeSet<FieldRef>> {
        self.graph.expanded_dependencies()
    }

    #[must_use]
    pub fn dependencies_of(&self, field: &FieldRef) -> Option<BTreeSet<FieldRef>> {
        self.graph.dependencies_of(field)
    }

    /// Iterate the formula keys of the current dependency graph.
    pub fn formulas(&self) -> impl Iterator<Item = &FieldRef> {
        self.graph.formulas()
    }

    /// Return the formulas whose expanded dependencies contain `field`.
    #[must_use]
    pub fn direct_dependents(&self, field: &FieldRef) -> BTreeSet<FieldRef> {
        self.graph.direct_dependents(field)
    }

    #[must_use]
    pub fn affected_by(&self, changed: &FieldRef) -> Vec<FieldRef> {
        self.graph.affected_by(changed)
    }

    /// For each of `targets`, the fields of `changed` that reach it.
    ///
    /// A changed field reaches exactly the formulas in [`Self::affected_by`]'s
    /// result for that field, including itself when the graph loops back to it.
    #[must_use]
    pub fn reaching_fields<'a>(
        &self,
        changed: impl IntoIterator<Item = &'a FieldRef>,
        targets: &BTreeSet<FieldRef>,
    ) -> BTreeMap<FieldRef, BTreeSet<FieldRef>> {
        self.graph.reaching_fields(changed, targets)
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
    #[error("formula '{formula}' reads an empty aggregate")]
    EmptyAggregate { formula: FieldRef },
    #[error("formula dependency cycle: {path:?}")]
    Cycle { path: Vec<FieldRef> },
    #[error("calculation for '{field}' produced a non-finite result")]
    NonFiniteResult { field: FieldRef },
    #[error("formula '{formula}' violates the bound expression contract: {message}")]
    InvalidExpression { formula: FieldRef, message: String },
    #[error("formula column '{column}' is invalid: {message}")]
    InvalidColumn { column: ColumnRef, message: String },
}

/// Calculate every numeric literal and formula in deterministic stable-ID order.
///
/// This compatibility API is a convenience projection of [`calculate_complete`].
/// A failed complete outcome projects the first stable-node-keyed primary
/// failure into the historical fail-first error family. In particular, a
/// projected cycle path is not semantic authority; [`CalculationFailure::Cycle`]
/// carries the authoritative complete SCC membership.
///
/// # Errors
///
/// Returns a [`CalculationError`] for invalid bound expressions, missing or
/// non-numeric references, cycles, division by zero, or non-finite results.
pub fn calculate(document: &Document) -> Result<Calculation, CalculationError> {
    match calculate_complete(document) {
        CalculationOutcome::Complete(calculation) => Ok(calculation),
        CalculationOutcome::Failed(failures) => Err(failures.compatibility_error()),
    }
}

/// Run the ADR-0018 complete atomic full-recompute correctness oracle.
///
/// Every calculation node receives one deterministic value-or-primary-failure
/// outcome. Independent failures accumulate, cycles use complete SCC member
/// sets, and dependency failures name only directly failed dependencies. A
/// failed outcome publishes no partial [`Calculation`].
#[must_use]
pub fn calculate_complete(document: &Document) -> CalculationOutcome {
    let FullCalculationParts {
        values,
        failures,
        graph,
        ..
    } = full_calculation_parts(document);

    if failures.is_empty() {
        CalculationOutcome::Complete(Calculation { values, graph })
    } else {
        CalculationOutcome::Failed(CalculationFailures { failures, graph })
    }
}

type ValueNodes<'document> = BTreeMap<FieldRef, &'document Value>;
type FormulaNodes<'document> = BTreeMap<FieldRef, &'document Expression>;
type FailureMap = BTreeMap<FieldRef, CalculationFailure>;

fn full_calculation_parts(document: &Document) -> FullCalculationParts {
    let (nodes, formulas, graph) = collect_calculation_nodes(document);
    let mut failures = pregraph_failures(document, &nodes, &formulas);
    assign_cycle_failures(&formulas, &graph, &mut failures);
    let mut values = initial_values(&nodes, &failures);
    evaluate_remaining_formulas(document, &formulas, &graph, &mut failures, &mut values);
    FullCalculationParts {
        nodes: nodes.into_keys().collect(),
        values,
        failures,
        graph,
    }
}

fn collect_calculation_nodes(
    document: &Document,
) -> (ValueNodes<'_>, FormulaNodes<'_>, DependencyGraph) {
    let mut nodes = BTreeMap::new();
    let mut formulas = BTreeMap::new();
    for (entity_id, entity) in &document.entities {
        for (field_id, value) in &entity.fields {
            if matches!(value, Value::Number(_) | Value::Formula(_)) {
                let field = FieldRef::new(entity_id.clone(), field_id.clone());
                nodes.insert(field.clone(), value);
                if let Value::Formula(expression) = value {
                    formulas.insert(field, expression);
                }
            }
        }
    }
    let graph = DependencyGraph::new(document, &formulas);
    (nodes, formulas, graph)
}

fn calculation_value<'document>(
    document: &'document Document,
    field: &FieldRef,
) -> Option<&'document Value> {
    document
        .entities
        .get(&field.entity)
        .and_then(|entity| entity.fields.get(&field.field))
        .filter(|value| matches!(value, Value::Number(_) | Value::Formula(_)))
}

/// The cells and columns one formula reads, from a document snapshot.
fn formula_uses(
    document: &Document,
    field: &FieldRef,
) -> Option<(BTreeSet<FieldRef>, BTreeSet<ColumnRef>)> {
    match calculation_value(document, field) {
        Some(Value::Formula(expression)) => Some((
            direct_references(field, expression),
            extract_column_totals(expression),
        )),
        Some(
            Value::Number(_)
            | Value::Text(_)
            | Value::Boolean(_)
            | Value::Date(_)
            | Value::Reference(_),
        )
        | None => None,
    }
}

/// Formula dependency graph: a formula holds one edge per column it reads,
/// not one per row, so memory stays linear in rows.
///
/// `reference_uses` keeps the cells a formula names directly and `column_uses`
/// the columns it reads. `cell_readers` and `column_readers` invert those
/// edges; `column_slots` holds every entity of a read column's schema (blank
/// slots included), exactly the set [`extract_dependencies`] expands to, and
/// `slot_columns` maps a slot back to its column.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct DependencyGraph {
    reference_uses: BTreeMap<FieldRef, BTreeSet<FieldRef>>,
    column_uses: BTreeMap<FieldRef, BTreeSet<ColumnRef>>,
    cell_readers: BTreeMap<FieldRef, BTreeSet<FieldRef>>,
    column_readers: BTreeMap<ColumnRef, BTreeSet<FieldRef>>,
    column_slots: BTreeMap<ColumnRef, BTreeSet<FieldRef>>,
    slot_columns: BTreeMap<FieldRef, ColumnRef>,
    entity_schemas: BTreeMap<tachiko_semantic_core::EntityId, tachiko_semantic_core::SchemaId>,
}

impl DependencyGraph {
    fn new(document: &Document, formulas: &FormulaNodes<'_>) -> Self {
        let mut graph = Self::default();
        for (entity_id, entity) in &document.entities {
            graph
                .entity_schemas
                .insert(entity_id.clone(), entity.schema.clone());
        }
        for (formula, expression) in formulas {
            let references = direct_references(formula, expression);
            let columns = extract_column_totals(expression);
            graph.insert_edges(document, formula, references, columns);
        }
        graph
    }

    fn insert_edges(
        &mut self,
        document: &Document,
        formula: &FieldRef,
        references: BTreeSet<FieldRef>,
        columns: BTreeSet<ColumnRef>,
    ) {
        for reference in &references {
            self.cell_readers
                .entry(reference.clone())
                .or_default()
                .insert(formula.clone());
        }
        for column in &columns {
            self.column_readers
                .entry(column.clone())
                .or_default()
                .insert(formula.clone());
            self.ensure_column_slots(document, column);
        }
        self.reference_uses.insert(formula.clone(), references);
        self.column_uses.insert(formula.clone(), columns);
    }

    fn ensure_column_slots(&mut self, document: &Document, column: &ColumnRef) {
        if self.column_slots.contains_key(column) {
            return;
        }
        let slots = document
            .entities
            .iter()
            .filter(|(_, entity)| entity.schema == column.schema)
            .map(|(entity_id, _)| FieldRef::new(entity_id.clone(), column.field.clone()))
            .collect::<BTreeSet<_>>();
        for slot in &slots {
            self.slot_columns.insert(slot.clone(), column.clone());
        }
        self.column_slots.insert(column.clone(), slots);
    }

    /// Replace one formula's outgoing edges from the current document.
    fn replace_formula(&mut self, document: &Document, root: &FieldRef) {
        if let Some(references) = self.reference_uses.remove(root) {
            for reference in references {
                let empty = self
                    .cell_readers
                    .get_mut(&reference)
                    .is_some_and(|readers| {
                        readers.remove(root);
                        readers.is_empty()
                    });
                if empty {
                    self.cell_readers.remove(&reference);
                }
            }
        }
        if let Some(columns) = self.column_uses.remove(root) {
            for column in columns {
                let empty = self.column_readers.get_mut(&column).is_some_and(|readers| {
                    readers.remove(root);
                    readers.is_empty()
                });
                if empty {
                    self.column_readers.remove(&column);
                    if let Some(slots) = self.column_slots.remove(&column) {
                        for slot in slots {
                            self.slot_columns.remove(&slot);
                        }
                    }
                }
            }
        }
        if let Some((references, columns)) = formula_uses(document, root) {
            self.insert_edges(document, root, references, columns);
        }
    }

    /// Recompute the member slots of every column the document's row set may
    /// have changed.
    fn refresh_column_slots(&mut self, document: &Document, columns: &BTreeSet<ColumnRef>) {
        // Clear every column before refilling any: a row moved between two of
        // them must not lose its new slot when its old column is cleared.
        for column in columns {
            if let Some(previous) = self.column_slots.remove(column) {
                for slot in previous {
                    self.slot_columns.remove(&slot);
                }
            }
        }
        for column in columns {
            if self.column_readers.contains_key(column) {
                self.ensure_column_slots(document, column);
            }
        }
    }

    /// Remember the current schema of a root's row so a later removal of that
    /// row can still refresh its columns' slots.
    fn remember_entity(&mut self, document: &Document, entity: &tachiko_semantic_core::EntityId) {
        if let Some(entity) = document.entities.get(entity) {
            self.entity_schemas
                .insert(entity.id.clone(), entity.schema.clone());
        }
    }

    fn forget_entity(&mut self, entity: &tachiko_semantic_core::EntityId) {
        self.entity_schemas.remove(entity);
    }

    /// The schemas a root's row belongs to on either side of this update: the
    /// retained one and the current one differ when the row moved, and it may
    /// exist on only one side when it was inserted or removed.
    fn schemas_of(
        &self,
        document: &Document,
        entity: &tachiko_semantic_core::EntityId,
    ) -> impl Iterator<Item = tachiko_semantic_core::SchemaId> {
        let retained = self.entity_schemas.get(entity).cloned();
        let current = document
            .entities
            .get(entity)
            .map(|entity| entity.schema.clone());
        retained.into_iter().chain(current)
    }

    fn formulas(&self) -> impl Iterator<Item = &FieldRef> {
        self.reference_uses.keys()
    }

    fn tracked_columns(&self) -> impl Iterator<Item = &ColumnRef> {
        self.column_slots.keys()
    }

    fn column_readers<'a>(&'a self, column: &ColumnRef) -> impl Iterator<Item = &'a FieldRef> + 'a {
        self.column_readers.get(column).into_iter().flatten()
    }

    fn columns_of<'a>(&'a self, formula: &FieldRef) -> impl Iterator<Item = &'a ColumnRef> + 'a {
        self.column_uses.get(formula).into_iter().flatten()
    }

    fn expanded_dependencies(&self) -> BTreeMap<FieldRef, BTreeSet<FieldRef>> {
        self.reference_uses
            .keys()
            .map(|formula| {
                (
                    formula.clone(),
                    self.dependencies_of(formula)
                        .expect("every tracked formula has an edge entry"),
                )
            })
            .collect()
    }

    fn dependencies_of(&self, field: &FieldRef) -> Option<BTreeSet<FieldRef>> {
        let references = self.reference_uses.get(field)?;
        let mut dependencies = references.clone();
        for column in self.column_uses.get(field).into_iter().flatten() {
            if let Some(slots) = self.column_slots.get(column) {
                dependencies.extend(slots.iter().cloned());
            }
        }
        Some(dependencies)
    }

    /// Formulas whose expanded dependencies contain `field`: the ones naming
    /// it directly plus the readers of the column it is a slot of.
    fn direct_dependents(&self, field: &FieldRef) -> BTreeSet<FieldRef> {
        let mut dependents = self.cell_readers.get(field).cloned().unwrap_or_default();
        if let Some(column) = self.slot_columns.get(field) {
            dependents.extend(self.column_readers(column).cloned());
        }
        dependents
    }

    /// The formula dependencies of `formula` that are members of `within`.
    fn formula_dependencies_within(
        &self,
        formula: &FieldRef,
        within: &BTreeSet<FieldRef>,
    ) -> BTreeSet<FieldRef> {
        let mut dependencies: BTreeSet<FieldRef> = self
            .reference_uses
            .get(formula)
            .map(|references| references.intersection(within).cloned().collect())
            .unwrap_or_default();
        for column in self.column_uses.get(formula).into_iter().flatten() {
            for slot in self.column_slots.get(column).into_iter().flatten() {
                if within.contains(slot) {
                    dependencies.insert(slot.clone());
                }
            }
        }
        dependencies
    }

    fn reverse_closure(
        &self,
        roots: &BTreeSet<FieldRef>,
        include_roots: bool,
    ) -> (BTreeSet<FieldRef>, usize) {
        let mut queue = roots.iter().cloned().collect::<VecDeque<_>>();
        let mut affected = BTreeSet::new();
        let mut traversed = 0_usize;
        while let Some(field) = queue.pop_front() {
            for dependent in self.direct_dependents(&field) {
                traversed = traversed.saturating_add(1);
                if (!include_roots && roots.contains(&dependent))
                    || !affected.insert(dependent.clone())
                {
                    continue;
                }
                queue.push_back(dependent);
            }
        }
        (affected, traversed)
    }

    fn affected_by(&self, changed: &FieldRef) -> Vec<FieldRef> {
        let roots = BTreeSet::from([changed.clone()]);
        self.reverse_closure(&roots, true).0.into_iter().collect()
    }

    fn reaching_fields<'a>(
        &self,
        changed: impl IntoIterator<Item = &'a FieldRef>,
        targets: &BTreeSet<FieldRef>,
    ) -> BTreeMap<FieldRef, BTreeSet<FieldRef>> {
        let mut reaching = BTreeMap::<FieldRef, BTreeSet<FieldRef>>::new();
        for changed in changed {
            let roots = BTreeSet::from([changed.clone()]);
            for formula in self.reverse_closure(&roots, true).0 {
                if targets.contains(&formula) {
                    reaching.entry(formula).or_default().insert(changed.clone());
                }
            }
        }
        reaching
    }

    fn entry_count(&self) -> usize {
        self.reference_uses.len()
            + self.column_uses.len()
            + self.cell_readers.values().map(BTreeSet::len).sum::<usize>()
            + self
                .column_readers
                .values()
                .map(BTreeSet::len)
                .sum::<usize>()
            + self.column_slots.values().map(BTreeSet::len).sum::<usize>()
            + self.slot_columns.len()
            + self.entity_schemas.len()
    }
}

fn pregraph_failures(
    document: &Document,
    nodes: &ValueNodes<'_>,
    formulas: &FormulaNodes<'_>,
) -> FailureMap {
    let mut failures = BTreeMap::new();
    // Phase 1a: structural failures take precedence over every later phase.
    for (formula, expression) in formulas {
        if let Err(error) = validate_expression_structure(expression) {
            failures.insert(
                formula.clone(),
                CalculationFailure::InvalidExpression { error },
            );
        } else if expression.expression_type() != Some(FormulaExpressionType::Number) {
            failures.insert(formula.clone(), CalculationFailure::TypeMismatch);
        }
    }

    // Phase 1b: a column identity that does not resolve to a numeric schema
    // field fails the naming formula before any row-level target check.
    for (formula, expression) in formulas {
        if failures.contains_key(formula) {
            continue;
        }
        let columns = column_failures(document, expression);
        if !columns.is_empty() {
            failures.insert(
                formula.clone(),
                CalculationFailure::InvalidColumn { columns },
            );
        }
    }

    // Phase 1c: capture every directly discovered missing/stale/type target.
    // A column total re-checks the same cells for every formula that reads the
    // column, so resolve one column's cell failures once for the whole phase.
    let mut column_targets: BTreeMap<ColumnRef, Vec<(FieldRef, Option<ReferenceFailure>)>> =
        BTreeMap::new();
    for (field, value) in nodes {
        if failures.contains_key(field) {
            continue;
        }
        let mut invalid_targets = BTreeMap::new();
        if let Some(failure) = reference_failure(document, field, true) {
            invalid_targets.insert(field.clone(), failure);
        }
        if let Value::Formula(expression) = value {
            let unconditional = expression.unconditional_references(field);
            let mut checks = expression
                .reference_uses(field)
                .into_iter()
                .map(|(dependency, numeric)| {
                    // A value missing only inside an IF branch is decided at
                    // evaluation, when the branch is actually selected; the
                    // reference itself must still be well-formed.
                    let failure = if unconditional.contains(&dependency) {
                        reference_failure(document, &dependency, numeric)
                    } else {
                        structural_reference_failure(document, &dependency, numeric)
                    };
                    (dependency, failure)
                })
                .collect::<Vec<_>>();
            // A column total reads every non-blank cell of its column; a blank
            // cell is skipped by the sum, not a missing target. A present cell
            // must hold a Number wherever the total sits.
            let mut pending = vec![expression];
            while let Some(node) = pending.pop() {
                if let Expression::ColumnTotal(column) = node {
                    let targets = column_targets.entry(column.clone()).or_insert_with(|| {
                        column_cells(document, column)
                            .map(|cell| {
                                let failure = reference_failure(document, &cell, true);
                                (cell, failure)
                            })
                            .collect::<Vec<_>>()
                    });
                    checks.extend(targets.iter().cloned());
                }
                pending.extend(node.children());
            }
            for (dependency, failure) in checks {
                if let Some(failure) = failure {
                    invalid_targets.insert(dependency, failure);
                }
            }
        }
        if !invalid_targets.is_empty() {
            failures.insert(
                field.clone(),
                CalculationFailure::InvalidReferences {
                    targets: invalid_targets,
                },
            );
        }
    }
    failures
}

fn assign_cycle_failures(
    formulas: &FormulaNodes<'_>,
    graph: &DependencyGraph,
    failures: &mut FailureMap,
) {
    // Phase 2: semantic cycles are complete SCCs in the graph induced by
    // formulas without an earlier primary failure.
    let eligible_formulas = formulas
        .keys()
        .filter(|formula| !failures.contains_key(*formula))
        .cloned()
        .collect::<BTreeSet<_>>();
    for members in cyclic_components(&eligible_formulas, graph) {
        let members = Arc::new(members);
        for member in members.iter() {
            failures.insert(
                member.clone(),
                CalculationFailure::Cycle {
                    members: Arc::clone(&members),
                },
            );
        }
    }
}

fn initial_values(nodes: &ValueNodes<'_>, failures: &FailureMap) -> BTreeMap<FieldRef, Number> {
    // Valid stored numbers are the initial ready values. Failed stored nodes
    // remain primary failures but never become partial published state.
    nodes
        .iter()
        .filter_map(|(field, value)| match value {
            Value::Number(number) if !failures.contains_key(field) => {
                Some((field.clone(), *number))
            }
            _ => None,
        })
        .collect()
}

/// How many unresolved subjects each remaining formula and read column awaits.
///
/// A column's count is the slots that still have to resolve, computed once per
/// column; a formula counts its remaining cell references plus, once each, its
/// columns with a remaining slot.
fn remaining_counts(
    graph: &DependencyGraph,
    remaining: &BTreeSet<FieldRef>,
) -> (BTreeMap<FieldRef, usize>, BTreeMap<ColumnRef, usize>) {
    let mut column_counts = BTreeMap::<ColumnRef, usize>::new();
    for formula in remaining {
        for column in graph.columns_of(formula) {
            column_counts.entry(column.clone()).or_insert_with(|| {
                graph.column_slots.get(column).map_or(0, |slots| {
                    slots
                        .iter()
                        .filter(|slot| remaining.contains(*slot))
                        .count()
                })
            });
        }
    }
    let dependency_counts = remaining
        .iter()
        .map(|formula| {
            let cells = graph.reference_uses.get(formula).map_or(0, |references| {
                references
                    .iter()
                    .filter(|reference| remaining.contains(*reference))
                    .count()
            });
            let columns = graph.column_uses.get(formula).map_or(0, |columns| {
                columns
                    .iter()
                    .filter(|column| column_counts.get(*column).copied().unwrap_or(0) > 0)
                    .count()
            });
            (formula.clone(), cells + columns)
        })
        .collect::<BTreeMap<_, _>>();
    (dependency_counts, column_counts)
}

fn evaluate_remaining_formulas(
    document: &Document,
    formulas: &FormulaNodes<'_>,
    graph: &DependencyGraph,
    failures: &mut FailureMap,
    values: &mut BTreeMap<FieldRef, Number>,
) {
    // Phases 3 and 4: process the remaining acyclic graph dependency-first.
    // Edges to already-failed nodes are ready and become direct dependency
    // subjects. Each formula awaits the cells it names directly plus, once
    // each, the columns it reads: a cell named both ways has two paths, each
    // counted and released on its own.
    let remaining_formulas = formulas
        .keys()
        .filter(|formula| !failures.contains_key(*formula))
        .cloned()
        .collect::<BTreeSet<_>>();

    let (mut remaining_dependency_counts, mut remaining_column_counts) =
        remaining_counts(graph, &remaining_formulas);
    let mut ready = remaining_dependency_counts
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(formula, _)| formula.clone())
        .collect::<BTreeSet<_>>();

    // One read of each column per pass, shared by every formula that names it
    // and dropped with this call. All of a column's formula members resolved
    // before its first reader became ready, so no entry can go stale.
    let mut columns = evaluation::ColumnCache::default();
    while let Some(formula) = ready.pop_first() {
        remaining_dependency_counts.remove(&formula);
        let expression = formulas[&formula];
        for column in extract_column_totals(expression) {
            columns.fill(document, values, failures, &column);
        }
        let evaluation = evaluation::Evaluation {
            document,
            owner: &formula,
            values,
            columns: &columns,
        };
        let failed_dependencies = evaluation.failed_required_references(expression, failures);
        if failed_dependencies.is_empty() {
            match evaluation.number(expression) {
                Ok(value) => {
                    values.insert(formula.clone(), value);
                }
                Err(failure) => {
                    failures.insert(formula.clone(), failure);
                }
            }
        } else {
            failures.insert(
                formula.clone(),
                CalculationFailure::FailedDependencies {
                    dependencies: failed_dependencies,
                },
            );
        }

        // Release this cell's direct readers, then the column it is a slot of.
        // A column releases its readers only once every slot has resolved.
        if let Some(readers) = graph.cell_readers.get(&formula) {
            for reader in readers {
                if let Some(count) = remaining_dependency_counts.get_mut(reader) {
                    *count -= 1;
                    if *count == 0 {
                        ready.insert(reader.clone());
                    }
                }
            }
        }
        if let Some(column) = graph.slot_columns.get(&formula) {
            if let Some(count) = remaining_column_counts.get_mut(column) {
                *count -= 1;
                if *count == 0 {
                    for reader in graph.column_readers(column) {
                        if let Some(count) = remaining_dependency_counts.get_mut(reader) {
                            *count -= 1;
                            if *count == 0 {
                                ready.insert(reader.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    assert!(
        remaining_dependency_counts.is_empty(),
        "SCC condensation must be acyclic"
    );
}

fn column_failures(
    document: &Document,
    expression: &Expression,
) -> BTreeMap<ColumnRef, ColumnFailure> {
    let mut failures = BTreeMap::new();
    let value_columns = extract_value_columns(expression);
    for column in extract_column_totals(expression) {
        let needs_number = value_columns.contains(&column);
        let failure = match document.schemas.get(&column.schema) {
            None => Some(ColumnFailure::MissingSchema),
            Some(schema) => match schema.fields.get(&column.field) {
                None => Some(ColumnFailure::MissingField),
                Some(definition)
                    if (needs_number && definition.field_type != FieldType::Number)
                        || matches!(definition.field_type, FieldType::Reference { .. }) =>
                {
                    Some(ColumnFailure::NonNumeric)
                }
                Some(_) => None,
            },
        };
        if let Some(failure) = failure {
            failures.insert(column, failure);
        }
    }
    failures
}

/// The non-blank cells a column total sums, in entity order.
pub(crate) fn column_cells<'a>(
    document: &'a Document,
    column: &'a ColumnRef,
) -> impl Iterator<Item = FieldRef> + 'a {
    #[cfg(test)]
    column_walk_counter::record();
    document
        .entities
        .iter()
        .filter(|(_, entity)| {
            entity.schema == column.schema && entity.fields.contains_key(&column.field)
        })
        .map(|(entity_id, _)| FieldRef::new(entity_id.clone(), column.field.clone()))
}

/// The reference names an existing entity and field of the right type,
/// whatever value it holds now.
fn structural_reference_failure(
    document: &Document,
    field: &FieldRef,
    numeric: bool,
) -> Option<ReferenceFailure> {
    let Some(entity) = document.entities.get(&field.entity) else {
        return Some(ReferenceFailure::Missing);
    };
    let Some(definition) = document
        .schemas
        .get(&entity.schema)
        .and_then(|schema| schema.fields.get(&field.field))
    else {
        return Some(ReferenceFailure::Missing);
    };
    (numeric && definition.field_type != FieldType::Number).then_some(ReferenceFailure::NonNumeric)
}

fn reference_failure(
    document: &Document,
    field: &FieldRef,
    numeric: bool,
) -> Option<ReferenceFailure> {
    if let Some(failure) = structural_reference_failure(document, field, numeric) {
        return Some(failure);
    }
    if !numeric {
        return None;
    }
    let entity = &document.entities[&field.entity];
    let definition = &document.schemas[&entity.schema].fields[&field.field];
    match entity.fields.get(&field.field) {
        Some(Value::Number(_) | Value::Formula(_)) => None,
        Some(Value::Text(_) | Value::Boolean(_) | Value::Date(_) | Value::Reference(_)) => {
            Some(ReferenceFailure::NonNumeric)
        }
        None if !definition.required => None,
        None => Some(ReferenceFailure::Missing),
    }
}

fn depth_first_finish_order(adjacency: &[Vec<usize>]) -> Vec<usize> {
    let mut visited = vec![false; adjacency.len()];
    let mut finish_order = Vec::new();
    for root in 0..adjacency.len() {
        if visited[root] {
            continue;
        }
        visited[root] = true;
        let mut stack = vec![(root, 0_usize)];
        while !stack.is_empty() {
            let next = {
                let (node, next_index) = stack.last_mut().expect("DFS stack is non-empty");
                if *next_index < adjacency[*node].len() {
                    let target = adjacency[*node][*next_index];
                    *next_index += 1;
                    Some(target)
                } else {
                    None
                }
            };
            if let Some(target) = next {
                if !visited[target] {
                    visited[target] = true;
                    stack.push((target, 0));
                }
            } else {
                let (node, _) = stack.pop().expect("DFS stack is non-empty");
                finish_order.push(node);
            }
        }
    }
    finish_order
}

fn cyclic_components(
    eligible: &BTreeSet<FieldRef>,
    graph: &DependencyGraph,
) -> Vec<BTreeSet<FieldRef>> {
    // Walk a bipartite view of the graph: formula cells and column nodes.
    // Formula -> column -> each formula slot of that column gives exactly the
    // expanded formula edges without materializing one edge per row.
    let mut nodes = eligible
        .iter()
        .cloned()
        .map(GraphNode::Cell)
        .collect::<Vec<_>>();
    let formula_count = nodes.len();
    let mut columns = BTreeSet::new();
    for formula in eligible {
        columns.extend(graph.columns_of(formula).cloned());
    }
    nodes.extend(columns.iter().cloned().map(GraphNode::Column));
    let mut index = BTreeMap::<GraphNode, usize>::new();
    for (position, node) in nodes.iter().enumerate() {
        index.insert(node.clone(), position);
    }

    let mut adjacency = vec![Vec::<usize>::new(); nodes.len()];
    let mut reverse = vec![Vec::<usize>::new(); nodes.len()];
    let mut connect = |source: usize, target: usize| {
        adjacency[source].push(target);
        reverse[target].push(source);
    };
    for (source, formula) in eligible.iter().enumerate() {
        for reference in graph.reference_uses.get(formula).into_iter().flatten() {
            if eligible.contains(reference) {
                let target = index[&GraphNode::Cell(reference.clone())];
                connect(source, target);
            }
        }
        for column in graph.columns_of(formula) {
            let target = index[&GraphNode::Column(column.clone())];
            connect(source, target);
        }
    }
    for column in &columns {
        let source = index[&GraphNode::Column(column.clone())];
        for slot in graph.column_slots.get(column).into_iter().flatten() {
            if eligible.contains(slot) {
                let target = index[&GraphNode::Cell(slot.clone())];
                connect(source, target);
            }
        }
    }

    let finish_order = depth_first_finish_order(&adjacency);

    let mut assigned = vec![false; nodes.len()];
    let mut cycles = Vec::new();
    for root in finish_order.into_iter().rev() {
        if assigned[root] {
            continue;
        }
        assigned[root] = true;
        let mut component = Vec::new();
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            component.push(node);
            for dependent in &reverse[node] {
                if !assigned[*dependent] {
                    assigned[*dependent] = true;
                    stack.push(*dependent);
                }
            }
        }
        let members = component
            .iter()
            .filter(|position| **position < formula_count)
            .map(|position| {
                let GraphNode::Cell(formula) = &nodes[*position] else {
                    unreachable!("formula positions hold cells")
                };
                formula.clone()
            })
            .collect::<BTreeSet<_>>();
        let cyclic = component.len() > 1
            || component
                .first()
                .is_some_and(|position| *position < formula_count)
                && members.first().is_some_and(|member| {
                    graph
                        .reference_uses
                        .get(member)
                        .is_some_and(|targets| targets.contains(member))
                });
        if cyclic {
            cycles.push(members);
        }
    }
    cycles
}

/// One node of the bipartite cycle graph. A column node never appears in any
/// output; it only links a formula to the formula slots of the column it reads.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum GraphNode {
    Cell(FieldRef),
    Column(ColumnRef),
}

/// Counts [`column_cells`] calls so a test can bound one calculation's walks.
#[cfg(test)]
mod column_walk_counter {
    use std::cell::Cell;

    thread_local! {
        static WALKS: Cell<usize> = const { Cell::new(0) };
    }

    pub(crate) fn record() {
        WALKS.with(|walks| walks.set(walks.get() + 1));
    }

    pub(crate) fn take() -> usize {
        WALKS.with(|walks| walks.replace(0))
    }
}
