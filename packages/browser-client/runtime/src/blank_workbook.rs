//! Headerless grid creation and atomic positional writes.
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    DesignerError, DesignerResponse, DesignerRuntime, FieldDefinition, FieldKey, FieldRef,
    FieldType, IdGenerator, NewTableIds, PublicationProjection, RowIds, ScalarEditInput, SchemaId,
    SemanticCommand, SemanticIdKind, appended_field_id, auto_input, cell_edit_commands,
    native_table_row, parse_column_initializer, table_error,
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

pub(super) fn column_label(mut index: usize) -> String {
    let mut letters = Vec::new();
    while index > 0 {
        index -= 1;
        letters.push(char::from(
            b'A' + u8::try_from(index % 26).expect("base 26 digit"),
        ));
        index /= 26;
    }
    format!("Column {}", letters.into_iter().rev().collect::<String>())
}

impl DesignerRuntime {
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
    let mut added = Vec::new();
    for position in existing + 1..=column {
        serial = serial
            .checked_add(1)
            .ok_or_else(|| table_error("column identity allocation is exhausted"))?;
        let id = appended_field_id(&schema, serial, namespace);
        let name = column_label(position);
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
