use tachiko_workspace_engine::{CellStyle, FieldId, SchemaId};

use crate::{DesignerError, DesignerRuntime, HistoryAction, HistoryEntry, PublicationProjection};

pub(super) struct ColumnPresentation {
    pub(super) style: Option<CellStyle>,
    pub(super) width: Option<u32>,
}

impl DesignerRuntime {
    pub(super) fn record_column_presentation(
        &mut self,
        expected: &str,
        (schema, field): (SchemaId, FieldId),
        before: ColumnPresentation,
        after: ColumnPresentation,
    ) -> Result<PublicationProjection, DesignerError> {
        let publication = self.apply_column_presentation(
            expected,
            &schema,
            &field,
            after.style.clone(),
            after.width,
        )?;
        self.record_history_entry(HistoryEntry::new(
            HistoryAction::ColumnPresentation {
                schema: schema.clone(),
                field: field.clone(),
                style: after.style,
                width: after.width,
            },
            HistoryAction::ColumnPresentation {
                schema,
                field,
                style: before.style,
                width: before.width,
            },
        ));
        Ok(publication)
    }
}
