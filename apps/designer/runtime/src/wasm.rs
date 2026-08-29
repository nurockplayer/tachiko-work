//! Browser-only private byte bridge for the app-local Designer Worker.

use std::cell::RefCell;

use crate::{DesignerRuntime, process_wire_request};

thread_local! {
    static REQUEST: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static RESPONSE: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static RUNTIME: RefCell<Option<DesignerRuntime>> = const { RefCell::new(None) };
}

/// Resize the private request arena and return its linear-memory offset.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_designer_request_reserve(length: u32) -> u32 {
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
            let response = process_wire_request(&mut runtime.borrow_mut(), &request.borrow());
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
