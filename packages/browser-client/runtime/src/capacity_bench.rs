//! Capacity Stage A (Plan A-01) baseline harness.
//!
//! This module is test-only and never part of the product surface. It measures
//! the existing resident publication, projection and persistence paths against
//! deterministic fixtures larger than the current admission profile.
//!
//! Run it explicitly in release, with the counter hooks enabled:
//!
//! ```text
//! cargo test --release \
//!   --manifest-path packages/browser-client/runtime/Cargo.toml \
//!   --features capacity-bench -- --ignored --test-threads=1 --nocapture
//! ```
//!
//! The counting allocator lives in this unit-test binary only. Every
//! `#[global_allocator]` and `unsafe impl` below is confined to
//! `cfg(all(test, feature = "capacity-bench"))`; production WASM and the
//! ordinary test profile use the system allocator and the no-op counter hooks.

use std::time::Instant;

use tachiko_storage::{decode_roproj_v3, encode_roproj_v3};
use tachiko_workspace_engine::{
    Document, FieldId, Number, Value,
    instrumentation::{self, CapacityCounters},
    patch_lifecycle::{DocumentScopeId, SemanticPublicationAuthority, TrustedInstant},
    resident_session::{ResidentWorkspaceSession, TrustedPublicationTimeSource},
    validate,
};

use super::{
    DESIGNER_PRINCIPAL, DesignerClock, DesignerRequest, DesignerRuntime, FieldTarget, PrincipalId,
    ScalarEditInput, collection_specs, decode_project_bundle, designer_lifecycle, document_scope,
    encode_project_bundle, ensure_projection_size_with_limit, formula_sources,
    select_default_collection,
};
use crate::capacity_fixtures::{
    DEPENDENT_FORMULAS, FixtureSpec, RATE_FIELD, capacity_document, entity_id, field_id,
};

/// Stage A fixtures: 1k, 10k and 100k grid cells at ten columns (about 91%
/// populated; the rest are deliberate blanks).
const STAGE_A_SIZES: [(usize, usize); 3] = [(100, 10), (1_000, 10), (10_000, 10)];
const EDIT_ITERATIONS: usize = 5;
/// Undo steps retained to answer the Priority 1 history-memory question.
///
/// This matches the runtime's own cap: the sixty-fifth edit evicts the oldest
/// entry, so undoing more steps than this fails by design.
const HISTORY_DEPTH: usize = 64;
const BULK_ITERATIONS: usize = 3;
const SEED: u64 = 0x00C0_FFEE_D00D_1234;
/// Largest fixture the ordinary runtime admission profile still accepts.
const RUNTIME_EDIT_ROWS: usize = 64;
const RUNTIME_EDIT_COLUMNS: usize = 5;

#[cfg(feature = "capacity-bench")]
mod counting_allocator {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        sync::atomic::{AtomicU64, Ordering},
    };

    static LIVE: AtomicU64 = AtomicU64::new(0);
    static PEAK: AtomicU64 = AtomicU64::new(0);

    /// Counting allocator used only by the capacity baseline test binary.
    pub struct Counting;

    // SAFETY: every operation delegates to `System`; the only additional work
    // is relaxed atomic accounting of requested layout bytes. The counters
    // cannot affect allocation addresses or sizes.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            // SAFETY: caller upholds the `GlobalAlloc::alloc` contract.
            let pointer = unsafe { System.alloc(layout) };
            if !pointer.is_null() {
                add(bytes_of(layout));
            }
            pointer
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            subtract(bytes_of(layout));
            // SAFETY: pointer and layout come from this allocator unchanged.
            unsafe { System.dealloc(pointer, layout) };
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            // SAFETY: caller upholds the `GlobalAlloc::alloc_zeroed` contract.
            let pointer = unsafe { System.alloc_zeroed(layout) };
            if !pointer.is_null() {
                add(bytes_of(layout));
            }
            pointer
        }

        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            // SAFETY: pointer and layout come from this allocator unchanged, and
            // the caller upholds the `GlobalAlloc::realloc` contract.
            let resized = unsafe { System.realloc(pointer, layout, new_size) };
            if !resized.is_null() {
                // A failed realloc keeps the original block, so only a success
                // changes the accounted live bytes.
                let before = bytes_of(layout);
                let after = u64::try_from(new_size).unwrap_or(u64::MAX);
                if after >= before {
                    add(after - before);
                } else {
                    subtract(before - after);
                }
            }
            resized
        }
    }

    fn bytes_of(layout: Layout) -> u64 {
        u64::try_from(layout.size()).unwrap_or(u64::MAX)
    }

    fn add(bytes: u64) {
        let live = LIVE
            .fetch_add(bytes, Ordering::Relaxed)
            .saturating_add(bytes);
        PEAK.fetch_max(live, Ordering::Relaxed);
    }

    fn subtract(bytes: u64) {
        LIVE.fetch_sub(bytes, Ordering::Relaxed);
    }

    pub fn live() -> u64 {
        LIVE.load(Ordering::Relaxed)
    }

    pub fn peak() -> u64 {
        PEAK.load(Ordering::Relaxed)
    }

    pub fn reset_peak() {
        PEAK.store(live(), Ordering::Relaxed);
    }
}

#[cfg(feature = "capacity-bench")]
#[global_allocator]
static CAPACITY_BENCH_ALLOCATOR: counting_allocator::Counting = counting_allocator::Counting;

#[cfg(feature = "capacity-bench")]
mod alloc_stats {
    pub use super::counting_allocator::{live, peak, reset_peak};
}

#[cfg(not(feature = "capacity-bench"))]
mod alloc_stats {
    pub const fn live() -> u64 {
        0
    }

    pub const fn peak() -> u64 {
        0
    }

    pub const fn reset_peak() {}
}

#[derive(Clone, Copy, Debug, Default)]
struct Sample {
    millis: f64,
    counters: CapacityCounters,
    peak_bytes: u64,
    formula_nodes: u64,
}

fn observe<R>(action: impl FnOnce() -> R) -> (R, Sample) {
    instrumentation::reset();
    alloc_stats::reset_peak();
    let live_before = alloc_stats::live();
    let started = Instant::now();
    let result = action();
    let millis = started.elapsed().as_secs_f64() * 1_000.0;
    let peak = alloc_stats::peak();
    (
        result,
        Sample {
            millis,
            counters: instrumentation::counters(),
            peak_bytes: peak.saturating_sub(live_before),
            formula_nodes: 0,
        },
    )
}

fn summarize(samples: &[Sample]) -> Sample {
    let mut ordered = samples.to_vec();
    ordered.sort_by(|left, right| left.millis.total_cmp(&right.millis));
    let median = ordered[ordered.len() / 2];
    Sample {
        peak_bytes: samples
            .iter()
            .map(|sample| sample.peak_bytes)
            .max()
            .unwrap_or(0),
        ..median
    }
}

struct Row {
    cells: usize,
    rows: usize,
    columns: usize,
    operation: &'static str,
    sample: Sample,
    output_bytes: usize,
}

fn report_row(
    rows: &mut Vec<Row>,
    size: (usize, usize),
    operation: &'static str,
    samples: &[Sample],
    output_bytes: usize,
) {
    rows.push(Row {
        cells: size.0 * size.1,
        rows: size.0,
        columns: size.1,
        operation,
        sample: summarize(samples),
        output_bytes,
    });
}

struct FixedTime;

impl TrustedPublicationTimeSource for FixedTime {
    fn now(&mut self) -> TrustedInstant {
        TrustedInstant::new(1)
    }
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).expect("benchmark numbers are finite"))
}

fn set_number(document: &mut Document, row: usize, field: &str, value: f64) {
    document
        .entities
        .get_mut(&entity_id(row))
        .expect("fixture row exists")
        .fields
        .insert(FieldId::from(field), number(value));
}

/// Construct a runtime for a fixture the current admission profile refuses.
///
/// This mirrors the trusted constructor's setup while intentionally omitting
/// the size/profile admission, so the measurement covers the real projection
/// and persistence code instead of an artificial placeholder.
fn bench_runtime(document: Document, occurrence_id: &str) -> DesignerRuntime {
    let collection_specs = collection_specs(&document);
    let default_collection =
        select_default_collection(&collection_specs).expect("fixture has a collection");
    let collections = collection_specs
        .values()
        .map(|spec| spec.summary.clone())
        .collect::<Vec<_>>();
    let title = document.title.clone();
    let document_scope = document_scope(occurrence_id, &document).expect("fixture scope");
    let formula_sources = formula_sources(&document).expect("fixture formulas");
    let principal = PrincipalId::from(DESIGNER_PRINCIPAL);
    let lifecycle =
        designer_lifecycle(&document_scope, &document, &principal).expect("fixture lifecycle");
    let session = ResidentWorkspaceSession::new(document_scope.clone(), document);
    DesignerRuntime {
        text_capacity: false,
        title,
        document_scope,
        default_collection,
        collections,
        collection_specs,
        formula_sources,
        session,
        lifecycle,
        principal,
        clock: DesignerClock::default(),
        proposal_serial: 0,
        row_serial: 0,
        row_namespace: occurrence_id.to_owned(),
        undo: Vec::new(),
        redo: Vec::new(),
        pending_cleanup: None,
        duplicate_serial: 0,
        opened_from_v3: false,
    }
}

fn measure_engine_edits(document: &Document, size: (usize, usize), rows: &mut Vec<Row>) {
    let scope = DocumentScopeId::from("capacity-benchmark");
    let session_document = document.clone();
    let (mut session, sample) =
        observe(|| ResidentWorkspaceSession::new(scope.clone(), session_document));
    report_row(rows, size, "session build", &[sample], 0);
    measure_scalar_edits(&mut session, &scope, size, rows);
    measure_snapshot_exports(&session, size, rows);
    measure_formula_edits(size, rows);
}

fn measure_scalar_edits(
    session: &mut ResidentWorkspaceSession,
    scope: &DocumentScopeId,
    size: (usize, usize),
    rows: &mut Vec<Row>,
) {
    let mut samples = Vec::with_capacity(EDIT_ITERATIONS);
    for iteration in 0..EDIT_ITERATIONS {
        let revision = session.revision().clone();
        let scope = scope.clone();
        // Edit the last rows so the publication's map equality cannot
        // short-circuit on an early difference.
        let entity = entity_id(size.0 - 1 - (iteration % size.0));
        let field = field_id(0);
        let value = 1_000.0 + f64::from(u32::try_from(iteration).expect("bounded iteration"));
        let ((), sample) = observe(|| {
            let mut candidate = session.export_snapshot().into_document();
            candidate
                .entities
                .get_mut(&entity)
                .expect("fixture row exists")
                .fields
                .insert(field, number(value));
            let mut time = FixedTime;
            session
                .publication_authority(&mut time)
                .publish_if_current(&scope, &revision, candidate, |_| Some(()))
                .expect("scalar publication succeeds");
        });
        samples.push(sample);
    }
    report_row(rows, size, "scalar edit + publish", &samples, 0);
}

fn measure_snapshot_exports(
    session: &ResidentWorkspaceSession,
    size: (usize, usize),
    rows: &mut Vec<Row>,
) {
    let mut samples = Vec::with_capacity(BULK_ITERATIONS);
    for _ in 0..BULK_ITERATIONS {
        let ((), sample) = observe(|| {
            let _ = session.export_snapshot();
        });
        samples.push(sample);
    }
    report_row(rows, size, "snapshot export", &samples, 0);
}

fn measure_formula_edits(size: (usize, usize), rows: &mut Vec<Row>) {
    let document = capacity_document(FixtureSpec {
        rows: size.0,
        columns: size.1,
        formula_cells: DEPENDENT_FORMULAS,
        seed: SEED ^ 0xF0F0,
    });
    validate(&document).expect("formula fixture validates");
    let scope = DocumentScopeId::from("capacity-benchmark-formulas");
    let mut session = ResidentWorkspaceSession::new(scope.clone(), document);
    let mut samples = Vec::with_capacity(EDIT_ITERATIONS);
    for iteration in 0..EDIT_ITERATIONS {
        let revision = session.revision().clone();
        let scope = scope.clone();
        let before = session.runtime_measurements().calculation_nodes_recomputed;
        let value = 3.0 + f64::from(u32::try_from(iteration).expect("bounded iteration"));
        let ((), mut sample) = observe(|| {
            let mut candidate = session.export_snapshot().into_document();
            set_number(&mut candidate, 0, RATE_FIELD, value);
            let mut time = FixedTime;
            session
                .publication_authority(&mut time)
                .publish_if_current(&scope, &revision, candidate, |_| Some(()))
                .expect("formula publication succeeds");
        });
        let after = session.runtime_measurements().calculation_nodes_recomputed;
        sample.formula_nodes = u64::try_from(after.saturating_sub(before)).unwrap_or(u64::MAX);
        samples.push(sample);
    }
    report_row(rows, size, "rate edit, 100 dependent formulas", &samples, 0);
}

fn measure_projection(document: &Document, size: (usize, usize), rows: &mut Vec<Row>) {
    let runtime = bench_runtime(document.clone(), "00000000-0000-4000-8000-0000000000a1");
    let mut samples = Vec::with_capacity(BULK_ITERATIONS);
    let mut last = None;
    for _ in 0..BULK_ITERATIONS {
        let (table, sample) = observe(|| {
            runtime
                .build_table_projection_unbounded("grid")
                .expect("fixture table projection builds")
        });
        last = Some(table);
        samples.push(sample);
    }
    report_row(rows, size, "full table projection build", &samples, 0);
    let table = last.expect("at least one projection");
    let ((), sample) = observe(|| {
        ensure_projection_size_with_limit(&table, usize::MAX).expect("projection serializes");
    });
    let output_bytes = usize::try_from(sample.counters.json_bytes_serialized).unwrap_or(usize::MAX);
    report_row(rows, size, "table projection JSON", &[sample], output_bytes);
}

fn measure_persistence(document: &Document, size: (usize, usize), rows: &mut Vec<Row>) {
    let (tree, encode) = observe(|| encode_roproj_v3(document).expect("v3 encode"));
    let tree_bytes = tree
        .files()
        .iter()
        .map(|file| file.bytes().len())
        .sum::<usize>();
    report_row(rows, size, "v3 encode", &[encode], tree_bytes);
    let (bundle, bundle_sample) = observe(|| {
        encode_project_bundle(tree.files().iter().map(|file| (file.path(), file.bytes())))
            .expect("v3 bundle encode")
    });
    let bundle_bytes = bundle.len();
    report_row(
        rows,
        size,
        "v3 bundle encode",
        &[bundle_sample],
        bundle_bytes,
    );
    let ((), decode) = observe(|| {
        let _ = decode_roproj_v3(&tree).expect("v3 decode");
    });
    report_row(rows, size, "v3 decode", &[decode], 0);
    // The runtime bundle decoder re-admits the whole document, so it refuses
    // every size above the current profile. Keep the measurement when it is
    // available and record the refusal instead of failing the baseline.
    let (decoded, bundle_decode) = observe(|| decode_project_bundle(&bundle));
    match decoded {
        Ok(_) => report_row(rows, size, "v3 bundle decode", &[bundle_decode], 0),
        Err(error) => {
            let (row_count, columns) = size;
            println!(
                "size {row_count}x{columns}: v3 bundle decode refused by runtime admission: {error}"
            );
        }
    }
}

/// Measure the complete admitted runtime edit path, including lifecycle
/// proposal/preview, per-request preflight admission and history retention.
///
/// Stage A sizes are still refused by the runtime admission profile, so this
/// row is measured at the largest fixture the profile accepts.
fn measure_runtime_edit(rows_report: &mut Vec<Row>) {
    let size = (RUNTIME_EDIT_ROWS, RUNTIME_EDIT_COLUMNS);
    let document = capacity_document(FixtureSpec {
        rows: size.0,
        columns: size.1,
        formula_cells: 0,
        seed: SEED ^ 0xA5A5,
    });
    validate(&document).expect("runtime fixture validates");
    let mut runtime =
        match DesignerRuntime::from_document(document, "00000000-0000-4000-8000-0000000000a2") {
            Ok(runtime) => runtime,
            Err(error) => {
                println!("runtime edit path skipped: {error}");
                return;
            }
        };
    let mut samples = Vec::with_capacity(EDIT_ITERATIONS);
    for iteration in 0..EDIT_ITERATIONS {
        let revision = runtime.observe_occurrence().revision;
        let target = FieldTarget {
            entity: entity_id(iteration % size.0).to_string(),
            field: field_id(0).to_string(),
        };
        let input_value = 10.0 + f64::from(u32::try_from(iteration).expect("bounded iteration"));
        let input = ScalarEditInput::Number {
            input: format!("{input_value}"),
        };
        let ((), sample) = observe(|| {
            let response = runtime
                .handle(DesignerRequest::EditScalar {
                    expected_revision: revision.clone(),
                    target: target.clone(),
                    input: input.clone(),
                })
                .expect("admitted runtime edit succeeds");
            assert!(matches!(response, super::DesignerResponse::Published(_)));
        });
        samples.push(sample);
    }
    report_row(
        rows_report,
        size,
        "runtime EditScalar (admitted)",
        &samples,
        0,
    );
}

/// Priority 1 check: how many live bytes the runtime retains per undo step.
///
/// Each step is one small scalar edit, all undone afterwards. If a history entry
/// held a full `Document` copy, retained bytes per step would be at least the
/// whole fixture document; the measured value answers that directly.
fn measure_history() {
    let size = (RUNTIME_EDIT_ROWS, RUNTIME_EDIT_COLUMNS);
    let baseline = alloc_stats::live();
    let document = capacity_document(FixtureSpec {
        rows: size.0,
        columns: size.1,
        formula_cells: 0,
        seed: SEED ^ 0x5A5A,
    });
    let document_bytes = alloc_stats::live().saturating_sub(baseline);
    validate(&document).expect("history fixture validates");
    let mut runtime =
        match DesignerRuntime::from_document(document, "00000000-0000-4000-8000-0000000000a3") {
            Ok(runtime) => runtime,
            Err(error) => {
                println!("history measurement skipped: {error}");
                return;
            }
        };
    let live_before = alloc_stats::live();
    let edit_started = Instant::now();
    for iteration in 0..HISTORY_DEPTH {
        let revision = runtime.observe_occurrence().revision;
        let target = FieldTarget {
            entity: entity_id(iteration % size.0).to_string(),
            field: field_id(0).to_string(),
        };
        let input_value = 100.0 + f64::from(u32::try_from(iteration).expect("bounded iteration"));
        runtime
            .handle(DesignerRequest::EditScalar {
                expected_revision: revision,
                target,
                input: ScalarEditInput::Number {
                    input: format!("{input_value}"),
                },
            })
            .expect("history edit succeeds");
    }
    let edit_millis = edit_started.elapsed().as_secs_f64() * 1_000.0;
    let retained = alloc_stats::live().saturating_sub(live_before);
    let per_entry = retained / u64::try_from(HISTORY_DEPTH).unwrap_or(1);
    let undo_started = Instant::now();
    for _ in 0..HISTORY_DEPTH {
        let revision = runtime.observe_occurrence().revision;
        runtime
            .handle(DesignerRequest::Undo {
                expected_revision: revision,
            })
            .expect("history undo succeeds");
    }
    let undo_millis = undo_started.elapsed().as_secs_f64() * 1_000.0;
    let live_after_undo = alloc_stats::live().saturating_sub(live_before);
    println!(
        "\nhistory: {HISTORY_DEPTH} admitted edits retained {retained} bytes ({per_entry} bytes/entry) in {edit_millis:.3} ms; {HISTORY_DEPTH} undos in {undo_millis:.3} ms; retained after undo {live_after_undo} bytes; fixture document {document_bytes} bytes"
    );
}

fn print_report(rows: &[Row]) {
    println!(
        "\n| cells | rows | columns | operation | median ms | document clones | entities inspected | fields inspected | document comparisons | formula nodes | JSON bytes | output bytes | peak delta MiB |"
    );
    println!(
        "| ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"
    );
    for row in rows {
        let cells = row.cells;
        let row_count = row.rows;
        let columns = row.columns;
        let operation = row.operation;
        let sample = row.sample;
        let millis = sample.millis;
        let output_bytes = row.output_bytes;
        let counters = sample.counters;
        let clones = counters.document_clones;
        let entities = counters.publication_entities_inspected;
        let fields = counters.publication_fields_inspected;
        let comparisons = counters.publication_document_comparisons;
        let formula_nodes = sample.formula_nodes;
        let json_bytes = counters.json_bytes_serialized;
        let peak_mib = sample.peak_bytes / (1024 * 1024);
        println!(
            "| {cells} | {row_count} | {columns} | {operation} | {millis:.3} | {clones} | {entities} | {fields} | {comparisons} | {formula_nodes} | {json_bytes} | {output_bytes} | {peak_mib} |"
        );
    }
}

#[test]
#[ignore = "capacity baseline; run explicitly in release with --features capacity-bench"]
#[allow(clippy::too_many_lines)] // One linear baseline script is easier to audit.
fn capacity_stage_a_baseline() {
    let mut rows = Vec::new();
    for size in STAGE_A_SIZES {
        let spec = FixtureSpec {
            rows: size.0,
            columns: size.1,
            formula_cells: 0,
            seed: SEED,
        };
        let baseline = alloc_stats::live();
        let document = capacity_document(spec);
        let document_bytes = alloc_stats::live().saturating_sub(baseline);
        validate(&document).expect("fixture validates");
        let (row_count, columns, cells) = (spec.rows, spec.columns, spec.cells());
        let populated = document
            .entities
            .values()
            .map(|entity| entity.fields.len())
            .sum::<usize>();
        println!(
            "size {row_count}x{columns} ({cells} grid cells, {populated} populated): fixture document live bytes {document_bytes}"
        );
        measure_engine_edits(&document, size, &mut rows);
        measure_projection(&document, size, &mut rows);
        measure_persistence(&document, size, &mut rows);
    }
    measure_runtime_edit(&mut rows);
    measure_history();
    print_report(&rows);
}
