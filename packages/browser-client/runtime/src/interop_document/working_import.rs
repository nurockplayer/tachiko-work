use std::{borrow::Cow, collections::BTreeSet};

use super::{
    DesignerError, DesignerRuntime, ExportA1Maps, FidelityCategory, FidelityFinding,
    ImportedOutput, ImportedWindowProjection, InteropMetadata, InteropRowMetadata,
    stable_reference_binding,
};
use super::{
    ImportAdmission, ImportSelection, SourceWorkbook, import_capacity_refusal,
    import_workbook_document, normalize_import_date_styles, promote_import_presentation,
    validate_import_metadata_for_profile,
};
use crate::{
    MAX_TEXT_CAPACITY_PROJECTION_BYTES, REPLY_ENVELOPE_RESERVE, ensure_complete_projection_size,
    interop_adapter::cap_ledger, serialized_projection_bytes,
};

pub(crate) fn import_admitted_workbook_working_size(
    workbook: SourceWorkbook,
    selection: &ImportSelection,
    occurrence_id: &str,
) -> Result<(DesignerRuntime, ImportedWindowProjection), DesignerError> {
    candidate(Cow::Owned(workbook), selection, occurrence_id, true).map_err(import_capacity_refusal)
}

pub(super) fn candidate(
    workbook: Cow<'_, SourceWorkbook>,
    selection: &ImportSelection,
    occurrence_id: &str,
    source_admitted: bool,
) -> Result<(DesignerRuntime, ImportedWindowProjection), DesignerError> {
    let (mut runtime, mut metadata, mut ledger, export_maps) = import_workbook_document(
        &workbook,
        selection,
        occurrence_id,
        DesignerRuntime::from_document_working_size,
        ImportAdmission {
            working: true,
            source_admitted,
        },
    )?;
    normalize_import_date_styles(&workbook, &mut metadata, runtime.profile);
    validate_import_metadata_for_profile(runtime.session.document(), &metadata, runtime.profile)?;
    runtime = promote_import_presentation(
        runtime,
        &workbook,
        &mut metadata,
        &mut ledger,
        occurrence_id,
    )?;
    // The host owns the decoded source. Release it before allocating the
    // exportability proof's rebuilt workbook.
    drop(workbook);
    finish(runtime, metadata, ledger, &export_maps)
}

fn finish(
    mut runtime: DesignerRuntime,
    mut metadata: InteropMetadata,
    mut ledger: Vec<FidelityFinding>,
    export_maps: &ExportA1Maps,
) -> Result<(DesignerRuntime, ImportedWindowProjection), DesignerError> {
    discard_presentation(&mut metadata, &mut ledger);
    metadata.version = 2;
    // The same representation proof as an ordinary import, without promotion.
    drop(runtime.export_imported_workbook_with_maps(
        runtime.current_revision(),
        &metadata,
        ImportedOutput::Projection,
        Some(export_maps),
    )?);
    ledger.push(stable_reference_binding());
    cap_ledger(&mut ledger);
    // The reply also carries the metadata and ledger, so the window gets what
    // is left of the one reply budget.
    runtime.attach_imported_metadata(&metadata);
    let overhead = serialized_projection_bytes(&(&metadata, &ledger))? + REPLY_ENVELOPE_RESERVE;
    let maximum = MAX_TEXT_CAPACITY_PROJECTION_BYTES.saturating_sub(overhead);
    let opened = runtime.opened_window_within(maximum)?;
    let projection = ImportedWindowProjection {
        opened,
        metadata,
        ledger,
    };
    ensure_complete_projection_size("imported", &projection, MAX_TEXT_CAPACITY_PROJECTION_BYTES)?;
    Ok((runtime, projection))
}

fn discard_presentation(metadata: &mut InteropMetadata, ledger: &mut Vec<FidelityFinding>) {
    for sheet in &mut metadata.sheets {
        for (index, column) in sheet.columns.iter_mut().enumerate() {
            let dropped = dropped_styles(&sheet.rows, index);
            if !dropped.is_empty() {
                ledger.push(FidelityFinding {
                    category: FidelityCategory::LossyOnExport,
                    code: "presentation_not_kept".into(),
                    location: format!("{}:column {}", sheet.name, index + 1),
                    message: format!(
                        "Presentation not kept: {}.",
                        dropped.into_iter().collect::<Vec<_>>().join(", ")
                    ),
                    blocking: false,
                });
            }
            column.width = None;
        }
        sheet.rows.clear();
    }
}

fn dropped_styles(rows: &[InteropRowMetadata], index: usize) -> BTreeSet<String> {
    let mut dropped = BTreeSet::new();
    for row in rows {
        let style = &row.styles[index];
        if let Some(pattern) = &style.number_format {
            dropped.insert(format!("number format {pattern:?}"));
        }
        if let Some(alignment) = &style.alignment {
            dropped.insert(format!("alignment {alignment:?}"));
        }
        if let Some(fill) = &style.fill {
            dropped.insert(format!("fill {fill:?}"));
        }
    }
    dropped
}
