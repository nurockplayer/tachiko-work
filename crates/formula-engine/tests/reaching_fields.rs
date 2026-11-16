use std::collections::{BTreeMap, BTreeSet};

use tachiko_formula_engine::calculate;
use tachiko_semantic_core::{
    ColumnRef, Document, DocumentId, Entity, EntityId, EntityKey, Expression, FieldConstraint,
    FieldDefinition, FieldId, FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey,
    Value,
};

fn field(entity: &str) -> FieldRef {
    FieldRef::new(entity, "v")
}

fn schema(id: &str) -> Schema {
    Schema {
        id: SchemaId::from(id),
        key: SchemaKey::from(id),
        display_name: None,
        field_order: None,
        fields: BTreeMap::from([(
            FieldId::from("v"),
            FieldDefinition {
                id: FieldId::from("v"),
                key: FieldKey::from("v"),
                display_name: None,
                field_type: FieldType::Number,
                required: false,
                constraint: FieldConstraint::None,
                number_format: None,
                date_format: None,
            },
        )]),
    }
}

fn row(id: &str, schema: &str, value: Option<Value>) -> Entity {
    Entity {
        id: EntityId::from(id),
        key: EntityKey::from(id),
        schema: SchemaId::from(schema),
        fields: value
            .into_iter()
            .map(|value| (FieldId::from("v"), value))
            .collect(),
    }
}

fn document() -> Document {
    let rows = [
        row(
            "a",
            "inputs",
            Some(Value::Number(Number::new(2.0).unwrap())),
        ),
        row("blank", "inputs", None),
        row(
            "sum",
            "outputs",
            Some(Value::Formula(Expression::ColumnTotal(ColumnRef::new(
                "inputs", "v",
            )))),
        ),
        row(
            "tail",
            "outputs",
            Some(Value::Formula(Expression::Reference(field("sum")))),
        ),
        row(
            "other",
            "outputs",
            Some(Value::Formula(Expression::Number(Number::default()))),
        ),
    ];
    Document {
        id: DocumentId::from("reaching-fields"),
        title: "Causal traversal".to_owned(),
        schemas: [schema("inputs"), schema("outputs")]
            .into_iter()
            .map(|schema| (schema.id.clone(), schema))
            .collect(),
        entities: rows.into_iter().map(|row| (row.id.clone(), row)).collect(),
        keyed_grouped_sum_definitions: BTreeMap::new(),
    }
}

#[test]
fn requested_targets_keep_exact_causes_including_blank_column_slots() {
    let calculation = calculate(&document()).unwrap();
    let changed = BTreeSet::from([
        field("a"),
        field("blank"),
        field("sum"),
        field("tail"),
        field("other"),
    ]);
    let targets = BTreeSet::from([field("sum"), field("tail"), field("a"), field("missing")]);
    assert_eq!(
        calculation.reaching_fields(&changed, &targets),
        BTreeMap::from([
            (field("sum"), BTreeSet::from([field("a"), field("blank")])),
            (
                field("tail"),
                BTreeSet::from([field("a"), field("blank"), field("sum")])
            ),
        ])
    );
    for target in targets {
        let expected = changed
            .iter()
            .filter(|changed| calculation.affected_by(changed).contains(&target))
            .cloned()
            .collect::<BTreeSet<_>>();
        let actual = calculation.reaching_fields(&changed, &BTreeSet::from([target.clone()]));
        assert_eq!(actual.get(&target).cloned().unwrap_or_default(), expected);
    }
}

#[test]
fn empty_requests_and_duplicate_changes_do_not_create_causes() {
    let calculation = calculate(&document()).unwrap();
    let targets = BTreeSet::from([field("sum")]);
    assert!(calculation.reaching_fields([], &targets).is_empty());
    assert!(
        calculation
            .reaching_fields([&field("a")], &BTreeSet::new())
            .is_empty()
    );
    assert_eq!(
        calculation.reaching_fields([&field("a"), &field("a")], &targets),
        BTreeMap::from([(field("sum"), BTreeSet::from([field("a")]))])
    );
}
