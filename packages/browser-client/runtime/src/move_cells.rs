//! Snapshot planning for one atomic Cut → Paste publication.
use std::collections::{BTreeMap, BTreeSet};

use super::{
    CellMove, CollectionSpec, ColumnSpec, DesignerError, DesignerRuntime, EntityId, Expression,
    FieldRef, FieldType, HistoryAction, HistoryEntry, PublicationProjection, ScalarEditInput,
    SemanticCommand, Value,
    auto_input::{CellDisplay, cell_style},
    cell_edit_commands, cell_edit_value, require_copy_target_number, tracker_error,
};

struct OriginalCell<'a> {
    column: &'a ColumnSpec,
    native_table_rows: bool,
    value: Option<&'a Value>,
}

type CellMap = BTreeMap<FieldRef, FieldRef>;

#[derive(Default)]
struct Edit {
    commands: Vec<SemanticCommand>,
    styles: Vec<CellDisplay>,
}

impl From<Edit> for HistoryAction {
    fn from(edit: Edit) -> Self {
        Self::InputCommands {
            commands: edit.commands,
            styles: edit.styles,
        }
    }
}

#[derive(Default)]
pub(super) struct MovePlan {
    forward: Edit,
    inverse: Edit,
}

impl MovePlan {
    fn value(&mut self, field: &FieldRef, old: Option<&Value>, new: Option<&Value>) {
        if old != new {
            self.forward.commands.push(transition(field, old, new));
            self.inverse.commands.push(transition(field, new, old));
        }
    }

    fn style(&mut self, runtime: &DesignerRuntime, field: &FieldRef, source: Option<&FieldRef>) {
        let old = cell_style(&runtime.presentation, field);
        let new = source.and_then(|source| cell_style(&runtime.presentation, source));
        if old != new {
            self.forward.styles.push(CellDisplay {
                field: field.clone(),
                style: new,
            });
            self.inverse.styles.push(CellDisplay {
                field: field.clone(),
                style: old,
            });
        }
    }

    pub(super) fn publish(
        self,
        runtime: &mut DesignerRuntime,
        expected: &str,
    ) -> Result<PublicationProjection, DesignerError> {
        if self.forward.styles.is_empty() {
            return runtime.record_edit(expected, self.forward.commands, self.inverse.commands);
        }
        let publication = runtime.publish_input_commands(
            expected,
            &self.forward.commands,
            &self.forward.styles,
        )?;
        runtime.record_history_entry(HistoryEntry::new(self.forward.into(), self.inverse.into()));
        Ok(publication)
    }
}

pub(super) fn plan(
    runtime: &DesignerRuntime,
    moves: &[CellMove],
) -> Result<MovePlan, DesignerError> {
    let (by_source, by_destination) = move_maps(moves)?;
    let targets = by_source
        .keys()
        .chain(by_destination.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    if targets.is_empty() || targets.len() > runtime.max_batch_targets() {
        return Err(tracker_error("the edit range is empty or too large"));
    }
    let originals = capture_originals(runtime, &targets)?;
    let mut plan = MovePlan::default();
    for (field, original) in &originals {
        let source = by_destination.get(field);
        let value = source.and_then(|source| {
            let mut value = originals[source].value.cloned()?;
            if let Value::Formula(expression) = &mut value {
                retarget(expression, &source.entity, &field.entity, &by_source);
            }
            Some(value)
        });
        admit_value(runtime, field, original, value.as_ref())?;
        plan.value(field, original.value, value.as_ref());
        plan.style(runtime, field, source);
    }
    plan_inbound(runtime, &by_source, &targets, &mut plan);
    plan.inverse.commands.reverse();
    Ok(plan)
}

fn move_maps(moves: &[CellMove]) -> Result<(CellMap, CellMap), DesignerError> {
    let mut by_source = BTreeMap::new();
    let mut by_destination = BTreeMap::new();
    for movement in moves {
        let source = movement.from.as_field_ref();
        let destination = movement.to.as_field_ref();
        if by_source
            .insert(source.clone(), destination.clone())
            .is_some()
            || by_destination.insert(destination, source).is_some()
        {
            return Err(tracker_error(
                "duplicate move sources or destinations are unsupported",
            ));
        }
    }
    Ok((by_source, by_destination))
}

fn plan_inbound(
    runtime: &DesignerRuntime,
    moves: &CellMap,
    targets: &BTreeSet<FieldRef>,
    plan: &mut MovePlan,
) {
    let inbound = moves
        .iter()
        .filter(|(source, destination)| source != destination)
        .flat_map(|(source, _)| runtime.session.direct_field_dependents(source))
        .collect::<BTreeSet<_>>();
    let document = runtime.session.document();
    for field in inbound.difference(targets) {
        let old = &document.entities[&field.entity].fields[&field.field];
        let Value::Formula(expression) = old else {
            unreachable!("a direct formula dependent contains a formula");
        };
        let mut expression = expression.clone();
        retarget(&mut expression, &field.entity, &field.entity, moves);
        plan.value(field, Some(old), Some(&Value::Formula(expression)));
    }
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
        Some(Value::Formula(_)) => {
            return admit_formula(runtime, field, &original.column.field_type);
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

fn admit_formula(
    runtime: &DesignerRuntime,
    field: &FieldRef,
    field_type: &FieldType,
) -> Result<(), DesignerError> {
    match field_type {
        FieldType::Auto | FieldType::Date => Ok(()),
        _ => require_copy_target_number(runtime.session.document(), field),
    }
}

fn transition(field: &FieldRef, old: Option<&Value>, value: Option<&Value>) -> SemanticCommand {
    if let (None, Some(Value::Formula(expression))) = (old, value) {
        SemanticCommand::formula_restore(field.clone(), expression.clone())
    } else {
        cell_edit_commands(field, old, value).0
    }
}

fn retarget(
    expression: &mut Expression,
    source: &EntityId,
    destination: &EntityId,
    moves: &CellMap,
) {
    match expression {
        Expression::Reference(field) | Expression::AnchoredReference(field) => {
            if let Some(target) = moves.get(field) {
                field.clone_from(target);
            }
        }
        Expression::SameRowReference(field) | Expression::AnchoredSameRowReference(field) => {
            let original = FieldRef::new(source.clone(), field.clone());
            let target = moves.get(&original).unwrap_or(&original);
            let anchored = matches!(expression, Expression::AnchoredSameRowReference(_));
            *expression = match (target.entity == *destination, anchored) {
                (true, false) => Expression::SameRowReference(target.field.clone()),
                (true, true) => Expression::AnchoredSameRowReference(target.field.clone()),
                (false, false) => Expression::Reference(target.clone()),
                (false, true) => Expression::AnchoredReference(target.clone()),
            };
        }
        _ => {
            for child in expression.children_mut() {
                retarget(child, source, destination, moves);
            }
        }
    }
}
