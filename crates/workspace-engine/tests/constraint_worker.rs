use std::collections::BTreeMap;

use tachiko_workspace_engine::{
    Document, Entity, Expression, FieldConstraint, FieldDefinition, FieldType, Number, Schema,
    Value, patch_lifecycle::DocumentScopeId, resident_session::ResidentWorkspaceSession,
};

#[test]
fn resident_complete_formula_validation_matches_the_authoritative_report() {
    let mut document = Document::empty("document", "Constraint resident parity");
    document.schemas.insert(
        "schema".into(),
        Schema {
            id: "schema".into(),
            key: "items".into(),
            fields: BTreeMap::from([(
                "total".into(),
                FieldDefinition {
                    id: "total".into(),
                    key: "total".into(),
                    field_type: FieldType::Number,
                    required: true,
                    constraint: FieldConstraint::NumberInclusiveRange {
                        min: Number::new(0.0).expect("finite minimum"),
                        max: Number::new(10.0).expect("finite maximum"),
                    },
                },
            )]),
        },
    );
    document.entities.insert(
        "entity".into(),
        Entity {
            id: "entity".into(),
            key: "item".into(),
            schema: "schema".into(),
            fields: BTreeMap::from([(
                "total".into(),
                Value::Formula(Expression::Number(
                    Number::new(11.0).expect("finite formula result"),
                )),
            )]),
        },
    );

    let authoritative = tachiko_workspace_engine::validation_report(&document);
    let resident = ResidentWorkspaceSession::new(DocumentScopeId::from("occurrence"), document);

    assert_eq!(resident.validation_report().value(), &authoritative);
    assert!(
        resident
            .validation_report()
            .value()
            .diagnostics()
            .iter()
            .any(|diagnostic| {
                diagnostic.code.as_str() == "core.field_value_constraint_mismatch"
                    && diagnostic.path == "entities.entity.fields.total"
            })
    );
}
