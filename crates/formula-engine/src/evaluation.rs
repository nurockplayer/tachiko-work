use std::collections::{BTreeMap, BTreeSet};

use tachiko_semantic_core::{
    ColumnAggregateFunction, ComparisonOperator, Document, Expression, FieldRef, FieldType, Number,
};

use crate::{CalculationFailure, column_cells};

pub(crate) struct Evaluation<'a> {
    pub(crate) document: &'a Document,
    pub(crate) owner: &'a FieldRef,
    pub(crate) values: &'a BTreeMap<FieldRef, Number>,
}

impl Evaluation<'_> {
    // Static dependencies still govern scheduling, cycles, and invalidation.
    // Failure propagation follows only the branches that are actually read.
    pub(crate) fn required_references(&self, expression: &Expression) -> BTreeSet<FieldRef> {
        let mut references = BTreeSet::new();
        let mut pending = vec![expression];
        while let Some(node) = pending.pop() {
            match node {
                Expression::Reference(reference) => {
                    references.insert(reference.clone());
                }
                Expression::SameRowReference(field) => {
                    references.insert(FieldRef::new(self.owner.entity.clone(), field.clone()));
                }
                Expression::ColumnTotal(column) | Expression::ColumnAggregate { column, .. } => {
                    references.extend(column_cells(self.document, column));
                }
                Expression::If {
                    condition,
                    then_value,
                    else_value,
                } => {
                    pending.push(condition);
                    if let Ok(condition) = self.condition(condition) {
                        pending.push(if condition { then_value } else { else_value });
                    }
                }
                // ISBLANK observes presence, not the referenced calculated value.
                Expression::IsBlank(_) => {}
                _ => pending.extend(node.children().into_iter().rev()),
            }
        }
        references
    }

    pub(crate) fn number(&self, expression: &Expression) -> Result<Number, CalculationFailure> {
        match expression {
            Expression::Number(number) => Ok(*number),
            Expression::Reference(reference) => self.reference(reference),
            Expression::SameRowReference(field) => {
                self.reference(&FieldRef::new(self.owner.entity.clone(), field.clone()))
            }
            Expression::If {
                condition,
                then_value,
                else_value,
            } => self.number(if self.condition(condition)? {
                then_value
            } else {
                else_value
            }),
            Expression::ColumnTotal(column) => {
                // Blank cells are skipped. Every other row already passed the
                // target checks, so a value missing here is a failed
                // contributing row rather than an empty column.
                let mut total = 0.0;
                for reference in column_cells(self.document, column) {
                    let Some(value) = self.values.get(&reference) else {
                        return Err(CalculationFailure::FailedDependencies {
                            dependencies: BTreeSet::from([reference]),
                        });
                    };
                    total += value.get();
                }
                number_result(total)
            }
            Expression::ColumnAggregate { function, column } => {
                let mut values = Vec::new();
                for reference in column_cells(self.document, column) {
                    let Some(value) = self.values.get(&reference) else {
                        return Err(CalculationFailure::FailedDependencies {
                            dependencies: BTreeSet::from([reference]),
                        });
                    };
                    values.push(value.get());
                }
                let count = value_count(values.len());
                if values.is_empty() && *function != ColumnAggregateFunction::Count {
                    return Err(CalculationFailure::EmptyAggregate);
                }
                number_result(match function {
                    ColumnAggregateFunction::Average => average(&values),
                    ColumnAggregateFunction::Count => count,
                    ColumnAggregateFunction::Minimum => {
                        values.iter().copied().fold(f64::INFINITY, f64::min)
                    }
                    ColumnAggregateFunction::Maximum => {
                        values.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                    }
                })
            }
            Expression::Round { value, digits } => round(self.number(value)?, *digits),
            Expression::Add { left, right } => {
                self.binary(left, right, |left, right| number_result(left + right))
            }
            Expression::Subtract { left, right } => {
                self.binary(left, right, |left, right| number_result(left - right))
            }
            Expression::Multiply { left, right } => {
                self.binary(left, right, |left, right| number_result(left * right))
            }
            Expression::Divide { left, right } => self.binary(left, right, |left, right| {
                if right == 0.0 {
                    Err(CalculationFailure::DivisionByZero)
                } else {
                    number_result(left / right)
                }
            }),
            Expression::Minimum { left, right } => {
                self.binary(left, right, |left, right| number_result(left.min(right)))
            }
            Expression::Maximum { left, right } => {
                self.binary(left, right, |left, right| number_result(left.max(right)))
            }
            _ => Err(CalculationFailure::TypeMismatch),
        }
    }

    fn binary(
        &self,
        left: &Expression,
        right: &Expression,
        operation: impl FnOnce(f64, f64) -> Result<Number, CalculationFailure>,
    ) -> Result<Number, CalculationFailure> {
        let left = self.number(left)?;
        let right = self.number(right)?;
        operation(left.get(), right.get())
    }

    fn reference(&self, reference: &FieldRef) -> Result<Number, CalculationFailure> {
        if let Some(value) = self.values.get(reference) {
            return Ok(*value);
        }
        if self
            .document
            .entities
            .get(&reference.entity)
            .is_some_and(|entity| {
                !entity.fields.contains_key(&reference.field)
                    && self
                        .document
                        .schemas
                        .get(&entity.schema)
                        .and_then(|schema| schema.fields.get(&reference.field))
                        .is_some_and(|field| {
                            field.field_type == FieldType::Number && !field.required
                        })
            })
        {
            return Ok(Number::default());
        }
        Err(CalculationFailure::FailedDependencies {
            dependencies: BTreeSet::from([reference.clone()]),
        })
    }

    fn condition(&self, expression: &Expression) -> Result<bool, CalculationFailure> {
        match expression {
            Expression::Compare {
                operator,
                left,
                right,
            } => {
                let left = self.number(left)?;
                let right = self.number(right)?;
                Ok(match operator {
                    ComparisonOperator::Equal => left == right,
                    ComparisonOperator::NotEqual => left != right,
                    ComparisonOperator::Less => left < right,
                    ComparisonOperator::LessOrEqual => left <= right,
                    ComparisonOperator::Greater => left > right,
                    ComparisonOperator::GreaterOrEqual => left >= right,
                })
            }
            Expression::And(arguments) | Expression::Or(arguments) => {
                let and = matches!(expression, Expression::And(_));
                let mut result = and;
                for argument in arguments {
                    let value = self.condition(argument)?;
                    result = if and { result & value } else { result | value };
                }
                Ok(result)
            }
            Expression::Not(value) => self.condition(value).map(|value| !value),
            Expression::IsBlank(value) => {
                let reference = match value.as_ref() {
                    Expression::Reference(reference) => reference.clone(),
                    Expression::SameRowReference(field) => {
                        FieldRef::new(self.owner.entity.clone(), field.clone())
                    }
                    _ => return Err(CalculationFailure::TypeMismatch),
                };
                Ok(self
                    .document
                    .entities
                    .get(&reference.entity)
                    .is_some_and(|entity| !entity.fields.contains_key(&reference.field)))
            }
            _ => Err(CalculationFailure::TypeMismatch),
        }
    }
}

fn round(value: Number, digits: u8) -> Result<Number, CalculationFailure> {
    if digits > 10 {
        return Err(CalculationFailure::TypeMismatch);
    }
    let value = value.get();
    // Integral binary64 values already satisfy every supported precision.
    if value.fract() == 0.0 {
        return number_result(value);
    }
    number_result(round_decimal(value, usize::from(digits)))
}

/// Round half away from zero on the decimal the user sees, not on the binary64
/// product: `1.005 * 100.0` is `100.49999999999999`, which would round down,
/// while spreadsheets give `ROUND(1.005, 2) = 1.01`. Rust's `Display` prints
/// the shortest decimal that round-trips to the same binary64 value, so the
/// digit after the kept places decides, then the result is parsed back.
fn round_decimal(value: f64, digits: usize) -> f64 {
    let text = format!("{}", value.abs());
    let (integer, fraction) = text.split_once('.').unwrap_or((text.as_str(), ""));
    if fraction.len() <= digits {
        return value;
    }
    let mut kept = integer
        .bytes()
        .chain(fraction.bytes().take(digits))
        .collect::<Vec<_>>();
    if fraction.as_bytes()[digits] >= b'5' {
        let mut index = kept.len();
        loop {
            if index == 0 {
                kept.insert(0, b'1');
                break;
            }
            index -= 1;
            if kept[index] == b'9' {
                kept[index] = b'0';
            } else {
                kept[index] += 1;
                break;
            }
        }
    }
    let (whole, places) = kept.split_at(kept.len() - digits);
    let mut rounded = String::from_utf8_lossy(whole).into_owned();
    if !places.is_empty() {
        rounded.push('.');
        rounded.push_str(&String::from_utf8_lossy(places));
    }
    let magnitude = rounded
        .parse::<f64>()
        .expect("ASCII decimal digits always parse");
    if magnitude == 0.0 {
        0.0
    } else {
        magnitude.copysign(value)
    }
}

/// The compensated sum over the count, so near-cancelling values keep their
/// small remainder; a sum beyond f64 falls back to summing scaled values.
fn average(values: &[f64]) -> f64 {
    let count = value_count(values.len());
    let sum = compensated_sum(values.iter().copied());
    if sum.is_finite() {
        sum / count
    } else {
        compensated_sum(values.iter().map(|value| value / count))
    }
}

// Columns hold far fewer than 2^32 rows, so the count converts exactly.
fn value_count(len: usize) -> f64 {
    f64::from(u32::try_from(len).unwrap_or(u32::MAX))
}

// Neumaier's compensated summation.
fn compensated_sum(values: impl Iterator<Item = f64>) -> f64 {
    let (mut sum, mut compensation) = (0.0_f64, 0.0_f64);
    for value in values {
        let next = sum + value;
        compensation += if sum.abs() >= value.abs() {
            (sum - next) + value
        } else {
            (value - next) + sum
        };
        sum = next;
    }
    sum + compensation
}

fn number_result(value: f64) -> Result<Number, CalculationFailure> {
    Number::new(value).map_err(|_| CalculationFailure::NonFiniteResult)
}
