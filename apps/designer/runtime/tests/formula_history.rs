use std::collections::BTreeMap;

use tachiko_designer_runtime::{
    DesignerRequest, DesignerResponse, DesignerRuntime, FieldProjection, FieldTarget, open_project,
};
use tachiko_workspace_engine::{
    Document, Entity, EntityId, EntityKey, Expression, FieldDefinition, FieldId, FieldKey,
    FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000300";

/// Build the smallest numeric document that can switch between scalar and formula meaning.
fn fixture() -> DesignerRuntime {
    let mut document = Document::empty("formula_history", "Formula history");
    let schema = SchemaId::from("items");
    let field = FieldId::from("n");
    let source = FieldId::from("source");
    let dependent = FieldId::from("dependent");
    let label = FieldId::from("label");
    document.schemas.insert(
        schema.clone(),
        Schema {
            id: schema.clone(),
            key: SchemaKey::from("items"),
            fields: BTreeMap::from([
                (
                    field.clone(),
                    FieldDefinition {
                        id: field.clone(),
                        key: FieldKey::from("n"),
                        field_type: FieldType::Number,
                        required: true,
                    },
                ),
                (
                    source.clone(),
                    FieldDefinition {
                        id: source.clone(),
                        key: FieldKey::from("source"),
                        field_type: FieldType::Number,
                        required: true,
                    },
                ),
                (
                    dependent.clone(),
                    FieldDefinition {
                        id: dependent.clone(),
                        key: FieldKey::from("dependent"),
                        field_type: FieldType::Number,
                        required: true,
                    },
                ),
                (
                    label.clone(),
                    FieldDefinition {
                        id: label.clone(),
                        key: FieldKey::from("label"),
                        field_type: FieldType::Text,
                        required: true,
                    },
                ),
            ]),
        },
    );
    document.entities.insert(
        EntityId::from("r1"),
        Entity {
            id: EntityId::from("r1"),
            key: EntityKey::from("r1"),
            schema,
            fields: BTreeMap::from([
                (field, Value::Number(Number::new(2.0).unwrap())),
                (source, Value::Number(Number::new(5.0).unwrap())),
                (
                    dependent,
                    Value::Formula(Expression::Add {
                        left: Box::new(Expression::Reference(FieldRef::new("r1", "n"))),
                        right: Box::new(Expression::Number(Number::new(1.0).unwrap())),
                    }),
                ),
                (label, Value::Text("one".to_owned())),
            ]),
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

/// Query one calculated Number at an exact expected resident revision.
fn calculated_number(runtime: &mut DesignerRuntime, revision: u32, target: &str) -> Option<f64> {
    let DesignerResponse::Fields(mut projection) = runtime
        .handle(DesignerRequest::QueryFields {
            expected_revision: format!("resident/{revision}"),
            fields: vec![FieldTarget::from(target)],
        })
        .unwrap()
    else {
        panic!("expected fields response")
    };
    projection
        .fields
        .remove(0)
        .calculated
        .as_ref()
        .and_then(tachiko_designer_runtime::CalculationProjection::number)
}

/// Publish one supported `FormulaUpdate` through the ordinary Designer request path.
fn formula(runtime: &mut DesignerRuntime, revision: u32, source: &str) {
    runtime
        .handle(DesignerRequest::FormulaUpdate {
            expected_revision: format!("resident/{revision}"),
            target: "r1.n".into(),
            source: source.to_owned(),
        })
        .unwrap();
}

/// Rejected nonnumeric targets must be classified by `FormulaUpdate` lifecycle admission.
#[test]
fn rejected_nonnumeric_formula_target_does_not_preempt_lifecycle_admission_or_history() {
    let mut runtime = fixture();
    let before = runtime.export_project("resident/0").unwrap().bytes;
    let error = runtime
        .handle(DesignerRequest::FormulaUpdate {
            expected_revision: "resident/0".to_owned(),
            target: "r1.label".into(),
            source: "1".to_owned(),
        })
        .expect_err("a Text target must be rejected by FormulaUpdate admission");
    assert_eq!(error.failure_projection("resident/0").code, "edit_rejected");
    assert_eq!(runtime.export_project("resident/0").unwrap().bytes, before);

    formula(&mut runtime, 0, "3");
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/1".to_owned(),
        })
        .expect("the rejected request must not create or corrupt formula history");
    assert!(field(&mut runtime, 2).formula.is_none());
}

/// Removing a formula dependency must refresh dependent calculations through normal publication.
#[test]
fn formula_inverse_refreshes_dependent_calculations_and_invalidation() {
    let mut runtime = fixture();
    let DesignerResponse::Published(applied) = runtime
        .handle(DesignerRequest::FormulaUpdate {
            expected_revision: "resident/0".to_owned(),
            target: "r1.n".into(),
            source: "[r1.source] + 1".to_owned(),
        })
        .expect("the numeric formula must publish")
    else {
        panic!("expected formula publication")
    };
    assert!(
        applied
            .affected_calculations
            .contains(&FieldTarget::from("r1.dependent"))
    );
    assert_eq!(
        calculated_number(&mut runtime, 1, "r1.dependent"),
        Some(7.0)
    );

    let DesignerResponse::Published(restored) = runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/1".to_owned(),
        })
        .expect("the scalar inverse must publish")
    else {
        panic!("expected inverse publication")
    };
    assert!(
        restored
            .affected_calculations
            .contains(&FieldTarget::from("r1.dependent"))
    );
    assert_eq!(
        calculated_number(&mut runtime, 2, "r1.dependent"),
        Some(3.0)
    );
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
    let restored_bytes = runtime.export_project("resident/2").unwrap().bytes;
    let mut reopened = None;
    open_project(
        &mut reopened,
        &restored_bytes,
        "00000000-0000-4000-8000-000000000301",
    )
    .expect("the restored scalar must remain a valid saved project");
    let mut reopened = reopened.expect("project must be open");
    let reopened = field(&mut reopened, 0);
    assert!(reopened.formula.is_none());
    assert_eq!(
        reopened.stored.and_then(|value| match value {
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
    let redone = field(&mut runtime, 3);
    assert_eq!(
        redone.formula.as_ref().map(|value| value.source.as_str()),
        Some("3")
    );
    assert!(redone.stored.is_none());
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
        .handle(DesignerRequest::Redo {
            expected_revision: "resident/3".to_owned(),
        })
        .expect("the latest formula replacement must redo exactly");
    assert_eq!(
        field(&mut runtime, 4)
            .formula
            .as_ref()
            .map(|value| value.source.as_str()),
        Some("4")
    );

    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/4".to_owned(),
        })
        .expect("redo must not consume the earlier formula history");

    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/5".to_owned(),
        })
        .expect("the earlier scalar-to-formula action must remain below it");
    let restored = field(&mut runtime, 6);
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
fn rejected_formula_update_preserves_the_existing_formula_history_directions() {
    let mut runtime = fixture();
    formula(&mut runtime, 0, "3");
    formula(&mut runtime, 1, "4");
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/2".to_owned(),
        })
        .expect("the second formula must create a pending redo");

    let before = runtime.export_project("resident/3").unwrap().bytes;
    let error = runtime
        .handle(DesignerRequest::FormulaUpdate {
            expected_revision: "resident/3".to_owned(),
            target: "r1.n".into(),
            source: "[missing.n] + 1".to_owned(),
        })
        .expect_err("unbound formula must not publish");
    assert_ne!(error.failure_projection("resident/3").code, "no_change");
    assert_eq!(runtime.export_project("resident/3").unwrap().bytes, before);

    runtime
        .handle(DesignerRequest::Redo {
            expected_revision: "resident/3".to_owned(),
        })
        .expect("a rejected formula attempt must retain the pending redo");
    assert_eq!(
        field(&mut runtime, 4)
            .formula
            .as_ref()
            .map(|value| value.source.as_str()),
        Some("4")
    );
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/4".to_owned(),
        })
        .expect("the rejected request must preserve the newer inverse direction");
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/5".to_owned(),
        })
        .expect("the rejected request must preserve the earlier undo direction");
    let restored = field(&mut runtime, 6);
    assert!(restored.formula.is_none());
    assert_eq!(
        restored.stored.and_then(|value| match value {
            tachiko_designer_runtime::StoredValueProjection::Number { value } => Some(value),
            _ => None,
        }),
        Some(2.0)
    );
}

/// Stale and semantic-NoChange formula attempts must leave both history directions usable.
#[test]
fn stale_and_no_change_formula_attempts_preserve_existing_history_directions() {
    let mut runtime = fixture();
    formula(&mut runtime, 0, "3");
    formula(&mut runtime, 1, "4");
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/2".to_owned(),
        })
        .expect("the second formula must create a pending redo");
    let before = runtime.export_project("resident/3").unwrap().bytes;

    let stale = runtime
        .handle(DesignerRequest::FormulaUpdate {
            expected_revision: "resident/2".to_owned(),
            target: "r1.n".into(),
            source: "5".to_owned(),
        })
        .expect_err("stale formula update must not publish");
    assert_eq!(
        stale.failure_projection("resident/3").code,
        "stale_revision"
    );

    let no_change = runtime
        .handle(DesignerRequest::FormulaUpdate {
            expected_revision: "resident/3".to_owned(),
            target: "r1.n".into(),
            source: "3".to_owned(),
        })
        .expect_err("identical bound formula meaning must be NoChange");
    assert_eq!(no_change.failure_projection("resident/3").code, "no_change");
    assert_eq!(runtime.export_project("resident/3").unwrap().bytes, before);

    runtime
        .handle(DesignerRequest::Redo {
            expected_revision: "resident/3".to_owned(),
        })
        .expect("stale and NoChange attempts must preserve the pending redo");
    assert_eq!(
        field(&mut runtime, 4)
            .formula
            .as_ref()
            .map(|value| value.source.as_str()),
        Some("4")
    );
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/4".to_owned(),
        })
        .expect("stale and NoChange attempts must preserve the newer inverse direction");
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/5".to_owned(),
        })
        .expect("stale and NoChange attempts must preserve the earlier undo direction");
    let restored = field(&mut runtime, 6);
    assert!(restored.formula.is_none());
    assert_eq!(
        restored.stored.and_then(|value| match value {
            tachiko_designer_runtime::StoredValueProjection::Number { value } => Some(value),
            _ => None,
        }),
        Some(2.0)
    );
}
