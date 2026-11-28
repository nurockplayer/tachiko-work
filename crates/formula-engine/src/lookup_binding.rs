use std::collections::BTreeSet;

use super::{
    AddressIndex, ColumnAddress, ColumnRef, ConditionalAggregate, Document, Expression, FieldRef,
    FieldType, FormulaBindError, FormulaExpressionType, UnboundExpression, bind_column_reference,
    bind_criterion_column, bind_criterion_value, bind_node, field_type,
};

fn lookup_key_type<'a>(
    document: &'a Document,
    owner: &FieldRef,
    key: &Expression,
) -> Option<&'a FieldType> {
    let (entity, field) = match key {
        Expression::Reference(reference) | Expression::AnchoredReference(reference) => {
            (&reference.entity, &reference.field)
        }
        Expression::SameRowReference(field) | Expression::AnchoredSameRowReference(field) => {
            (&owner.entity, field)
        }
        _ => {
            return (key.expression_type() == Some(FormulaExpressionType::Number))
                .then_some(&FieldType::Number);
        }
    };
    let entity = document.entities.get(entity)?;
    Some(
        &document
            .schemas
            .get(&entity.schema)?
            .fields
            .get(field)?
            .field_type,
    )
}

pub(super) fn bind_lookup(
    document: &Document,
    index: &AddressIndex,
    owner: &FieldRef,
    key: &UnboundExpression,
    (key_column, result_column, if_missing): (
        &ColumnAddress,
        &ColumnAddress,
        Option<&UnboundExpression>,
    ),
) -> Result<Expression, FormulaBindError> {
    let auto = field_type(document, owner) == Some(&FieldType::Auto);
    let key_column = bind_criterion_column(document, index, key_column)?;
    let result_column = bind_column_reference(document, index, result_column, auto)?;
    let key = bind_criterion_value(document, index, owner, key)?;
    if key_column.schema != result_column.schema
        || !lookup_key_matches(document, owner, &key, &key_column)
    {
        return Err(FormulaBindError::TypeMismatch {
            message: "LOOKUP requires columns in one schema and a key of the key column's type"
                .into(),
        });
    }
    let if_missing = if_missing
        .map(|value| bind_node(document, index, owner, value).map(Box::new))
        .transpose()?;
    Ok(Expression::Lookup {
        key: Box::new(key),
        key_column,
        result_column,
        if_missing,
    })
}

pub(super) fn lookup_key_matches(
    document: &Document,
    owner: &FieldRef,
    key: &Expression,
    column: &ColumnRef,
) -> bool {
    if matches!(key, Expression::DeletedReference {}) {
        return true;
    }
    let column_type = column_type(document, column);
    let key_type = lookup_key_type(document, owner, key);
    key_type == column_type
        || (field_type(document, owner) == Some(&FieldType::Auto)
            && column_type == Some(&FieldType::Auto)
            && matches!(
                key_type,
                Some(FieldType::Number | FieldType::Text | FieldType::Date | FieldType::Boolean)
            ))
}

fn column_type<'a>(document: &'a Document, column: &ColumnRef) -> Option<&'a FieldType> {
    Some(
        &document
            .schemas
            .get(&column.schema)?
            .fields
            .get(&column.field)?
            .field_type,
    )
}

/// Lookups in Auto cells can read an Auto result column. Other numeric
/// column operators still require a declared Number column.
pub(super) fn extract_value_columns(
    document: &Document,
    owner: &FieldRef,
    expression: &Expression,
) -> BTreeSet<ColumnRef> {
    let auto = field_type(document, owner) == Some(&FieldType::Auto);
    let mut columns = BTreeSet::new();
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        columns.extend(required_number_column(document, node, auto).cloned());
        pending.extend(node.children());
    }
    columns
}

fn required_number_column<'a>(
    document: &Document,
    node: &'a Expression,
    auto: bool,
) -> Option<&'a ColumnRef> {
    match node {
        Expression::ColumnTotal(column) | Expression::ColumnAggregate { column, .. } => {
            Some(column)
        }
        Expression::Lookup { result_column, .. }
            if !(auto && column_type(document, result_column) == Some(&FieldType::Auto)) =>
        {
            Some(result_column)
        }
        Expression::ColumnConditional { aggregate, .. } => match aggregate.as_ref() {
            ConditionalAggregate::Sum(column) | ConditionalAggregate::Average(column) => {
                Some(column)
            }
            ConditionalAggregate::Count => None,
        },
        _ => None,
    }
}
