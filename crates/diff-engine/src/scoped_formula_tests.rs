use super::*;
use tachiko_formula_engine::RetainedCalculationState;
use tachiko_semantic_core::{Expression, FieldConstraint, FieldType};

fn number(value: f64) -> Number {
    Number::new(value).unwrap()
}

fn formula_table(rows: usize) -> Document {
    let mut document = Document::empty("doc", "Scoped formula diff");
    document.schemas.insert(
        "s".into(),
        Schema {
            id: "s".into(),
            key: "s".into(),
            display_name: None,
            field_order: None,
            fields: ["n", "a", "b", "c"]
                .into_iter()
                .map(|id| {
                    let field = FieldDefinition {
                        id: id.into(),
                        key: id.into(),
                        display_name: None,
                        number_format: None,
                        date_format: None,
                        field_type: FieldType::Number,
                        required: false,
                        constraint: FieldConstraint::None,
                    };
                    (field.id.clone(), field)
                })
                .collect(),
        },
    );
    for row in 0..rows {
        let id = format!("row-{row:05}");
        let mut fields = BTreeMap::from([("n".into(), Value::Number(number(1.0)))]);
        for field in ["a", "b", "c"] {
            fields.insert(
                field.into(),
                Value::Formula(Expression::Reference(FieldRef::new(id.as_str(), "n"))),
            );
        }
        let entity = Entity {
            id: id.as_str().into(),
            key: id.as_str().into(),
            schema: "s".into(),
            rank: None,
            fields,
        };
        document.entities.insert(entity.id.clone(), entity);
    }
    document
}

#[test]
fn scoped_ten_thousand_rows_reports_three_ordered_impacts_with_full_diff_causes() {
    let before = formula_table(10_000);
    let (before_state, _) = RetainedCalculationState::rebuild(&before);
    let mut after = before.clone();
    let entity = EntityId::from("row-05000");
    let root = FieldRef::new(entity.clone(), "n");
    after
        .entities
        .get_mut(&entity)
        .unwrap()
        .fields
        .insert("n".into(), Value::Number(number(2.0)));
    let mut after_state = before_state.clone();
    let _ = after_state.update(&after, &BTreeSet::from([root.clone()]));
    let before_calculation = before_state.complete().unwrap();
    let after_calculation = after_state.complete().unwrap();
    let changes = diff_changes_scoped(
        &before,
        &after,
        before_calculation,
        after_calculation,
        &DiffScope {
            entities: BTreeSet::from([entity.clone()]),
            ..DiffScope::default()
        },
    );
    let full =
        diff_with_calculations(&before, &after, before_calculation, after_calculation).unwrap();
    assert_eq!(changes, full.changes());
    let impacts = changes
        .into_iter()
        .filter(|change| matches!(change, SemanticChange::FormulaImpact { .. }))
        .collect::<Vec<_>>();
    let expected = ["a", "b", "c"].map(|field| SemanticChange::FormulaImpact {
        field: FieldRef::new(entity.clone(), field),
        before: tachiko_formula_engine::CalculatedValue::Number(number(1.0)),
        after: tachiko_formula_engine::CalculatedValue::Number(number(2.0)),
        causes: vec![root.clone()],
    });
    assert_eq!(impacts, expected);
}

#[test]
fn changed_formula_values_filters_stored_nodes_and_accepts_formulas_on_either_side() {
    let before = formula_table(1);
    let mut after = before.clone();
    let fields = &mut after
        .entities
        .get_mut(&EntityId::from("row-00000"))
        .unwrap()
        .fields;
    fields.insert("a".into(), Value::Number(number(2.0)));
    fields.insert("n".into(), Value::Formula(Expression::Number(number(3.0))));
    fields.remove("c");
    let before_calculation = calculate(&before).unwrap();
    let after_calculation = calculate(&after).unwrap();
    assert_eq!(
        before_calculation.changed_formula_values(&after_calculation),
        [("a", 1.0, 2.0), ("b", 1.0, 3.0), ("n", 1.0, 3.0)].map(|(field, a, b)| (
            FieldRef::new("row-00000", field),
            CalculatedValue::Number(number(a)),
            CalculatedValue::Number(number(b))
        ))
    );
    for (before, after, a, b) in [
        (&before, &after, &before_calculation, &after_calculation),
        (&after, &before, &after_calculation, &before_calculation),
    ] {
        let scoped = diff_changes_scoped(
            before,
            after,
            a,
            b,
            &DiffScope {
                entities: BTreeSet::from([EntityId::from("row-00000")]),
                ..DiffScope::default()
            },
        );
        assert_eq!(
            scoped,
            diff_with_calculations(before, after, a, b)
                .unwrap()
                .changes()
        );
    }
}

#[test]
fn calculated_date_changes_are_reported_in_full_and_scoped_impacts() {
    let mut before = formula_table(1);
    for definition in before.schemas.get_mut("s").unwrap().fields.values_mut() {
        definition.field_type = FieldType::Date;
    }
    before
        .entities
        .get_mut("row-00000")
        .unwrap()
        .fields
        .insert("n".into(), Value::Date("2026-11-12".parse().unwrap()));
    let mut after = before.clone();
    after
        .entities
        .get_mut("row-00000")
        .unwrap()
        .fields
        .insert("n".into(), Value::Date("2026-11-13".parse().unwrap()));
    let a = calculate(&before).unwrap();
    let b = calculate(&after).unwrap();
    let expected = ["a", "b", "c"].map(|field| SemanticChange::FormulaImpact {
        field: FieldRef::new("row-00000", field),
        before: CalculatedValue::Date("2026-11-12".parse().unwrap()),
        after: CalculatedValue::Date("2026-11-13".parse().unwrap()),
        causes: vec![FieldRef::new("row-00000", "n")],
    });
    let full = diff_with_calculations(&before, &after, &a, &b).unwrap();
    assert!(full.render_text().contains("2026-11-12 -> 2026-11-13"));
    let scoped = diff_changes_scoped(
        &before,
        &after,
        &a,
        &b,
        &DiffScope {
            entities: BTreeSet::from([EntityId::from("row-00000")]),
            ..DiffScope::default()
        },
    );
    assert_eq!(scoped, full.changes());
    assert_eq!(
        scoped
            .into_iter()
            .filter(|c| matches!(c, SemanticChange::FormulaImpact { .. }))
            .collect::<Vec<_>>(),
        expected
    );
}
