//! Runtime ownership of imported worksheet identity, names and source order.
use tachiko_workspace_engine::patch_lifecycle::SemanticCommand;
use tachiko_workspace_engine::{Document, Schema};

use crate::interop_document::{
    InteropColumnMetadata, InteropMetadata, InteropSheetMetadata,
    validate_import_metadata_for_profile,
};
use crate::{DesignerError, DesignerRuntime, HistoryAction, HistoryEntry, PublicationProjection};

/// History owns independent before/after mappings so a deleted sheet's source
/// styles and position survive Undo. No runtime borrow crosses publication.
#[derive(Clone, Debug)]
pub(super) struct ImportedState {
    pub metadata: InteropMetadata,
}

impl HistoryEntry {
    pub(super) fn with_imported(
        mut self,
        before: Option<ImportedState>,
        after: Option<&ImportedState>,
    ) -> Self {
        if let (Some(before), Some(after)) = (before, after) {
            // Both owned mappings count against the working-size history cap.
            for state in [&before, after] {
                self.weight = self.weight.saturating_add(
                    crate::serialized_projection_bytes(&state.metadata).unwrap_or(usize::MAX),
                );
            }
            self.imported = Some((before, after.clone()));
        }
        self
    }
}

impl DesignerRuntime {
    pub(super) fn attach_imported_metadata(
        &mut self,
        metadata: &InteropMetadata,
    ) -> Result<(), DesignerError> {
        if self.imported.is_none() {
            validate_import_metadata_for_profile(self.session.document(), metadata, self.profile)?;
            self.imported = Some(ImportedState {
                metadata: metadata.clone(),
            });
            self.order_imported_collections();
        }
        Ok(())
    }

    pub(super) fn refresh_imported_opened(
        &self,
        opened: &mut crate::OpenedProjection,
    ) -> Result<(), DesignerError> {
        opened.bootstrap = self.bootstrap_projection();
        opened.table.collection = self
            .collection_spec(&opened.table.collection.id)?
            .summary
            .clone();
        crate::ensure_opened_wire_projection_size(opened, self.profile)
    }

    pub(super) fn order_imported_collections(&mut self) {
        if let Some(state) = &self.imported {
            for collection in &mut self.collections {
                if let Some(sheet) = state
                    .metadata
                    .sheets
                    .iter()
                    .find(|sheet| sheet.schema_id == collection.id)
                {
                    collection.display_name.clone_from(&sheet.name);
                    if let Some(spec) = self.collection_specs.get_mut(&collection.key) {
                        spec.summary.display_name.clone_from(&sheet.name);
                    }
                }
            }
            self.collections.sort_by_key(|collection| {
                state
                    .metadata
                    .sheets
                    .iter()
                    .position(|sheet| sheet.schema_id == collection.id)
                    .unwrap_or(usize::MAX)
            });
        }
    }

    pub(super) fn imported_table_name<'a>(&'a self, schema: &'a Schema) -> &'a str {
        if schema.display_name.is_none() {
            if let Some(sheet) = self.imported.as_ref().and_then(|state| {
                state
                    .metadata
                    .sheets
                    .iter()
                    .find(|sheet| sheet.schema_id == schema.id.as_str())
            }) {
                return &sheet.name;
            }
        }
        schema.effective_name()
    }

    /// Export accepts the mapping the client most recently received; mixing
    /// an older sidecar with a newer worksheet lifecycle would lose identities.
    pub(super) fn metadata_for_export<'a>(
        &'a self,
        source: &'a InteropMetadata,
    ) -> Result<&'a InteropMetadata, DesignerError> {
        match &self.imported {
            Some(state) if &state.metadata != source => {
                Err(crate::interop_document::imported_export_error(
                    "workbook",
                    "Import metadata is stale; use the mapping published by the current worksheet lifecycle.",
                ))
            }
            Some(state) => Ok(&state.metadata),
            None => Ok(source),
        }
    }

    pub(super) fn plan_imported_state(
        &self,
        document: &Document,
        commands: &[SemanticCommand],
    ) -> Result<Option<ImportedState>, DesignerError> {
        let Some(before) = &self.imported else {
            return Ok(None);
        };
        if !commands.iter().any(|command| {
            matches!(
                command,
                SemanticCommand::SetSchemaDisplayName { .. }
                    | SemanticCommand::AppendCollection { .. }
                    | SemanticCommand::RemoveCollection { .. }
            )
        }) {
            return Ok(None);
        }
        let mut state = before.clone();
        state
            .metadata
            .sheets
            .retain(|sheet| document.schemas.contains_key(sheet.schema_id.as_str()));
        for sheet in &mut state.metadata.sheets {
            if let Some(name) = &document.schemas[sheet.schema_id.as_str()].display_name {
                sheet.name.clone_from(name);
            }
        }
        for schema in document.schemas.values() {
            if !state
                .metadata
                .sheets
                .iter()
                .any(|sheet| sheet.schema_id == schema.id.as_str())
            {
                state.metadata.sheets.push(added_sheet_metadata(schema));
            }
        }
        validate_import_metadata_for_profile(document, &state.metadata, self.profile)?;
        Ok(Some(state))
    }

    pub(super) fn record_imported_table_edit(
        &mut self,
        expected: &str,
        forward: Vec<SemanticCommand>,
        inverse: Vec<SemanticCommand>,
    ) -> Result<PublicationProjection, DesignerError> {
        let before = self.imported.clone();
        let publication = self.publish_commands(expected, &forward)?;
        self.record_history_entry(
            HistoryEntry::new(
                HistoryAction::Commands(forward),
                HistoryAction::Commands(inverse),
            )
            .with_imported(before, self.imported.as_ref()),
        );
        Ok(publication)
    }
}

fn added_sheet_metadata(schema: &Schema) -> InteropSheetMetadata {
    let columns = schema
        .ordered_fields()
        .map(|field| InteropColumnMetadata {
            field_id: field.id.to_string(),
            name: field.effective_name().to_owned(),
            width: None,
        })
        .collect();
    InteropSheetMetadata {
        schema_id: schema.id.to_string(),
        name: schema.effective_name().to_owned(),
        has_header: true,
        columns,
        rows: Vec::new(),
    }
}
