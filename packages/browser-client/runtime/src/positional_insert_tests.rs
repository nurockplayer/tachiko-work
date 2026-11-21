use serde_json::json;
use std::collections::BTreeMap;

use super::{
    DesignerError, DesignerRequest, DesignerRuntime, Document, Entity, EntityId, EntityKey,
    Expression, FieldDefinition, FieldId, FieldKey, FieldRef, FieldType, FindDirection,
    FindInViewInput, Number, RowInitializer, RowPosition, ScalarEditInput, Schema, SchemaId,
    SchemaKey, TableView, Value, open_project,
};
use tachiko_workspace_engine::{FieldConstraint, RowRank};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000873";

fn fixture(count: usize) -> Document {
    let mut document = Document::empty("insert_rows", "Insert rows");
    let fields = [
        ("amount", FieldType::Number),
        ("calc", FieldType::Number),
        ("label", FieldType::Text),
    ]
    .into_iter()
    .map(|(name, field_type)| {
        let id = FieldId::from(name);
        (
            id.clone(),
            FieldDefinition {
                id,
                key: FieldKey::from(name),
                display_name: None,
                field_type,
                required: name == "label",
                constraint: FieldConstraint::None,
                number_format: None,
                date_format: None,
            },
        )
    })
    .collect();
    document.schemas.insert(
        SchemaId::from("rows"),
        Schema {
            headerless: false,
            id: SchemaId::from("rows"),
            key: SchemaKey::from("rows"),
            display_name: None,
            field_order: None,
            fields,
        },
    );
    for index in 0..count {
        let id = EntityId::from(format!("e{index:04}"));
        document.entities.insert(
            id.clone(),
            Entity {
                id: id.clone(),
                key: EntityKey::from(id.as_str()),
                schema: SchemaId::from("rows"),
                rank: None,
                fields: BTreeMap::from([
                    (FieldId::from("label"), Value::Text(id.to_string())),
                    (
                        FieldId::from("amount"),
                        Value::Number(Number::new(2.0).unwrap()),
                    ),
                    (
                        FieldId::from("calc"),
                        Value::Formula(Expression::Reference(FieldRef::new(id, "amount"))),
                    ),
                ]),
            },
        );
    }
    document
}

fn open(document: Document) -> DesignerRuntime {
    DesignerRuntime::from_document(document, OCCURRENCE).unwrap()
}

fn labels(runtime: &DesignerRuntime) -> Vec<String> {
    runtime.collection_specs["rows"]
        .entities
        .iter()
        .map(
            |id| match &runtime.session.document().entities[id].fields[&FieldId::from("label")] {
                Value::Text(text) => text.clone(),
                other => panic!("expected label, got {other:?}"),
            },
        )
        .collect()
}

fn inputs(label: &str) -> Vec<RowInitializer> {
    vec![
        RowInitializer {
            field: "label".into(),
            input: ScalarEditInput::Text {
                value: label.into(),
            },
        },
        RowInitializer {
            field: "amount".into(),
            input: ScalarEditInput::Number { input: "7".into() },
        },
    ]
}

fn insert(runtime: &mut DesignerRuntime, position: Option<RowPosition>, label: &str) {
    let expected_revision = runtime.current_revision().to_owned();
    runtime
        .handle(DesignerRequest::InsertRow {
            expected_revision,
            collection: "rows".into(),
            position,
            initializers: inputs(label),
        })
        .unwrap();
}

fn history(runtime: &mut DesignerRuntime, redo: bool) {
    let revision = runtime.current_revision().to_owned();
    runtime.history_edit(&revision, redo).unwrap();
}

#[test]
fn insert_before_after_and_append_preserve_initializers() {
    let mut runtime = open(fixture(3));
    insert(
        &mut runtime,
        Some(RowPosition::Before(EntityId::from("e0001"))),
        "before",
    );
    insert(
        &mut runtime,
        Some(RowPosition::After(EntityId::from("e0001"))),
        "after",
    );
    insert(&mut runtime, None, "append");
    assert_eq!(
        labels(&runtime),
        ["e0000", "before", "e0001", "after", "e0002", "append"]
    );
    for entity in runtime
        .session
        .document()
        .entities
        .values()
        .filter(|row| row.id.as_str().starts_with("native_table_row"))
    {
        assert_eq!(
            entity.fields[&FieldId::from("amount")],
            Value::Number(Number::new(7.0).unwrap())
        );
        assert_eq!(
            entity.fields[&FieldId::from("calc")],
            Value::Formula(Expression::SameRowReference(FieldId::from("amount")))
        );
    }
    let revision = runtime.current_revision().to_owned();
    let view = TableView::default();
    let window = runtime
        .query_table_window(&revision, "rows", &view, 0, 6, true)
        .unwrap();
    let id = &window.rows[1].id;
    assert_eq!(window.rows[1].position, Some(2));
    let located = runtime
        .locate_in_view(&revision, "rows", &view, id)
        .unwrap();
    assert_eq!((located.index, located.position), (Some(1), Some(2)));
    let found = runtime
        .find_in_view(FindInViewInput {
            expected_revision: &revision,
            collection: "rows",
            view: &view,
            query: "before",
            fields: &["label".into()],
            from: None,
            direction: FindDirection::Next,
        })
        .unwrap()
        .found
        .unwrap();
    assert_eq!((found.entity, found.position), (id.clone(), 2));
}

fn assert_refused(runtime: &mut DesignerRuntime, request: DesignerRequest) -> DesignerError {
    let before = runtime.session.document().clone();
    let revision = runtime.current_revision().to_owned();
    let history_size = (runtime.undo.len(), runtime.redo.len(), runtime.row_serial);
    let error = runtime.handle(request).unwrap_err();
    assert_eq!(runtime.session.document(), &before);
    assert_eq!(runtime.current_revision(), revision);
    assert_eq!(
        (runtime.undo.len(), runtime.redo.len(), runtime.row_serial),
        history_size
    );
    error
}

#[test]
fn insert_rejects_unknown_or_foreign_anchor_atomically() {
    let mut document = fixture(2);
    let mut schema = document.schemas["rows"].clone();
    schema.id = SchemaId::from("foreign");
    schema.key = SchemaKey::from("foreign");
    document.schemas.insert(schema.id.clone(), schema);
    let mut row = document.entities["e0000"].clone();
    row.id = EntityId::from("foreign_row");
    row.key = EntityKey::from("foreign_row");
    row.schema = SchemaId::from("foreign");
    document.entities.insert(row.id.clone(), row);
    let mut runtime = open(document);
    for anchor in ["missing", "foreign_row"] {
        for position in [
            RowPosition::Before(EntityId::from(anchor)),
            RowPosition::After(EntityId::from(anchor)),
        ] {
            assert_refused(
                &mut runtime,
                DesignerRequest::InsertRow {
                    position: Some(position),
                    expected_revision: "resident/0".into(),
                    collection: "rows".into(),
                    initializers: inputs("refused"),
                },
            );
        }
    }
    for position in [json!({"before":"e0000", "after":"e0001"}), json!({})] {
        let request = json!({"type":"insert_row", "expected_revision":"resident/0", "collection":"rows", "initializers":[], "position":position});
        assert!(serde_json::from_value::<DesignerRequest>(request).is_err());
    }
    assert_eq!(runtime.undo.len(), 0);
}

#[test]
fn first_insert_ranks_all_rows_and_one_undo_restores_none() {
    for position in [
        RowPosition::Before(EntityId::from("e0000")),
        RowPosition::After(EntityId::from("e0001")),
    ] {
        let document = fixture(2);
        let mut runtime = open(document.clone());
        insert(&mut runtime, Some(position), "inserted");
        assert_eq!(runtime.undo.len(), 1);
        assert!(
            runtime
                .session
                .document()
                .entities
                .values()
                .all(|row| row.rank.is_some())
        );
        let forward = runtime.session.document().clone();
        history(&mut runtime, false);
        assert_eq!(runtime.session.document(), &document);
        assert!(
            runtime
                .session
                .document()
                .entities
                .values()
                .all(|row| row.rank.is_none())
        );
        history(&mut runtime, true);
        assert_eq!(runtime.session.document(), &forward);
    }
}

#[test]
fn insert_between_equal_or_exhausted_ranks_rebalances_and_redoes() {
    let exhausted = format!("A{}1", "0".repeat(62));
    for ranks in [
        [Some("B"), Some("B"), None],
        [Some("A"), Some(exhausted.as_str()), Some("Z")],
        [Some("A"), None, None],
    ] {
        let mut document = fixture(3);
        for (row, rank) in document.entities.values_mut().zip(ranks) {
            row.rank = rank.map(|value| RowRank::new(value).unwrap());
        }
        let mut runtime = open(document.clone());
        let anchor = if ranks[1].is_none() { "e0002" } else { "e0001" };
        insert(
            &mut runtime,
            Some(RowPosition::Before(EntityId::from(anchor))),
            "middle",
        );
        let expected = if ranks[1].is_none() {
            vec!["e0000", "e0001", "middle", "e0002"]
        } else {
            vec!["e0000", "middle", "e0001", "e0002"]
        };
        assert_eq!(labels(&runtime), expected);
        assert_ne!(
            runtime.session.document().entities["e0000"].rank,
            document.entities["e0000"].rank
        );
        let forward = runtime.session.document().clone();
        history(&mut runtime, false);
        assert_eq!(runtime.session.document(), &document);
        history(&mut runtime, true);
        assert_eq!(runtime.session.document(), &forward);
    }
}

#[test]
fn insert_preserves_capacity_and_required_field_refusals() {
    for count in [2, super::MAX_TABLE_ROWS] {
        let mut document = fixture(count);
        document
            .schemas
            .get_mut(&SchemaId::from("rows"))
            .unwrap()
            .fields
            .retain(|id, _| id.as_str() == "label");
        for row in document.entities.values_mut() {
            row.fields.retain(|id, _| id.as_str() == "label");
        }
        let mut runtime = open(document);
        let error = assert_refused(
            &mut runtime,
            DesignerRequest::InsertRow {
                expected_revision: "resident/0".into(),
                collection: "rows".into(),
                position: Some(RowPosition::Before(EntityId::from("e0000"))),
                initializers: if count == 2 {
                    vec![]
                } else {
                    vec![RowInitializer {
                        field: "label".into(),
                        input: ScalarEditInput::Text {
                            value: "overflow".into(),
                        },
                    }]
                },
            },
        );
        if count == super::MAX_TABLE_ROWS {
            assert!(matches!(error, DesignerError::CapacityExceeded(error)
                if error.limit == super::CapacityLimit::TableRows
                    && error.maximum == count && error.attempted == count + 1));
        }
    }
}

#[test]
fn append_after_ranked_rows_lands_last() {
    let mut document = fixture(3);
    for row in document.entities.values_mut() {
        row.rank = Some(RowRank::new("B").unwrap());
    }
    let mut runtime = open(document);
    insert(&mut runtime, None, "appended");
    let expected = ["e0000", "e0001", "e0002", "appended"];
    assert_eq!(labels(&runtime), expected);
    let revision = runtime.current_revision().to_owned();
    runtime
        .paste_cells(
            &revision,
            "rows",
            None,
            "label",
            &[vec!["paste1".into()], vec!["paste2".into()]],
        )
        .unwrap();
    assert_eq!(
        labels(&runtime),
        ["e0000", "e0001", "e0002", "appended", "paste1", "paste2"]
    );
    history(&mut runtime, false);
    assert_eq!(labels(&runtime), expected);
    history(&mut runtime, true);
    assert_eq!(labels(&runtime).last().unwrap(), "paste2");
}

#[test]
fn positional_insert_save_reopen_and_xlsx_csv_round_trips() {
    use super::interop_adapter::{
        ImportOptions, OutputProfile, SourceValue, export_csv_for_profile, export_xlsx_for_profile,
        import_csv, import_xlsx,
    };
    let mut runtime = open(fixture(2));
    insert(
        &mut runtime,
        Some(RowPosition::Before(EntityId::from("e0001"))),
        "inserted",
    );
    let saved = runtime
        .export_project_v3(runtime.current_revision())
        .unwrap();
    let mut reopened = None;
    open_project(&mut reopened, &saved.bytes, OCCURRENCE).unwrap();
    let reopened = reopened.unwrap();
    assert_eq!(reopened.session.document(), runtime.session.document());
    let workbook = reopened
        .export_native_tables_workbook("resident/0", "rows")
        .unwrap();
    let expected = ["e0000", "inserted", "e0001"].map(|value| SourceValue::Text {
        value: value.into(),
    });
    let xlsx = export_xlsx_for_profile(&workbook, OutputProfile::Shared).unwrap();
    let csv = export_csv_for_profile(&workbook.sheets[0], OutputProfile::Shared).unwrap();
    for decoded in [
        import_xlsx(&xlsx).unwrap(),
        import_csv(&csv, &ImportOptions::default()).unwrap(),
    ] {
        assert_eq!(
            decoded.sheets[0]
                .rows
                .iter()
                .map(|row| row[2].value.clone())
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn unranked_append_preserves_save_and_export_bytes() {
    use super::interop_adapter::{OutputProfile, export_csv_for_profile, export_xlsx_for_profile};
    let mut runtime = open(fixture(2));
    insert(&mut runtime, None, "appended");
    assert!(
        runtime
            .session
            .document()
            .entities
            .values()
            .all(|row| row.rank.is_none())
    );
    let mut expected = fixture(2);
    let new_row = runtime
        .session
        .document()
        .entities
        .values()
        .find(|row| row.id.as_str().starts_with("native_table_row"))
        .unwrap()
        .clone();
    expected.entities.insert(new_row.id.clone(), new_row);
    let baseline = open(expected);
    assert_eq!(
        runtime
            .export_project_v3(runtime.current_revision())
            .unwrap()
            .bytes,
        baseline.export_project_v3("resident/0").unwrap().bytes
    );
    let actual = runtime
        .export_native_tables_workbook(runtime.current_revision(), "rows")
        .unwrap();
    let expected = baseline
        .export_native_tables_workbook("resident/0", "rows")
        .unwrap();
    assert_eq!(
        export_xlsx_for_profile(&actual, OutputProfile::Shared).unwrap(),
        export_xlsx_for_profile(&expected, OutputProfile::Shared).unwrap()
    );
    assert_eq!(
        export_csv_for_profile(&actual.sheets[0], OutputProfile::Shared).unwrap(),
        export_csv_for_profile(&expected.sheets[0], OutputProfile::Shared).unwrap()
    );
}

#[test]
fn ranked_edges_and_ranked_unranked_boundary_allocate_without_reordering_old_rows() {
    for (position, expected) in [
        (
            RowPosition::Before(EntityId::from("e0000")),
            vec!["inserted", "e0000", "e0001", "e0002"],
        ),
        (
            RowPosition::Before(EntityId::from("e0001")),
            vec!["e0000", "inserted", "e0001", "e0002"],
        ),
        (
            RowPosition::After(EntityId::from("e0000")),
            vec!["e0000", "inserted", "e0001", "e0002"],
        ),
    ] {
        let mut document = fixture(3);
        document
            .entities
            .get_mut(&EntityId::from("e0000"))
            .unwrap()
            .rank = Some(RowRank::new("A").unwrap());
        let mut runtime = open(document.clone());
        insert(&mut runtime, Some(position), "inserted");
        assert_eq!(labels(&runtime), expected);
        for (id, row) in &document.entities {
            assert_eq!(runtime.session.document().entities[id].rank, row.rank);
        }
        history(&mut runtime, false);
        assert_eq!(runtime.session.document(), &document);
    }
}

#[test]
fn append_with_an_earlier_identity_preserves_unranked_order_and_undo() {
    let mut document = fixture(1);
    let mut row = document.entities.remove(&EntityId::from("e0000")).unwrap();
    row.id = EntityId::from("z_last");
    row.key = EntityKey::from("z_last");
    row.fields.insert(
        FieldId::from("calc"),
        Value::Formula(Expression::SameRowReference(FieldId::from("amount"))),
    );
    document.entities.insert(row.id.clone(), row);
    let mut runtime = open(document.clone());
    insert(&mut runtime, None, "appended");
    assert_eq!(labels(&runtime), ["appended", "e0000"]);
    assert!(
        runtime
            .session
            .document()
            .entities
            .values()
            .all(|row| row.rank.is_none())
    );
    history(&mut runtime, false);
    assert_eq!(runtime.session.document(), &document);
    history(&mut runtime, true);
    assert_eq!(labels(&runtime), ["appended", "e0000"]);
    let appended = runtime.session.document().clone();
    let revision = runtime.current_revision().to_owned();
    runtime
        .paste_cells(&revision, "rows", None, "label", &[vec!["pasted".into()]])
        .unwrap();
    assert_eq!(labels(&runtime), ["appended", "pasted", "e0000"]);
    assert!(
        runtime
            .session
            .document()
            .entities
            .values()
            .all(|row| row.rank.is_none())
    );
    history(&mut runtime, false);
    assert_eq!(runtime.session.document(), &appended);
}
