//! ADR-0044 complete, document-owned definition facts; no wire contract is added.
use tachiko_diff_engine::{
    CANONICAL_SEMANTIC_DELTA_V2 as V2, CanonicalDeltaError, CanonicalDirectFact as Fact,
    DeltaInputSide, KeyedGroupedSumDefinitionPayload as Payload, canonical_delta,
};
use tachiko_semantic_core::*;

fn fixture() -> Document {
    let mut d = Document::empty("doc", "Before");
    for id in ["a", "b", "c"] {
        d.schemas.insert(
            id.into(),
            Schema {
                field_order: None,
                id: id.into(),
                key: id.into(),
                display_name: None,
                fields: [
                    "key",
                    "category",
                    "key2",
                    "category2",
                    "quantity",
                    "price",
                    "quantity2",
                    "price2",
                ]
                .into_iter()
                .map(|id| {
                    (
                        id.into(),
                        FieldDefinition {
                            id: id.into(),
                            key: id.into(),
                            display_name: None,
                            number_format: None,
                            field_type: if id.starts_with("key") || id.starts_with("category") {
                                FieldType::Text
                            } else {
                                FieldType::Number
                            },
                            required: false,
                            constraint: FieldConstraint::None,
                        },
                    )
                })
                .collect(),
            },
        );
    }
    d
}
fn definition(id: &str) -> KeyedGroupedSumDefinition {
    KeyedGroupedSumDefinition {
        id: id.into(),
        orders: KeyedGroupedSumOrdersBinding {
            schema: "a".into(),
            lookup_key_field: "key".into(),
            quantity_field: "quantity".into(),
        },
        products: KeyedGroupedSumProductsBinding {
            schema: "b".into(),
            key_field: "key".into(),
            category_field: "category".into(),
            price_field: "price".into(),
        },
    }
}
fn insert(d: &mut Document, value: KeyedGroupedSumDefinition) {
    d.keyed_grouped_sum_definitions
        .insert(value.id.clone(), value);
}
fn facts(a: &Document, b: &Document) -> Vec<Fact> {
    let delta = canonical_delta(V2, a, b).unwrap();
    assert_eq!(delta.document_id(), &a.id);
    assert_eq!(delta.contract(), V2);
    delta.facts().to_vec()
}
#[test]
fn complete_presence_create_delete_equal_and_each_role_change() {
    let absent = fixture();
    let original = definition("a"); // Same spelling as SchemaId does not alias it.
    let payload = Payload {
        orders: original.orders.clone(),
        products: original.products.clone(),
    };
    let mut present = absent.clone();
    insert(&mut present, original.clone());
    assert_eq!(
        facts(&absent, &present),
        vec![Fact::KeyedGroupedSumDefinitionCreated {
            definition: "a".into(),
            after: payload.clone()
        }]
    );
    assert_eq!(
        facts(&present, &absent),
        vec![Fact::KeyedGroupedSumDefinitionDeleted {
            definition: "a".into(),
            before: payload.clone()
        }]
    );
    assert_eq!(facts(&present, &present), []); // Empty groups are still PRESENT.
    for binding in 0..8 {
        let mut changed = original.clone();
        match binding {
            0 => changed.orders.schema = "c".into(),
            1 => changed.orders.lookup_key_field = "key2".into(),
            2 => changed.orders.quantity_field = "quantity2".into(),
            3 => changed.products.schema = "c".into(),
            4 => changed.products.key_field = "key2".into(),
            5 => changed.products.category_field = "category2".into(),
            6 => changed.products.price_field = "price2".into(),
            7 => {
                changed.orders.schema = "c".into();
                changed.products.key_field = "key2".into();
                changed.products.price_field = "price2".into();
            }
            _ => unreachable!(),
        }
        let mut after = present.clone();
        insert(&mut after, changed.clone());
        let expected = Fact::KeyedGroupedSumDefinitionChanged {
            definition: "a".into(),
            before: payload.clone(),
            after: Payload::from(&changed),
        };
        assert_eq!(
            facts(&present, &after),
            vec![expected.clone()],
            "binding {binding}"
        );
        assert_ne!(facts(&present, &after), facts(&present, &present));
        assert_eq!(facts(&present, &after), vec![expected]);
    }
}
#[test]
fn replacement_is_delete_create_and_ids_use_exact_scalar_order() {
    let mut before = fixture();
    for id in ["a", "z", "é", "\u{e000}"] {
        insert(&mut before, definition(id));
    }
    let mut after = fixture();
    for id in ["aa", "a", "e\u{301}", "\u{10000}"] {
        insert(&mut after, definition(id));
    }
    after
        .keyed_grouped_sum_definitions
        .get_mut(&"a".into())
        .unwrap()
        .products
        .price_field = "price2".into();
    let observed = facts(&before, &after);
    let expected = vec![
        Fact::KeyedGroupedSumDefinitionChanged {
            definition: "a".into(),
            before: Payload::from(&definition("a")),
            after: Payload::from(&after.keyed_grouped_sum_definitions[&"a".into()]),
        },
        Fact::KeyedGroupedSumDefinitionCreated {
            definition: "aa".into(),
            after: Payload::from(&definition("aa")),
        },
        Fact::KeyedGroupedSumDefinitionCreated {
            definition: "e\u{301}".into(),
            after: Payload::from(&definition("e\u{301}")),
        },
        Fact::KeyedGroupedSumDefinitionDeleted {
            definition: "z".into(),
            before: Payload::from(&definition("z")),
        },
        Fact::KeyedGroupedSumDefinitionDeleted {
            definition: "é".into(),
            before: Payload::from(&definition("é")),
        },
        Fact::KeyedGroupedSumDefinitionDeleted {
            definition: "\u{e000}".into(),
            before: Payload::from(&definition("\u{e000}")),
        },
        Fact::KeyedGroupedSumDefinitionCreated {
            definition: "\u{10000}".into(),
            after: Payload::from(&definition("\u{10000}")),
        },
    ];
    assert_eq!(observed, expected);
    let mut replacement = before.clone();
    replacement
        .keyed_grouped_sum_definitions
        .remove(&"z".into());
    insert(&mut replacement, definition("zz"));
    assert_eq!(
        facts(&before, &replacement),
        vec![
            Fact::KeyedGroupedSumDefinitionDeleted {
                definition: "z".into(),
                before: Payload::from(&definition("z"))
            },
            Fact::KeyedGroupedSumDefinitionCreated {
                definition: "zz".into(),
                after: Payload::from(&definition("zz"))
            },
        ]
    );
}
#[test]
fn names_constraints_keys_and_parent_suppression_keep_definition_facts_separate() {
    let mut before = fixture();
    insert(&mut before, definition("summary"));
    let mut after = before.clone();
    let schema = after.schemas.get_mut("a").unwrap();
    schema.key = "renamed_key".into();
    schema.display_name = Some("My Expenses".into());
    let field = schema.fields.get_mut("quantity").unwrap();
    field.display_name = Some("Amount".into());
    field.constraint = FieldConstraint::NumberInclusiveRange {
        min: Number::new(0.0).unwrap(),
        max: Number::new(100.0).unwrap(),
    };
    assert!(matches!(
        facts(&before, &after).as_slice(),
        [
            Fact::SchemaKeyChanged { .. },
            Fact::SchemaDisplayNameChanged { .. },
            Fact::SchemaFieldConstraintChanged { .. },
            Fact::SchemaFieldDisplayNameChanged { .. }
        ]
    ));
    insert(&mut after, definition("new"));
    assert!(matches!(
        facts(&before, &after).last(),
        Some(Fact::KeyedGroupedSumDefinitionCreated { .. })
    ));
    let empty = Document::empty("doc", "Before");
    let created = facts(&empty, &before);
    assert!(matches!(
        created.as_slice(),
        [
            Fact::SchemaCreated { .. },
            Fact::SchemaCreated { .. },
            Fact::SchemaCreated { .. },
            Fact::KeyedGroupedSumDefinitionCreated { .. }
        ]
    ));
    let deleted = facts(&before, &empty);
    assert!(matches!(
        deleted.as_slice(),
        [
            Fact::SchemaDeleted { .. },
            Fact::SchemaDeleted { .. },
            Fact::SchemaDeleted { .. },
            Fact::KeyedGroupedSumDefinitionDeleted { .. }
        ]
    ));
}
#[test]
fn invalid_created_changed_or_removed_definitions_fail_the_whole_request_on_either_side() {
    let valid = fixture();
    for case in 0..7 {
        let mut bad = valid.clone();
        let mut value = definition("summary");
        match case {
            0 => value.orders.schema = "missing".into(),
            1 => value.products.price_field = "missing".into(),
            2 => value.orders.quantity_field = "key".into(),
            3 => value.id = "wrong-map-id".into(),
            4 => value.products.category_field = "quantity".into(),
            5 => value.id = "".into(),
            6 => {
                bad.schemas
                    .get_mut("b")
                    .unwrap()
                    .fields
                    .get_mut("price")
                    .unwrap()
                    .field_type = FieldType::Text;
            }
            _ => unreachable!(),
        }
        bad.keyed_grouped_sum_definitions
            .insert("summary".into(), value);
        bad.title = "No partial title fact".into();
        for (a, b, expected) in [
            (&valid, &bad, DeltaInputSide::After),
            (&bad, &valid, DeltaInputSide::Before),
        ] {
            assert!(
                matches!(canonical_delta(V2, a, b), Err(CanonicalDeltaError::InvalidKeyedGroupedSumDefinitions {side, ..}) if side == expected),
                "case {case}"
            );
        }
    }
    for unsupported in [
        "tachiko.semantic-delta/v1",
        "tachiko.semantic-delta/v3",
        "unknown-fact",
    ] {
        assert!(matches!(
            canonical_delta(unsupported, &valid, &valid),
            Err(CanonicalDeltaError::UnsupportedContract { .. })
        ));
    }
}
