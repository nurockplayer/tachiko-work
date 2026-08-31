//! Cross-crate exact-admission oracle wiring for Issue #175.

use std::collections::BTreeMap;

use tachiko_storage::issue_175_admit_a0_a1;
use tachiko_workspace_engine::{
    Document, DocumentId, Entity, EntityId, EntityKey, Expression, FieldDefinition, FieldId,
    FieldKey, FieldRef, FieldType, Issue175CalculationOutcome, Number, Schema, SchemaId, SchemaKey,
    Value, issue_175_calculate_complete, validation_report,
};

#[test]
fn issue_175_a0_and_a1_outputs_match_complete_formula_and_validation_oracles() {
    let mut outcomes = BTreeMap::new();
    for (label, expected) in pressure_documents() {
        let (a0, a1) = issue_175_admit_a0_a1(&expected).unwrap();
        assert_eq!(a0, expected, "A0 document mismatch for {label}");
        assert_eq!(a1, expected, "A1 document mismatch for {label}");

        let expected_calculation = issue_175_calculate_complete(&expected);
        assert_eq!(
            issue_175_calculate_complete(&a0),
            expected_calculation,
            "A0 calculation mismatch for {label}"
        );
        assert_eq!(
            issue_175_calculate_complete(&a1),
            expected_calculation,
            "A1 calculation mismatch for {label}"
        );

        let expected_observations = validation_report(&expected).stable_observations();
        assert_eq!(
            validation_report(&a0).stable_observations(),
            expected_observations,
            "A0 validation observations mismatch for {label}"
        );
        assert_eq!(
            validation_report(&a1).stable_observations(),
            expected_observations,
            "A1 validation observations mismatch for {label}"
        );
        outcomes.insert(label, (expected_calculation, expected_observations));
    }

    assert!(matches!(
        outcomes["valid_chain"].0,
        Issue175CalculationOutcome::Complete(_)
    ));
    assert!(outcomes["valid_chain"].1.is_empty());
    assert!(matches!(
        outcomes["cold_numeric_mutation"].0,
        Issue175CalculationOutcome::Complete(_)
    ));
    assert!(outcomes["cold_numeric_mutation"].1.is_empty());
    assert_ne!(
        outcomes["valid_chain"].0, outcomes["cold_numeric_mutation"].0,
        "cold numeric mutation must change the complete calculation"
    );
    for label in ["cross_cold_scc", "division_by_zero"] {
        assert!(matches!(
            outcomes[label].0,
            Issue175CalculationOutcome::Failed(_)
        ));
        assert!(
            !outcomes[label].1.is_empty(),
            "{label} must remain in stable workspace diagnostics"
        );
    }
}

fn pressure_documents() -> Vec<(&'static str, Document)> {
    let chain = chain_document(false);
    let mut changed = chain.clone();
    changed
        .entities
        .get_mut(&EntityId::from("issue-175-oracle-chain-00000000"))
        .unwrap()
        .fields
        .insert(
            FieldId::from("value"),
            Value::Number(Number::new(7.0).unwrap()),
        );
    let cycle = chain_document(true);
    let mut division_by_zero = chain_document(false);
    division_by_zero
        .entities
        .get_mut(&EntityId::from("issue-175-oracle-chain-00000128"))
        .unwrap()
        .fields
        .insert(
            FieldId::from("value"),
            Value::Formula(Expression::Divide {
                left: Box::new(Expression::Number(Number::new(1.0).unwrap())),
                right: Box::new(Expression::Number(Number::new(0.0).unwrap())),
            }),
        );
    vec![
        ("valid_chain", chain),
        ("cold_numeric_mutation", changed),
        ("cross_cold_scc", cycle),
        ("division_by_zero", division_by_zero),
    ]
}

fn chain_document(cycle: bool) -> Document {
    let entity_count = 257;
    let schema_id = SchemaId::from("issue-175-oracle-chain-schema");
    let value_field = FieldId::from("value");
    let entities = (0..entity_count)
        .map(|index| {
            let id = EntityId::from(format!("issue-175-oracle-chain-{index:08}"));
            let value = if index == 0 && !cycle {
                Value::Number(Number::new(1.0).unwrap())
            } else {
                let target = if index == 0 {
                    entity_count - 1
                } else {
                    index - 1
                };
                Value::Formula(Expression::Reference(FieldRef::new(
                    format!("issue-175-oracle-chain-{target:08}"),
                    value_field.clone(),
                )))
            };
            (
                id.clone(),
                Entity {
                    id,
                    key: EntityKey::from(format!("oracle_chain_{index:08}")),
                    schema: schema_id.clone(),
                    fields: BTreeMap::from([(value_field.clone(), value)]),
                },
            )
        })
        .collect();
    Document {
        id: DocumentId::from("issue-175-oracle-chain-document"),
        title: if cycle {
            "Issue 175 cross-cold oracle cycle"
        } else {
            "Issue 175 oracle dependency chain"
        }
        .to_owned(),
        schemas: BTreeMap::from([(
            schema_id.clone(),
            Schema {
                id: schema_id,
                key: SchemaKey::from("oracle_chain"),
                fields: BTreeMap::from([(
                    value_field.clone(),
                    FieldDefinition {
                        id: value_field,
                        key: FieldKey::from("value"),
                        field_type: FieldType::Number,
                        required: true,
                    },
                )]),
            },
        )]),
        entities,
    }
}
