use std::collections::{BTreeMap, BTreeSet};

use tachiko_formula_engine::{
    CanonicalAuthoringProjectionError, ExpressionComplexityError, FormulaBindError,
    UnboundExpression, bind_expression, extract_dependencies, parse_expression, project_expression,
};
use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, EntityKey, Expression, FieldAddress, FieldDefinition,
    FieldId, FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

fn document() -> Document {
    let schema_id = SchemaId::from("schema-stable");
    let damage_id = FieldId::from("damage-stable");
    let dps_id = FieldId::from("dps-stable");
    let entity_id = EntityId::from("entity-stable");
    Document {
        id: DocumentId::from("document-stable"),
        title: "Balance".to_owned(),
        schemas: BTreeMap::from([(
            schema_id.clone(),
            Schema {
                id: schema_id.clone(),
                key: SchemaKey::from("weapon"),
                fields: BTreeMap::from([
                    (
                        damage_id.clone(),
                        FieldDefinition {
                            id: damage_id.clone(),
                            key: FieldKey::from("damage"),
                            field_type: FieldType::Number,
                            required: true,
                        },
                    ),
                    (
                        dps_id.clone(),
                        FieldDefinition {
                            id: dps_id.clone(),
                            key: FieldKey::from("dps"),
                            field_type: FieldType::Number,
                            required: true,
                        },
                    ),
                ]),
            },
        )]),
        entities: BTreeMap::from([(
            entity_id.clone(),
            Entity {
                id: entity_id,
                key: EntityKey::from("iron_sword"),
                schema: schema_id,
                fields: BTreeMap::from([
                    (damage_id, Value::Number(Number::new(50.0).unwrap())),
                    (dps_id, Value::Number(Number::new(50.0).unwrap())),
                ]),
            },
        )]),
    }
}

#[test]
fn authoring_addresses_bind_once_to_stable_ids() {
    let document = document();
    let unbound = parse_expression("[iron_sword.damage] + 5").unwrap();

    let bound = bind_expression(&document, &unbound).unwrap();

    let reference = FieldRef::new("entity-stable", "damage-stable");
    assert_eq!(
        bound,
        Expression::Add {
            left: Box::new(Expression::Reference(reference.clone())),
            right: Box::new(Expression::Number(Number::new(5.0).unwrap())),
        }
    );
    assert_eq!(extract_dependencies(&bound), BTreeSet::from([reference]));
}

#[test]
fn addresses_that_resolve_to_non_numeric_fields_are_rejected_exactly() {
    let mut document = document();
    document
        .schemas
        .get_mut("schema-stable")
        .unwrap()
        .fields
        .get_mut("damage-stable")
        .unwrap()
        .field_type = FieldType::Text;
    document
        .entities
        .get_mut("entity-stable")
        .unwrap()
        .fields
        .insert(
            FieldId::from("damage-stable"),
            Value::Text("not a number".to_owned()),
        );

    let error =
        bind_expression(&document, &parse_expression("[iron_sword.damage]").unwrap()).unwrap_err();

    assert_eq!(
        error,
        FormulaBindError::NonNumericTarget {
            address: FieldAddress::new("iron_sword", "damage"),
            reference: FieldRef::new("entity-stable", "damage-stable"),
        }
    );
}

#[test]
fn projection_uses_current_keys_without_rewriting_bound_ast() {
    let mut document = document();
    let bound = bind_expression(
        &document,
        &parse_expression("[iron_sword.damage] + 5").unwrap(),
    )
    .unwrap();
    let before = bound.clone();

    document.entities.get_mut("entity-stable").unwrap().key = EntityKey::from("moonblade");
    document
        .schemas
        .get_mut("schema-stable")
        .unwrap()
        .fields
        .get_mut("damage-stable")
        .unwrap()
        .key = FieldKey::from("power");

    assert_eq!(
        project_expression(&document, &bound).unwrap(),
        "([moonblade.power] + 5)"
    );
    assert_eq!(bound, before);
}

#[test]
fn projection_fails_with_stable_targets_instead_of_retargeting_reused_keys() {
    let mut document = document();
    let target = FieldRef::new("entity-stable", "damage-stable");
    let expression = Expression::Reference(target.clone());
    document.entities.remove("entity-stable");
    document.entities.insert(
        EntityId::from("replacement-stable"),
        Entity {
            id: EntityId::from("replacement-stable"),
            key: EntityKey::from("iron_sword"),
            schema: SchemaId::from("schema-stable"),
            fields: BTreeMap::new(),
        },
    );

    let error = project_expression(&document, &expression).unwrap_err();

    assert_eq!(
        error,
        CanonicalAuthoringProjectionError::UnresolvableBoundReferences {
            targets: BTreeSet::from([target]),
        }
    );
}

#[test]
fn directly_supplied_unbound_expressions_are_bounded_before_recursive_binding() {
    let mut expression = UnboundExpression::Number(Number::new(1.0).unwrap());
    for _ in 0..64 {
        expression = UnboundExpression::Add {
            left: Box::new(expression),
            right: Box::new(UnboundExpression::Number(Number::new(1.0).unwrap())),
        };
    }

    assert_eq!(
        bind_expression(&document(), &expression).unwrap_err(),
        FormulaBindError::Complexity(ExpressionComplexityError::DepthLimit)
    );
}

#[test]
fn incoherent_field_store_returns_a_typed_binding_error_without_panicking() {
    let mut document = document();
    let schema = document.schemas.get_mut("schema-stable").unwrap();
    let mut definition = schema.fields.remove("damage-stable").unwrap();
    definition.id = FieldId::from("different-nested-id");
    schema
        .fields
        .insert(FieldId::from("damage-stable"), definition);
    let unbound = parse_expression("[iron_sword.damage]").unwrap();

    let result = std::panic::catch_unwind(|| bind_expression(&document, &unbound))
        .expect("binding malformed input must not panic");

    assert!(matches!(result, Err(FormulaBindError::Index { .. })));
}
