use std::collections::BTreeMap;

use tachiko_semantic_core::{
    Document, Entity, EntityId, FieldDefinition, FieldId, FieldRef, FieldType, Schema, SchemaId,
    Value,
};
use tachiko_workspace_engine::{
    FieldAddress, RuntimeValue, WorkspaceError, calculate_fields, explain_field, runtime_export,
    set_scalar, validate, validate_field_value_suggestion,
};
use tachiko_workspace_engine::{
    patch_lifecycle::DocumentScopeId, resident_session::ResidentWorkspaceSession,
};

fn date_document(value: &str) -> Document {
    let field_id = FieldId::from("published");
    Document {
        id: "dates".into(),
        title: "Dates".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("event"),
            Schema {
                headerless: false,
                field_order: None,
                id: SchemaId::from("event"),
                key: "event".into(),
                display_name: None,
                fields: BTreeMap::from([(
                    field_id.clone(),
                    FieldDefinition {
                        id: field_id,
                        key: "published".into(),
                        display_name: None,
                        number_format: None,
                        date_format: None,
                        field_type: FieldType::Date,
                        required: true,
                        constraint: tachiko_semantic_core::FieldConstraint::None,
                    },
                )]),
            },
        )]),
        entities: BTreeMap::from([(
            EntityId::from("launch"),
            Entity {
                rank: None,
                id: "launch".into(),
                key: "launch".into(),
                schema: "event".into(),
                fields: BTreeMap::from([("published".into(), Value::Date(value.parse().unwrap()))]),
            },
        )]),
        keyed_grouped_sum_definitions: BTreeMap::new(),
    }
}

#[test]
fn stored_dates_are_not_calculation_results() {
    let document = date_document("2026-01-02");
    assert_eq!(calculate_fields(&document).unwrap(), []);
    let session = ResidentWorkspaceSession::new(DocumentScopeId::from("dates"), document);
    assert_eq!(session.calculate_fields().unwrap().value().as_slice(), []);
}

#[test]
fn runtime_export_carries_calculated_date() {
    let mut document = date_document("2026-01-02");
    let mut definition = document.schemas["event"].fields["published"].clone();
    definition.id = "later".into();
    definition.key = "later".into();
    document
        .schemas
        .get_mut("event")
        .unwrap()
        .fields
        .insert("later".into(), definition);
    let owner = FieldRef::new("launch", "later");
    let expression = tachiko_formula_engine::bind_expression(
        &document,
        &owner,
        &tachiko_formula_engine::parse_expression("[.published] + 7").unwrap(),
    )
    .unwrap();
    document
        .entities
        .get_mut("launch")
        .unwrap()
        .fields
        .insert("later".into(), Value::Formula(expression));
    assert_eq!(
        runtime_export(&document).unwrap().entities["launch"].fields["later"],
        RuntimeValue::Date("2026-01-09".parse().unwrap())
    );
    let explanation = explain_field(&document, &FieldAddress::new("launch", "later")).unwrap();
    assert_eq!(explanation.display_value, "2026-01-09");
    let source = explain_field(&document, &FieldAddress::new("launch", "published")).unwrap();
    assert_eq!(source.affected_formulas.len(), 1);
    assert_eq!(source.affected_formulas[0].display_value, "2026-01-09");
    let expected = tachiko_workspace_engine::CalculatedValue::Date("2026-01-09".parse().unwrap());
    let analysis = tachiko_workspace_engine::analyze_field(&document, "dates", &owner).unwrap();
    assert_eq!(analysis.calculated_value, Some(expected));
    let source = tachiko_workspace_engine::analyze_field(
        &document,
        "dates",
        &FieldRef::new("launch", "published"),
    )
    .unwrap();
    assert_eq!(source.downstream_impacts.len(), 1);
    assert_eq!(source.downstream_impacts[0].value, expected);
    let tagged = serde_json::json!({"kind": "date", "value": "2026-01-09"});
    assert_eq!(
        serde_json::to_value(analysis).unwrap()["calculated_value"],
        tagged
    );
    assert_eq!(
        serde_json::to_value(source).unwrap()["downstream_impacts"][0]["value"],
        tagged
    );
}

#[test]
fn date_flows_through_validation_edit_suggestion_runtime_and_resident_queries() {
    let document = date_document("2024-02-29");
    validate(&document).unwrap();

    let exported = runtime_export(&document).unwrap();
    assert_eq!(
        exported.entities["launch"].fields["published"],
        RuntimeValue::Date("2024-02-29".parse().unwrap())
    );

    let suggestion = validate_field_value_suggestion(
        &document,
        FieldRef::new("launch", "published"),
        Value::Date("2025-01-01".parse().unwrap()),
    )
    .unwrap();
    assert_eq!(suggestion.value, Value::Date("2025-01-01".parse().unwrap()));

    let edited = set_scalar(
        &document,
        &FieldAddress::new("launch", "published"),
        "2025-01-01",
    )
    .unwrap();
    assert_eq!(
        edited.document.entities["launch"].fields["published"],
        Value::Date("2025-01-01".parse().unwrap())
    );
    assert!(matches!(
        set_scalar(
            &document,
            &FieldAddress::new("launch", "published"),
            "1900-02-29"
        ),
        Err(WorkspaceError::InvalidValue {
            expected: "date (YYYY-MM-DD)",
            ..
        })
    ));

    let session =
        ResidentWorkspaceSession::new(DocumentScopeId::from("dates-occurrence"), document);
    let projection = session
        .query_fields(&[FieldRef::new("launch", "published")])
        .unwrap();
    assert_eq!(
        projection.value()[0].stored_value,
        Some(Value::Date("2024-02-29".parse().unwrap()))
    );
}
