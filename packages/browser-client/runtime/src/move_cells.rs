//! Snapshot planning for one atomic Cut → Paste publication.
use std::collections::{BTreeMap, BTreeSet};

use super::{
    CellMove, CollectionSpec, ColumnSpec, DesignerError, DesignerRuntime, EntityId, Expression,
    FieldRef, ScalarEditInput, SemanticCommand, Value, cell_edit_commands, cell_edit_value,
    require_copy_target_number, tracker_error,
};

struct OriginalCell<'a> {
    column: &'a ColumnSpec,
    native_table_rows: bool,
    value: Option<&'a Value>,
}

pub(super) fn plan(
    runtime: &DesignerRuntime,
    moves: &[CellMove],
) -> Result<(Vec<SemanticCommand>, Vec<SemanticCommand>), DesignerError> {
    let mut sources = BTreeSet::new();
    let mut destinations = BTreeSet::new();
    for movement in moves {
        if !sources.insert(movement.from.as_field_ref())
            || !destinations.insert(movement.to.as_field_ref())
        {
            return Err(tracker_error(
                "duplicate move sources or destinations are unsupported",
            ));
        }
    }
    let targets = sources
        .union(&destinations)
        .cloned()
        .collect::<BTreeSet<_>>();
    if targets.is_empty() || targets.len() > runtime.max_batch_targets() {
        return Err(tracker_error("the edit range is empty or too large"));
    }
    let originals = capture_originals(runtime, &targets)?;
    let mut values = BTreeMap::new();
    for movement in moves {
        let source = movement.from.as_field_ref();
        let target = movement.to.as_field_ref();
        let mut value = originals[&source].value.cloned();
        if source.entity != target.entity {
            if let Some(Value::Formula(expression)) = &mut value {
                preserve_source_row(expression, &source.entity);
            }
        }
        values.insert(target, value);
    }
    for source in sources.difference(&destinations) {
        values.insert(source.clone(), None);
    }
    let mut forward = Vec::new();
    let mut inverse = Vec::new();
    for (field, value) in values {
        let original = &originals[&field];
        admit_value(runtime, &field, original, value.as_ref())?;
        if value.as_ref() != original.value {
            forward.push(transition(&field, original.value, value.as_ref()));
            inverse.push(transition(&field, value.as_ref(), original.value));
        }
    }
    inverse.reverse();
    Ok((forward, inverse))
}

fn capture_originals<'a>(
    runtime: &'a DesignerRuntime,
    targets: &BTreeSet<FieldRef>,
) -> Result<BTreeMap<FieldRef, OriginalCell<'a>>, DesignerError> {
    let mut specs: BTreeMap<&EntityId, &CollectionSpec> = BTreeMap::new();
    for spec in runtime.collection_specs.values() {
        for entity in &spec.entities {
            specs.entry(entity).or_insert(spec);
        }
    }
    let document = runtime.session.document();
    targets
        .iter()
        .map(|field| {
            let unsupported = || DesignerError::UnsupportedScalarEdit {
                field: field.clone(),
            };
            let spec = specs.get(&field.entity).ok_or_else(unsupported)?;
            let column = spec
                .columns
                .iter()
                .find(|column| column.id == field.field)
                .ok_or_else(unsupported)?;
            let value = document
                .entities
                .get(&field.entity)
                .ok_or_else(unsupported)?
                .fields
                .get(&field.field);
            Ok((
                field.clone(),
                OriginalCell {
                    column,
                    native_table_rows: spec.native_table_rows,
                    value,
                },
            ))
        })
        .collect()
}

fn admit_value(
    runtime: &DesignerRuntime,
    field: &FieldRef,
    original: &OriginalCell<'_>,
    value: Option<&Value>,
) -> Result<(), DesignerError> {
    if (original.value.is_none() || value.is_none()) && !original.native_table_rows {
        return Err(DesignerError::UnsupportedScalarEdit {
            field: field.clone(),
        });
    }
    let input = match value {
        // Formula installation uses the same Number target check as copy. The
        // restore command and candidate validation prove the complete result.
        Some(Value::Formula(_)) => {
            return require_copy_target_number(runtime.session.document(), field);
        }
        Some(Value::Number(value)) => ScalarEditInput::Number {
            input: value.to_string(),
        },
        Some(Value::Text(value)) => ScalarEditInput::Text {
            value: value.clone(),
        },
        Some(Value::Boolean(value)) => ScalarEditInput::Boolean { value: *value },
        Some(Value::Date(value)) => ScalarEditInput::Date {
            value: value.to_string(),
        },
        Some(Value::Reference(_)) => {
            return Err(DesignerError::UnsupportedScalarEdit {
                field: field.clone(),
            });
        }
        None => ScalarEditInput::Clear,
    };
    cell_edit_value(original.column, original.value, &input, field)?;
    Ok(())
}

fn transition(field: &FieldRef, old: Option<&Value>, value: Option<&Value>) -> SemanticCommand {
    if let (None, Some(Value::Formula(expression))) = (old, value) {
        SemanticCommand::formula_restore(field.clone(), expression.clone())
    } else {
        cell_edit_commands(field, old, value).0
    }
}

fn preserve_source_row(expression: &mut Expression, source: &EntityId) {
    match expression {
        Expression::SameRowReference(field) => {
            *expression = Expression::Reference(FieldRef::new(source.clone(), field.clone()));
        }
        Expression::AnchoredSameRowReference(field) => {
            *expression =
                Expression::AnchoredReference(FieldRef::new(source.clone(), field.clone()));
        }
        _ => {
            for child in expression.children_mut() {
                preserve_source_row(child, source);
            }
        }
    }
}
