use std::collections::BTreeMap;
use tachiko_semantic_core::{
    Document, Entity, FieldConstraint, FieldDefinition, FieldType, Number, Schema, Value,
};

fn range(min: f64, max: f64) -> FieldConstraint {
    FieldConstraint::NumberInclusiveRange {
        min: Number::new(min).unwrap(),
        max: Number::new(max).unwrap(),
    }
}

fn document(field_type: FieldType, constraint: FieldConstraint, value: Option<Value>) -> Document {
    let mut document = Document::empty("doc", "Constraints acceptance");
    document.schemas.insert(
        "schema".into(),
        Schema {
            id: "schema".into(),
            key: "items".into(),
            fields: BTreeMap::from([(
                "field".into(),
                FieldDefinition {
                    id: "field".into(),
                    key: "value".into(),
                    field_type,
                    required: false,
                    constraint,
                },
            )]),
        },
    );
    if let Some(value) = value {
        document.entities.insert(
            "entity".into(),
            Entity {
                id: "entity".into(),
                key: "item".into(),
                schema: "schema".into(),
                fields: BTreeMap::from([("field".into(), value)]),
            },
        );
    }
    document
}

use tachiko_semantic_core::{Diagnostic, DiagnosticCode, validate_document};

fn text(values: &[&str]) -> FieldConstraint {
    FieldConstraint::TextLiteralSet {
        values: values.iter().map(|v| (*v).to_owned()).collect(),
    }
}

fn declaration_errors(document: &Document) -> Vec<Diagnostic> {
    validate_document(document)
        .into_iter()
        .filter(|d| d.code == DiagnosticCode::new("core.field_constraint_declaration"))
        .collect()
}

#[test]
fn invalid_declarations_are_rejected_even_without_entities() {
    let cases = [
        (FieldType::Text, text(&[])),
        (FieldType::Text, text(&["same", "same"])),
        (FieldType::Text, text(&["z", "a"])),
        (FieldType::Number, range(2.0, 1.0)),
        (FieldType::Number, text(&["one"])),
        (FieldType::Text, range(0.0, 1.0)),
        (FieldType::Boolean, text(&["true"])),
        (FieldType::Date, range(0.0, 1.0)),
        (
            FieldType::Reference {
                schema: "schema".into(),
            },
            text(&["item"]),
        ),
    ];
    for (field_type, constraint) in cases {
        let document = document(field_type, constraint, None);
        assert!(document.entities.is_empty());
        let errors = declaration_errors(&document);
        assert_eq!(errors.len(), 1, "{document:?}: {errors:?}");
        assert_eq!(errors[0].path, "schemas.schema.fields.field.constraint");
    }
}

#[test]
fn text_limits_count_decoded_utf8_bytes_and_accept_exact_bounds() {
    let cases = [
        (vec![String::new()], true),
        ((0..256).map(|i| format!("v{i:03}")).collect(), true),
        ((0..257).map(|i| format!("v{i:03}")).collect(), false),
        (vec!["é".repeat(512)], true),
        (vec!["é".repeat(513)], false),
        (
            (0..64)
                .map(|i| format!("{i:03}{}", "a".repeat(1021)))
                .collect(),
            true,
        ),
        (
            (0..65)
                .map(|i| format!("{i:03}{}", "a".repeat(1021)))
                .collect(),
            false,
        ),
    ];
    for (values, valid) in cases {
        let candidate = document(
            FieldType::Text,
            FieldConstraint::TextLiteralSet { values },
            None,
        );
        assert_eq!(declaration_errors(&candidate).is_empty(), valid);
    }
}

#[test]
fn literal_membership_is_exact_without_case_or_unicode_normalization() {
    for value in ["", "A", "a", "é"] {
        let candidate = document(
            FieldType::Text,
            text(&["", "A", "a", "é"]),
            Some(Value::Text(value.to_owned())),
        );
        assert!(validate_document(&candidate).is_empty());
    }
    for value in ["a ", "e\u{301}", "Ｅ"] {
        let candidate = document(
            FieldType::Text,
            text(&["", "A", "a", "é"]),
            Some(Value::Text(value.to_owned())),
        );
        let diagnostics = validate_document(&candidate);
        assert!(diagnostics.iter().any(|d| d.code
            == DiagnosticCode::new("core.field_value_constraint_mismatch")
            && d.path == "entities.entity.fields.field"));
    }
}

#[test]
fn optional_absence_remains_absent_while_present_values_obey_inclusive_range() {
    let mut absent = document(FieldType::Number, range(-1.0, 1.0), None);
    absent.entities.insert(
        "entity".into(),
        Entity {
            id: "entity".into(),
            key: "item".into(),
            schema: "schema".into(),
            fields: BTreeMap::new(),
        },
    );
    assert!(validate_document(&absent).is_empty());
    assert!(absent.entities["entity"].fields.is_empty());
    for (value, valid) in [
        (-1.0, true),
        (0.0, true),
        (1.0, true),
        (-1.0001, false),
        (1.0001, false),
    ] {
        let candidate = document(
            FieldType::Number,
            range(-1.0, 1.0),
            Some(Value::Number(Number::new(value).unwrap())),
        );
        assert_eq!(validate_document(&candidate).is_empty(), valid);
    }
}

#[test]
fn required_closed_semantic_facet_is_not_defaulted_or_extended() {
    let field = document(FieldType::Text, FieldConstraint::None, None)
        .schemas
        .remove("schema")
        .unwrap()
        .fields
        .remove("field")
        .unwrap();
    let original = serde_json::to_value(field).unwrap();
    for replacement in [
        None,
        Some(serde_json::Value::Null),
        Some(serde_json::json!({"type":"unknown"})),
        Some(serde_json::json!({"type":"none", "extra":true})),
    ] {
        let mut json = original.clone();
        if let Some(replacement) = replacement {
            json["constraint"] = replacement;
        } else {
            json.as_object_mut().unwrap().remove("constraint");
        }
        assert!(serde_json::from_value::<FieldDefinition>(json).is_err());
    }
}
