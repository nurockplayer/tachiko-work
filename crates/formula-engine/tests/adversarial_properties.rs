use std::collections::{BTreeMap, BTreeSet};

use tachiko_formula_engine::calculate;
use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, Expression, FieldDefinition, FieldId, FieldKey,
    FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

fn next(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}

fn field_id(index: usize) -> String {
    format!("field-{index:02}")
}

fn numeric(value: f64) -> Expression {
    Expression::Number(Number::new(value).unwrap())
}

fn generate_expression(
    state: &mut u64,
    prior: &[Number],
    depth: usize,
) -> (Expression, Number, BTreeSet<FieldRef>) {
    if depth == 0 || next(state) % 4 == 0 {
        if next(state) % 2 == 0 {
            let index = usize::try_from(next(state) % u64::try_from(prior.len()).unwrap()).unwrap();
            let reference = FieldRef::new("entity", field_id(index));
            return (
                Expression::Reference(reference.clone()),
                prior[index],
                BTreeSet::from([reference]),
            );
        }
        let raw = i32::try_from(next(state) % 41).unwrap() - 20;
        let number = Number::new(f64::from(raw) / 4.0).unwrap();
        return (Expression::Number(number), number, BTreeSet::new());
    }

    let (left, left_value, mut dependencies) = generate_expression(state, prior, depth - 1);
    let (mut right, mut right_value, mut right_dependencies) =
        generate_expression(state, prior, depth - 1);

    let operation = next(state) % 6;
    if operation == 3 && right_value.get() == 0.0 {
        right = numeric(1.0);
        right_value = Number::new(1.0).unwrap();
        right_dependencies.clear();
    }
    dependencies.extend(right_dependencies);
    let result = match operation {
        0 => left_value.get() + right_value.get(),
        1 => left_value.get() - right_value.get(),
        2 => left_value.get() * right_value.get(),
        3 => left_value.get() / right_value.get(),
        4 => left_value.get().min(right_value.get()),
        _ => left_value.get().max(right_value.get()),
    };
    let result = Number::new(result).unwrap();
    let expression = match operation {
        0 => Expression::Add {
            left: Box::new(left),
            right: Box::new(right),
        },
        1 => Expression::Subtract {
            left: Box::new(left),
            right: Box::new(right),
        },
        2 => Expression::Multiply {
            left: Box::new(left),
            right: Box::new(right),
        },
        3 => Expression::Divide {
            left: Box::new(left),
            right: Box::new(right),
        },
        4 => Expression::Minimum {
            left: Box::new(left),
            right: Box::new(right),
        },
        _ => Expression::Maximum {
            left: Box::new(left),
            right: Box::new(right),
        },
    };
    (expression, result, dependencies)
}

fn independent_affected(
    dependencies: &BTreeMap<FieldRef, BTreeSet<FieldRef>>,
    changed: &FieldRef,
) -> Vec<FieldRef> {
    let mut frontier = BTreeSet::from([changed.clone()]);
    let mut affected = BTreeSet::new();
    loop {
        let next: BTreeSet<_> = dependencies
            .iter()
            .filter(|(formula, inputs)| {
                !affected.contains(*formula) && !inputs.is_disjoint(&frontier)
            })
            .map(|(formula, _)| formula.clone())
            .collect();
        if next.is_empty() {
            break;
        }
        frontier.clone_from(&next);
        affected.extend(next);
    }
    affected.into_iter().collect()
}

#[test]
fn generated_acyclic_graphs_match_an_independent_value_and_impact_oracle() {
    const CASES: usize = 5_000;
    const FIELDS: usize = 24;

    let mut state = 0x082e_fa98_ec4e_6c89_u64;
    for case in 0..CASES {
        let mut definitions = BTreeMap::new();
        let mut fields = BTreeMap::new();
        let mut expected = Vec::new();
        let mut dependencies = BTreeMap::new();

        for index in 0..FIELDS {
            let id = field_id(index);
            definitions.insert(
                FieldId::from(id.as_str()),
                FieldDefinition {
                    id: FieldId::from(id.as_str()),
                    key: FieldKey::from(id.as_str()),
                    field_type: FieldType::Number,
                    required: true,
                },
            );
            let (value, result) = if index < 3 || next(&mut state) % 5 == 0 {
                let raw = i32::try_from(next(&mut state) % 33).unwrap() - 16;
                let number = Number::new(f64::from(raw) / 2.0).unwrap();
                (Value::Number(number), number)
            } else {
                let (expression, result, inputs) = generate_expression(&mut state, &expected, 3);
                dependencies.insert(FieldRef::new("entity", id.as_str()), inputs);
                (Value::Formula(expression), result)
            };
            expected.push(result);
            fields.insert(FieldId::from(id), value);
        }

        let document = Document {
            id: DocumentId::from("document"),
            title: format!("Generated case {case}"),
            schemas: BTreeMap::from([(
                SchemaId::from("schema"),
                Schema {
                    id: SchemaId::from("schema"),
                    key: SchemaKey::from("schema"),
                    fields: definitions,
                },
            )]),
            entities: BTreeMap::from([(
                EntityId::from("entity"),
                Entity {
                    id: EntityId::from("entity"),
                    key: "entity".into(),
                    schema: SchemaId::from("schema"),
                    fields,
                },
            )]),
        };

        let calculation = calculate(&document).unwrap();
        assert_eq!(calculate(&document).unwrap(), calculation, "case {case}");
        for (index, expected) in expected.iter().enumerate() {
            let field = FieldRef::new("entity", field_id(index));
            assert_eq!(
                calculation.value(&field).map(Number::to_bits),
                Some(expected.to_bits()),
                "case {case}, field {index}"
            );
            if let Some(inputs) = dependencies.get(&field) {
                assert_eq!(calculation.dependencies_of(&field), Some(inputs));
            }
        }
        for index in [0, FIELDS / 2, FIELDS - 1] {
            let changed = FieldRef::new("entity", field_id(index));
            assert_eq!(
                calculation.affected_by(&changed),
                independent_affected(&dependencies, &changed),
                "case {case}, changed field {index}"
            );
        }
    }
}
