//! Deterministic capacity fixtures for the Stage A baseline harness (Plan A-01).
//!
//! The generator builds [`Document`] values directly, so it can produce sizes
//! far beyond the runtime's current admission profile (128 rows / 1,024
//! entities). It is test-only and never part of the product surface.
//!
//! Every value is derived from one seeded `SplitMix64` stream in row-major,
//! column-major order, so the same arguments always produce byte-identical
//! documents.

use std::collections::BTreeMap;

use tachiko_workspace_engine::{
    Date, Document, Entity, EntityId, EntityKey, Expression, FieldConstraint, FieldDefinition,
    FieldId, FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

/// Field carrying the shared driver value for the dependent-formula fixture.
pub const RATE_FIELD: &str = "rate";
/// Field carrying one same-row formula.
pub const CALC_FIELD: &str = "calc";
/// Field carrying the column-total formula.
pub const TOTAL_FIELD: &str = "total";
/// First data column; always populated so formulas can reference it.
pub const FIRST_COLUMN: &str = "c0";

/// Number of same-row formulas whose recalculation one rate edit triggers.
///
/// The column total makes the dependent count exactly 100 for this value.
pub const DEPENDENT_FORMULAS: usize = 99;

/// One deterministic capacity fixture request.
#[derive(Clone, Copy, Debug)]
pub struct FixtureSpec {
    pub rows: usize,
    pub columns: usize,
    /// Same-row formula cells; zero produces a formula-free data document.
    pub formula_cells: usize,
    pub seed: u64,
}

impl FixtureSpec {
    #[must_use]
    pub const fn cells(self) -> usize {
        self.rows * self.columns
    }
}

/// Build the fixture document described by `spec`.
///
/// The value mix cycles Number, Text, Date columns and leaves roughly ten
/// percent of data cells unpopulated. `c0` is always populated. When
/// `formula_cells` is non-zero the schema gains a shared `rate` driver cell in
/// row 0, one same-row arithmetic formula per formula cell, and one balanced
/// column-total formula.
#[must_use]
#[allow(clippy::too_many_lines)] // One fixture description reads best in one place.
pub fn capacity_document(spec: FixtureSpec) -> Document {
    assert!(spec.rows > 0, "a capacity fixture needs at least one row");
    assert!(
        spec.columns > 0,
        "a capacity fixture needs at least one column"
    );
    assert!(
        spec.formula_cells <= spec.rows,
        "formula cells must fit within the fixture rows"
    );
    let mut rng = SplitMix64(spec.seed);
    let mut document = Document::empty("capacity-benchmark", "Capacity Fixture");
    let mut schema = Schema {
        id: SchemaId::from("grid"),
        key: SchemaKey::from("grid"),
        display_name: None,
        fields: BTreeMap::new(),
    };
    for column in 0..spec.columns {
        let id = column_id(column);
        schema.fields.insert(
            id.clone(),
            FieldDefinition {
                id,
                key: FieldKey::from(column_key(column)),
                display_name: None,
                field_type: column_type(column),
                required: false,
                constraint: FieldConstraint::None,
            },
        );
    }
    if spec.formula_cells > 0 {
        for (id, field_type) in [
            (RATE_FIELD, FieldType::Number),
            (CALC_FIELD, FieldType::Number),
            (TOTAL_FIELD, FieldType::Number),
        ] {
            schema.fields.insert(
                FieldId::from(id),
                FieldDefinition {
                    id: FieldId::from(id),
                    key: FieldKey::from(id),
                    display_name: None,
                    field_type,
                    required: false,
                    constraint: FieldConstraint::None,
                },
            );
        }
    }
    document.schemas.insert(schema.id.clone(), schema);

    for row in 0..spec.rows {
        let entity_id = entity_id(row);
        let mut fields = BTreeMap::new();
        for column in 0..spec.columns {
            let id = column_id(column);
            // Keep `c0` populated; blank roughly one in ten other cells.
            let blank = column > 0 && rng.next() % 10 == 0;
            if blank {
                continue;
            }
            fields.insert(id, column_value(column, row, rng.next()));
        }
        document.entities.insert(
            entity_id.clone(),
            Entity {
                id: entity_id.clone(),
                key: EntityKey::from(format!("k{row:05}")),
                schema: SchemaId::from("grid"),
                fields,
            },
        );
    }

    if spec.formula_cells > 0 {
        apply_formulas(&mut document, spec.formula_cells);
    }
    document
}

/// Populate the shared driver, the same-row formulas and the column total.
fn apply_formulas(document: &mut Document, formula_cells: usize) {
    let driver = FieldRef::new(entity_id(0), RATE_FIELD);
    let rate = document
        .entities
        .get_mut(&entity_id(0))
        .expect("row 0 exists");
    rate.fields
        .insert(FieldId::from(RATE_FIELD), Value::Number(number(2.0)));
    for row in 0..formula_cells {
        let id = FieldId::from(CALC_FIELD);
        let expression = Expression::Multiply {
            left: Box::new(Expression::Reference(driver.clone())),
            right: Box::new(Expression::SameRowReference(FieldId::from(FIRST_COLUMN))),
        };
        document
            .entities
            .get_mut(&entity_id(row))
            .expect("formula row exists")
            .fields
            .insert(id, Value::Formula(expression));
    }
    let terms = (0..formula_cells)
        .map(|row| FieldRef::new(entity_id(row), CALC_FIELD))
        .collect::<Vec<_>>();
    document
        .entities
        .get_mut(&entity_id(0))
        .expect("row 0 exists")
        .fields
        .insert(
            FieldId::from(TOTAL_FIELD),
            Value::Formula(balanced_sum(&terms)),
        );
}

fn balanced_sum(terms: &[FieldRef]) -> Expression {
    debug_assert!(!terms.is_empty(), "a column total needs at least one term");
    if terms.len() == 1 {
        return Expression::Reference(terms[0].clone());
    }
    let middle = terms.len() / 2;
    Expression::Add {
        left: Box::new(balanced_sum(&terms[..middle])),
        right: Box::new(balanced_sum(&terms[middle..])),
    }
}

fn column_id(column: usize) -> FieldId {
    FieldId::from(column_key(column))
}

fn column_key(column: usize) -> String {
    format!("c{column}")
}

const fn column_type(column: usize) -> FieldType {
    match column % 3 {
        0 => FieldType::Number,
        1 => FieldType::Text,
        _ => FieldType::Date,
    }
}

fn column_value(column: usize, row: usize, random: u64) -> Value {
    match column % 3 {
        0 => Value::Number(number(
            f64::from(u32::try_from(random % 100_000).expect("bounded")) / 100.0,
        )),
        1 => {
            let value = random % 10_000;
            Value::Text(format!("item-{value:05}"))
        }
        _ => Value::Date(
            Date::new(
                2024,
                u8::try_from(1 + random % 12).expect("bounded"),
                u8::try_from(1 + u64::try_from(row).unwrap_or(u64::MAX).wrapping_add(random) % 28)
                    .expect("bounded"),
            )
            .expect("fixture dates are valid"),
        ),
    }
}

fn number(value: f64) -> Number {
    Number::new(value).expect("fixture numbers are finite")
}

/// Stable row identity used by every fixture size.
#[must_use]
pub fn entity_id(row: usize) -> EntityId {
    EntityId::from(format!("r{row:05}"))
}

/// Stable field identity of one data column.
#[must_use]
pub fn field_id(column: usize) -> FieldId {
    column_id(column)
}

/// Small deterministic `SplitMix64` stream; reproducibility matters more than
/// statistical quality.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }
}
