//! Acceptance tests for the working-size session profile (#678, stage 2).
#![allow(clippy::too_many_lines)] // Each test walks one whole acceptance case.

use std::collections::{BTreeMap, BTreeSet};

use tachiko_storage::{encode_roproj_v3, encode_roproj_v3_with_presentation};
use tachiko_workspace_engine::{
    ColumnRef, Expression, FieldConstraint, Number, NumberFormat, NumberFormatKind,
    PresentationStore, TablePresentation, Value,
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
    FieldDefinition, FieldId, FieldKey, FieldProjection, FieldRef, FieldTarget, FieldType,
    NewTableColumnInput, OpenedWindowProjection, PublicationProjection, RowInitializer,
    RowProjection, ScalarEditInput, Schema, SchemaId, SchemaKey, StoredValueProjection, TableView,
    TableWindowProjection, encode_portable_package_v1, encode_project_bundle, open_local_document,
    open_local_document_working_size, open_portable_ro, open_portable_ro_working_size,
    open_project, open_project_working_size, process_wire_request, to_canonical_string,
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
        .unwrap()
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
            (
                FieldId::from("label"),
                Value::Text(format!("row-{row}")),
            ),
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
fn assert_changed_covers(before: &WindowCells, after: &WindowCells, publication: &PublicationProjection) {
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
        ScalarEditInput::Number { input: "4.5".into() },
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
    let publication = formula_update(
        &mut runtime,
        target("r00001", "first"),
        "[.value] * 3",
    );
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
        ScalarEditInput::Number { input: "4.5".into() },
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

#[test]
fn a_failed_formula_dependent_carries_its_failure() {
    let mut runtime =
        DesignerRuntime::from_document_working_size(redraw_grid(), OCCURRENCE).unwrap();
    let publication = edited(
        &mut runtime,
        target("r00000", "divisor"),
        ScalarEditInput::Number { input: "0".into() },
    );
    let second = publication
        .changed
        .iter()
        .find(|field| field.target == target("r00000", "second"))
        .expect("the dependent formula is a changed cell");
    let Some(CalculationProjection::Failure { code, .. }) = &second.calculated else {
        panic!("expected a failed calculation, got {:?}", second.calculated);
    };
    assert_eq!(code, "division_by_zero");
    assert!(
        !second.diagnostics.is_empty(),
        "a failed formula carries diagnostics"
    );

    // The window shows exactly the same field.
    let window = all_rows_window(&mut runtime, "ledger");
    let window_field = window
        .rows
        .iter()
        .find(|row| row.id == "r00000")
        .and_then(|row| row.fields.iter().find(|field| field.target.field == "second"))
        .expect("the window carries the formula");
    assert_eq!(second, window_field);
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
    assert!(publication.entities.contains(&"r00002".to_owned()));
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
        Some(StoredValueProjection::Number { value }) if value == 9.0
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
    assert!(publication.entities.is_empty());
    assert!(publication.fields.is_empty());
    assert!(publication.affected_calculations.is_empty());
    assert!(publication.changed.is_empty());
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
    assert!(publication.changed.is_empty());
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
        ScalarEditInput::Number { input: "4.5".into() },
    );
    assert!(publication.changed.is_empty());
    let json = serde_json::to_string(&DesignerResponse::Published(publication)).unwrap();
    assert!(!json.contains("\"changed\""), "{json}");
}

#[test]
fn the_wire_reply_carries_changed_cells() {
    let mut runtime = Some(
        DesignerRuntime::from_document_working_size(redraw_grid(), OCCURRENCE).unwrap(),
    );
    let request = DesignerRequest::EditCells {
        expected_revision: runtime.as_ref().unwrap().current_revision().to_owned(),
        edits: vec![CellEdit {
            target: target("r00000", "value"),
            input: ScalarEditInput::Number { input: "4.5".into() },
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
