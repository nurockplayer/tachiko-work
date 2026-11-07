//! Acceptance tests for the working-size session profile (#678, stage 2).
#![allow(clippy::too_many_lines)] // Each test walks one whole acceptance case.

use std::collections::BTreeMap;

use tachiko_storage::{encode_roproj_v3, encode_roproj_v3_with_presentation};
use tachiko_workspace_engine::{
    Expression, FieldConstraint, Number, PresentationStore, TablePresentation, Value,
};

use super::capacity::{CapacityExceeded, CapacityLimit, SessionProfile};
use super::capacity_fixtures::{CALC_FIELD, FixtureSpec, capacity_document, entity_id};
use super::interop_document::CleanupOperation;
use super::{
    CalculationProjection, CellEdit, DesignerError, DesignerProfile, DesignerRequest,
    DesignerResponse, DesignerRuntime, DesignerWireReply, Document, Entity, EntityId, EntityKey,
    FieldDefinition, FieldId, FieldKey, FieldProjection, FieldTarget, FieldType,
    NewTableColumnInput, OpenedWindowProjection, RowInitializer, RowProjection, ScalarEditInput,
    Schema, SchemaId, SchemaKey, StoredValueProjection, encode_portable_package_v1,
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

    let too_large = capacity_document(FixtureSpec {
        rows: 129,
        columns: 28,
        formula_cells: 128,
        seed: 678,
    });
    let bundle = project_bundle(&too_large);
    let raw = raw_document(&too_large);
    let portable = portable_ro(&too_large);

    for error in [
        open_project_working_size(&mut resident, &bundle, OCCURRENCE).unwrap_err(),
        open_local_document_working_size(&mut resident, &raw, OCCURRENCE).unwrap_err(),
        open_portable_ro_working_size(&mut resident, &portable, OCCURRENCE).unwrap_err(),
    ] {
        assert_capacity_refusal(&error, CapacityLimit::TableRows, 128, 129);
    }

    let resident = resident.unwrap();
    assert_eq!(resident.current_revision(), "resident/0");
    assert_eq!(resident.session.export_snapshot().document(), &before);
    assert_eq!(resident.profile, SessionProfile::WorkingSize);
}

#[test]
fn new_working_size_table_keeps_its_profile_up_to_todays_row_limit() {
    let columns = [NewTableColumnInput {
        name: "amount".into(),
        field_type: "number".into(),
        required: true,
    }];
    let mut runtime =
        DesignerRuntime::new_working_size_table(OCCURRENCE, "Growth", &columns).unwrap();
    let collection = runtime.collection_specs.keys().next().unwrap().clone();
    let column = runtime.query_table(&collection).unwrap().columns[0]
        .id
        .clone();
    let initializers = [RowInitializer {
        field: column,
        input: ScalarEditInput::Number { input: "1".into() },
    }];
    for _ in 0..128 {
        let current = revision(&runtime);
        runtime
            .insert_row(&current, &collection, &initializers)
            .unwrap();
    }
    assert_eq!(runtime.profile, SessionProfile::WorkingSize);
    let current = revision(&runtime);
    let error = runtime
        .insert_row(&current, &collection, &initializers)
        .unwrap_err();
    assert_capacity_refusal(&error, CapacityLimit::TableRows, 128, 129);
    assert_eq!(runtime.current_revision(), current);
    assert_eq!(runtime.profile, SessionProfile::WorkingSize);
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
    let mut full_runtime = DesignerRuntime::from_document_working_size(document, OCCURRENCE).unwrap();
    let full = full_runtime.opened_window().unwrap();
    assert_eq!(full.window.rows.len(), 4);
    let full_bytes = serde_json::to_vec(&full).unwrap().len();

    let mut shrunk_runtime =
        DesignerRuntime::from_document_working_size(text_grid(4, 1), OCCURRENCE).unwrap();
    let shrunk = shrunk_runtime.opened_window_within(full_bytes / 2).unwrap();
    assert!(
        shrunk.window.rows.len() < full.window.rows.len(),
        "{} rows did not shrink below {}",
        shrunk.window.rows.len(),
        full.window.rows.len()
    );
    assert!(!shrunk.window.rows.is_empty());

    let mut tiny_runtime =
        DesignerRuntime::from_document_working_size(text_grid(4, 1), OCCURRENCE).unwrap();
    let error = expect_error(tiny_runtime.opened_window_within(0));
    assert!(
        matches!(error, DesignerError::ProjectionTooLarge { maximum: 0, .. }),
        "{error:?}"
    );
}
