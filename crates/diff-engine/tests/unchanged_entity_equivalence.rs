//! Literal semantic acceptance for #497. No optimization or candidate oracle.
use std::collections::BTreeMap;

use tachiko_diff_engine::{DiffError, SemanticChange as Change, diff};
use tachiko_formula_engine::CalculationError;
use tachiko_semantic_core::{
    Document, Entity, Expression, FieldConstraint, FieldDefinition, FieldRef, FieldType,
    KeyedGroupedSumDefinition, KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding,
    Number, Schema, Value,
};

fn number(value: f64) -> Number {
    Number::new(value).unwrap()
}
fn literal(value: f64) -> Expression {
    Expression::Number(number(value))
}
fn reference(entity: &str) -> Expression {
    Expression::Reference(FieldRef::new(entity, "n"))
}
fn definition(id: &str, field_type: FieldType) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        field_type,
        required: false,
        constraint: FieldConstraint::None,
    }
}
fn entity(id: &str, name: &str) -> Entity {
    Entity {
        id: id.into(),
        key: id.into(),
        schema: "s".into(),
        fields: BTreeMap::from([
            ("name".into(), Value::Text(name.into())),
            ("n".into(), Value::Number(number(0.0))),
        ]),
    }
}
fn fixture() -> Document {
    let mut document = Document::empty("doc", "Diff equivalence");
    document.schemas.insert(
        "s".into(),
        Schema {
            id: "s".into(),
            key: "meters".into(),
            fields: [
                definition("extra", FieldType::Text),
                definition("f", FieldType::Number),
                definition("link", FieldType::Reference { schema: "s".into() }),
                definition("n", FieldType::Number),
                definition("name", FieldType::Text),
            ]
            .into_iter()
            .map(|field| (field.id.clone(), field))
            .collect(),
        },
    );
    for (id, name) in [("a", "Input"), ("b", "Dependent"), ("c", "Tail")] {
        document.entities.insert(id.into(), entity(id, name));
    }
    document
        .entities
        .get_mut("a")
        .unwrap()
        .fields
        .insert("n".into(), Value::Number(number(2.0)));
    document.entities.get_mut("b").unwrap().fields.insert(
        "f".into(),
        Value::Formula(Expression::Multiply {
            left: Box::new(reference("a")),
            right: Box::new(literal(2.0)),
        }),
    );
    document.entities.get_mut("c").unwrap().fields.insert(
        "f".into(),
        Value::Formula(Expression::Add {
            left: Box::new(Expression::Reference(FieldRef::new("b", "f"))),
            right: Box::new(literal(1.0)),
        }),
    );
    document
}
fn impacts() -> Vec<Change> {
    vec![
        Change::FieldChanged {
            field: FieldRef::new("a", "n"),
            before: Value::Number(number(2.0)),
            after: Value::Number(number(3.0)),
        },
        Change::FormulaImpact {
            field: FieldRef::new("b", "f"),
            before: number(4.0),
            after: number(6.0),
            causes: vec![FieldRef::new("a", "n")],
        },
        Change::FormulaImpact {
            field: FieldRef::new("c", "f"),
            before: number(5.0),
            after: number(7.0),
            causes: vec![FieldRef::new("a", "n")],
        },
    ]
}

#[test]
fn unchanged_entities_keep_transitive_impacts_descriptors_and_owned_results() {
    let mut before = fixture();
    let mut after = before.clone();
    after
        .entities
        .get_mut("a")
        .unwrap()
        .fields
        .insert("n".into(), Value::Number(number(3.0)));
    assert_eq!(before.entities["b"], after.entities["b"]);
    assert_eq!(before.entities["c"], after.entities["c"]);
    let observed = diff(&before, &after).unwrap();
    let expected = "Meters Input\nn: 2 -> 3\n\nMeters Dependent\naffected f: 4 -> 6\n\nMeters Tail\naffected f: 5 -> 7\n";
    assert_eq!(observed.changes(), impacts());
    assert_eq!(observed.render_text(), expected);
    // A borrowed input mutation must not alter either retained owned snapshot.
    before.entities.clear();
    before.schemas.clear();
    after
        .entities
        .get_mut("b")
        .unwrap()
        .fields
        .insert("name".into(), Value::Text("mutated".into()));
    after.schemas.get_mut("s").unwrap().key = "mutated".into();
    drop(before);
    drop(after);
    assert_eq!(observed.changes(), impacts());
    assert_eq!(observed.render_text(), expected);
}

#[test]
fn equal_calculated_output_does_not_hide_a_formula_definition_change() {
    let before = fixture();
    let mut after = before.clone();
    let changed = Value::Formula(Expression::Add {
        left: Box::new(literal(2.0)),
        right: Box::new(literal(2.0)),
    });
    after
        .entities
        .get_mut("b")
        .unwrap()
        .fields
        .insert("f".into(), changed.clone());
    assert_eq!(
        diff(&before, &after).unwrap().changes(),
        [Change::FieldChanged {
            field: FieldRef::new("b", "f"),
            before: Value::Formula(Expression::Multiply {
                left: Box::new(reference("a")),
                right: Box::new(literal(2.0))
            }),
            after: changed,
        }]
    );
    assert_eq!(
        diff(&before, &after).unwrap().render_text(),
        "Meters Dependent\nf: ([a.n] * 2) -> (2 + 2)\n"
    );
}

#[test]
fn schema_key_and_field_key_changes_preserve_unchanged_entity_presentation() {
    let before = fixture();
    let mut after = before.clone();
    let schema = after.schemas.get_mut("s").unwrap();
    schema.key = "gauges".into();
    schema.fields.get_mut("n").unwrap().key = "amount".into();
    after
        .entities
        .get_mut("a")
        .unwrap()
        .fields
        .insert("n".into(), Value::Number(number(3.0)));
    assert_eq!(before.entities["b"], after.entities["b"]);
    let mut expected = vec![
        Change::SchemaKeyChanged {
            schema: "s".into(),
            before: "meters".into(),
            after: "gauges".into(),
        },
        Change::FieldKeyChanged {
            schema: "s".into(),
            field: "n".into(),
            before: "n".into(),
            after: "amount".into(),
        },
    ];
    expected.extend(impacts());
    let observed = diff(&before, &after).unwrap();
    assert_eq!(observed.changes(), expected);
    assert_eq!(
        observed.render_text(),
        "Schema gauges\nkey: meters -> gauges\nfield key: n -> amount\n\nGauges Input\namount: 2 -> 3\n\nGauges Dependent\naffected f: 4 -> 6\n\nGauges Tail\naffected f: 5 -> 7\n"
    );
}

#[test]
fn additions_removals_keys_text_and_references_keep_exact_order() {
    let mut before = fixture();
    let removed = Schema {
        id: "old".into(),
        key: "old".into(),
        fields: BTreeMap::new(),
    };
    let added = Schema {
        id: "new".into(),
        key: "new".into(),
        fields: BTreeMap::new(),
    };
    before.schemas.insert("old".into(), removed.clone());
    before.entities.insert("z".into(), entity("z", "Removed"));
    before
        .entities
        .get_mut("b")
        .unwrap()
        .fields
        .insert("link".into(), Value::Reference("a".into()));
    before
        .entities
        .get_mut("a")
        .unwrap()
        .fields
        .insert("extra".into(), Value::Text("old".into()));
    let mut after = before.clone();
    after.schemas.remove("old");
    after.schemas.insert("new".into(), added.clone());
    after.entities.remove("z");
    after.entities.insert("d".into(), entity("d", "Added"));
    let source = after.entities.get_mut("a").unwrap();
    source.key = "renamed".into();
    source.fields.remove("extra");
    source
        .fields
        .insert("name".into(), Value::Text("Renamed".into()));
    after
        .entities
        .get_mut("b")
        .unwrap()
        .fields
        .insert("link".into(), Value::Reference("c".into()));
    after
        .entities
        .get_mut("b")
        .unwrap()
        .fields
        .insert("extra".into(), Value::Text("雪".into()));
    let observed = diff(&before, &after).unwrap();
    assert_eq!(
        observed.changes(),
        [
            Change::SchemaAdded {
                schema: "new".into(),
                definition: added
            },
            Change::SchemaRemoved {
                schema: "old".into(),
                definition: removed
            },
            Change::EntityKeyChanged {
                entity: "a".into(),
                before: "a".into(),
                after: "renamed".into()
            },
            Change::FieldRemoved {
                field: FieldRef::new("a", "extra"),
                value: Value::Text("old".into())
            },
            Change::FieldChanged {
                field: FieldRef::new("a", "name"),
                before: Value::Text("Input".into()),
                after: Value::Text("Renamed".into())
            },
            Change::FieldAdded {
                field: FieldRef::new("b", "extra"),
                value: Value::Text("雪".into())
            },
            Change::FieldChanged {
                field: FieldRef::new("b", "link"),
                before: Value::Reference("a".into()),
                after: Value::Reference("c".into())
            },
            Change::EntityAdded { entity: "d".into() },
            Change::EntityRemoved { entity: "z".into() },
        ]
    );
    assert_eq!(
        observed.render_text(),
        "Schema new\nschema added\n\nSchema old\nschema removed\n\nMeters Renamed\nkey: a -> renamed\nextra removed: \"old\"\nname: \"Input\" -> \"Renamed\"\n\nMeters Dependent\nextra added: \"雪\"\nlink: reference(a) -> reference(c)\n\nMeters Added\nentity added\n\nMeters Removed\nentity removed\n"
    );
}

#[test]
fn equality_never_bypasses_refusal_or_before_after_error_priority() {
    let valid = fixture();
    let equal = diff(&valid, &valid).unwrap();
    assert_eq!(equal.changes(), &[]);
    assert_eq!(equal.render_text(), "No semantic changes.\n");
    let mut invalid = valid.clone();
    invalid.entities.get_mut("b").unwrap().fields.insert(
        "f".into(),
        Value::Formula(Expression::Reference(FieldRef::new("absent", "n"))),
    );
    for (before, after, before_error) in [
        (&invalid, &invalid, true),
        (&invalid, &valid, true),
        (&valid, &invalid, false),
    ] {
        let expected = CalculationError::MissingReference {
            reference: FieldRef::new("absent", "n"),
        };
        match diff(before, after).unwrap_err() {
            DiffError::BeforeCalculation(source) if before_error => assert_eq!(source, expected),
            DiffError::AfterCalculation(source) if !before_error => assert_eq!(source, expected),
            other => panic!("wrong error stage: {other:?}"),
        }
    }
    let mut changed_definition = invalid.clone();
    changed_definition.keyed_grouped_sum_definitions.insert(
        "sum".into(),
        KeyedGroupedSumDefinition {
            id: "sum".into(),
            orders: KeyedGroupedSumOrdersBinding {
                schema: "orders".into(),
                lookup_key_field: "lookup".into(),
                quantity_field: "quantity".into(),
            },
            products: KeyedGroupedSumProductsBinding {
                schema: "products".into(),
                key_field: "key".into(),
                category_field: "category".into(),
                price_field: "price".into(),
            },
        },
    );
    assert!(matches!(
        diff(&invalid, &changed_definition),
        Err(DiffError::UnsupportedKeyedGroupedSumDefinitionChange)
    ));
    invalid
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("n")
        .unwrap()
        .constraint = FieldConstraint::NumberInclusiveRange {
        min: number(0.0),
        max: number(10.0),
    };
    assert!(matches!(
        diff(&invalid, &changed_definition),
        Err(DiffError::UnsupportedFieldConstraintChange)
    ));
    assert!(matches!(
        diff(&invalid, &invalid),
        Err(DiffError::UnsupportedFieldConstraintChange)
    ));
}
