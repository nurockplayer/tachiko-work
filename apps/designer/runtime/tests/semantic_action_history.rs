use std::collections::BTreeMap;

use tachiko_designer_runtime::{
    CellEdit, CleanupOperation, DesignerRequest, DesignerResponse, DesignerRuntime, FieldProjection,
    ScalarEditInput, StoredValueProjection,
};
use tachiko_workspace_engine::{
    Document, Entity, EntityId, EntityKey, FieldDefinition, FieldId, FieldKey, FieldType, Number,
    Schema, SchemaId, SchemaKey, Value,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000301";

fn fixture() -> DesignerRuntime {
    let mut document = Document::empty("semantic_action_history", "Semantic action history");
    let schema = SchemaId::from("items");
    document.schemas.insert(
        schema.clone(),
        Schema {
            id: schema.clone(),
            key: SchemaKey::from("items"),
            fields: [
                ("name", FieldType::Text),
                ("a", FieldType::Number),
                ("b", FieldType::Number),
                ("c", FieldType::Number),
            ]
            .into_iter()
            .map(|(name, field_type)| {
                let field = FieldId::from(name);
                (
                    field.clone(),
                    FieldDefinition {
                        id: field,
                        key: FieldKey::from(name),
                        field_type,
                        required: true,
                    },
                )
            })
            .collect(),
        },
    );
    for index in 1..=3 {
        let entity = EntityId::from(format!("r{index}"));
        document.entities.insert(
            entity.clone(),
            Entity {
                id: entity.clone(),
                key: EntityKey::from(entity.to_string()),
                schema: schema.clone(),
                fields: BTreeMap::from([
                    (
                        FieldId::from("name"),
                        Value::Text(format!("  row {index}  ")),
                    ),
                    (
                        FieldId::from("a"),
                        Value::Number(Number::new(f64::from(index)).unwrap()),
                    ),
                    (
                        FieldId::from("b"),
                        Value::Number(Number::new(10.0).unwrap()),
                    ),
                    (
                        FieldId::from("c"),
                        Value::Number(Number::new(99.0).unwrap()),
                    ),
                ]),
            },
        );
    }
    DesignerRuntime::from_document(document, OCCURRENCE).unwrap()
}

fn field(runtime: &mut DesignerRuntime, revision: u32, target: &str) -> FieldProjection {
    let DesignerResponse::Fields(mut projection) = runtime
        .handle(DesignerRequest::QueryFields {
            expected_revision: format!("resident/{revision}"),
            fields: vec![target.into()],
        })
        .unwrap()
    else {
        panic!("expected fields response")
    };
    projection.fields.remove(0)
}

fn text(runtime: &mut DesignerRuntime, revision: u32, target: &str) -> String {
    let Some(StoredValueProjection::Text { value }) = field(runtime, revision, target).stored else {
        panic!("expected Text")
    };
    value
}

fn number(runtime: &mut DesignerRuntime, revision: u32, target: &str) -> f64 {
    let Some(StoredValueProjection::Number { value }) = field(runtime, revision, target).stored else {
        panic!("expected Number")
    };
    value
}

#[test]
fn generic_scalar_edit_interleaves_with_existing_semantic_history() {
    let mut runtime = fixture();
    runtime
        .handle(DesignerRequest::EditCells {
            expected_revision: "resident/0".into(),
            edits: vec![CellEdit {
                target: "r1.name".into(),
                input: ScalarEditInput::Text {
                    value: "first action".into(),
                },
            }],
        })
        .unwrap();
    runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/1".into(),
            target: "r1.a".into(),
            input: ScalarEditInput::Number { input: "5".into() },
        })
        .unwrap();

    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/2".into(),
        })
        .expect("generic scalar publication must be the next reversible semantic action");
    assert_eq!(number(&mut runtime, 3, "r1.a"), 1.0);
    assert_eq!(text(&mut runtime, 3, "r1.name"), "first action");

    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/3".into(),
        })
        .expect("the earlier semantic action must remain below the generic edit");
    assert_eq!(text(&mut runtime, 4, "r1.name"), "  row 1  ");

    runtime
        .handle(DesignerRequest::Redo {
            expected_revision: "resident/4".into(),
        })
        .unwrap();
    runtime
        .handle(DesignerRequest::Redo {
            expected_revision: "resident/5".into(),
        })
        .unwrap();
    assert_eq!(text(&mut runtime, 6, "r1.name"), "first action");
    assert_eq!(number(&mut runtime, 6, "r1.a"), 5.0);
}

#[test]
fn formula_copy_is_one_reversible_action_and_preserves_source_formula_history() {
    let mut runtime = fixture();
    runtime
        .handle(DesignerRequest::FormulaUpdate {
            expected_revision: "resident/0".into(),
            target: "r1.c".into(),
            source: "[r1.a] * 2".into(),
        })
        .unwrap();
    runtime
        .handle(DesignerRequest::CopyFormula {
            expected_revision: "resident/1".into(),
            source: "r1.c".into(),
            destinations: vec!["r2.c".into(), "r3.c".into()],
            fixed_references: Vec::new(),
            relative_rows: true,
            relative_columns: false,
        })
        .unwrap();

    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/2".into(),
        })
        .expect("formula copy must be one reversible semantic action");
    assert_eq!(number(&mut runtime, 3, "r2.c"), 99.0);
    assert_eq!(number(&mut runtime, 3, "r3.c"), 99.0);
    assert!(field(&mut runtime, 3, "r1.c").formula.is_some());

    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/3".into(),
        })
        .expect("the source formula action must remain independently undoable");
    assert_eq!(number(&mut runtime, 4, "r1.c"), 99.0);
}

#[test]
fn cleanup_commit_is_one_reversible_action_without_discarding_prior_history() {
    let mut runtime = fixture();
    runtime
        .handle(DesignerRequest::EditCells {
            expected_revision: "resident/0".into(),
            edits: vec![CellEdit {
                target: "r3.name".into(),
                input: ScalarEditInput::Text {
                    value: "prior action".into(),
                },
            }],
        })
        .unwrap();

    let DesignerResponse::CleanupPreview(preview) = runtime
        .handle(DesignerRequest::PreviewCleanup {
            expected_revision: "resident/1".into(),
            operation: CleanupOperation::Trim {
                fields: vec!["r1.name".into(), "r2.name".into()],
            },
        })
        .unwrap()
    else {
        panic!("expected cleanup preview")
    };
    runtime
        .handle(DesignerRequest::CommitCleanup {
            expected_revision: preview.revision,
            preview_id: preview.preview_id,
        })
        .unwrap();
    assert_eq!(text(&mut runtime, 2, "r1.name"), "row 1");
    assert_eq!(text(&mut runtime, 2, "r2.name"), "row 2");

    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/2".into(),
        })
        .expect("cleanup commit must be one reversible semantic action");
    assert_eq!(text(&mut runtime, 3, "r1.name"), "  row 1  ");
    assert_eq!(text(&mut runtime, 3, "r2.name"), "  row 2  ");
    assert_eq!(text(&mut runtime, 3, "r3.name"), "prior action");

    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/3".into(),
        })
        .expect("cleanup must not erase the earlier semantic action");
    assert_eq!(text(&mut runtime, 4, "r3.name"), "  row 3  ");
}
