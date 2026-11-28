//! Snapshot planning for one atomic Cut → Paste publication.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{
    CollectionSpec, ColumnSpec, DesignerError, DesignerRequest, DesignerResponse, DesignerRuntime,
    Entity, EntityId, Expression, FieldDefinition, FieldId, FieldRef, FieldTarget, FieldType,
    HistoryAction, HistoryEntry, PublicationProjection, RowIds, ScalarEditInput, SchemaId,
    SemanticCommand, Value,
    auto_input::{CellDisplay, cell_style},
    blank_workbook::{added_columns, check_dimensions, column_commands},
    cell_edit_commands, cell_edit_value, explicit_column_display_name, field_target,
    native_table_row, table_error, tracker_error,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CellMove {
    pub from: FieldTarget,
    pub to: FieldTarget,
}

/// Move a stored cell to a 1-based data position in the destination collection.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PositionalCellMove {
    pub from: FieldTarget,
    pub row: usize,
    pub column: usize,
}

#[derive(Clone, Copy)]
struct OriginalCell<'a> {
    column: &'a ColumnSpec,
    native_table_rows: bool,
    initialize_column: bool,
    value: Option<&'a Value>,
}

type CellMap = BTreeMap<FieldRef, FieldRef>;
type ColumnValues = BTreeMap<FieldId, BTreeMap<EntityId, Value>>;

impl From<&FieldDefinition> for ColumnSpec {
    fn from(field: &FieldDefinition) -> Self {
        Self {
            id: field.id.clone(),
            key: field.key.to_string(),
            display_name: field.effective_name().to_owned(),
            display_name_explicit: explicit_column_display_name(field).is_some(),
            field_type: field.field_type.clone(),
            required: field.required,
            constraint: field.constraint.clone(),
            number_format: field.number_format.clone(),
            date_format: field.date_format,
        }
    }
}

pub(super) fn dispatch(
    runtime: &mut DesignerRuntime,
    request: DesignerRequest,
) -> Result<DesignerResponse, DesignerError> {
    let publication = match request {
        DesignerRequest::MoveCells {
            expected_revision,
            moves,
        } => {
            runtime.check_revision(&expected_revision)?;
            plan(runtime, &moves, &BTreeMap::new())?.publish(runtime, &expected_revision)?
        }
        DesignerRequest::MoveCellsAt {
            expected_revision,
            collection,
            moves,
        } => move_cells_at(runtime, &expected_revision, &collection, &moves)?,
        _ => unreachable!("the move dispatcher only receives move requests"),
    };
    Ok(DesignerResponse::Published(publication))
}

fn move_cells_at(
    runtime: &mut DesignerRuntime,
    expected: &str,
    collection: &str,
    moves: &[PositionalCellMove],
) -> Result<PublicationProjection, DesignerError> {
    runtime.check_revision(expected)?;
    let spec = runtime.collection_spec(collection)?;
    if !spec.native_table_rows
        || moves
            .iter()
            .any(|movement| movement.row == 0 || movement.column == 0)
    {
        return Err(table_error(
            "positional moves require 1-based positions in a native scalar table",
        ));
    }
    check_batch_size(runtime, moves.len())?;
    let (last_row, last_column) = moves.iter().fold((0, 0), |(row, column), movement| {
        (row.max(movement.row), column.max(movement.column))
    });
    check_dimensions(runtime.profile, collection, last_row, last_column)?;
    let allocation = Allocation::new(runtime, spec, last_row, last_column)?;
    let columns: Vec<_> = allocation.columns.iter().map(ColumnSpec::from).collect();
    let mut originals = BTreeMap::new();
    let moves: Vec<_> = moves
        .iter()
        .map(|movement| allocation.resolve(movement, &columns, &mut originals))
        .collect();
    let mut plan = plan(runtime, &moves, &originals)?;
    let serial = allocation.row_serial;
    let (mut forward, inverse) = allocation.commands(runtime, &plan.column_values);
    forward.append(&mut plan.forward.commands);
    plan.forward.commands = forward;
    plan.inverse.commands.extend(inverse);
    let publication = plan.publish(runtime, expected)?;
    runtime.row_serial = serial;
    Ok(publication)
}

struct Allocation<'a> {
    spec: &'a CollectionSpec,
    columns: Vec<FieldDefinition>,
    rows: Vec<Entity>,
    row_serial: usize,
}

impl<'a> Allocation<'a> {
    fn new(
        runtime: &DesignerRuntime,
        spec: &'a CollectionSpec,
        last_row: usize,
        last_column: usize,
    ) -> Result<Self, DesignerError> {
        let document = runtime.session.document();
        let schema = &document.schemas[&SchemaId::from(spec.summary.id.clone())];
        let columns = added_columns(
            schema,
            spec.columns.len(),
            last_column,
            runtime.proposal_serial,
            &runtime.row_namespace,
        )?;
        if last_row > spec.entities.len()
            && runtime.row_serial > usize::MAX - runtime.row_identity_reserve()
        {
            return Err(table_error("row identity allocation is exhausted"));
        }
        let mut ids = RowIds::new(
            &runtime.row_namespace,
            runtime.row_serial,
            "native_table_row",
        );
        let mut allocated = BTreeSet::new();
        let rows = (spec.entities.len()..last_row)
            .map(|_| native_table_row(document, spec, &mut ids, &mut allocated))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            spec,
            columns,
            rows,
            row_serial: ids.serial(),
        })
    }

    fn resolve<'cell>(
        &'cell self,
        movement: &PositionalCellMove,
        columns: &'cell [ColumnSpec],
        originals: &mut BTreeMap<FieldRef, OriginalCell<'cell>>,
    ) -> CellMove {
        let row = movement.row - 1;
        let column = movement.column - 1;
        let existing_entity = self.spec.entities.get(row);
        let existing_column = self.spec.columns.get(column);
        let new_row = existing_entity
            .is_none()
            .then(|| &self.rows[row - self.spec.entities.len()]);
        let entity = existing_entity.unwrap_or_else(|| &new_row.expect("new row is allocated").id);
        let definition =
            existing_column.unwrap_or_else(|| &columns[column - self.spec.columns.len()]);
        let target = FieldRef::new(entity.clone(), definition.id.clone());
        if new_row.is_some() || existing_column.is_none() {
            originals.insert(
                target.clone(),
                OriginalCell {
                    column: definition,
                    native_table_rows: true,
                    initialize_column: new_row.is_none(),
                    value: new_row.and_then(|row| row.fields.get(&definition.id)),
                },
            );
        }
        CellMove {
            from: movement.from.clone(),
            to: field_target(&target),
        }
    }

    fn commands(
        mut self,
        runtime: &DesignerRuntime,
        column_values: &ColumnValues,
    ) -> (Vec<SemanticCommand>, Vec<SemanticCommand>) {
        let document = runtime.session.document();
        let schema = SchemaId::from(self.spec.summary.id.clone());
        let mut forward = Vec::new();
        let mut inverse = Vec::new();
        if !self.columns.is_empty() {
            inverse.push(SemanticCommand::SetSchemaFieldOrder {
                schema: schema.clone(),
                order: document.schemas[&schema].field_order.clone(),
            });
        }
        for field in &self.columns {
            let (append, remove) = column_commands(
                &schema,
                std::slice::from_ref(field),
                column_values
                    .get(&field.id)
                    .map(|values| (&field.id, values)),
            );
            forward.extend(append);
            inverse.extend(remove);
        }
        if !self.rows.is_empty() {
            let (rank, restore) = tachiko_workspace_engine::row_order::plan_insertion(
                document,
                &schema,
                &mut self.rows,
                self.spec.entities.len(),
                true,
            );
            forward.extend(rank);
            inverse.extend(restore);
        }
        for entity in self.rows {
            inverse.push(SemanticCommand::RemoveEntity {
                entity: entity.id.clone(),
            });
            forward.push(SemanticCommand::AppendEntity { entity });
        }
        inverse.reverse();
        (forward, inverse)
    }
}

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
struct MovePlan {
    forward: Edit,
    inverse: Edit,
    column_values: ColumnValues,
}

impl MovePlan {
    fn moved_value(&mut self, field: &FieldRef, original: &OriginalCell<'_>, value: Option<Value>) {
        match value {
            Some(value) if original.initialize_column && !matches!(value, Value::Formula(_)) => {
                // A new column's scalar slots are initialized by its schema command.
                self.column_values
                    .entry(field.field.clone())
                    .or_default()
                    .insert(field.entity.clone(), value);
            }
            value => self.value(field, original.value, value.as_ref()),
        }
    }

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

    fn publish(
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

fn plan(
    runtime: &DesignerRuntime,
    moves: &[CellMove],
    allocated: &BTreeMap<FieldRef, OriginalCell<'_>>,
) -> Result<MovePlan, DesignerError> {
    let (by_source, by_destination) = move_maps(moves)?;
    let targets = by_source
        .keys()
        .chain(by_destination.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    check_batch_size(runtime, targets.len())?;
    if let Some(field) = by_source
        .keys()
        .find(|field| allocated.contains_key(*field))
    {
        return Err(DesignerError::UnsupportedScalarEdit {
            field: field.clone(),
        });
    }
    let originals = capture_originals(runtime, &targets, allocated)?;
    let mut plan = MovePlan::default();
    for (field, original) in &originals {
        let source = by_destination.get(field);
        let value = match (
            source,
            original.column.required,
            &original.column.field_type,
        ) {
            (Some(source), _, _) => originals[source].value.cloned().map(|mut value| {
                if let Value::Formula(expression) = &mut value {
                    retarget(expression, &source.entity, &field.entity, &by_source);
                }
                value
            }),
            (None, true, FieldType::Text) => Some(Value::Text(String::new())),
            _ => None,
        };
        admit_value(field, original, value.as_ref())?;
        plan.moved_value(field, original, value);
        plan.style(runtime, field, source);
    }
    plan_inbound(runtime, &by_source, &targets, &mut plan);
    plan.inverse.commands.reverse();
    Ok(plan)
}

fn check_batch_size(runtime: &DesignerRuntime, size: usize) -> Result<(), DesignerError> {
    if size == 0 || size > runtime.max_batch_targets() {
        return Err(tracker_error("the edit range is empty or too large"));
    }
    Ok(())
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
    allocated: &BTreeMap<FieldRef, OriginalCell<'a>>,
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
            if let Some(original) = allocated.get(field) {
                return Ok((field.clone(), *original));
            }
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
                    initialize_column: false,
                    value,
                },
            ))
        })
        .collect()
}

fn admit_value(
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
            return admit_formula(original);
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

fn admit_formula(original: &OriginalCell<'_>) -> Result<(), DesignerError> {
    match original.column.field_type {
        FieldType::Auto | FieldType::Date => Ok(()),
        FieldType::Number
            if matches!(
                original.value,
                None | Some(Value::Number(_) | Value::Formula(_))
            ) =>
        {
            Ok(())
        }
        _ => Err(tracker_error(
            "formula copy requires declared, supported references",
        )),
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
