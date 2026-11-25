use tachiko_workspace_engine::SchemaId;
use tachiko_workspace_engine::patch_lifecycle::PatchLifecycleError;

use crate::imported_lifecycle::ImportedState;
use crate::{
    DesignerError, DesignerResponse, DesignerRuntime, HistoryAction, HistoryEntry, InteropMetadata,
    PublicationProjection,
};

impl DesignerRuntime {
    pub(super) fn order_collections(&mut self) {
        let imported = self.imported.as_ref();
        let order = &self.presentation.collection_order;
        self.collections.sort_unstable_by(|left, right| {
            let position = |id: &str| match imported {
                Some(state) => state
                    .metadata
                    .sheets
                    .iter()
                    .position(|sheet| sheet.schema_id == id),
                None => order.iter().position(|schema| schema.as_str() == id),
            };
            position(&left.id)
                .unwrap_or(usize::MAX)
                .cmp(&position(&right.id).unwrap_or(usize::MAX))
                .then_with(|| left.key.cmp(&right.key))
        });
    }

    pub(super) fn has_custom_collection_order(&self) -> bool {
        !self.presentation.collection_order.is_empty()
            && self
                .collections
                .windows(2)
                .any(|pair| pair[0].key > pair[1].key)
    }

    pub(super) fn validate_saved_collection_order(
        &self,
        metadata: &InteropMetadata,
    ) -> Result<(), DesignerError> {
        let order = &self.presentation.collection_order;
        if !order.is_empty()
            && order
                .iter()
                .map(SchemaId::as_str)
                .ne(metadata.sheets.iter().map(|sheet| sheet.schema_id.as_str()))
        {
            return Err(crate::interop_document::imported_export_error(
                "workbook",
                "Import metadata order does not match the saved worksheet order.",
            ));
        }
        Ok(())
    }

    pub(super) fn move_collection(
        &mut self,
        expected: &str,
        collection: &str,
        to_index: u32,
    ) -> Result<PublicationProjection, DesignerError> {
        self.check_revision(expected)?;
        let mut order = self
            .collections
            .iter()
            .map(|summary| SchemaId::from(summary.id.clone()))
            .collect::<Vec<_>>();
        let from = self
            .collections
            .iter()
            .position(|summary| summary.key == collection)
            .ok_or_else(|| DesignerError::MissingCollection {
                collection: collection.to_owned(),
            })?;
        let to = usize::try_from(to_index)
            .unwrap_or(usize::MAX)
            .min(order.len() - 1);
        if from == to {
            return Err(PatchLifecycleError::NoChange.into());
        }
        let moved = order.remove(from);
        order.insert(to, moved);

        let before = self.presentation.collection_order.clone();
        let imported_before = self.imported.clone();
        let mut imported = imported_before.clone();
        if let Some(state) = &mut imported {
            let moved = state.metadata.sheets.remove(from);
            state.metadata.sheets.insert(to, moved);
            state.lifecycle_changed = true;
            self.preflight_imported_reply(&state.metadata, |candidate| {
                candidate
                    .apply_collection_order("resident/0", order.clone(), None)
                    .map(DesignerResponse::Published)
            })?;
        }

        let publication = self.apply_collection_order(expected, order.clone(), imported)?;
        self.record_history_entry(
            HistoryEntry::new(
                HistoryAction::CollectionOrder(order),
                HistoryAction::CollectionOrder(before),
            )
            .with_imported(imported_before, self.imported.as_ref()),
        );
        Ok(publication)
    }

    pub(super) fn apply_collection_order(
        &mut self,
        expected: &str,
        order: Vec<SchemaId>,
        imported: Option<ImportedState>,
    ) -> Result<PublicationProjection, DesignerError> {
        let mut presentation = self.presentation.clone();
        presentation.collection_order = order;
        let resulting_revision = self.publish_presentation(expected, presentation)?;
        let import_metadata = imported.as_ref().map(|state| state.metadata.clone());
        if imported.is_some() {
            self.imported = imported;
            self.import_metadata_published = true;
        }
        self.order_collections();
        Ok(PublicationProjection {
            base_revision: expected.to_owned(),
            resulting_revision,
            schemas: self
                .collections
                .iter()
                .map(|summary| summary.id.clone())
                .collect(),
            schema_fields: Vec::new(),
            entities: Vec::new(),
            fields: Vec::new(),
            affected_calculations: Vec::new(),
            many: false,
            changed: Vec::new(),
            import_metadata,
            header_fallbacks: Vec::new(),
        })
    }
}
