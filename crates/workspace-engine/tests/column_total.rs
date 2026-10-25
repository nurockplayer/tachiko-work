//! A column total keeps its column identity through authoring, renames, row
//! removal, and explanation.
use std::collections::BTreeMap;

use tachiko_formula_engine::{FormulaBindError, calculate};
use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, EntityKey, FieldAddress, FieldConstraint,
    FieldDefinition, FieldId, FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey,
    Value,
};
use tachiko_workspace_engine::{
    WorkspaceError, explain_field, remove_entity, rename_field, rename_schema, set_formula,
};

fn number_field(id: &str) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(id),
        display_name: None,
        field_type: FieldType::Number,
        required: true,
        constraint: FieldConstraint::None,
    }
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn plan_row(id: &str, reserve: f64) -> Entity {
    Entity {
        id: EntityId::from(id),
        key: EntityKey::from(id),
        schema: SchemaId::from("plan"),
        fields: BTreeMap::from([
            (FieldId::from("item"), Value::Text(id.to_owned())),
            (FieldId::from("reserve"), number(reserve)),
        ]),
    }
}

fn document() -> Document {
    Document {
        id: DocumentId::from("monthly-plan"),
        title: "Monthly plan".to_owned(),
        schemas: BTreeMap::from([
            (
                SchemaId::from("plan"),
                Schema {
                    id: SchemaId::from("plan"),
                    key: SchemaKey::from("plan"),
                    display_name: None,
                    fields: BTreeMap::from([
                        (
                            FieldId::from("item"),
                            FieldDefinition {
                                field_type: FieldType::Text,
                                ..number_field("item")
                            },
                        ),
                        (FieldId::from("reserve"), number_field("reserve")),
                    ]),
                },
            ),
            (
                SchemaId::from("summary"),
                Schema {
                    id: SchemaId::from("summary"),
                    key: SchemaKey::from("summary"),
                    display_name: None,
                    fields: BTreeMap::from([(FieldId::from("total"), number_field("total"))]),
                },
            ),
        ]),
        entities: BTreeMap::from([
            ("rent".into(), plan_row("rent", 12_000.0)),
            ("ads".into(), plan_row("ads", 500.0)),
            (
                EntityId::from("summary"),
                Entity {
                    id: EntityId::from("summary"),
                    key: EntityKey::from("summary"),
                    schema: SchemaId::from("summary"),
                    fields: BTreeMap::from([(FieldId::from("total"), number(0.0))]),
                },
            ),
        ]),
        keyed_grouped_sum_definitions: BTreeMap::new(),
    }
}

fn summary_total() -> FieldAddress {
    FieldAddress::new("summary", "total")
}

fn total_value(document: &Document) -> Number {
    calculate(document)
        .expect("the document must calculate")
        .value(&FieldRef::new("summary", "total"))
        .expect("the total must have a value")
}

#[test]
fn a_column_total_authoring_round_trip_names_the_column() {
    let document = document();
    let preview = set_formula(&document, &summary_total(), "[plan.reserve:total]")
        .expect("the column total should be admitted");
    assert_eq!(
        total_value(&preview.document),
        Number::new(12_500.0).unwrap()
    );
    let explanation =
        explain_field(&preview.document, &summary_total()).expect("the total should explain");
    assert_eq!(
        explanation.expression.as_deref(),
        Some("[plan.reserve:total]")
    );
    assert_eq!(explanation.display_value, "12500");
}

#[test]
fn an_unknown_column_is_a_typed_binding_refusal() {
    let document = document();
    let error = set_formula(&document, &summary_total(), "[budget.reserve:total]").unwrap_err();
    let WorkspaceError::FormulaBinding { source, .. } = error else {
        panic!("expected a binding refusal, got {error:?}");
    };
    assert!(matches!(*source, FormulaBindError::Column { .. }));
}

#[test]
fn renaming_the_column_keeps_the_total_on_the_same_cells() {
    let document = document();
    let authored = set_formula(&document, &summary_total(), "[plan.reserve:total]")
        .unwrap()
        .document;
    let renamed = rename_schema(&authored, "plan", "monthly")
        .and_then(|preview| rename_field(&preview.document, "monthly", "reserve", "buffer"))
        .expect("renames should keep the formula projectable");
    assert_eq!(
        total_value(&renamed.document),
        Number::new(12_500.0).unwrap()
    );
    let explanation =
        explain_field(&renamed.document, &summary_total()).expect("the total should explain");
    assert_eq!(
        explanation.expression.as_deref(),
        Some("[monthly.buffer:total]")
    );
}

#[test]
fn removing_a_summed_row_recomputes_the_total() {
    let document = document();
    let authored = set_formula(&document, &summary_total(), "[plan.reserve:total]")
        .unwrap()
        .document;
    let removed = remove_entity(&authored, "ads").expect("a summed row may still be removed");
    assert_eq!(
        total_value(&removed.document),
        Number::new(12_000.0).unwrap()
    );
}

#[test]
fn removing_an_entity_that_a_direct_reference_reads_is_still_refused() {
    let document = document();
    let authored = set_formula(&document, &summary_total(), "[ads.reserve]")
        .unwrap()
        .document;
    let error = remove_entity(&authored, "ads").unwrap_err();
    assert!(matches!(error, WorkspaceError::EntityReferenced { .. }));
}

#[test]
fn a_column_total_over_its_own_row_is_refused_as_a_cycle() {
    let document = document();
    let error = set_formula(
        &document,
        &FieldAddress::new("rent", "reserve"),
        "[plan.reserve:total]",
    )
    .expect_err("a total that includes its own row is a cycle");
    assert!(matches!(error, WorkspaceError::InvalidDocument { .. }));
}
