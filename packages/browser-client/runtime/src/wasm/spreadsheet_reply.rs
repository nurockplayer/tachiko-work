use super::{PROJECT, RESPONSE, RETAINED_SOURCE, SpreadsheetResult, current_revision};
use crate::source_reuse::InspectedSource;
use crate::{DesignerRuntime, DesignerWireReply, encode_reply, encode_sized_reply};

pub(super) fn set_success(runtime: &mut Option<DesignerRuntime>, result: SpreadsheetResult) {
    let reply = DesignerWireReply::Ok {
        response: result.response,
    };
    // The response arena owns its allocation across spreadsheet calls;
    // an inspect must not allocate another preview-sized output buffer.
    let mut buffer = RESPONSE.with(|response| std::mem::take(&mut *response.borrow_mut()));
    if let Some((_, size @ 1..)) = &result.inspected_source {
        // Use the admission byte count to allocate the cold arena once.
        buffer.clear();
        buffer.reserve_exact(size + crate::REPLY_ENVELOPE_RESERVE);
    }
    let encoded = match encode_sized_reply(&reply, result.profile, buffer) {
        Err(error) => encode_reply(&DesignerWireReply::Error {
            error: error.failure_projection(current_revision(runtime.as_ref())),
        }),
        Ok(encoded) => {
            if let Some(candidate) = result.candidate {
                *runtime = Some(candidate);
            }
            if let Some(bytes) = result.export {
                PROJECT.with(|slot| *slot.borrow_mut() = bytes);
            }
            if let Some((identity, _)) = result.inspected_source {
                RETAINED_SOURCE.with(|retained| {
                    *retained.borrow_mut() = Some(InspectedSource::from_preview(identity, reply));
                });
            }
            encoded
        }
    };
    RESPONSE.with(|response| *response.borrow_mut() = encoded);
}
