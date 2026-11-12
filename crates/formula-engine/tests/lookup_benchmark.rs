//! Release-mode performance evidence for Issue #668.

use std::time::{Duration, Instant};

use tachiko_formula_engine::{
    CalculationOutcome, bind_expression, calculate_complete, parse_expression,
};
use tachiko_semantic_core::{
    Document, Entity, FieldConstraint, FieldDefinition, FieldRef, FieldType, Number, Schema, Value,
    validate_document,
};

fn field(id: &str, field_type: FieldType) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        display_name: None,
        field_type,
        required: false,
        constraint: FieldConstraint::None,
        number_format: None,
        date_format: None,
    }
}

fn product_index(order: usize, rows: usize) -> usize {
    // Pair adjacent orders so keys repeat while covering the whole table.
    ((order / 2) * 7919) % rows
}

fn price(index: usize) -> Number {
    Number::new(f64::from(u32::try_from(index + 1).unwrap())).unwrap()
}

fn document(rows: usize) -> Document {
    let mut document = Document::empty("lookup-benchmark", "Lookup benchmark");
    for schema in ["products", "orders"] {
        document.schemas.insert(
            schema.into(),
            Schema {
                id: schema.into(),
                key: schema.into(),
                display_name: None,
                field_order: None,
                fields: [
                    field("sku", FieldType::Text),
                    field("price", FieldType::Number),
                ]
                .into_iter()
                .map(|field| (field.id.clone(), field))
                .collect(),
            },
        );
    }
    for index in 0..rows {
        for (schema, product) in [("products", index), ("orders", product_index(index, rows))] {
            let id = format!("{schema}-{index:05}");
            let mut fields = [("sku".into(), Value::Text(format!("SKU-{:05}", product + 1)))]
                .into_iter()
                .collect::<std::collections::BTreeMap<_, _>>();
            if schema == "products" {
                fields.insert("price".into(), Value::Number(price(product)));
            }
            document.entities.insert(
                id.clone().into(),
                Entity {
                    id: id.clone().into(),
                    key: id.into(),
                    schema: schema.into(),
                    fields,
                },
            );
        }
    }
    let expression = bind_expression(
        &document,
        &FieldRef::new("orders-00000", "price"),
        &parse_expression("LOOKUP([.sku], [products.sku:column], [products.price:column])")
            .unwrap(),
    )
    .unwrap();
    for index in 0..rows {
        document
            .entities
            .get_mut(format!("orders-{index:05}").as_str())
            .unwrap()
            .fields
            .insert("price".into(), Value::Formula(expression.clone()));
    }
    document
}

fn run_case(rows: usize) -> Duration {
    let document = document(rows);
    let start = Instant::now();
    let diagnostics = validate_document(&document);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let outcome = calculate_complete(&document);
    let elapsed = start.elapsed();
    let CalculationOutcome::Complete(calculation) = outcome else {
        panic!("lookup calculation must succeed");
    };
    let values = calculation.values();
    for index in [0, rows / 2, rows - 1] {
        assert_eq!(
            values[&FieldRef::new(format!("orders-{index:05}"), "price")],
            price(product_index(index, rows))
        );
    }
    let actual_sum: f64 = (0..rows)
        .map(|index| values[&FieldRef::new(format!("orders-{index:05}"), "price")].get())
        .sum();
    let expected_sum: f64 = (0..rows)
        .map(|index| price(product_index(index, rows)).get())
        .sum();
    assert_eq!(actual_sum, expected_sum);
    elapsed
}

#[test]
#[ignore = "run explicitly in release mode to record Issue #668 lookup evidence"]
fn lookup_full_calculation_benchmark() {
    println!("rows,lookups,full_calc_ms");
    for rows in [1_000, 10_000] {
        let elapsed = run_case(rows);
        println!("{rows},{rows},{:.3}", elapsed.as_secs_f64() * 1_000.0);
    }
    let t2000 = run_case(2_000);
    let t8000 = run_case(8_000);
    // Four times as many rows is about 4x with one index per pass (5x with the
    // index's log factor) and 16x if each lookup scanned the key column, so
    // 10x leaves room for runner noise and still rejects quadratic growth.
    assert!(t8000 < t2000 * 10, "2,000: {t2000:?}, 8,000: {t8000:?}");
}
