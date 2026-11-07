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
    ColumnCriterion, ColumnRef, ComparisonOperator, ConditionalAggregate, Date, Document, Entity,
    EntityId, EntityKey, Expression, FieldConstraint, FieldDefinition, FieldId, FieldKey, FieldRef,
    FieldType, Number, Schema, SchemaId, SchemaKey, Value,
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
        field_order: None,
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
                number_format: None,
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
                    number_format: None,
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

/// Rows in every fixed W1 shape (Plan W1 scratch measurement).
pub const W1_ROWS: usize = 10_000;

/// Build one W1 working-size shape and return it with its rows and columns.
///
/// The value mix, blank pattern and seeded stream match [`capacity_document`];
/// the derived columns replace the named last columns and become Number.
/// Returns `None` for an unknown shape name.
#[must_use]
pub fn w1_document(shape: &str) -> Option<(Document, usize, usize)> {
    if shape == "a100" {
        // The old table's 100-dependent case: 99 same-row formulas plus the
        // balanced total; edits `rate` on row 0.
        return Some((
            capacity_document(FixtureSpec {
                rows: W1_ROWS,
                columns: 10,
                formula_cells: DEPENDENT_FORMULAS,
                seed: 0x00C0_FFEE_D00D_1234,
            }),
            W1_ROWS,
            10,
        ));
    }
    let (rows, columns, derived, _) = w1_definition(shape)?;
    Some((w1_grid(rows, columns, &derived, None), rows, columns))
}

/// Like [`w1_document`], but the column phase 7 copies holds its formula on
/// row 0 only and a Number on every other row, so one `CopyFormula` request
/// writes one formula per destination row instead of changing nothing.
#[must_use]
pub fn w1_copy_document(shape: &str) -> Option<(Document, usize, usize)> {
    if shape == "a100" {
        return w1_document(shape);
    }
    let (rows, columns, derived, primary) = w1_definition(shape)?;
    Some((w1_grid(rows, columns, &derived, primary), rows, columns))
}

/// Shape geometry, its always-formula columns, and the column the copy phase
/// seeds on row 0 only.
type W1Definition = (usize, usize, Vec<(usize, Expression)>, Option<usize>);

fn w1_definition(shape: &str) -> Option<W1Definition> {
    Some(match shape {
        "a" => (W1_ROWS, 10, Vec::new(), Some(9)),
        "b1" => (W1_ROWS, 10, vec![(9, w1_product())], Some(9)),
        "b3" => (
            W1_ROWS,
            10,
            vec![(9, w1_product()), (8, w1_sum()), (7, w1_chain())],
            Some(9),
        ),
        "d" => (
            W1_ROWS,
            30,
            vec![
                (29, w1_product()),
                (28, w1_sum()),
                (27, w1_chain_over("c29")),
            ],
            Some(29),
        ),
        _ => {
            if let Some(rows) = shape.strip_prefix("c1-") {
                let rows = rows.parse().ok()?;
                (rows, 10, vec![(9, w1_column_ratio())], Some(9))
            } else {
                let rows = shape.strip_prefix("c2-")?;
                let rows = rows.parse().ok()?;
                (rows, 10, vec![(9, w1_column_conditional())], Some(9))
            }
        }
    })
}

/// The grid used by every W1 shape: the [`capacity_document`] value stream
/// with the listed columns replaced by one formula in every row. A column
/// named by `row_zero_only` instead gets its formula on row 0 and a Number
/// everywhere else.
fn w1_grid(
    rows: usize,
    columns: usize,
    derived: &[(usize, Expression)],
    row_zero_only: Option<usize>,
) -> Document {
    let mut rng = SplitMix64(0x00C0_FFEE_D00D_1234);
    let mut document = Document::empty("capacity-benchmark", "W1 Shape");
    let mut schema = Schema {
        field_order: None,
        id: SchemaId::from("grid"),
        key: SchemaKey::from("grid"),
        display_name: None,
        fields: BTreeMap::new(),
    };
    for column in 0..columns {
        let id = column_id(column);
        let field_type = if derived.iter().any(|(index, _)| *index == column) {
            FieldType::Number
        } else {
            column_type(column)
        };
        schema.fields.insert(
            id.clone(),
            FieldDefinition {
                key: FieldKey::from(column_key(column)),
                id,
                display_name: None,
                field_type,
                required: false,
                constraint: FieldConstraint::None,
                number_format: None,
            },
        );
    }
    document.schemas.insert(schema.id.clone(), schema);
    for row in 0..rows {
        let entity_id = entity_id(row);
        let mut fields = BTreeMap::new();
        for column in 0..columns {
            let blank = column > 0 && rng.next() % 10 == 0;
            if blank {
                continue;
            }
            fields.insert(column_id(column), column_value(column, row, rng.next()));
        }
        for (column, expression) in derived {
            let id = column_id(*column);
            let value = if row_zero_only == Some(*column) && row > 0 {
                // A plain Number on every row but 0, so the copy has work to do
                // and the column still parses as Number.
                Value::Number(number(
                    1.0 + f64::from(u32::try_from(row % 100_000).expect("bounded")) / 10.0,
                ))
            } else {
                Value::Formula(expression.clone())
            };
            fields.insert(id, value);
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
    document
}

fn w1_same_row(field: &str) -> Expression {
    Expression::SameRowReference(FieldId::from(field))
}

fn w1_product() -> Expression {
    Expression::Multiply {
        left: Box::new(w1_same_row("c0")),
        right: Box::new(w1_same_row("c3")),
    }
}

fn w1_sum() -> Expression {
    Expression::Add {
        left: Box::new(w1_same_row("c0")),
        right: Box::new(w1_same_row("c6")),
    }
}

fn w1_chain() -> Expression {
    w1_chain_over("c9")
}

fn w1_chain_over(source: &str) -> Expression {
    Expression::Multiply {
        left: Box::new(w1_same_row(source)),
        right: Box::new(Expression::Number(number(1.1))),
    }
}

fn w1_column_ratio() -> Expression {
    Expression::Divide {
        left: Box::new(w1_same_row("c0")),
        right: Box::new(Expression::ColumnTotal(ColumnRef::new("grid", "c0"))),
    }
}

fn w1_column_conditional() -> Expression {
    Expression::ColumnConditional {
        aggregate: Box::new(ConditionalAggregate::Sum(ColumnRef::new("grid", "c0"))),
        criteria: vec![ColumnCriterion {
            column: ColumnRef::new("grid", "c3"),
            operator: ComparisonOperator::Equal,
            value: Some(Box::new(w1_same_row("c3"))),
        }],
    }
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

/// Number of populated data columns in every formula-sweep table.
pub const SWEEP_DATA_COLUMNS: usize = 3;

/// Build the formula-count sweep fixture.
///
/// The document holds `tables` Number tables named `t0..`, each with
/// `rows_per_table` rows. Every row populates the three data columns `c0..c2`
/// and carries `formula_columns` same-row formula columns `f0..`, each
/// `c0 * 2`. Entity ids are `t{table}-r{row:05}` and every entity key is
/// unique across the document.
#[must_use]
pub fn formula_sweep_document(
    tables: usize,
    rows_per_table: usize,
    formula_columns: usize,
) -> Document {
    assert!(tables > 0, "a formula sweep needs at least one table");
    assert!(rows_per_table > 0, "a formula sweep needs at least one row");
    let mut document = Document::empty("capacity-benchmark", "Formula Sweep");
    for table in 0..tables {
        let schema_id = SchemaId::from(format!("t{table}"));
        let mut schema = Schema {
            field_order: None,
            id: schema_id.clone(),
            key: SchemaKey::from(format!("t{table}")),
            display_name: None,
            fields: BTreeMap::new(),
        };
        for column in 0..SWEEP_DATA_COLUMNS {
            let id = FieldId::from(sweep_data_column(column));
            schema.fields.insert(id.clone(), number_field(id));
        }
        for column in 0..formula_columns {
            let id = FieldId::from(sweep_formula_column(column));
            schema.fields.insert(id.clone(), number_field(id));
        }
        document.schemas.insert(schema_id.clone(), schema);

        for row in 0..rows_per_table {
            let entity_id = sweep_entity_id(table, row);
            let mut fields = BTreeMap::new();
            for column in 0..SWEEP_DATA_COLUMNS {
                fields.insert(
                    FieldId::from(sweep_data_column(column)),
                    Value::Number(number(sweep_data_value(row, column))),
                );
            }
            for column in 0..formula_columns {
                fields.insert(
                    FieldId::from(sweep_formula_column(column)),
                    Value::Formula(sweep_formula()),
                );
            }
            document.entities.insert(
                entity_id.clone(),
                Entity {
                    id: entity_id.clone(),
                    key: EntityKey::from(format!("t{table}-r{row:05}")),
                    schema: schema_id.clone(),
                    fields,
                },
            );
        }
    }
    document
}

/// Stable entity identity of one formula-sweep row.
#[must_use]
pub fn sweep_entity_id(table: usize, row: usize) -> EntityId {
    EntityId::from(format!("t{table}-r{row:05}"))
}

fn sweep_data_column(column: usize) -> String {
    format!("c{column}")
}

fn sweep_formula_column(column: usize) -> String {
    format!("f{column}")
}

fn sweep_formula() -> Expression {
    Expression::Multiply {
        left: Box::new(Expression::SameRowReference(FieldId::from(
            sweep_data_column(0),
        ))),
        right: Box::new(Expression::Number(number(2.0))),
    }
}

fn sweep_data_value(row: usize, column: usize) -> f64 {
    let raw = u32::try_from(
        row.saturating_mul(SWEEP_DATA_COLUMNS)
            .saturating_add(column)
            % 997,
    )
    .expect("the modulus bounds the value");
    1.0 + f64::from(raw)
}

fn number_field(id: FieldId) -> FieldDefinition {
    FieldDefinition {
        key: FieldKey::from(id.to_string()),
        id,
        display_name: None,
        field_type: FieldType::Number,
        required: false,
        constraint: FieldConstraint::None,
        number_format: None,
    }
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
