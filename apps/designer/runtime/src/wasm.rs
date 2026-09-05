//! Browser-only private byte bridge for the app-local Designer Worker.

use std::cell::{Cell, RefCell};

use serde::Deserialize;

use crate::interop_adapter::{
    FidelityCategory, FidelityFinding, ImportOptions, InteropError, MAX_SOURCE_BYTES,
    SourceWorkbook, export_csv, export_xlsx, import_csv, import_xlsx,
};
use crate::{
    DesignerError, DesignerResponse, DesignerRuntime, DesignerWireReply,
    MAX_PROJECT_TRANSFER_BYTES, MAX_WIRE_REQUEST_BYTES, ProjectExportProjection, encode_reply,
    inspect_project, open_project, process_wire_request, request_too_large_reply,
};
use crate::{
    ImportSelection, InteropMetadata, SpreadsheetExportProjection, ensure_projection_size,
    import_workbook, inspect_imported_project,
};

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
    Export {
        expected_revision: String,
        metadata: InteropMetadata,
        format: SpreadsheetFormat,
        collection: String,
    },
}

struct SpreadsheetResult {
    response: DesignerResponse,
    candidate: Option<DesignerRuntime>,
    export: Option<Vec<u8>>,
}

impl SpreadsheetResult {
    fn read_only(response: DesignerResponse) -> Self {
        Self {
            response,
            candidate: None,
            export: None,
        }
    }
}

thread_local! {
    static REQUEST: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static RESPONSE: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static PROJECT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static RUNTIME: RefCell<Option<DesignerRuntime>> = const { RefCell::new(None) };
    static REQUEST_TOO_LARGE: Cell<bool> = const { Cell::new(false) };
    static PROJECT_TOO_LARGE: Cell<bool> = const { Cell::new(false) };
}

/// Resize the private request arena and return its linear-memory offset.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_designer_request_reserve(length: u32) -> u32 {
    if length as usize > MAX_WIRE_REQUEST_BYTES {
        REQUEST_TOO_LARGE.set(true);
        REQUEST.with(|request| request.borrow_mut().clear());
        return 0;
    }
    REQUEST_TOO_LARGE.set(false);
    REQUEST.with(|request| {
        let mut request = request.borrow_mut();
        request.resize(length as usize, 0);
        request.as_mut_ptr() as usize as u32
    })
}

/// Execute the private request bytes most recently written by the Worker.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_designer_request_run() {
    REQUEST.with(|request| {
        RUNTIME.with(|runtime| {
            let mut runtime = runtime.borrow_mut();
            let response = if REQUEST_TOO_LARGE.replace(false) {
                request_too_large_reply(runtime.as_ref())
            } else {
                process_wire_request(&mut runtime, &request.borrow())
            };
            RESPONSE.with(|slot| *slot.borrow_mut() = response);
        });
    });
}

/// Return the linear-memory offset of the current response arena.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_designer_response_ptr() -> u32 {
    RESPONSE.with(|response| response.borrow().as_ptr() as usize as u32)
}

/// Return the current response length in bytes.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_designer_response_len() -> u32 {
    RESPONSE.with(|response| response.borrow().len() as u32)
}

/// Resize the separate bounded project-transfer arena.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_designer_project_reserve(length: u32) -> u32 {
    if length as usize > MAX_PROJECT_TRANSFER_BYTES {
        PROJECT_TOO_LARGE.set(true);
        PROJECT.with(|project| *project.borrow_mut() = Vec::new());
        return 0;
    }
    PROJECT_TOO_LARGE.set(false);
    PROJECT.with(|project| {
        let mut project = project.borrow_mut();
        project.resize(length as usize, 0);
        project.as_mut_ptr() as usize as u32
    })
}

/// Fully admit and install one project candidate from the project arena.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_designer_project_open() {
    process_project_candidate(true);
}

/// Inspect a fully admitted candidate without replacing resident state.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_designer_project_inspect() {
    process_project_candidate(false);
}

/// Run one private JSON spreadsheet operation with source bytes in PROJECT.
/// Admission and response bounds are checked before installing any candidate.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_designer_spreadsheet_run() {
    let request_too_large = REQUEST_TOO_LARGE.replace(false);
    let project_too_large = PROJECT_TOO_LARGE.replace(false);
    let request = REQUEST.with(|slot| std::mem::take(&mut *slot.borrow_mut()));
    let project = PROJECT.with(|slot| std::mem::take(&mut *slot.borrow_mut()));
    RUNTIME.with(|slot| {
        let mut runtime = slot.borrow_mut();
        if request_too_large || request.len() > MAX_WIRE_REQUEST_BYTES {
            RESPONSE.with(|response| {
                *response.borrow_mut() = request_too_large_reply(runtime.as_ref());
            });
            return;
        }
        let result = if project_too_large || project.len() > MAX_PROJECT_TRANSFER_BYTES {
            Err(DesignerError::ProjectTransferTooLarge {
                actual: MAX_PROJECT_TRANSFER_BYTES.saturating_add(1),
                maximum: MAX_PROJECT_TRANSFER_BYTES,
            })
        } else {
            serde_json::from_slice::<SpreadsheetOperation>(&request)
                .map_err(|error| DesignerError::InvalidProjectTransfer {
                    message: format!("Invalid spreadsheet operation: {error}"),
                })
                .and_then(|operation| spreadsheet_operation(runtime.as_ref(), operation, &project))
        };
        let reply = match result {
            Ok(result) => {
                let reply = DesignerWireReply::Ok {
                    response: result.response,
                };
                if let Err(error) = ensure_projection_size(&reply) {
                    DesignerWireReply::Error {
                        error: error.failure_projection(current_revision(runtime.as_ref())),
                    }
                } else {
                    if let Some(candidate) = result.candidate {
                        *runtime = Some(candidate);
                    }
                    if let Some(bytes) = result.export {
                        PROJECT.with(|slot| *slot.borrow_mut() = bytes);
                    }
                    reply
                }
            }
            Err(error) => DesignerWireReply::Error {
                error: error.failure_projection(current_revision(runtime.as_ref())),
            },
        };
        set_response(&reply);
    });
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
    DesignerError::InvalidProjectTransfer { message: error.0 }
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
            Ok(SpreadsheetResult::read_only(
                DesignerResponse::ImportPreview(Box::new(workbook)),
            ))
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
                response: DesignerResponse::Imported(Box::new(imported)),
                candidate: install.then_some(candidate),
                export: None,
            })
        }
        SpreadsheetOperation::InspectProject { metadata } => {
            let opened = inspect_imported_project(project, &metadata)?;
            Ok(SpreadsheetResult::read_only(DesignerResponse::Opened(
                Box::new(opened),
            )))
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
    }
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
    if matches!(format, SpreadsheetFormat::Xlsx) {
        for sheet in &mut export_metadata.sheets {
            if !sheet.has_header {
                sheet.has_header = true;
                inserted_headers.push(sheet.name.clone());
            }
        }
    }
    let mut workbook = runtime.export_workbook(revision, &export_metadata)?;
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
            let index = metadata
                .sheets
                .iter()
                .position(|sheet| sheet.schema_id == collection)
                .ok_or_else(|| DesignerError::InvalidProjectTransfer {
                    message: "CSV export requires a known stable collection ID.".to_owned(),
                })?;
            let sheet = workbook.sheets.get(index).ok_or_else(|| {
                DesignerError::InvalidProjectTransfer {
                    message: "CSV export collection is unavailable.".to_owned(),
                }
            })?;
            let bytes = export_csv(sheet).map_err(interop_error)?;
            workbook.ledger.push(FidelityFinding {
                category: FidelityCategory::LossyOnExport,
                code: "csv_values_only".to_owned(),
                location: sheet.name.clone(),
                message: "CSV exports selected-sheet scalar values only. Formula definitions, presentation formatting, and other sheets are not preserved; retain the source project.".to_owned(),
                blocking: false,
            });
            bytes
        }
        SpreadsheetFormat::Xlsx => export_xlsx(&workbook).map_err(interop_error)?,
    };
    crate::enforce_project_transfer_limit(bytes.len())?;
    Ok(SpreadsheetResult {
        response: DesignerResponse::SpreadsheetExported(SpreadsheetExportProjection {
            revision: revision.to_owned(),
            byte_length: bytes.len(),
            ledger: workbook.ledger,
        }),
        candidate: None,
        export: Some(bytes),
    })
}

fn process_project_candidate(install: bool) {
    let project = PROJECT.with(|project| std::mem::take(&mut *project.borrow_mut()));
    RUNTIME.with(|runtime| {
        let mut runtime = runtime.borrow_mut();
        let reply = if PROJECT_TOO_LARGE.replace(false) {
            DesignerWireReply::Error {
                error: DesignerError::ProjectTransferTooLarge {
                    actual: MAX_PROJECT_TRANSFER_BYTES.saturating_add(1),
                    maximum: MAX_PROJECT_TRANSFER_BYTES,
                }
                .failure_projection(current_revision(runtime.as_ref())),
            }
        } else if !install {
            match inspect_project(&project) {
                Ok(opened) => DesignerWireReply::Ok {
                    response: DesignerResponse::Opened(Box::new(opened)),
                },
                Err(error) => DesignerWireReply::Error {
                    error: error.failure_projection(current_revision(runtime.as_ref())),
                },
            }
        } else if REQUEST_TOO_LARGE.replace(false) {
            DesignerWireReply::Error {
                error: DesignerError::InvalidOccurrenceIdentity
                    .failure_projection(current_revision(runtime.as_ref())),
            }
        } else {
            REQUEST.with(|request| {
                let request = request.borrow();
                let Ok(occurrence_id) = std::str::from_utf8(&request) else {
                    return DesignerWireReply::Error {
                        error: DesignerError::InvalidOccurrenceIdentity
                            .failure_projection(current_revision(runtime.as_ref())),
                    };
                };
                match open_project(&mut runtime, &project, occurrence_id) {
                    Ok(opened) => DesignerWireReply::Ok {
                        response: DesignerResponse::Opened(Box::new(opened)),
                    },
                    Err(error) => DesignerWireReply::Error {
                        error: error.failure_projection(current_revision(runtime.as_ref())),
                    },
                }
            })
        };
        set_response(&reply);
    });
}

/// Encode the exact expected resident revision into the project arena.
///
/// The caller writes the UTF-8 expected revision into the normal request
/// arena first. Successful export replaces the project arena with an opaque
/// canonical project bundle.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_designer_project_export() {
    RUNTIME.with(|runtime| {
        let runtime = runtime.borrow();
        let reply = if let Some(runtime) = runtime.as_ref() {
            REQUEST.with(|request| {
                let request = request.borrow();
                match std::str::from_utf8(&request) {
                    Ok(expected_revision) => match runtime.export_project(expected_revision) {
                        Ok(export) => {
                            let byte_length = export.bytes.len();
                            PROJECT.with(|project| *project.borrow_mut() = export.bytes);
                            DesignerWireReply::Ok {
                                response: DesignerResponse::ProjectExported(
                                    ProjectExportProjection {
                                        revision: export.revision,
                                        byte_length,
                                    },
                                ),
                            }
                        }
                        Err(error) => DesignerWireReply::Error {
                            error: error.failure_projection(runtime.current_revision()),
                        },
                    },
                    Err(_) => DesignerWireReply::Error {
                        error: DesignerError::InvalidProjectTransfer {
                            message: "expected revision is not UTF-8".to_owned(),
                        }
                        .failure_projection(runtime.current_revision()),
                    },
                }
            })
        } else {
            DesignerWireReply::Error {
                error: crate::FailureProjection {
                    code: "no_project_open".to_owned(),
                    message: "No Designer project is open.".to_owned(),
                    current_revision: "unavailable".to_owned(),
                    diagnostics: Vec::new(),
                },
            }
        };
        set_response(&reply);
    });
}

/// Release project bytes after the Worker has copied an export receipt.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_designer_project_release() {
    PROJECT.with(|project| *project.borrow_mut() = Vec::new());
}

/// Destroy the current occurrence and release transient project bytes.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_designer_project_close() {
    RUNTIME.with(|runtime| *runtime.borrow_mut() = None);
    tachiko_designer_project_release();
}

/// Return the current project-transfer arena offset.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_designer_project_ptr() -> u32 {
    PROJECT.with(|project| project.borrow().as_ptr() as usize as u32)
}

/// Return the current project-transfer arena length.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_designer_project_len() -> u32 {
    PROJECT.with(|project| project.borrow().len() as u32)
}

fn current_revision(runtime: Option<&DesignerRuntime>) -> &str {
    runtime.map_or("unavailable", DesignerRuntime::current_revision)
}

fn set_response(reply: &DesignerWireReply) {
    RESPONSE.with(|response| *response.borrow_mut() = encode_reply(reply));
}
