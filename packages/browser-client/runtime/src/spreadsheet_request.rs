//! Shared requests for native and browser spreadsheet bridges.
use serde::Deserialize;

use crate::interop_adapter::{ImportOptions, SourceWorkbook};
use crate::source_reuse::InspectedSource;
use crate::{
    DesignerError, DesignerProfile, DesignerResponse, DesignerRuntime, ImportSelection,
    InteropMetadata, NativeBudgetExportPresentation, NativeTrackerExportPresentation,
};

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SpreadsheetFormat {
    Csv,
    Xlsx,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum SpreadsheetOperation {
    Inspect {
        format: SpreadsheetFormat,
        csv_options: ImportOptions,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile: Option<DesignerProfile>,
    },
    Import(SpreadsheetImport),
    InspectProject {
        metadata: InteropMetadata,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile: Option<DesignerProfile>,
    },
    OpenProject {
        occurrence_id: String,
        metadata: InteropMetadata,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile: Option<DesignerProfile>,
    },
    ReopenImportedXlsxWorkingSize {
        expected: crate::OccurrenceProjection,
        occurrence_id: String,
    },
    Export {
        expected_revision: String,
        metadata: InteropMetadata,
        format: SpreadsheetFormat,
        collection: String,
    },
    ExportXlsxOverwrite {
        expected: crate::OccurrenceProjection,
    },
    ExportNativeTables {
        expected_revision: String,
        collection: String,
        format: SpreadsheetFormat,
    },
    ExportNativeTracker {
        expected_revision: String,
        presentation: NativeTrackerExportPresentation,
        format: SpreadsheetFormat,
    },
    ExportNativeBudget {
        expected_revision: String,
        presentation: NativeBudgetExportPresentation,
        format: SpreadsheetFormat,
    },
}

pub(super) fn admit_saved_import(
    project: &[u8],
    occurrence_id: &str,
    metadata: &InteropMetadata,
    profile: Option<DesignerProfile>,
) -> Result<(DesignerRuntime, DesignerResponse), DesignerError> {
    match profile {
        None => {
            let (candidate, opened) =
                crate::admit_imported_project(project, occurrence_id, metadata)?;
            Ok((candidate, DesignerResponse::Opened(Box::new(opened))))
        }
        Some(DesignerProfile::WorkingSize) => {
            let (candidate, opened) =
                crate::admit_imported_project_working_size(project, occurrence_id, metadata)?;
            Ok((candidate, DesignerResponse::OpenedWindow(Box::new(opened))))
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SpreadsheetImport {
    pub(super) format: SpreadsheetFormat,
    pub(super) csv_options: ImportOptions,
    pub(super) selection: ImportSelection,
    pub(super) occurrence_id: String,
    pub(super) install: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) profile: Option<DesignerProfile>,
}

impl SpreadsheetImport {
    pub(super) fn original<'a>(&self, bytes: &'a [u8]) -> Option<&'a [u8]> {
        matches!(self.format, SpreadsheetFormat::Xlsx).then_some(bytes)
    }

    pub(super) fn source(
        &self,
        bytes: &[u8],
        retained: &mut Option<InspectedSource>,
        read: impl FnOnce(
            SpreadsheetFormat,
            &ImportOptions,
            bool,
        ) -> Result<SourceWorkbook, DesignerError>,
    ) -> Result<SourceWorkbook, DesignerError> {
        let working = self.profile.is_some();
        let workbook = retained.take().and_then(|source| {
            source.consume(
                bytes,
                &self.csv_options,
                working,
                self.original(bytes).is_some(),
            )
        });
        workbook.map_or_else(
            || read(self.format, &self.csv_options, working),
            |workbook| Ok(*workbook),
        )
    }
}
