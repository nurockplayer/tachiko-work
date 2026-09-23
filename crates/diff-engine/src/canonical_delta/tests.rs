use std::collections::BTreeMap;

use tachiko_formula_engine::CalculationError;
use tachiko_semantic_core::{
    Document, Entity, Expression, FieldConstraint, FieldDefinition, FieldId, FieldType,
    KeyedGroupedSumBindingRole, KeyedGroupedSumDefinition, KeyedGroupedSumDefinitionError,
    KeyedGroupedSumDefinitionId, KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding,
    Number, Schema, SchemaId, Value,
};

use super::{
    CANONICAL_SEMANTIC_DELTA_V2, CanonicalDeltaError, CanonicalDirectFact as Fact, DeltaInputSide,
    canonical_delta,
};

fn number(value: f64) -> Number {
    Number::new(value).unwrap()
}

fn text_schema(id: &str) -> Schema {
    Schema {
        id: id.into(),
        key: "schema".into(),
        fields: BTreeMap::from([(
            "f".into(),
            FieldDefinition {
                id: "f".into(),
                key: "f".into(),
                field_type: FieldType::Text,
                required: false,
                constraint: FieldConstraint::None,
            },
        )]),
    }
}

fn doc_with_schemas(ids: &[&str]) -> Document {
    let mut document = Document::empty("doc", "title");
    let mut sorted_ids = ids.to_vec();
    sorted_ids.sort_unstable();
    for id in ids {
        let mut schema = text_schema(id);
        let index = sorted_ids.binary_search(id).unwrap();
        schema.key = format!("schema_{index}").into();
        document.schemas.insert((*id).into(), schema);
    }
    document
}

fn keyed_definition() -> KeyedGroupedSumDefinition {
    KeyedGroupedSumDefinition {
        id: "summary".into(),
        orders: KeyedGroupedSumOrdersBinding {
            schema: "orders".into(),
            lookup_key_field: "lookup".into(),
            quantity_field: "quantity".into(),
        },
        products: KeyedGroupedSumProductsBinding {
            schema: "products".into(),
            key_field: "key".into(),
            category_field: "category".into(),
            price_field: "price".into(),
        },
    }
}

fn numeric_or_text_schema(id: &str, fields: &[(&str, FieldType)]) -> Schema {
    Schema {
        id: id.into(),
        key: id.into(),
        fields: fields
            .iter()
            .map(|(field_id, field_type)| {
                (
                    (*field_id).into(),
                    FieldDefinition {
                        id: (*field_id).into(),
                        key: (*field_id).into(),
                        field_type: field_type.clone(),
                        required: false,
                        constraint: FieldConstraint::None,
                    },
                )
            })
            .collect(),
    }
}

#[test]
fn stable_id_order_uses_scalar_order_and_shorter_prefix_first_repeatedly() {
    let before = doc_with_schemas(&[]);
    let after = doc_with_schemas(&["aa", "a", "\u{10000}", "\u{e000}"]);
    let expected = ["a", "aa", "\u{e000}", "\u{10000}"];
    for _ in 0..4 {
        let result = canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &before, &after).unwrap();
        let observed = result
            .facts()
            .iter()
            .map(|fact| match fact {
                Fact::SchemaCreated { schema, .. } => schema.as_str(),
                other => panic!("unexpected fact: {other:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(observed, expected);
    }
    let same_state_different_insertion = doc_with_schemas(&["\u{e000}", "aa", "\u{10000}", "a"]);
    assert_eq!(
        canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &before, &after).unwrap(),
        canonical_delta(
            CANONICAL_SEMANTIC_DELTA_V2,
            &before,
            &same_state_different_insertion
        )
        .unwrap(),
    );
}

#[test]
fn invalid_constraint_lists_are_refused_without_normalization() {
    let mut before = Document::empty("doc", "title");
    let mut schema = text_schema("s");
    schema.fields.get_mut("f").unwrap().constraint = FieldConstraint::TextLiteralSet {
        values: vec!["same".into(), "same".into()],
    };
    before.schemas.insert("s".into(), schema);
    let after = Document::empty("doc", "title");
    assert!(matches!(
        canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &before, &after),
        Err(CanonicalDeltaError::InvalidInput {
            side: DeltaInputSide::Before,
            ..
        })
    ));
}

#[test]
fn calculation_failure_is_reported_for_the_failing_side() {
    let valid = Document::empty("doc", "title");
    let mut failed = valid.clone();
    let schema = Schema {
        id: "s".into(),
        key: "s".into(),
        fields: BTreeMap::from([(
            "n".into(),
            FieldDefinition {
                id: "n".into(),
                key: "n".into(),
                field_type: FieldType::Number,
                required: false,
                constraint: FieldConstraint::None,
            },
        )]),
    };
    failed.schemas.insert("s".into(), schema);
    failed.entities.insert(
        "e".into(),
        Entity {
            id: "e".into(),
            key: "e".into(),
            schema: "s".into(),
            fields: BTreeMap::from([(
                "n".into(),
                Value::Formula(Expression::Divide {
                    left: Box::new(Expression::Number(number(1.0))),
                    right: Box::new(Expression::Number(number(0.0))),
                }),
            )]),
        },
    );
    assert!(matches!(
        canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &valid, &failed),
        Err(CanonicalDeltaError::AfterCalculation(
            CalculationError::DivisionByZero { .. }
        ))
    ));
    assert!(matches!(
        canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &failed, &valid),
        Err(CanonicalDeltaError::BeforeCalculation(
            CalculationError::DivisionByZero { .. }
        ))
    ));
}

#[test]
fn same_field_text_transition_compares_schema_qualified_targets() {
    let mut before = doc_with_schemas(&["old", "new"]);
    before.entities.insert(
        "e".into(),
        Entity {
            id: "e".into(),
            key: "e".into(),
            schema: SchemaId::from("old"),
            fields: BTreeMap::from([(FieldId::from("f"), Value::Text("x".into()))]),
        },
    );
    let mut after = before.clone();
    after.entities.get_mut("e").unwrap().schema = "new".into();
    let facts = canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &before, &after).unwrap();
    assert!(facts.facts().contains(&Fact::EntityFieldValueCleared {
        entity: "e".into(),
        schema: "old".into(),
        field: "f".into(),
        before: Value::Text("x".into()),
    }));
    assert!(facts.facts().contains(&Fact::EntityFieldValueCreated {
        entity: "e".into(),
        schema: "new".into(),
        field: "f".into(),
        after: Value::Text("x".into()),
    }));
}

#[test]
fn keyed_grouped_sum_definition_change_refuses_the_whole_delta() {
    let before = Document::empty("doc", "title");
    let mut after = before.clone();
    after.keyed_grouped_sum_definitions.insert(
        "summary".into(),
        KeyedGroupedSumDefinition {
            id: KeyedGroupedSumDefinitionId::from("summary"),
            orders: KeyedGroupedSumOrdersBinding {
                schema: "orders".into(),
                lookup_key_field: "lookup".into(),
                quantity_field: "quantity".into(),
            },
            products: KeyedGroupedSumProductsBinding {
                schema: "products".into(),
                key_field: "key".into(),
                category_field: "category".into(),
                price_field: "price".into(),
            },
        },
    );
    assert!(matches!(
        canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &before, &after),
        Err(CanonicalDeltaError::UnsupportedKeyedGroupedSumDefinitionChange)
    ));
}

#[test]
fn equal_intrinsically_invalid_keyed_definition_is_rejected_with_before_cause() {
    let mut before = Document::empty("doc", "before");
    before
        .keyed_grouped_sum_definitions
        .insert("summary".into(), keyed_definition());
    let mut after = before.clone();
    after.title = "after".into();
    assert!(matches!(
        canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &before, &after),
        Err(CanonicalDeltaError::InvalidKeyedGroupedSumDefinitions {
            side: DeltaInputSide::Before,
            source: KeyedGroupedSumDefinitionError::MissingSchema {
                role: "orders",
                schema,
            },
        }) if schema == SchemaId::from("orders")
    ));
}

#[test]
fn equal_keyed_definition_invalidated_by_schema_edit_is_rejected_on_after_side() {
    let mut before = Document::empty("doc", "title");
    before.schemas.insert(
        "orders".into(),
        numeric_or_text_schema(
            "orders",
            &[("lookup", FieldType::Text), ("quantity", FieldType::Number)],
        ),
    );
    before.schemas.insert(
        "products".into(),
        numeric_or_text_schema(
            "products",
            &[
                ("key", FieldType::Text),
                ("category", FieldType::Text),
                ("price", FieldType::Number),
            ],
        ),
    );
    before
        .keyed_grouped_sum_definitions
        .insert("summary".into(), keyed_definition());
    let mut after = before.clone();
    after
        .schemas
        .get_mut("orders")
        .unwrap()
        .fields
        .remove("lookup");
    assert!(matches!(
        canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &before, &after),
        Err(CanonicalDeltaError::InvalidKeyedGroupedSumDefinitions {
            side: DeltaInputSide::After,
            source: KeyedGroupedSumDefinitionError::MissingField {
                role: KeyedGroupedSumBindingRole::OrdersLookupKey,
                schema,
                field,
            },
        }) if schema == SchemaId::from("orders") && field == FieldId::from("lookup")
    ));
}
