//! Acceptance tests for the working-size session profile (#678, stage 2).
#![allow(clippy::too_many_lines)] // Each test walks one whole acceptance case.

use std::collections::{BTreeMap, BTreeSet};

use tachiko_storage::{encode_roproj_v3, encode_roproj_v3_with_presentation, style_id};
use tachiko_workspace_engine::{
    CellStyle, ColumnAggregateFunction, ColumnCriterion, ColumnRef, ComparisonOperator,
    ConditionalAggregate, Date, Expression, FieldConstraint, FooterAggregate,
    KeyedGroupedSumDefinition, KeyedGroupedSumDefinitionId, KeyedGroupedSumOrdersBinding,
    KeyedGroupedSumProductsBinding, Number, NumberFormat, NumberFormatKind, PresentationStore,
    TablePresentation, Value,
};

use super::capacity::{
    CapacityExceeded, CapacityLimit, CapacityLimits, CapacityUsage, DEFAULT_LIMITS, SessionProfile,
    TableCapacityUsage, limits_for_profile,
};
use super::capacity_fixtures::{CALC_FIELD, FixtureSpec, capacity_document, entity_id};
use super::interop_document::CleanupOperation;
use super::{
    CalculationProjection, CellEdit, DesignerError, DesignerProfile, DesignerRequest,
    DesignerResponse, DesignerRuntime, DesignerWireReply, Document, Entity, EntityId, EntityKey,
    FailureProjection, FieldDefinition, FieldId, FieldKey, FieldProjection, FieldRef, FieldTarget,
    FieldType, NewTableColumnInput, OpenedWindowProjection, PasteCellInput, PublicationProjection,
    RowInitializer, RowProjection, ScalarEditInput, Schema, SchemaId, SchemaKey,
    StoredValueProjection, TableView, TableWindowProjection, encode_portable_package_v1,
    encode_project_bundle, open_local_document, open_local_document_working_size, open_portable_ro,
    open_portable_ro_working_size, open_project, open_project_working_size, process_wire_request,
    to_canonical_string,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000678";
const OTHER_OCCURRENCE: &str = "00000000-0000-4000-8000-000000000679";
const MAX_WORKING_STORED_TEXT_BYTES: usize = 32 * 1024 * 1024;
const STORED_TEXT_ITEM_MAX: usize = 64 * 1024;

/// The brief's fixture: 128 rows x 28 data columns plus three formula columns
/// (rate, calc, total), so one added column reaches the 32-field limit.
fn fixture() -> Document {
    capacity_document(FixtureSpec {
        rows: 128,
        columns: 28,
        formula_cells: 128,
        seed: 678,
    })
}

fn project_bundle(document: &Document) -> Vec<u8> {
    let tree = encode_roproj_v3(document).unwrap();
    encode_project_bundle(tree.files().iter().map(|file| (file.path(), file.bytes()))).unwrap()
}

fn portable_ro(document: &Document) -> Vec<u8> {
    let tree = encode_roproj_v3(document).unwrap();
    encode_portable_package_v1(&tree).unwrap()
}

fn raw_document(document: &Document) -> Vec<u8> {
    to_canonical_string(document).unwrap().into_bytes()
}

fn projected<'a>(row: &'a RowProjection, field: &str) -> &'a FieldProjection {
    row.fields
        .iter()
        .find(|projection| projection.target.field == field)
        .unwrap_or_else(|| panic!("row {} has no {field} field", row.id))
}

fn expect_error<T>(result: Result<T, DesignerError>) -> DesignerError {
    match result {
        Ok(_) => panic!("expected a refusal"),
        Err(error) => error,
    }
}

fn revision(runtime: &DesignerRuntime) -> String {
    runtime.current_revision().to_owned()
}

fn assert_capacity_refusal(
    error: &DesignerError,
    limit: CapacityLimit,
    maximum: usize,
    attempted: usize,
) {
    match error {
        DesignerError::CapacityExceeded(CapacityExceeded {
            limit: actual_limit,
            maximum: actual_maximum,
            attempted: actual_attempted,
            ..
        }) => {
            assert_eq!(*actual_limit, limit);
            assert_eq!(*actual_maximum, maximum);
            assert_eq!(*actual_attempted, attempted);
        }
        other => panic!("expected a capacity refusal, got {other:?}"),
    }
}

fn assert_tracker_message(error: &DesignerError, fragment: &str) {
    match error {
        DesignerError::InvalidTrackerOperation { message } => {
            assert!(
                message.contains(fragment),
                "expected {message:?} to contain {fragment:?}"
            );
        }
        other => panic!("expected a tracker refusal, got {other:?}"),
    }
}

fn assert_scalar_edit_message(error: &DesignerError, fragment: &str) {
    assert!(
        error.to_string().contains(fragment),
        "expected {error:?} to contain {fragment:?}"
    );
}

fn assert_unsupported_message(error: &DesignerError, fragment: &str) {
    match error {
        DesignerError::UnsupportedProject { message } => {
            assert!(
                message.contains(fragment),
                "expected {message:?} to contain {fragment:?}"
            );
        }
        other => panic!("expected an unsupported-project refusal, got {other:?}"),
    }
}

fn assert_ordinary_refusal(error: &DesignerError) {
    match error {
        DesignerError::ProjectionTooLarge { actual, maximum } => {
            assert_eq!(*maximum, super::MAX_PROJECTION_BYTES);
            assert!(*actual > *maximum);
        }
        DesignerError::UnsupportedProject { message } => {
            assert!(message.contains("65536"), "unexpected refusal: {message}");
        }
        other => panic!("expected today's open to refuse the fixture, got {other:?}"),
    }
}

fn assert_fixture_window(opened: &OpenedWindowProjection) {
    assert_eq!(opened.profile, DesignerProfile::WorkingSize);
    assert_eq!(opened.bootstrap.default_collection, "grid");
    assert_eq!(opened.window.total_rows, 128);
    assert_eq!(opened.window.unfiltered_rows, 128);
    assert_eq!(opened.window.start, 0);
    assert_eq!(opened.window.rows.len(), 100);
    for row in &opened.window.rows {
        let Some(StoredValueProjection::Number { value }) = &projected(row, "c0").stored else {
            panic!("row {} has no stored Number in c0", row.id);
        };
        let Some(CalculationProjection::Value { value: calculated }) =
            &projected(row, CALC_FIELD).calculated
        else {
            panic!("row {} has no calculated value in {CALC_FIELD}", row.id);
        };
        let expected = value * 2.0;
        assert!(
            (calculated - expected).abs() < 1e-9,
            "row {}: calc {calculated} != 2 x c0 {value}",
            row.id
        );
    }
}

fn assert_window_reply_shape(opened: &OpenedWindowProjection) {
    let json = serde_json::to_string(opened).unwrap();
    assert!(json.contains("\"profile\":\"working_size\""));
    assert!(!json.contains("\"table\":"));
}

fn text_grid(rows: usize, columns: usize) -> Document {
    let mut document = Document::empty("working-size-bench", "Working Size Fixture");
    let mut schema = Schema {
        field_order: None,
        id: SchemaId::from("grid"),
        key: SchemaKey::from("grid"),
        display_name: None,
        fields: BTreeMap::new(),
    };
    for column in 0..columns {
        let id = FieldId::from(format!("c{column}"));
        schema.fields.insert(
            id.clone(),
            FieldDefinition {
                id,
                key: FieldKey::from(format!("c{column}")),
                display_name: None,
                field_type: FieldType::Text,
                required: false,
                constraint: FieldConstraint::None,
                number_format: None,
            },
        );
    }
    document.schemas.insert(schema.id.clone(), schema);
    for row in 0..rows {
        let id = EntityId::from(format!("r{row:05}"));
        document.entities.insert(
            id.clone(),
            Entity {
                id: id.clone(),
                key: EntityKey::from(format!("k{row:05}")),
                schema: SchemaId::from("grid"),
                fields: BTreeMap::new(),
            },
        );
    }
    document
}

fn fill_text(document: &mut Document, row: usize, column: usize, len: usize) {
    document
        .entities
        .get_mut(&EntityId::from(format!("r{row:05}")))
        .unwrap()
        .fields
        .insert(
            FieldId::from(format!("c{column}")),
            Value::Text("x".repeat(len)),
        );
}

fn add_chain(terms: usize) -> Expression {
    let mut expression = Expression::Number(Number::new(1.0).unwrap());
    for _ in 1..terms {
        expression = Expression::Add {
            left: Box::new(expression),
            right: Box::new(Expression::Number(Number::new(1.0).unwrap())),
        };
    }
    expression
}

#[test]
fn working_size_open_carries_the_first_bounded_window() {
    let document = fixture();
    let bundle = project_bundle(&document);
    let raw = raw_document(&document);
    let portable = portable_ro(&document);

    // Today's opens prove the complete reply is over 64 KiB and refuse.
    let mut resident: Option<DesignerRuntime> = None;
    assert_ordinary_refusal(&open_project(&mut resident, &bundle, OCCURRENCE).unwrap_err());
    assert!(resident.is_none());
    assert_ordinary_refusal(&open_local_document(&mut resident, &raw, OCCURRENCE).unwrap_err());
    assert!(resident.is_none());
    assert_ordinary_refusal(&open_portable_ro(&mut resident, &portable, OCCURRENCE).unwrap_err());
    assert!(resident.is_none());

    let mut project_resident: Option<DesignerRuntime> = None;
    let opened = open_project_working_size(&mut project_resident, &bundle, OCCURRENCE).unwrap();
    assert_eq!(
        project_resident.as_ref().unwrap().profile,
        SessionProfile::WorkingSize
    );
    assert_fixture_window(&opened);
    assert_window_reply_shape(&opened);

    let mut local_resident: Option<DesignerRuntime> = None;
    let local = open_local_document_working_size(&mut local_resident, &raw, OCCURRENCE).unwrap();
    assert_eq!(
        local_resident.as_ref().unwrap().profile,
        SessionProfile::WorkingSize
    );
    assert_fixture_window(&local);

    let mut portable_resident: Option<DesignerRuntime> = None;
    let ro = open_portable_ro_working_size(&mut portable_resident, &portable, OCCURRENCE).unwrap();
    assert_eq!(
        portable_resident.as_ref().unwrap().profile,
        SessionProfile::WorkingSize
    );
    assert_fixture_window(&ro);
}

#[test]
fn failed_working_size_open_keeps_the_previous_occurrence() {
    let columns = [
        NewTableColumnInput {
            name: "amount".into(),
            field_type: "number".into(),
            required: false,
        },
        NewTableColumnInput {
            name: "note".into(),
            field_type: "text".into(),
            required: false,
        },
    ];
    let runtime =
        DesignerRuntime::new_working_size_table(OTHER_OCCURRENCE, "Resident", &columns).unwrap();
    let before = runtime.session.export_snapshot().into_document();
    let mut resident = Some(runtime);

    let too_large = text_grid(10_001, 1);
    let bundle = project_bundle(&too_large);
    let raw = raw_document(&too_large);
    let portable = portable_ro(&too_large);

    for error in [
        open_project_working_size(&mut resident, &bundle, OCCURRENCE).unwrap_err(),
        open_local_document_working_size(&mut resident, &raw, OCCURRENCE).unwrap_err(),
        open_portable_ro_working_size(&mut resident, &portable, OCCURRENCE).unwrap_err(),
    ] {
        assert_capacity_refusal(&error, CapacityLimit::TableRows, 10_000, 10_001);
    }

    let resident = resident.unwrap();
    assert_eq!(resident.current_revision(), "resident/0");
    assert_eq!(resident.session.export_snapshot().document(), &before);
    assert_eq!(resident.profile, SessionProfile::WorkingSize);
}

#[test]
fn a_ten_thousand_row_document_opens_at_working_size_only() {
    let raw = raw_document(&text_grid(10_000, 1));

    let mut resident: Option<DesignerRuntime> = None;
    let opened = open_local_document_working_size(&mut resident, &raw, OCCURRENCE).unwrap();
    assert_eq!(
        resident.as_ref().unwrap().profile,
        SessionProfile::WorkingSize
    );
    assert_eq!(opened.window.total_rows, 10_000);
    assert_eq!(opened.window.rows.len(), 100);

    // The same bytes still refuse today's ordinary open.
    let mut ordinary: Option<DesignerRuntime> = None;
    let error = open_local_document(&mut ordinary, &raw, OCCURRENCE).unwrap_err();
    assert_capacity_refusal(&error, CapacityLimit::TableRows, 128, 10_000);
    assert!(ordinary.is_none());

    let mut resident: Option<DesignerRuntime> = None;
    let error = open_local_document_working_size(
        &mut resident,
        &raw_document(&text_grid(10_001, 1)),
        OCCURRENCE,
    )
    .unwrap_err();
    assert_capacity_refusal(&error, CapacityLimit::TableRows, 10_000, 10_001);
    assert!(resident.is_none());
}

#[test]
fn working_size_admits_ten_thousand_rows_and_refuses_the_next() {
    let mut runtime =
        DesignerRuntime::from_document_working_size(text_grid(10_000, 1), OCCURRENCE).unwrap();
    assert_eq!(runtime.profile, SessionProfile::WorkingSize);
    let current = revision(&runtime);
    let error = runtime.insert_row(&current, "grid", &[]).unwrap_err();
    assert_capacity_refusal(&error, CapacityLimit::TableRows, 10_000, 10_001);
    assert_eq!(runtime.current_revision(), current);
    assert_eq!(runtime.profile, SessionProfile::WorkingSize);
}

#[test]
fn working_limits_admit_at_the_bound_and_refuse_one_above() {
    let limits = limits_for_profile(SessionProfile::WorkingSize);
    assert_eq!(limits.table_rows, 10_000);
    let at_limit = CapacityUsage {
        collections: limits.collections,
        tables: BTreeMap::from([(
            "grid".to_owned(),
            TableCapacityUsage {
                rows: limits.table_rows,
                fields: limits.table_fields,
            },
        )]),
        total_entities: limits.total_entities,
        populated_cells: limits.populated_cells,
        formulas: limits.formulas,
        formula_ast_nodes: limits.formula_ast_nodes,
        dependency_edges: limits.dependency_edges,
    };
    assert!(at_limit.check(limits).is_ok());

    let mut over = at_limit.clone();
    over.tables.get_mut("grid").unwrap().rows += 1;
    assert_capacity_refusal(
        &over.check(limits).unwrap_err(),
        CapacityLimit::TableRows,
        10_000,
        10_001,
    );

    let mut over = at_limit.clone();
    over.total_entities += 1;
    assert_capacity_refusal(
        &over.check(limits).unwrap_err(),
        CapacityLimit::TotalEntities,
        10_000,
        10_001,
    );

    let mut over = at_limit.clone();
    over.populated_cells += 1;
    assert_capacity_refusal(
        &over.check(limits).unwrap_err(),
        CapacityLimit::PopulatedCells,
        100_000,
        100_001,
    );

    let mut over = at_limit.clone();
    over.formulas += 1;
    assert_capacity_refusal(
        &over.check(limits).unwrap_err(),
        CapacityLimit::Formulas,
        100_000,
        100_001,
    );

    let mut over = at_limit.clone();
    over.formula_ast_nodes += 1;
    assert_capacity_refusal(
        &over.check(limits).unwrap_err(),
        CapacityLimit::FormulaAstNodes,
        1_000_000,
        1_000_001,
    );

    let mut over = at_limit.clone();
    over.dependency_edges += 1;
    assert_capacity_refusal(
        &over.check(limits).unwrap_err(),
        CapacityLimit::DependencyEdges,
        1_000_000,
        1_000_001,
    );
}

/// A working-size session reads a whole column once, so accounting counts one
/// edge per formula instead of one per row.
#[test]
fn working_size_counts_a_column_read_once() {
    let mut document = capacity_document(FixtureSpec {
        rows: 200,
        columns: 1,
        formula_cells: 0,
        seed: 21,
    });
    for row in 0..3 {
        document
            .entities
            .get_mut(&entity_id(row))
            .unwrap()
            .fields
            .insert(
                FieldId::from(format!("agg{row}")),
                Value::Formula(Expression::ColumnTotal(ColumnRef::new("grid", "c0"))),
            );
    }
    assert_eq!(
        CapacityUsage::measure_for(&document, SessionProfile::WorkingSize).dependency_edges,
        3
    );
    assert_eq!(CapacityUsage::measure(&document).dependency_edges, 200 * 3);

    document
        .entities
        .get_mut(&entity_id(0))
        .unwrap()
        .fields
        .insert(
            FieldId::from("same_row"),
            Value::Formula(Expression::SameRowReference(FieldId::from("c0"))),
        );
    assert_eq!(
        CapacityUsage::measure_for(&document, SessionProfile::WorkingSize).dependency_edges,
        4
    );
    assert_eq!(
        CapacityUsage::measure(&document).dependency_edges,
        200 * 3 + 1
    );
}

#[test]
fn ordinary_and_text_capacity_limits_are_unchanged() {
    assert_eq!(limits_for_profile(SessionProfile::Ordinary), DEFAULT_LIMITS);
    assert_eq!(
        limits_for_profile(SessionProfile::TextCapacity),
        CapacityLimits {
            table_rows: 8_406,
            total_entities: 8_406,
            ..DEFAULT_LIMITS
        }
    );
}

#[test]
fn working_size_native_row_removal_follows_the_working_row_limit() {
    let mut runtime =
        DesignerRuntime::from_document_working_size(text_grid(300, 1), OCCURRENCE).unwrap();
    let removed = (0..200).map(|row| format!("r{row:05}")).collect::<Vec<_>>();
    let current = revision(&runtime);
    runtime
        .remove_table_rows(&current, "grid", &removed)
        .unwrap();
    assert_eq!(
        runtime.session.export_snapshot().document().entities.len(),
        100
    );

    let current = revision(&runtime);
    runtime.history_edit(&current, false).unwrap();
    assert_eq!(
        runtime.session.export_snapshot().document().entities.len(),
        300
    );

    // A selection above the working row limit is refused before any lookup.
    let current = revision(&runtime);
    let rows = (0..=10_000)
        .map(|row| format!("x{row}"))
        .collect::<Vec<_>>();
    let error = runtime
        .remove_table_rows(&current, "grid", &rows)
        .unwrap_err();
    assert!(
        matches!(
            error,
            DesignerError::InvalidTableOperation { ref message }
                if message == "the row selection is empty or too large"
        ),
        "{error:?}"
    );
    assert_eq!(runtime.current_revision(), current);

    // Today's ordinary profile still refuses a 129-row selection.
    let mut ordinary = DesignerRuntime::from_document_profile(
        text_grid(128, 1),
        OCCURRENCE,
        SessionProfile::Ordinary,
    )
    .unwrap();
    let rows = (0..=128).map(|row| format!("x{row}")).collect::<Vec<_>>();
    let current = revision(&ordinary);
    let error = ordinary
        .remove_table_rows(&current, "grid", &rows)
        .unwrap_err();
    assert!(
        matches!(
            error,
            DesignerError::InvalidTableOperation { ref message }
                if message == "the row selection is empty or too large"
        ),
        "{error:?}"
    );
    assert_eq!(ordinary.current_revision(), current);
}

#[test]
fn working_size_capacity_query_reports_the_working_limits() {
    let mut runtime =
        DesignerRuntime::from_document_working_size(text_grid(2, 1), OCCURRENCE).unwrap();
    let response = runtime
        .handle(DesignerRequest::QueryCapacity {
            expected_revision: None,
        })
        .unwrap();
    let DesignerResponse::Capacity(projection) = response else {
        panic!("queryCapacity replies a capacity projection");
    };
    assert_eq!(
        projection.limits,
        limits_for_profile(SessionProfile::WorkingSize)
    );
    assert_eq!(projection.limits.table_rows, 10_000);
}

#[test]
fn working_size_never_promotes_and_accepts_what_the_encoding_refuses() {
    let mut document = capacity_document(FixtureSpec {
        rows: 65,
        columns: 2,
        formula_cells: 0,
        seed: 19,
    });
    document
        .schemas
        .get_mut(&SchemaId::from("grid"))
        .unwrap()
        .fields
        .insert(
            FieldId::from("result"),
            FieldDefinition {
                id: FieldId::from("result"),
                key: FieldKey::from("result"),
                display_name: None,
                field_type: FieldType::Number,
                required: false,
                constraint: FieldConstraint::None,
                number_format: None,
            },
        );
    let long_text = "x".repeat(4097);
    document
        .entities
        .get_mut(&entity_id(0))
        .unwrap()
        .fields
        .insert(FieldId::from("c1"), Value::Text(long_text));

    assert!(super::is_text_capacity_eligible_document(&document));
    let capacity_error = expect_error(DesignerRuntime::from_document_profile(
        document.clone(),
        OCCURRENCE,
        SessionProfile::TextCapacity,
    ));
    match capacity_error {
        DesignerError::InvalidTrackerOperation { message } => {
            assert!(message.contains("4096"), "unexpected refusal: {message}");
        }
        other => panic!("expected the encoding closure to refuse, got {other:?}"),
    }

    let mut runtime = DesignerRuntime::from_document_working_size(document, OCCURRENCE).unwrap();
    assert_eq!(runtime.profile, SessionProfile::WorkingSize);

    let current = revision(&runtime);
    runtime
        .edit_cells(
            &current,
            &[CellEdit {
                target: FieldTarget::from("r00000.c1"),
                input: ScalarEditInput::Text {
                    value: "y".repeat(4097),
                },
            }],
        )
        .unwrap();
    assert_eq!(runtime.profile, SessionProfile::WorkingSize);

    let current = revision(&runtime);
    runtime
        .edit_cells(
            &current,
            &[CellEdit {
                target: FieldTarget::from("r00000.result"),
                input: ScalarEditInput::Number { input: "3".into() },
            }],
        )
        .unwrap();
    let current = revision(&runtime);
    runtime
        .update_formula(&current, &FieldTarget::from("r00000.result"), "[.c0] * 2")
        .unwrap();
    assert_eq!(runtime.profile, SessionProfile::WorkingSize);

    let table = runtime.query_table("grid").unwrap();
    let row = table
        .rows
        .iter()
        .find(|row| row.id == "r00000")
        .expect("row 0 exists");
    match &projected(row, "c1").stored {
        Some(StoredValueProjection::Text { value }) => assert_eq!(value.len(), 4097),
        other => panic!("expected the accepted text, got {other:?}"),
    }
}

#[test]
fn every_command_succeeds_and_keeps_the_working_size_profile() {
    let mut runtime = DesignerRuntime::from_document_working_size(fixture(), OCCURRENCE).unwrap();
    assert_eq!(runtime.profile, SessionProfile::WorkingSize);

    let current = revision(&runtime);
    runtime
        .edit_cells(
            &current,
            &[CellEdit {
                target: FieldTarget::from("r00000.c1"),
                input: ScalarEditInput::Text {
                    value: "  spaced  ".into(),
                },
            }],
        )
        .unwrap();
    assert_eq!(runtime.profile, SessionProfile::WorkingSize);

    let current = revision(&runtime);
    runtime
        .add_column_with_requiredness(&current, "grid", "Extra", "text", false, &[])
        .unwrap();
    let current = revision(&runtime);
    runtime
        .change_column_type(&current, "grid", "c2", "text")
        .unwrap();
    let current = revision(&runtime);
    runtime
        .set_column_requiredness(&current, "grid", "c0", true)
        .unwrap();
    assert_eq!(runtime.profile, SessionProfile::WorkingSize);

    let current = revision(&runtime);
    runtime
        .update_formula(
            &current,
            &FieldTarget::from("r00000.total"),
            "[.rate] + [.c0]",
        )
        .unwrap();
    let current = revision(&runtime);
    let last_row = runtime
        .query_table("grid")
        .unwrap()
        .rows
        .last()
        .unwrap()
        .id
        .clone();
    runtime
        .remove_table_rows(&current, "grid", std::slice::from_ref(&last_row))
        .unwrap();
    let current = revision(&runtime);
    runtime
        .insert_row(
            &current,
            "grid",
            &[RowInitializer {
                field: "c0".into(),
                input: ScalarEditInput::Number { input: "5".into() },
            }],
        )
        .unwrap();
    let current = revision(&runtime);
    runtime
        .duplicate_collection(&current, "grid", "Copy", &BTreeMap::new())
        .unwrap();
    assert_eq!(runtime.profile, SessionProfile::WorkingSize);

    let current = revision(&runtime);
    runtime
        .update_formula(
            &current,
            &FieldTarget::from("r00000.calc"),
            "[.rate] * [.c0]",
        )
        .unwrap();
    let current = revision(&runtime);
    runtime.history_edit(&current, false).unwrap();
    let current = revision(&runtime);
    runtime.history_edit(&current, true).unwrap();
    assert_eq!(runtime.profile, SessionProfile::WorkingSize);

    let current = revision(&runtime);
    let preview = runtime
        .preview_cleanup(
            &current,
            &CleanupOperation::Trim {
                fields: vec![FieldTarget::from("r00000.c1")],
            },
        )
        .unwrap();
    assert_eq!(preview.changes.len(), 1);
    runtime
        .commit_cleanup(&current, &preview.preview_id)
        .unwrap();
    assert_eq!(runtime.profile, SessionProfile::WorkingSize);

    let table = runtime.query_table("grid").unwrap();
    let row = table
        .rows
        .iter()
        .find(|row| row.id == "r00000")
        .expect("row 0 exists");
    match &projected(row, "c1").stored {
        Some(StoredValueProjection::Text { value }) => assert_eq!(value, "spaced"),
        other => panic!("expected the trimmed text, got {other:?}"),
    }
}

#[test]
fn item_bounds_refuse_on_open_and_on_edit() {
    let mut identity = capacity_document(FixtureSpec {
        rows: 2,
        columns: 2,
        formula_cells: 0,
        seed: 5,
    });
    identity.title = "x".repeat(4097);
    let error = expect_error(DesignerRuntime::from_document_working_size(
        identity, OCCURRENCE,
    ));
    assert_unsupported_message(&error, "4096-byte maximum");

    let mut long_text = capacity_document(FixtureSpec {
        rows: 2,
        columns: 2,
        formula_cells: 0,
        seed: 5,
    });
    long_text
        .entities
        .get_mut(&entity_id(0))
        .unwrap()
        .fields
        .insert(
            FieldId::from("c1"),
            Value::Text("x".repeat(STORED_TEXT_ITEM_MAX + 1)),
        );
    let error = expect_error(DesignerRuntime::from_document_working_size(
        long_text, OCCURRENCE,
    ));
    assert_unsupported_message(&error, "65536-byte projection maximum");

    let mut chain = capacity_document(FixtureSpec {
        rows: 2,
        columns: 2,
        formula_cells: 0,
        seed: 5,
    });
    chain
        .schemas
        .get_mut(&SchemaId::from("grid"))
        .unwrap()
        .fields
        .insert(
            FieldId::from("chain"),
            FieldDefinition {
                id: FieldId::from("chain"),
                key: FieldKey::from("chain"),
                display_name: None,
                field_type: FieldType::Number,
                required: false,
                constraint: FieldConstraint::None,
                number_format: None,
            },
        );
    chain
        .entities
        .get_mut(&entity_id(0))
        .unwrap()
        .fields
        .insert(FieldId::from("chain"), Value::Formula(add_chain(129)));
    let error = expect_error(DesignerRuntime::from_document_working_size(
        chain, OCCURRENCE,
    ));
    assert_unsupported_message(&error, "256-node maximum");

    let mut over = text_grid(128, 5);
    for row in 0..128 {
        for column in 0..4 {
            fill_text(&mut over, row, column, STORED_TEXT_ITEM_MAX);
        }
    }
    fill_text(&mut over, 127, 3, 1);
    fill_text(&mut over, 0, 4, STORED_TEXT_ITEM_MAX);
    let error = expect_error(DesignerRuntime::from_document_working_size(
        over, OCCURRENCE,
    ));
    assert_unsupported_message(&error, "33554432-byte working-size maximum");

    let mut near = text_grid(128, 5);
    for row in 0..128 {
        for column in 0..4 {
            fill_text(&mut near, row, column, STORED_TEXT_ITEM_MAX);
        }
    }
    fill_text(&mut near, 127, 3, 1);
    let stored = 511 * STORED_TEXT_ITEM_MAX + 1;
    assert!(stored < MAX_WORKING_STORED_TEXT_BYTES);
    let mut runtime = DesignerRuntime::from_document_working_size(near, OCCURRENCE).unwrap();
    assert_eq!(runtime.profile, SessionProfile::WorkingSize);

    let current = revision(&runtime);
    let error = runtime
        .edit_cells(
            &current,
            &[CellEdit {
                target: FieldTarget::from("r00000.c4"),
                input: ScalarEditInput::Text {
                    value: "x".repeat(STORED_TEXT_ITEM_MAX),
                },
            }],
        )
        .unwrap_err();
    assert_unsupported_message(&error, "33554432-byte working-size maximum");
    assert_eq!(runtime.current_revision(), current);

    let mut small = DesignerRuntime::from_document_working_size(
        capacity_document(FixtureSpec {
            rows: 2,
            columns: 2,
            formula_cells: 0,
            seed: 5,
        }),
        OCCURRENCE,
    )
    .unwrap();
    let current = revision(&small);
    let error = small
        .edit_cells(
            &current,
            &[CellEdit {
                target: FieldTarget::from("r00000.c1"),
                input: ScalarEditInput::Text {
                    value: "x".repeat(STORED_TEXT_ITEM_MAX + 1),
                },
            }],
        )
        .unwrap_err();
    assert_unsupported_message(&error, "65536-byte projection maximum");
    assert_eq!(small.current_revision(), current);
}

#[test]
fn saved_projects_reopen_at_working_size_with_the_same_window() {
    let document = fixture();
    let bundle = project_bundle(&document);
    let mut resident: Option<DesignerRuntime> = None;
    let opened = open_project_working_size(&mut resident, &bundle, OCCURRENCE).unwrap();
    let mut runtime = resident.unwrap();
    assert!(runtime.opened_from_v3);
    let window = opened.window;
    assert_eq!(window, runtime.opened_window().unwrap().window);

    let current = revision(&runtime);
    let v3 = runtime.export_project_v3(&current).unwrap();
    let mut reopened: Option<DesignerRuntime> = None;
    let from_v3 = open_project_working_size(&mut reopened, &v3.bytes, OCCURRENCE).unwrap();
    assert_eq!(from_v3.window, window);

    let current = revision(&runtime);
    let tree = runtime.export_canonical_tree(&current).unwrap();
    let mut reopened: Option<DesignerRuntime> = None;
    let from_tree = open_project_working_size(&mut reopened, &tree.bytes, OCCURRENCE).unwrap();
    assert_eq!(from_tree.window, window);

    let current = revision(&runtime);
    let portable = runtime.export_portable_ro(&current).unwrap();
    let mut reopened: Option<DesignerRuntime> = None;
    let from_ro =
        open_portable_ro_working_size(&mut reopened, &portable.bytes, OCCURRENCE).unwrap();
    assert_eq!(from_ro.window, window);

    let mut resident: Option<DesignerRuntime> = None;
    assert_ordinary_refusal(&open_project(&mut resident, &v3.bytes, OCCURRENCE).unwrap_err());
    let mut resident: Option<DesignerRuntime> = None;
    assert_ordinary_refusal(&open_project(&mut resident, &tree.bytes, OCCURRENCE).unwrap_err());
    let mut resident: Option<DesignerRuntime> = None;
    assert_ordinary_refusal(
        &open_portable_ro(&mut resident, &portable.bytes, OCCURRENCE).unwrap_err(),
    );
}

/// A document can hold more text than one reply, so its first window is short
/// rather than leaving the document unable to open or save.
#[test]
fn a_first_window_over_the_reply_budget_comes_back_shorter() {
    let mut document = text_grid(128, 5);
    for row in 0..100 {
        for column in 0..5 {
            fill_text(&mut document, row, column, 48 * 1024);
        }
    }
    let mut resident: Option<DesignerRuntime> = None;
    let opened =
        open_project_working_size(&mut resident, &project_bundle(&document), OCCURRENCE).unwrap();
    assert!(
        (1..100).contains(&opened.window.rows.len()),
        "{} rows",
        opened.window.rows.len()
    );
    assert_eq!(opened.window.total_rows, 128);

    let runtime = resident.unwrap();
    let current = revision(&runtime);
    for bytes in [
        runtime.export_project_v3(&current).unwrap().bytes,
        runtime.export_canonical_tree(&current).unwrap().bytes,
    ] {
        let mut reopened: Option<DesignerRuntime> = None;
        let again = open_project_working_size(&mut reopened, &bytes, OCCURRENCE).unwrap();
        assert_eq!(again.window, opened.window);
    }
}

/// The largest row this profile admits still fits one reply: 32 cells of the
/// largest text, each byte escaped six times over. So a first window of one row
/// can always open, and every writer's reopen check passes.
#[test]
fn a_row_of_the_largest_cells_opens_and_saves() {
    let mut document = text_grid(1, 32);
    let text = "\u{1}".repeat(STORED_TEXT_ITEM_MAX);
    for column in 0..32 {
        document
            .entities
            .get_mut(&EntityId::from("r00000"))
            .unwrap()
            .fields
            .insert(
                FieldId::from(format!("c{column}")),
                Value::Text(text.clone()),
            );
    }
    let mut runtime = DesignerRuntime::from_document_working_size(document, OCCURRENCE).unwrap();
    assert_eq!(runtime.opened_window().unwrap().window.rows.len(), 1);
    let current = revision(&runtime);
    runtime.export_project_v3(&current).unwrap();
    runtime.export_canonical_tree(&current).unwrap();
    runtime.export_portable_ro(&current).unwrap();
}

/// Stored presentation arrives through every working-size open and leaves
/// through every save, as it does on the ordinary routes.
#[test]
fn working_size_open_and_save_carry_the_presentation_store() {
    let document = text_grid(2, 1);
    let store = PresentationStore {
        tables: BTreeMap::from([(
            SchemaId::from("grid"),
            TablePresentation {
                title: Some("Styled".into()),
                footer: BTreeMap::new(),
                column_styles: BTreeMap::new(),
            },
        )]),
        ..PresentationStore::default()
    };
    let tree = encode_roproj_v3_with_presentation(&document, &store).unwrap();
    let bundle =
        encode_project_bundle(tree.files().iter().map(|file| (file.path(), file.bytes()))).unwrap();
    let portable = encode_portable_package_v1(&tree).unwrap();

    let mut resident: Option<DesignerRuntime> = None;
    open_project_working_size(&mut resident, &bundle, OCCURRENCE).unwrap();
    let runtime = resident.unwrap();
    assert_eq!(runtime.presentation, store);

    let mut from_portable: Option<DesignerRuntime> = None;
    open_portable_ro_working_size(&mut from_portable, &portable, OCCURRENCE).unwrap();
    assert_eq!(from_portable.unwrap().presentation, store);
    let mut from_local: Option<DesignerRuntime> = None;
    open_local_document_working_size(&mut from_local, &portable, OCCURRENCE).unwrap();
    assert_eq!(from_local.unwrap().presentation, store);

    let current = revision(&runtime);
    for bytes in [
        runtime.export_project_v3(&current).unwrap().bytes,
        runtime.export_canonical_tree(&current).unwrap().bytes,
    ] {
        let mut reopened: Option<DesignerRuntime> = None;
        open_project_working_size(&mut reopened, &bytes, OCCURRENCE).unwrap();
        assert_eq!(reopened.unwrap().presentation, store);
    }
    let saved = runtime.export_portable_ro(&current).unwrap().bytes;
    let mut reopened: Option<DesignerRuntime> = None;
    open_portable_ro_working_size(&mut reopened, &saved, OCCURRENCE).unwrap();
    assert_eq!(reopened.unwrap().presentation, store);
}

fn wire_columns() -> [NewTableColumnInput; 1] {
    [NewTableColumnInput {
        name: "amount".into(),
        field_type: "number".into(),
        required: false,
    }]
}

fn decode_wire(bytes: &[u8]) -> DesignerWireReply {
    serde_json::from_slice(bytes).unwrap()
}

#[test]
fn new_table_without_a_profile_keeps_todays_wire_shape() {
    let request = DesignerRequest::NewTable {
        occurrence_id: OCCURRENCE.to_owned(),
        name: "Ordinary".to_owned(),
        columns: wire_columns().to_vec(),
        profile: None,
    };
    let input = serde_json::to_vec(&request).unwrap();
    assert!(
        !String::from_utf8_lossy(&input).contains("profile"),
        "a host that does not ask sends no profile field"
    );
    let mut resident: Option<DesignerRuntime> = None;
    let reply = decode_wire(&process_wire_request(&mut resident, &input));
    let DesignerWireReply::Ok {
        response: DesignerResponse::Opened(opened),
    } = reply
    else {
        panic!("today's new_table keeps the opened reply");
    };
    assert_eq!(opened.table.columns.len(), 1);
    let runtime = resident.unwrap();
    assert_eq!(runtime.profile, SessionProfile::Ordinary);
}

#[test]
fn new_table_with_the_working_size_profile_replies_a_zero_row_window() {
    let request = DesignerRequest::NewTable {
        occurrence_id: OCCURRENCE.to_owned(),
        name: "Working".to_owned(),
        columns: wire_columns().to_vec(),
        profile: Some(DesignerProfile::WorkingSize),
    };
    let input = serde_json::to_vec(&request).unwrap();
    let mut resident: Option<DesignerRuntime> = None;
    let reply = decode_wire(&process_wire_request(&mut resident, &input));
    let DesignerWireReply::Ok {
        response: DesignerResponse::OpenedWindow(opened),
    } = reply
    else {
        panic!("a working-size new_table replies one window");
    };
    assert_eq!(opened.profile, DesignerProfile::WorkingSize);
    assert_eq!(opened.window.total_rows, 0);
    assert_eq!(opened.window.rows, Vec::<RowProjection>::new());
    let collection = opened.bootstrap.default_collection.clone();
    assert_window_reply_shape(&opened);

    let mut runtime = resident.unwrap();
    assert_eq!(runtime.profile, SessionProfile::WorkingSize);
    let current = revision(&runtime);
    let inserted = runtime.insert_row(&current, &collection, &[]).unwrap();
    assert_ne!(inserted.resulting_revision, current);
    assert_eq!(runtime.profile, SessionProfile::WorkingSize);

    // A refused candidate leaves the resident exactly as it was.
    let current = revision(&runtime);
    let before = runtime.session.export_snapshot();
    let refused = DesignerRequest::NewTable {
        occurrence_id: OCCURRENCE.to_owned(),
        name: "Empty".to_owned(),
        columns: Vec::new(),
        profile: Some(DesignerProfile::WorkingSize),
    };
    let mut resident = Some(runtime);
    let reply = decode_wire(&process_wire_request(
        &mut resident,
        &serde_json::to_vec(&refused).unwrap(),
    ));
    let DesignerWireReply::Error { error } = reply else {
        panic!("a column-less candidate refuses");
    };
    assert_eq!(error.code, "invalid_table_operation");
    let runtime = resident.unwrap();
    assert_eq!(runtime.current_revision(), current);
    assert_eq!(runtime.session.export_snapshot(), before);
    assert_eq!(runtime.profile, SessionProfile::WorkingSize);
}

#[test]
fn an_unknown_profile_is_an_invalid_request() {
    let request = DesignerRequest::NewTable {
        occurrence_id: OCCURRENCE.to_owned(),
        name: "Other".to_owned(),
        columns: wire_columns().to_vec(),
        profile: None,
    };
    let mut request = serde_json::to_value(&request).unwrap();
    request
        .as_object_mut()
        .unwrap()
        .insert("profile".into(), serde_json::Value::String("other".into()));
    let input = serde_json::to_vec(&request).unwrap();
    let runtime = DesignerRuntime::tracker(OCCURRENCE).unwrap();
    let before = runtime.session.export_snapshot();
    let mut resident = Some(runtime);
    let reply = decode_wire(&process_wire_request(&mut resident, &input));
    let DesignerWireReply::Error { error } = reply else {
        panic!("an unknown profile is not a request");
    };
    assert_eq!(error.code, "invalid_request");
    let runtime = resident.unwrap();
    assert_eq!(runtime.session.export_snapshot(), before);
    assert_eq!(runtime.profile, SessionProfile::Ordinary);
}

#[test]
fn handle_and_the_wire_agree_on_a_working_size_new_table() {
    let request = DesignerRequest::NewTable {
        occurrence_id: OCCURRENCE.to_owned(),
        name: "Agreed".to_owned(),
        columns: wire_columns().to_vec(),
        profile: Some(DesignerProfile::WorkingSize),
    };
    let mut handled = DesignerRuntime::tracker(OCCURRENCE).unwrap();
    let expected = handled.handle(request.clone()).unwrap();
    let mut wire = Some(DesignerRuntime::tracker(OCCURRENCE).unwrap());
    let reply = decode_wire(&process_wire_request(
        &mut wire,
        &serde_json::to_vec(&request).unwrap(),
    ));
    assert_eq!(reply, DesignerWireReply::Ok { response: expected });
}

/// A caller's byte budget shrinks the opening window by halving, and a budget
/// below one row is the same typed refusal the profile uses everywhere.
#[test]
fn opened_window_within_halves_to_the_budget() {
    let document = text_grid(4, 1);
    let mut full_runtime =
        DesignerRuntime::from_document_working_size(document, OCCURRENCE).unwrap();
    let full = full_runtime.opened_window().unwrap();
    assert_eq!(full.window.rows.len(), 4);
    let full_bytes = serde_json::to_vec(&full).unwrap().len();

    let mut shrunk_runtime =
        DesignerRuntime::from_document_working_size(text_grid(4, 1), OCCURRENCE).unwrap();
    let shrunk = shrunk_runtime.opened_window_within(full_bytes - 1).unwrap();
    assert!(
        shrunk.window.rows.len() < full.window.rows.len(),
        "{} rows did not shrink below {}",
        shrunk.window.rows.len(),
        full.window.rows.len()
    );
    assert_ne!(
        shrunk.window.rows,
        [] as [RowProjection; 0],
        "the shrunk window keeps at least one row"
    );

    let mut tiny_runtime =
        DesignerRuntime::from_document_working_size(text_grid(4, 1), OCCURRENCE).unwrap();
    let error = expect_error(tiny_runtime.opened_window_within(0));
    assert!(
        matches!(error, DesignerError::ProjectionTooLarge { maximum: 0, .. }),
        "{error:?}"
    );
}

fn window_at(
    runtime: &mut DesignerRuntime,
    revision: &str,
    start: usize,
    count: usize,
) -> TableWindowProjection {
    match runtime
        .handle(DesignerRequest::QueryTableWindow {
            expected_revision: revision.to_owned(),
            collection: "grid".to_owned(),
            view: TableView::default(),
            start,
            count,
            display: false,
        })
        .unwrap()
    {
        DesignerResponse::TableWindow(window) => window,
        other => panic!("expected a table window, got {other:?}"),
    }
}

fn edited(
    runtime: &mut DesignerRuntime,
    target: FieldTarget,
    input: ScalarEditInput,
) -> PublicationProjection {
    let expected_revision = runtime.current_revision().to_owned();
    match runtime
        .handle(DesignerRequest::EditCells {
            expected_revision,
            edits: vec![CellEdit { target, input }],
        })
        .unwrap_or_else(|error| panic!("edit failed: {error:?}"))
    {
        DesignerResponse::Published(publication) => publication,
        other => panic!("expected a publication, got {other:?}"),
    }
}

fn target(entity: &str, field: &str) -> FieldTarget {
    FieldTarget {
        entity: entity.to_owned(),
        field: field.to_owned(),
    }
}

/// Every row of one Text column carries the largest stored text, so the whole
/// 300-row view is over the 16 MiB reply.
fn large_text_grid(rows: usize) -> Document {
    let mut document = text_grid(rows, 1);
    for row in 0..rows {
        fill_text(&mut document, row, 0, STORED_TEXT_ITEM_MAX);
    }
    document
}

/// The largest row this profile admits, repeated, so two rows cannot share one
/// reply but one always can.
fn largest_row_grid(rows: usize) -> Document {
    let mut document = text_grid(rows, 32);
    let text = "\u{1}".repeat(STORED_TEXT_ITEM_MAX);
    for row in 0..rows {
        for column in 0..32 {
            document
                .entities
                .get_mut(&EntityId::from(format!("r{row:05}")))
                .unwrap()
                .fields
                .insert(
                    FieldId::from(format!("c{column}")),
                    Value::Text(text.clone()),
                );
        }
    }
    document
}

/// A Number column whose calculated column reads one shared stored cell by
/// absolute `Reference`, so one edit has as many dependents as rows.
fn shared_reference_grid(rows: usize) -> Document {
    let mut document = Document::empty("shared-reference", "Shared Reference");
    let mut schema = Schema {
        field_order: None,
        id: SchemaId::from("grid"),
        key: SchemaKey::from("grid"),
        display_name: None,
        fields: BTreeMap::new(),
    };
    for id in ["value", "calc"] {
        schema.fields.insert(
            FieldId::from(id),
            FieldDefinition {
                id: FieldId::from(id),
                key: FieldKey::from(id),
                display_name: None,
                field_type: FieldType::Number,
                required: false,
                constraint: FieldConstraint::None,
                number_format: None,
            },
        );
    }
    document.schemas.insert(schema.id.clone(), schema);
    let shared = Expression::Reference(FieldRef::new(
        EntityId::from("r00000"),
        FieldId::from("value"),
    ));
    for row in 0..rows {
        let id = EntityId::from(format!("r{row:05}"));
        let mut fields = BTreeMap::from([(FieldId::from("calc"), Value::Formula(shared.clone()))]);
        if row == 0 {
            fields.insert(
                FieldId::from("value"),
                Value::Number(Number::new(1.0).unwrap()),
            );
        }
        document.entities.insert(
            id.clone(),
            Entity {
                id: id.clone(),
                key: EntityKey::from(format!("k{row:05}")),
                schema: SchemaId::from("grid"),
                fields,
            },
        );
    }
    document
}

fn publication_with_targets(total: usize) -> PublicationProjection {
    PublicationProjection {
        base_revision: "resident/1".to_owned(),
        resulting_revision: "resident/2".to_owned(),
        schemas: vec!["grid".to_owned()],
        schema_fields: vec![super::SchemaFieldTarget {
            schema: "grid".to_owned(),
            field: "c0".to_owned(),
        }],
        entities: Vec::new(),
        fields: (0..total)
            .map(|index| FieldTarget {
                entity: "r00000".to_owned(),
                field: format!("c{index}"),
            })
            .collect(),
        affected_calculations: Vec::new(),
        many: false,
        changed: Vec::new(),
    }
}

fn number_field(id: &str, number_format: Option<NumberFormat>) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(id),
        display_name: None,
        field_type: FieldType::Number,
        required: false,
        constraint: FieldConstraint::None,
        number_format,
    }
}

fn text_field(id: &str) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(id),
        display_name: None,
        field_type: FieldType::Text,
        required: false,
        constraint: FieldConstraint::None,
        number_format: None,
    }
}

fn num(value: f64) -> Number {
    Number::new(value).expect("fixture numbers are finite")
}

fn plain_format() -> NumberFormat {
    NumberFormat {
        decimals: 2,
        kind: NumberFormatKind::Plain,
        thousands_separator: true,
    }
}

/// The redraw differential's fixture. `ledger` holds a value, a same-row
/// formula over it, a second formula reading the first, a column total over the
/// values and a formatted formula column; `mirror` reads one ledger value by
/// absolute reference from another schema.
fn redraw_grid() -> Document {
    let mut document = Document::empty("redraw", "Redraw");
    let mut ledger = Schema {
        field_order: None,
        id: SchemaId::from("ledger"),
        key: SchemaKey::from("ledger"),
        display_name: None,
        fields: BTreeMap::new(),
    };
    ledger
        .fields
        .insert(FieldId::from("value"), number_field("value", None));
    ledger
        .fields
        .insert(FieldId::from("divisor"), number_field("divisor", None));
    ledger
        .fields
        .insert(FieldId::from("label"), text_field("label"));
    ledger.fields.insert(
        FieldId::from("first"),
        number_field("first", Some(plain_format())),
    );
    ledger
        .fields
        .insert(FieldId::from("second"), number_field("second", None));
    ledger
        .fields
        .insert(FieldId::from("agg"), number_field("agg", None));
    document.schemas.insert(ledger.id.clone(), ledger);
    let mut mirror = Schema {
        field_order: None,
        id: SchemaId::from("mirror"),
        key: SchemaKey::from("mirror"),
        display_name: None,
        fields: BTreeMap::new(),
    };
    mirror
        .fields
        .insert(FieldId::from("mirror_of"), number_field("mirror_of", None));
    document.schemas.insert(mirror.id.clone(), mirror);

    for row in 0..3 {
        let id = EntityId::from(format!("r{row:05}"));
        let row_number = f64::from(u32::try_from(row).expect("bounded"));
        let value = num(1.5 + row_number);
        let divisor = num(2.0 * (row_number + 1.0));
        let mut fields = BTreeMap::from([
            (FieldId::from("value"), Value::Number(value)),
            (FieldId::from("divisor"), Value::Number(divisor)),
            (FieldId::from("label"), Value::Text(format!("row-{row}"))),
            (
                FieldId::from("first"),
                Value::Formula(Expression::Multiply {
                    left: Box::new(Expression::SameRowReference(FieldId::from("value"))),
                    right: Box::new(Expression::Number(num(2.0))),
                }),
            ),
            (
                FieldId::from("second"),
                Value::Formula(Expression::Divide {
                    left: Box::new(Expression::SameRowReference(FieldId::from("first"))),
                    right: Box::new(Expression::SameRowReference(FieldId::from("divisor"))),
                }),
            ),
        ]);
        if row == 0 {
            fields.insert(
                FieldId::from("agg"),
                Value::Formula(Expression::ColumnTotal(ColumnRef::new("ledger", "value"))),
            );
        }
        document.entities.insert(
            id.clone(),
            Entity {
                id,
                key: EntityKey::from(format!("k{row:05}")),
                schema: SchemaId::from("ledger"),
                fields,
            },
        );
    }
    let shared = Expression::Reference(FieldRef::new(
        EntityId::from("r00000"),
        FieldId::from("value"),
    ));
    for row in 0..2 {
        let id = EntityId::from(format!("m{row:05}"));
        document.entities.insert(
            id.clone(),
            Entity {
                id,
                key: EntityKey::from(format!("mk{row:05}")),
                schema: SchemaId::from("mirror"),
                fields: BTreeMap::from([(
                    FieldId::from("mirror_of"),
                    Value::Formula(shared.clone()),
                )]),
            },
        );
    }
    document
}

/// One table cell value as the window shows it: a present field, or `None` for
/// a cell the row names in `blank_fields`.
type WindowCells = BTreeMap<(String, String), Option<FieldProjection>>;

fn window_cells(runtime: &mut DesignerRuntime, collection: &str) -> WindowCells {
    let expected_revision = runtime.current_revision().to_owned();
    let window = match runtime
        .handle(DesignerRequest::QueryTableWindow {
            expected_revision,
            collection: collection.to_owned(),
            view: TableView::default(),
            start: 0,
            count: 256,
            display: true,
        })
        .unwrap()
    {
        DesignerResponse::TableWindow(window) => window,
        other => panic!("expected a table window, got {other:?}"),
    };
    let mut cells = WindowCells::new();
    for row in &window.rows {
        for field in &row.fields {
            cells.insert(
                (row.id.clone(), field.target.field.clone()),
                Some(field.clone()),
            );
        }
        for field in &row.blank_fields {
            cells.insert((row.id.clone(), field.clone()), None);
        }
    }
    cells
}

fn all_cells(runtime: &mut DesignerRuntime) -> WindowCells {
    let mut cells = window_cells(runtime, "ledger");
    cells.extend(window_cells(runtime, "mirror"));
    cells
}

fn formula_update(
    runtime: &mut DesignerRuntime,
    field: FieldTarget,
    source: &str,
) -> PublicationProjection {
    match runtime
        .handle(DesignerRequest::FormulaUpdate {
            expected_revision: runtime.current_revision().to_owned(),
            target: field,
            source: source.to_owned(),
        })
        .unwrap()
    {
        DesignerResponse::Published(publication) => publication,
        other => panic!("expected a publication, got {other:?}"),
    }
}

fn history(runtime: &mut DesignerRuntime, redo: bool) -> PublicationProjection {
    let expected_revision = runtime.current_revision().to_owned();
    match runtime
        .handle(if redo {
            DesignerRequest::Redo { expected_revision }
        } else {
            DesignerRequest::Undo { expected_revision }
        })
        .unwrap()
    {
        DesignerResponse::Published(publication) => publication,
        other => panic!("expected a publication, got {other:?}"),
    }
}

/// Every cell whose projection differs is in `changed`, and every `changed`
/// entry equals the field the after-window shows (a blank cell appears in the
/// row's `blank_fields`, so its entry must be `blank`).
fn assert_changed_covers(
    before: &WindowCells,
    after: &WindowCells,
    publication: &PublicationProjection,
) {
    let changed = publication
        .changed
        .iter()
        .map(|field| {
            (
                (field.target.entity.clone(), field.target.field.clone()),
                field,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let keys = before.keys().chain(after.keys()).collect::<BTreeSet<_>>();
    for key in keys {
        let before_cell = before.get(key);
        let after_cell = after.get(key);
        if before_cell == after_cell {
            continue;
        }
        assert!(
            changed.contains_key(key),
            "changed cell {key:?} is missing from the reply: {publication:?}"
        );
    }
    for (key, field) in &changed {
        match after.get(key) {
            Some(Some(window_field)) => assert_eq!(
                *field, window_field,
                "changed entry {key:?} differs from the window"
            ),
            Some(None) => assert!(
                field.blank,
                "changed entry {key:?} must be a blank projection"
            ),
            None => panic!("changed entry {key:?} is not a cell of the after-window"),
        }
    }
}

fn all_rows_window(runtime: &mut DesignerRuntime, collection: &str) -> TableWindowProjection {
    let expected_revision = runtime.current_revision().to_owned();
    match runtime
        .handle(DesignerRequest::QueryTableWindow {
            expected_revision,
            collection: collection.to_owned(),
            view: TableView::default(),
            start: 0,
            count: 256,
            display: true,
        })
        .unwrap()
    {
        DesignerResponse::TableWindow(window) => window,
        other => panic!("expected a table window, got {other:?}"),
    }
}

#[test]
fn a_window_over_its_bytes_pages_the_whole_view() {
    let mut runtime =
        DesignerRuntime::from_document_working_size(large_text_grid(300), OCCURRENCE).unwrap();
    let current = revision(&runtime);

    // A request may ask for at most 256 rows, and 256 of these rows already
    // exceed the 16 MiB reply, so the first window comes back short.
    let mut window = window_at(&mut runtime, &current, 0, 256);
    assert!(window.byte_limited);
    assert!(window.rows.len() < 256);
    assert_eq!(window.total_rows, 300);

    let mut start = 0;
    let mut seen = Vec::new();
    loop {
        assert_eq!(window.start, start);
        for (offset, row) in window.rows.iter().enumerate() {
            assert_eq!(row.id, format!("r{:05}", start + offset));
            assert_eq!(
                projected(row, "c0").stored,
                Some(StoredValueProjection::Text {
                    value: "x".repeat(STORED_TEXT_ITEM_MAX),
                })
            );
            seen.push(row.id.clone());
        }
        start += window.rows.len();
        if start >= window.total_rows {
            assert!(
                !window.byte_limited,
                "the last page had room for every row left"
            );
            break;
        }
        assert!(
            window.byte_limited,
            "a short page must say it was byte limited"
        );
        window = window_at(&mut runtime, &current, start, 256);
    }
    assert_eq!(
        seen,
        (0..300).map(|row| format!("r{row:05}")).collect::<Vec<_>>()
    );
}

#[test]
fn a_window_starting_inside_the_view_keeps_one_row() {
    let mut runtime =
        DesignerRuntime::from_document_working_size(largest_row_grid(2), OCCURRENCE).unwrap();
    let current = revision(&runtime);

    let first = window_at(&mut runtime, &current, 0, 2);
    assert_eq!(first.total_rows, 2);
    assert_eq!(first.rows.len(), 1);
    assert!(first.byte_limited);

    let last = window_at(&mut runtime, &current, 1, 2);
    assert_eq!(last.rows.len(), 1);
    assert!(!last.byte_limited);

    for start in [2, 5] {
        let past = window_at(&mut runtime, &current, start, 2);
        assert_eq!(past.start, 2);
        assert_eq!(past.rows, [] as [RowProjection; 0]);
        assert!(!past.byte_limited);
    }

    let none = window_at(&mut runtime, &current, 0, 0);
    assert_eq!(none.rows, [] as [RowProjection; 0]);
    assert!(!none.byte_limited);
}

#[test]
fn a_window_under_its_bytes_has_no_byte_limited_key() {
    let mut runtime =
        DesignerRuntime::from_document_working_size(text_grid(4, 1), OCCURRENCE).unwrap();
    let current = revision(&runtime);
    let window = window_at(&mut runtime, &current, 0, 2);
    assert_eq!(window.rows.len(), 2);
    assert!(!window.byte_limited);
    let json = serde_json::to_string(&window).unwrap();
    assert!(!json.contains("byte_limited"), "{json}");
}

/// Ordinary documents cannot grow a window past the 1 MiB reply they may open
/// with, so the unchanged ordinary branch is checked at its budget check.
#[test]
fn an_ordinary_window_over_its_bytes_still_refuses() {
    let runtime = DesignerRuntime::from_document(text_grid(2, 1), OCCURRENCE).unwrap();
    assert_eq!(runtime.profile, SessionProfile::Ordinary);
    let spec = runtime.collection_spec("grid").unwrap();
    let entities = spec.entities.clone();
    let positions = (0..entities.len()).collect::<Vec<_>>();
    let mut used = super::MAX_TABLE_WINDOW_BYTES;
    let error = runtime
        .build_window_rows(spec, &positions, &entities, &mut used, false)
        .unwrap_err();
    assert!(
        matches!(
            error,
            DesignerError::ProjectionTooLarge { maximum, .. }
                if maximum == super::MAX_TABLE_WINDOW_BYTES
        ),
        "{error:?}"
    );
    assert_eq!(used, super::MAX_TABLE_WINDOW_BYTES);
}

#[test]
fn a_publication_lists_targets_up_to_the_bound_then_turns_into_a_marker() {
    let listed = super::bound_publication_targets(publication_with_targets(1_024));
    assert!(!listed.many);
    assert_eq!(listed.fields.len(), 1_024);
    assert_eq!(listed.schemas, vec!["grid".to_owned()]);
    assert_eq!(listed.schema_fields.len(), 1);
    assert_eq!(listed.base_revision, "resident/1");
    assert_eq!(listed.resulting_revision, "resident/2");

    let bounded = super::bound_publication_targets(publication_with_targets(1_025));
    assert!(bounded.many);
    assert_eq!(bounded.entities, [] as [String; 0]);
    assert_eq!(bounded.fields, [] as [FieldTarget; 0]);
    assert_eq!(bounded.affected_calculations, [] as [FieldTarget; 0]);
    assert_eq!(bounded.schemas, vec!["grid".to_owned()]);
    assert_eq!(bounded.schema_fields.len(), 1);
    assert_eq!(bounded.base_revision, "resident/1");
    assert_eq!(bounded.resulting_revision, "resident/2");
}

#[test]
fn a_publication_over_the_target_list_comes_back_as_a_marker() {
    let mut runtime =
        DesignerRuntime::from_document_working_size(shared_reference_grid(1_100), OCCURRENCE)
            .unwrap();
    let publication = edited(
        &mut runtime,
        target("r00000", "value"),
        ScalarEditInput::Number { input: "7".into() },
    );
    assert!(publication.many);
    assert_eq!(publication.entities, [] as [String; 0]);
    assert_eq!(publication.fields, [] as [FieldTarget; 0]);
    assert_eq!(publication.affected_calculations, [] as [FieldTarget; 0]);
    assert_ne!(publication.base_revision, publication.resulting_revision);
    let json = serde_json::to_string(&publication).unwrap();
    assert!(json.contains("\"many\":true"), "{json}");

    let window = window_at(&mut runtime, &publication.resulting_revision, 0, 3);
    assert_eq!(window.rows.len(), 3);
    for row in &window.rows {
        assert_eq!(
            projected(row, "calc").calculated,
            Some(CalculationProjection::Value { value: 7.0 })
        );
    }

    // A cell with no dependents still lists its target.
    let listed = edited(
        &mut runtime,
        target("r00001", "value"),
        ScalarEditInput::Number { input: "4".into() },
    );
    assert!(!listed.many);
    assert_eq!(listed.fields, vec![target("r00001", "value")]);
    assert_eq!(listed.affected_calculations, [] as [FieldTarget; 0]);
    let json = serde_json::to_string(&listed).unwrap();
    assert!(!json.contains("\"many\""), "{json}");
}

#[test]
fn row_removal_and_undo_over_the_target_list_come_back_as_markers() {
    let mut runtime =
        DesignerRuntime::from_document_working_size(text_grid(1_100, 1), OCCURRENCE).unwrap();
    let current = revision(&runtime);
    let rows = (0..1_100)
        .map(|row| format!("r{row:05}"))
        .collect::<Vec<_>>();
    let removed = runtime.remove_table_rows(&current, "grid", &rows).unwrap();
    assert!(removed.many);
    assert_eq!(removed.entities, [] as [String; 0]);
    assert_eq!(removed.fields, [] as [FieldTarget; 0]);
    assert_eq!(removed.affected_calculations, [] as [FieldTarget; 0]);
    let json = serde_json::to_string(&removed).unwrap();
    assert!(json.contains("\"many\":true"), "{json}");

    let current = revision(&runtime);
    let undone = runtime.history_edit(&current, false).unwrap();
    assert!(undone.many);
    let json = serde_json::to_string(&undone).unwrap();
    assert!(json.contains("\"many\":true"), "{json}");

    // Today's ordinary replies never carry the marker.
    let mut ordinary = DesignerRuntime::from_document(text_grid(2, 1), OCCURRENCE).unwrap();
    let listed = edited(
        &mut ordinary,
        target("r00000", "c0"),
        ScalarEditInput::Text { value: "t".into() },
    );
    assert!(!listed.many);
    let json = serde_json::to_string(&listed).unwrap();
    assert!(!json.contains("\"many\""), "{json}");
}

#[test]
fn the_wire_reply_carries_the_publication_marker() {
    let mut runtime =
        Some(DesignerRuntime::from_document_working_size(text_grid(1_100, 1), OCCURRENCE).unwrap());
    let request = DesignerRequest::RemoveTableRows {
        expected_revision: runtime.as_ref().unwrap().current_revision().to_owned(),
        collection: "grid".to_owned(),
        entities: (0..1_100).map(|row| format!("r{row:05}")).collect(),
    };
    let reply = process_wire_request(&mut runtime, &serde_json::to_vec(&request).unwrap());
    let text = String::from_utf8(reply).unwrap();
    assert!(text.contains("\"many\":true"), "{text}");
}

#[test]
fn every_field_that_changes_is_in_the_reply() {
    let mut runtime =
        DesignerRuntime::from_document_working_size(redraw_grid(), OCCURRENCE).unwrap();

    // A number edit, read back through whole windows with display text.
    let before = all_cells(&mut runtime);
    let publication = edited(
        &mut runtime,
        target("r00000", "value"),
        ScalarEditInput::Number {
            input: "4.5".into(),
        },
    );
    let after = all_cells(&mut runtime);
    assert_changed_covers(&before, &after, &publication);

    // A clear to blank.
    let before = after;
    let publication = edited(
        &mut runtime,
        target("r00001", "value"),
        ScalarEditInput::Clear,
    );
    let after = all_cells(&mut runtime);
    assert_changed_covers(&before, &after, &publication);

    // A formula update and its dependents.
    let before = after;
    let publication = formula_update(&mut runtime, target("r00001", "first"), "[.value] * 3");
    let after = all_cells(&mut runtime);
    assert_changed_covers(&before, &after, &publication);

    // Undo and redo of the formula update.
    let before = after;
    let publication = history(&mut runtime, false);
    let after = all_cells(&mut runtime);
    assert_changed_covers(&before, &after, &publication);

    let before = after;
    let publication = history(&mut runtime, true);
    let after = all_cells(&mut runtime);
    assert_changed_covers(&before, &after, &publication);
}

#[test]
fn changed_cells_carry_the_windows_display_text() {
    let mut runtime =
        DesignerRuntime::from_document_working_size(redraw_grid(), OCCURRENCE).unwrap();
    let publication = edited(
        &mut runtime,
        target("r00000", "value"),
        ScalarEditInput::Number {
            input: "4.5".into(),
        },
    );
    let first = publication
        .changed
        .iter()
        .find(|field| field.target == target("r00000", "first"))
        .expect("the formatted formula is a changed cell");
    assert_eq!(first.display.as_deref(), Some("9.00"));

    // A Number without a format still carries its plain display text.
    let value = publication
        .changed
        .iter()
        .find(|field| field.target == target("r00000", "value"))
        .expect("the edited cell is a changed cell");
    assert_eq!(value.display.as_deref(), Some("4.5"));

    // A Text cell has no Number display text.
    let publication = edited(
        &mut runtime,
        target("r00001", "label"),
        ScalarEditInput::Text {
            value: "changed".into(),
        },
    );
    let label = publication
        .changed
        .iter()
        .find(|field| field.target == target("r00001", "label"))
        .expect("the edited text cell is a changed cell");
    assert_eq!(label.display, None);
}

/// A failed formula never reaches a reply: the lifecycle refuses to publish a
/// document whose calculation failed, so `changed` never has to carry one.
#[test]
fn a_failing_edit_is_refused_before_it_reaches_a_reply() {
    let mut runtime =
        DesignerRuntime::from_document_working_size(redraw_grid(), OCCURRENCE).unwrap();
    let revision = runtime.current_revision().to_owned();
    let error = runtime
        .handle(DesignerRequest::EditCells {
            expected_revision: revision.clone(),
            edits: vec![CellEdit {
                target: target("r00000", "divisor"),
                input: ScalarEditInput::Number { input: "0".into() },
            }],
        })
        .expect_err("a document with a failed formula is not published");
    assert!(
        matches!(&error, DesignerError::Lifecycle(_)),
        "the refusal is a lifecycle validation failure: {error:?}"
    );
    assert!(
        format!("{error:?}").contains("formula.division_by_zero"),
        "the refusal names the failure: {error:?}"
    );
    assert_eq!(runtime.current_revision(), revision);
}

#[test]
fn a_cleared_cell_appears_blank() {
    let mut runtime =
        DesignerRuntime::from_document_working_size(redraw_grid(), OCCURRENCE).unwrap();
    let publication = edited(
        &mut runtime,
        target("r00001", "value"),
        ScalarEditInput::Clear,
    );
    let cleared = publication
        .changed
        .iter()
        .find(|field| field.target == target("r00001", "value"))
        .expect("the cleared cell is a changed cell");
    assert!(cleared.blank);
    assert_eq!(cleared.stored, None);
    assert_eq!(cleared.calculated, None);

    let window = all_rows_window(&mut runtime, "ledger");
    let row = window
        .rows
        .iter()
        .find(|row| row.id == "r00001")
        .expect("the row remains");
    assert!(row.blank_fields.iter().any(|field| field == "value"));
    assert!(!row.fields.iter().any(|field| field.target.field == "value"));
}

#[test]
fn a_removed_rows_cells_are_not_changed_cells() {
    let mut runtime =
        DesignerRuntime::from_document_working_size(redraw_grid(), OCCURRENCE).unwrap();
    let publication = match runtime
        .handle(DesignerRequest::RemoveTableRows {
            expected_revision: runtime.current_revision().to_owned(),
            collection: "ledger".to_owned(),
            entities: vec!["r00002".to_owned()],
        })
        .unwrap()
    {
        DesignerResponse::Published(publication) => publication,
        other => panic!("expected a publication, got {other:?}"),
    };
    assert!(
        publication.entities.contains(&"r00002".to_owned()),
        "the removed row is named: {publication:?}"
    );
    assert!(
        publication
            .changed
            .iter()
            .all(|field| field.target.entity != "r00002"),
        "a removed row's cells are never changed cells: {publication:?}"
    );

    // An inserted row's cells are changed cells.
    let publication = match runtime
        .handle(DesignerRequest::InsertRow {
            expected_revision: runtime.current_revision().to_owned(),
            collection: "ledger".to_owned(),
            initializers: vec![RowInitializer {
                field: "value".to_owned(),
                input: ScalarEditInput::Number { input: "9".into() },
            }],
        })
        .unwrap()
    {
        DesignerResponse::Published(publication) => publication,
        other => panic!("expected a publication, got {other:?}"),
    };
    let inserted = publication
        .entities
        .first()
        .expect("the publication lists the new row");
    let inserted_value = publication
        .changed
        .iter()
        .find(|field| &field.target.entity == inserted && field.target.field == "value")
        .expect("the new row's cell is a changed cell");
    assert!(matches!(
        inserted_value.stored,
        Some(StoredValueProjection::Number { value }) if value.to_bits() == 9.0_f64.to_bits()
    ));
}

#[test]
fn a_number_format_change_shows_the_new_display_text() {
    // The format lives in `collection_specs`, which the command updates after
    // publication, so only a list built at the reply boundary sees it.
    let mut runtime =
        DesignerRuntime::from_document_working_size(redraw_grid(), OCCURRENCE).unwrap();
    let publication = match runtime
        .handle(DesignerRequest::SetColumnNumberFormat {
            expected_revision: runtime.current_revision().to_owned(),
            collection: "ledger".to_owned(),
            field: "value".to_owned(),
            number_format: Some(plain_format()),
        })
        .unwrap()
    {
        DesignerResponse::Published(publication) => publication,
        other => panic!("expected a publication, got {other:?}"),
    };
    let value = publication
        .changed
        .iter()
        .find(|field| field.target == target("r00000", "value"))
        .expect("the formatted column's cells are changed cells");
    assert_eq!(value.display.as_deref(), Some("1.50"));
}

#[test]
fn a_reply_over_the_changed_cell_budget_is_marked_many() {
    let mut runtime =
        DesignerRuntime::from_document_working_size(text_grid(20, 1), OCCURRENCE).unwrap();
    let text = "x".repeat(STORED_TEXT_ITEM_MAX);
    let edits = (0..20)
        .map(|row| CellEdit {
            target: target(&format!("r{row:05}"), "c0"),
            input: ScalarEditInput::Text {
                value: text.clone(),
            },
        })
        .collect::<Vec<_>>();
    let expected_revision = runtime.current_revision().to_owned();
    let publication = match runtime
        .handle(DesignerRequest::EditCells {
            expected_revision,
            edits,
        })
        .unwrap()
    {
        DesignerResponse::Published(publication) => publication,
        other => panic!("expected a publication, got {other:?}"),
    };
    assert!(publication.many, "over-budget changed cells are not listed");
    assert_eq!(publication.entities, [] as [String; 0]);
    assert_eq!(publication.fields, [] as [FieldTarget; 0]);
    assert_eq!(publication.affected_calculations, [] as [FieldTarget; 0]);
    assert_eq!(publication.changed, [] as [FieldProjection; 0]);
    let json = serde_json::to_string(&publication).unwrap();
    assert!(!json.contains("\"changed\""), "{json}");

    // The edit itself published.
    assert_ne!(publication.base_revision, publication.resulting_revision);
    let window = all_rows_window(&mut runtime, "grid");
    let row = window
        .rows
        .iter()
        .find(|row| row.id == "r00000")
        .expect("the first row remains");
    match &projected(row, "c0").stored {
        Some(StoredValueProjection::Text { value }) => {
            assert_eq!(value.len(), STORED_TEXT_ITEM_MAX);
        }
        other => panic!("expected the stored text, got {other:?}"),
    }
}

#[test]
fn a_marked_publication_carries_no_changed_cells() {
    let mut runtime =
        DesignerRuntime::from_document_working_size(shared_reference_grid(1_100), OCCURRENCE)
            .unwrap();
    let publication = edited(
        &mut runtime,
        target("r00000", "value"),
        ScalarEditInput::Number { input: "7".into() },
    );
    assert!(publication.many);
    assert_eq!(publication.changed, [] as [FieldProjection; 0]);
    let json = serde_json::to_string(&publication).unwrap();
    assert!(!json.contains("\"changed\""), "{json}");
}

#[test]
fn an_ordinary_edit_carries_no_changed_cells() {
    let mut runtime = DesignerRuntime::from_document(redraw_grid(), OCCURRENCE).unwrap();
    assert_eq!(runtime.profile, SessionProfile::Ordinary);
    let publication = edited(
        &mut runtime,
        target("r00000", "value"),
        ScalarEditInput::Number {
            input: "4.5".into(),
        },
    );
    assert_eq!(publication.changed, [] as [FieldProjection; 0]);
    let json = serde_json::to_string(&DesignerResponse::Published(publication)).unwrap();
    assert!(!json.contains("\"changed\""), "{json}");
}

#[test]
fn a_duplicated_collections_cells_are_changed_cells() {
    let mut runtime =
        DesignerRuntime::from_document_working_size(redraw_grid(), OCCURRENCE).unwrap();
    let reply = runtime
        .handle(DesignerRequest::DuplicateCollection {
            expected_revision: runtime.current_revision().to_owned(),
            collection: "ledger".into(),
            name: "Copy".into(),
            column_names: BTreeMap::new(),
        })
        .unwrap();
    let DesignerResponse::Duplicated(duplicated) = reply else {
        panic!("expected a duplicated collection");
    };
    assert!(!duplicated.publication.many);
    let copy = window_cells(&mut runtime, &duplicated.collection.key);
    let populated = copy.values().filter(|cell| cell.is_some()).count();
    assert!(populated > 0, "the copy has populated cells");
    for (key, cell) in &copy {
        let listed = duplicated
            .publication
            .changed
            .iter()
            .find(|field| (&field.target.entity, &field.target.field) == (&key.0, &key.1));
        if let Some(window_field) = cell {
            assert_eq!(listed, Some(window_field), "{key:?}");
        }
    }
}

#[test]
fn the_wire_reply_carries_changed_cells() {
    let mut runtime =
        Some(DesignerRuntime::from_document_working_size(redraw_grid(), OCCURRENCE).unwrap());
    let request = DesignerRequest::EditCells {
        expected_revision: runtime.as_ref().unwrap().current_revision().to_owned(),
        edits: vec![CellEdit {
            target: target("r00000", "value"),
            input: ScalarEditInput::Number {
                input: "4.5".into(),
            },
        }],
    };
    let reply = process_wire_request(&mut runtime, &serde_json::to_vec(&request).unwrap());
    let text = String::from_utf8(reply).unwrap();
    assert!(text.contains("\"changed\""), "{text}");
    let DesignerWireReply::Ok {
        response: DesignerResponse::Published(publication),
    } = decode_wire(text.as_bytes())
    else {
        panic!("expected a publication reply, got {text}");
    };
    assert!(
        publication
            .changed
            .iter()
            .any(|field| field.target == target("r00000", "value")),
        "{text}"
    );
}

// ---------------------------------------------------------------------------
// Stage 4: whole-column fill and large pastes in one request (#678).
// ---------------------------------------------------------------------------

fn number_columns(count: usize) -> Vec<NewTableColumnInput> {
    (0..count)
        .map(|index| NewTableColumnInput {
            name: format!("c{index}"),
            field_type: "number".to_owned(),
            required: false,
        })
        .collect()
}

/// A fresh working-size table with `count` Number columns and no rows, plus its
/// collection key and the stable field ids in declared order.
fn blank_working_table(occurrence: &str, count: usize) -> (DesignerRuntime, String, Vec<String>) {
    let runtime =
        DesignerRuntime::new_working_size_table(occurrence, "Bulk", &number_columns(count))
            .unwrap();
    let collection = runtime.default_collection.clone();
    let fields = runtime.collection_specs[&collection]
        .columns
        .iter()
        .map(|column| column.id.to_string())
        .collect();
    (runtime, collection, fields)
}

/// A Number grid: every row stores `value`, and row 0's `calc` reads its own
/// row's value by absolute reference, so a relative copy shifts it per row.
fn fill_grid(rows: usize) -> Document {
    let mut document = Document::empty("fill", "Fill");
    let mut schema = Schema {
        field_order: None,
        id: SchemaId::from("grid"),
        key: SchemaKey::from("grid"),
        display_name: None,
        fields: BTreeMap::new(),
    };
    schema
        .fields
        .insert(FieldId::from("value"), number_field("value", None));
    schema
        .fields
        .insert(FieldId::from("calc"), number_field("calc", None));
    document.schemas.insert(schema.id.clone(), schema);
    for row in 0..rows {
        let id = EntityId::from(format!("r{row:05}"));
        let value = f64::from(u32::try_from(row).expect("bounded rows")) + 1.0;
        let mut fields = BTreeMap::from([(FieldId::from("value"), Value::Number(num(value)))]);
        if row == 0 {
            fields.insert(
                FieldId::from("calc"),
                Value::Formula(Expression::Reference(FieldRef::new(
                    EntityId::from("r00000"),
                    FieldId::from("value"),
                ))),
            );
        }
        document.entities.insert(
            id.clone(),
            Entity {
                id: id.clone(),
                key: EntityKey::from(format!("k{row:05}")),
                schema: SchemaId::from("grid"),
                fields,
            },
        );
    }
    document
}

/// Two Number collections, both directly editable, for the lookup tests.
fn two_collection_grid() -> Document {
    let mut document = Document::empty("lookups", "Lookups");
    for collection in ["grid", "mirror"] {
        let mut schema = Schema {
            field_order: None,
            id: SchemaId::from(collection),
            key: SchemaKey::from(collection),
            display_name: None,
            fields: BTreeMap::new(),
        };
        schema
            .fields
            .insert(FieldId::from("value"), number_field("value", None));
        schema
            .fields
            .insert(FieldId::from("calc"), number_field("calc", None));
        document.schemas.insert(schema.id.clone(), schema);
    }
    for row in 0..3 {
        let id = EntityId::from(format!("r{row:05}"));
        let mut fields = BTreeMap::from([(
            FieldId::from("value"),
            Value::Number(num(f64::from(u32::try_from(row).expect("bounded")) + 1.0)),
        )]);
        if row == 0 {
            fields.insert(
                FieldId::from("calc"),
                Value::Formula(Expression::Reference(FieldRef::new(
                    EntityId::from("r00000"),
                    FieldId::from("value"),
                ))),
            );
        }
        document.entities.insert(
            id.clone(),
            Entity {
                id: id.clone(),
                key: EntityKey::from(format!("k{row:05}")),
                schema: SchemaId::from("grid"),
                fields,
            },
        );
    }
    for row in 0..2 {
        let id = EntityId::from(format!("m{row:05}"));
        document.entities.insert(
            id.clone(),
            Entity {
                id: id.clone(),
                key: EntityKey::from(format!("mk{row:05}")),
                schema: SchemaId::from("mirror"),
                fields: BTreeMap::from([(
                    FieldId::from("value"),
                    Value::Number(num(f64::from(u32::try_from(row).expect("bounded")) + 10.0)),
                )]),
            },
        );
    }
    document
}

fn collection_window(
    runtime: &mut DesignerRuntime,
    collection: &str,
    start: usize,
    count: usize,
) -> TableWindowProjection {
    let expected_revision = runtime.current_revision().to_owned();
    match runtime
        .handle(DesignerRequest::QueryTableWindow {
            expected_revision,
            collection: collection.to_owned(),
            view: TableView::default(),
            start,
            count,
            display: true,
        })
        .unwrap()
    {
        DesignerResponse::TableWindow(window) => window,
        other => panic!("expected a table window, got {other:?}"),
    }
}

fn published(response: Result<DesignerResponse, DesignerError>) -> PublicationProjection {
    match response.unwrap_or_else(|error| panic!("request failed: {error:?}")) {
        DesignerResponse::Published(publication) => publication,
        other => panic!("expected a publication, got {other:?}"),
    }
}

fn edit_request(runtime: &DesignerRuntime, edits: Vec<CellEdit>) -> DesignerRequest {
    DesignerRequest::EditCells {
        expected_revision: runtime.current_revision().to_owned(),
        edits,
    }
}

fn cell_edit(entity: &str, field: &str, input: ScalarEditInput) -> CellEdit {
    CellEdit {
        target: target(entity, field),
        input,
    }
}

fn paste_request(
    collection: &str,
    start_field: &str,
    rows: Vec<Vec<PasteCellInput>>,
    expected_revision: &str,
) -> DesignerRequest {
    DesignerRequest::PasteCells {
        expected_revision: expected_revision.to_owned(),
        collection: collection.to_owned(),
        start_entity: None,
        start_field: start_field.to_owned(),
        rows,
    }
}

fn assert_wire_capacity(
    error: &FailureProjection,
    limit: CapacityLimit,
    maximum: usize,
    attempted: usize,
) {
    let capacity = error
        .capacity
        .as_ref()
        .expect("the refusal names its capacity");
    assert_eq!(capacity.limit, limit);
    assert_eq!(capacity.maximum, maximum);
    assert_eq!(capacity.attempted, attempted);
}

fn copy_request(expected_revision: String, destinations: Vec<FieldTarget>) -> DesignerRequest {
    DesignerRequest::CopyFormula {
        expected_revision,
        source: target("r00000", "calc"),
        destinations,
        fixed_references: Vec::new(),
        relative_rows: true,
        relative_columns: false,
    }
}

#[test]
fn a_whole_column_edit_reaches_working_size_and_undo_restores_it() {
    let mut runtime =
        DesignerRuntime::from_document_working_size(text_grid(1_025, 1), OCCURRENCE).unwrap();
    let edits = (0..1_025)
        .map(|row| {
            cell_edit(
                &format!("r{row:05}"),
                "c0",
                ScalarEditInput::Text {
                    value: format!("v{row}"),
                },
            )
        })
        .collect::<Vec<_>>();
    let request = edit_request(&runtime, edits);
    let publication = published(runtime.handle(request));
    assert!(publication.many, "1,025 targets are over the listed bound");
    assert!(
        publication.changed.is_empty(),
        "a marked publication carries no changed cells"
    );

    history(&mut runtime, false);
    let snapshot = runtime.session.export_snapshot();
    assert!(
        snapshot
            .document()
            .entities
            .values()
            .all(|entity| entity.fields.is_empty()),
        "undo restores every edited cell to blank"
    );
}

#[test]
fn an_ordinary_session_refuses_a_1_025_cell_edit() {
    let mut ordinary = DesignerRuntime::tracker(OTHER_OCCURRENCE).unwrap();
    let edits = (0..1_025)
        .map(|row| cell_edit(&format!("r{row:05}"), "c0", ScalarEditInput::Clear))
        .collect::<Vec<_>>();
    let request = edit_request(&ordinary, edits);
    assert_tracker_message(
        &expect_error(ordinary.handle(request)),
        "the edit range is empty or too large",
    );
}

#[test]
fn an_edit_past_the_working_cell_limit_refuses_and_changes_nothing() {
    let mut runtime =
        DesignerRuntime::from_document_working_size(text_grid(1, 1), OCCURRENCE).unwrap();
    let before = revision(&runtime);
    let edits = (0..100_001)
        .map(|row| cell_edit(&format!("r{row:05}"), "c0", ScalarEditInput::Clear))
        .collect::<Vec<_>>();
    let request = DesignerRequest::EditCells {
        expected_revision: before.clone(),
        edits,
    };
    assert_tracker_message(
        &expect_error(runtime.handle(request)),
        "the edit range is empty or too large",
    );
    assert_eq!(revision(&runtime), before);
}

#[test]
fn a_whole_column_copy_reaches_working_size_and_undo_restores_it() {
    let mut runtime =
        DesignerRuntime::from_document_working_size(fill_grid(1_026), OCCURRENCE).unwrap();
    let request = DesignerRequest::CopyFormula {
        expected_revision: revision(&runtime),
        source: target("r00000", "calc"),
        destinations: (1..=1_025)
            .map(|row| target(&format!("r{row:05}"), "calc"))
            .collect(),
        fixed_references: Vec::new(),
        relative_rows: true,
        relative_columns: false,
    };
    let publication = published(runtime.handle(request));
    assert!(publication.many, "1,025 targets are over the listed bound");

    let window = collection_window(&mut runtime, "grid", 0, 256);
    assert_eq!(window.rows.len(), 256);
    for row in &window.rows {
        let Some(StoredValueProjection::Number { value }) = &projected(row, "value").stored else {
            panic!("row {} has no stored value", row.id);
        };
        assert_eq!(
            projected(row, "calc")
                .calculated
                .as_ref()
                .and_then(CalculationProjection::number),
            Some(*value),
            "row {} must carry its own shifted formula result",
            row.id
        );
    }
    let tail = collection_window(&mut runtime, "grid", 1_024, 2);
    assert_eq!(tail.rows.len(), 2);
    for row in &tail.rows {
        let Some(StoredValueProjection::Number { value }) = &projected(row, "value").stored else {
            panic!("row {} has no stored value", row.id);
        };
        assert_eq!(
            projected(row, "calc")
                .calculated
                .as_ref()
                .and_then(CalculationProjection::number),
            Some(*value),
            "row {} must carry its own shifted formula result",
            row.id
        );
    }

    history(&mut runtime, false);
    let snapshot = runtime.session.export_snapshot();
    for row in 1..=1_025 {
        let entity = &snapshot.document().entities[&EntityId::from(format!("r{row:05}"))];
        assert!(
            !entity.fields.contains_key(&FieldId::from("calc")),
            "undo clears the copied formula in row {row}"
        );
    }
}

#[test]
fn an_ordinary_session_refuses_a_1_025_destination_copy() {
    let mut ordinary = DesignerRuntime::tracker(OTHER_OCCURRENCE).unwrap();
    let request = DesignerRequest::CopyFormula {
        expected_revision: revision(&ordinary),
        source: target("r00000", "calc"),
        destinations: (1..=1_025)
            .map(|row| target(&format!("r{row:05}"), "calc"))
            .collect(),
        fixed_references: Vec::new(),
        relative_rows: true,
        relative_columns: false,
    };
    assert_tracker_message(
        &expect_error(ordinary.handle(request)),
        "formula copy range is empty or too large",
    );
}

#[test]
fn fill_lookups_keep_their_messages_and_targets() {
    let mut runtime =
        DesignerRuntime::from_document_working_size(two_collection_grid(), OCCURRENCE).unwrap();

    let duplicate = edit_request(
        &runtime,
        vec![
            cell_edit(
                "r00000",
                "value",
                ScalarEditInput::Number { input: "9".into() },
            ),
            cell_edit(
                "r00000",
                "value",
                ScalarEditInput::Number { input: "9".into() },
            ),
        ],
    );
    assert_tracker_message(
        &expect_error(runtime.handle(duplicate)),
        "duplicate cell targets are unsupported",
    );

    let unknown_entity = edit_request(
        &runtime,
        vec![cell_edit(
            "nope",
            "value",
            ScalarEditInput::Number { input: "9".into() },
        )],
    );
    assert_scalar_edit_message(
        &expect_error(runtime.handle(unknown_entity)),
        "field 'nope.value' is not editable through this directly stored scalar path",
    );

    let unknown_column = edit_request(
        &runtime,
        vec![cell_edit(
            "r00000",
            "nope",
            ScalarEditInput::Number { input: "9".into() },
        )],
    );
    assert_scalar_edit_message(
        &expect_error(runtime.handle(unknown_column)),
        "field 'r00000.nope' is not editable through this directly stored scalar path",
    );

    // A target in a second collection still edits that collection.
    let second_collection = edit_request(
        &runtime,
        vec![cell_edit(
            "m00000",
            "value",
            ScalarEditInput::Number { input: "12".into() },
        )],
    );
    published(runtime.handle(second_collection));
    let mirror = window_cells(&mut runtime, "mirror");
    let Some(Some(field)) = mirror.get(&("m00000".to_owned(), "value".to_owned())) else {
        panic!("the mirror cell is missing");
    };
    let Some(StoredValueProjection::Number { value }) = &field.stored else {
        panic!("the mirror cell is not a stored Number");
    };
    assert_eq!(*value, 12.0);
    let grid = window_cells(&mut runtime, "grid");
    let Some(Some(field)) = grid.get(&("r00000".to_owned(), "value".to_owned())) else {
        panic!("the grid cell is missing");
    };
    let Some(StoredValueProjection::Number { value }) = &field.stored else {
        panic!("the grid cell is not a stored Number");
    };
    assert_eq!(*value, 1.0, "the other collection is untouched");

    assert_tracker_message(
        &expect_error(runtime.handle(copy_request(
            revision(&runtime),
            vec![target("r00001", "calc"), target("r00001", "calc")],
        ))),
        "duplicate formula copy targets are unsupported",
    );
    assert_tracker_message(
        &expect_error(runtime.handle(copy_request(
            revision(&runtime),
            vec![target("nope", "calc")],
        ))),
        "formula copy target must belong to the source collection",
    );
    assert_tracker_message(
        &expect_error(runtime.handle(copy_request(
            revision(&runtime),
            vec![target("r00001", "nope")],
        ))),
        "formula copy target must belong to the source collection",
    );
    assert_tracker_message(
        &expect_error(runtime.handle(copy_request(
            revision(&runtime),
            vec![target("m00000", "value")],
        ))),
        "formula copy target must belong to the source collection",
    );
}

#[test]
fn a_large_paste_reaches_working_size_through_the_wire() {
    let (runtime, collection, fields) = blank_working_table(OCCURRENCE, 5);
    let mut resident = Some(runtime);
    let rows = (0..2_000)
        .map(|row| {
            (0..5)
                .map(|column| PasteCellInput::Text(format!("{}", 1_000_000 + row * 5 + column)))
                .collect()
        })
        .collect::<Vec<Vec<PasteCellInput>>>();
    let request = paste_request(
        &collection,
        &fields[0],
        rows,
        resident.as_ref().unwrap().current_revision(),
    );
    let input = serde_json::to_vec(&request).unwrap();
    assert!(
        input.len() > 65_536,
        "the paste request is only {} bytes",
        input.len()
    );
    let reply = decode_wire(&process_wire_request(&mut resident, &input));
    let DesignerWireReply::Ok {
        response: DesignerResponse::Published(publication),
    } = reply
    else {
        panic!("a working-size paste publishes, got {reply:?}");
    };
    assert!(
        publication.many,
        "2,000 created rows are over the listed bound"
    );

    let mut runtime = resident.unwrap();
    let window = collection_window(&mut runtime, &collection, 0, 256);
    assert_eq!(window.total_rows, 2_000);
    assert_eq!(window.rows.len(), 256);
    let first = &window.rows[0];
    for (column, field) in fields.iter().enumerate() {
        let Some(StoredValueProjection::Number { value }) = &projected(first, field).stored else {
            panic!("the first pasted row has no {field} value");
        };
        assert_eq!(
            *value,
            1_000_000.0 + f64::from(u32::try_from(column).expect("bounded columns"))
        );
    }
    assert!(
        window
            .rows
            .iter()
            .all(|row| row.fields.iter().all(|field| field.stored.is_some())),
        "every pasted row carries every pasted value"
    );

    let before_undo = revision(&runtime);
    history(&mut runtime, false);
    assert_ne!(revision(&runtime), before_undo);
    let emptied = collection_window(&mut runtime, &collection, 0, 256);
    assert_eq!(emptied.total_rows, 0);
    assert!(emptied.rows.is_empty(), "undo empties the pasted table");
}

#[test]
fn an_ordinary_session_refuses_a_working_size_paste() {
    let rows = (0..2_000)
        .map(|row| {
            (0..5)
                .map(|column| PasteCellInput::Text(format!("{}", 1_000_000 + row * 5 + column)))
                .collect()
        })
        .collect::<Vec<Vec<PasteCellInput>>>();
    let request = paste_request("tracker", "task", rows, "resident/0");
    let input = serde_json::to_vec(&request).unwrap();
    assert!(input.len() > 65_536);
    let mut ordinary = Some(DesignerRuntime::tracker(OTHER_OCCURRENCE).unwrap());
    let reply = decode_wire(&process_wire_request(&mut ordinary, &input));
    let DesignerWireReply::Error { error } = reply else {
        panic!("an ordinary session refuses the paste, got {reply:?}");
    };
    assert_eq!(error.code, "request_too_large");
    assert_eq!(
        error.message,
        "The Designer request exceeds the private 65536-byte bridge limit."
    );
}

#[test]
fn the_working_wire_limit_names_itself_and_changes_nothing() {
    let mut resident =
        Some(DesignerRuntime::from_document_working_size(text_grid(1, 1), OCCURRENCE).unwrap());
    let before = revision(resident.as_ref().unwrap());
    let input = vec![b'x'; super::MAX_WORKING_WIRE_REQUEST_BYTES + 1];
    let reply = decode_wire(&process_wire_request(&mut resident, &input));
    let DesignerWireReply::Error { error } = reply else {
        panic!("a request over the working wire limit refuses, got {reply:?}");
    };
    assert_eq!(error.code, "request_too_large");
    assert!(
        error.message.contains("67108864"),
        "unexpected message: {}",
        error.message
    );
    assert_eq!(revision(resident.as_ref().unwrap()), before);
}

#[test]
fn a_paste_past_the_working_limits_refuses_and_changes_nothing() {
    let (runtime, collection, fields) = blank_working_table(OCCURRENCE, 1);
    let mut resident = Some(runtime);
    let before = revision(resident.as_ref().unwrap());
    let rows = (0..10_001)
        .map(|_| vec![PasteCellInput::Text("1".to_owned())])
        .collect::<Vec<Vec<PasteCellInput>>>();
    let request = paste_request(
        &collection,
        &fields[0],
        rows,
        resident.as_ref().unwrap().current_revision(),
    );
    let reply = decode_wire(&process_wire_request(
        &mut resident,
        &serde_json::to_vec(&request).unwrap(),
    ));
    let DesignerWireReply::Error { error } = reply else {
        panic!("a paste past the row limit refuses, got {reply:?}");
    };
    assert_wire_capacity(&error, CapacityLimit::TableRows, 10_000, 10_001);
    assert_eq!(revision(resident.as_ref().unwrap()), before);

    let (runtime, collection, fields) = blank_working_table(OTHER_OCCURRENCE, 11);
    let mut resident = Some(runtime);
    let before = revision(resident.as_ref().unwrap());
    let rows = (0..9_091)
        .map(|_| {
            (0..11)
                .map(|_| PasteCellInput::Text("1".to_owned()))
                .collect()
        })
        .collect::<Vec<Vec<PasteCellInput>>>();
    let request = paste_request(
        &collection,
        &fields[0],
        rows,
        resident.as_ref().unwrap().current_revision(),
    );
    let reply = decode_wire(&process_wire_request(
        &mut resident,
        &serde_json::to_vec(&request).unwrap(),
    ));
    let DesignerWireReply::Error { error } = reply else {
        panic!("a paste past the cell limit refuses, got {reply:?}");
    };
    assert_wire_capacity(&error, CapacityLimit::PopulatedCells, 100_000, 100_001);
    assert_eq!(revision(resident.as_ref().unwrap()), before);
}

// ---------------------------------------------------------------------------
// Saved-size bound (#678 stage 6)
// ---------------------------------------------------------------------------

fn empty_presentation() -> PresentationStore {
    PresentationStore::default()
}

fn bound_of(document: &Document, presentation: &PresentationStore) -> usize {
    super::save_size::saved_bytes_upper_bound(document)
        + super::save_size::presentation_saved_bytes(presentation).unwrap()
}

fn real_saves(document: &Document, presentation: &PresentationStore) -> (usize, usize) {
    let tree = encode_roproj_v3_with_presentation(document, presentation).unwrap();
    let bundle =
        encode_project_bundle(tree.files().iter().map(|file| (file.path(), file.bytes()))).unwrap();
    let portable = encode_portable_package_v1(&tree).unwrap();
    (bundle.len(), portable.len())
}

fn assert_bound_covers(
    name: &str,
    document: &Document,
    presentation: &PresentationStore,
) -> usize {
    let bound = bound_of(document, presentation);
    let (bundle, portable) = real_saves(document, presentation);
    let parts = super::save_size::saved_bytes_parts(document);
    let tree = encode_roproj_v3_with_presentation(document, presentation).unwrap();
    let files = tree
        .files()
        .iter()
        .map(|file| (file.path(), file.bytes().len()))
        .collect::<Vec<_>>();
    eprintln!("PARTS|{name}|{parts:?}|files={files:?}");
    assert!(bound >= bundle, "{name}: bound {bound} < project save {bundle}");
    assert!(
        bound >= portable,
        "{name}: bound {bound} < portable save {portable}"
    );
    eprintln!("SAVED|{name}|bound={bound}|project={bundle}|portable={portable}");
    bound
}

fn add_field(
    fields: &mut BTreeMap<FieldId, FieldDefinition>,
    id: &str,
    field_type: FieldType,
    display_name: Option<&str>,
    constraint: FieldConstraint,
    number_format: Option<NumberFormat>,
) {
    let field_id = FieldId::from(id);
    fields.insert(
        field_id.clone(),
        FieldDefinition {
            id: field_id,
            key: FieldKey::from(id),
            display_name: display_name.map(str::to_owned),
            field_type,
            required: false,
            constraint,
            number_format,
        },
    );
}

fn values_grid() -> Document {
    let mut document = Document::empty("working-size-values", "Working Size Values");
    let mut refs_fields = BTreeMap::new();
    add_field(
        &mut refs_fields,
        "name",
        FieldType::Text,
        None,
        FieldConstraint::None,
        None,
    );
    document.schemas.insert(
        SchemaId::from("refs"),
        Schema {
            field_order: None,
            id: SchemaId::from("refs"),
            key: SchemaKey::from("refs"),
            display_name: None,
            fields: refs_fields,
        },
    );
    document.entities.insert(
        EntityId::from("t00000"),
        Entity {
            id: EntityId::from("t00000"),
            key: EntityKey::from("kt00000"),
            schema: SchemaId::from("refs"),
            fields: BTreeMap::from([(
                FieldId::from("name"),
                Value::Text("target".to_owned()),
            )]),
        },
    );
    let mut fields = BTreeMap::new();
    add_field(&mut fields, "n", FieldType::Number, None, FieldConstraint::None, None);
    add_field(&mut fields, "t", FieldType::Text, None, FieldConstraint::None, None);
    add_field(&mut fields, "b", FieldType::Boolean, None, FieldConstraint::None, None);
    add_field(&mut fields, "d", FieldType::Date, None, FieldConstraint::None, None);
    add_field(
        &mut fields,
        "r",
        FieldType::Reference {
            schema: SchemaId::from("refs"),
        },
        None,
        FieldConstraint::None,
        None,
    );
    document.schemas.insert(
        SchemaId::from("grid"),
        Schema {
            field_order: None,
            id: SchemaId::from("grid"),
            key: SchemaKey::from("grid"),
            display_name: None,
            fields,
        },
    );
    let mut row = BTreeMap::new();
    row.insert(FieldId::from("n"), Value::Number(Number::new(12.5).unwrap()));
    row.insert(FieldId::from("t"), Value::Text("value".to_owned()));
    row.insert(FieldId::from("b"), Value::Boolean(true));
    row.insert(
        FieldId::from("d"),
        Value::Date(Date::new(2026, 11, 8).unwrap()),
    );
    row.insert(FieldId::from("r"), Value::Reference(EntityId::from("t00000")));
    for index in 0..3 {
        let id = EntityId::from(format!("r{index:05}"));
        document.entities.insert(
            id.clone(),
            Entity {
                id: id.clone(),
                key: EntityKey::from(format!("k{index:05}")),
                schema: SchemaId::from("grid"),
                fields: if index == 0 { row.clone() } else { BTreeMap::new() },
            },
        );
    }
    document
}

fn escape_text_grid() -> Document {
    let mut document = text_grid(1, 1);
    document
        .entities
        .get_mut(&EntityId::from("r00000"))
        .unwrap()
        .fields
        .insert(
            FieldId::from("c0"),
            Value::Text("\"\\\u{8}\u{9}\u{a}\u{c}\u{d}\u{0}\u{1}\u{1f}é中🙂".to_owned()),
        );
    document
}

fn long_identity_grid() -> Document {
    let long = "a".repeat(4096);
    let mut document = Document::empty("working-size-identities", "Working Size Identities");
    let schema_id = SchemaId::from(long.clone());
    let field_id = FieldId::from(long.clone());
    let mut fields = BTreeMap::new();
    fields.insert(
        field_id.clone(),
        FieldDefinition {
            id: field_id.clone(),
            key: FieldKey::from(long.clone()),
            display_name: Some(long.clone()),
            field_type: FieldType::Text,
            required: false,
            constraint: FieldConstraint::None,
            number_format: None,
        },
    );
    document.schemas.insert(
        schema_id.clone(),
        Schema {
            field_order: Some(vec![field_id.clone()]),
            id: schema_id.clone(),
            key: SchemaKey::from(long.clone()),
            display_name: Some(long.clone()),
            fields,
        },
    );
    let entity_id = EntityId::from(long.clone());
    document.entities.insert(
        entity_id.clone(),
        Entity {
            id: entity_id,
            key: EntityKey::from(long.clone()),
            schema: schema_id,
            fields: BTreeMap::from([(field_id, Value::Text(long))]),
        },
    );
    document
}

fn formula_grid() -> Document {
    let mut document = text_grid(21, 2);
    {
        let schema = document.schemas.get_mut(&SchemaId::from("grid")).unwrap();
        for field in ["c0", "c1"] {
            schema
                .fields
                .get_mut(&FieldId::from(field))
                .unwrap()
                .field_type = FieldType::Number;
        }
    }
    let column = || ColumnRef {
        schema: SchemaId::from("grid"),
        field: FieldId::from("c0"),
    };
    let number = || Expression::Number(Number::new(1.0).unwrap());
    let criterion = |value: Option<Expression>| ColumnCriterion {
        column: column(),
        operator: ComparisonOperator::Equal,
        value: value.map(Box::new),
    };
    let comparison = || Expression::Compare {
        operator: ComparisonOperator::Less,
        left: Box::new(number()),
        right: Box::new(number()),
    };
    let expressions = vec![
        number(),
        Expression::Reference(FieldRef::new("r00000", "c0")),
        Expression::SameRowReference(FieldId::from("c0")),
        Expression::ColumnTotal(column()),
        Expression::ColumnAggregate {
            function: ColumnAggregateFunction::Average,
            column: column(),
        },
        Expression::ColumnConditional {
            aggregate: Box::new(ConditionalAggregate::Sum(column())),
            criteria: vec![criterion(Some(number()))],
        },
        Expression::ColumnConditional {
            aggregate: Box::new(ConditionalAggregate::Average(column())),
            criteria: vec![criterion(None)],
        },
        Expression::ColumnConditional {
            aggregate: Box::new(ConditionalAggregate::Count),
            criteria: vec![criterion(Some(number()))],
        },
        Expression::Add {
            left: Box::new(number()),
            right: Box::new(number()),
        },
        Expression::Subtract {
            left: Box::new(number()),
            right: Box::new(number()),
        },
        Expression::Multiply {
            left: Box::new(number()),
            right: Box::new(number()),
        },
        Expression::Divide {
            left: Box::new(number()),
            right: Box::new(number()),
        },
        Expression::Minimum {
            left: Box::new(number()),
            right: Box::new(number()),
        },
        Expression::Maximum {
            left: Box::new(number()),
            right: Box::new(number()),
        },
        comparison(),
        Expression::If {
            condition: Box::new(comparison()),
            then_value: Box::new(number()),
            else_value: Box::new(number()),
        },
        Expression::And(vec![comparison(), comparison()]),
        Expression::Or(vec![comparison(), comparison()]),
        Expression::Not(Box::new(comparison())),
        Expression::IsBlank(Box::new(Expression::SameRowReference(FieldId::from("c0")))),
        Expression::Round {
            value: Box::new(number()),
            digits: 2,
        },
    ];
    for (index, expression) in expressions.into_iter().enumerate() {
        document
            .entities
            .get_mut(&EntityId::from(format!("r{index:05}")))
            .unwrap()
            .fields
            .insert(FieldId::from("c1"), Value::Formula(expression));
    }
    document
}

fn formats_fixture() -> (Document, PresentationStore) {
    let mut document = Document::empty("working-size-formats", "Working Size Formats");
    let mut fields = BTreeMap::new();
    add_field(
        &mut fields,
        "amount",
        FieldType::Number,
        Some("Amount"),
        FieldConstraint::NumberInclusiveRange {
            min: Number::new(-1.5).unwrap(),
            max: Number::new(1_000_000.0).unwrap(),
        },
        Some(NumberFormat {
            decimals: 2,
            kind: NumberFormatKind::Currency {
                code: "USD".to_owned(),
            },
            thousands_separator: true,
        }),
    );
    add_field(
        &mut fields,
        "ratio",
        FieldType::Number,
        None,
        FieldConstraint::None,
        Some(NumberFormat {
            decimals: 0,
            kind: NumberFormatKind::Percent,
            thousands_separator: false,
        }),
    );
    add_field(
        &mut fields,
        "label",
        FieldType::Text,
        None,
        FieldConstraint::TextLiteralSet {
            values: vec!["a\"b".to_owned(), "é\n".to_owned()],
        },
        None,
    );
    add_field(&mut fields, "flag", FieldType::Boolean, None, FieldConstraint::None, None);
    add_field(&mut fields, "day", FieldType::Date, None, FieldConstraint::None, None);
    let field_order = ["amount", "ratio", "label", "flag", "day"]
        .map(FieldId::from)
        .to_vec();
    document.schemas.insert(
        SchemaId::from("entries"),
        Schema {
            field_order: Some(field_order),
            id: SchemaId::from("entries"),
            key: SchemaKey::from("entries"),
            display_name: Some("Entries".to_owned()),
            fields,
        },
    );
    let mut products = BTreeMap::new();
    add_field(&mut products, "key", FieldType::Text, None, FieldConstraint::None, None);
    add_field(&mut products, "category", FieldType::Text, None, FieldConstraint::None, None);
    add_field(
        &mut products,
        "price",
        FieldType::Number,
        None,
        FieldConstraint::None,
        Some(NumberFormat {
            decimals: 2,
            kind: NumberFormatKind::Plain,
            thousands_separator: false,
        }),
    );
    document.schemas.insert(
        SchemaId::from("products"),
        Schema {
            field_order: None,
            id: SchemaId::from("products"),
            key: SchemaKey::from("products"),
            display_name: None,
            fields: products,
        },
    );
    document
        .entities
        .insert(
            EntityId::from("e00000"),
            Entity {
                id: EntityId::from("e00000"),
                key: EntityKey::from("ke00000"),
                schema: SchemaId::from("entries"),
                fields: BTreeMap::from([
                    (FieldId::from("amount"), Value::Number(Number::new(12.5).unwrap())),
                    (FieldId::from("ratio"), Value::Number(Number::new(0.25).unwrap())),
                    (FieldId::from("label"), Value::Text("a\"b".to_owned())),
                    (FieldId::from("flag"), Value::Boolean(false)),
                    (FieldId::from("day"), Value::Date(Date::new(2026, 11, 8).unwrap())),
                ]),
            },
        );
    let definition_id = KeyedGroupedSumDefinitionId::from("definition");
    document.keyed_grouped_sum_definitions.insert(
        definition_id.clone(),
        KeyedGroupedSumDefinition {
            id: definition_id,
            orders: KeyedGroupedSumOrdersBinding {
                schema: SchemaId::from("entries"),
                lookup_key_field: FieldId::from("label"),
                quantity_field: FieldId::from("amount"),
            },
            products: KeyedGroupedSumProductsBinding {
                schema: SchemaId::from("products"),
                key_field: FieldId::from("key"),
                category_field: FieldId::from("category"),
                price_field: FieldId::from("price"),
            },
        },
    );
    let style = CellStyle {
        bold: Some(true),
        italic: None,
        fill: Some("#FFEEAA".to_owned()),
        text_color: None,
        alignment: None,
        wrap: None,
        borders: None,
        number_format: None,
        date_format: None,
    };
    let style_id = style_id(&style).unwrap();
    let presentation = PresentationStore {
        styles: BTreeMap::from([(style_id.clone(), style)]),
        tables: BTreeMap::from([(
            SchemaId::from("entries"),
            TablePresentation {
                title: Some("Formatted".to_owned()),
                footer: BTreeMap::from([(FieldId::from("amount"), FooterAggregate::Sum)]),
                column_styles: BTreeMap::from([(FieldId::from("ratio"), style_id.clone())]),
            },
        )]),
        rules: BTreeMap::new(),
        cells: BTreeMap::from([((EntityId::from("e00000"), FieldId::from("amount")), style_id)]),
    };
    (document, presentation)
}

fn bound_fixtures() -> Vec<(&'static str, Document, PresentationStore)> {
    let (formats, presentation) = formats_fixture();
    vec![
        ("values", values_grid(), empty_presentation()),
        ("formula_grid", formula_grid(), empty_presentation()),
        ("escapes", escape_text_grid(), empty_presentation()),
        ("identities", long_identity_grid(), empty_presentation()),
        ("formats", formats, presentation),
        ("qualification", fixture(), empty_presentation()),
    ]
}

#[test]
fn the_saved_bound_never_shortens_a_real_save() {
    for (name, document, presentation) in bound_fixtures() {
        assert_bound_covers(name, &document, &presentation);
    }
}

#[test]
fn the_saved_bound_is_tight_on_realistic_documents() {
    for (name, document, presentation) in bound_fixtures() {
        let bound = assert_bound_covers(name, &document, &presentation);
        let (bundle, portable) = real_saves(&document, &presentation);
        let real = bundle.max(portable);
        assert!(
            bound <= real * 105 / 100 + 64 * 1024,
            "{name}: bound {bound} is not tight against real save {real}"
        );
    }
}

fn fill_control_cells(document: &mut Document, cells: usize) {
    let text = "\u{1}".repeat(STORED_TEXT_ITEM_MAX);
    let mut filled = 0;
    for row in 0..6 {
        for column in 0..28 {
            if filled == cells {
                return;
            }
            document
                .entities
                .get_mut(&EntityId::from(format!("r{row:05}")))
                .unwrap()
                .fields
                .insert(
                    FieldId::from(format!("c{column}")),
                    Value::Text(text.clone()),
                );
            filled += 1;
        }
    }
}

fn crossing_documents() -> (Document, Document, String) {
    let mut before = text_grid(6, 28);
    fill_control_cells(&mut before, 167);
    let mut after = before.clone();
    after
        .entities
        .get_mut(&EntityId::from("r00005"))
        .unwrap()
        .fields
        .insert(
            FieldId::from("c27"),
            Value::Text("\u{1}".repeat(STORED_TEXT_ITEM_MAX)),
        );
    let pasted = "\u{1}".repeat(STORED_TEXT_ITEM_MAX);
    (before, after, pasted)
}

#[test]
fn an_accepted_working_size_edit_can_always_be_saved() {
    let (before, after, pasted) = crossing_documents();
    let before_bound = bound_of(&before, &empty_presentation());
    let after_bound = bound_of(&after, &empty_presentation());
    assert!(before_bound <= super::capacity::MAX_WORKING_SAVED_BYTES);
    assert!(after_bound > super::capacity::MAX_WORKING_SAVED_BYTES);

    let mut runtime = DesignerRuntime::from_document_working_size(before, OCCURRENCE).unwrap();
    let current = revision(&runtime);
    let project = runtime.export_project_v3(&current).unwrap().bytes;
    runtime.export_portable_ro(&current).unwrap();
    let mut reopened: Option<DesignerRuntime> = None;
    open_project_working_size(&mut reopened, &project, OCCURRENCE).unwrap();
    assert_eq!(revision(reopened.as_ref().unwrap()), current);

    let request = DesignerRequest::PasteCells {
        expected_revision: current.clone(),
        collection: "grid".to_owned(),
        start_entity: Some("r00005".to_owned()),
        start_field: "c27".to_owned(),
        rows: vec![vec![PasteCellInput::Text(pasted)]],
    };
    let error = runtime.handle(request.clone()).unwrap_err();
    assert_capacity_refusal(
        &error,
        CapacityLimit::SavedBytes,
        super::capacity::MAX_WORKING_SAVED_BYTES,
        after_bound,
    );
    assert_eq!(revision(&runtime), current);

    // The same refusal over the wire names the limit for the typed client.
    let mut wire = Some(runtime);
    let reply = decode_wire(&process_wire_request(
        &mut wire,
        &serde_json::to_vec(&request).unwrap(),
    ));
    let DesignerWireReply::Error { error } = reply else {
        panic!("a crossing paste refuses, got {reply:?}");
    };
    assert_wire_capacity(
        &error,
        CapacityLimit::SavedBytes,
        super::capacity::MAX_WORKING_SAVED_BYTES,
        after_bound,
    );
    assert!(serde_json::to_string(&error).unwrap().contains("\"saved_bytes\""));
}
