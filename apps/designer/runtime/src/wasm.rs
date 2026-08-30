//! Browser-only private byte bridge for the app-local Designer Worker.

use std::cell::{Cell, RefCell};

use crate::{
    DesignerError, DesignerResponse, DesignerRuntime, DesignerWireReply,
    MAX_PROJECT_TRANSFER_BYTES, MAX_WIRE_REQUEST_BYTES, ProjectExportProjection, encode_reply,
    open_project, process_wire_request, request_too_large_reply,
};

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
