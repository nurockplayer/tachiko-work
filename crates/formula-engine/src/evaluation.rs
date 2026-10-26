use std::collections::{BTreeMap, BTreeSet};

use tachiko_semantic_core::{
    ComparisonOperator, Document, Expression, FieldRef, FieldType, Number,
};

use crate::CalculationFailure;

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
    // Returning them directly also avoids scaling overflow and large-value drift.
    if value.fract() == 0.0 {
        return number_result(value);
    }
    let scale = 10_f64.powi(i32::from(digits));
    let exponent_bits = (value.abs().to_bits() >> 52) & 0x7ff;
    let exponent = i32::try_from(exponent_bits).expect("binary64 exponent fits i32") - 1023;
    // Rounding by less than one binary64 spacing cannot change the Number.
    // Avoid a multiply/divide round trip that could perturb such a value.
    if 2_f64.powi(exponent - 52) >= 1.0 / scale {
        return number_result(value);
    }
    // Scaling is one binary64 operation, followed by half-away-from-zero rounding.
    // This deliberately makes ROUND(2.675, 2) = 2.68 without decimal formatting.
    number_result((value * scale).round() / scale)
}

fn number_result(value: f64) -> Result<Number, CalculationFailure> {
    Number::new(value).map_err(|_| CalculationFailure::NonFiniteResult)
}
