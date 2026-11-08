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
use std::io::Write;
use std::time::Instant;

use tachiko_storage::{decode_roproj_v3, encode_roproj_v3};
use tachiko_workspace_engine::{
    Document, Expression, FieldConstraint, FieldId, Number, Value,
    instrumentation::{self, CapacityCounters},
    patch_lifecycle::{DocumentScopeId, SemanticPublicationAuthority, TrustedInstant},
    resident_session::{ResidentWorkspaceSession, TrustedPublicationTimeSource},
    validate,
};

use super::{
    CellEdit, DESIGNER_PRINCIPAL, DesignerClock, DesignerRequest, DesignerResponse,
    DesignerRuntime, Entity, EntityId, EntityKey, FieldDefinition, FieldKey, FieldRef, FieldTarget,
    FieldType, PrincipalId, RowInitializer, ScalarEditInput, Schema, SchemaId, SchemaKey,
    TableView, ViewIndexCache, collection_specs, decode_project_bundle, designer_lifecycle,
    document_requires_v2_evidence, document_scope, encode_project_bundle,
    ensure_projection_size_with_limit, formula_sources, open_portable_ro_working_size,
    open_project_working_size, process_wire_request, select_default_collection,
};
use crate::capacity::SessionProfile;
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
        profile: SessionProfile::Ordinary,
        capacity_usage,
        working_admission: None,
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
                .publish_if_current(&scope, &revision, &candidate, |_| Some(()))
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
                .publish_if_current(&scope, &revision, &candidate, |_| Some(()))
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
/// Every field carries a display name, so every shape takes the v2 evidence
/// path.
#[derive(Clone, Copy, Debug)]
enum WqShape {
    /// Ten populated Number columns (100,000 cells) and twenty empty ones.
    Values,
    /// Nine populated Number columns and one same-row formula column
    /// (10,000 formulas, 100,000 cells).
    Formulas,
    /// Nine populated Number columns and one formula column; every row's `c9`
    /// reads `r00000.c1`, so one edit reaches all 10,000 rows.
    Fanout,
    /// The first 101 rows' `c9` reads `r00000.c1`; the rest read their own
    /// `c1`. One edit reaches 101 dependents.
    Fanout101,
}

impl WqShape {
    const ALL: [Self; 4] = [Self::Values, Self::Formulas, Self::Fanout, Self::Fanout101];

    const fn name(self) -> &'static str {
        match self {
            Self::Values => "values",
            Self::Formulas => "formulas",
            Self::Fanout => "fanout",
            Self::Fanout101 => "fanout101",
        }
    }

    const fn populated_columns(self) -> usize {
        match self {
            Self::Values => 10,
            Self::Formulas | Self::Fanout | Self::Fanout101 => 9,
        }
    }
}

const WQ_OCCURRENCE: &str = "00000000-0000-4000-8000-000000000678";
const WQ_ROWS: usize = 10_000;
const WQ_COLUMNS: usize = 30;
const WQ_EDIT_COUNT: usize = 20;
/// How many rows of the `fanout101` shape read `r00000.c1`.
const WQ_FANOUT_DEPENDENTS: usize = 101;

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
        match shape {
            WqShape::Values => {}
            WqShape::Formulas => {
                fields.insert(
                    FieldId::from("c9"),
                    Value::Formula(Expression::Add {
                        left: Box::new(Expression::SameRowReference(FieldId::from("c0"))),
                        right: Box::new(Expression::SameRowReference(FieldId::from("c1"))),
                    }),
                );
            }
            WqShape::Fanout | WqShape::Fanout101 => {
                let dependents = if matches!(shape, WqShape::Fanout101) {
                    WQ_FANOUT_DEPENDENTS
                } else {
                    WQ_ROWS
                };
                let right = if row < dependents {
                    Expression::Reference(FieldRef::new("r00000", "c1"))
                } else {
                    Expression::SameRowReference(FieldId::from("c1"))
                };
                fields.insert(
                    FieldId::from("c9"),
                    Value::Formula(Expression::Add {
                        left: Box::new(Expression::SameRowReference(FieldId::from("c0"))),
                        right: Box::new(right),
                    }),
                );
            }
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

    // The fan-out shapes measure one edit and the re-query it forces, so they
    // skip the save, reopen and structural steps their shapes don't need.
    if matches!(shape, WqShape::Fanout | WqShape::Fanout101) {
        let mut runtime = match DesignerRuntime::from_document_working_size(document, WQ_OCCURRENCE)
        {
            Ok(runtime) => runtime,
            Err(error) => {
                wq_refused(name, "open_document", &error);
                return;
            }
        };
        qualify_working_size_fanout(shape, &mut runtime);
        return;
    }

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
        let (result, sample) = observe(|| runtime.handle(request));
        match result {
            Ok(response) => {
                handle_samples.push(sample);
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

/// Print the median and p95 of one latency group.
fn wq_print_latency(name: &str, prefix: &str, samples: &[Sample]) {
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
    wq_print(name, &format!("{prefix}_median_ms"), format!("{median:.3}"));
    wq_print(
        name,
        &format!("{prefix}_p95_ms"),
        format!("{:.3}", ordered[p95_index]),
    );
}

/// One edit of `r00000.c1` through `edit_cells` and the host's re-query, the
/// window and the column totals a `many` reply forces, measured
/// `WQ_EDIT_COUNT` times.
fn qualify_working_size_fanout(shape: WqShape, runtime: &mut DesignerRuntime) {
    let name = shape.name();
    let entities = (0..WQ_ROWS)
        .map(|row| format!("r{row:05}"))
        .collect::<Vec<_>>();
    let mut edit_samples = Vec::with_capacity(WQ_EDIT_COUNT);
    let mut requery_samples = Vec::with_capacity(WQ_EDIT_COUNT);
    let mut edit_requery_samples = Vec::with_capacity(WQ_EDIT_COUNT);
    let mut window_samples = Vec::with_capacity(WQ_EDIT_COUNT);
    let mut column_totals_samples = Vec::with_capacity(WQ_EDIT_COUNT);
    let mut view_totals_samples = Vec::with_capacity(WQ_EDIT_COUNT);
    let mut edit_view_requery_samples = Vec::with_capacity(WQ_EDIT_COUNT);
    let mut many = false;
    for row in 0..WQ_EDIT_COUNT {
        let revision = runtime.observe_occurrence().revision;
        let target = FieldTarget {
            entity: "r00000".to_owned(),
            field: "c1".to_owned(),
        };
        let input = ScalarEditInput::Number {
            input: format!("{}", 100 + row),
        };
        let (edited, edit_sample) =
            observe(|| runtime.edit_cells(&revision, &[CellEdit { target, input }]));
        let publication = match edited {
            Ok(publication) => publication,
            Err(error) => {
                wq_refused(name, "edit", &error);
                return;
            }
        };
        many = publication.many;

        // The re-query a host runs after a `many` reply: the first 100 rows of
        // every column, then the totals over every row of every column.
        let revision = runtime.observe_occurrence().revision;
        let window = DesignerRequest::QueryTableWindow {
            expected_revision: revision.clone(),
            collection: "grid".to_owned(),
            view: TableView::default(),
            start: 0,
            count: 100,
            display: true,
        };
        let totals = DesignerRequest::QueryColumnTotals {
            expected_revision: revision.clone(),
            collection: "grid".to_owned(),
            entities: entities.clone(),
        };
        // Sheet's own totals query lists the shown rows, which its 60 KB
        // request bound keeps well under 10,000, so a view's totals are
        // measured too, and each part on its own.
        let view_totals = DesignerRequest::QueryViewTotals {
            expected_revision: revision,
            collection: "grid".to_owned(),
            view: TableView::default(),
        };
        let (windowed, window_sample) = observe(|| runtime.handle(window.clone()));
        let (totalled, column_totals_sample) = observe(|| runtime.handle(totals.clone()));
        let (viewed, view_totals_sample) = observe(|| runtime.handle(view_totals));
        for (step, outcome) in [
            ("requery_window", windowed),
            ("requery_column_totals", totalled),
            ("requery_view_totals", viewed),
        ] {
            if let Err(error) = outcome {
                wq_refused(name, step, &error);
                return;
            }
        }
        let (requeried, requery_sample) = observe(|| {
            runtime.handle(window)?;
            runtime.handle(totals)?;
            Ok::<(), super::DesignerError>(())
        });
        if let Err(error) = requeried {
            wq_refused(name, "requery", &error);
            return;
        }
        edit_requery_samples.push(Sample {
            millis: edit_sample.millis + requery_sample.millis,
            ..edit_sample
        });
        edit_view_requery_samples.push(Sample {
            millis: edit_sample.millis + window_sample.millis + view_totals_sample.millis,
            ..edit_sample
        });
        edit_samples.push(edit_sample);
        requery_samples.push(requery_sample);
        window_samples.push(window_sample);
        column_totals_samples.push(column_totals_sample);
        view_totals_samples.push(view_totals_sample);
    }
    wq_print(name, "edit_n", edit_samples.len());
    wq_print(name, "edit_many", many);
    wq_print_latency(name, "edit", &edit_samples);
    wq_print_latency(name, "requery", &requery_samples);
    wq_print_latency(name, "edit_requery", &edit_requery_samples);
    wq_print_latency(name, "requery_window", &window_samples);
    wq_print_latency(name, "requery_column_totals", &column_totals_samples);
    wq_print_latency(name, "requery_view_totals", &view_totals_samples);
    wq_print_latency(name, "edit_view_requery", &edit_view_requery_samples);
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
