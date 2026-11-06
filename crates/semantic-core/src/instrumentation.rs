//! Optional capacity-instrumentation counters.
//!
//! The counters exist to measure the existing resident publication path for
//! capacity engineering (Plan A-01). They are not semantic identity, a public
//! contract, or a performance guarantee.
//!
//! With the `capacity-instrumentation` feature disabled — the default, and the
//! only configuration for release WASM — every recorder below is an empty
//! inline function, so call sites compile away to nothing.
//!
//! This branch also carries unconditional wall-clock phase timers (Plan A-06b
//! scratch measurement). They are thread-local and never part of the product
//! surface; remove them before any merge.

use std::cell::RefCell;
use std::time::Instant;

thread_local! {
    /// Completed phases for the current measurement window: label and
    /// microseconds. Nested phases each record their own inclusive duration.
    static PHASES: RefCell<Vec<(&'static str, u128)>> = const { RefCell::new(Vec::new()) };
    /// Phases currently open, innermost last.
    static PHASE_STACK: RefCell<Vec<(&'static str, Instant)>> = const { RefCell::new(Vec::new()) };
}

/// Discard every completed phase and any still-open one.
pub fn reset_phases() {
    PHASES.with(|phases| phases.borrow_mut().clear());
    PHASE_STACK.with(|stack| stack.borrow_mut().clear());
}

/// Open one named phase on the calling thread's stack.
pub fn phase_start(name: &'static str) {
    PHASE_STACK.with(|stack| stack.borrow_mut().push((name, Instant::now())));
}

/// Close the innermost open phase and record its inclusive duration.
pub fn phase_end() {
    let opened = PHASE_STACK.with(|stack| stack.borrow_mut().pop());
    if let Some((name, started)) = opened {
        let micros = started.elapsed().as_micros();
        PHASES.with(|phases| phases.borrow_mut().push((name, micros)));
    }
}

/// Take the completed phases recorded since the last reset.
#[must_use]
pub fn take_phases() -> Vec<(&'static str, u128)> {
    PHASES.with(|phases| std::mem::take(&mut *phases.borrow_mut()))
}

/// RAII phase timer: opens on construction, closes on drop.
pub struct PhaseTimer {
    _private: (),
}

impl PhaseTimer {
    /// Open the named phase until the returned timer is dropped.
    #[must_use]
    pub fn new(name: &'static str) -> Self {
        phase_start(name);
        Self { _private: () }
    }
}

impl Drop for PhaseTimer {
    fn drop(&mut self) {
        phase_end();
    }
}

/// Cumulative counters recorded by one instrumented process.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CapacityCounters {
    /// Every full [`Document`](crate::Document) clone.
    pub document_clones: u64,
    /// Entities visited by the publication invalidation scan.
    pub publication_entities_inspected: u64,
    /// Fields compared by the publication invalidation scan.
    pub publication_fields_inspected: u64,
    /// Full-document equality comparisons attempted by publication.
    pub publication_document_comparisons: u64,
    /// JSON bytes emitted by projection serialization checks.
    pub json_bytes_serialized: u64,
}

/// Read the current cumulative counters.
#[must_use]
pub fn counters() -> CapacityCounters {
    enabled::counters()
}

/// Reset every counter to zero.
pub fn reset() {
    enabled::reset();
}

/// Record one full document clone.
#[inline]
pub fn record_document_clone() {
    enabled::record_document_clone();
}

/// Record entities visited by the publication invalidation scan.
#[inline]
pub fn record_publication_entities_inspected(count: u64) {
    enabled::record_publication_entities_inspected(count);
}

/// Record fields compared by the publication invalidation scan.
#[inline]
pub fn record_publication_fields_inspected(count: u64) {
    enabled::record_publication_fields_inspected(count);
}

/// Record one attempted full-document equality comparison.
#[inline]
pub fn record_publication_document_comparison() {
    enabled::record_publication_document_comparison();
}

/// Record JSON bytes emitted by one projection serialization check.
#[inline]
pub fn record_json_bytes(count: u64) {
    enabled::record_json_bytes(count);
}

#[cfg(feature = "capacity-instrumentation")]
#[allow(clippy::missing_const_for_fn)] // atomic operations are not const
mod enabled {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::CapacityCounters;

    static DOCUMENT_CLONES: AtomicU64 = AtomicU64::new(0);
    static PUBLICATION_ENTITIES_INSPECTED: AtomicU64 = AtomicU64::new(0);
    static PUBLICATION_FIELDS_INSPECTED: AtomicU64 = AtomicU64::new(0);
    static PUBLICATION_DOCUMENT_COMPARISONS: AtomicU64 = AtomicU64::new(0);
    static JSON_BYTES_SERIALIZED: AtomicU64 = AtomicU64::new(0);

    pub(super) fn counters() -> CapacityCounters {
        CapacityCounters {
            document_clones: DOCUMENT_CLONES.load(Ordering::Relaxed),
            publication_entities_inspected: PUBLICATION_ENTITIES_INSPECTED.load(Ordering::Relaxed),
            publication_fields_inspected: PUBLICATION_FIELDS_INSPECTED.load(Ordering::Relaxed),
            publication_document_comparisons: PUBLICATION_DOCUMENT_COMPARISONS
                .load(Ordering::Relaxed),
            json_bytes_serialized: JSON_BYTES_SERIALIZED.load(Ordering::Relaxed),
        }
    }

    pub(super) fn reset() {
        DOCUMENT_CLONES.store(0, Ordering::Relaxed);
        PUBLICATION_ENTITIES_INSPECTED.store(0, Ordering::Relaxed);
        PUBLICATION_FIELDS_INSPECTED.store(0, Ordering::Relaxed);
        PUBLICATION_DOCUMENT_COMPARISONS.store(0, Ordering::Relaxed);
        JSON_BYTES_SERIALIZED.store(0, Ordering::Relaxed);
    }

    pub(super) fn record_document_clone() {
        DOCUMENT_CLONES.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn record_publication_entities_inspected(count: u64) {
        PUBLICATION_ENTITIES_INSPECTED.fetch_add(count, Ordering::Relaxed);
    }

    pub(super) fn record_publication_fields_inspected(count: u64) {
        PUBLICATION_FIELDS_INSPECTED.fetch_add(count, Ordering::Relaxed);
    }

    pub(super) fn record_publication_document_comparison() {
        PUBLICATION_DOCUMENT_COMPARISONS.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn record_json_bytes(count: u64) {
        JSON_BYTES_SERIALIZED.fetch_add(count, Ordering::Relaxed);
    }
}

#[cfg(not(feature = "capacity-instrumentation"))]
mod enabled {
    use super::CapacityCounters;

    pub(super) fn counters() -> CapacityCounters {
        CapacityCounters::default()
    }

    pub(super) const fn reset() {}

    pub(super) const fn record_document_clone() {}

    pub(super) const fn record_publication_entities_inspected(_count: u64) {}

    pub(super) const fn record_publication_fields_inspected(_count: u64) {}

    pub(super) const fn record_publication_document_comparison() {}

    pub(super) const fn record_json_bytes(_count: u64) {}
}
