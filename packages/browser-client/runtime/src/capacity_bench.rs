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

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::time::Instant;

use tachiko_formula_engine::{RetainedCalculationState, calculate};
use tachiko_storage::{decode_roproj_v3, encode_roproj_v3};
use tachiko_workspace_engine::{
    Document, Expression, FieldConstraint, FieldId, Number, Value,
    instrumentation::{self, CapacityCounters},
    patch_lifecycle::{DocumentScopeId, SemanticPublicationAuthority, TrustedInstant},
    resident_session::{ResidentWorkspaceSession, TrustedPublicationTimeSource},
    validate,
};

use super::{
    CellEdit, ComparisonOperator, DESIGNER_PRINCIPAL, DesignerClock, DesignerRequest,
    DesignerResponse, DesignerRuntime, Entity, EntityId, EntityKey, FieldDefinition, FieldKey,
    FieldTarget, FieldType, PrincipalId, RowInitializer, ScalarEditInput, Schema, SchemaId,
    SchemaKey, SortDirection, TableView, ViewFilter, ViewIndexCache, ViewSortKey,
    collection_specs, decode_project_bundle, designer_lifecycle, document_requires_v2_evidence,
    document_scope, encode_project_bundle, ensure_projection_size_with_limit, formula_sources,
    open_portable_ro_working_size, open_project, open_project_working_size, process_wire_request,
    select_default_collection,
};
use crate::capacity::SessionProfile;
use crate::capacity_fixtures::{
    DEPENDENT_FORMULAS, FixtureSpec, RATE_FIELD, SWEEP_DATA_COLUMNS, capacity_document, entity_id,
    field_id, formula_sweep_document, w1_copy_document, w1_document,
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
        profile: SessionProfile::Ordinary,
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
        presentation: crate::PresentationStore::default(),
        presentation_saved_bytes: 0,
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

/// One working-size qualification shape: 10,000 rows by 30 declared columns.
///
/// Every field carries a display name, so both shapes take the v2 evidence
/// path Sheet's blank workbook takes.
#[derive(Clone, Copy, Debug)]
enum WqShape {
    /// Ten populated Number columns (100,000 cells) and twenty empty ones.
    Values,
    /// Nine populated Number columns and one same-row formula column
    /// (10,000 formulas, 100,000 cells).
    Formulas,
}

impl WqShape {
    const ALL: [Self; 2] = [Self::Values, Self::Formulas];

    const fn name(self) -> &'static str {
        match self {
            Self::Values => "values",
            Self::Formulas => "formulas",
        }
    }

    const fn populated_columns(self) -> usize {
        match self {
            Self::Values => 10,
            Self::Formulas => 9,
        }
    }
}

const WQ_OCCURRENCE: &str = "00000000-0000-4000-8000-000000000678";
const WQ_ROWS: usize = 10_000;
const WQ_COLUMNS: usize = 30;
const WQ_EDIT_COUNT: usize = 20;

fn wq_document(shape: WqShape) -> Document {
    let mut document = Document::empty("working-size-qualification", "Working Size Qualification");
    let schema_id = SchemaId::from("grid");
    let mut schema = Schema {
        field_order: None,
        id: schema_id.clone(),
        key: SchemaKey::from("grid"),
        display_name: None,
        fields: BTreeMap::new(),
    };
    for column in 0..WQ_COLUMNS {
        let id = FieldId::from(format!("c{column}"));
        schema.fields.insert(
            id.clone(),
            FieldDefinition {
                id,
                key: FieldKey::from(format!("c{column}")),
                display_name: Some(format!("Column {}", column + 1)),
                field_type: FieldType::Number,
                required: false,
                constraint: FieldConstraint::None,
                number_format: None,
            },
        );
    }
    document.schemas.insert(schema_id.clone(), schema);
    for row in 0..WQ_ROWS {
        let id = EntityId::from(format!("r{row:05}"));
        let mut fields = BTreeMap::new();
        for column in 0..shape.populated_columns() {
            let value = u32::try_from(row % 1_000).expect("bounded row") + 1;
            fields.insert(
                FieldId::from(format!("c{column}")),
                number(f64::from(value)),
            );
        }
        if matches!(shape, WqShape::Formulas) {
            fields.insert(
                FieldId::from("c9"),
                Value::Formula(Expression::Add {
                    left: Box::new(Expression::SameRowReference(FieldId::from("c0"))),
                    right: Box::new(Expression::SameRowReference(FieldId::from("c1"))),
                }),
            );
        }
        document.entities.insert(
            id.clone(),
            Entity {
                id,
                key: EntityKey::from(format!("k{row:05}")),
                schema: schema_id.clone(),
                fields,
            },
        );
    }
    document
}

/// One machine line per fact, flushed as it is printed.
fn wq_print(shape: &str, fact: &str, value: impl std::fmt::Display) {
    println!("WQ|{shape}|{fact}|{value}");
    let _ = std::io::stdout().flush();
}

fn wq_refused(shape: &str, step: &str, error: &super::DesignerError) {
    let code = error.failure_projection("").code;
    wq_print(shape, step, format!("refused:{code}"));
}

fn wq_millis(sample: &Sample) -> String {
    format!("{:.3}", sample.millis)
}

/// Working-size qualification: the two 10,000-row by 30-column shapes Sheet's
/// blank workbook can produce, measured through the admitted runtime in
/// release. A refused step prints `refused:<code>` and the run continues.
#[test]
#[ignore = "working-size qualification; run explicitly in release with --features capacity-bench"]
fn working_size_qualification() {
    for shape in WqShape::ALL {
        qualify_working_size_shape(shape);
    }
}

#[allow(clippy::too_many_lines)] // One linear per-shape script is easier to audit.
fn qualify_working_size_shape(shape: WqShape) {
    let name = shape.name();
    let document = wq_document(shape);
    assert!(
        document_requires_v2_evidence(&document),
        "{name} must take the v2 evidence path"
    );
    validate(&document).expect("qualification fixture validates");

    // These fixtures carry no presentation, so the document bound is the whole
    // saved-size bound the working-size session checks.
    let (saved_bound, bound_sample) =
        observe(|| super::save_size::saved_bytes_upper_bound(&document));
    wq_print(name, "saved_bound_ms", wq_millis(&bound_sample));
    wq_print(name, "saved_bound_bytes", saved_bound);

    let runtime = match DesignerRuntime::from_document_working_size(document, WQ_OCCURRENCE) {
        Ok(runtime) => runtime,
        Err(error) => {
            wq_refused(name, "open_document", &error);
            return;
        }
    };
    let revision = runtime.observe_occurrence().revision;
    let (saved, save_sample) = observe(|| runtime.export_project_v3(&revision));
    let saved = match saved {
        Ok(saved) => saved,
        Err(error) => {
            wq_refused(name, "save_v3", &error);
            return;
        }
    };
    wq_print(name, "save_v3_ms", wq_millis(&save_sample));
    wq_print(name, "save_v3_bytes", saved.bytes.len());
    wq_print(name, "saved_actual_bytes", saved.bytes.len());
    wq_print(
        name,
        "saved_margin_bytes",
        saved_bound.saturating_sub(u64::try_from(saved.bytes.len()).unwrap_or(u64::MAX)),
    );
    drop(runtime);

    let mut resident: Option<DesignerRuntime> = None;
    let (opened, open_sample) =
        observe(|| open_project_working_size(&mut resident, &saved.bytes, WQ_OCCURRENCE));
    let opened = match opened {
        Ok(opened) => opened,
        Err(error) => {
            wq_refused(name, "open", &error);
            return;
        }
    };
    wq_print(name, "open_ms", wq_millis(&open_sample));
    wq_print(
        name,
        "open_peak_rss",
        describe_bytes(open_sample.peak_delta_bytes),
    );
    wq_print(name, "window_rows", opened.window.rows.len());
    wq_print(name, "total_rows", opened.window.total_rows);
    drop(opened);
    let Some(mut runtime) = resident else {
        return;
    };

    qualify_working_size_edits(shape, &mut runtime);
    qualify_working_size_saves(shape, &runtime);

    let (reply, _) = observe(|| {
        runtime.handle(DesignerRequest::QueryCapacity {
            expected_revision: None,
        })
    });
    match reply {
        Ok(DesignerResponse::Capacity(projection)) => {
            let usage = projection.usage;
            wq_print(name, "capacity_edges", usage.dependency_edges);
            wq_print(name, "capacity_cells", usage.populated_cells);
            wq_print(name, "capacity_formulas", usage.formulas);
        }
        Ok(_) => wq_print(name, "capacity", "unexpected_reply"),
        Err(error) => wq_refused(name, "capacity", &error),
    }
}

#[allow(clippy::too_many_lines)] // One linear per-shape script is easier to audit.
fn qualify_working_size_edits(shape: WqShape, runtime: &mut DesignerRuntime) {
    let name = shape.name();
    // c0 feeds the same-row formula column in the `formulas` shape.
    let field = "c0";
    let mut samples = Vec::with_capacity(WQ_EDIT_COUNT);
    for row in 0..WQ_EDIT_COUNT {
        let revision = runtime.observe_occurrence().revision;
        let target = FieldTarget {
            entity: format!("r{row:05}"),
            field: field.to_owned(),
        };
        let input = ScalarEditInput::Number {
            input: format!("{}", 100 + row),
        };
        let (result, sample) = observe(|| {
            runtime.edit_cells(
                &revision,
                &[CellEdit {
                    target: target.clone(),
                    input: input.clone(),
                }],
            )
        });
        match result {
            Ok(_) => samples.push(sample),
            Err(error) => {
                wq_refused(name, "edit", &error);
                return;
            }
        }
    }
    let mut ordered = samples
        .iter()
        .map(|sample| sample.millis)
        .collect::<Vec<_>>();
    ordered.sort_by(f64::total_cmp);
    let median = ordered[ordered.len() / 2];
    let p95_index = ordered
        .len()
        .saturating_mul(95)
        .div_ceil(100)
        .saturating_sub(1);
    wq_print(name, "edit_n", ordered.len());
    wq_print(name, "edit_median_ms", format!("{median:.3}"));
    wq_print(name, "edit_p95_ms", format!("{:.3}", ordered[p95_index]));

    // The same edit through the reply boundary, where a working-size session
    // builds the changed-cell list. The JSON reply's size is the list's cost.
    let mut handle_samples = Vec::with_capacity(WQ_EDIT_COUNT);
    let mut handle_phases: Vec<BTreeMap<&'static str, f64>> = Vec::with_capacity(WQ_EDIT_COUNT);
    let mut largest_reply = 0;
    for index in 0..WQ_EDIT_COUNT {
        let revision = runtime.observe_occurrence().revision;
        let request = DesignerRequest::EditCells {
            expected_revision: revision,
            edits: vec![CellEdit {
                target: FieldTarget {
                    entity: format!("r{:05}", WQ_EDIT_COUNT + index),
                    field: field.to_owned(),
                },
                input: ScalarEditInput::Number {
                    input: format!("{}", 200 + index),
                },
            }],
        };
        instrumentation::reset_phases();
        let (result, sample) = observe(|| runtime.handle(request));
        let phases = phase_millis(&instrumentation::take_phases());
        match result {
            Ok(response) => {
                handle_samples.push(sample);
                handle_phases.push(phases);
                let encoded = serde_json::to_vec(&response).expect("the reply encodes");
                largest_reply = largest_reply.max(encoded.len());
            }
            Err(error) => {
                wq_refused(name, "handle_edit", &error);
                return;
            }
        }
    }
    let mut ordered = handle_samples
        .iter()
        .map(|sample| sample.millis)
        .collect::<Vec<_>>();
    ordered.sort_by(f64::total_cmp);
    let median = ordered[ordered.len() / 2];
    let p95_index = ordered
        .len()
        .saturating_mul(95)
        .div_ceil(100)
        .saturating_sub(1);
    wq_print(name, "handle_edit_median_ms", format!("{median:.3}"));
    wq_print(
        name,
        "handle_edit_p95_ms",
        format!("{:.3}", ordered[p95_index]),
    );
    wq_print(name, "edit_reply_bytes", largest_reply);

    // Per-phase cost of the command-to-reply path.
    let phase_names = handle_phases
        .iter()
        .flat_map(|phases| phases.keys().copied())
        .collect::<BTreeSet<_>>();
    for phase in phase_names {
        let mut samples = handle_phases
            .iter()
            .map(|phases| phases.get(phase).copied().unwrap_or(0.0))
            .collect::<Vec<_>>();
        samples.sort_by(f64::total_cmp);
        let phase_median = samples[samples.len() / 2];
        let phase_p95 = w1_percentile(&samples, 95.0);
        wq_print(
            name,
            &format!("phase_{phase}_median_ms"),
            format!("{phase_median:.3}"),
        );
        wq_print(
            name,
            &format!("phase_{phase}_p95_ms"),
            format!("{phase_p95:.3}"),
        );
    }

    // A full table refuses another row, so removal comes first.
    let revision = runtime.observe_occurrence().revision;
    let rows = (0..100).map(|row| format!("r{row:05}")).collect::<Vec<_>>();
    let (removed, sample) = observe(|| runtime.remove_table_rows(&revision, "grid", &rows));
    match removed {
        Ok(_) => wq_print(name, "remove_100_rows_ms", wq_millis(&sample)),
        Err(error) => wq_refused(name, "remove_100_rows", &error),
    }

    let revision = runtime.observe_occurrence().revision;
    let initializers = [RowInitializer {
        field: field.to_owned(),
        input: ScalarEditInput::Number {
            input: "7".to_owned(),
        },
    }];
    let (inserted, sample) = observe(|| runtime.insert_row(&revision, "grid", &initializers));
    match inserted {
        Ok(_) => wq_print(name, "insert_row_ms", wq_millis(&sample)),
        Err(error) => wq_refused(name, "insert_row", &error),
    }
}

fn qualify_working_size_saves(shape: WqShape, runtime: &DesignerRuntime) {
    let name = shape.name();
    let revision = runtime.observe_occurrence().revision;

    let (canonical, sample) = observe(|| runtime.export_canonical_tree(&revision));
    match canonical {
        Ok(export) => {
            wq_print(name, "save_canonical_ms", wq_millis(&sample));
            wq_print(name, "save_canonical_bytes", export.bytes.len());
            let mut resident: Option<DesignerRuntime> = None;
            let (reopened, reopen_sample) =
                observe(|| open_project_working_size(&mut resident, &export.bytes, WQ_OCCURRENCE));
            match reopened {
                Ok(_) => wq_print(name, "reopen_canonical_ms", wq_millis(&reopen_sample)),
                Err(error) => wq_refused(name, "reopen_canonical", &error),
            }
        }
        Err(error) => wq_refused(name, "save_canonical", &error),
    }

    let (portable, sample) = observe(|| runtime.export_portable_ro(&revision));
    match portable {
        Ok(export) => {
            wq_print(name, "save_portable_ms", wq_millis(&sample));
            wq_print(name, "save_portable_bytes", export.bytes.len());
            let mut resident: Option<DesignerRuntime> = None;
            let (reopened, reopen_sample) = observe(|| {
                open_portable_ro_working_size(&mut resident, &export.bytes, WQ_OCCURRENCE)
            });
            match reopened {
                Ok(_) => wq_print(name, "reopen_portable_ms", wq_millis(&reopen_sample)),
                Err(error) => wq_refused(name, "reopen_portable", &error),
            }
        }
        Err(error) => wq_refused(name, "save_portable", &error),
    }
}

// ---------------------------------------------------------------------------
// Working-size fill and paste through the wire (#678, stage 4).
//
// Each step builds a fresh working-size runtime, sends its request as JSON
// through `process_wire_request`, and reports one `WB|<step>|<fact>|<value>`
// line per fact. A refusal prints `WB|<step>|refused|<code>` and the run
// continues.
// ---------------------------------------------------------------------------

const WB_OCCURRENCE: &str = "00000000-0000-4000-8000-000000000678";
const WB_ROWS: usize = 10_000;
const WB_COLUMNS: usize = 10;

fn wb_print(step: &str, fact: &str, value: impl std::fmt::Display) {
    println!("WB|{step}|{fact}|{value}");
    let _ = std::io::stdout().flush();
}

fn wb_refused(step: &str, error: &super::DesignerError) {
    let code = error.failure_projection("").code;
    wb_print(step, "refused", code);
}

/// A working-size table with ten Number columns and no rows.
fn wb_blank_document() -> Document {
    let mut document = Document::empty("working-size-bulk", "Working Size Bulk");
    let schema_id = SchemaId::from("grid");
    let mut schema = Schema {
        field_order: None,
        id: schema_id.clone(),
        key: SchemaKey::from("grid"),
        display_name: None,
        fields: BTreeMap::new(),
    };
    for column in 0..WB_COLUMNS {
        let id = FieldId::from(format!("c{column}"));
        schema.fields.insert(
            id.clone(),
            FieldDefinition {
                id,
                key: FieldKey::from(format!("c{column}")),
                display_name: None,
                field_type: FieldType::Number,
                required: false,
                constraint: FieldConstraint::None,
                number_format: None,
            },
        );
    }
    document.schemas.insert(schema_id, schema);
    document
}

/// A working-size table with `rows` existing rows, every cell populated.
fn wb_seeded_document(rows: usize) -> Document {
    let mut document = wb_blank_document();
    let schema_id = SchemaId::from("grid");
    for row in 0..rows {
        let value = f64::from(u32::try_from(row).expect("bounded row")) + 1.0;
        let fields = (0..WB_COLUMNS)
            .map(|column| (FieldId::from(format!("c{column}")), number(value)))
            .collect();
        let id = EntityId::from(format!("r{row:05}"));
        document.entities.insert(
            id.clone(),
            Entity {
                id,
                key: EntityKey::from(format!("k{row:05}")),
                schema: schema_id.clone(),
                fields,
            },
        );
    }
    document
}

fn wb_open(step: &str, document: Document) -> Option<DesignerRuntime> {
    match DesignerRuntime::from_document_working_size(document, WB_OCCURRENCE) {
        Ok(runtime) => Some(runtime),
        Err(error) => {
            wb_refused(step, &error);
            None
        }
    }
}

fn wb_field(entity: &str, field: &str) -> FieldTarget {
    FieldTarget {
        entity: entity.to_owned(),
        field: field.to_owned(),
    }
}

/// Send one request through the wire, print its facts, then undo it the same
/// way and print how long that took.
fn wb_measure(step: &str, mut resident: Option<DesignerRuntime>, request: &DesignerRequest) {
    let bytes = match serde_json::to_vec(&request) {
        Ok(bytes) => bytes,
        Err(error) => {
            wb_print(step, "refused", format!("encode:{error}"));
            return;
        }
    };
    wb_print(step, "request_bytes", bytes.len());
    let (reply, sample) = observe(|| process_wire_request(&mut resident, &bytes));
    wb_print(step, "ms", wq_millis(&sample));
    let reply_length = reply.len();
    let reply = match serde_json::from_slice::<super::DesignerWireReply>(&reply) {
        Ok(reply) => reply,
        Err(error) => {
            wb_print(step, "refused", format!("decode:{error}"));
            return;
        }
    };
    match reply {
        super::DesignerWireReply::Ok {
            response: DesignerResponse::Published(publication),
        } => {
            wb_print(step, "reply_bytes", reply_length);
            wb_print(step, "many", publication.many);
        }
        super::DesignerWireReply::Error { error } => {
            wb_print(step, "refused", error.code);
            return;
        }
        other @ super::DesignerWireReply::Ok { .. } => {
            wb_print(step, "refused", format!("unexpected:{other:?}"));
            return;
        }
    }

    let Some(runtime) = resident.as_ref() else {
        return;
    };
    let undo = DesignerRequest::Undo {
        expected_revision: runtime.current_revision().to_owned(),
    };
    let Ok(undo) = serde_json::to_vec(&undo) else {
        wb_print(step, "refused", "undo_encode");
        return;
    };
    let (undo_reply, undo_sample) = observe(|| process_wire_request(&mut resident, &undo));
    wb_print(step, "undo_ms", wq_millis(&undo_sample));
    if !matches!(
        serde_json::from_slice::<super::DesignerWireReply>(&undo_reply),
        Ok(super::DesignerWireReply::Ok { .. })
    ) {
        wb_print(step, "undo_refused", "undo");
    }
}

fn wb_paste(step: &str, existing_rows: usize, pasted_rows: usize) {
    let Some(runtime) = wb_open(step, wb_seeded_document(existing_rows)) else {
        return;
    };
    let rows = (0..pasted_rows)
        .map(|row| {
            (0..WB_COLUMNS)
                .map(|column| {
                    super::PasteCellInput::Text((row * WB_COLUMNS + column + 1).to_string())
                })
                .collect()
        })
        .collect();
    let request = DesignerRequest::PasteCells {
        expected_revision: runtime.current_revision().to_owned(),
        collection: "grid".to_owned(),
        start_entity: None,
        start_field: "c0".to_owned(),
        rows,
    };
    wb_measure(step, Some(runtime), &request);
}

/// `count` edits over the first `count` cells of the ten-column values shape,
/// row-major: `100_000` addresses every populated cell.
fn wb_value_edits(count: usize) -> Vec<CellEdit> {
    (0..count)
        .map(|index| CellEdit {
            target: wb_field(
                &format!("r{:05}", index / WB_COLUMNS),
                &format!("c{}", index % WB_COLUMNS),
            ),
            input: ScalarEditInput::Number {
                input: (index + 1).to_string(),
            },
        })
        .collect()
}

fn wb_edit_values(step: &str, count: usize) {
    let Some(runtime) = wb_open(step, wq_document(WqShape::Values)) else {
        return;
    };
    let request = DesignerRequest::EditCells {
        expected_revision: runtime.current_revision().to_owned(),
        edits: wb_value_edits(count),
    };
    wb_measure(step, Some(runtime), &request);
}

fn wb_edit_formulas() {
    const STEP: &str = "edit_formulas_10000";
    let Some(runtime) = wb_open(STEP, wq_document(WqShape::Formulas)) else {
        return;
    };
    let edits = (0..WB_ROWS)
        .map(|row| CellEdit {
            target: wb_field(&format!("r{row:05}"), "c0"),
            input: ScalarEditInput::Number {
                input: (row + 1).to_string(),
            },
        })
        .collect();
    let request = DesignerRequest::EditCells {
        expected_revision: runtime.current_revision().to_owned(),
        edits,
    };
    wb_measure(STEP, Some(runtime), &request);
}

fn wb_fill_9999() {
    const STEP: &str = "fill_9999";
    let Some(runtime) = wb_open(STEP, wq_document(WqShape::Values)) else {
        return;
    };
    let mut resident = Some(runtime);
    let update = DesignerRequest::FormulaUpdate {
        expected_revision: resident
            .as_ref()
            .expect("the runtime is resident")
            .current_revision()
            .to_owned(),
        target: wb_field("r00000", "c0"),
        source: "[.c1]".to_owned(),
    };
    let Ok(bytes) = serde_json::to_vec(&update) else {
        wb_print(STEP, "refused", "formula_encode");
        return;
    };
    wb_print(STEP, "formula_request_bytes", bytes.len());
    let (reply, sample) = observe(|| process_wire_request(&mut resident, &bytes));
    wb_print(STEP, "formula_ms", wq_millis(&sample));
    if !matches!(
        serde_json::from_slice::<super::DesignerWireReply>(&reply),
        Ok(super::DesignerWireReply::Ok { .. })
    ) {
        wb_print(STEP, "refused", "formula_update");
        return;
    }
    let request = DesignerRequest::CopyFormula {
        expected_revision: resident
            .as_ref()
            .expect("the runtime is resident")
            .current_revision()
            .to_owned(),
        source: wb_field("r00000", "c0"),
        destinations: (1..WB_ROWS)
            .map(|row| wb_field(&format!("r{row:05}"), "c0"))
            .collect(),
        fixed_references: Vec::new(),
        relative_rows: true,
        relative_columns: false,
    };
    wb_measure(STEP, resident, &request);
}

/// Whole-column fill and large pastes through the wire (#678, stage 4).
#[test]
#[ignore = "working-size bulk benchmark; run explicitly in release with --features capacity-bench"]
fn working_size_bulk() {
    wb_paste("paste_blank", 0, WB_ROWS);
    wb_paste("paste_append_5000", 5_000, 5_000);
    wb_edit_values("edit_values_10000", 10_000);
    wb_edit_values("edit_values_100000", 100_000);
    wb_edit_formulas();
    wb_fill_9999();
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
    ("handle/changed_cells", 1),
    ("handle/build_response", 1),
    ("edit_cells", 0),
    ("edit_cells/pre_publish", 1),
    ("record_edit/history", 1),
    ("publish_commands", 1),
    ("publish_commands/export_snapshot_clone", 2),
    ("publish_commands/apply_commands", 2),
    ("publish_commands/capacity_check", 2),
    ("publish_commands/saved_bytes", 2),
    ("publish_commands/validate", 2),
    ("publish_commands/ensure_document_profile", 2),
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
    ("refresh_structure/collection_specs", 3),
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
        DesignerRuntime::from_document_profile(document, &occurrence, SessionProfile::TextCapacity)
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
    // The first edit pays one-off lazy setup; keep only the steady-state edits.
    if samples.len() > 1 {
        samples.remove(0);
        wall_millis.remove(0);
        clone_counts.remove(0);
        edits_run -= 1;
        println!("first edit dropped as warm-up");
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
/// A, B, C and D through the ordinary constructor, then through the
/// text-capacity constructor (refused for the readable profiles). Seven edits
/// per run; the first is dropped as warm-up and the remaining six are reported.
#[test]
#[ignore = "capacity phase breakdown; run explicitly in release with --features capacity-bench"]
fn capacity_edit_phase_breakdown() {
    const EDITS: usize = 7;
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
        measure_phase_fixture(label, spec, target, true, EDITS, FIXTURE_TIME_LIMIT_MILLIS);
    }
}

// ---------------------------------------------------------------------------
// Plan W1 working-size measurement (scratch, never merge).
//
// One shape per process (`W1_SHAPE`), so VmHWM covers that shape alone. Every
// result is one flushed `W1|<shape>|<metric>|<value>` line: a killed process
// keeps what it printed. Any `handle` error ends that phase only.
// ---------------------------------------------------------------------------

/// What one scalar edit writes.
#[derive(Clone, Copy)]
enum W1Input {
    Number,
    Text,
}

/// One measured scalar edit.
struct W1EditSample {
    millis: f64,
    clones: u64,
    json_bytes: usize,
    phases: BTreeMap<&'static str, f64>,
}

fn w1_line(shape: &str, metric: &str, value: impl std::fmt::Display) {
    println!("W1|{shape}|{metric}|{value}");
    let _ = std::io::stdout().flush();
}

fn w1_failure(shape: &str, phase: &str, error: impl std::fmt::Display) {
    w1_line(
        shape,
        &format!("{phase}_error"),
        format!("{error}").replace('\n', " "),
    );
}

fn w1_optional(value: Option<u64>) -> String {
    value.map_or_else(|| "unavailable".to_owned(), |bytes| bytes.to_string())
}

/// Nearest-rank percentile; empty input is a caller bug.
fn w1_percentile(values: &[f64], percentile: f64) -> f64 {
    let mut ordered = values.to_vec();
    ordered.sort_by(f64::total_cmp);
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let rank = ((percentile / 100.0) * ordered.len() as f64).ceil() as usize;
    ordered[rank.saturating_sub(1).min(ordered.len() - 1)]
}

fn w1_duration_summary(shape: &str, prefix: &str, millis: &[f64]) {
    if millis.is_empty() {
        return;
    }
    w1_line(
        shape,
        &format!("{prefix}_median_ms"),
        format!("{:.3}", median(millis.to_vec())),
    );
    w1_line(
        shape,
        &format!("{prefix}_p95_ms"),
        format!("{:.3}", w1_percentile(millis, 95.0)),
    );
    w1_line(
        shape,
        &format!("{prefix}_max_ms"),
        format!("{:.3}", millis.iter().copied().fold(f64::MIN, f64::max)),
    );
}

/// `MemTotal` from `/proc/meminfo`, in MiB.
fn w1_mem_total_mib() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = text.lines().find(|line| line.starts_with("MemTotal:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib / 1024)
}

/// Print the median phase table for measured edits, in `PHASE_ORDER` order,
/// then any label the order does not know.
fn w1_phase_table(label: &str, samples: &[W1EditSample], whole_median: f64) {
    println!("| phase `{label}` | median ms | share of edit |");
    println!("| --- | ---: | ---: |");
    let median_for = |name: &str| {
        median(
            samples
                .iter()
                .map(|sample| sample.phases.get(name).copied().unwrap_or(0.0))
                .collect(),
        )
    };
    for (name, depth) in PHASE_ORDER {
        if !samples
            .iter()
            .any(|sample| sample.phases.contains_key(name))
        {
            continue;
        }
        let median_ms = median_for(name);
        let share = if whole_median > 0.0 {
            median_ms / whole_median * 100.0
        } else {
            0.0
        };
        let indent = "  ".repeat(*depth);
        println!("| {indent}`{name}` | {median_ms:.3} | {share:.1}% |");
    }
    let mut unknown: Vec<&'static str> = Vec::new();
    for sample in samples {
        for name in sample.phases.keys() {
            if !PHASE_ORDER.iter().any(|(known, _)| known == name) && !unknown.contains(name) {
                unknown.push(name);
            }
        }
    }
    for name in unknown {
        let median_ms = median_for(name);
        let share = if whole_median > 0.0 {
            median_ms / whole_median * 100.0
        } else {
            0.0
        };
        println!("| `{name}` | {median_ms:.3} | {share:.1}% |");
    }
}

/// Run `EditScalar` samples through `handle`, drop the first as warm-up, and
/// print count, durations, clones, reply bytes and the median phase table.
#[allow(clippy::too_many_arguments)] // One measurement call carries its own knobs.
static W1_VALUE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Never the same value twice in one process: the same row and value again
/// would be refused as a no-op and end the phase.
fn w1_next_value() -> u64 {
    100_000 + W1_VALUE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn w1_lite() -> bool {
    std::env::var("W1_MODE").is_ok_and(|value| value == "lite")
}

fn w1_scalar_edits(
    shape: &str,
    label: &str,
    runtime: &mut DesignerRuntime,
    rows: usize,
    field: &str,
    row_zero: bool,
    input: W1Input,
    max_edits: usize,
    time_limit_ms: f64,
) -> Vec<W1EditSample> {
    let started = Instant::now();
    let mut stopped = false;
    let mut samples = Vec::new();
    for iteration in 0..max_edits {
        if iteration >= (if w1_lite() { 3 } else { 20 }) && started.elapsed().as_secs_f64() * 1_000.0 > time_limit_ms {
            stopped = true;
            break;
        }
        let row = if row_zero {
            0
        } else {
            (iteration * 7919) % rows
        };
        let revision = runtime.observe_occurrence().revision;
        let target = FieldTarget {
            entity: entity_id(row).to_string(),
            field: field.to_owned(),
        };
        #[allow(clippy::cast_precision_loss)]
        let edit_input = match input {
            W1Input::Number => ScalarEditInput::Number {
                input: format!("{}", w1_next_value()),
            },
            W1Input::Text => ScalarEditInput::Text {
                value: format!("w1-{}", w1_next_value()),
            },
        };
        instrumentation::reset();
        instrumentation::reset_phases();
        let edit_started = Instant::now();
        let response = runtime.handle(DesignerRequest::EditScalar {
            expected_revision: revision,
            target,
            input: edit_input,
        });
        let millis = edit_started.elapsed().as_secs_f64() * 1_000.0;
        let phases = phase_millis(&instrumentation::take_phases());
        let clones = instrumentation::counters().document_clones;
        match response {
            Ok(DesignerResponse::Published(publication)) => {
                let json_bytes = serde_json::to_vec(&publication).map_or(0, |bytes| bytes.len());
                samples.push(W1EditSample {
                    millis,
                    clones,
                    json_bytes,
                    phases,
                });
            }
            Ok(_) => {
                w1_failure(shape, label, "handle returned a non-published response");
                break;
            }
            Err(error) => {
                w1_failure(shape, label, error);
                break;
            }
        }
    }
    if samples.len() > 1 {
        samples.remove(0);
        println!("{label}: first edit dropped as warm-up");
    }
    w1_line(shape, &format!("{label}_n"), samples.len());
    if stopped {
        w1_line(
            shape,
            &format!("{label}_stopped"),
            format!("limit {time_limit_ms:.0} ms"),
        );
    }
    if samples.is_empty() {
        return samples;
    }
    let millis = samples
        .iter()
        .map(|sample| sample.millis)
        .collect::<Vec<_>>();
    let whole_median = median(millis.clone());
    w1_duration_summary(shape, label, &millis);
    #[allow(clippy::cast_precision_loss)]
    let clones = median(samples.iter().map(|sample| sample.clones as f64).collect());
    w1_line(
        shape,
        &format!("{label}_clones_median"),
        format!("{clones:.0}"),
    );
    #[allow(clippy::cast_precision_loss)]
    let bytes = samples
        .iter()
        .map(|sample| sample.json_bytes as f64)
        .collect::<Vec<_>>();
    w1_line(
        shape,
        &format!("{label}_json_bytes_median"),
        format!("{:.0}", median(bytes.clone())),
    );
    w1_line(
        shape,
        &format!("{label}_json_bytes_max"),
        format!("{:.0}", bytes.iter().copied().fold(0.0, f64::max)),
    );
    w1_phase_table(label, &samples, whole_median);
    samples
}

/// Varied Number edits over every row of `c0`, one `EditCells` request each.
fn w1_edit_cells(
    shape: &str,
    runtime: &mut DesignerRuntime,
    rows: usize,
    samples_wanted: usize,
    time_limit_ms: f64,
) {
    let started = Instant::now();
    let mut millis = Vec::new();
    for sample_index in 0..samples_wanted {
        if sample_index > 0 && started.elapsed().as_secs_f64() * 1_000.0 > time_limit_ms {
            w1_line(
                shape,
                "edit_cells_stopped",
                format!("limit {time_limit_ms:.0} ms"),
            );
            break;
        }
        let revision = runtime.observe_occurrence().revision;
        #[allow(clippy::cast_precision_loss)]
        let offset = (sample_index as f64 + 1.0) * 10.0;
        let edits = (0..rows)
            .map(|row| {
                #[allow(clippy::cast_precision_loss)]
                let value = offset + row as f64;
                CellEdit {
                    target: FieldTarget {
                        entity: entity_id(row).to_string(),
                        field: "c0".to_owned(),
                    },
                    input: ScalarEditInput::Number {
                        input: format!("{value}"),
                    },
                }
            })
            .collect::<Vec<_>>();
        instrumentation::reset_phases();
        let (response, sample) = observe(|| {
            runtime.handle(DesignerRequest::EditCells {
                expected_revision: revision,
                edits,
            })
        });
        w1_print_phases("edit_cells", sample.millis);
        match response {
            Ok(DesignerResponse::Published(_)) => millis.push(sample.millis),
            Ok(_) => {
                w1_failure(
                    shape,
                    "edit_cells",
                    "handle returned a non-published response",
                );
                break;
            }
            Err(error) => {
                w1_failure(shape, "edit_cells", error);
                break;
            }
        }
    }
    w1_line(shape, "edit_cells_n", millis.len());
    w1_duration_summary(shape, "edit_cells", &millis);
}

/// Row maintenance: `InsertRow` at the end with no initializers, then
/// `AppendRow` once (a different path; it refuses on a non-tracker table).
fn w1_row_maintenance(shape: &str, runtime: &mut DesignerRuntime) {
    const MAX_INSERTS: usize = 50;
    const INSERT_TIME_LIMIT_MS: f64 = 180_000.0;
    let collection = runtime.default_collection.clone();
    let started = Instant::now();
    let mut millis = Vec::new();
    let mut stopped = false;
    for iteration in 0..MAX_INSERTS {
        if iteration >= 5 && started.elapsed().as_secs_f64() * 1_000.0 > INSERT_TIME_LIMIT_MS {
            stopped = true;
            break;
        }
        let revision = runtime.observe_occurrence().revision;
        let (response, sample) = observe(|| {
            runtime.handle(DesignerRequest::InsertRow {
                expected_revision: revision,
                collection: collection.clone(),
                initializers: Vec::new(),
            })
        });
        match response {
            Ok(DesignerResponse::Published(_)) => millis.push(sample.millis),
            Ok(_) => {
                w1_failure(
                    shape,
                    "insert_row",
                    "handle returned a non-published response",
                );
                break;
            }
            Err(error) => {
                w1_failure(shape, "insert_row", error);
                break;
            }
        }
    }
    w1_line(shape, "insert_row_n", millis.len());
    if stopped {
        w1_line(
            shape,
            "insert_row_stopped",
            format!("limit {INSERT_TIME_LIMIT_MS:.0} ms"),
        );
    }
    w1_duration_summary(shape, "insert_row", &millis);

    let revision = runtime.observe_occurrence().revision;
    let (response, sample) = observe(|| {
        runtime.handle(DesignerRequest::AppendRow {
            expected_revision: revision,
            collection: collection.clone(),
        })
    });
    match response {
        Ok(DesignerResponse::Published(_)) => {
            w1_line(shape, "append_row_n", 1);
            w1_duration_summary(shape, "append_row", &[sample.millis]);
        }
        Ok(_) => w1_failure(
            shape,
            "append_row",
            "handle returned a non-published response",
        ),
        Err(error) => w1_failure(shape, "append_row", error),
    }
}

/// Open a fresh fixture for the copy measurement.
fn w1_fresh_runtime(shape: &str, serial: u64) -> Option<DesignerRuntime> {
    let (document, _rows, _columns) = w1_copy_document(shape)?;
    let occurrence = format!("00000000-0000-4000-8000-{serial:012x}");
    match DesignerRuntime::from_document_profile_with_opened(document, &occurrence, SessionProfile::Ordinary) {
        Ok((runtime, _opened)) => Some(runtime),
        Err(error) => {
            w1_failure(shape, "copy_open", error);
            None
        }
    }
}

/// One request's phases, summed per label, largest first.
fn w1_print_phases(label: &str, whole: f64) {
    let mut phases = phase_millis(&instrumentation::take_phases())
        .into_iter()
        .collect::<Vec<_>>();
    phases.sort_by(|left, right| right.1.total_cmp(&left.1));
    for (name, millis) in phases.into_iter().take(24) {
        println!("| {label} `{name}` | {millis:.1} | {:.1}% |", millis / whole * 100.0);
    }
}

/// `CopyFormula` and `EditCells` at growing sizes, each on a fresh runtime, one
/// sample each. A rung over a minute ends that ladder.
fn w1_copy_ladder(shape: &str) {
    const STOP_AFTER_MS: f64 = 60_000.0;
    let field = if shape == "d" { "c29" } else { "c9" };
    for (index, count) in [100_usize, 500, 1_000, 2_000, 5_000, 9_999].into_iter().enumerate() {
        let Some(mut runtime) = w1_fresh_runtime(shape, 0x400 + index as u64) else {
            return;
        };
        if shape == "a" {
            let revision = runtime.observe_occurrence().revision;
            if let Err(error) = runtime.handle(DesignerRequest::FormulaUpdate {
                expected_revision: revision,
                target: FieldTarget {
                    entity: entity_id(0).to_string(),
                    field: field.to_owned(),
                },
                source: "[.c0] * [.c3]".to_owned(),
            }) {
                w1_failure(shape, "copy_seed_formula", error);
                return;
            }
        }
        let destinations = (1..=count)
            .map(|row| FieldTarget {
                entity: entity_id(row).to_string(),
                field: field.to_owned(),
            })
            .collect::<Vec<_>>();
        let revision = runtime.observe_occurrence().revision;
        instrumentation::reset_phases();
        let (response, sample) = observe(|| {
            runtime.handle(DesignerRequest::CopyFormula {
                expected_revision: revision,
                source: FieldTarget {
                    entity: entity_id(0).to_string(),
                    field: field.to_owned(),
                },
                destinations,
                fixed_references: Vec::new(),
                relative_rows: true,
                relative_columns: false,
            })
        });
        w1_print_phases("copy_formula", sample.millis);
        match response {
            Ok(DesignerResponse::Published(publication)) => {
                w1_line(shape, &format!("copy_formula_{count}_ms"), format!("{:.3}", sample.millis));
                w1_line(
                    shape,
                    &format!("copy_formula_{count}_json_bytes"),
                    serde_json::to_vec(&publication).map_or(0, |bytes| bytes.len()),
                );
            }
            Ok(_) => {
                w1_failure(shape, "copy_formula", "non-published response");
                break;
            }
            Err(error) => {
                w1_failure(shape, "copy_formula", error);
                break;
            }
        }
        if sample.millis > STOP_AFTER_MS {
            w1_line(shape, "copy_formula_ladder_stopped", count);
            break;
        }
    }
    for (index, count) in [100_usize, 1_000, 5_000, 10_000].into_iter().enumerate() {
        let Some(mut runtime) = w1_fresh_runtime(shape, 0x500 + index as u64) else {
            return;
        };
        w1_line(shape, "edit_cells_count", count);
        let started = Instant::now();
        w1_edit_cells(shape, &mut runtime, count, 1, STOP_AFTER_MS);
        if started.elapsed().as_secs_f64() * 1_000.0 > STOP_AFTER_MS {
            w1_line(shape, "edit_cells_ladder_stopped", count);
            break;
        }
    }
}

/// Query one window at the runtime's current revision.
fn w1_query_view(
    runtime: &mut DesignerRuntime,
    collection: &str,
    view: &TableView,
    start: usize,
    count: usize,
) -> (Result<DesignerResponse, crate::DesignerError>, Sample) {
    let revision = runtime.observe_occurrence().revision;
    observe(|| {
        runtime.handle(DesignerRequest::QueryTableWindow {
            expected_revision: revision,
            collection: collection.to_owned(),
            view: view.clone(),
            start,
            count,
                display: Default::default(),
        })
    })
}

/// View kinds measured at size (Sheet S2). `Plain` sorts and filters nothing
/// and serves as the baseline.
#[derive(Clone, Copy)]
enum W1ViewKind {
    Plain,
    SortNumber,
    SortText,
    SortFormula,
    FilterNumber,
    FilterText,
    SortFilter,
}

impl W1ViewKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::SortNumber => "sort-number",
            Self::SortText => "sort-text",
            Self::SortFormula => "sort-formula",
            Self::FilterNumber => "filter-number",
            Self::FilterText => "filter-text",
            Self::SortFilter => "sort-filter",
        }
    }

    fn view(self) -> TableView {
        let sort_asc = |field: &str| {
            vec![ViewSortKey {
                text_order: Default::default(),
                field: field.to_owned(),
                direction: SortDirection::Asc,
            }]
        };
        let number_over_500 = ViewFilter::NumberCompare {
            field: "c0".to_owned(),
            op: ComparisonOperator::Gt,
            value: 500.0,
        };
        match self {
            Self::Plain => TableView::default(),
            Self::SortNumber => TableView {
                sort: sort_asc("c0"),
                ..TableView::default()
            },
            Self::SortText => TableView {
                sort: sort_asc("c1"),
                ..TableView::default()
            },
            Self::SortFormula => TableView {
                sort: sort_asc("c9"),
                ..TableView::default()
            },
            Self::FilterNumber => TableView {
                filters: vec![number_over_500],
                ..TableView::default()
            },
            Self::FilterText => TableView {
                filters: vec![ViewFilter::TextContains {
                    field: "c1".to_owned(),
                    value: "item-1".to_owned(),
                }],
                ..TableView::default()
            },
            Self::SortFilter => TableView {
                sort: sort_asc("c1"),
                filters: vec![number_over_500],
                ..TableView::default()
            },
        }
    }
}

/// Sorted and filtered windows at size: per view and round one `c0` edit,
/// then the query cold (new revision, first read of that view), warm (same
/// revision again) and deep (start 9000, or the last 100 rows). The edit is
/// timed separately and never joins a query figure; round 0 is dropped.
fn w1_views(shape: &str, runtime: &mut DesignerRuntime, rows: usize) {
    const MIN_ROUNDS: usize = 10;
    const MAX_ROUNDS: usize = 30;
    const VIEW_TIME_LIMIT_MS: f64 = 120_000.0;
    let collection = runtime.default_collection.clone();
    let mut kinds = vec![
        W1ViewKind::Plain,
        W1ViewKind::SortNumber,
        W1ViewKind::SortText,
    ];
    if shape == "b3" {
        kinds.push(W1ViewKind::SortFormula);
    }
    kinds.extend([
        W1ViewKind::FilterNumber,
        W1ViewKind::FilterText,
        W1ViewKind::SortFilter,
    ]);
    for kind in kinds {
        let name = kind.name();
        let view = kind.view();
        let started = Instant::now();
        let mut edits = Vec::new();
        let mut cold = Vec::new();
        let mut warm = Vec::new();
        let mut deep = Vec::new();
        let mut total_rows = 0;
        let mut json_bytes = 0;
        let mut first_cold_peak = None;
        let mut stopped = false;
        let mut failure: Option<String> = None;
        for iteration in 0..MAX_ROUNDS {
            if iteration >= MIN_ROUNDS
                && started.elapsed().as_secs_f64() * 1_000.0 > VIEW_TIME_LIMIT_MS
            {
                stopped = true;
                break;
            }
            let row = (iteration * 7919) % rows;
            let revision = runtime.observe_occurrence().revision;
            #[allow(clippy::cast_precision_loss)]
            let input = ScalarEditInput::Number {
                input: format!("{}", w1_next_value()),
            };
            let edit_started = Instant::now();
            let response = runtime.handle(DesignerRequest::EditScalar {
                expected_revision: revision,
                target: FieldTarget {
                    entity: entity_id(row).to_string(),
                    field: "c0".to_owned(),
                },
                input,
            });
            let edit_millis = edit_started.elapsed().as_secs_f64() * 1_000.0;
            match response {
                Ok(DesignerResponse::Published(_)) => edits.push(edit_millis),
                Ok(_) => {
                    failure = Some("handle returned a non-published response".to_owned());
                    break;
                }
                Err(error) => {
                    failure = Some(format!("{error}"));
                    break;
                }
            }
            let (response, sample) = w1_query_view(runtime, &collection, &view, 0, 100);
            match response {
                Ok(DesignerResponse::TableWindow(window)) => {
                    total_rows = window.total_rows;
                    json_bytes = serde_json::to_vec(&window).map_or(0, |bytes| bytes.len());
                    if iteration == 0 {
                        first_cold_peak = sample.peak_delta_bytes;
                    }
                    if iteration > 0 {
                        cold.push(sample.millis);
                    }
                }
                Ok(_) => {
                    failure = Some("cold query returned a non-window response".to_owned());
                    break;
                }
                Err(error) => {
                    failure = Some(format!("cold: {error}"));
                    break;
                }
            }
            let (response, sample) = w1_query_view(runtime, &collection, &view, 0, 100);
            match response {
                Ok(DesignerResponse::TableWindow(_)) => {
                    if iteration > 0 {
                        warm.push(sample.millis);
                    }
                }
                Ok(_) => {
                    failure = Some("warm query returned a non-window response".to_owned());
                    break;
                }
                Err(error) => {
                    failure = Some(format!("warm: {error}"));
                    break;
                }
            }
            let deep_start = if total_rows > 9_100 {
                9_000
            } else {
                total_rows.saturating_sub(100)
            };
            let (response, sample) = w1_query_view(runtime, &collection, &view, deep_start, 100);
            match response {
                Ok(DesignerResponse::TableWindow(_)) => {
                    if iteration > 0 {
                        deep.push(sample.millis);
                    }
                }
                Ok(_) => {
                    failure = Some("deep query returned a non-window response".to_owned());
                    break;
                }
                Err(error) => {
                    failure = Some(format!("deep: {error}"));
                    break;
                }
            }
        }
        if let Some(message) = failure {
            w1_failure(shape, &format!("view-{name}"), message);
            continue;
        }
        if stopped {
            w1_line(
                shape,
                &format!("view-{name}-stopped"),
                format!("limit {VIEW_TIME_LIMIT_MS:.0} ms"),
            );
        }
        w1_line(
            shape,
            &format!("view-{name}-peak_delta_bytes"),
            w1_optional(first_cold_peak),
        );
        if cold.is_empty() {
            continue;
        }
        w1_line(
            shape,
            &format!("view-{name}-edit_median_ms"),
            format!("{:.3}", median(edits)),
        );
        w1_line(
            shape,
            &format!("view-{name}"),
            format!(
                "cold={:.3},cold_p95={:.3},warm={:.3},deep={:.3},n={},total_rows={total_rows},json_bytes={json_bytes}",
                median(cold.clone()),
                w1_percentile(&cold, 95.0),
                median(warm),
                median(deep),
                cold.len()
            ),
        );
    }
}

fn w1_rows(shape: &str) -> usize {
    shape
        .strip_prefix("c1-")
        .or_else(|| shape.strip_prefix("c2-"))
        .or_else(|| shape.strip_prefix("g-"))
        .and_then(|rows| rows.parse().ok())
        .unwrap_or(10_000)
}

/// Export the live runtime both ways, then reopen the v3 bytes.
fn w1_save_and_reopen(shape: &str, runtime: &DesignerRuntime) {
    let revision = runtime.observe_occurrence().revision;
    let mut v3_bytes = None;
    let (exported, sample) = observe(|| runtime.export_project_v3(&revision));
    match exported {
        Ok(export) => {
            w1_line(shape, "export_v3_ms", format!("{:.3}", sample.millis));
            w1_line(shape, "export_v3_bytes", export.bytes.len());
            v3_bytes = Some(export.bytes);
        }
        Err(error) => w1_failure(shape, "export_v3", error),
    }
    let (exported, sample) = observe(|| runtime.export_project(&revision));
    match exported {
        Ok(export) => {
            w1_line(shape, "export_legacy_ms", format!("{:.3}", sample.millis));
            w1_line(shape, "export_legacy_bytes", export.bytes.len());
        }
        Err(error) => w1_failure(shape, "export_legacy", error),
    }
    if let Some(bytes) = v3_bytes {
        let mut reopened: Option<DesignerRuntime> = None;
        let occurrence = format!("00000000-0000-4000-8000-{:012x}", 0x500_u64);
        let (result, sample) = observe(|| open_project(&mut reopened, &bytes, &occurrence));
        match result {
            Ok(_) => {
                w1_line(shape, "reopen_ms", format!("{:.3}", sample.millis));
                w1_line(
                    shape,
                    "reopen_peak_delta_bytes",
                    w1_optional(sample.peak_delta_bytes),
                );
                w1_line(
                    shape,
                    "reopen_vmrss_bytes",
                    w1_optional(memory_stats::rss_bytes()),
                );
                w1_line(
                    shape,
                    "reopen_vmhwm_bytes",
                    w1_optional(memory_stats::peak_bytes()),
                );
            }
            Err(error) => w1_failure(shape, "reopen", error),
        }
    }
}

/// Eight minutes of scalar edits on the phase-5 runtime, sampling VmRSS every
/// 200 edits and reporting the second-half slope in bytes per edit.
fn w1_long_run(shape: &str, runtime: &mut DesignerRuntime, rows: usize) {
    const LIMIT_MS: f64 = 480_000.0;
    let started = Instant::now();
    let mut readings: Vec<(u64, Option<u64>)> = Vec::new();
    let mut edits: u64 = 0;
    loop {
        if started.elapsed().as_secs_f64() * 1_000.0 > LIMIT_MS {
            w1_line(shape, "long_run_stopped", format!("limit {LIMIT_MS:.0} ms"));
            break;
        }
        let index = usize::try_from(edits).unwrap_or(usize::MAX);
        let row = index.saturating_mul(7919) % rows;
        let revision = runtime.observe_occurrence().revision;
        #[allow(clippy::cast_precision_loss)]
        let input = ScalarEditInput::Number {
            input: format!("{}", w1_next_value()),
        };
        let response = runtime.handle(DesignerRequest::EditScalar {
            expected_revision: revision,
            target: FieldTarget {
                entity: entity_id(row).to_string(),
                field: "c0".to_owned(),
            },
            input,
        });
        match response {
            Ok(DesignerResponse::Published(_)) => {}
            Ok(_) => {
                w1_failure(
                    shape,
                    "long_run",
                    "handle returned a non-published response",
                );
                break;
            }
            Err(error) => {
                w1_failure(shape, "long_run", error);
                break;
            }
        }
        edits += 1;
        if edits % 200 == 0 {
            let rss = memory_stats::rss_bytes();
            w1_line(
                shape,
                &format!("long_run_rss_at_{edits}_bytes"),
                w1_optional(rss),
            );
            readings.push((edits, rss));
        }
    }
    w1_line(shape, "long_run_edits", edits);
    if readings.len() >= 2 {
        let half = readings.len() / 2;
        let first = readings[half];
        let last = readings[readings.len() - 1];
        if let (Some(before), Some(after)) = (first.1, last.1) {
            let edit_span = last.0.saturating_sub(first.0);
            if edit_span > 0 {
                #[allow(clippy::cast_precision_loss)]
                let slope = (after as f64 - before as f64) / edit_span as f64;
                w1_line(
                    shape,
                    "long_run_slope_bytes_per_edit",
                    format!("{slope:.2}"),
                );
            }
        }
    }
    w1_line(shape, "long_run_undo_depth", runtime.undo.len());
    w1_line(
        shape,
        "long_run_execution_receipts",
        runtime.lifecycle.execution_receipts().len(),
    );
    w1_line(
        shape,
        "long_run_execution_receipts_v2",
        runtime.lifecycle.execution_receipts_v2().len(),
    );
    w1_line(
        shape,
        "long_run_receipts_debug_bytes",
        format!("{:?}", runtime.lifecycle.execution_receipts()).len(),
    );
}

/// Plan W1: one working-size shape per process.
#[test]
#[ignore = "W1 working-size measurement; run one shape per process with W1_SHAPE"]
#[allow(clippy::too_many_lines)] // One linear measurement script reads best in one place.
fn capacity_w1_shape() {
    let shape = std::env::var("W1_SHAPE").expect("W1_SHAPE must name one shape");
    // Every timed edit skips the request-time admission preflight; phase 5a
    // turns it back on for one comparison.
    instrumentation::set_skip_preflight(true);
    let views_only = std::env::var("W1_VIEW").is_ok_and(|value| value == "1");
    if let Some(mib) = w1_mem_total_mib() {
        w1_line(&shape, "mem_total_mib", mib);
    }
    let rss_before_build = memory_stats::rss_bytes();
    let Some((document, rows, columns)) = w1_document(&shape) else {
        w1_line(&shape, "unknown_shape", 1);
        return;
    };
    let rss_after_build = memory_stats::rss_bytes();
    w1_line(&shape, "rows", rows);
    w1_line(&shape, "columns", columns);
    w1_line(
        &shape,
        "document_rss_before_bytes",
        w1_optional(rss_before_build),
    );
    w1_line(
        &shape,
        "document_rss_after_bytes",
        w1_optional(rss_after_build),
    );
    w1_line(
        &shape,
        "document_rss_delta_bytes",
        w1_optional(
            rss_before_build
                .zip(rss_after_build)
                .map(|(before, after)| after.saturating_sub(before)),
        ),
    );

    // 1. Document accounting and one clone.
    let (usage, usage_sample) = observe(|| crate::CapacityUsage::measure(&document));
    w1_line(
        &shape,
        "usage_measure_ms",
        format!("{:.3}", usage_sample.millis),
    );
    w1_line(&shape, "usage_formulas", usage.formulas);
    w1_line(&shape, "usage_formula_ast_nodes", usage.formula_ast_nodes);
    w1_line(&shape, "usage_dependency_edges", usage.dependency_edges);
    w1_line(&shape, "usage_populated_cells", usage.populated_cells);
    let mut clone_millis = Vec::new();
    for _ in 0..5 {
        let (_, sample) = observe(|| std::hint::black_box(document.clone()));
        clone_millis.push(sample.millis);
    }
    w1_line(
        &shape,
        "document_clone_median_ms",
        format!("{:.3}", median(clone_millis)),
    );

    // 2. Retained calculation state.
    let ((retained, incremental_work), retained_sample) =
        observe(|| RetainedCalculationState::rebuild(&document));
    w1_line(
        &shape,
        "retained_rebuild_ms",
        format!("{:.3}", retained_sample.millis),
    );
    w1_line(
        &shape,
        "retained_peak_delta_bytes",
        w1_optional(retained_sample.peak_delta_bytes),
    );
    w1_line(
        &shape,
        "retained_rebuild_full_rebuilds",
        incremental_work.full_rebuilds,
    );
    w1_line(
        &shape,
        "retained_rebuild_nodes_recomputed",
        incremental_work.nodes_recomputed,
    );
    w1_line(
        &shape,
        "retained_rebuild_nodes_reused",
        incremental_work.nodes_reused,
    );
    let sizes = retained.sizes();
    w1_line(&shape, "retained_nodes", sizes.nodes);
    w1_line(&shape, "retained_values", sizes.values);
    w1_line(&shape, "retained_dependency_keys", sizes.dependency_keys);
    w1_line(&shape, "retained_dependency_edges", sizes.dependency_edges);
    w1_line(&shape, "retained_reverse_keys", sizes.reverse_keys);
    w1_line(&shape, "retained_reverse_edges", sizes.reverse_edges);
    drop(retained);

    // 3. Open and the first window.
    let occurrence = format!("00000000-0000-4000-8000-{:012x}", 0x300_u64);
    let open_document = document.clone();
    let (opened, open_sample) = observe(|| {
        DesignerRuntime::from_document_profile_with_opened(open_document, &occurrence, SessionProfile::Ordinary)
    });
    let mut runtime = match opened {
        Ok((runtime, opened_projection)) => {
            w1_line(&shape, "open_ms", format!("{:.3}", open_sample.millis));
            w1_line(
                &shape,
                "open_projection_json_bytes",
                serde_json::to_vec(&opened_projection).map_or(0, |bytes| bytes.len()),
            );
            w1_line(
                &shape,
                "open_vmhwm_bytes",
                w1_optional(memory_stats::peak_bytes()),
            );
            w1_line(
                &shape,
                "open_vmrss_bytes",
                w1_optional(memory_stats::rss_bytes()),
            );
            Some(runtime)
        }
        Err(error) => {
            w1_failure(&shape, "open", error);
            None
        }
    };
    if let Some(runtime) = &mut runtime {
        let revision = runtime.observe_occurrence().revision;
        let collection = runtime.default_collection.clone();
        let (response, sample) = observe(|| {
            runtime.handle(DesignerRequest::QueryTableWindow {
                expected_revision: revision,
                collection,
                view: TableView::default(),
                start: 0,
                count: 100,
                display: Default::default(),
            })
        });
        match response {
            Ok(DesignerResponse::TableWindow(window)) => {
                w1_line(&shape, "window_100_ms", format!("{:.3}", sample.millis));
                w1_line(
                    &shape,
                    "window_100_json_bytes",
                    serde_json::to_vec(&window).map_or(0, |bytes| bytes.len()),
                );
            }
            Ok(_) => w1_failure(
                &shape,
                "window_100",
                "handle returned a non-window response",
            ),
            Err(error) => w1_failure(&shape, "window_100", error),
        }
    }

    // 4. One full calculate.
    let (calculated, calculate_sample) = observe(|| calculate(&document));
    match calculated {
        Ok(_) => w1_line(
            &shape,
            "calculate_ms",
            format!("{:.3}", calculate_sample.millis),
        ),
        Err(error) => w1_failure(&shape, "calculate", error),
    }
    drop(document);

    match std::env::var("W1_MODE").as_deref() {
        Ok("copy") => {
            drop(runtime);
            w1_copy_ladder(&shape);
            return;
        }
        Ok("long") => {
            // First thing after open, in its own process. Freed pages from the
            // open still absorb early growth; see the receipts line.
            if let Some(runtime) = &mut runtime {
                w1_long_run(&shape, runtime, rows);
            }
            return;
        }
        _ => {}
    }

    // 5. Scalar edits.
    if let Some(runtime) = &mut runtime {
        if views_only {
            // 10. Sorted and filtered windows at size; phases 5 to 9 are out.
            w1_views(&shape, runtime, rows);
        } else {
            match shape.as_str() {
                "a" => {
                    w1_scalar_edits(
                        &shape,
                        "edit",
                        runtime,
                        rows,
                        "c0",
                        false,
                        W1Input::Number,
                        300,
                        300_000.0,
                    );
                }
                "a100" => {
                    w1_scalar_edits(
                        &shape,
                        "edit",
                        runtime,
                        rows,
                        RATE_FIELD,
                        true,
                        W1Input::Number,
                        300,
                        300_000.0,
                    );
                }
                "b1" | "b3" | "d" | "f6" | "f10" => {
                    w1_scalar_edits(
                        &shape,
                        "edit",
                        runtime,
                        rows,
                        "c0",
                        false,
                        W1Input::Number,
                        300,
                        300_000.0,
                    );
                }
                shape if shape.starts_with("c1-") || shape.starts_with("c2-") || shape.starts_with("g-") => {
                    w1_scalar_edits(
                        shape,
                        "edit_unrelated",
                        runtime,
                        rows,
                        "c1",
                        false,
                        W1Input::Text,
                        if w1_lite() { 4 } else { 300 },
                        if w1_lite() { 1.0 } else { 180_000.0 },
                    );
                    w1_scalar_edits(
                        shape,
                        "edit_column",
                        runtime,
                        rows,
                        "c0",
                        false,
                        W1Input::Number,
                        if w1_lite() { 4 } else { 300 },
                        if w1_lite() { 1.0 } else { 180_000.0 },
                    );
                }
                _ => {}
            }
            if w1_lite() {
                return;
            }
            // 5a. Preflight ON, for the old table's comparison shapes.
            if shape == "a" || shape == "a100" {
                instrumentation::set_skip_preflight(false);
                let field = if shape == "a100" { RATE_FIELD } else { "c0" };
                w1_scalar_edits(
                    &shape,
                    "edit_preflight_on",
                    runtime,
                    rows,
                    field,
                    shape == "a100",
                    W1Input::Number,
                    20,
                    300_000.0,
                );
                instrumentation::set_skip_preflight(true);
            }

            // 6. Row maintenance.
            w1_row_maintenance(&shape, runtime);

            // 8. Save and reopen.
            w1_save_and_reopen(&shape, runtime);

        }
    }

}
