//! Runtime ownership of imported worksheet identity, names and source order.
use tachiko_workspace_engine::patch_lifecycle::SemanticCommand;
use tachiko_workspace_engine::{Document, Schema};

use crate::interop_document::{
    InteropColumnMetadata, InteropMetadata, InteropSheetMetadata,
    validate_import_metadata_for_profile,
};
use crate::{
    DesignerError, DesignerResponse, DesignerRuntime, DesignerWireReply, HistoryAction,
    HistoryEntry, PublicationProjection,
};

/// History owns independent before/after mappings so a deleted sheet's source
/// styles and position survive Undo. No runtime borrow crosses publication.
#[derive(Clone, Debug)]
pub(super) struct ImportedState {
    pub metadata: InteropMetadata,
    /// Restored with the mapping by Undo, including the untouched import state.
    pub lifecycle_changed: bool,
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
    pub(super) fn attach_imported_metadata(&mut self, metadata: &InteropMetadata) {
        // Admission already proved the client's mapping through main's gates.
        // Source order lives in the sidecar even after saving and reopening.
        self.imported = Some(ImportedState {
            metadata: metadata.clone(),
            lifecycle_changed: false,
        });
        self.order_collections();
    }

    /// Run the existing publisher against an independent occurrence before
    /// committing. Its invalidation plus the complete sidecar must fit the
    /// actual response envelope, including add-table's collection summary.
    pub(super) fn preflight_imported_reply(
        &self,
        metadata: &InteropMetadata,
        publish: impl FnOnce(&mut Self) -> Result<DesignerResponse, DesignerError>,
    ) -> Result<(), DesignerError> {
        let mut candidate = Self::from_document_profile_baseline(
            self.session.document().clone(),
            crate::PREFLIGHT_OCCURRENCE,
            self.profile,
        )?;
        candidate
            .default_collection
            .clone_from(&self.default_collection);
        candidate.opened_from_v3 = self.opened_from_v3;
        candidate.set_presentation(self.presentation.clone())?;
        // No imported state is attached to this disposable runtime, so its
        // publication cannot recursively preflight another metadata reply.
        let mut response = publish(&mut candidate)?;
        if candidate.profile == crate::SessionProfile::WorkingSize {
            response = candidate.with_changed_cells(response);
        }
        let publication = match &mut response {
            DesignerResponse::Published(publication) => publication,
            DesignerResponse::Duplicated(duplicated) => &mut duplicated.publication,
            _ => unreachable!("imported mutation publishes an invalidation"),
        };
        self.current_revision()
            .clone_into(&mut publication.base_revision);
        let generation = self
            .current_revision()
            .strip_prefix("resident/")
            .and_then(|generation| generation.parse::<u64>().ok())
            .and_then(|generation| generation.checked_add(1))
            .ok_or_else(|| crate::tracker_error("resident revision counter exhausted"))?;
        publication.resulting_revision = format!("resident/{generation}");
        publication.import_metadata = Some(metadata.clone());
        crate::ensure_wire_reply_size(&DesignerWireReply::Ok { response }, self.profile)
    }

    /// Export accepts the mapping the client most recently received; mixing
    /// an older sidecar with a newer worksheet lifecycle would lose identities.
    pub(super) fn metadata_for_export<'a>(
        &'a self,
        source: &'a InteropMetadata,
    ) -> Result<&'a InteropMetadata, DesignerError> {
        match self
            .imported
            .as_ref()
            .filter(|state| state.lifecycle_changed)
        {
            Some(state) if !same_mapping(&state.metadata, source) => {
                Err(crate::interop_document::imported_export_error(
                    "workbook",
                    "Import metadata is stale; use the mapping published by the current worksheet lifecycle.",
                ))
            }
            Some(state) => Ok(&state.metadata),
            None => Ok(source),
        }
    }

    pub(super) fn project_import_metadata(
        &self,
        source: Option<&InteropMetadata>,
    ) -> Result<Option<InteropMetadata>, DesignerError> {
        self.imported
            .as_ref()
            .map(|state| {
                let metadata = self.metadata_for_export(source.unwrap_or(&state.metadata))?;
                validate_import_metadata_for_profile(
                    self.session.document(),
                    metadata,
                    self.profile,
                )?;
                Ok(metadata.clone())
            })
            .transpose()
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
                    | SemanticCommand::SetSchemaHeaderless { .. }
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
            let schema = &document.schemas[sheet.schema_id.as_str()];
            if let Some(name) = &schema.display_name {
                sheet.name.clone_from(name);
            }
            if commands.iter().any(|command| {
                matches!(command, SemanticCommand::SetSchemaHeaderless { schema: id, .. }
                    if id == &schema.id)
            }) {
                sheet.has_header = !schema.headerless;
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
        state.lifecycle_changed |= !same_mapping(&before.metadata, &state.metadata);
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

/// Presentation and metadata wire versions do not identify a worksheet mapping.
fn same_mapping(left: &InteropMetadata, right: &InteropMetadata) -> bool {
    left.sheets.len() == right.sheets.len()
        && left.sheets.iter().zip(&right.sheets).all(|(left, right)| {
            left.schema_id == right.schema_id
                && left.name == right.name
                && left.has_header == right.has_header
                && left.columns.len() == right.columns.len()
                && left
                    .columns
                    .iter()
                    .zip(&right.columns)
                    .all(|(left, right)| left.field_id == right.field_id && left.name == right.name)
        })
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
