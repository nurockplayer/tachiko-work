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
//! Peak memory is read from Linux procfs: the harness resets `VmHWM` through
//! `/proc/self/clear_refs` before each measured operation and reports the peak
//! RSS delta afterwards. Those numbers are resident set size — allocator
//! retention included — not live heap. Non-Linux targets report "unavailable"
//! instead of failing the run.

use std::collections::BTreeMap;
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
    ScalarEditInput, ViewIndexCache, collection_specs, decode_project_bundle, designer_lifecycle,
    document_scope, encode_project_bundle, ensure_projection_size_with_limit, formula_sources,
    select_default_collection,
};
use crate::capacity_fixtures::{
    DEPENDENT_FORMULAS, FixtureSpec, RATE_FIELD, SWEEP_DATA_COLUMNS, capacity_document, entity_id,
    field_id, formula_sweep_document,
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

/// Linux procfs memory readings.
///
/// The harness runs natively only in the Linux `OrbStack` container. On other
/// targets every reading is `None`, which the report prints as "unavailable"
/// instead of failing the run.
mod memory_stats {
    /// Resident set size in bytes, from `VmRSS`.
    pub fn rss_bytes() -> Option<u64> {
        status_kib("VmRSS:").map(kib_to_bytes)
    }

    /// Resident high-water mark in bytes, from `VmHWM`.
    pub(super) fn peak_bytes() -> Option<u64> {
        status_kib("VmHWM:").map(kib_to_bytes)
    }

    /// Reset the high-water mark to the current RSS.
    ///
    /// Returns `false` when the platform or the container does not expose the
    /// interface; the caller then records the peak as unavailable.
    pub fn reset_peak() -> bool {
        #[cfg(target_os = "linux")]
        {
            std::fs::write("/proc/self/clear_refs", "5").is_ok()
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    fn kib_to_bytes(kib: u64) -> u64 {
        kib.saturating_mul(1024)
    }

    #[cfg(target_os = "linux")]
    fn status_kib(key: &str) -> Option<u64> {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        status.lines().find_map(|line| {
            let value = line.strip_prefix(key)?;
            value.split_whitespace().next()?.parse().ok()
        })
    }

    #[cfg(not(target_os = "linux"))]
    fn status_kib(_key: &str) -> Option<u64> {
        None
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Sample {
    millis: f64,
    counters: CapacityCounters,
    peak_delta_bytes: Option<u64>,
    formula_nodes: u64,
}

fn observe<R>(action: impl FnOnce() -> R) -> (R, Sample) {
    instrumentation::reset();
    let rss_before = memory_stats::rss_bytes();
    let tracking_peak = memory_stats::reset_peak();
    let started = Instant::now();
    let result = action();
    let millis = started.elapsed().as_secs_f64() * 1_000.0;
    (
        result,
        Sample {
            millis,
            counters: instrumentation::counters(),
            peak_delta_bytes: peak_delta(rss_before, tracking_peak),
            formula_nodes: 0,
        },
    )
}

/// Peak resident memory the operation added: `VmHWM` after minus `VmRSS`
/// before the operation started.
fn peak_delta(rss_before: Option<u64>, tracking_peak: bool) -> Option<u64> {
    if !tracking_peak {
        return None;
    }
    let before = rss_before?;
    memory_stats::peak_bytes().map(|peak| peak.saturating_sub(before))
}

/// RSS change since the earlier reading, or `None` where procfs is unavailable.
fn rss_delta(before: Option<u64>) -> Option<u64> {
    let before = before?;
    memory_stats::rss_bytes().map(|after| after.saturating_sub(before))
}

fn describe_bytes(value: Option<u64>) -> String {
    value.map_or_else(|| "unavailable".to_owned(), |bytes| bytes.to_string())
}

fn summarize(samples: &[Sample]) -> Sample {
    let mut ordered = samples.to_vec();
    ordered.sort_by(|left, right| left.millis.total_cmp(&right.millis));
    let median = ordered[ordered.len() / 2];
    Sample {
        peak_delta_bytes: samples
            .iter()
            .filter_map(|sample| sample.peak_delta_bytes)
            .max(),
        ..median
    }
}

struct Row {
    cells: usize,
    rows: usize,
    columns: usize,
    operation: String,
    sample: Sample,
    output_bytes: usize,
}

fn report_row(
    rows: &mut Vec<Row>,
    size: (usize, usize),
    operation: impl Into<String>,
    samples: &[Sample],
    output_bytes: usize,
) {
    rows.push(Row {
        cells: size.0 * size.1,
        rows: size.0,
        columns: size.1,
        operation: operation.into(),
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
    let capacity_usage = crate::CapacityUsage::measure(&document);
    let session = ResidentWorkspaceSession::new(document_scope.clone(), document);
    DesignerRuntime {
        text_capacity: false,
        capacity_usage,
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
        view_index: ViewIndexCache::default(),
        totals_cache: None,
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

/// Priority 1 check: how much resident memory the runtime retains per undo step.
///
/// Each step is one small scalar edit, all undone afterwards. If a history entry
/// held a full `Document` copy, retained RSS per step would be at least the
/// whole fixture document; the measured value answers that directly. RSS is
/// page-granular and includes allocator retention, so small differences read as
/// zero or in 4 KiB steps.
fn measure_history() {
    let size = (RUNTIME_EDIT_ROWS, RUNTIME_EDIT_COLUMNS);
    let rss_baseline = memory_stats::rss_bytes();
    let document = capacity_document(FixtureSpec {
        rows: size.0,
        columns: size.1,
        formula_cells: 0,
        seed: SEED ^ 0x5A5A,
    });
    let document_bytes = rss_delta(rss_baseline);
    validate(&document).expect("history fixture validates");
    let mut runtime =
        match DesignerRuntime::from_document(document, "00000000-0000-4000-8000-0000000000a3") {
            Ok(runtime) => runtime,
            Err(error) => {
                println!("history measurement skipped: {error}");
                return;
            }
        };
    let rss_before = memory_stats::rss_bytes();
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
    let retained = rss_delta(rss_before);
    let per_entry = retained.map(|bytes| bytes / u64::try_from(HISTORY_DEPTH).unwrap_or(1));
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
    let after_undo = rss_delta(rss_before);
    println!(
        "\nhistory: {HISTORY_DEPTH} admitted edits retained {} bytes ({} bytes/entry) in {edit_millis:.3} ms; {HISTORY_DEPTH} undos in {undo_millis:.3} ms; retained after undo {} bytes; fixture document {} bytes",
        describe_bytes(retained),
        describe_bytes(per_entry),
        describe_bytes(after_undo),
        describe_bytes(document_bytes),
    );
}

fn print_report_header() {
    println!(
        "\n| cells | rows | columns | operation | median ms | document clones | entities inspected | fields inspected | document comparisons | formula nodes | JSON bytes | output bytes | peak RSS delta MiB |"
    );
    println!(
        "| ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"
    );
}

fn print_row(row: &Row) {
    let cells = row.cells;
    let row_count = row.rows;
    let columns = row.columns;
    let operation = &row.operation;
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
    let peak_mib = sample.peak_delta_bytes.map_or_else(
        || "unavailable".to_owned(),
        |bytes| {
            let tenths = bytes.saturating_mul(10) / (1024 * 1024);
            format!("{}.{}", tenths / 10, tenths % 10)
        },
    );
    println!(
        "| {cells} | {row_count} | {columns} | {operation} | {millis:.3} | {clones} | {entities} | {fields} | {comparisons} | {formula_nodes} | {json_bytes} | {output_bytes} | {peak_mib} |"
    );
}

fn print_report(rows: &[Row]) {
    print_report_header();
    for row in rows {
        print_row(row);
    }
}

/// Append one sweep row and print it immediately: a cancelled run still
/// reports every completed operation.
fn sweep_report_row(
    rows: &mut Vec<Row>,
    size: (usize, usize),
    operation: impl Into<String>,
    samples: &[Sample],
    output_bytes: usize,
) {
    report_row(rows, size, operation, samples, output_bytes);
    if let Some(row) = rows.last() {
        print_row(row);
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
        let rss_baseline = memory_stats::rss_bytes();
        let document = capacity_document(spec);
        let document_bytes = rss_delta(rss_baseline);
        validate(&document).expect("fixture validates");
        let (row_count, columns, cells) = (spec.rows, spec.columns, spec.cells());
        let populated = document
            .entities
            .values()
            .map(|entity| entity.fields.len())
            .sum::<usize>();
        println!(
            "size {row_count}x{columns} ({cells} grid cells, {populated} populated): fixture document RSS increase {} bytes",
            describe_bytes(document_bytes),
        );
        measure_engine_edits(&document, size, &mut rows);
        measure_projection(&document, size, &mut rows);
        measure_persistence(&document, size, &mut rows);
    }
    measure_runtime_edit(&mut rows);
    measure_history();
    print_report(&rows);
}

/// Isolate the A-02 O(document) admission scan from publication and encoding.
#[test]
#[ignore = "capacity accounting; run explicitly in release with --features capacity-bench"]
fn capacity_accounting_baseline() {
    let mut rows = Vec::new();
    for size in [(100, 10), (1_000, 10)] {
        let document = capacity_document(FixtureSpec {
            rows: size.0,
            columns: size.1,
            formula_cells: 0,
            seed: SEED,
        });
        let limits = crate::CapacityLimits {
            table_rows: 10_000,
            total_entities: 100_000,
            ..crate::capacity::DEFAULT_LIMITS
        };
        let samples = (0..EDIT_ITERATIONS)
            .map(|_| {
                observe(|| {
                    std::hint::black_box(
                        crate::capacity::check_document(&document, limits).unwrap(),
                    );
                })
                .1
            })
            .collect::<Vec<_>>();
        report_row(&mut rows, size, "capacity accounting", &samples, 0);
    }
    print_report(&rows);
}

/// Formula-count sweep through the real admitted runtime.
///
/// Every shape stays inside today's row and entity limits, so the sweep shows
/// where formula count stops being serviceable rather than where the row
/// profile refuses. A refusal is a result: the shape reports the error verbatim
/// and moves on. The sweep stops at the first sample above 120 seconds.
#[test]
#[ignore = "capacity formula sweep; run explicitly in release with --features capacity-bench"]
#[allow(clippy::too_many_lines)] // One linear sweep script is easier to audit.
fn capacity_formula_count_sweep() {
    /// Shapes as (tables, rows per table, formula columns). Every shape fits
    /// the current 8-table/128-row/1,024-entity limits.
    const SHAPES: [(usize, usize, usize); 16] = [
        (1, 64, 1),
        (1, 64, 2),
        (1, 64, 4),
        (1, 64, 8),
        (1, 64, 12),
        (8, 8, 8),
        (8, 4, 24),
        (1, 128, 1),
        (1, 128, 2),
        (1, 128, 4),
        (1, 32, 1),
        (2, 128, 1),
        (8, 128, 1),
        (8, 128, 4),
        (8, 128, 16),
        (8, 128, 29),
    ];
    const OPEN_ITERATIONS: usize = 3;
    const FORMULA_ITERATIONS: usize = 3;
    const SAMPLE_LIMIT_MILLIS: f64 = 120_000.0;
    print_report_header();
    let mut rows = Vec::new();
    for (tables, rows_per_table, formula_columns) in SHAPES {
        let formula_count = tables * rows_per_table * formula_columns;
        let size = (
            tables * rows_per_table,
            SWEEP_DATA_COLUMNS + formula_columns,
        );
        let document = formula_sweep_document(tables, rows_per_table, formula_columns);
        validate(&document).expect("formula sweep fixture validates");
        let usage = crate::CapacityUsage::measure(&document);
        println!(
            "\nformulas={formula_count} ({tables} tables x {rows_per_table} rows x {formula_columns} formula columns): populated_cells={} measured_formulas={} formula_ast_nodes={} dependency_edges={}",
            usage.populated_cells, usage.formulas, usage.formula_ast_nodes, usage.dependency_edges,
        );

        let mut open_samples = Vec::with_capacity(OPEN_ITERATIONS);
        let mut runtime = None;
        let mut refused = false;
        for iteration in 0..OPEN_ITERATIONS {
            let candidate = document.clone();
            let occurrence = format!(
                "00000000-0000-4000-8000-{:012x}",
                0x100 + u64::try_from(iteration).expect("bounded iteration")
            );
            let (opened, sample) =
                observe(|| DesignerRuntime::from_document(candidate, &occurrence));
            match opened {
                Ok(opened) => runtime = Some(opened),
                Err(error) => {
                    println!("formulas={formula_count} refused: {error}");
                    refused = true;
                    break;
                }
            }
            open_samples.push(sample);
            if sample.millis > SAMPLE_LIMIT_MILLIS {
                println!(
                    "formulas={formula_count}: stopping the sweep because runtime open took {:.3} s (> 120 s)",
                    sample.millis / 1_000.0
                );
                sweep_report_row(
                    &mut rows,
                    size,
                    format!("formulas={formula_count}: runtime open"),
                    &open_samples,
                    0,
                );
                print_report(&rows);
                return;
            }
        }
        if refused || open_samples.is_empty() {
            continue;
        }
        sweep_report_row(
            &mut rows,
            size,
            format!("formulas={formula_count}: runtime open"),
            &open_samples,
            0,
        );
        let Some(mut runtime) = runtime else {
            continue;
        };
        let bootstrap_bytes = serde_json::to_vec(&runtime.bootstrap_projection())
            .map(|bytes| bytes.len())
            .ok();
        println!(
            "formulas={formula_count}: bootstrap projection {} bytes (wire limit {}, native tracker limit {})",
            describe_bytes(bootstrap_bytes.map(|bytes| u64::try_from(bytes).unwrap_or(u64::MAX))),
            crate::MAX_PROJECTION_BYTES,
            crate::MAX_NATIVE_TRACKER_PROJECTION_BYTES,
        );

        let last_entity = format!("t{}-r{:05}", tables - 1, rows_per_table - 1);
        let mut unrelated_samples = Vec::with_capacity(EDIT_ITERATIONS);
        for iteration in 0..EDIT_ITERATIONS {
            let revision = runtime.observe_occurrence().revision;
            let target = FieldTarget {
                entity: last_entity.clone(),
                field: "c1".to_owned(),
            };
            let value = 100.0 + f64::from(u32::try_from(iteration).expect("bounded iteration"));
            let ((), sample) = observe(|| {
                let response = runtime
                    .handle(DesignerRequest::EditScalar {
                        expected_revision: revision.clone(),
                        target: target.clone(),
                        input: ScalarEditInput::Number {
                            input: format!("{value}"),
                        },
                    })
                    .expect("admitted unrelated edit succeeds");
                assert!(matches!(response, super::DesignerResponse::Published(_)));
            });
            unrelated_samples.push(sample);
            if sample.millis > SAMPLE_LIMIT_MILLIS {
                println!(
                    "formulas={formula_count}: stopping the sweep because an unrelated edit took {:.3} s (> 120 s)",
                    sample.millis / 1_000.0
                );
                sweep_report_row(
                    &mut rows,
                    size,
                    format!("formulas={formula_count}: unrelated edit (c1)"),
                    &unrelated_samples,
                    0,
                );
                print_report(&rows);
                return;
            }
        }
        sweep_report_row(
            &mut rows,
            size,
            format!("formulas={formula_count}: unrelated edit (c1)"),
            &unrelated_samples,
            0,
        );

        let mut dependent_samples = Vec::with_capacity(EDIT_ITERATIONS);
        for iteration in 0..EDIT_ITERATIONS {
            let revision = runtime.observe_occurrence().revision;
            let target = FieldTarget {
                entity: last_entity.clone(),
                field: "c0".to_owned(),
            };
            let value = 1_000.0 + f64::from(u32::try_from(iteration).expect("bounded iteration"));
            let ((), sample) = observe(|| {
                let response = runtime
                    .handle(DesignerRequest::EditScalar {
                        expected_revision: revision.clone(),
                        target: target.clone(),
                        input: ScalarEditInput::Number {
                            input: format!("{value}"),
                        },
                    })
                    .expect("admitted dependent edit succeeds");
                assert!(matches!(response, super::DesignerResponse::Published(_)));
            });
            dependent_samples.push(sample);
            if sample.millis > SAMPLE_LIMIT_MILLIS {
                println!(
                    "formulas={formula_count}: stopping the sweep because a dependent edit took {:.3} s (> 120 s)",
                    sample.millis / 1_000.0
                );
                sweep_report_row(
                    &mut rows,
                    size,
                    format!("formulas={formula_count}: dependent edit (c0)"),
                    &dependent_samples,
                    0,
                );
                print_report(&rows);
                return;
            }
        }
        sweep_report_row(
            &mut rows,
            size,
            format!("formulas={formula_count}: dependent edit (c0)"),
            &dependent_samples,
            0,
        );

        if formula_columns == 0 {
            println!("formulas={formula_count}: formula edit skipped (no formula columns)");
            continue;
        }
        let mut formula_samples = Vec::with_capacity(FORMULA_ITERATIONS);
        for iteration in 0..FORMULA_ITERATIONS {
            let revision = runtime.observe_occurrence().revision;
            let target = FieldTarget {
                entity: last_entity.clone(),
                field: "f0".to_owned(),
            };
            // Start above the fixture's literal 2 so every sample is a real
            // change; an identical formula is refused as `NoChange`.
            let source = format!("[.c0] * {}", 3 + iteration);
            let ((), sample) = observe(|| {
                let response = runtime
                    .handle(DesignerRequest::FormulaUpdate {
                        expected_revision: revision.clone(),
                        target: target.clone(),
                        source: source.clone(),
                    })
                    .expect("admitted formula edit succeeds");
                assert!(matches!(response, super::DesignerResponse::Published(_)));
            });
            formula_samples.push(sample);
            if sample.millis > SAMPLE_LIMIT_MILLIS {
                println!(
                    "formulas={formula_count}: stopping the sweep because a formula edit took {:.3} s (> 120 s)",
                    sample.millis / 1_000.0
                );
                sweep_report_row(
                    &mut rows,
                    size,
                    format!("formulas={formula_count}: formula edit (f0)"),
                    &formula_samples,
                    0,
                );
                print_report(&rows);
                return;
            }
        }
        sweep_report_row(
            &mut rows,
            size,
            format!("formulas={formula_count}: formula edit (f0)"),
            &formula_samples,
            0,
        );
    }
    print_report(&rows);
}

/// Which cell one measured edit touches.
#[derive(Clone, Copy)]
enum PhaseTarget {
    /// `c0` of the last row: an existing Number no fixture formula reads.
    LastRowFirstColumn,
    /// Row 0 `rate`: the driver of the 99 same-row formulas plus the total.
    Driver,
}

/// Display order and nesting depth of every phase the timers record.
const PHASE_ORDER: &[(&str, usize)] = &[
    ("handle", 0),
    ("edit_cells", 0),
    ("edit_cells/pre_publish", 1),
    ("record_edit/history", 1),
    ("publish_commands", 1),
    ("publish_commands/export_snapshot_clone", 2),
    ("publish_commands/apply_commands", 2),
    ("publish_commands/capacity_check", 2),
    ("publish_commands/validate", 2),
    ("publish_commands/preflight", 2),
    ("propose", 2),
    ("propose/plan_commands", 3),
    ("propose/finalize_evaluation", 3),
    ("finalize_evaluation/validate", 4),
    ("finalize_evaluation/preflight_formula_projections", 4),
    ("finalize_evaluation/canonical_delta", 4),
    ("finalize_evaluation/diff", 4),
    ("finalize_evaluation/derive_disclosures", 4),
    ("finalize_evaluation/validation_report", 4),
    ("publish_commands/preview", 2),
    ("publish_commands/execute", 2),
    ("publish_commands/execute/execute_profile", 3),
    ("execute_profile/current_snapshot", 4),
    ("execute_profile/candidate_clone", 4),
    ("execute_profile/publish_candidate", 4),
    ("publish_if_current", 5),
    ("publish_if_current/equality", 6),
    ("publish_if_current/projection_invalidation", 6),
    ("projection_invalidation/retained_update", 7),
    ("retained_update/calculation_update", 8),
    ("retained_update/validation_report", 8),
    ("publish_if_current/return_clone", 6),
    ("execute_profile/verify_publication", 4),
    ("publish_commands/refresh_structure", 2),
    ("refresh_structure/capacity_measure", 3),
    ("handle/build_response", 1),
];

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

/// Inclusive milliseconds per label for one edit; a phase entered more than
/// once (the v2 path validates twice) is summed under its one label.
fn phase_millis(phases: &[(&'static str, u128)]) -> BTreeMap<&'static str, f64> {
    let mut map = BTreeMap::new();
    for (name, micros) in phases {
        #[allow(clippy::cast_precision_loss)]
        let millis = *micros as f64 / 1_000.0;
        *map.entry(*name).or_insert(0.0) += millis;
    }
    map
}

fn edit_target(spec: &FixtureSpec, target: PhaseTarget) -> (String, String) {
    match target {
        PhaseTarget::LastRowFirstColumn => (
            entity_id(spec.rows - 1).to_string(),
            field_id(0).to_string(),
        ),
        PhaseTarget::Driver => (entity_id(0).to_string(), RATE_FIELD.to_owned()),
    }
}

/// Open one fixture, run some scalar edits through `handle`, print the median
/// phase table for its whole request.
fn measure_phase_fixture(
    label: &str,
    spec: FixtureSpec,
    target: PhaseTarget,
    text_capacity: bool,
    edits: usize,
    fixture_time_limit_ms: f64,
) {
    let constructor = if text_capacity {
        "text-capacity constructor"
    } else {
        "ordinary constructor"
    };
    let document = capacity_document(spec);
    let occurrence = format!(
        "00000000-0000-4000-8000-{:012x}",
        if text_capacity { 0x200 } else { 0x100 }
    );
    let started = Instant::now();
    let opened = if text_capacity {
        DesignerRuntime::from_document_profile(document, &occurrence, true)
    } else {
        DesignerRuntime::from_document(document, &occurrence)
    };
    let open_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let mut runtime = match opened {
        Ok(runtime) => runtime,
        Err(error) => {
            println!(
                "\nfixture {label} ({}x{} = {} cells, {constructor}): open refused after {open_ms:.1} ms: {error}",
                spec.rows,
                spec.columns,
                spec.cells(),
            );
            return;
        }
    };
    println!(
        "\n### fixture {label}: {} rows x {} columns = {} cells, {constructor}, open {open_ms:.1} ms",
        spec.rows,
        spec.columns,
        spec.cells(),
    );

    let mut edits_run = 0usize;
    let mut wall_millis = Vec::new();
    let mut clone_counts = Vec::new();
    let mut samples: Vec<BTreeMap<&'static str, f64>> = Vec::new();
    let fixture_started = Instant::now();
    for iteration in 0..edits {
        if iteration > 0 {
            let elapsed = fixture_started.elapsed().as_secs_f64() * 1_000.0;
            if elapsed > fixture_time_limit_ms {
                println!(
                    "fixture {label}: stopping after {iteration} edits ({elapsed:.0} ms > {fixture_time_limit_ms:.0} ms)"
                );
                break;
            }
        }
        let revision = runtime.observe_occurrence().revision;
        let (entity, field) = edit_target(&spec, target);
        #[allow(clippy::cast_precision_loss)]
        let value = 1_000.0 + iteration as f64;
        instrumentation::reset();
        instrumentation::reset_phases();
        let started = Instant::now();
        let response = runtime.handle(DesignerRequest::EditScalar {
            expected_revision: revision,
            target: FieldTarget { entity, field },
            input: ScalarEditInput::Number {
                input: format!("{value}"),
            },
        });
        let millis = started.elapsed().as_secs_f64() * 1_000.0;
        match response {
            Ok(super::DesignerResponse::Published(_)) => {}
            Ok(_) => {
                println!("fixture {label}: edit {iteration} returned an unexpected response");
                break;
            }
            Err(error) => {
                println!("fixture {label}: edit {iteration} refused: {error}");
                break;
            }
        }
        let counters = instrumentation::counters();
        samples.push(phase_millis(&instrumentation::take_phases()));
        wall_millis.push(millis);
        clone_counts.push(counters.document_clones);
        edits_run += 1;
    }
    if samples.is_empty() {
        println!("fixture {label}: no edit completed");
        return;
    }
    #[allow(clippy::cast_precision_loss)]
    let clones = median(clone_counts.iter().map(|count| *count as f64).collect());
    let whole = median(wall_millis);
    println!(
        "edits measured: {edits_run}; document clones per edit (median): {clones:.0}; whole edit median: {whole:.3} ms"
    );
    println!("| phase | median ms | share of edit |");
    println!("| --- | ---: | ---: |");
    for (name, depth) in PHASE_ORDER {
        if !samples.iter().any(|sample| sample.contains_key(name)) {
            continue;
        }
        let median_ms = median(
            samples
                .iter()
                .map(|sample| sample.get(name).copied().unwrap_or(0.0))
                .collect(),
        );
        let share = if whole > 0.0 {
            median_ms / whole * 100.0
        } else {
            0.0
        };
        let indent = "  ".repeat(*depth);
        println!("| {indent}`{name}` | {median_ms:.3} | {share:.1}% |");
    }
    let mut unknown: Vec<&'static str> = Vec::new();
    for sample in &samples {
        for name in sample.keys() {
            if !PHASE_ORDER.iter().any(|(known, _)| known == name) && !unknown.contains(name) {
                unknown.push(name);
            }
        }
    }
    for name in unknown {
        let median_ms = median(
            samples
                .iter()
                .map(|sample| sample.get(name).copied().unwrap_or(0.0))
                .collect(),
        );
        let share = if whole > 0.0 {
            median_ms / whole * 100.0
        } else {
            0.0
        };
        println!("| `{name}` | {median_ms:.3} | {share:.1}% |");
    }
}

/// Plan A-06b: where one runtime `EditScalar` spends its time at 100k cells.
///
/// A, B, C and D through the ordinary constructor; A and B also through the
/// text-capacity constructor. Every fixture keeps all its samples.
#[test]
#[ignore = "capacity phase breakdown; run explicitly in release with --features capacity-bench"]
fn capacity_edit_phase_breakdown() {
    const EDITS: usize = 3;
    const FIXTURE_TIME_LIMIT_MILLIS: f64 = 600_000.0;
    let fixtures = [
        (
            "A",
            FixtureSpec {
                rows: 1_000,
                columns: 10,
                formula_cells: 0,
                seed: SEED ^ 0xA1A1,
            },
            PhaseTarget::LastRowFirstColumn,
        ),
        (
            "B",
            FixtureSpec {
                rows: 10_000,
                columns: 10,
                formula_cells: 0,
                seed: SEED ^ 0xB2B2,
            },
            PhaseTarget::LastRowFirstColumn,
        ),
        (
            "C",
            FixtureSpec {
                rows: 10_000,
                columns: 10,
                formula_cells: DEPENDENT_FORMULAS,
                seed: SEED ^ 0xC3C3,
            },
            PhaseTarget::Driver,
        ),
        (
            "D",
            FixtureSpec {
                rows: 10_000,
                columns: 10,
                formula_cells: 255,
                seed: SEED ^ 0xD4D4,
            },
            PhaseTarget::LastRowFirstColumn,
        ),
    ];
    for (label, spec, target) in fixtures {
        measure_phase_fixture(label, spec, target, false, EDITS, FIXTURE_TIME_LIMIT_MILLIS);
        if matches!(label, "A" | "B") {
            measure_phase_fixture(label, spec, target, true, EDITS, FIXTURE_TIME_LIMIT_MILLIS);
        }
    }
}
