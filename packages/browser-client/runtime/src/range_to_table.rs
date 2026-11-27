//! Turn a stored rectangle into one independently addressable table.
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    CollectionSpec, ColumnSpec, DesignerError, DesignerRequest, DesignerResponse, DesignerRuntime,
    Document, DuplicateCollectionProjection, Entity, EntityId, FieldId, HistoryAction,
    HistoryEntry, IdGenerator, NewTableIds, Schema, SchemaId, SelectionStatisticsRange,
    SemanticCommand, SemanticIdKind, admit_duplicate_name, allocate_key, blank_workbook, capacity,
    collection_copy_formula::CopyReferences, header_promotion, table_error,
};

impl DesignerRuntime {
    pub(super) fn dispatch_table_creation(
        &mut self,
        request: DesignerRequest,
    ) -> Result<DesignerResponse, DesignerError> {
        match request {
            DesignerRequest::AddTable {
                expected_revision,
                name,
                columns,
            } => Ok(DesignerResponse::Duplicated(self.add_table(
                &expected_revision,
                &name,
                &columns,
            )?)),
            DesignerRequest::RangeToTable {
                expected_revision,
                range,
                name,
                has_header,
            } => Ok(DesignerResponse::Duplicated(self.range_to_table(
                &expected_revision,
                &range,
                &name,
                has_header,
            )?)),
            DesignerRequest::DuplicateCollection {
                expected_revision,
                collection,
                name,
                column_names,
            } => Ok(DesignerResponse::Duplicated(self.duplicate_collection(
                &expected_revision,
                &collection,
                &name,
                &column_names,
                None,
            )?)),
            DesignerRequest::DuplicateCollectionAt {
                expected_revision,
                collection,
                name,
                column_names,
                to_index,
            } => Ok(DesignerResponse::Duplicated(self.duplicate_collection(
                &expected_revision,
                &collection,
                &name,
                &column_names,
                Some(to_index),
            )?)),
            _ => unreachable!("table creation request expected"),
        }
    }

    pub(super) fn range_to_table(
        &mut self,
        expected: &str,
        range: &SelectionStatisticsRange,
        name: &str,
        has_header: bool,
    ) -> Result<DuplicateCollectionProjection, DesignerError> {
        self.check_revision(expected)?;
        let source = self.collection_spec(&range.collection)?;
        let (rows, columns) = rectangle(source, range)?;
        let document = self.session.document();
        let name = admit_duplicate_name(document, name)?;
        let data = &rows[usize::from(has_header)..];
        check_capacity(document, self.profile, name, data.len(), columns.len())?;
        let source_schema = &document.schemas[source.summary.id.as_str()];
        let names = if has_header {
            let texts = self.header_texts(columns, &document.entities[&rows[0]])?;
            header_promotion::header_names(&texts, source_schema.id.as_str())?.0
        } else {
            let mut occupied = BTreeSet::new();
            (1..=columns.len())
                .map(|index| blank_workbook::column_label(index, &mut occupied))
                .collect()
        };
        let occupied = document
            .schemas
            .values()
            .map(|schema| schema.key.to_string())
            .collect();
        let key = allocate_key(name, "table", &occupied)?;
        let mut ids = NewTableIds::with_serial(
            &format!("{}/range", self.row_namespace),
            self.duplicate_serial,
        );
        let mut schema = Schema {
            id: new_schema_id(document, &mut ids),
            key: key.clone().into(),
            display_name: Some(name.to_owned()),
            headerless: false,
            fields: BTreeMap::new(),
            field_order: None,
        };
        let field_ids = copy_fields(
            document,
            source_schema,
            columns.iter().zip(names),
            &mut ids,
            &mut schema,
        );
        let schema_ids = if data.len() == source.entities.len() {
            BTreeMap::from([(source_schema.id.clone(), schema.id.clone())])
        } else {
            BTreeMap::new()
        };
        let identities = row_identities(document, data.len(), &mut ids);
        let entities = copy_rows(
            document,
            data,
            &schema.id,
            identities,
            (&field_ids, &schema_ids),
        )?;
        self.publish_range_table(expected, schema, entities, ids.serial)
    }

    fn publish_range_table(
        &mut self,
        expected: &str,
        schema: Schema,
        entities: Vec<Entity>,
        serial: usize,
    ) -> Result<DuplicateCollectionProjection, DesignerError> {
        let key = schema.key.to_string();
        let inverse = vec![SemanticCommand::RemoveCollection {
            schema: schema.id.clone(),
            entities: entities.iter().map(|entity| entity.id.clone()).collect(),
        }];
        let forward = vec![SemanticCommand::AppendCollection { schema, entities }];
        let imported_before = self.imported.clone();
        let publication =
            self.publish_commands_with_imported(expected, &forward, None, Some(&key), None)?;
        self.duplicate_serial = serial;
        self.record_history_entry(
            HistoryEntry::new(
                HistoryAction::Commands(forward),
                HistoryAction::Commands(inverse),
            )
            .with_imported(imported_before, self.imported.as_ref()),
        );
        Ok(DuplicateCollectionProjection {
            publication,
            collection: self.collection_specs[&key].summary.clone(),
        })
    }
}

fn check_capacity(
    document: &Document,
    profile: capacity::SessionProfile,
    name: &str,
    rows: usize,
    columns: usize,
) -> Result<(), DesignerError> {
    let limits = capacity::limits_for_profile(profile);
    for (limit, maximum, observed) in [
        (
            crate::CapacityLimit::Collections,
            limits.collections,
            document.schemas.len() + 1,
        ),
        (
            crate::CapacityLimit::TotalEntities,
            limits.total_entities,
            document.entities.len() + rows,
        ),
        (crate::CapacityLimit::TableRows, limits.table_rows, rows),
        (
            crate::CapacityLimit::TableFields,
            limits.table_fields,
            columns,
        ),
    ] {
        capacity::check_limit(limit, maximum, observed, Some(name))?;
    }
    Ok(())
}

fn rectangle<'a>(
    source: &'a CollectionSpec,
    range: &SelectionStatisticsRange,
) -> Result<(&'a [EntityId], &'a [ColumnSpec]), DesignerError> {
    let row_index =
        |id: &EntityId| {
            source.stored_indices.get(id).copied().ok_or_else(|| {
                table_error("range row anchor is not a stored row of the source table")
            })
        };
    let first = row_index(&range.first)?;
    let last = row_index(&range.last)?;
    let columns = source
        .columns
        .windows(range.fields.len().max(1))
        .find(|columns| {
            columns
                .iter()
                .map(|column| column.id.as_str())
                .eq(range.fields.iter().map(String::as_str))
        })
        .ok_or_else(|| {
            table_error("range fields must be nonempty and contiguous in source column order")
        })?;
    Ok((&source.entities[first.min(last)..=first.max(last)], columns))
}

fn new_schema_id(document: &Document, ids: &mut NewTableIds) -> SchemaId {
    loop {
        let id = SchemaId::from(ids.generate(SemanticIdKind::Schema));
        if !document.schemas.contains_key(&id) {
            return id;
        }
    }
}

fn copy_fields<'a>(
    document: &Document,
    source: &Schema,
    columns: impl Iterator<Item = (&'a ColumnSpec, String)>,
    ids: &mut NewTableIds,
    schema: &mut Schema,
) -> BTreeMap<FieldId, FieldId> {
    let mut field_ids = BTreeMap::new();
    let mut order = Vec::new();
    for (column, name) in columns {
        let id = loop {
            let id = FieldId::from(ids.generate(SemanticIdKind::Field));
            if !document
                .schemas
                .values()
                .any(|schema| schema.fields.contains_key(&id))
            {
                break id;
            }
        };
        let mut field = source.fields[&column.id].clone();
        field.id.clone_from(&id);
        field.display_name = Some(name);
        field_ids.insert(column.id.clone(), id.clone());
        order.push(id.clone());
        schema.fields.insert(id, field);
    }
    schema.field_order = Some(order);
    schema.normalize_field_order();
    field_ids
}

fn row_identities(
    document: &Document,
    rows: usize,
    ids: &mut NewTableIds,
) -> BTreeMap<EntityId, String> {
    let occupied: BTreeSet<_> = document
        .entities
        .values()
        .map(|entity| entity.key.as_str())
        .collect();
    let mut identities = BTreeMap::new();
    while identities.len() < rows {
        let id = EntityId::from(ids.generate(SemanticIdKind::Entity));
        let key = format!("range_row_{}", ids.serial);
        if !document.entities.contains_key(&id) && !occupied.contains(key.as_str()) {
            identities.insert(id, key);
        }
    }
    identities
}

fn copy_rows(
    document: &Document,
    rows: &[EntityId],
    schema: &SchemaId,
    identities: BTreeMap<EntityId, String>,
    (fields, schemas): (&BTreeMap<FieldId, FieldId>, &BTreeMap<SchemaId, SchemaId>),
) -> Result<Vec<Entity>, DesignerError> {
    // Rank ties and unranked rows use ID order, so assign IDs in sorted order.
    let entity_ids = rows
        .iter()
        .cloned()
        .zip(identities.keys().cloned())
        .collect();
    rows.iter()
        .zip(identities)
        .map(|(row, (id, key))| {
            let source = &document.entities[row];
            let remap = CopyReferences::whole(&entity_ids, fields, schemas).for_row(row);
            let mut copied = BTreeMap::new();
            for (original, target) in fields {
                if let Some(value) = source.fields.get(original) {
                    copied.insert(target.clone(), remap.value(value)?);
                }
            }
            Ok(Entity {
                id,
                key: key.into(),
                rank: source.rank.clone(),
                schema: schema.clone(),
                fields: copied,
            })
        })
        .collect()
}
