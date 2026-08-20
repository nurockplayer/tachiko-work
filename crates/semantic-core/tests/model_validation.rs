use std::collections::BTreeMap;

use tachiko_semantic_core::{
    DiagnosticCode, Document, DocumentId, Entity, EntityId, Expression, FieldDefinition, FieldId,
    FieldRef, FieldType, Schema, SchemaId, Value, validate_document,
};

fn valid_document() -> Document {
    let weapon_schema = Schema {
        id: SchemaId::from("weapon"),
        fields: BTreeMap::from([
            (
                FieldId::from("damage"),
                FieldDefinition {
                    field_type: FieldType::Number,
                    required: true,
                },
            ),
            (
                FieldId::from("name"),
                FieldDefinition {
                    field_type: FieldType::Text,
                    required: true,
                },
            ),
            (
                FieldId::from("upgrade_from"),
                FieldDefinition {
                    field_type: FieldType::Reference {
                        schema: SchemaId::from("weapon"),
                    },
                    required: false,
                },
            ),
            (
                FieldId::from("dps"),
                FieldDefinition {
                    field_type: FieldType::Number,
                    required: true,
                },
            ),
        ]),
    };

    let sword = Entity {
        id: EntityId::from("sword"),
        schema: SchemaId::from("weapon"),
        fields: BTreeMap::from([
            (FieldId::from("damage"), Value::Number(100.0)),
            (FieldId::from("name"), Value::Text("Sword".to_owned())),
            (
                FieldId::from("dps"),
                Value::Formula(Expression::Divide {
                    left: Box::new(Expression::Reference(FieldRef::new("sword", "damage"))),
                    right: Box::new(Expression::Number(1.25)),
                }),
            ),
        ]),
    };

    Document {
        id: DocumentId::from("balance"),
        title: "Balance".to_owned(),
        schemas: BTreeMap::from([(SchemaId::from("weapon"), weapon_schema)]),
        entities: BTreeMap::from([(EntityId::from("sword"), sword)]),
    }
}

#[test]
fn valid_typed_document_has_no_diagnostics() {
    assert!(validate_document(&valid_document()).is_empty());
}

#[test]
fn missing_required_field_is_rejected() {
    let mut document = valid_document();
    document
        .entities
        .get_mut("sword")
        .unwrap()
        .fields
        .remove("name");

    let diagnostics = validate_document(&document);

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::MissingRequiredField
            && diagnostic.path == "entities.sword.fields.name"
    }));
}

#[test]
fn wrong_value_kind_is_rejected() {
    let mut document = valid_document();
    document
        .entities
        .get_mut("sword")
        .unwrap()
        .fields
        .insert(FieldId::from("damage"), Value::Text("high".to_owned()));

    let diagnostics = validate_document(&document);

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::TypeMismatch
            && diagnostic.path == "entities.sword.fields.damage"
    }));
}

#[test]
fn broken_typed_reference_is_rejected() {
    let mut document = valid_document();
    document.entities.get_mut("sword").unwrap().fields.insert(
        FieldId::from("upgrade_from"),
        Value::Reference(EntityId::from("missing_weapon")),
    );

    let diagnostics = validate_document(&document);

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::MissingReference
            && diagnostic.path == "entities.sword.fields.upgrade_from"
    }));
}

#[test]
fn reference_to_wrong_schema_is_rejected() {
    let mut document = valid_document();
    document.schemas.insert(
        SchemaId::from("character"),
        Schema {
            id: SchemaId::from("character"),
            fields: BTreeMap::new(),
        },
    );
    document.entities.insert(
        EntityId::from("hero"),
        Entity {
            id: EntityId::from("hero"),
            schema: SchemaId::from("character"),
            fields: BTreeMap::new(),
        },
    );
    document.entities.get_mut("sword").unwrap().fields.insert(
        FieldId::from("upgrade_from"),
        Value::Reference(EntityId::from("hero")),
    );

    let diagnostics = validate_document(&document);

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::ReferenceTypeMismatch
            && diagnostic.path == "entities.sword.fields.upgrade_from"
    }));
}

#[test]
fn map_key_and_semantic_id_must_match() {
    let mut document = valid_document();
    document.entities.get_mut("sword").unwrap().id = EntityId::from("other");

    let diagnostics = validate_document(&document);

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::KeyMismatch && diagnostic.path == "entities.sword.id"
    }));
}

#[test]
fn non_finite_numbers_are_rejected() {
    let mut document = valid_document();
    document
        .entities
        .get_mut("sword")
        .unwrap()
        .fields
        .insert(FieldId::from("damage"), Value::Number(f64::INFINITY));

    let diagnostics = validate_document(&document);

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::NonFiniteNumber
            && diagnostic.path == "entities.sword.fields.damage"
    }));
}

#[test]
fn formula_references_must_target_numeric_fields() {
    let mut document = valid_document();
    document.entities.get_mut("sword").unwrap().fields.insert(
        FieldId::from("dps"),
        Value::Formula(Expression::Reference(FieldRef::new("sword", "name"))),
    );

    let diagnostics = validate_document(&document);

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::FormulaReferenceTypeMismatch
            && diagnostic.path == "entities.sword.fields.dps"
    }));
}

#[test]
fn diagnostics_have_deterministic_ordering() {
    let mut document = valid_document();
    let sword = document.entities.get_mut("sword").unwrap();
    sword.fields.remove("name");
    sword
        .fields
        .insert(FieldId::from("unexpected"), Value::Boolean(true));

    let diagnostics = validate_document(&document);
    let mut sorted = diagnostics.clone();
    sorted.sort();

    assert_eq!(diagnostics, sorted);
}

#[test]
fn identifiers_must_be_unambiguous_stable_paths() {
    let mut document = valid_document();
    document.id = DocumentId::from("balance data");
    let mut sword = document.entities.remove("sword").unwrap();
    sword.id = EntityId::from("sword.damage");
    document
        .entities
        .insert(EntityId::from("sword.damage"), sword);

    let diagnostics = validate_document(&document);

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.path == "id" && diagnostic.message.contains("lowercase identifier")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.path == "entities.sword.damage.id"
            && diagnostic.message.contains("lowercase identifier")
    }));
}
