//! Headerless grid creation and atomic adjacent positional writes.
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    ColumnSpec, DesignerError, DesignerResponse, DesignerRuntime, FieldDefinition, FieldKey,
    FieldRef, FieldType, IdGenerator, NewTableIds, PublicationProjection, RowIds, ScalarEditInput,
    SchemaId, SemanticCommand, SemanticIdKind, appended_field_id, auto_input, native_table_row,
    parse_column_initializer, table_error, value_command,
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

fn column_label(mut index: usize) -> String {
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
    #[allow(clippy::too_many_lines)] // One plan, one publication and one inverse for structural input.
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
        if row == 0 || column == 0 {
            return Err(table_error("write_cell_at positions are 1-based"));
        }
        if row > spec.entities.len() + 1 || column > spec.columns.len() + 1 {
            return Err(table_error(
                "distant writes are unsupported; write within or adjacent to the used range",
            ));
        }
        if !spec.native_table_rows {
            return Err(table_error("write_cell_at requires a native scalar table"));
        }
        let document = self.session.document();
        let schema_id = SchemaId::from(spec.summary.id.clone());
        let schema = &document.schemas[&schema_id];
        let mut forward = Vec::new();
        let mut inverse = Vec::new();
        let mut added = None;
        let field_id = if column > spec.columns.len() {
            let serial = self
                .proposal_serial
                .checked_add(1)
                .ok_or_else(|| table_error("column identity allocation is exhausted"))?;
            let id = appended_field_id(schema, serial, &self.row_namespace);
            let name = column_label(column);
            let occupied = schema
                .fields
                .values()
                .map(|field| field.key.to_string())
                .collect();
            let key = crate::allocate_key(&name, "column", &occupied)?;
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
            added = Some(field);
            id
        } else {
            spec.columns[column - 1].id.clone()
        };
        let definition = added.as_ref().unwrap_or_else(|| &schema.fields[&field_id]);
        let value = if definition.field_type == FieldType::Auto {
            auto_input::value(input)?
        } else if matches!(input, ScalarEditInput::Clear) {
            None
        } else {
            Some(parse_column_initializer(&definition.field_type, input)?)
        };
        let added_values = if added.is_some() && row <= spec.entities.len() {
            value
                .iter()
                .map(|value| (spec.entities[row - 1].clone(), value.clone()))
                .collect()
        } else {
            BTreeMap::new()
        };
        if let Some(field) = &added {
            forward.push(SemanticCommand::AppendSchemaField {
                schema: schema_id.clone(),
                field: field.clone(),
                values: added_values.clone(),
            });
        }
        let mut row_ids = RowIds::new(&self.row_namespace, self.row_serial, "native_table_row");
        let entity_id = if row > spec.entities.len() {
            let mut entity = native_table_row(document, &spec, &mut row_ids, &mut BTreeSet::new())?;
            if let Some(value) = value.clone() {
                entity.fields.insert(field_id.clone(), value);
            }
            let (ranks, restore) = tachiko_workspace_engine::row_order::plan_insertion(
                document,
                &schema_id,
                std::slice::from_mut(&mut entity),
                spec.entities.len(),
                false,
            );
            forward.extend(ranks);
            inverse.extend(restore);
            let id = entity.id.clone();
            forward.push(SemanticCommand::AppendEntity { entity });
            inverse.push(SemanticCommand::RemoveEntity { entity: id.clone() });
            id
        } else {
            let id = spec.entities[row - 1].clone();
            let target = FieldRef::new(id.clone(), field_id.clone());
            let before = document.entities[&id].fields.get(&field_id).cloned();
            // A new column carries its initial value in AppendSchemaField.
            if added.is_none() && before != value {
                forward.push(value_command(&target, value.as_ref()));
                inverse.push(value_command(&target, before.as_ref()));
            }
            id
        };
        let target = FieldRef::new(entity_id, field_id);
        let display_column = ColumnSpec {
            id: definition.id.clone(),
            key: definition.key.to_string(),
            display_name: definition.effective_name().to_owned(),
            display_name_explicit: definition.display_name.is_some(),
            field_type: definition.field_type.clone(),
            required: definition.required,
            constraint: definition.constraint.clone(),
            number_format: definition.number_format.clone(),
            date_format: definition.date_format,
        };
        let mut displays = Vec::new();
        auto_input::collect_display(&display_column, &target, input, &mut displays);
        if let Some(field) = added {
            inverse.push(SemanticCommand::RemoveSchemaField {
                schema: schema_id.clone(),
                field,
                values: added_values,
            });
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
