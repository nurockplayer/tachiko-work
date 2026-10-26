//! Native private byte bridge for an app-local desktop host.
//!
//! This is the native counterpart of the browser-only `wasm.rs` bridge. It
//! performs the same private operations over the same Rust-authoritative
//! runtime and returns the same JSON wire replies, but owns its state in an
//! ordinary value instead of WASM linear-memory arenas. It is compiled only for
//! non-WASM targets so the browser artifact is unchanged.
//!
//! Like `wasm.rs`, this is provisional delivery mechanics for a first-party
//! host, not a public Semantic API, SDK or stable ABI. The host is a single
//! serial owner: callers must not interleave operations from several threads.

use serde::Deserialize;

use crate::interop_adapter::{
    FidelityCategory, FidelityFinding, ImportOptions, InteropError, MAX_SOURCE_BYTES,
    OutputProfile, SourceWorkbook, export_csv, export_csv_for_profile, export_xlsx,
    export_xlsx_for_profile, import_csv, import_xlsx,
};
use crate::{
    DesignerError, DesignerResponse, DesignerRuntime, DesignerWireReply, FailureProjection,
    MAX_PROJECT_TRANSFER_BYTES, MAX_WIRE_REQUEST_BYTES, ProjectExportProjection, encode_reply,
    ensure_wire_reply_size, inspect_project, open_local_document, open_portable_ro, open_project,
    process_wire_request, request_too_large_reply, verify_portable_ro,
};
use crate::{
    ImportSelection, InteropMetadata, NativeBudgetExportPresentation,
    NativeTrackerExportPresentation, SpreadsheetExportProjection, import_workbook,
};

/// The browser request arena's spreadsheet metadata limit. The crate constant
/// is WASM-only; it is mirrored here so the browser artifact stays unchanged.
const MAX_SPREADSHEET_METADATA_REQUEST_BYTES: usize = 4 * 1024 * 1024;

/// Largest spreadsheet operation request, mirroring the browser request arena.
pub const MAX_HOST_SPREADSHEET_REQUEST_BYTES: usize = MAX_SPREADSHEET_METADATA_REQUEST_BYTES;
/// Largest ordinary request, occurrence identity or expected revision.
pub const MAX_HOST_WIRE_REQUEST_BYTES: usize = MAX_WIRE_REQUEST_BYTES;
/// Largest project, spreadsheet source or export transfer.
pub const MAX_HOST_PROJECT_TRANSFER_BYTES: usize = MAX_PROJECT_TRANSFER_BYTES;

type OpenCandidate =
    fn(&mut Option<DesignerRuntime>, &[u8], &str) -> Result<crate::OpenedProjection, DesignerError>;

/// One encoded wire reply plus the opaque bytes a successful export produced.
///
/// `reply` is the exact JSON `DesignerWireReply` the browser bridge would
/// return for the same operation. `bytes` is present only for successful
/// project, canonical-tree, portable `.ro` and spreadsheet exports.
pub struct HostReply {
    pub reply: Vec<u8>,
    pub bytes: Option<Vec<u8>>,
}

impl HostReply {
    fn reply_only(reply: &DesignerWireReply) -> Self {
        Self {
            reply: encode_reply(reply),
            bytes: None,
        }
    }
}

/// The single resident occurrence owned by one native host.
#[derive(Default)]
pub struct DesignerHost {
    runtime: Option<DesignerRuntime>,
}

impl DesignerHost {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Execute one ordinary private JSON request.
    #[must_use]
    pub fn request(&mut self, request: &[u8]) -> Vec<u8> {
        process_wire_request(&mut self.runtime, request)
    }

    /// Run one private JSON spreadsheet operation over `source` bytes.
    /// Admission and response bounds are checked before installing any candidate.
    #[must_use]
    pub fn spreadsheet(&mut self, request: &[u8], source: &[u8]) -> HostReply {
        let runtime = &mut self.runtime;
        // The browser request arena refuses these before anything reads them.
        if request.len() > MAX_SPREADSHEET_METADATA_REQUEST_BYTES {
            return HostReply {
                reply: request_too_large_reply(runtime.as_ref()),
                bytes: None,
            };
        }
        let metadata_control = if request.len() > MAX_WIRE_REQUEST_BYTES {
            request
                .iter()
                .copied()
                .find(|byte| !byte.is_ascii_whitespace())
                == Some(b'{')
                && serde_json::from_slice::<MetadataControl>(request).is_ok_and(|control| {
                    matches!(
                        control.kind,
                        MetadataControlKind::Export
                            | MetadataControlKind::InspectProject
                            | MetadataControlKind::OpenProject
                    )
                })
        } else {
            true
        };
        if !metadata_control {
            return HostReply {
                reply: request_too_large_reply(runtime.as_ref()),
                bytes: None,
            };
        }
        let result = if source.len() > MAX_PROJECT_TRANSFER_BYTES {
            Err(DesignerError::ProjectTransferTooLarge {
                actual: MAX_PROJECT_TRANSFER_BYTES.saturating_add(1),
                maximum: MAX_PROJECT_TRANSFER_BYTES,
            })
        } else {
            serde_json::from_slice::<SpreadsheetOperation>(request)
                .map_err(|error| DesignerError::InvalidProjectTransfer {
                    message: format!("Invalid spreadsheet operation: {error}"),
                })
                .and_then(|operation| spreadsheet_operation(runtime.as_ref(), operation, source))
        };
        match result {
            Ok(result) => {
                let reply = DesignerWireReply::Ok {
                    response: result.response,
                };
                if let Err(error) = ensure_wire_reply_size(&reply, result.text_capacity) {
                    HostReply::reply_only(&DesignerWireReply::Error {
                        error: error.failure_projection(current_revision(runtime.as_ref())),
                    })
                } else {
                    if let Some(candidate) = result.candidate {
                        *runtime = Some(candidate);
                    }
                    HostReply {
                        reply: encode_reply(&reply),
                        bytes: result.export,
                    }
                }
            }
            Err(error) => HostReply::reply_only(&DesignerWireReply::Error {
                error: error.failure_projection(current_revision(runtime.as_ref())),
            }),
        }
    }

    /// Inspect a fully admitted candidate without replacing resident state.
    #[must_use]
    pub fn inspect_project(&self, project: &[u8]) -> Vec<u8> {
        let reply = if project.len() > MAX_PROJECT_TRANSFER_BYTES {
            too_large_project(self.runtime.as_ref())
        } else {
            match inspect_project(project) {
                Ok(opened) => DesignerWireReply::Ok {
                    response: DesignerResponse::Opened(Box::new(opened)),
                },
                Err(error) => DesignerWireReply::Error {
                    error: error.failure_projection(current_revision(self.runtime.as_ref())),
                },
            }
        };
        encode_reply(&reply)
    }

    /// Fully admit and install one project candidate.
    #[must_use]
    pub fn open_project(&mut self, project: &[u8], occurrence_id: &[u8]) -> Vec<u8> {
        self.open_candidate(project, occurrence_id, open_project)
    }

    /// Fully admit and install one raw local `.ro` document.
    #[must_use]
    pub fn open_local_document(&mut self, project: &[u8], occurrence_id: &[u8]) -> Vec<u8> {
        self.open_candidate(project, occurrence_id, open_local_document)
    }

    /// Fully verify and admit a genuine portable-package/v1 `.ro` candidate.
    #[must_use]
    pub fn open_portable_ro(&mut self, project: &[u8], occurrence_id: &[u8]) -> Vec<u8> {
        self.open_candidate(project, occurrence_id, open_portable_ro)
    }

    /// Verify a genuine portable-package/v1 `.ro` artifact without replacing
    /// the resident occurrence.
    #[must_use]
    pub fn verify_portable_ro(&self, project: &[u8]) -> Vec<u8> {
        let reply = if project.len() > MAX_PROJECT_TRANSFER_BYTES {
            too_large_project(self.runtime.as_ref())
        } else {
            match verify_portable_ro(project) {
                Ok(()) => DesignerWireReply::Ok {
                    response: DesignerResponse::PortableRoVerified(crate::PortableRoVerification {
                        accepted: true,
                    }),
                },
                Err(error) => DesignerWireReply::Error {
                    error: error.failure_projection(current_revision(self.runtime.as_ref())),
                },
            }
        };
        encode_reply(&reply)
    }

    /// Export the exact expected resident revision as an opaque project bundle.
    #[must_use]
    pub fn export_project(&self, expected_revision: &[u8]) -> HostReply {
        self.export_bytes(
            expected_revision,
            DesignerRuntime::export_project,
            DesignerResponse::ProjectExported,
        )
    }

    /// Export the explicitly selected opaque canonical v3 project.
    #[must_use]
    pub fn export_project_v3(&self, expected_revision: &[u8]) -> HostReply {
        self.export_bytes(
            expected_revision,
            DesignerRuntime::export_project_v3,
            DesignerResponse::ProjectExported,
        )
    }

    /// Export the complete canonical v1 tree as its private path/byte record.
    #[must_use]
    pub fn export_canonical_tree(&self, expected_revision: &[u8]) -> HostReply {
        self.export_bytes(
            expected_revision,
            DesignerRuntime::export_canonical_tree,
            DesignerResponse::CanonicalTreeExported,
        )
    }

    /// Export a genuine portable-package/v1 `.ro` artifact.
    #[must_use]
    pub fn export_portable_ro(&self, expected_revision: &[u8]) -> HostReply {
        self.export_bytes(
            expected_revision,
            DesignerRuntime::export_portable_ro,
            DesignerResponse::PortableRoExported,
        )
    }

    /// Read the trusted scope of the active resident occurrence.
    #[must_use]
    pub fn observe_occurrence(&self) -> Vec<u8> {
        let reply = match self.runtime.as_ref() {
            Some(runtime) => DesignerWireReply::Ok {
                response: DesignerResponse::OccurrenceObserved(runtime.observe_occurrence()),
            },
            None => no_project_open(),
        };
        encode_reply(&reply)
    }

    /// Destroy the current occurrence.
    pub fn close_project(&mut self) {
        self.runtime = None;
    }

    fn open_candidate(
        &mut self,
        project: &[u8],
        occurrence_id: &[u8],
        open: OpenCandidate,
    ) -> Vec<u8> {
        let runtime = &mut self.runtime;
        let reply = if project.len() > MAX_PROJECT_TRANSFER_BYTES {
            too_large_project(runtime.as_ref())
        } else if occurrence_id.len() > MAX_SPREADSHEET_METADATA_REQUEST_BYTES {
            // The browser request arena refuses to reserve this identity.
            DesignerWireReply::Error {
                error: DesignerError::InvalidOccurrenceIdentity
                    .failure_projection(current_revision(runtime.as_ref())),
            }
        } else if occurrence_id.len() > MAX_WIRE_REQUEST_BYTES {
            return request_too_large_reply(runtime.as_ref());
        } else if let Ok(occurrence_id) = std::str::from_utf8(occurrence_id) {
            match open(runtime, project, occurrence_id) {
                Ok(opened) => DesignerWireReply::Ok {
                    response: DesignerResponse::Opened(Box::new(opened)),
                },
                Err(error) => DesignerWireReply::Error {
                    error: error.failure_projection(current_revision(runtime.as_ref())),
                },
            }
        } else {
            DesignerWireReply::Error {
                error: DesignerError::InvalidOccurrenceIdentity
                    .failure_projection(current_revision(runtime.as_ref())),
            }
        };
        encode_reply(&reply)
    }

    fn export_bytes<E>(
        &self,
        expected_revision: &[u8],
        export: fn(&DesignerRuntime, &str) -> Result<E, DesignerError>,
        response: fn(ProjectExportProjection) -> DesignerResponse,
    ) -> HostReply
    where
        E: IntoProjectExport,
    {
        let Some(runtime) = self.runtime.as_ref() else {
            return HostReply::reply_only(&no_project_open());
        };
        if expected_revision.len() > MAX_WIRE_REQUEST_BYTES {
            return HostReply {
                reply: request_too_large_reply(Some(runtime)),
                bytes: None,
            };
        }
        let Ok(expected_revision) = std::str::from_utf8(expected_revision) else {
            return HostReply::reply_only(&DesignerWireReply::Error {
                error: DesignerError::InvalidProjectTransfer {
                    message: "expected revision is not UTF-8".to_owned(),
                }
                .failure_projection(runtime.current_revision()),
            });
        };
        match export(runtime, expected_revision) {
            Ok(export) => {
                let export = export.into_project_export();
                let byte_length = export.bytes.len();
                HostReply {
                    reply: encode_reply(&DesignerWireReply::Ok {
                        response: response(ProjectExportProjection {
                            revision: export.revision,
                            byte_length,
                        }),
                    }),
                    bytes: Some(export.bytes),
                }
            }
            Err(error) => HostReply::reply_only(&DesignerWireReply::Error {
                error: error.failure_projection(runtime.current_revision()),
            }),
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SpreadsheetFormat {
    Csv,
    Xlsx,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum SpreadsheetOperation {
    Inspect {
        format: SpreadsheetFormat,
        csv_options: ImportOptions,
    },
    Import {
        format: SpreadsheetFormat,
        csv_options: ImportOptions,
        selection: ImportSelection,
        occurrence_id: String,
        install: bool,
    },
    InspectProject {
        metadata: InteropMetadata,
    },
    OpenProject {
        occurrence_id: String,
        metadata: InteropMetadata,
    },
    Export {
        expected_revision: String,
        metadata: InteropMetadata,
        format: SpreadsheetFormat,
        collection: String,
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

#[derive(Deserialize)]
struct MetadataControl {
    #[serde(rename = "type")]
    kind: MetadataControlKind,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum MetadataControlKind {
    Export,
    InspectProject,
    OpenProject,
}

struct SpreadsheetResult {
    text_capacity: bool,
    response: DesignerResponse,
    candidate: Option<DesignerRuntime>,
    export: Option<Vec<u8>>,
}

impl SpreadsheetResult {
    fn read_only(response: DesignerResponse) -> Self {
        Self {
            text_capacity: false,
            response,
            candidate: None,
            export: None,
        }
    }
}

fn import_source(
    source: &[u8],
    format: SpreadsheetFormat,
    options: &ImportOptions,
) -> Result<SourceWorkbook, DesignerError> {
    if source.is_empty() || source.len() > MAX_SOURCE_BYTES {
        return Err(DesignerError::InvalidProjectTransfer {
            message: format!("Spreadsheet source must contain 1..={MAX_SOURCE_BYTES} bytes"),
        });
    }
    match format {
        SpreadsheetFormat::Csv => import_csv(source, options),
        SpreadsheetFormat::Xlsx => import_xlsx(source),
    }
    .map_err(interop_error)
}

fn interop_error(error: InteropError) -> DesignerError {
    match error {
        InteropError::Invalid(message) => DesignerError::InvalidProjectTransfer { message },
        InteropError::Capacity(capacity) => capacity.into(),
    }
}

fn spreadsheet_operation(
    runtime: Option<&DesignerRuntime>,
    operation: SpreadsheetOperation,
    project: &[u8],
) -> Result<SpreadsheetResult, DesignerError> {
    match operation {
        SpreadsheetOperation::Inspect {
            format,
            csv_options,
        } => {
            let workbook = import_source(project, format, &csv_options)?;
            let capacity =
                crate::interop_adapter::validate_text_capacity_inspection(&workbook).is_ok();
            let mut result =
                SpreadsheetResult::read_only(DesignerResponse::ImportPreview(Box::new(workbook)));
            result.text_capacity = capacity;
            Ok(result)
        }
        SpreadsheetOperation::Import {
            format,
            csv_options,
            selection,
            occurrence_id,
            install,
        } => {
            let workbook = import_source(project, format, &csv_options)?;
            let (candidate, imported) = import_workbook(&workbook, &selection, &occurrence_id)?;
            Ok(SpreadsheetResult {
                text_capacity: candidate.text_capacity,
                response: DesignerResponse::Imported(Box::new(imported)),
                candidate: install.then_some(candidate),
                export: None,
            })
        }
        SpreadsheetOperation::InspectProject { metadata } => {
            let (opened, text_capacity) =
                crate::interop_document::inspect_imported_project_with_profile(project, &metadata)?;
            let mut result =
                SpreadsheetResult::read_only(DesignerResponse::Opened(Box::new(opened)));
            result.text_capacity = text_capacity;
            Ok(result)
        }
        SpreadsheetOperation::OpenProject {
            occurrence_id,
            metadata,
        } => {
            let (candidate, opened) =
                crate::admit_imported_project(project, &occurrence_id, &metadata)?;
            Ok(SpreadsheetResult {
                text_capacity: candidate.text_capacity,
                response: DesignerResponse::Opened(Box::new(opened)),
                candidate: Some(candidate),
                export: None,
            })
        }
        SpreadsheetOperation::Export {
            expected_revision,
            metadata,
            format,
            collection,
        } => {
            let runtime = runtime.ok_or_else(|| DesignerError::InvalidProjectTransfer {
                message: "No Designer project is open for spreadsheet export.".to_owned(),
            })?;
            export_spreadsheet(runtime, &expected_revision, &metadata, format, &collection)
        }
        SpreadsheetOperation::ExportNativeTables {
            expected_revision,
            collection,
            format,
        } => export_native_tables_spreadsheet(runtime, &expected_revision, &collection, format),
        SpreadsheetOperation::ExportNativeTracker {
            expected_revision,
            presentation,
            format,
        } => export_native_tracker_spreadsheet(
            runtime.ok_or_else(|| DesignerError::InvalidProjectTransfer {
                message: "No Designer project is open for native Tracker spreadsheet export."
                    .to_owned(),
            })?,
            &expected_revision,
            &presentation,
            format,
        ),
        SpreadsheetOperation::ExportNativeBudget {
            expected_revision,
            presentation,
            format,
        } => export_native_budget_spreadsheet(
            runtime.ok_or_else(|| DesignerError::InvalidProjectTransfer {
                message: "No Designer project is open for native Budget spreadsheet export."
                    .to_owned(),
            })?,
            &expected_revision,
            &presentation,
            format,
        ),
    }
}

fn export_native_tables_spreadsheet(
    runtime: Option<&DesignerRuntime>,
    revision: &str,
    collection: &str,
    format: SpreadsheetFormat,
) -> Result<SpreadsheetResult, DesignerError> {
    let runtime = runtime.ok_or_else(|| DesignerError::InvalidProjectTransfer {
        message: "No Designer project is open for native tables export.".into(),
    })?;
    let (response, bytes) = crate::native_tables_export::encode_native_tables_spreadsheet(
        runtime,
        revision,
        collection,
        matches!(format, SpreadsheetFormat::Csv),
    )?;
    Ok(SpreadsheetResult {
        text_capacity: runtime.text_capacity,
        response,
        candidate: None,
        export: Some(bytes),
    })
}

fn export_native_tracker_spreadsheet(
    runtime: &DesignerRuntime,
    revision: &str,
    presentation: &NativeTrackerExportPresentation,
    format: SpreadsheetFormat,
) -> Result<SpreadsheetResult, DesignerError> {
    let mut workbook = runtime.export_native_tracker_workbook(revision, presentation)?;
    workbook.ledger.push(FidelityFinding {
        category: FidelityCategory::NativeEquivalent,
        code: "native_tracker_outbound_profile".to_owned(),
        location: "Tracker".to_owned(),
        message: "Native Tracker export maps every accepted task, estimate, and done row by stable identity. It supports 0..=128 outbound rows; the separate incoming spreadsheet profile remains limited to 64 rows.".to_owned(),
        blocking: false,
    });
    workbook.ledger.push(FidelityFinding {
        category: FidelityCategory::LossyOnExport,
        code: "native_tracker_session_presentation".to_owned(),
        location: "Tracker".to_owned(),
        message: "Browser-only column widths, row height, header emphasis, and session Undo/Redo history have no declared spreadsheet mapping and are not exported. Supported per-cell bold, fill, wrap, border, alignment, and number formatting are preserved when valid.".to_owned(),
        blocking: false,
    });
    let bytes = match format {
        SpreadsheetFormat::Csv => {
            let sheet =
                workbook
                    .sheets
                    .first()
                    .ok_or_else(|| DesignerError::InvalidProjectTransfer {
                        message: "Native Tracker export has no worksheet.".to_owned(),
                    })?;
            let bytes = export_csv_for_profile(sheet, OutputProfile::NativeTracker)
                .map_err(interop_error)?;
            workbook.ledger.push(FidelityFinding {
                category: FidelityCategory::LossyOnExport,
                code: "csv_values_only".to_owned(),
                location: sheet.name.clone(),
                message: "CSV exports Tracker scalar values only. Formatting is not preserved, and formula-leading Text is refused rather than changed; use typed XLSX for that Text.".to_owned(),
                blocking: false,
            });
            bytes
        }
        SpreadsheetFormat::Xlsx => export_xlsx_for_profile(&workbook, OutputProfile::NativeTracker)
            .map_err(interop_error)?,
    };
    crate::enforce_project_transfer_limit(bytes.len())?;
    Ok(SpreadsheetResult {
        text_capacity: runtime.text_capacity,
        response: DesignerResponse::SpreadsheetExported(SpreadsheetExportProjection {
            revision: revision.to_owned(),
            byte_length: bytes.len(),
            ledger: workbook.ledger,
        }),
        candidate: None,
        export: Some(bytes),
    })
}

fn export_native_budget_spreadsheet(
    runtime: &DesignerRuntime,
    revision: &str,
    presentation: &NativeBudgetExportPresentation,
    format: SpreadsheetFormat,
) -> Result<SpreadsheetResult, DesignerError> {
    let mut workbook = match format {
        SpreadsheetFormat::Csv => {
            runtime.export_native_budget_csv_workbook(revision, presentation)?
        }
        SpreadsheetFormat::Xlsx => runtime.export_native_budget_workbook(revision, presentation)?,
    };
    if matches!(format, SpreadsheetFormat::Xlsx) {
        workbook.ledger.push(FidelityFinding {
            category: FidelityCategory::NativeEquivalent,
            code: "native_budget_outbound_profile".to_owned(),
            location: "Budget".to_owned(),
            message: "Native Budget export maps every admitted source collection, row, and formula dependency by stable identity. View aliases remain presentation-only and do not duplicate canonical worksheet data.".to_owned(),
            blocking: false,
        });
    }
    workbook.ledger.push(FidelityFinding {
        category: FidelityCategory::LossyOnExport,
        code: "native_budget_session_presentation".to_owned(),
        location: "Budget".to_owned(),
        message: "Browser-only view state, session Undo/Redo history, and chart composition have no declared spreadsheet mapping and are not exported. Supported per-cell styles are preserved when valid.".to_owned(),
        blocking: false,
    });
    let bytes = match format {
        SpreadsheetFormat::Csv => {
            let sheet =
                workbook
                    .sheets
                    .first()
                    .ok_or_else(|| DesignerError::InvalidProjectTransfer {
                        message: "Native Budget export active view is unavailable.".to_owned(),
                    })?;
            let bytes = export_csv(sheet).map_err(interop_error)?;
            workbook.ledger.push(FidelityFinding {
                category: FidelityCategory::LossyOnExport,
                code: "csv_values_only".to_owned(),
                location: sheet.name.clone(),
                message: "CSV exports the active Budget view's calculated scalar values only. Formula definitions, other source collections, presentation formatting, aliases, and charts are not preserved; retain the source project.".to_owned(),
                blocking: false,
            });
            bytes
        }
        SpreadsheetFormat::Xlsx => export_xlsx(&workbook).map_err(interop_error)?,
    };
    crate::enforce_project_transfer_limit(bytes.len())?;
    Ok(SpreadsheetResult {
        text_capacity: runtime.text_capacity,
        response: DesignerResponse::SpreadsheetExported(SpreadsheetExportProjection {
            revision: revision.to_owned(),
            byte_length: bytes.len(),
            ledger: workbook.ledger,
        }),
        candidate: None,
        export: Some(bytes),
    })
}

fn export_spreadsheet(
    runtime: &DesignerRuntime,
    revision: &str,
    metadata: &InteropMetadata,
    format: SpreadsheetFormat,
    collection: &str,
) -> Result<SpreadsheetResult, DesignerError> {
    // XLSX's declared import profile requires a header. Insert one only in
    // the output mapping so bound formulas project to the shifted rows;
    // neither the source metadata nor the resident document is changed.
    let mut export_metadata = metadata.clone();
    let mut inserted_headers = Vec::new();
    if matches!(format, SpreadsheetFormat::Xlsx) && !runtime.text_capacity {
        for sheet in &mut export_metadata.sheets {
            if !sheet.has_header {
                sheet.has_header = true;
                inserted_headers.push(sheet.name.clone());
            }
        }
    }
    let mut workbook = runtime.export_imported_workbook(
        revision,
        &export_metadata,
        match format {
            SpreadsheetFormat::Csv => crate::interop_document::ImportedOutput::Csv(collection),
            SpreadsheetFormat::Xlsx => crate::interop_document::ImportedOutput::Xlsx,
        },
    )?;
    for name in inserted_headers {
        workbook.ledger.push(FidelityFinding {
            category: FidelityCategory::Converted,
            code: "xlsx_header_inserted".to_owned(),
            location: name,
            message: "XLSX output inserts the declared column names as a header row and adjusts formula coordinates. The source project and its original header setting are unchanged.".to_owned(),
            blocking: false,
        });
    }
    let bytes = match format {
        SpreadsheetFormat::Csv => {
            let sheet = &workbook.sheets[0];
            let bytes = export_csv(sheet).map_err(|error| {
                crate::interop_document::imported_export_error("workbook", &error.to_string())
            })?;
            workbook.ledger.push(FidelityFinding {
                category: FidelityCategory::LossyOnExport,
                code: "csv_values_only".to_owned(),
                location: sheet.name.clone(),
                message: "CSV exports selected-sheet scalar values only. Blank and present empty Text both become empty CSV fields. Formula definitions, presentation formatting, and other sheets are not preserved; retain the source project.".to_owned(),
                blocking: false,
            });
            bytes
        }
        SpreadsheetFormat::Xlsx => export_xlsx(&workbook).map_err(|error| {
            crate::interop_document::imported_export_error("workbook", &error.to_string())
        })?,
    };
    crate::enforce_project_transfer_limit(bytes.len())?;
    Ok(SpreadsheetResult {
        text_capacity: runtime.text_capacity,
        response: DesignerResponse::SpreadsheetExported(SpreadsheetExportProjection {
            revision: revision.to_owned(),
            byte_length: bytes.len(),
            ledger: workbook.ledger,
        }),
        candidate: None,
        export: Some(bytes),
    })
}

fn too_large_project(runtime: Option<&DesignerRuntime>) -> DesignerWireReply {
    DesignerWireReply::Error {
        error: DesignerError::ProjectTransferTooLarge {
            actual: MAX_PROJECT_TRANSFER_BYTES.saturating_add(1),
            maximum: MAX_PROJECT_TRANSFER_BYTES,
        }
        .failure_projection(current_revision(runtime)),
    }
}

fn no_project_open() -> DesignerWireReply {
    DesignerWireReply::Error {
        error: FailureProjection {
            code: "no_project_open".to_owned(),
            message: "No Designer project is open.".to_owned(),
            current_revision: "unavailable".to_owned(),
            diagnostics: Vec::new(),
            name_failures: Vec::new(),
            constraint_failure: None,
            row_count: None,
            capacity: None,
        },
    }
}

trait IntoProjectExport {
    fn into_project_export(self) -> crate::ProjectExport;
}

impl IntoProjectExport for crate::ProjectExport {
    fn into_project_export(self) -> crate::ProjectExport {
        self
    }
}

impl IntoProjectExport for crate::CanonicalTreeExport {
    fn into_project_export(self) -> crate::ProjectExport {
        crate::ProjectExport {
            revision: self.revision,
            bytes: self.bytes,
        }
    }
}

fn current_revision(runtime: Option<&DesignerRuntime>) -> &str {
    runtime.map_or("unavailable", DesignerRuntime::current_revision)
}
