//! Steward-owned acceptance for Issue #450's explicitly selected logical delta.
use std::collections::BTreeMap;

use tachiko_diff_engine::{
    CANONICAL_SEMANTIC_DELTA_V2, CanonicalDeltaError, CanonicalDirectFact as Fact, DeltaInputSide,
    EntityDefinitionPayload, FieldDefinitionPayload, SchemaDefinitionPayload, canonical_delta,
    diff,
};
use tachiko_formula_engine::calculate;
use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, Expression, FieldConstraint, FieldDefinition, FieldId,
    FieldRef, FieldType, KeyedGroupedSumBindingRole, KeyedGroupedSumDefinition,
    KeyedGroupedSumDefinitionError, KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding,
    Number, Schema, SchemaId, Value, validate_complete_formula_constraints, validate_document_core,
    validate_keyed_grouped_sum_definitions,
};

fn number(value: f64) -> Number {
    Number::new(value).unwrap()
}

fn range(min: f64, max: f64) -> FieldConstraint {
    FieldConstraint::NumberInclusiveRange {
        min: number(min),
        max: number(max),
    }
}

fn field(id: &str, field_type: FieldType) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        field_type,
        required: false,
        constraint: FieldConstraint::None,
    }
}

fn field_payload(definition: &FieldDefinition) -> FieldDefinitionPayload {
    FieldDefinitionPayload {
        key: definition.key.clone(),
        field_type: definition.field_type.clone(),
        required: definition.required,
        constraint: definition.constraint.clone(),
    }
}

fn schema_payload(schema: &Schema) -> SchemaDefinitionPayload {
    SchemaDefinitionPayload {
        key: schema.key.clone(),
        fields: schema
            .fields
            .iter()
            .map(|(id, definition)| (id.clone(), field_payload(definition)))
            .collect(),
    }
}

fn entity_payload(entity: &Entity) -> EntityDefinitionPayload {
    EntityDefinitionPayload {
        key: entity.key.clone(),
        schema: entity.schema.clone(),
        fields: entity.fields.clone(),
    }
}

fn document() -> Document {
    Document {
        id: "delta-acceptance".into(),
        title: "Before".to_owned(),
        schemas: BTreeMap::from([(
            "schema".into(),
            Schema {
                id: "schema".into(),
                key: "table".into(),
                fields: BTreeMap::from([
                    ("amount".into(), field("amount", FieldType::Number)),
                    ("label".into(), field("label", FieldType::Text)),
                ]),
            },
        )]),
        entities: BTreeMap::from([(
            "entity".into(),
            Entity {
                id: "entity".into(),
                key: "row".into(),
                schema: "schema".into(),
                fields: BTreeMap::from([
                    ("amount".into(), Value::Number(number(2.0))),
                    ("label".into(), Value::Text("old".to_owned())),
                ]),
            },
        )]),
        keyed_grouped_sum_definitions: BTreeMap::new(),
    }
}

fn valid_keyed_definition() -> KeyedGroupedSumDefinition {
    KeyedGroupedSumDefinition {
        id: "summary".into(),
        orders: KeyedGroupedSumOrdersBinding {
            schema: "schema".into(),
            lookup_key_field: "label".into(),
            quantity_field: "amount".into(),
        },
        products: KeyedGroupedSumProductsBinding {
            schema: "schema".into(),
            key_field: "label".into(),
            category_field: "label".into(),
            price_field: "amount".into(),
        },
    }
}

fn facts(before: &Document, after: &Document) -> Vec<Fact> {
    let delta = canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, before, after).unwrap();
    assert_eq!(delta.contract(), "tachiko.semantic-delta/v2");
    assert_eq!(delta.document_id(), &before.id);
    delta.facts().to_vec()
}

#[test]
fn equal_intrinsically_invalid_keyed_definitions_refuse_before_emitting_title_fact() {
    let mut before = document();
    let mut invalid = valid_keyed_definition();
    invalid.orders.schema = "missing".into();
    before
        .keyed_grouped_sum_definitions
        .insert("summary".into(), invalid);
    let mut after = before.clone();
    after.title = "After".into();
    assert!(matches!(
        canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &before, &after),
        Err(CanonicalDeltaError::InvalidKeyedGroupedSumDefinitions {
            side: DeltaInputSide::Before,
            source: KeyedGroupedSumDefinitionError::MissingSchema {
                role: "orders",
                schema,
            },
        }) if schema == SchemaId::from("missing")
    ));
}

#[test]
fn unchanged_keyed_definition_binding_is_checked_against_both_schema_snapshots() {
    let mut before = document();
    before
        .keyed_grouped_sum_definitions
        .insert("summary".into(), valid_keyed_definition());
    assert!(validate_keyed_grouped_sum_definitions(&before).is_ok());

    let mut valid_title_change = before.clone();
    valid_title_change.title = "After".into();
    assert_eq!(
        facts(&before, &valid_title_change),
        vec![Fact::DocumentTitleChanged {
            document: before.id.clone(),
            before: "Before".into(),
            after: "After".into(),
        }]
    );

    let mut after = before.clone();
    after
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .remove("amount");
    after
        .entities
        .get_mut("entity")
        .unwrap()
        .fields
        .remove("amount");
    assert!(validate_document_core(&after).is_empty());
    assert!(matches!(
        canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &before, &after),
        Err(CanonicalDeltaError::InvalidKeyedGroupedSumDefinitions {
            side: DeltaInputSide::After,
            source: KeyedGroupedSumDefinitionError::MissingField {
                role: KeyedGroupedSumBindingRole::OrdersQuantity,
                schema,
                field,
            },
        }) if schema == SchemaId::from("schema") && field == FieldId::from("amount")
    ));
}

#[test]
fn independent_properties_are_ranked_after_typed_targets() {
    let before = document();
    let mut after = before.clone();
    after.title = "After".to_owned();
    let schema = after.schemas.get_mut("schema").unwrap();
    schema.key = "renamed_table".into();
    let amount = schema.fields.get_mut("amount").unwrap();
    amount.key = "renamed_amount".into();
    amount.required = true;
    amount.constraint = range(0.0, 10.0);
    let label = schema.fields.get_mut("label").unwrap();
    label.key = "renamed_label".into();
    label.field_type = FieldType::Boolean;
    let entity = after.entities.get_mut("entity").unwrap();
    entity.key = "renamed_row".into();
    entity
        .fields
        .insert("amount".into(), Value::Number(number(3.0)));
    entity.fields.insert("label".into(), Value::Boolean(false));

    assert_eq!(
        facts(&before, &after),
        vec![
            Fact::DocumentTitleChanged {
                document: before.id.clone(),
                before: "Before".into(),
                after: "After".into()
            },
            Fact::SchemaKeyChanged {
                schema: "schema".into(),
                before: "table".into(),
                after: "renamed_table".into()
            },
            Fact::SchemaFieldKeyChanged {
                schema: "schema".into(),
                field: "amount".into(),
                before: "amount".into(),
                after: "renamed_amount".into()
            },
            Fact::SchemaFieldRequirednessChanged {
                schema: "schema".into(),
                field: "amount".into(),
                before: false,
                after: true
            },
            Fact::SchemaFieldConstraintChanged {
                schema: "schema".into(),
                field: "amount".into(),
                before: FieldConstraint::None,
                after: range(0.0, 10.0)
            },
            Fact::SchemaFieldKeyChanged {
                schema: "schema".into(),
                field: "label".into(),
                before: "label".into(),
                after: "renamed_label".into()
            },
            Fact::SchemaFieldTypeChanged {
                schema: "schema".into(),
                field: "label".into(),
                before: FieldType::Text,
                after: FieldType::Boolean
            },
            Fact::EntityKeyChanged {
                entity: "entity".into(),
                before: "row".into(),
                after: "renamed_row".into()
            },
            Fact::EntityFieldValueChanged {
                entity: "entity".into(),
                schema: "schema".into(),
                field: "amount".into(),
                before: Value::Number(number(2.0)),
                after: Value::Number(number(3.0))
            },
            Fact::EntityFieldValueChanged {
                entity: "entity".into(),
                schema: "schema".into(),
                field: "label".into(),
                before: Value::Text("old".into()),
                after: Value::Boolean(false)
            },
        ]
    );
}

#[test]
fn parent_creation_and_deletion_have_complete_payloads_and_suppress_children() {
    let mut empty = document();
    empty.schemas.clear();
    empty.entities.clear();
    let mut full = document();
    full.schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("amount")
        .unwrap()
        .constraint = range(0.0, 5.0);
    assert_eq!(
        facts(&empty, &full),
        vec![
            Fact::SchemaCreated {
                schema: "schema".into(),
                definition: schema_payload(&full.schemas["schema"])
            },
            Fact::EntityCreated {
                entity: "entity".into(),
                definition: entity_payload(&full.entities["entity"])
            },
        ]
    );
    assert_eq!(
        facts(&full, &empty),
        vec![
            Fact::SchemaDeleted {
                schema: "schema".into(),
                definition: schema_payload(&full.schemas["schema"])
            },
            Fact::EntityDeleted {
                entity: "entity".into(),
                definition: entity_payload(&full.entities["entity"])
            },
        ]
    );
    assert_eq!(
        schema_payload(&full.schemas["schema"]).fields["label"].constraint,
        FieldConstraint::None
    );
}

#[test]
fn continuing_parent_field_replacement_uses_complete_definitions_and_values() {
    let before = document();
    let mut after = before.clone();
    let schema = after.schemas.get_mut("schema").unwrap();
    schema.fields.remove("label");
    let mut replacement = field("replacement", FieldType::Text);
    replacement.key = "label".into();
    replacement.constraint = FieldConstraint::TextLiteralSet {
        values: vec!["new".into()],
    };
    schema
        .fields
        .insert("replacement".into(), replacement.clone());
    let entity = after.entities.get_mut("entity").unwrap();
    entity.fields.remove("label");
    entity
        .fields
        .insert("replacement".into(), Value::Text("new".into()));
    assert_eq!(
        facts(&before, &after),
        vec![
            Fact::SchemaFieldDeleted {
                schema: "schema".into(),
                field: "label".into(),
                definition: field_payload(&before.schemas["schema"].fields["label"])
            },
            Fact::SchemaFieldCreated {
                schema: "schema".into(),
                field: "replacement".into(),
                definition: field_payload(&replacement)
            },
            Fact::EntityFieldValueCleared {
                entity: "entity".into(),
                schema: "schema".into(),
                field: "label".into(),
                before: Value::Text("old".into())
            },
            Fact::EntityFieldValueCreated {
                entity: "entity".into(),
                schema: "schema".into(),
                field: "replacement".into(),
                after: Value::Text("new".into())
            },
        ]
    );
}

#[test]
fn schema_transition_never_collapses_equal_field_text_and_value() {
    let mut before = document();
    let mut other = before.schemas["schema"].clone();
    other.id = "other".into();
    other.key = "other".into();
    before.schemas.insert(other.id.clone(), other);
    let mut after = before.clone();
    after.entities.get_mut("entity").unwrap().schema = "other".into();
    assert_eq!(
        facts(&before, &after),
        vec![
            Fact::EntitySchemaChanged {
                entity: "entity".into(),
                before: "schema".into(),
                after: "other".into()
            },
            Fact::EntityFieldValueCreated {
                entity: "entity".into(),
                schema: "other".into(),
                field: "amount".into(),
                after: Value::Number(number(2.0))
            },
            Fact::EntityFieldValueCreated {
                entity: "entity".into(),
                schema: "other".into(),
                field: "label".into(),
                after: Value::Text("old".into())
            },
            Fact::EntityFieldValueCleared {
                entity: "entity".into(),
                schema: "schema".into(),
                field: "amount".into(),
                before: Value::Number(number(2.0))
            },
            Fact::EntityFieldValueCleared {
                entity: "entity".into(),
                schema: "schema".into(),
                field: "label".into(),
                before: Value::Text("old".into())
            },
        ]
    );
}

#[test]
fn constraint_only_change_is_one_atomic_fact_and_equal_state_is_empty() {
    let mut before = document();
    before
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("amount")
        .unwrap()
        .constraint = range(0.0, 5.0);
    let mut after = before.clone();
    after
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("amount")
        .unwrap()
        .constraint = range(1.0, 4.0);
    assert_eq!(
        facts(&before, &after),
        vec![Fact::SchemaFieldConstraintChanged {
            schema: "schema".into(),
            field: "amount".into(),
            before: range(0.0, 5.0),
            after: range(1.0, 4.0),
        }]
    );
    assert!(facts(&after, &after).is_empty());
    assert_eq!(
        canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &before, &after).unwrap(),
        canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &before.clone(), &after.clone()).unwrap()
    );
}

#[test]
fn independently_constructed_equal_text_sets_are_empty_and_changes_are_atomic() {
    let mut before = document();
    let mut after = document();
    let before_set = FieldConstraint::TextLiteralSet {
        values: vec![String::new(), "old".into(), "é".into()],
    };
    before
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("label")
        .unwrap()
        .constraint = before_set.clone();
    after
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("label")
        .unwrap()
        .constraint = FieldConstraint::TextLiteralSet {
        values: ["", "old", "é"].into_iter().map(str::to_owned).collect(),
    };
    assert!(facts(&before, &after).is_empty());
    let after_set = FieldConstraint::TextLiteralSet {
        values: vec!["old".into(), "Ω".into()],
    };
    after
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("label")
        .unwrap()
        .constraint = after_set.clone();
    assert_eq!(
        facts(&before, &after),
        vec![Fact::SchemaFieldConstraintChanged {
            schema: "schema".into(),
            field: "label".into(),
            before: before_set,
            after: after_set,
        }]
    );
}

fn formula_document() -> Document {
    let mut document = document();
    let mut total = field("total", FieldType::Number);
    total.constraint = range(0.0, 10.0);
    document
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .insert("total".into(), total);
    document.entities.get_mut("entity").unwrap().fields.insert(
        "total".into(),
        Value::Formula(Expression::Multiply {
            left: Box::new(Expression::Reference(FieldRef::new("entity", "amount"))),
            right: Box::new(Expression::Number(number(2.0))),
        }),
    );
    document
}

#[test]
fn calculated_impact_is_excluded_but_bound_expression_change_is_direct() {
    let before = formula_document();
    let mut after = before.clone();
    after
        .entities
        .get_mut("entity")
        .unwrap()
        .fields
        .insert("amount".into(), Value::Number(number(3.0)));
    assert_eq!(
        facts(&before, &after),
        vec![Fact::EntityFieldValueChanged {
            entity: "entity".into(),
            schema: "schema".into(),
            field: "amount".into(),
            before: Value::Number(number(2.0)),
            after: Value::Number(number(3.0)),
        }]
    );
    let old_formula = after.entities["entity"].fields["total"].clone();
    let mut expression_change = after.clone();
    let new_formula = Value::Formula(Expression::Number(number(6.0)));
    expression_change
        .entities
        .get_mut("entity")
        .unwrap()
        .fields
        .insert("total".into(), new_formula.clone());
    assert_eq!(
        facts(&after, &expression_change),
        vec![Fact::EntityFieldValueChanged {
            entity: "entity".into(),
            schema: "schema".into(),
            field: "total".into(),
            before: old_formula,
            after: new_formula,
        }]
    );
}

#[test]
fn unsupported_selector_and_different_document_never_produce_canonical_facts() {
    let before = document();
    let mut after = before.clone();
    after.id = DocumentId::from("different");
    for selector in ["tachiko.semantic-delta/v1", "tachiko.semantic-delta/v3", ""] {
        assert!(matches!(
            canonical_delta(selector, &before, &after),
            Err(CanonicalDeltaError::UnsupportedContract { .. })
        ));
    }
    assert!(matches!(
        canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &before, &after),
        Err(CanonicalDeltaError::DocumentIdMismatch { .. })
    ));
    // Existing provisional evidence remains available and is not relabeled canonical.
    assert!(
        diff(&before, &after)
            .unwrap()
            .render_text()
            .contains("different")
    );
}

#[test]
fn declaration_direct_value_and_complete_formula_admission_apply_to_both_sides() {
    let valid = formula_document();
    let mut declaration = valid.clone();
    declaration
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("amount")
        .unwrap()
        .constraint = range(5.0, 1.0);
    let mut direct_value = valid.clone();
    direct_value
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("amount")
        .unwrap()
        .constraint = range(0.0, 1.0);
    let mut calculated_value = valid.clone();
    calculated_value
        .entities
        .get_mut("entity")
        .unwrap()
        .fields
        .insert("amount".into(), Value::Number(number(6.0)));
    for invalid in [declaration, direct_value, calculated_value] {
        let mut expected = validate_document_core(&invalid);
        if expected.is_empty() {
            let complete = calculate(&invalid).unwrap();
            expected = validate_complete_formula_constraints(&invalid, complete.values());
        }
        let expected: Vec<_> = expected
            .iter()
            .map(tachiko_semantic_core::Diagnostic::stable_observation)
            .collect();
        assert!(!expected.is_empty());
        for (before, after, expected_side) in [
            (&valid, &invalid, DeltaInputSide::After),
            (&invalid, &valid, DeltaInputSide::Before),
        ] {
            let error = canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, before, after).unwrap_err();
            let CanonicalDeltaError::InvalidInput { side, diagnostics } = error else {
                panic!("wrong refusal: {error:?}");
            };
            assert_eq!(side, expected_side);
            let observed: Vec<_> = diagnostics
                .iter()
                .map(tachiko_semantic_core::Diagnostic::stable_observation)
                .collect();
            assert_eq!(observed, expected);
        }
    }
}

#[test]
fn stable_id_replacement_is_delete_create_even_when_mutable_contents_match() {
    let before = document();
    let mut after = before.clone();
    let mut entity = after.entities.remove("entity").unwrap();
    entity.id = EntityId::from("replacement");
    after.entities.insert(entity.id.clone(), entity);
    assert_eq!(
        facts(&before, &after),
        vec![
            Fact::EntityDeleted {
                entity: "entity".into(),
                definition: entity_payload(&before.entities["entity"])
            },
            Fact::EntityCreated {
                entity: "replacement".into(),
                definition: entity_payload(&after.entities["replacement"])
            },
        ]
    );
}

#[test]
fn independent_field_type_requiredness_key_and_constraint_changes_share_one_target() {
    let mut before = document();
    before.entities.clear();
    let definition = before
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("label")
        .unwrap();
    definition.constraint = FieldConstraint::TextLiteralSet {
        values: vec![String::new(), "old".into()],
    };
    let mut after = before.clone();
    let definition = after
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("label")
        .unwrap();
    definition.key = "numeric".into();
    definition.field_type = FieldType::Number;
    definition.required = true;
    definition.constraint = range(-1.0, 1.0);
    let schema = SchemaId::from("schema");
    let field = FieldId::from("label");
    assert_eq!(
        facts(&before, &after),
        vec![
            Fact::SchemaFieldKeyChanged {
                schema: schema.clone(),
                field: field.clone(),
                before: "label".into(),
                after: "numeric".into()
            },
            Fact::SchemaFieldTypeChanged {
                schema: schema.clone(),
                field: field.clone(),
                before: FieldType::Text,
                after: FieldType::Number
            },
            Fact::SchemaFieldRequirednessChanged {
                schema: schema.clone(),
                field: field.clone(),
                before: false,
                after: true
            },
            Fact::SchemaFieldConstraintChanged {
                schema,
                field,
                before: FieldConstraint::TextLiteralSet {
                    values: vec![String::new(), "old".into()]
                },
                after: range(-1.0, 1.0)
            },
        ]
    );
}
