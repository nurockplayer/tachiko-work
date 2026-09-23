use std::collections::BTreeMap;

use tachiko_semantic_core::{
    DiagnosticCode, DiagnosticSeverity, Document, Entity, FieldConstraint, FieldDefinition,
    FieldId, FieldKey, FieldRef, FieldType, Number, Schema, SemanticSubject, Value,
    validate_document,
};

fn number(value: f64) -> Number {
    Number::new(value).expect("finite worker fixture")
}

fn constrained_document(
    field_type: FieldType,
    constraint: FieldConstraint,
    fields: BTreeMap<&str, Value>,
) -> Document {
    let mut document = Document::empty("document", "Constraint worker");
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
    document.entities = fields
        .into_iter()
        .map(|(id, value)| {
            (
                id.into(),
                Entity {
                    id: id.into(),
                    key: id.into(),
                    schema: "schema".into(),
                    fields: BTreeMap::from([("field".into(), value)]),
                },
            )
        })
        .collect();
    document
}

fn constraint_diagnostics(document: &Document) -> Vec<tachiko_semantic_core::Diagnostic> {
    validate_document(document)
        .into_iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic.code,
                DiagnosticCode::FIELD_CONSTRAINT_DECLARATION
                    | DiagnosticCode::FIELD_VALUE_CONSTRAINT_MISMATCH
            )
        })
        .collect()
}

#[test]
fn field_constraint_serde_rejects_missing_null_unknown_and_extra_members() {
    let original = serde_json::to_value(FieldDefinition {
        id: FieldId::from("f"),
        key: FieldKey::from("f"),
        field_type: FieldType::Text,
        required: false,
        constraint: FieldConstraint::None,
    })
    .expect("field definition serializes");

    for replacement in [
        None,
        Some(serde_json::Value::Null),
        Some(serde_json::json!({"type": "unknown"})),
        Some(serde_json::json!({"type": "none", "extra": true})),
    ] {
        let mut value = original.clone();
        if let Some(replacement) = replacement {
            value["constraint"] = replacement;
        } else {
            value
                .as_object_mut()
                .expect("serialized field definition is an object")
                .remove("constraint");
        }
        assert!(serde_json::from_value::<FieldDefinition>(value).is_err());
    }
}

#[test]
fn declaration_limits_emit_the_exact_schema_subject_and_diagnostic_contract() {
    let document = constrained_document(
        FieldType::Text,
        FieldConstraint::TextLiteralSet {
            values: (0..257).map(|index| format!("v{index:03}")).collect(),
        },
        BTreeMap::new(),
    );

    let diagnostics = constraint_diagnostics(&document);
    assert_eq!(diagnostics.len(), 1);
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(
        diagnostic.code,
        DiagnosticCode::FIELD_CONSTRAINT_DECLARATION
    );
    assert_eq!(diagnostic.path, "schemas.schema.fields.field.constraint");
    assert_eq!(
        diagnostic.subjects,
        vec![SemanticSubject::SchemaField {
            schema: "schema".into(),
            field: "field".into(),
        }]
    );

    for values in [
        vec!["é".repeat(512)],
        (0..64)
            .map(|index| format!("{index:03}{}", "a".repeat(1021)))
            .collect(),
    ] {
        let document = constrained_document(
            FieldType::Text,
            FieldConstraint::TextLiteralSet { values },
            BTreeMap::new(),
        );
        assert!(constraint_diagnostics(&document).is_empty());
    }
}

#[test]
fn direct_constraints_preserve_unicode_absence_inclusive_bounds_and_sorted_subjects() {
    let text_document = constrained_document(
        FieldType::Text,
        FieldConstraint::TextLiteralSet {
            values: vec!["é".into()],
        },
        BTreeMap::from([
            ("z", Value::Text("e\u{301}".into())),
            ("a", Value::Text("É".into())),
        ]),
    );
    let diagnostics = constraint_diagnostics(&text_document);
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.path.as_str())
            .collect::<Vec<_>>(),
        vec!["entities.a.fields.field", "entities.z.fields.field"]
    );
    for (entity, diagnostic) in [("a", &diagnostics[0]), ("z", &diagnostics[1])] {
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
        assert_eq!(
            diagnostic.code,
            DiagnosticCode::FIELD_VALUE_CONSTRAINT_MISMATCH
        );
        assert_eq!(
            diagnostic.subjects,
            vec![SemanticSubject::EntityField(FieldRef::new(entity, "field"))]
        );
    }

    let range = FieldConstraint::NumberInclusiveRange {
        min: number(-1.0),
        max: number(1.0),
    };
    for (value, valid) in [
        (-1.0, true),
        (1.0, true),
        (-1.000_001, false),
        (1.000_001, false),
    ] {
        let document = constrained_document(
            FieldType::Number,
            range.clone(),
            BTreeMap::from([("entity", Value::Number(number(value)))]),
        );
        assert_eq!(constraint_diagnostics(&document).is_empty(), valid);
    }

    let mut absent = constrained_document(FieldType::Number, range, BTreeMap::new());
    absent.entities.insert(
        "optional".into(),
        Entity {
            id: "optional".into(),
            key: "optional".into(),
            schema: "schema".into(),
            fields: BTreeMap::new(),
        },
    );
    assert!(constraint_diagnostics(&absent).is_empty());
}

#[test]
fn text_literal_set_declaration_boundaries_reject_oversize_aggregate_and_noncanonical_values() {
    let valid_maximum = (0..256).map(|index| format!("v{index:03}")).collect();
    let valid_aggregate = (0..64)
        .map(|index| format!("{index:03}{}", "a".repeat(1021)))
        .collect();

    let cases = [
        ("accepts the maximum literal count", valid_maximum, false),
        (
            "rejects a UTF-8 literal over the byte limit",
            vec!["é".repeat(513)],
            true,
        ),
        ("accepts the aggregate byte limit", valid_aggregate, false),
        (
            "rejects an aggregate over the byte limit",
            (0..65)
                .map(|index| format!("{index:03}{}", "a".repeat(1021)))
                .collect(),
            true,
        ),
        (
            "rejects duplicate literals",
            vec!["same".into(), "same".into()],
            true,
        ),
        (
            "rejects bytewise descending literals",
            vec!["é".into(), "z".into()],
            true,
        ),
    ];

    for (case, values, rejects) in cases {
        let document = constrained_document(
            FieldType::Text,
            FieldConstraint::TextLiteralSet { values },
            BTreeMap::new(),
        );
        let diagnostics = constraint_diagnostics(&document);
        assert_eq!(!diagnostics.is_empty(), rejects, "{case}: {diagnostics:#?}");
        if rejects {
            assert_eq!(
                diagnostics[0].code,
                DiagnosticCode::FIELD_CONSTRAINT_DECLARATION
            );
        }
    }
}

#[test]
fn declaration_rejects_wrong_type_pairings_and_accepts_normalized_degenerate_ranges() {
    let text_constraint = FieldConstraint::TextLiteralSet {
        values: vec!["allowed".into()],
    };
    let range_constraint = FieldConstraint::NumberInclusiveRange {
        min: number(-1.0),
        max: number(1.0),
    };
    let invalid_pairs = [
        (FieldType::Text, range_constraint.clone()),
        (FieldType::Number, text_constraint.clone()),
        (FieldType::Boolean, text_constraint.clone()),
        (FieldType::Date, range_constraint.clone()),
        (
            FieldType::Reference {
                schema: "target".into(),
            },
            text_constraint,
        ),
    ];

    for (field_type, constraint) in invalid_pairs {
        let diagnostics = constraint_diagnostics(&constrained_document(
            field_type,
            constraint,
            BTreeMap::new(),
        ));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::FIELD_CONSTRAINT_DECLARATION
        );
    }

    for (min, max) in [(number(7.0), number(7.0)), (number(-0.0), number(0.0))] {
        let document = constrained_document(
            FieldType::Number,
            FieldConstraint::NumberInclusiveRange { min, max },
            BTreeMap::new(),
        );
        assert!(constraint_diagnostics(&document).is_empty());
    }

    let reversed = constrained_document(
        FieldType::Number,
        FieldConstraint::NumberInclusiveRange {
            min: number(1.0),
            max: number(-1.0),
        },
        BTreeMap::new(),
    );
    assert_eq!(constraint_diagnostics(&reversed).len(), 1);
}
