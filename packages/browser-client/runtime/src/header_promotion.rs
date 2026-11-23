//! Promote headings without replacing field identities, keys or declared types.
use std::collections::{BTreeMap, BTreeSet};

use tachiko_workspace_engine::trim_display_name;

use crate::{
    DesignerError, DesignerResponse, DesignerRuntime, FieldRef, HeaderFallbackProjection,
    HeaderFallbackReason, PublicationProjection, SchemaId, SemanticCommand, auto_input,
    blank_workbook, display_text, name_admission, view_value,
};

fn header_names(
    texts: &[String],
    schema: &str,
) -> Result<(Vec<String>, Vec<Option<HeaderFallbackReason>>), DesignerError> {
    let mut failures = Vec::new();
    let mut names = Vec::new();
    let mut reasons = Vec::new();
    let mut seen = BTreeSet::new();
    // Reserve real headings before generating labels, so a blank A1 cannot
    // steal a later heading spelled "Column A".
    let mut occupied: BTreeSet<String> = texts
        .iter()
        .map(|text| trim_display_name(text).to_owned())
        .filter(|name| !name.is_empty())
        .collect();
    for (index, text) in texts.iter().enumerate() {
        let name = trim_display_name(text);
        let reason = if name.is_empty() {
            Some(HeaderFallbackReason::Blank)
        } else {
            name_admission::check_name(
                text,
                Some(&name_admission::NameInput::Column {
                    index,
                    schema: Some(schema.to_owned()),
                }),
                &[],
                &mut failures,
            );
            (!seen.insert(name.to_owned())).then_some(HeaderFallbackReason::Duplicate)
        };
        names.push(if reason.is_some() {
            let base = blank_workbook::column_label(index + 1);
            let mut generated = base.clone();
            let mut suffix = 2;
            while !occupied.insert(generated.clone()) {
                generated = format!("{base} ({suffix})");
                suffix += 1;
            }
            generated
        } else {
            name.to_owned()
        });
        reasons.push(reason);
    }
    name_admission::refuse(failures)?;
    Ok((names, reasons))
}

impl DesignerRuntime {
    fn header_texts(
        &self,
        spec: &crate::CollectionSpec,
        row: &crate::Entity,
    ) -> Result<Vec<String>, DesignerError> {
        let targets: Vec<_> = spec
            .columns
            .iter()
            .filter(|column| row.fields.contains_key(&column.id))
            .map(|column| FieldRef::new(row.id.clone(), column.id.clone()))
            .collect();
        let values = self.session.query_fields(&targets)?;
        let values: BTreeMap<_, _> = values
            .value()
            .iter()
            .map(|value| (&value.field.field, value))
            .collect();
        spec.columns
            .iter()
            .map(|column| {
                let value = values.get(&column.id).and_then(|value| view_value(value));
                if matches!(row.fields.get(&column.id), Some(crate::Value::Formula(_)))
                    && value.is_none()
                {
                    return Err(DesignerError::HeaderFormulaUnavailable);
                }
                let (number, date) =
                    auto_input::cell_formats(&self.presentation, column, row.id.as_str());
                Ok(value
                    .as_ref()
                    .and_then(|value| display_text::display_text(value, number, date))
                    .unwrap_or_default())
            })
            .collect()
    }

    pub(super) fn promote_first_row_to_headers(
        &mut self,
        expected: &str,
        collection: &str,
    ) -> Result<PublicationProjection, DesignerError> {
        self.check_revision(expected)?;
        let spec = self.collection_spec(collection)?.clone();
        let schema_id = SchemaId::from(spec.summary.id.clone());
        let schema = &self.session.document().schemas[&schema_id];
        if !schema.headerless {
            return Err(DesignerError::TableAlreadyHasHeaders);
        }
        let row_id = spec.entities.first().ok_or(DesignerError::NoHeaderRow)?;
        let row = self.session.document().entities[row_id].clone();
        let texts = self.header_texts(&spec, &row)?;
        let (names, reasons) = header_names(&texts, schema_id.as_str())?;
        let mut forward = vec![SemanticCommand::SetSchemaHeaderless {
            schema: schema_id.clone(),
            headerless: false,
        }];
        let mut inverse = vec![
            SemanticCommand::SetSchemaHeaderless {
                schema: schema_id.clone(),
                headerless: true,
            },
            SemanticCommand::AppendEntity { entity: row },
        ];
        let mut fallbacks = Vec::new();
        for ((column, name), reason) in spec.columns.iter().zip(names).zip(reasons) {
            inverse.push(SemanticCommand::SetSchemaFieldDisplayName {
                schema: schema_id.clone(),
                field: column.id.clone(),
                display_name: schema.fields[&column.id].display_name.clone(),
            });
            if let Some(reason) = reason {
                fallbacks.push(HeaderFallbackProjection {
                    field: column.id.to_string(),
                    name: name.clone(),
                    reason,
                });
            }
            forward.push(SemanticCommand::SetSchemaFieldDisplayName {
                schema: schema_id.clone(),
                field: column.id.clone(),
                display_name: Some(name),
            });
        }
        forward.push(SemanticCommand::RemoveEntity {
            entity: row_id.clone(),
        });
        if let Some(imported) = &self.imported {
            let mut metadata = imported.metadata.clone();
            metadata
                .sheets
                .iter_mut()
                .find(|sheet| sheet.schema_id == schema_id.as_str())
                .expect("imported metadata covers the promoted schema")
                .has_header = true;
            // The operation-specific labels must fit alongside the complete
            // imported publication before its commands or history are committed.
            self.preflight_imported_reply(&metadata, |candidate| {
                let mut publication = candidate.publish_commands("resident/0", &forward)?;
                publication.header_fallbacks.clone_from(&fallbacks);
                Ok(DesignerResponse::Published(publication))
            })?;
        }
        let mut publication = self.record_imported_table_edit(expected, forward, inverse)?;
        publication.header_fallbacks = fallbacks;
        Ok(publication)
    }
}
