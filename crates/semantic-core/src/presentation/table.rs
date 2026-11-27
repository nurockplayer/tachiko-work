use std::collections::{BTreeMap, BTreeSet};

use crate::display_name_failures;

use super::{
    Document, EntityId, FieldId, FooterAggregate, PresentationError, PresentationStore, SchemaId,
    StyleId,
};

/// Presentation of one whole table.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TablePresentation {
    pub title: Option<String>,
    pub footer: BTreeMap<FieldId, FooterAggregate>,
    pub column_styles: BTreeMap<FieldId, StyleId>,
    pub column_widths: BTreeMap<FieldId, u32>,
    pub hidden_rows: BTreeSet<EntityId>,
    pub hidden_fields: BTreeSet<FieldId>,
}

pub(super) fn prune_table(
    schema_id: &SchemaId,
    table: &TablePresentation,
    document: &Document,
) -> Option<TablePresentation> {
    let schema = document.schemas.get(schema_id)?;
    let mut table = table.clone();
    table
        .footer
        .retain(|field, _| schema.fields.contains_key(field));
    table
        .column_styles
        .retain(|field, _| schema.fields.contains_key(field));
    table
        .column_widths
        .retain(|field, _| schema.fields.contains_key(field));
    table
        .hidden_fields
        .retain(|field| schema.fields.contains_key(field));
    table
        .hidden_rows
        .retain(|entity| row_in_schema(document, schema_id, entity));
    (table != TablePresentation::default()).then_some(table)
}

fn row_in_schema(document: &Document, schema: &SchemaId, entity: &EntityId) -> bool {
    document
        .entities
        .get(entity)
        .is_some_and(|row| &row.schema == schema)
}

pub(super) fn validate_table(
    schema_id: &SchemaId,
    table: &TablePresentation,
    store: &PresentationStore,
    document: &Document,
) -> Result<(), PresentationError> {
    let schema =
        document
            .schemas
            .get(schema_id)
            .ok_or_else(|| PresentationError::MissingSchema {
                schema: schema_id.clone(),
            })?;
    if *table == TablePresentation::default() {
        return Err(PresentationError::EmptyTable {
            schema: schema_id.clone(),
        });
    }
    if let Some(title) = &table.title {
        let failures = display_name_failures(title);
        if !failures.is_empty() {
            return Err(PresentationError::InvalidTitle {
                schema: schema_id.clone(),
                failures,
            });
        }
    }
    validate_table_targets(table, schema, document)?;
    for (field, width) in &table.column_widths {
        if !(1..=65280).contains(width) {
            return Err(PresentationError::InvalidColumnWidth {
                schema: schema_id.clone(),
                field: field.clone(),
                width: *width,
            });
        }
    }
    for style in table.column_styles.values() {
        if !store.styles.contains_key(style) {
            return Err(PresentationError::MissingStyle {
                style: style.clone(),
            });
        }
    }
    Ok(())
}

fn validate_table_targets(
    table: &TablePresentation,
    schema: &crate::Schema,
    document: &Document,
) -> Result<(), PresentationError> {
    let schema_id = &schema.id;
    for field in table
        .footer
        .keys()
        .chain(table.column_styles.keys())
        .chain(table.column_widths.keys())
        .chain(&table.hidden_fields)
    {
        if !schema.fields.contains_key(field) {
            return Err(PresentationError::MissingField {
                schema: schema_id.clone(),
                field: field.clone(),
            });
        }
    }
    for entity in &table.hidden_rows {
        if !row_in_schema(document, schema_id, entity) {
            return Err(PresentationError::MissingEntity {
                entity: entity.clone(),
            });
        }
    }
    Ok(())
}
