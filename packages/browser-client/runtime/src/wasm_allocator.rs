//! Issue #678: amortize WASM memory growth through the existing allocator.
#![cfg(target_arch = "wasm32")]

use std::alloc::{GlobalAlloc, Layout, System};

struct GranularAllocator;

#[global_allocator]
static ALLOCATOR: GranularAllocator = GranularAllocator;

fn replenish(previous_pages: usize, pointer: *mut u8) {
    let current_pages = core::arch::wasm32::memory_size::<0>();
    if pointer.is_null() || current_pages <= previous_pages {
        return;
    }
    // Reuse System's free list instead of growing linear memory behind its back.
    // Leave 8 MiB below the unchanged raw-ABI budget for allocator rounding.
    let remaining = (248 * 1024 * 1024usize).saturating_sub(current_pages * 64 * 1024);
    let size = remaining.min(4 * 1024 * 1024);
    if size == 0 {
        return;
    }
    // This unused byte reserve needs no over-alignment (or memalign chunk splitting).
    let layout = Layout::from_size_align(size, 1).unwrap();
    // SAFETY: layout is nonzero and valid. This temporary allocation is never
    // exposed or accessed, and is freed exactly once with its original layout.
    // Null means optional headroom failed; the caller's allocation is untouched.
    unsafe {
        let reserve = System.alloc(layout);
        if !reserve.is_null() {
            System.dealloc(reserve, layout);
        }
    }
}

// SAFETY: every caller allocation is delegated unchanged to System, including
// its alignment/layout and ownership. Replenishment uses a separate allocation
// and never accesses caller memory. No allocator state or lock is retained.
unsafe impl GlobalAlloc for GranularAllocator {
    // SAFETY: callers must supply the nonzero valid layout required by GlobalAlloc.
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pages = core::arch::wasm32::memory_size::<0>();
        // SAFETY: forward the caller's valid layout unchanged to System.
        let pointer = unsafe { System.alloc(layout) };
        replenish(pages, pointer);
        pointer
    }

    // SAFETY: callers must supply the nonzero valid layout required by GlobalAlloc.
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pages = core::arch::wasm32::memory_size::<0>();
        // SAFETY: System zeroes the caller's valid layout without changing it.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        replenish(pages, pointer);
        pointer
    }

    // SAFETY: pointer must be live and owned, with its original allocation layout.
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: pointer/layout came from System; transfer ownership back once.
        unsafe { System.dealloc(pointer, layout) };
    }

    // SAFETY: pointer/layout must name a live allocation; new_size must satisfy
    // GlobalAlloc's nonzero size and representable-layout requirements.
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let pages = core::arch::wasm32::memory_size::<0>();
        // SAFETY: delegate the original pointer/layout and new size unchanged.
        // System preserves the old allocation on failure and its alignment on success.
        let resized = unsafe { System.realloc(pointer, layout, new_size) };
        replenish(pages, resized);
        resized
    }
}
