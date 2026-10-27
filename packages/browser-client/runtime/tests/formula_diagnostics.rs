use std::collections::BTreeMap;

use tachiko_designer_runtime::{
    CalculationProjection, DesignerError, DesignerRequest, DesignerResponse, DesignerRuntime,
    FieldProjection, FieldTarget, StoredValueProjection,
};
use tachiko_workspace_engine::{
    Document, Entity, EntityId, EntityKey, FieldDefinition, FieldId, FieldKey, FieldRef, FieldType,
    Number, Schema, SchemaId, SchemaKey, SemanticSubject, Value,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000500";

fn definition(field: &str, field_type: FieldType) -> (FieldId, FieldDefinition) {
    let id = FieldId::from(field);
    (
        id.clone(),
        FieldDefinition {
            id,
            key: FieldKey::from(field),
            display_name: None,
            number_format: None,
            field_type,
            required: true,
            constraint: tachiko_workspace_engine::FieldConstraint::None,
        },
    )
}

fn fixture() -> DesignerRuntime {
    let mut document = Document::empty("formula_diagnostics", "Formula diagnostics");
    let schema = SchemaId::from("items");
    document.schemas.insert(
        schema.clone(),
        Schema {
            id: schema.clone(),
            key: SchemaKey::from("items"),
            display_name: None,
            fields: BTreeMap::from([
                definition("n", FieldType::Number),
                definition("m", FieldType::Number),
                definition("label", FieldType::Text),
            ]),
        },
    );
    document.entities.insert(
        EntityId::from("r1"),
        Entity {
            id: EntityId::from("r1"),
            key: EntityKey::from("r1"),
            schema,
            fields: BTreeMap::from([
                ("n".into(), Value::Number(Number::new(2.0).unwrap())),
                ("m".into(), Value::Number(Number::new(3.0).unwrap())),
                ("label".into(), Value::Text("one".to_owned())),
            ]),
        },
    );
    DesignerRuntime::from_document(document, OCCURRENCE).unwrap()
}

fn update(
    runtime: &mut DesignerRuntime,
    revision: u32,
    target: &str,
    source: &str,
) -> Result<(), DesignerError> {
    runtime
        .handle(DesignerRequest::FormulaUpdate {
            expected_revision: format!("resident/{revision}"),
            target: target.into(),
            source: source.to_owned(),
        })
        .map(|_| ())
}

fn field(runtime: &mut DesignerRuntime, revision: u32, target: &str) -> FieldProjection {
    let DesignerResponse::Fields(mut projection) = runtime
        .handle(DesignerRequest::QueryFields {
            expected_revision: format!("resident/{revision}"),
            fields: vec![FieldTarget::from(target)],
        })
        .expect("refused updates must retain the accepted revision")
    else {
        panic!("expected fields response")
    };
    projection.fields.remove(0)
}

fn calculated(field: &FieldProjection) -> Option<f64> {
    field
        .calculated
        .as_ref()
        .and_then(CalculationProjection::number)
}

fn stored_number(field: &FieldProjection) -> Option<f64> {
    match field.stored {
        Some(StoredValueProjection::Number { value }) => Some(value),
        _ => None,
    }
}

#[test]
fn syntax_error_is_typed_with_position_and_preserves_the_previous_formula() {
    let mut runtime = fixture();
    update(&mut runtime, 0, "r1.n", "[r1.m] * 2").unwrap();
    let error = update(&mut runtime, 1, "r1.n", "[r1.m").expect_err("invalid syntax must refuse");
    let failure = error.failure_projection("resident/1");
    assert_eq!(failure.code, "formula_syntax");
    assert!(
        failure
            .message
            .starts_with("Formula syntax error at character ")
    );
    let diagnostic = &failure.diagnostics[0];
    assert_eq!(diagnostic.code, "formula_syntax");
    assert_eq!(diagnostic.path, "formulas.r1.n");
    assert_eq!(
        diagnostic.subjects,
        [SemanticSubject::EntityField(FieldRef::new("r1", "n"))]
    );
    assert_eq!(diagnostic.facts[0].name, "position");
    assert_eq!(diagnostic.facts[0].value, "6");
    assert!(failure.message.contains("at character 6:"));

    let unchanged = field(&mut runtime, 1, "r1.n");
    assert_eq!(
        unchanged
            .formula
            .as_ref()
            .map(|formula| formula.source.as_str()),
        Some("([r1.m] * 2)")
    );
    assert_eq!(calculated(&unchanged), Some(6.0));
}

#[test]
fn syntax_position_counts_characters_rather_than_utf8_bytes() {
    let mut runtime = fixture();
    // The multiplication sign ends the expression: the refusal counts the
    // characters typed before it, which the message and fact report one-based.
    let error = update(&mut runtime, 0, "r1.n", "[r1.m] × 2")
        .expect_err("a non-ASCII operator must refuse");
    let failure = error.failure_projection("resident/0");
    assert_eq!(failure.code, "formula_syntax");
    assert!(failure.message.contains("at character 8:"));
    assert_eq!(failure.diagnostics[0].facts[0].value, "8");

    // The byte budget ends inside a multi-byte character; the refusal reports
    // the character position the author reached, not the 4096-byte offset.
    let oversized = "＋".repeat(1400);
    assert_eq!(oversized.len(), 4200);
    let error =
        update(&mut runtime, 0, "r1.n", &oversized).expect_err("oversized input must refuse");
    let failure = error.failure_projection("resident/0");
    assert_eq!(failure.code, "formula_syntax");
    assert!(failure.message.contains("at character 1367:"));
    assert_eq!(failure.diagnostics[0].facts[0].value, "1367");
}

#[test]
fn unknown_reference_is_typed_and_preserves_the_previous_value() {
    let mut runtime = fixture();
    let error = update(&mut runtime, 0, "r1.n", "[r1.missing] + 1")
        .expect_err("unknown references must refuse");
    let failure = error.failure_projection("resident/0");
    assert_eq!(failure.code, "formula_unknown_reference");
    assert!(failure.message.contains("r1.missing"));
    let diagnostic = &failure.diagnostics[0];
    assert_eq!(diagnostic.code, "formula_unknown_reference");
    assert_eq!(diagnostic.path, "formulas.r1.n");
    assert_eq!(diagnostic.facts[0].name, "reference");
    assert_eq!(diagnostic.facts[0].value, "r1.missing");

    let unchanged = field(&mut runtime, 0, "r1.n");
    assert!(unchanged.formula.is_none());
    assert_eq!(stored_number(&unchanged), Some(2.0));
}

#[test]
fn non_numeric_reference_is_typed_as_a_type_mismatch() {
    let mut runtime = fixture();
    let error =
        update(&mut runtime, 0, "r1.n", "[r1.label] * 2").expect_err("Text operands must refuse");
    let failure = error.failure_projection("resident/0");
    assert_eq!(failure.code, "formula_type_mismatch");
    assert!(failure.message.contains("r1.label"));
    let diagnostic = &failure.diagnostics[0];
    assert_eq!(diagnostic.code, "formula_type_mismatch");
    assert_eq!(
        diagnostic.subjects,
        [SemanticSubject::EntityField(FieldRef::new("r1", "label"))]
    );
    assert_eq!(
        diagnostic.related_subjects,
        [SemanticSubject::EntityField(FieldRef::new("r1", "n"))]
    );

    let unchanged = field(&mut runtime, 0, "r1.n");
    assert!(unchanged.formula.is_none());
    assert_eq!(stored_number(&unchanged), Some(2.0));
}

#[test]
fn self_reference_is_typed_as_a_cycle_without_a_stale_current_value() {
    let mut runtime = fixture();
    let error =
        update(&mut runtime, 0, "r1.n", "[r1.n] + 1").expect_err("self-references must refuse");
    let failure = error.failure_projection("resident/0");
    assert_eq!(failure.code, "formula_cycle");
    assert!(failure.message.contains("dependency cycle"));
    assert!(
        failure
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "formula.cycle")
    );

    let unchanged = field(&mut runtime, 0, "r1.n");
    assert!(unchanged.formula.is_none());
    assert_eq!(stored_number(&unchanged), Some(2.0));
}

#[test]
fn mutual_reference_is_typed_as_a_cycle_and_keeps_the_accepted_formula() {
    let mut runtime = fixture();
    update(&mut runtime, 0, "r1.m", "[r1.n] + 1").unwrap();
    let refused = field(&mut runtime, 1, "r1.m");
    assert_eq!(calculated(&refused), Some(3.0));

    let error =
        update(&mut runtime, 1, "r1.n", "[r1.m] + 1").expect_err("mutual references must refuse");
    assert_eq!(error.failure_projection("resident/1").code, "formula_cycle");

    let target = field(&mut runtime, 1, "r1.n");
    assert!(target.formula.is_none());
    assert_eq!(stored_number(&target), Some(2.0));
    let source = field(&mut runtime, 1, "r1.m");
    assert_eq!(
        source
            .formula
            .as_ref()
            .map(|formula| formula.source.as_str()),
        Some("([r1.n] + 1)")
    );
    assert_eq!(calculated(&source), Some(3.0));
}

#[test]
fn condition_and_round_argument_type_mismatches_are_typed_and_atomic() {
    let mut runtime = fixture();
    for source in [
        "1 < 2",
        "1 + (1 < 2)",
        "IF(1, 2, 3)",
        "IF(1=1, 2, NOT(1=1))",
        "AND(1=1, 1)",
        "OR(1=1, 0)",
        "NOT(1)",
        "ROUND(1, -1)",
        "ROUND(1, [r1.m])",
        "ISBLANK(1)",
    ] {
        let before = runtime.export_project_v3("resident/0").unwrap().bytes;
        let error = update(&mut runtime, 0, "r1.n", source).unwrap_err();
        let failure = error.failure_projection("resident/0");
        assert_eq!(failure.code, "formula_type_mismatch", "{source}");
        assert_eq!(failure.diagnostics[0].code, "formula_type_mismatch");
        assert!(
            failure.diagnostics[0]
                .facts
                .iter()
                .any(|fact| fact.name == "position" && fact.value == "1")
        );
        assert_eq!(
            runtime.export_project_v3("resident/0").unwrap().bytes,
            before
        );
        assert_eq!(stored_number(&field(&mut runtime, 0, "r1.n")), Some(2.0));
    }
}
