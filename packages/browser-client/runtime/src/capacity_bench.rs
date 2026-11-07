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

// ---------------------------------------------------------------------------
// Working-size XLSX import/export measurement (scratch/capacity-interop).
//
// Each fixture runs in its own child process under `timeout -k 10 1200`, so a
// phase the synchronous importer cannot finish is cut at 20 minutes instead of
// losing the whole run. `exit 124` from the child means "this phase is over 20
// minutes" and is reported without a retry.
// ---------------------------------------------------------------------------

use crate::interop_adapter::{
    ImportOptions, OutputProfile, column_name, export_xlsx_for_profile, import_xlsx_with,
};
use crate::{ImportFieldType, ImportSelection, import_workbook};
use std::io::{Cursor, Write as _};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

/// Original `main` values, before this scratch branch raised them.
const ORIGINAL_MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;
const ORIGINAL_MAX_EXPANDED_BYTES: usize = 8 * 1024 * 1024;
const ORIGINAL_MAX_XML_NODES: usize = 100_000;
const ORIGINAL_MAX_WORKSHEET_ROW_INDEX_PREFLIGHT: usize = 8_406;
const ORIGINAL_MAX_COLUMNS: usize = 32;
const ORIGINAL_MAX_DATA_ROWS: usize = 128;
const ORIGINAL_MAX_TOTAL_DATA_ROWS: usize = 1_024;
const ORIGINAL_MAX_FORMULAS: usize = 256;
const ORIGINAL_MAX_POPULATED_CELLS: usize = 100_000;
const ORIGINAL_MAX_FORMULA_AST_NODES: usize = 100_000;
const ORIGINAL_MAX_DEPENDENCY_EDGES: usize = 200_000;
const ORIGINAL_MAX_PROJECTION_BYTES: usize = 65_536;
const ORIGINAL_MAX_NATIVE_DATA_ROWS: usize = 128;
const ORIGINAL_MAX_NATIVE_TOTAL_ROWS: usize = 1_024;
const ORIGINAL_MAX_NATIVE_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const ORIGINAL_MAX_NATIVE_EXPANDED_BYTES: usize = 24 * 1024 * 1024;

const PHASE_TIMEOUT_SECONDS: &str = "1200";

#[derive(Clone, Copy, Debug)]
enum FixtureColumn {
    Number,
    Text,
    Date,
    Boolean,
    Product(usize, usize),
    Sum(usize, usize),
    Difference(usize, usize),
    ColumnQuotient(usize),
}

impl FixtureColumn {
    fn import_type(self) -> ImportFieldType {
        match self {
            Self::Number
            | Self::Product(..)
            | Self::Sum(..)
            | Self::Difference(..)
            | Self::ColumnQuotient(..) => ImportFieldType::Number,
            Self::Text => ImportFieldType::Text,
            Self::Date => ImportFieldType::Date,
            Self::Boolean => ImportFieldType::Boolean,
        }
    }
}

struct XlsxFixture {
    bytes: Vec<u8>,
    worksheet_bytes: usize,
    worksheet_nodes: usize,
    shared_string_nodes: usize,
    expanded_bytes: usize,
    rows: usize,
    columns: usize,
    formulas: usize,
    types: Vec<ImportFieldType>,
}

fn say(line: impl std::fmt::Display) {
    println!("{line}");
    let _ = std::io::stdout().flush();
}

fn mib(value: Option<u64>) -> String {
    value.map_or_else(
        || "unavailable".to_owned(),
        |bytes| format!("{:.1}", bytes as f64 / (1024.0 * 1024.0)),
    )
}

fn plain_number(row: usize, column: usize) -> f64 {
    row as f64 + 1.0 + column as f64 * 0.5
}

fn column_total(rows: usize, column: usize) -> f64 {
    (0..rows).map(|row| plain_number(row, column)).sum()
}

fn cached_value(kind: FixtureColumn, rows: usize, row: usize) -> f64 {
    match kind {
        FixtureColumn::Product(left, right) => plain_number(row, left) * plain_number(row, right),
        FixtureColumn::Sum(left, right) => plain_number(row, left) + plain_number(row, right),
        FixtureColumn::Difference(left, right) => plain_number(row, left) - plain_number(row, right),
        FixtureColumn::ColumnQuotient(dividend) => {
            plain_number(row, dividend) / column_total(rows, dividend)
        }
        FixtureColumn::Number | FixtureColumn::Text | FixtureColumn::Date | FixtureColumn::Boolean => {
            unreachable!("only formula columns carry cached values")
        }
    }
}

fn formula_text(kind: FixtureColumn, rows: usize, row: usize) -> String {
    let a1_row = row + 2;
    let last_row = rows + 1;
    let letter = |column: usize| column_name(column);
    match kind {
        FixtureColumn::Product(left, right) => format!(
            "{a}{r}*{b}{r}",
            a = letter(left),
            b = letter(right),
            r = a1_row
        ),
        FixtureColumn::Sum(left, right) => format!(
            "{a}{r}+{b}{r}",
            a = letter(left),
            b = letter(right),
            r = a1_row
        ),
        FixtureColumn::Difference(left, right) => format!(
            "{a}{r}-{b}{r}",
            a = letter(left),
            b = letter(right),
            r = a1_row
        ),
        FixtureColumn::ColumnQuotient(dividend) => format!(
            "{l}{r}/SUM({l}$2:{l}${last_row})",
            l = letter(dividend),
            r = a1_row
        ),
        FixtureColumn::Number | FixtureColumn::Text | FixtureColumn::Date | FixtureColumn::Boolean => {
            unreachable!("only formula columns carry formulas")
        }
    }
}

fn intern(
    lookup: &mut std::collections::BTreeMap<String, usize>,
    values: &mut Vec<String>,
    value: &str,
) -> usize {
    if let Some(index) = lookup.get(value) {
        return *index;
    }
    let index = values.len();
    values.push(value.to_owned());
    lookup.insert(value.to_owned(), index);
    index
}

fn date_value(row: usize) -> String {
    format!("2024-{:02}-{:02}", 1 + row % 12, 1 + row % 28)
}

fn text_value(column: usize, row: usize) -> String {
    format!("t{column:02}-{row:05}")
}

fn build_xlsx(rows: usize, columns: &[FixtureColumn]) -> XlsxFixture {
    const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
    const REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
    const REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

    let mut lookup = std::collections::BTreeMap::new();
    let mut values = Vec::new();
    let mut string_cells = 0usize;
    let mut formulas = 0usize;

    let headers = (0..columns.len())
        .map(|column| format!("h{column}"))
        .collect::<Vec<_>>();
    let header_indices = headers
        .iter()
        .map(|header| {
            string_cells += 1;
            intern(&mut lookup, &mut values, header)
        })
        .collect::<Vec<_>>();

    // Grows past the worksheet part alone for S4, so reserve generously.
    let mut worksheet = String::with_capacity(rows * columns.len() * 48 + 4096);
    worksheet.push_str("<worksheet xmlns=\"");
    worksheet.push_str(MAIN);
    worksheet.push_str("\"><sheetData><row r=\"1\">");
    for (column, index) in header_indices.iter().enumerate() {
        worksheet.push_str(&format!(
            "<c r=\"{}{}\" t=\"s\"><v>{index}</v></c>",
            column_name(column),
            1
        ));
    }
    worksheet.push_str("</row>");
    for row in 0..rows {
        let source_row = row + 2;
        worksheet.push_str(&format!("<row r=\"{source_row}\">"));
        for (column, kind) in columns.iter().enumerate() {
            let address = format!("{}{}", column_name(column), source_row);
            match kind {
                FixtureColumn::Number => {
                    worksheet.push_str(&format!(
                        "<c r=\"{address}\"><v>{}</v></c>",
                        plain_number(row, column)
                    ));
                }
                FixtureColumn::Text => {
                    string_cells += 1;
                    let index = intern(&mut lookup, &mut values, &text_value(column, row));
                    worksheet.push_str(&format!(
                        "<c r=\"{address}\" t=\"s\"><v>{index}</v></c>"
                    ));
                }
                FixtureColumn::Date => {
                    worksheet.push_str(&format!(
                        "<c r=\"{address}\" t=\"d\"><v>{}</v></c>",
                        date_value(row)
                    ));
                }
                FixtureColumn::Boolean => {
                    worksheet.push_str(&format!(
                        "<c r=\"{address}\" t=\"b\"><v>{}</v></c>",
                        u8::from(row % 2 == 0)
                    ));
                }
                formula => {
                    formulas += 1;
                    worksheet.push_str(&format!(
                        "<c r=\"{address}\"><f>{}</f><v>{}</v></c>",
                        formula_text(*formula, rows, row),
                        cached_value(*formula, rows, row)
                    ));
                }
            }
        }
        worksheet.push_str("</row>");
    }
    worksheet.push_str("</sheetData></worksheet>");

    let worksheet_nodes = 2 + (rows + 1) + 2 * (rows + 1) * columns.len() + formulas;
    let shared_string_nodes = 1 + 2 * values.len();

    let mut shared = String::with_capacity(values.len() * 32 + 128);
    shared.push_str(&format!(
        "<sst xmlns=\"{MAIN}\" count=\"{string_cells}\" uniqueCount=\"{}\">",
        values.len()
    ));
    for value in &values {
        shared.push_str("<si><t xml:space=\"preserve\">");
        shared.push_str(value);
        shared.push_str("</t></si>");
    }
    shared.push_str("</sst>");

    let styles = format!(
        "<styleSheet xmlns=\"{MAIN}\"><fonts count=\"1\"><font><sz val=\"11\"/><name val=\"Arial\"/></font></fonts><fills count=\"2\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill></fills><borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs><cellXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/></cellXfs></styleSheet>"
    );
    let workbook = format!(
        "<workbook xmlns=\"{MAIN}\" xmlns:r=\"{REL_NS}\"><workbookPr date1904=\"0\"/><sheets><sheet name=\"Data\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>"
    );
    let workbook_rels = format!(
        "<Relationships xmlns=\"{REL}\"><Relationship Id=\"rId1\" Type=\"{REL_NS}/worksheet\" Target=\"worksheets/sheet1.xml\"/><Relationship Id=\"rId2\" Type=\"{REL_NS}/sharedStrings\" Target=\"sharedStrings.xml\"/><Relationship Id=\"rId3\" Type=\"{REL_NS}/styles\" Target=\"styles.xml\"/></Relationships>"
    );
    let root_rels = format!(
        "<Relationships xmlns=\"{REL}\"><Relationship Id=\"rId1\" Type=\"{REL_NS}/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>"
    );
    let content_types = "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/><Override PartName=\"/xl/sharedStrings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml\"/><Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/></Types>";

    let worksheet_bytes = worksheet.len();
    let parts = [
        ("[Content_Types].xml", content_types.to_owned()),
        ("_rels/.rels", root_rels),
        ("xl/_rels/workbook.xml.rels", workbook_rels),
        ("xl/sharedStrings.xml", shared),
        ("xl/styles.xml", styles),
        ("xl/workbook.xml", workbook),
        ("xl/worksheets/sheet1.xml", worksheet),
    ];
    let expanded_bytes = parts.iter().map(|(_, part)| part.len()).sum();
    let mut output = Cursor::new(Vec::new());
    {
        let mut writer = ZipWriter::new(&mut output);
        let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, part) in &parts {
            writer.start_file(*name, options).expect("fixture ZIP entry");
            writer.write_all(part.as_bytes()).expect("fixture ZIP write");
        }
        writer.finish().expect("fixture ZIP finish");
    }
    XlsxFixture {
        bytes: output.into_inner(),
        worksheet_bytes,
        worksheet_nodes,
        shared_string_nodes,
        expanded_bytes,
        rows,
        columns: columns.len(),
        formulas,
        types: columns.iter().map(|column| column.import_type()).collect(),
    }
}

fn fixture_columns(name: &str) -> (usize, Vec<FixtureColumn>) {
    use FixtureColumn::{Boolean, ColumnQuotient, Date, Difference, Number, Product, Sum, Text};
    let s1 = vec![
        Number, Number, Number, Number, Number, Text, Text, Text, Date, Boolean,
    ];
    let s2 = vec![
        Number, Number, Product(0, 1), Sum(0, 1), Difference(0, 1), Text, Text, Text, Date,
        Boolean,
    ];
    let s3 = vec![
        Number, Number, Number, Number, ColumnQuotient(1), Text, Text, Text, Date, Boolean,
    ];
    let mut s4 = vec![Number; 15];
    s4.extend([Text; 8]);
    s4.extend([Date; 2]);
    s4.extend([Boolean; 2]);
    s4.extend([Product(0, 1), Sum(0, 1), Difference(0, 1)]);
    match name {
        "S1" | "S1b" => (10_000, s1),
        "S2-250" => (250, s2),
        "S2-1000" => (1_000, s2),
        "S2-4000" => (4_000, s2),
        "S2" => (10_000, s2),
        "S3" => (10_000, s3),
        "S4" => (10_000, s4),
        other => panic!("unknown fixture {other}"),
    }
}

fn check(label: &str, attempted: usize, original_maximum: usize) {
    if attempted > original_maximum {
        say(format!(
            "  exceeded {label}: attempted {attempted} > original maximum {original_maximum}"
        ));
    }
}

fn observe_phase<R>(action: impl FnOnce() -> R) -> (R, f64, Option<u64>) {
    let (result, sample) = observe(action);
    (result, sample.millis, sample.peak_delta_bytes)
}

fn zip_expanded_bytes(bytes: &[u8]) -> usize {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("output ZIP is readable");
    let mut total = 0usize;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).expect("output ZIP entry");
        total += usize::try_from(entry.size()).expect("output entry size");
    }
    total
}

fn measure_fixture(name: &str) {
    let (rows, columns) = fixture_columns(name);
    let fixture = build_xlsx(rows, &columns);
    let display = name;
    say(format!(
        "{display}: source XLSX {} bytes, largest worksheet XML {} bytes, expanded {} bytes, worksheet nodes {}, sharedStrings nodes {}, {} formulas",
        fixture.bytes.len(),
        fixture.worksheet_bytes,
        fixture.expanded_bytes,
        fixture.worksheet_nodes,
        fixture.shared_string_nodes,
        fixture.formulas,
    ));
    say(format!("{display}: original limits exceeded while parsing the source (code order):"));
    check(
        "MAX_SOURCE_BYTES",
        fixture.bytes.len(),
        ORIGINAL_MAX_SOURCE_BYTES,
    );
    check(
        "MAX_EXPANDED_BYTES",
        fixture.expanded_bytes,
        ORIGINAL_MAX_EXPANDED_BYTES,
    );
    check(
        "MAX_XML_NODES (sharedStrings)",
        fixture.shared_string_nodes,
        ORIGINAL_MAX_XML_NODES,
    );
    check(
        "MAX_XML_NODES (worksheet)",
        fixture.worksheet_nodes,
        ORIGINAL_MAX_XML_NODES,
    );
    check(
        "MAX_WORKSHEET_ROW_INDEX_PREFLIGHT",
        fixture.rows,
        ORIGINAL_MAX_WORKSHEET_ROW_INDEX_PREFLIGHT,
    );
    check(
        "MAX_COLUMNS",
        fixture.columns,
        ORIGINAL_MAX_COLUMNS,
    );
    check("MAX_FORMULAS (source)", fixture.formulas, ORIGINAL_MAX_FORMULAS);
    check("MAX_DATA_ROWS", fixture.rows, ORIGINAL_MAX_DATA_ROWS);
    check(
        "MAX_TOTAL_DATA_ROWS",
        fixture.rows,
        ORIGINAL_MAX_TOTAL_DATA_ROWS,
    );

    say(format!("{display}: starting import_xlsx"));
    let (parsed, parse_ms, parse_peak) =
        observe_phase(|| import_xlsx_with(&fixture.bytes, &ImportOptions::default()));
    let parsed = match parsed {
        Ok(parsed) => parsed,
        Err(error) => {
            say(format!("{display}: import_xlsx refused: {error}"));
            return;
        }
    };
    say(format!(
        "{display}: import_xlsx done in {parse_ms:.1} ms, peak RSS {} MiB",
        mib(parse_peak)
    ));

    let selection = ImportSelection {
        column_types: vec![fixture.types.clone()],
        extra_columns: vec![Vec::new()],
    };
    say(format!("{display}: starting import_workbook"));
    let (imported, import_ms, import_peak) = observe_phase(|| {
        import_workbook(
            &parsed,
            &selection,
            "00000000-0000-4000-8000-00000000c0de",
        )
    });
    let (runtime, projection) = match imported {
        Ok(imported) => imported,
        Err(error) => {
            say(format!("{display}: import_workbook refused: {error}"));
            return;
        }
    };
    say(format!(
        "{display}: import_workbook done in {import_ms:.1} ms, peak RSS {} MiB",
        mib(import_peak)
    ));
    say(format!("{display}: original limits exceeded inside import_workbook (code order):"));
    let usage = &runtime.capacity_usage;
    check(
        "CapacityLimit::Collections",
        usage.collections,
        32,
    );
    let widest_rows = usage
        .tables
        .values()
        .map(|table| table.rows)
        .max()
        .unwrap_or(0);
    let widest_fields = usage
        .tables
        .values()
        .map(|table| table.fields)
        .max()
        .unwrap_or(0);
    check("CapacityLimit::TableFields", widest_fields, ORIGINAL_MAX_COLUMNS);
    check("CapacityLimit::TableRows", widest_rows, ORIGINAL_MAX_DATA_ROWS);
    check(
        "CapacityLimit::TotalEntities",
        usage.total_entities,
        ORIGINAL_MAX_TOTAL_DATA_ROWS,
    );
    check(
        "CapacityLimit::PopulatedCells",
        usage.populated_cells,
        ORIGINAL_MAX_POPULATED_CELLS,
    );
    check("CapacityLimit::Formulas", usage.formulas, ORIGINAL_MAX_FORMULAS);
    check(
        "CapacityLimit::FormulaAstNodes",
        usage.formula_ast_nodes,
        ORIGINAL_MAX_FORMULA_AST_NODES,
    );
    check(
        "CapacityLimit::DependencyEdges",
        usage.dependency_edges,
        ORIGINAL_MAX_DEPENDENCY_EDGES,
    );

    let reply_bytes = serde_json::to_vec(&crate::CompleteReply {
        status: "ok",
        response: crate::CompleteResponse {
            kind: "imported",
            payload: &projection,
        },
    })
    .expect("Imported reply serializes")
    .len();
    let metadata_bytes =
        serde_json::to_vec(&projection.metadata).expect("metadata serializes").len();
    say(format!("{display}: original limits exceeded while finishing the import (code order):"));
    check(
        "MAX_PROJECTION_BYTES (InteropMetadata)",
        metadata_bytes,
        ORIGINAL_MAX_PROJECTION_BYTES,
    );
    check(
        "MAX_PROJECTION_BYTES (Imported reply)",
        reply_bytes,
        ORIGINAL_MAX_PROJECTION_BYTES,
    );

    let survived = {
        let snapshot = runtime.session.export_snapshot();
        snapshot
            .document()
            .entities
            .values()
            .flat_map(|entity| entity.fields.values())
            .filter(|value| matches!(value, Value::Formula(_)))
            .count()
    };

    let revision = runtime.observe_occurrence().revision;
    let collection = {
        let snapshot = runtime.session.export_snapshot();
        select_default_collection(&collection_specs(snapshot.document()))
            .expect("the imported workbook has one collection")
    };
    say(format!("{display}: starting native tables XLSX export"));
    let (exported, export_ms, export_peak) = observe_phase(|| {
        let workbook = runtime
            .export_native_tables_workbook(&revision, &collection)
            .map_err(|error| error.to_string())?;
        let output = export_xlsx_for_profile(&workbook, OutputProfile::NativeTables)
            .map_err(|error| error.to_string())?;
        Ok::<_, String>((workbook, output))
    });
    let (workbook, output) = match exported {
        Ok((workbook, output)) => (workbook, output),
        Err(error) => {
            say(format!(
                "{display}: native tables XLSX export refused after {export_ms:.1} ms: {error}"
            ));
            say(format!(
                "| {display} | {rows} | {} | {} | {} | {} | {} | {parse_ms:.1} | {} | {import_ms:.1} | {} | {reply_bytes} | {metadata_bytes} | refused | | | | {survived} |",
                fixture.columns,
                fixture.formulas,
                fixture.bytes.len(),
                fixture.worksheet_bytes,
                fixture.expanded_bytes,
                mib(parse_peak),
                mib(import_peak),
            ));
            return;
        }
    };
    say(format!(
        "{display}: export done in {export_ms:.1} ms, peak RSS {} MiB",
        mib(export_peak)
    ));
    let exported_formulas = workbook
        .sheets
        .iter()
        .flat_map(|sheet| &sheet.rows)
        .flatten()
        .filter(|cell| cell.formula.is_some())
        .count();
    let chunks = survived.div_ceil(crate::MAX_FIELD_QUERY_TARGETS);
    let export_expanded = zip_expanded_bytes(&output);
    say(format!("{display}: original limits exceeded during export (code order):"));
    check(
        "MAX_NATIVE_TABLES_DATA_ROWS",
        rows,
        ORIGINAL_MAX_NATIVE_DATA_ROWS,
    );
    check(
        "MAX_NATIVE_TABLES_TOTAL_ROWS",
        rows,
        ORIGINAL_MAX_NATIVE_TOTAL_ROWS,
    );
    check(
        "MAX_NATIVE_TABLES_EXPANDED_BYTES",
        export_expanded,
        ORIGINAL_MAX_NATIVE_EXPANDED_BYTES,
    );
    check(
        "MAX_NATIVE_TABLES_OUTPUT_BYTES",
        output.len(),
        ORIGINAL_MAX_NATIVE_OUTPUT_BYTES,
    );
    if fixture.formulas > ORIGINAL_MAX_FORMULAS {
        say(format!(
            "  note: with the original MAX_FORMULAS={ORIGINAL_MAX_FORMULAS}, export would have written {} formula cells as computed values only.",
            exported_formulas
        ));
    }
    say(format!(
        "| {display} | {rows} | {} | {} | {} | {} | {} | {parse_ms:.1} | {} | {import_ms:.1} | {} | {reply_bytes} | {metadata_bytes} | {export_ms:.1} | {} | {export_expanded} | {chunks} | {survived}/{exported_formulas} |",
        fixture.columns,
        fixture.formulas,
        fixture.bytes.len(),
        fixture.worksheet_bytes,
        fixture.expanded_bytes,
        mib(parse_peak),
        mib(import_peak),
        output.len(),
    ));
}

#[test]
#[ignore = "working-size capacity measurement; run explicitly in release with --features capacity-bench"]
fn import_export_working_size() {
    if let Ok(name) = std::env::var("CAPACITY_MEASURE_FIXTURE") {
        measure_fixture(&name);
        return;
    }
    let executable = std::env::current_exe().expect("the test binary path is available");
    say(
        "| fixture | rows | columns | source formulas | source XLSX bytes | largest worksheet XML bytes | expanded bytes | import_xlsx ms | import_xlsx peak RSS MiB | import_workbook ms | import_workbook peak RSS MiB | Imported reply bytes | InteropMetadata bytes | export ms | output bytes | export expanded bytes | query_fields chunks | formulas survived |",
    );
    say(
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |",
    );
    for name in ["S2-250", "S2-1000", "S2-4000", "S1", "S2", "S3", "S4"] {
        let output = std::process::Command::new("timeout")
            .args(["-k", "10", PHASE_TIMEOUT_SECONDS])
            .arg(&executable)
            .args([
                "--ignored",
                "--exact",
                "capacity_bench::import_export_working_size",
                "--nocapture",
            ])
            .env("CAPACITY_MEASURE_FIXTURE", name)
            .output();
        match output {
            Ok(output) => {
                print!("{}", String::from_utf8_lossy(&output.stdout));
                if !output.stderr.is_empty() {
                    eprint!("{}", String::from_utf8_lossy(&output.stderr));
                }
                match output.status.code() {
                    Some(124) => say(format!(
                        "| {name} | over 20 minutes | | | | | | | | | | | | | | | | import_workbook exceeded 20 minutes (timeout {PHASE_TIMEOUT_SECONDS} s) |"
                    )),
                    Some(0) => {}
                    code => say(format!(
                        "| {name} | child exited with {code:?} | | | | | | | | | | | | | | | | not measured |"
                    )),
                }
                let _ = std::io::stdout().flush();
            }
            Err(error) => say(format!(
                "{name}: could not spawn the isolated measurement child: {error}"
            )),
        }
    }
}
