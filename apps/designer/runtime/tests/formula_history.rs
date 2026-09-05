use std::collections::BTreeMap;

use tachiko_designer_runtime::{
    DesignerRequest, DesignerResponse, DesignerRuntime, FieldProjection,
};
use tachiko_workspace_engine::{
    Document, Entity, EntityId, EntityKey, FieldDefinition, FieldId, FieldKey, FieldType, Number,
    Schema, SchemaId, SchemaKey, Value,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000300";

/// Build the smallest numeric document that can switch between scalar and formula meaning.
fn fixture() -> DesignerRuntime {
    let mut document = Document::empty("formula_history", "Formula history");
    let schema = SchemaId::from("items");
    let field = FieldId::from("n");
    document.schemas.insert(
        schema.clone(),
        Schema {
            id: schema.clone(),
            key: SchemaKey::from("items"),
            fields: BTreeMap::from([(
                field.clone(),
                FieldDefinition {
                    id: field.clone(),
                    key: FieldKey::from("n"),
                    field_type: FieldType::Number,
                    required: true,
                },
            )]),
        },
    );
    document.entities.insert(
        EntityId::from("r1"),
        Entity {
            id: EntityId::from("r1"),
            key: EntityKey::from("r1"),
            schema,
            fields: BTreeMap::from([(
                field,
                Value::Number(Number::new(2.0).unwrap()),
            )]),
        },
    );
    DesignerRuntime::from_document(document, OCCURRENCE).unwrap()
}

/// Query the only field at an exact expected resident revision.
fn field(runtime: &mut DesignerRuntime, revision: u32) -> FieldProjection {
    let DesignerResponse::Fields(mut projection) = runtime
        .handle(DesignerRequest::QueryFields {
            expected_revision: format!("resident/{revision}"),
            fields: vec!["r1.n".into()],
        })
        .unwrap()
    else {
        panic!("expected fields response")
    };
    projection.fields.remove(0)
}

/// Publish one supported FormulaUpdate through the ordinary Designer request path.
fn formula(runtime: &mut DesignerRuntime, revision: u32, source: &str) {
    runtime
        .handle(DesignerRequest::FormulaUpdate {
            expected_revision: format!("resident/{revision}"),
            target: "r1.n".into(),
            source: source.to_owned(),
        })
        .unwrap();
}

/// A scalar-to-formula action must round-trip through Undo and Redo without losing meaning.
#[test]
fn scalar_to_formula_undo_restores_exact_scalar_and_redo_restores_formula() {
    let mut runtime = fixture();
    formula(&mut runtime, 0, "3");

    let after_formula = field(&mut runtime, 1);
    assert_eq!(
        after_formula
            .formula
            .as_ref()
            .map(|value| value.source.as_str()),
        Some("3")
    );
    assert_eq!(after_formula.stored, None);

    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/1".to_owned(),
        })
        .expect("formula authoring must be one reversible semantic action");

    let restored = field(&mut runtime, 2);
    assert!(restored.formula.is_none());
    assert_eq!(
        restored.stored.and_then(|value| match value {
            tachiko_designer_runtime::StoredValueProjection::Number { value } => Some(value),
            _ => None,
        }),
        Some(2.0)
    );

    runtime
        .handle(DesignerRequest::Redo {
            expected_revision: "resident/2".to_owned(),
        })
        .expect("redo must restore the exact accepted formula action");
    assert_eq!(
        field(&mut runtime, 3)
            .formula
            .as_ref()
            .map(|value| value.source.as_str()),
        Some("3")
    );
}

/// Consecutive formula edits remain separate history actions above the original scalar.
#[test]
fn formula_to_formula_undo_restores_previous_formula_then_prior_scalar() {
    let mut runtime = fixture();
    formula(&mut runtime, 0, "3");
    formula(&mut runtime, 1, "4");

    assert_eq!(
        field(&mut runtime, 2)
            .formula
            .as_ref()
            .map(|value| value.source.as_str()),
        Some("4")
    );

    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/2".to_owned(),
        })
        .expect("the latest formula replacement must be reversible");
    assert_eq!(
        field(&mut runtime, 3)
            .formula
            .as_ref()
            .map(|value| value.source.as_str()),
        Some("3")
    );

    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/3".to_owned(),
        })
        .expect("the earlier scalar-to-formula action must remain below it");
    let restored = field(&mut runtime, 4);
    assert!(restored.formula.is_none());
    assert_eq!(
        restored.stored.and_then(|value| match value {
            tachiko_designer_runtime::StoredValueProjection::Number { value } => Some(value),
            _ => None,
        }),
        Some(2.0)
    );
}

/// A rejected formula body changes neither canonical bytes nor the prior reversible action.
#[test]
fn rejected_formula_update_preserves_the_existing_formula_undo_entry() {
    let mut runtime = fixture();
    formula(&mut runtime, 0, "3");

    let before = runtime.export_project("resident/1").unwrap().bytes;
    let error = runtime
        .handle(DesignerRequest::FormulaUpdate {
            expected_revision: "resident/1".to_owned(),
            target: "r1.n".into(),
            source: "[missing.n] + 1".to_owned(),
        })
        .expect_err("unbound formula must not publish");
    assert_ne!(error.failure_projection("resident/1").code, "no_change");
    assert_eq!(runtime.export_project("resident/1").unwrap().bytes, before);

    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/1".to_owned(),
        })
        .expect("a rejected formula attempt must not destroy prior reversible history");
    let restored = field(&mut runtime, 2);
    assert!(restored.formula.is_none());
}

/// Stale and semantic-NoChange formula attempts must leave the accepted action undoable.
#[test]
fn stale_and_no_change_formula_attempts_preserve_the_existing_undo_entry() {
    let mut runtime = fixture();
    formula(&mut runtime, 0, "3");
    let before = runtime.export_project("resident/1").unwrap().bytes;

    let stale = runtime
        .handle(DesignerRequest::FormulaUpdate {
            expected_revision: "resident/0".to_owned(),
            target: "r1.n".into(),
            source: "4".to_owned(),
        })
        .expect_err("stale formula update must not publish");
    assert_eq!(stale.failure_projection("resident/1").code, "stale_revision");

    let no_change = runtime
        .handle(DesignerRequest::FormulaUpdate {
            expected_revision: "resident/1".to_owned(),
            target: "r1.n".into(),
            source: "3".to_owned(),
        })
        .expect_err("identical bound formula meaning must be NoChange");
    assert_eq!(no_change.failure_projection("resident/1").code, "no_change");
    assert_eq!(runtime.export_project("resident/1").unwrap().bytes, before);

    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/1".to_owned(),
        })
        .expect("stale and NoChange attempts must preserve prior formula history");
    let restored = field(&mut runtime, 2);
    assert!(restored.formula.is_none());
}
