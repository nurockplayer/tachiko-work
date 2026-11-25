//! Headerless grid creation and atomic positional writes.
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    DesignerError, DesignerResponse, DesignerRuntime, FieldDefinition, FieldKey, FieldRef,
    FieldType, IdGenerator, NewTableIds, PasteCellInput, PublicationProjection, RowIds,
    ScalarEditInput, SchemaId, SemanticCommand, SemanticIdKind, appended_field_id, auto_input,
    cell_edit_commands, native_table_row, parse_column_initializer, pasted_column_input,
    table_error,
};

pub(super) fn candidate(
    occurrence_id: &str,
) -> Result<(DesignerRuntime, DesignerResponse), DesignerError> {
    let mut ids = NewTableIds::new(occurrence_id);
    let schema_id = SchemaId::from(ids.generate(SemanticIdKind::Schema));
    let schema = crate::Schema {
        id: schema_id.clone(),
        key: "sheet".into(),
        display_name: Some("Sheet 1".into()),
        headerless: true,
        fields: BTreeMap::new(),
        field_order: None,
    };
    let document = crate::Document {
        id: ids.generate(SemanticIdKind::Document).into(),
        title: "Untitled workbook".into(),
        schemas: BTreeMap::from([(schema_id, schema)]),
        entities: BTreeMap::new(),
        keyed_grouped_sum_definitions: BTreeMap::new(),
    };
    let mut runtime = DesignerRuntime::from_document_working_size(document, occurrence_id)?;
    let opened = runtime.opened_window()?;
    Ok((runtime, DesignerResponse::OpenedWindow(Box::new(opened))))
}

pub(super) fn column_label(mut index: usize, occupied: &mut BTreeSet<String>) -> String {
    let mut letters = Vec::new();
    while index > 0 {
        index -= 1;
        letters.push(char::from(
            b'A' + u8::try_from(index % 26).expect("base 26 digit"),
        ));
        index /= 26;
    }
    let base = format!("Column {}", letters.into_iter().rev().collect::<String>());
    let mut generated = base.clone();
    let mut suffix = 2;
    while !occupied.insert(generated.clone()) {
        generated = format!("{base} ({suffix})");
        suffix += 1;
    }
    generated
}

impl DesignerRuntime {
    /// Resolve a positional block once, then publish its structure and values together.
    #[allow(clippy::too_many_lines)] // One atomic allocation and input publication.
    pub(super) fn paste_cells_at(
        &mut self,
        expected: &str,
        collection: &str,
        row: usize,
        column: usize,
        rows: &[Vec<PasteCellInput>],
    ) -> Result<PublicationProjection, DesignerError> {
        self.check_revision(expected)?;
        let spec = self.collection_spec(collection)?.clone();
        if row == 0 || column == 0 || !spec.native_table_rows {
            return Err(table_error(
                "positional paste requires 1-based positions in a native scalar table",
            ));
        }
        if rows.is_empty() || rows[0].is_empty() || rows.iter().any(|r| r.len() != rows[0].len()) {
            return Err(table_error("paste must be a nonempty rectangular range"));
        }
        let last_row = row.saturating_add(rows.len() - 1);
        let last_column = column.saturating_add(rows[0].len() - 1);
        let limits = crate::capacity::limits_for_profile(self.profile);
        for (limit, maximum, observed) in [
            (crate::CapacityLimit::TableRows, limits.table_rows, last_row),
            (
                crate::CapacityLimit::TableFields,
                limits.table_fields,
                last_column,
            ),
        ] {
            crate::capacity::check_limit(limit, maximum, observed, Some(collection))?;
        }
        if self.row_serial > usize::MAX - self.row_identity_reserve() {
            return Err(table_error("row identity allocation is exhausted"));
        }
        let snapshot = self.session.export_snapshot();
        let document = snapshot.document();
        let schema_id = SchemaId::from(spec.summary.id.clone());
        let schema = &document.schemas[&schema_id];
        let added = added_columns(
            schema,
            spec.columns.len(),
            last_column,
            self.proposal_serial,
            &self.row_namespace,
        )?;
        let fields = spec
            .columns
            .iter()
            .map(|c| &schema.fields[&c.id])
            .chain(added.iter())
            .collect::<Vec<_>>();
        let mut forward = Vec::new();
        let mut inverse = Vec::new();
        let mut added_values = BTreeMap::new();
        let mut row_ids = RowIds::new(&self.row_namespace, self.row_serial, "native_table_row");
        let mut allocated = BTreeSet::new();
        let mut new_rows = (spec.entities.len()..last_row)
            .map(|_| native_table_row(document, &spec, &mut row_ids, &mut allocated))
            .collect::<Result<Vec<_>, _>>()?;
        let mut displays = Vec::new();
        let mut formulas = Vec::new();
        for (r, cells) in rows.iter().enumerate() {
            let position = row - 1 + r;
            let existing = spec.entities.get(position);
            let entity_id = existing
                .cloned()
                .unwrap_or_else(|| new_rows[position - spec.entities.len()].id.clone());
            for (c, cell) in cells.iter().enumerate() {
                let definition = fields[column - 1 + c];
                let input = match cell {
                    PasteCellInput::Text(text) => {
                        pasted_column_input(&definition.field_type, text)?
                    }
                    PasteCellInput::Typed(input) => input.clone(),
                };
                let target = FieldRef::new(entity_id.clone(), definition.id.clone());
                if definition.field_type == FieldType::Auto {
                    if let Some(source) = auto_input::formula_source(&input) {
                        formulas.push((target, source.to_owned()));
                        continue;
                    }
                }
                let value = if definition.field_type == FieldType::Auto {
                    auto_input::value(&input)?
                } else if matches!(input, ScalarEditInput::Clear) {
                    None
                } else {
                    Some(parse_column_initializer(&definition.field_type, &input)?)
                };
                auto_input::collect_display(&definition.field_type, &target, &input, &mut displays);
                if existing.is_some() {
                    if schema.fields.contains_key(&definition.id) {
                        let before = document.entities[&entity_id].fields.get(&definition.id);
                        if before != value.as_ref() {
                            let (command, undo) =
                                cell_edit_commands(&target, before, value.as_ref());
                            forward.push(command);
                            inverse.extend(undo);
                        }
                    } else if let Some(value) = value {
                        // New-field initializers belong to schema mutation scope,
                        // not SetFieldValue against a field absent from the base.
                        added_values
                            .entry(definition.id.clone())
                            .or_insert_with(BTreeMap::new)
                            .insert(entity_id.clone(), value);
                    }
                } else {
                    let entity = &mut new_rows[position - spec.entities.len()];
                    if let Some(value) = value {
                        entity.fields.insert(definition.id.clone(), value);
                    } else {
                        entity.fields.remove(&definition.id);
                    }
                }
            }
        }
        let formula_commands =
            positional_formula_commands(document, &schema_id, &added, &new_rows, formulas)?;
        let mut column_forward = Vec::new();
        let mut column_inverse = Vec::new();
        if !added.is_empty() {
            column_inverse.push(SemanticCommand::SetSchemaFieldOrder {
                schema: schema_id.clone(),
                order: schema.field_order.clone(),
            });
        }
        for field in &added {
            let values = added_values.remove(&field.id).unwrap_or_default();
            let (append, remove) =
                column_commands(&schema_id, std::slice::from_ref(field), &field.id, &values);
            column_forward.extend(append);
            column_inverse.extend(remove);
        }
        column_forward.append(&mut forward);
        column_inverse.append(&mut inverse);
        forward = column_forward;
        inverse = column_inverse;
        if !new_rows.is_empty() {
            let (ranks, restore) = tachiko_workspace_engine::row_order::plan_insertion(
                document,
                &schema_id,
                &mut new_rows,
                spec.entities.len(),
                true,
            );
            forward.extend(ranks);
            inverse.extend(restore);
        }
        for entity in new_rows {
            inverse.push(SemanticCommand::RemoveEntity {
                entity: entity.id.clone(),
            });
            forward.push(SemanticCommand::AppendEntity { entity });
        }
        for (target, command) in formula_commands {
            if let Some(entity) = document.entities.get(&target.entity) {
                if schema.fields.contains_key(&target.field) {
                    inverse.push(auto_input::inverse_formula(
                        &target,
                        entity.fields.get(&target.field),
                    ));
                }
            }
            forward.push(command);
        }
        inverse.reverse();
        let publication = self.record_input_edit(expected, forward, inverse, displays)?;
        self.row_serial = row_ids.serial();
        Ok(publication)
    }

    pub(super) fn write_cell_at(
        &mut self,
        expected: &str,
        collection: &str,
        row: usize,
        column: usize,
        input: &ScalarEditInput,
    ) -> Result<PublicationProjection, DesignerError> {
        self.check_revision(expected)?;
        let spec = self.collection_spec(collection)?.clone();
        check_position(self.profile, &spec, collection, row, column, input)?;
        let document = self.session.document();
        let schema_id = SchemaId::from(spec.summary.id.clone());
        let schema = &document.schemas[&schema_id];
        let added = added_columns(
            schema,
            spec.columns.len(),
            column,
            self.proposal_serial,
            &self.row_namespace,
        )?;
        let definition = added
            .last()
            .unwrap_or_else(|| &schema.fields[&spec.columns[column - 1].id]);
        if definition.field_type == FieldType::Auto && auto_input::formula_source(input).is_some() {
            return self.paste_cells_at(
                expected,
                collection,
                row,
                column,
                &[vec![PasteCellInput::Typed(input.clone())]],
            );
        }
        let field_id = definition.id.clone();
        let value = if definition.field_type == FieldType::Auto {
            auto_input::value(input)?
        } else if matches!(input, ScalarEditInput::Clear) {
            None
        } else {
            Some(parse_column_initializer(&definition.field_type, input)?)
        };
        let added_values = if !added.is_empty() && row <= spec.entities.len() {
            value
                .iter()
                .map(|value| (spec.entities[row - 1].clone(), value.clone()))
                .collect()
        } else {
            BTreeMap::new()
        };
        let (mut forward, mut inverse) =
            column_commands(&schema_id, &added, &field_id, &added_values);
        let mut row_ids = RowIds::new(&self.row_namespace, self.row_serial, "native_table_row");
        let entity_id = if row > spec.entities.len() {
            let mut allocated = BTreeSet::new();
            let mut rows = (spec.entities.len()..row)
                .map(|_| native_table_row(document, &spec, &mut row_ids, &mut allocated))
                .collect::<Result<Vec<_>, _>>()?;
            let entity = rows.last_mut().expect("at least one new row");
            if let Some(value) = value.clone() {
                entity.fields.insert(field_id.clone(), value);
            }
            let id = entity.id.clone();
            let (ranks, restore) = tachiko_workspace_engine::row_order::plan_insertion(
                document,
                &schema_id,
                &mut rows,
                spec.entities.len(),
                true,
            );
            forward.extend(ranks);
            inverse.extend(restore);
            for entity in rows {
                inverse.push(SemanticCommand::RemoveEntity {
                    entity: entity.id.clone(),
                });
                forward.push(SemanticCommand::AppendEntity { entity });
            }
            id
        } else {
            let id = spec.entities[row - 1].clone();
            let target = FieldRef::new(id.clone(), field_id.clone());
            let before = document.entities[&id].fields.get(&field_id).cloned();
            // A new column carries its initial value in AppendSchemaField.
            if added.is_empty() && before != value {
                let (command, undo) = cell_edit_commands(&target, before.as_ref(), value.as_ref());
                forward.push(command);
                inverse.extend(undo);
            }
            id
        };
        let target = FieldRef::new(entity_id, field_id);
        let mut displays = Vec::new();
        auto_input::collect_display(&definition.field_type, &target, input, &mut displays);
        if !added.is_empty() {
            // Remove rows/cells before their columns, then restore the original field order.
            inverse.rotate_left(added.len());
            inverse.push(SemanticCommand::SetSchemaFieldOrder {
                schema: schema_id,
                order: schema.field_order.clone(),
            });
        }
        let publication = self.record_input_edit(expected, forward, inverse, displays)?;
        self.row_serial = row_ids.serial();
        Ok(publication)
    }
}

fn positional_formula_commands(
    document: &crate::Document,
    schema_id: &SchemaId,
    added: &[FieldDefinition],
    new_rows: &[crate::Entity],
    formulas: Vec<(FieldRef, String)>,
) -> Result<Vec<(FieldRef, SemanticCommand)>, DesignerError> {
    if formulas.is_empty() {
        return Ok(Vec::new());
    }
    let mut candidate = document.clone();
    let schema = candidate
        .schemas
        .get_mut(schema_id)
        .expect("positional schema exists");
    for field in added {
        schema.insert_field(field.clone());
    }
    candidate
        .entities
        .extend(new_rows.iter().map(|row| (row.id.clone(), row.clone())));
    formulas
        .into_iter()
        .map(|(target, source)| {
            let command =
                SemanticCommand::bind_formula_update(&candidate, target.clone(), &source)?;
            Ok((target, command))
        })
        .collect()
}

fn added_columns(
    schema: &crate::Schema,
    existing: usize,
    column: usize,
    mut serial: u64,
    namespace: &str,
) -> Result<Vec<FieldDefinition>, DesignerError> {
    let mut schema = schema.clone();
    let mut occupied: BTreeSet<String> = schema
        .fields
        .values()
        .map(|field| field.key.to_string())
        .collect();
    let mut names = schema
        .fields
        .values()
        .map(|field| {
            field
                .display_name
                .as_deref()
                .unwrap_or(field.key.as_str())
                .to_owned()
        })
        .collect();
    let mut added = Vec::new();
    for position in existing + 1..=column {
        serial = serial
            .checked_add(1)
            .ok_or_else(|| table_error("column identity allocation is exhausted"))?;
        let id = appended_field_id(&schema, serial, namespace);
        let name = column_label(position, &mut names);
        let key = crate::allocate_key(&name, "column", &occupied)?;
        occupied.insert(key.clone());
        let field = FieldDefinition {
            id: id.clone(),
            key: FieldKey::from(key),
            display_name: Some(name),
            field_type: FieldType::Auto,
            required: false,
            constraint: tachiko_workspace_engine::FieldConstraint::None,
            number_format: None,
            date_format: None,
        };
        schema.fields.insert(id, field.clone());
        added.push(field);
    }
    Ok(added)
}

fn check_position(
    profile: crate::SessionProfile,
    spec: &crate::CollectionSpec,
    collection: &str,
    row: usize,
    column: usize,
    input: &ScalarEditInput,
) -> Result<(), DesignerError> {
    if row == 0 || column == 0 {
        return Err(table_error("write_cell_at positions are 1-based"));
    }
    if !spec.native_table_rows {
        return Err(table_error("write_cell_at requires a native scalar table"));
    }
    if matches!(input, ScalarEditInput::Clear)
        && (row > spec.entities.len() || column > spec.columns.len())
    {
        return Err(crate::PatchLifecycleError::NoChange.into());
    }
    let limits = crate::capacity::limits_for_profile(profile);
    crate::capacity::check_limit(
        crate::CapacityLimit::TableRows,
        limits.table_rows,
        row,
        Some(collection),
    )?;
    crate::capacity::check_limit(
        crate::CapacityLimit::TableFields,
        limits.table_fields,
        column,
        Some(collection),
    )?;
    Ok(())
}

fn column_commands(
    schema_id: &SchemaId,
    added: &[FieldDefinition],
    field_id: &crate::FieldId,
    added_values: &BTreeMap<crate::EntityId, crate::Value>,
) -> (Vec<SemanticCommand>, Vec<SemanticCommand>) {
    let mut forward = Vec::new();
    let mut inverse = Vec::new();
    for field in added {
        let values = if field.id == *field_id {
            added_values.clone()
        } else {
            BTreeMap::new()
        };
        forward.push(SemanticCommand::AppendSchemaField {
            schema: schema_id.clone(),
            field: field.clone(),
            values: values.clone(),
        });
        inverse.push(SemanticCommand::RemoveSchemaField {
            schema: schema_id.clone(),
            field: field.clone(),
            values,
        });
    }
    (forward, inverse)
}
