//! Identity-bound cell presentation commands; semantic values never change.
use serde::{Deserialize, Serialize};
use tachiko_workspace_engine::{CellStyle, EntityId, FieldId, PresentationStore, SchemaId};

use crate::{
    CollectionSpec, DesignerError, DesignerRuntime, HistoryAction, HistoryEntry,
    PatchLifecycleError, PublicationProjection, capacity, interop_adapter, prune_presentation,
    table_error, validate_presentation,
};

/// Changes are applied in order to the Cartesian product of record and field IDs.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CellStyleChange {
    Bold {
        value: bool,
    },
    Italic {
        value: bool,
    },
    ToggleBold,
    ToggleItalic,
    Fill {
        #[serde(deserialize_with = "deserialize_fill_color")]
        color: Option<String>,
    },
}

fn deserialize_fill_color<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}

impl DesignerRuntime {
    pub(super) fn set_cell_style(
        &mut self,
        expected: &str,
        collection: &str,
        entities: &[String],
        fields: &[String],
        changes: &[CellStyleChange],
    ) -> Result<PublicationProjection, DesignerError> {
        self.check_revision(expected)?;
        let schema = self.style_selection(collection, entities, fields)?;
        for change in changes {
            if let CellStyleChange::Fill { color: Some(color) } = change {
                let bytes = color.as_bytes();
                if bytes.len() != 7
                    || bytes[0] != b'#'
                    || !bytes[1..]
                        .iter()
                        .all(|byte| matches!(byte, b'0'..=b'9' | b'A'..=b'F'))
                {
                    return Err(DesignerError::InvalidStyleColor);
                }
            }
        }
        let entities = entities
            .iter()
            .map(|id| EntityId::from(id.clone()))
            .collect::<Vec<_>>();
        let fields = fields
            .iter()
            .map(|id| FieldId::from(id.clone()))
            .collect::<Vec<_>>();
        let mut candidate = self.presentation.clone();
        for change in changes {
            let change = resolve_toggle(&candidate, &schema, &entities, &fields, change);
            if matches!(change, CellStyleChange::Fill { color: None }) {
                for field in &fields {
                    self.expand_column_fill(&mut candidate, &schema, field)?;
                }
            }
            for entity in &entities {
                for field in &fields {
                    let mut style = cell_override(&candidate, entity, field);
                    match &change {
                        CellStyleChange::Bold { value } => style.bold = Some(*value),
                        CellStyleChange::Italic { value } => style.italic = Some(*value),
                        CellStyleChange::Fill { color } => style.fill.clone_from(color),
                        CellStyleChange::ToggleBold | CellStyleChange::ToggleItalic => {
                            unreachable!()
                        }
                    }
                    put_cell(&mut candidate, entity, field, style)?;
                }
            }
        }
        compact_palette(&mut candidate);
        if candidate == self.presentation {
            return Err(PatchLifecycleError::NoChange.into());
        }
        self.validate_cell_presentation(&candidate)?;
        let before = history_action(self.presentation.clone())?;
        let after = history_action(candidate.clone())?;
        let publication = self.apply_cell_presentation(expected, candidate)?;
        self.record_history_entry(HistoryEntry::new(after, before));
        Ok(publication)
    }

    fn style_selection(
        &self,
        collection: &str,
        entities: &[String],
        fields: &[String],
    ) -> Result<SchemaId, DesignerError> {
        let spec = self.collection_spec(collection)?;
        let schema = SchemaId::from(spec.summary.id.clone());
        let imported = self.imported.as_ref().is_some_and(|state| {
            state
                .metadata
                .sheets
                .iter()
                .any(|sheet| sheet.schema_id == schema.as_str())
        });
        if !spec.native_table_rows && !spec.direct_tracker_rows && !imported {
            return Err(DesignerError::ReadOnlyStyleTable {
                collection: collection.to_owned(),
            });
        }
        if entities.is_empty() || fields.is_empty() {
            return Err(table_error(
                "cell style selection must have records and fields",
            ));
        }
        for entity in entities {
            if self
                .session
                .document()
                .entities
                .get(&EntityId::from(entity.clone()))
                .is_none_or(|entity| entity.schema != schema)
            {
                return Err(DesignerError::UnknownEntity {
                    collection: collection.to_owned(),
                    entity: entity.clone(),
                });
            }
        }
        for field in fields {
            if !self.session.document().schemas[&schema]
                .fields
                .contains_key(&FieldId::from(field.clone()))
            {
                return Err(DesignerError::UnknownViewField {
                    collection: collection.to_owned(),
                    field: field.clone(),
                });
            }
        }
        Ok(schema)
    }

    // ADR-0049 has no explicit "no fill" sentinel. Move an inherited fill to
    // existing records before clearing a selected cell, preserving its neighbours.
    fn expand_column_fill(
        &self,
        store: &mut PresentationStore,
        schema: &SchemaId,
        field: &FieldId,
    ) -> Result<(), DesignerError> {
        let Some(id) = store
            .tables
            .get(schema)
            .and_then(|table| table.column_styles.get(field))
            .cloned()
        else {
            return Ok(());
        };
        let mut column = store.styles[&id].clone();
        let Some(fill) = column.fill.take() else {
            return Ok(());
        };
        let table = store
            .tables
            .get_mut(schema)
            .expect("column style has a table");
        table.column_styles.remove(field);
        if column != CellStyle::default() {
            let id = tachiko_storage::style_id(&column)?;
            store.styles.insert(id.clone(), column);
            table.column_styles.insert(field.clone(), id);
        }
        for entity in self
            .session
            .document()
            .entities
            .values()
            .filter(|entity| &entity.schema == schema)
        {
            let mut style = cell_override(store, &entity.id, field);
            if style.fill.is_none() {
                style.fill = Some(fill.clone());
                put_cell(store, &entity.id, field, style)?;
            }
        }
        Ok(())
    }

    fn validate_cell_presentation(&self, store: &PresentationStore) -> Result<(), DesignerError> {
        let active = prune_presentation(store, self.session.document());
        let maximum = capacity::limits_for_profile(self.profile).populated_cells;
        if active.cells.len() > maximum {
            return Err(capacity::CapacityExceeded {
                limit: capacity::CapacityLimit::PopulatedCells,
                maximum,
                attempted: active.cells.len(),
                collection: None,
            }
            .into());
        }
        validate_presentation(&active, self.session.document()).map_err(|error| {
            DesignerError::StyleCapacityExceeded {
                message: error.to_string(),
            }
        })
    }

    pub(super) fn preflight_presentation(
        &self,
        document: &tachiko_workspace_engine::Document,
        store: &PresentationStore,
    ) -> Result<(), DesignerError> {
        if self.profile != crate::SessionProfile::WorkingSize {
            // Reopen selects its profile before installing saved presentation.
            let (mut candidate, mut opened) =
                crate::admit_v3_document(document.clone(), crate::PREFLIGHT_OCCURRENCE)?;
            candidate.set_presentation(prune_presentation(store, document))?;
            if let Some(imported) = &self.imported {
                crate::interop_document::validate_saved_open_closure(
                    &candidate,
                    &imported.metadata,
                )?;
            }
            for collection in &candidate.collections {
                candidate.query_table(&collection.key)?;
            }
            opened.table = candidate.query_table(&candidate.default_collection)?;
            crate::ensure_opened_wire_projection_size(&opened, candidate.profile)?;
        }
        Ok(())
    }

    pub(super) fn apply_cell_presentation(
        &mut self,
        expected: &str,
        store: PresentationStore,
    ) -> Result<PublicationProjection, DesignerError> {
        self.validate_cell_presentation(&store)?;
        let resulting_revision = self.publish_presentation(expected, store)?;
        Ok(PublicationProjection {
            base_revision: expected.to_owned(),
            resulting_revision,
            schemas: Vec::new(),
            schema_fields: Vec::new(),
            entities: Vec::new(),
            fields: Vec::new(),
            affected_calculations: Vec::new(),
            many: true,
            changed: Vec::new(),
            import_metadata: None,
        })
    }

    pub(super) fn row_styles(
        &self,
        spec: &CollectionSpec,
        entity: &EntityId,
    ) -> Vec<interop_adapter::CellStyle> {
        if self.presentation.is_empty() {
            return Vec::new();
        }
        let schema = &self.session.document().schemas[&SchemaId::from(spec.summary.id.clone())];
        let styles = spec
            .columns
            .iter()
            .map(|column| {
                crate::interop_date_format::cell_style(
                    &self.presentation,
                    &schema.id,
                    &schema.fields[&column.id],
                    entity,
                    &interop_adapter::SourceValue::Empty,
                    interop_adapter::CellStyle::default(),
                )
            })
            .collect::<Vec<_>>();
        if styles
            .iter()
            .all(|style| *style == interop_adapter::CellStyle::default())
        {
            Vec::new()
        } else {
            styles
        }
    }
}

fn history_action(store: PresentationStore) -> Result<HistoryAction, DesignerError> {
    let bytes = tachiko_storage::encode_presentation(&store)?.map_or(0, |bytes| bytes.len());
    Ok(HistoryAction::CellPresentation { store, bytes })
}

fn cell_override(store: &PresentationStore, entity: &EntityId, field: &FieldId) -> CellStyle {
    store
        .cells
        .get(&(entity.clone(), field.clone()))
        .and_then(|id| store.styles.get(id))
        .cloned()
        .unwrap_or_default()
}

fn resolve_toggle(
    store: &PresentationStore,
    schema: &SchemaId,
    entities: &[EntityId],
    fields: &[FieldId],
    change: &CellStyleChange,
) -> CellStyleChange {
    let italic = match change {
        CellStyleChange::ToggleBold => false,
        CellStyleChange::ToggleItalic => true,
        _ => return change.clone(),
    };
    let all = entities.iter().all(|entity| {
        fields.iter().all(|field| {
            let cell = cell_override(store, entity, field);
            let column = store
                .tables
                .get(schema)
                .and_then(|table| table.column_styles.get(field))
                .and_then(|id| store.styles.get(id));
            if italic {
                cell.italic
                    .or_else(|| column.and_then(|style| style.italic))
                    .unwrap_or(false)
            } else {
                cell.bold
                    .or_else(|| column.and_then(|style| style.bold))
                    .unwrap_or(false)
            }
        })
    });
    if italic {
        CellStyleChange::Italic { value: !all }
    } else {
        CellStyleChange::Bold { value: !all }
    }
}

fn put_cell(
    store: &mut PresentationStore,
    entity: &EntityId,
    field: &FieldId,
    style: CellStyle,
) -> Result<(), DesignerError> {
    let key = (entity.clone(), field.clone());
    if style == CellStyle::default() {
        store.cells.remove(&key);
    } else {
        let id = tachiko_storage::style_id(&style)?;
        store.styles.insert(id.clone(), style);
        store.cells.insert(key, id);
    }
    Ok(())
}

fn compact_palette(store: &mut PresentationStore) {
    store
        .tables
        .retain(|_, table| *table != tachiko_workspace_engine::TablePresentation::default());
    let used = store
        .cells
        .values()
        .chain(
            store
                .tables
                .values()
                .flat_map(|table| table.column_styles.values()),
        )
        .chain(store.rules.values().map(|rule| &rule.style))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    store.styles.retain(|id, _| used.contains(id));
}
