//! Steward-owned literal acceptance for #497; expected diagnostics are not
//! calculated by the validator being tested. Production remains unchanged.
use std::collections::BTreeMap;

use tachiko_semantic_core::{
    Date, Diagnostic, DiagnosticCode as Code, DiagnosticFact, DiagnosticProvider,
    DiagnosticSeverity, Document, Entity, Expression, FieldConstraint, FieldDefinition, FieldId,
    FieldRef, FieldType, Number, Schema, SemanticSubject as Subject, Value, validate_document,
    validate_document_core,
};

fn definition(id: &str, field_type: FieldType, required: bool) -> (FieldId, FieldDefinition) {
    (
        id.into(),
        FieldDefinition {
            id: id.into(),
            key: id.into(),
            field_type,
            required,
            constraint: FieldConstraint::None,
        },
    )
}

fn fixture() -> Document {
    let mut document = Document::empty("doc", "Scalar equivalence");
    document.schemas.insert(
        "s".into(),
        Schema {
            id: "s".into(),
            key: "scalar".into(),
            fields: BTreeMap::from([
                definition("bool", FieldType::Boolean, true),
                definition("date", FieldType::Date, true),
                definition("f", FieldType::Number, true),
                definition("n", FieldType::Number, true),
                definition("optional", FieldType::Text, false),
                definition("ref", FieldType::Reference { schema: "s".into() }, false),
                definition("text", FieldType::Text, true),
            ]),
        },
    );
    document.schemas.insert(
        "other".into(),
        Schema {
            id: "other".into(),
            key: "other".into(),
            fields: BTreeMap::new(),
        },
    );
    document.entities.insert(
        "e".into(),
        Entity {
            id: "e".into(),
            key: "entity".into(),
            schema: "s".into(),
            fields: BTreeMap::from([
                ("bool".into(), Value::Boolean(true)),
                (
                    "date".into(),
                    Value::Date("2024-02-29".parse::<Date>().unwrap()),
                ),
                (
                    "f".into(),
                    Value::Formula(Expression::Reference(FieldRef::new("e", "n"))),
                ),
                ("n".into(), Value::Number(Number::new(1.0).unwrap())),
                ("ref".into(), Value::Reference("e".into())),
                ("text".into(), Value::Text("雪".into())),
            ]),
        },
    );
    document.entities.insert(
        "q".into(),
        Entity {
            id: "q".into(),
            key: "other_entity".into(),
            schema: "other".into(),
            fields: BTreeMap::new(),
        },
    );
    document
}

fn diagnostic(code: Code, path: &str, message: &str, subjects: Vec<Subject>) -> Diagnostic {
    Diagnostic::new(
        code,
        DiagnosticSeverity::Error,
        subjects,
        DiagnosticProvider::new("tachiko.semantic-core"),
    )
    .with_presentation(path, message)
}

fn field_diagnostic(code: Code, field: &str, message: &str) -> Diagnostic {
    diagnostic(
        code,
        &format!("entities.e.fields.{field}"),
        message,
        vec![Subject::EntityField(FieldRef::new("e", field))],
    )
}

fn assert_both(document: &Document, expected: &[Diagnostic]) {
    assert_eq!(validate_document(document), expected);
    assert_eq!(validate_document_core(document), expected);
}

#[test]
fn mixed_valid_scalars_optional_absence_and_exact_constraints() {
    let mut document = fixture();
    let wide = "x".repeat(4096);
    for text in ["", "雪", wide.as_str()] {
        document
            .entities
            .get_mut("e")
            .unwrap()
            .fields
            .insert("text".into(), Value::Text(text.into()));
        assert_both(&document, &[]);
    }
    document
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("text")
        .unwrap()
        .constraint = FieldConstraint::TextLiteralSet {
        values: vec![String::new(), "雪".into()],
    };
    document
        .entities
        .get_mut("e")
        .unwrap()
        .fields
        .insert("text".into(), Value::Text("雪".into()));
    document
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("n")
        .unwrap()
        .constraint = FieldConstraint::NumberInclusiveRange {
        min: Number::new(0.0).unwrap(),
        max: Number::new(2.0).unwrap(),
    };
    for value in [0.0, 2.0] {
        document
            .entities
            .get_mut("e")
            .unwrap()
            .fields
            .insert("n".into(), Value::Number(Number::new(value).unwrap()));
        assert_both(&document, &[]);
    }
}

fn mixed_invalid() -> (Document, Vec<Diagnostic>) {
    let mut document = fixture();
    let entity = document.entities.get_mut("e").unwrap();
    entity.id = "declared".into();
    entity.key = "Bad".into();
    entity.fields.remove("text");
    entity
        .fields
        .insert("n".into(), Value::Text("wrong".into()));
    entity
        .fields
        .insert("unknown".into(), Value::Boolean(false));
    // Keep the formula valid here so legacy/core agree on this exact vector.
    entity.fields.insert(
        "f".into(),
        Value::Formula(Expression::Number(Number::new(1.0).unwrap())),
    );
    let orphan = document.entities.get_mut("q").unwrap();
    orphan.schema = "missing".into();
    orphan
        .fields
        .insert("unknown".into(), Value::Text("suppressed cascade".into()));
    let expected = vec![
        diagnostic(
            Code::INVALID_KEY,
            "entities.e.key",
            "entity key 'Bad' must use only a-z, 0-9, '_' or '-', starting with a letter or digit",
            vec![Subject::Entity("e".into())],
        ),
        diagnostic(
            Code::KEY_MISMATCH,
            "entities.e.id",
            "entity store key 'e' does not match stable id 'declared'",
            vec![
                Subject::Entity("declared".into()),
                Subject::Entity("e".into()),
            ],
        )
        .with_fact(DiagnosticFact::new("declared_id", "declared"))
        .with_fact(DiagnosticFact::new("store_id", "e")),
        field_diagnostic(
            Code::MISSING_REQUIRED_FIELD,
            "text",
            "required field 'text' is missing",
        ),
        diagnostic(
            Code::MISSING_SCHEMA,
            "entities.q.schema",
            "schema 'missing' does not exist",
            vec![Subject::Entity("q".into())],
        )
        .with_related_subjects(vec![Subject::Schema("missing".into())]),
        field_diagnostic(
            Code::TYPE_MISMATCH,
            "n",
            "field 'e.n' expects number, but found text",
        )
        .with_fact(DiagnosticFact::new("actual_kind", "text"))
        .with_fact(DiagnosticFact::new("expected_kind", "number")),
        field_diagnostic(
            Code::UNEXPECTED_FIELD,
            "unknown",
            "field 'unknown' is not declared by schema 's'",
        )
        .with_related_subjects(vec![Subject::SchemaField {
            schema: "s".into(),
            field: "unknown".into(),
        }]),
    ];
    (document, expected)
}

#[test]
fn mixed_failures_preserve_complete_literal_ordered_diagnostics() {
    let (document, expected) = mixed_invalid();
    let before = document.clone();
    assert_both(&document, &expected);
    assert_eq!(document, before);
}

#[test]
fn direct_constraint_and_declaration_failures_keep_exact_fields() {
    let mut document = fixture();
    document
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("text")
        .unwrap()
        .constraint = FieldConstraint::TextLiteralSet {
        values: vec!["allowed".into()],
    };
    document
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("n")
        .unwrap()
        .constraint = FieldConstraint::NumberInclusiveRange {
        min: Number::new(2.0).unwrap(),
        max: Number::new(0.0).unwrap(),
    };
    assert_both(
        &document,
        &[
            diagnostic(
                Code::FIELD_CONSTRAINT_DECLARATION,
                "schemas.s.fields.n.constraint",
                "field constraint is invalid for its declared field type",
                vec![Subject::SchemaField {
                    schema: "s".into(),
                    field: "n".into(),
                }],
            ),
            field_diagnostic(
                Code::FIELD_VALUE_CONSTRAINT_MISMATCH,
                "n",
                "stored field value does not satisfy its declared constraint",
            ),
            field_diagnostic(
                Code::FIELD_VALUE_CONSTRAINT_MISMATCH,
                "text",
                "stored field value does not satisfy its declared constraint",
            ),
        ],
    );
}

#[test]
fn reference_errors_keep_targets_facts_and_type_mismatch() {
    for (target, expected) in [
        (
            "absent",
            field_diagnostic(
                Code::MISSING_REFERENCE,
                "ref",
                "referenced entity stable id 'absent' does not exist",
            )
            .with_related_subjects(vec![Subject::Entity("absent".into())]),
        ),
        (
            "q",
            field_diagnostic(
                Code::REFERENCE_TYPE_MISMATCH,
                "ref",
                "field 'e.ref' expects schema 's', but entity 'q' uses schema 'other'",
            )
            .with_related_subjects(vec![Subject::Entity("q".into())])
            .with_fact(DiagnosticFact::new("actual_schema", "other"))
            .with_fact(DiagnosticFact::new("expected_schema", "s")),
        ),
    ] {
        let mut document = fixture();
        document
            .entities
            .get_mut("e")
            .unwrap()
            .fields
            .insert("ref".into(), Value::Reference(target.into()));
        assert_both(&document, &[expected]);
    }
    let mut document = fixture();
    document.entities.get_mut("e").unwrap().fields.insert(
        "text".into(),
        Value::Formula(Expression::Number(Number::new(1.0).unwrap())),
    );
    assert_both(
        &document,
        &[field_diagnostic(
            Code::TYPE_MISMATCH,
            "text",
            "field 'e.text' expects text, but found formula",
        )
        .with_fact(DiagnosticFact::new("actual_kind", "formula"))
        .with_fact(DiagnosticFact::new("expected_kind", "text"))],
    );
}

#[test]
fn legacy_formula_reference_checks_remain_distinct_from_core_validation() {
    for (target, expected) in [
        (
            FieldRef::new("absent", "n"),
            field_diagnostic(
                Code::MISSING_FORMULA_REFERENCE,
                "f",
                "formula target entity stable id 'absent' does not exist",
            )
            .with_related_subjects(vec![Subject::EntityField(FieldRef::new("absent", "n"))]),
        ),
        (
            FieldRef::new("e", "text"),
            field_diagnostic(
                Code::FORMULA_REFERENCE_TYPE_MISMATCH,
                "f",
                "formula reference 'e.text' does not target a numeric field",
            )
            .with_related_subjects(vec![Subject::EntityField(FieldRef::new("e", "text"))])
            .with_fact(DiagnosticFact::new("actual_kind", "text"))
            .with_fact(DiagnosticFact::new("expected_kind", "number")),
        ),
    ] {
        let mut document = fixture();
        document
            .entities
            .get_mut("e")
            .unwrap()
            .fields
            .insert("f".into(), Value::Formula(Expression::Reference(target)));
        assert_eq!(validate_document(&document), [expected]);
        assert_eq!(validate_document_core(&document), Vec::<Diagnostic>::new());
    }
}

#[cfg(feature = "issue-175-research")]
#[test]
fn cancellation_keeps_exact_completed_results_and_can_stop() {
    use tachiko_semantic_core::validate_document_cancellable;
    let (document, expected) = mixed_invalid();
    assert_eq!(
        validate_document_cancellable(&document, || false),
        Some(expected)
    );
    assert_eq!(validate_document_cancellable(&document, || true), None);
}
