use std::collections::BTreeMap;

use tachiko_formula_engine::{CalculationError, calculate};
use tachiko_semantic_core::{
    Document, Entity, Expression, FieldConstraint, FieldDefinition, FieldId, FieldType,
    KeyedGroupedSumBindingRole, KeyedGroupedSumDefinition, KeyedGroupedSumDefinitionError,
    KeyedGroupedSumDefinitionId, KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding,
    Number, Schema, SchemaId, Value,
};

use crate::DiffScope;

use super::{
    CANONICAL_SEMANTIC_DELTA_V2, CanonicalDeltaError, CanonicalDirectFact as Fact,
    CanonicalSemanticDelta, DeltaInputSide, canonical_delta, canonical_delta_scoped,
    canonical_delta_with_calculations,
};

fn number(value: f64) -> Number {
    Number::new(value).unwrap()
}

fn text_schema(id: &str) -> Schema {
    Schema {
        field_order: None,
        id: id.into(),
        key: "schema".into(),
        display_name: None,
        fields: BTreeMap::from([(
            "f".into(),
            FieldDefinition {
                id: "f".into(),
                key: "f".into(),
                display_name: None,
                number_format: None,
                date_format: None,
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
        field_order: None,
        id: id.into(),
        key: id.into(),
        display_name: None,
        fields: fields
            .iter()
            .map(|(field_id, field_type)| {
                (
                    (*field_id).into(),
                    FieldDefinition {
                        id: (*field_id).into(),
                        key: (*field_id).into(),
                        display_name: None,
                        number_format: None,
                        date_format: None,
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
        field_order: None,
        id: "s".into(),
        key: "s".into(),
        display_name: None,
        fields: BTreeMap::from([(
            "n".into(),
            FieldDefinition {
                id: "n".into(),
                key: "n".into(),
                display_name: None,
                number_format: None,
                date_format: None,
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
fn invalid_created_definition_refuses_the_whole_delta() {
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
        Err(CanonicalDeltaError::InvalidKeyedGroupedSumDefinitions {
            side: DeltaInputSide::After,
            ..
        })
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

#[test]
fn saved_definition_ranks_append_without_moving_names_or_constraints() {
    let payload = super::KeyedGroupedSumDefinitionPayload::from(&keyed_definition());
    for (fact, rank) in [
        (
            Fact::KeyedGroupedSumDefinitionCreated {
                definition: "summary".into(),
                after: payload.clone(),
            },
            0,
        ),
        (
            Fact::KeyedGroupedSumDefinitionDeleted {
                definition: "summary".into(),
                before: payload.clone(),
            },
            1,
        ),
        (
            Fact::KeyedGroupedSumDefinitionChanged {
                definition: "summary".into(),
                before: payload.clone(),
                after: payload,
            },
            2,
        ),
    ] {
        assert_eq!(fact.ordering_key(), (5, vec!["summary"], rank));
    }
    assert_eq!(
        Fact::SchemaDisplayNameChanged {
            schema: "s".into(),
            before: None,
            after: Some("Name".into())
        }
        .ordering_key(),
        (1, vec!["s"], 3)
    );
    assert_eq!(
        Fact::SchemaFieldConstraintChanged {
            schema: "s".into(),
            field: "f".into(),
            before: FieldConstraint::None,
            after: FieldConstraint::None
        }
        .ordering_key(),
        (2, vec!["s", "f"], 5)
    );
    assert_eq!(
        Fact::SchemaFieldDisplayNameChanged {
            schema: "s".into(),
            field: "f".into(),
            before: None,
            after: Some("Name".into())
        }
        .ordering_key(),
        (2, vec!["s", "f"], 6)
    );
    assert_eq!(
        Fact::EntityFieldValueCreated {
            entity: "e".into(),
            schema: "s".into(),
            field: "f".into(),
            after: Value::Text("x".into())
        }
        .ordering_key()
        .0,
        4
    );
}

#[test]
fn a_reorder_alone_is_one_field_order_fact_with_both_orders() {
    let mut before = Document::empty("doc", "title");
    before.schemas.insert(
        "s".into(),
        numeric_or_text_schema("s", &[("a", FieldType::Number), ("b", FieldType::Text)]),
    );
    let mut after = before.clone();
    let reordered = Some(vec![FieldId::from("b"), FieldId::from("a")]);
    after
        .schemas
        .get_mut("s")
        .unwrap()
        .field_order
        .clone_from(&reordered);
    let delta = canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &before, &after).unwrap();
    assert_eq!(
        delta.facts(),
        [Fact::SchemaFieldOrderChanged {
            schema: SchemaId::from("s"),
            before: None,
            after: reordered.clone(),
        }]
    );
    let reverse = canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &after, &before).unwrap();
    assert_eq!(
        reverse.facts(),
        [Fact::SchemaFieldOrderChanged {
            schema: SchemaId::from("s"),
            before: reordered,
            after: None,
        }]
    );
}

// --- Caller-calculated input (#678) ---

fn value_document(value: &str) -> Document {
    let mut document = doc_with_schemas(&["s"]);
    document.entities.insert(
        "e".into(),
        Entity {
            id: "e".into(),
            key: "e".into(),
            schema: "s".into(),
            fields: BTreeMap::from([("f".into(), Value::Text(value.into()))]),
        },
    );
    document
}

/// Both saved-definition roles resolve, and only the after side is bound.
fn keyed_definition_documents() -> (Document, Document) {
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
    let mut after = before.clone();
    after
        .keyed_grouped_sum_definitions
        .insert("summary".into(), keyed_definition());
    (before, after)
}

fn compare_with_calculations(
    contract: &str,
    before: &Document,
    after: &Document,
) -> Result<CanonicalSemanticDelta, CanonicalDeltaError> {
    let before_calculation = calculate(before).unwrap();
    let after_calculation = calculate(after).unwrap();
    canonical_delta_with_calculations(
        contract,
        before,
        after,
        &before_calculation,
        &after_calculation,
    )
}

fn assert_same_delta_outcome(contract: &str, before: &Document, after: &Document) {
    let expected = canonical_delta(contract, before, after);
    let observed = compare_with_calculations(contract, before, after);
    match (expected, observed) {
        (Ok(expected), Ok(observed)) => assert_eq!(expected, observed),
        (Err(expected), Err(observed)) => {
            assert_eq!(format!("{expected:?}"), format!("{observed:?}"));
        }
        (expected, observed) => {
            panic!("outcomes differ: {expected:?} vs {observed:?}");
        }
    }
}

/// Precomputed calculations keep every existing document pair's facts identical.
#[test]
fn canonical_delta_with_calculations_equals_canonical_delta() {
    let empty = doc_with_schemas(&[]);
    let ordered = doc_with_schemas(&["aa", "a"]);
    let (keyed_before, keyed_after) = keyed_definition_documents();
    for (before, after) in [
        (empty.clone(), ordered.clone()),
        (ordered, empty),
        (value_document("old"), value_document("new")),
        (keyed_before, keyed_after),
    ] {
        assert_same_delta_outcome(CANONICAL_SEMANTIC_DELTA_V2, &before, &after);
        assert_same_delta_outcome(CANONICAL_SEMANTIC_DELTA_V2, &after, &before);
    }
}

/// The first-half refusals are unchanged when calculations are supplied.
#[test]
fn canonical_delta_with_calculations_keeps_first_half_refusals() {
    let before = Document::empty("doc", "title");
    let after = Document::empty("doc", "title");
    let other_id = Document::empty("other", "title");
    for (contract, left, right) in [
        ("tachiko.semantic-delta/v1", &before, &after),
        (CANONICAL_SEMANTIC_DELTA_V2, &before, &other_id),
    ] {
        assert_same_delta_outcome(contract, left, right);
    }
}

/// The second-half refusals are unchanged when calculations are supplied.
#[test]
fn canonical_delta_with_calculations_keeps_second_half_refusals() {
    let mut duplicate_constraint = Document::empty("doc", "title");
    let mut schema = text_schema("s");
    schema.fields.get_mut("f").unwrap().constraint = FieldConstraint::TextLiteralSet {
        values: vec!["same".into(), "same".into()],
    };
    duplicate_constraint.schemas.insert("s".into(), schema);

    let valid = Document::empty("doc", "title");
    let mut invalid_definition = Document::empty("doc", "title");
    invalid_definition
        .keyed_grouped_sum_definitions
        .insert("summary".into(), keyed_definition());

    for (before, after) in [
        (&duplicate_constraint, &valid),
        (&valid, &invalid_definition),
    ] {
        assert_same_delta_outcome(CANONICAL_SEMANTIC_DELTA_V2, before, after);
    }
}

// --- Scoped input (#678) ---

fn compare_scoped(
    before: &Document,
    after: &Document,
    scope: &DiffScope,
) -> CanonicalSemanticDelta {
    let before_calculation = calculate(before).unwrap();
    let after_calculation = calculate(after).unwrap();
    canonical_delta_scoped(
        CANONICAL_SEMANTIC_DELTA_V2,
        before,
        after,
        &before_calculation,
        &after_calculation,
        scope,
    )
    .unwrap()
}

fn orders_entity(value: &str) -> Entity {
    Entity {
        id: "e".into(),
        key: "e".into(),
        schema: "orders".into(),
        fields: BTreeMap::from([("lookup".into(), Value::Text(value.into()))]),
    }
}

/// A scope naming ids neither document has changes nothing, and both scopes
/// still equal the full delta, including the title and saved definitions the
/// scope does not name.
#[test]
fn canonical_delta_scoped_ignores_absent_ids() {
    let (mut before, mut after) = keyed_definition_documents();
    before.entities.insert("e".into(), orders_entity("old"));
    after.entities.insert("e".into(), orders_entity("new"));
    after.title = "renamed".into();
    after.schemas.get_mut("orders").unwrap().display_name = Some("Orders".into());

    let exact = compare_scoped(
        &before,
        &after,
        &DiffScope {
            schemas: ["orders".into()].into(),
            entities: ["e".into()].into(),
        },
    );
    let loose = compare_scoped(
        &before,
        &after,
        &DiffScope {
            schemas: ["orders".into(), "absent".into()].into(),
            entities: ["e".into(), "absent".into()].into(),
        },
    );
    assert_eq!(loose, exact);
    assert_eq!(
        exact,
        compare_with_calculations(CANONICAL_SEMANTIC_DELTA_V2, &before, &after).unwrap()
    );
    assert_eq!(
        exact.facts(),
        [
            Fact::DocumentTitleChanged {
                document: "doc".into(),
                before: "title".into(),
                after: "renamed".into(),
            },
            Fact::SchemaDisplayNameChanged {
                schema: "orders".into(),
                before: None,
                after: Some("Orders".into()),
            },
            Fact::EntityFieldValueChanged {
                entity: "e".into(),
                schema: "orders".into(),
                field: "lookup".into(),
                before: Value::Text("old".into()),
                after: Value::Text("new".into()),
            },
            Fact::KeyedGroupedSumDefinitionCreated {
                definition: "summary".into(),
                after: (&keyed_definition()).into(),
            },
        ]
    );

    // The reverse pair reports the deletion, so an absent id stays silent in
    // both directions.
    let reverse = compare_scoped(
        &after,
        &before,
        &DiffScope {
            schemas: ["orders".into(), "absent".into()].into(),
            entities: ["e".into(), "absent".into()].into(),
        },
    );
    assert_eq!(
        reverse,
        compare_with_calculations(CANONICAL_SEMANTIC_DELTA_V2, &after, &before).unwrap()
    );
}

#[test]
fn date_format_is_a_direct_field_fact_for_set_change_and_clear() {
    use tachiko_semantic_core::DateFormat;
    let mut before = doc_with_schemas(&["s"]);
    before
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("f")
        .unwrap()
        .field_type = FieldType::Date;
    assert!(
        canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &before, &before)
            .unwrap()
            .facts()
            .is_empty()
    );
    for (old, new) in [
        (None, Some(DateFormat::Mdy)),
        (Some(DateFormat::Mdy), Some(DateFormat::Dmy)),
        (Some(DateFormat::Dmy), None),
    ] {
        before
            .schemas
            .get_mut("s")
            .unwrap()
            .fields
            .get_mut("f")
            .unwrap()
            .date_format = old;
        let mut after = before.clone();
        after
            .schemas
            .get_mut("s")
            .unwrap()
            .fields
            .get_mut("f")
            .unwrap()
            .date_format = new;
        assert_eq!(
            canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &before, &after)
                .unwrap()
                .facts(),
            &[Fact::SchemaFieldDateFormatChanged {
                schema: "s".into(),
                field: "f".into(),
                before: old,
                after: new
            }]
        );
    }
}
