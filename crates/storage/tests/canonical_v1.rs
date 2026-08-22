use std::collections::BTreeMap;

use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, Expression, FieldDefinition, FieldId, FieldRef,
    FieldType, Schema, SchemaId, Value,
};
use tachiko_storage::{from_str, to_canonical_string};

fn all_v1_shapes_document() -> Document {
    Document {
        id: DocumentId::from("all-shapes"),
        title: "Café | Cafe\u{301}".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("node"),
            Schema {
                id: SchemaId::from("node"),
                fields: all_v1_field_types(),
            },
        )]),
        entities: BTreeMap::from([
            (
                EntityId::from("alpha"),
                Entity {
                    id: EntityId::from("alpha"),
                    schema: SchemaId::from("node"),
                    fields: all_v1_values_and_expressions(),
                },
            ),
            (
                EntityId::from("beta"),
                Entity {
                    id: EntityId::from("beta"),
                    schema: SchemaId::from("node"),
                    fields: BTreeMap::from([(FieldId::from("number"), Value::Number(4.0))]),
                },
            ),
        ]),
    }
}

fn all_v1_field_types() -> BTreeMap<FieldId, FieldDefinition> {
    let numeric_field = || FieldDefinition {
        field_type: FieldType::Number,
        required: false,
    };
    BTreeMap::from([
        (FieldId::from("add"), numeric_field()),
        (
            FieldId::from("boolean"),
            FieldDefinition {
                field_type: FieldType::Boolean,
                required: false,
            },
        ),
        (FieldId::from("divide"), numeric_field()),
        (FieldId::from("maximum"), numeric_field()),
        (FieldId::from("minimum"), numeric_field()),
        (FieldId::from("multiply"), numeric_field()),
        (
            FieldId::from("number"),
            FieldDefinition {
                field_type: FieldType::Number,
                required: true,
            },
        ),
        (
            FieldId::from("reference"),
            FieldDefinition {
                field_type: FieldType::Reference {
                    schema: SchemaId::from("node"),
                },
                required: false,
            },
        ),
        (FieldId::from("subtract"), numeric_field()),
        (
            FieldId::from("text"),
            FieldDefinition {
                field_type: FieldType::Text,
                required: false,
            },
        ),
    ])
}

fn all_v1_values_and_expressions() -> BTreeMap<FieldId, Value> {
    let reference = || Expression::Reference(FieldRef::new("beta", "number"));
    let number = || Box::new(Expression::Number(2.0));
    BTreeMap::from([
        (
            FieldId::from("add"),
            Value::Formula(Expression::Add {
                left: Box::new(reference()),
                right: number(),
            }),
        ),
        (FieldId::from("boolean"), Value::Boolean(true)),
        (
            FieldId::from("divide"),
            Value::Formula(Expression::Divide {
                left: Box::new(reference()),
                right: number(),
            }),
        ),
        (
            FieldId::from("maximum"),
            Value::Formula(Expression::Maximum {
                left: Box::new(reference()),
                right: number(),
            }),
        ),
        (
            FieldId::from("minimum"),
            Value::Formula(Expression::Minimum {
                left: Box::new(reference()),
                right: number(),
            }),
        ),
        (
            FieldId::from("multiply"),
            Value::Formula(Expression::Multiply {
                left: Box::new(reference()),
                right: number(),
            }),
        ),
        (FieldId::from("number"), Value::Number(8.0)),
        (
            FieldId::from("reference"),
            Value::Reference(EntityId::from("beta")),
        ),
        (
            FieldId::from("subtract"),
            Value::Formula(Expression::Subtract {
                left: Box::new(reference()),
                right: number(),
            }),
        ),
        (
            FieldId::from("text"),
            Value::Text("quote \" slash \\ tab\t line\n 日本語 😀".to_owned()),
        ),
    ])
}

#[test]
fn canonical_minimal_v1_has_exact_specified_bytes() {
    let document = Document::empty("doc", "Document");
    let expected = "{\n  \"format_version\": 1,\n  \"id\": \"doc\",\n  \"title\": \"Document\",\n  \"schemas\": {},\n  \"entities\": {}\n}\n";

    assert_eq!(to_canonical_string(&document).unwrap(), expected);
}

#[test]
fn every_v1_field_value_and_expression_discriminator_round_trips() {
    let document = all_v1_shapes_document();
    let encoded = to_canonical_string(&document).unwrap();
    let decoded = from_str(&encoded).unwrap();

    assert_eq!(decoded, document);
    for (member, discriminator) in [
        ("type", "number"),
        ("type", "text"),
        ("type", "boolean"),
        ("type", "reference"),
        ("kind", "number"),
        ("kind", "text"),
        ("kind", "boolean"),
        ("kind", "reference"),
        ("kind", "formula"),
        ("op", "number"),
        ("op", "reference"),
        ("op", "add"),
        ("op", "subtract"),
        ("op", "multiply"),
        ("op", "divide"),
        ("op", "minimum"),
        ("op", "maximum"),
    ] {
        assert!(
            encoded.contains(&format!("\"{member}\": \"{discriminator}\"")),
            "missing {member} discriminator {discriminator} in {encoded}"
        );
    }
    assert_eq!(to_canonical_string(&decoded).unwrap(), encoded);
}

#[test]
fn every_v1_shape_has_exact_canonical_bytes() {
    let document = all_v1_shapes_document();
    let expected = include_str!("fixtures/all-v1-shapes.ro");

    assert_eq!(to_canonical_string(&document).unwrap(), expected);
    assert_eq!(from_str(expected).unwrap(), document);
}

#[test]
fn canonical_v1_preserves_unicode_scalar_sequences_without_normalization() {
    let document = all_v1_shapes_document();
    let encoded = to_canonical_string(&document).unwrap();
    let decoded = from_str(&encoded).unwrap();

    assert!(encoded.contains("Café | Cafe\u{301}"));
    assert!(encoded.contains("日本語 😀"));
    assert_eq!(decoded.title, "Café | Cafe\u{301}");
    assert_ne!("é".as_bytes(), "e\u{301}".as_bytes());
}

#[test]
fn noncanonical_json_reencodes_to_the_unique_v1_layout() {
    let noncanonical = "{\r\n\t\"entities\": {}, \"schemas\": {},\r\n\"title\":\"\\u65e5\",\"id\":\"doc\",\"format_version\":1}\r\n";
    let expected = "{\n  \"format_version\": 1,\n  \"id\": \"doc\",\n  \"title\": \"日\",\n  \"schemas\": {},\n  \"entities\": {}\n}\n";

    let document = from_str(noncanonical).unwrap();
    let encoded = to_canonical_string(&document).unwrap();

    assert_eq!(encoded, expected);
    assert!(!encoded.starts_with('\u{feff}'));
    assert!(!encoded.contains('\r'));
    assert!(encoded.lines().all(|line| !line.ends_with([' ', '\t'])));
    assert!(encoded.ends_with('\n'));
    assert!(!encoded.ends_with("\n\n"));
}

#[test]
fn alternative_legal_string_escapes_have_one_canonical_spelling() {
    let escaped =
        r#"{"format_version":1,"id":"doc","title":"\u65e5\/work","schemas":{},"entities":{}}"#;
    let literal = r#"{"format_version":1,"id":"doc","title":"日/work","schemas":{},"entities":{}}"#;

    let escaped_output = to_canonical_string(&from_str(escaped).unwrap()).unwrap();
    let literal_output = to_canonical_string(&from_str(literal).unwrap()).unwrap();

    assert_eq!(escaped_output, literal_output);
    assert!(literal_output.contains("日/work"));
    assert!(!literal_output.contains("\\/"));
}

#[test]
fn every_legacy_id_map_uses_ascii_lexicographic_order() {
    let document = Document {
        id: DocumentId::from("doc"),
        title: "Ordering".to_owned(),
        schemas: BTreeMap::from([
            (
                SchemaId::from("z-schema"),
                Schema {
                    id: SchemaId::from("z-schema"),
                    fields: BTreeMap::from([
                        (
                            FieldId::from("z-field"),
                            FieldDefinition {
                                field_type: FieldType::Text,
                                required: false,
                            },
                        ),
                        (
                            FieldId::from("a-field"),
                            FieldDefinition {
                                field_type: FieldType::Text,
                                required: false,
                            },
                        ),
                    ]),
                },
            ),
            (
                SchemaId::from("a-schema"),
                Schema {
                    id: SchemaId::from("a-schema"),
                    fields: BTreeMap::new(),
                },
            ),
        ]),
        entities: BTreeMap::from([
            (
                EntityId::from("z-entity"),
                Entity {
                    id: EntityId::from("z-entity"),
                    schema: SchemaId::from("z-schema"),
                    fields: BTreeMap::from([
                        (FieldId::from("z-field"), Value::Text("z".to_owned())),
                        (FieldId::from("a-field"), Value::Text("a".to_owned())),
                    ]),
                },
            ),
            (
                EntityId::from("a-entity"),
                Entity {
                    id: EntityId::from("a-entity"),
                    schema: SchemaId::from("a-schema"),
                    fields: BTreeMap::new(),
                },
            ),
        ]),
    };

    let encoded = to_canonical_string(&document).unwrap();
    let schemas = &encoded
        [encoded.find("  \"schemas\": {").unwrap()..encoded.find("  \"entities\": {").unwrap()];
    let entities = &encoded[encoded.find("  \"entities\": {").unwrap()..];

    assert!(
        schemas.find("    \"a-schema\": {").unwrap() < schemas.find("    \"z-schema\": {").unwrap()
    );
    assert!(
        schemas.find("        \"a-field\": {").unwrap()
            < schemas.find("        \"z-field\": {").unwrap()
    );
    assert!(
        entities.find("    \"a-entity\": {").unwrap()
            < entities.find("    \"z-entity\": {").unwrap()
    );
    assert!(
        entities.find("        \"a-field\": {").unwrap()
            < entities.find("        \"z-field\": {").unwrap()
    );
}

#[test]
fn checked_in_ro_examples_are_canonical_and_byte_stable() {
    let fixtures = [
        include_str!("../../../examples/game-balance/game-balance.ro"),
        include_str!("../../../examples/game-balance/buffed-sword.ro"),
    ];

    for fixture in fixtures {
        let document = from_str(fixture).unwrap();
        let encoded = to_canonical_string(&document).unwrap();
        let decoded = from_str(&encoded).unwrap();

        assert_eq!(encoded, fixture);
        assert_eq!(to_canonical_string(&decoded).unwrap(), encoded);
    }
}
