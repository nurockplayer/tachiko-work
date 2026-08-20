//! Deterministic formula evaluation for Tachiko Work.

use std::collections::{BTreeMap, BTreeSet};

use tachiko_semantic_core::{Document, Expression, FieldRef, Value};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq)]
pub struct Calculation {
    values: BTreeMap<FieldRef, f64>,
    dependencies: BTreeMap<FieldRef, BTreeSet<FieldRef>>,
}

impl Calculation {
    #[must_use]
    pub fn value(&self, field: &FieldRef) -> Option<f64> {
        self.values.get(field).copied()
    }

    #[must_use]
    pub fn values(&self) -> &BTreeMap<FieldRef, f64> {
        &self.values
    }

    #[must_use]
    pub fn dependencies(&self) -> &BTreeMap<FieldRef, BTreeSet<FieldRef>> {
        &self.dependencies
    }

    #[must_use]
    pub fn dependencies_of(&self, field: &FieldRef) -> Option<&BTreeSet<FieldRef>> {
        self.dependencies.get(field)
    }

    #[must_use]
    pub fn affected_by(&self, changed: &FieldRef) -> Vec<FieldRef> {
        let mut frontier = BTreeSet::from([changed.clone()]);
        let mut affected = BTreeSet::new();

        loop {
            let newly_affected: BTreeSet<_> = self
                .dependencies
                .iter()
                .filter(|(formula, dependencies)| {
                    !affected.contains(*formula) && !dependencies.is_disjoint(&frontier)
                })
                .map(|(formula, _)| formula.clone())
                .collect();
            if newly_affected.is_empty() {
                break;
            }
            frontier.clone_from(&newly_affected);
            affected.extend(newly_affected);
        }

        affected.into_iter().collect()
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum CalculationError {
    #[error("formula reference '{reference}' does not exist")]
    MissingReference { reference: FieldRef },
    #[error("formula reference '{reference}' is not numeric")]
    NonNumericReference { reference: FieldRef },
    #[error("formula '{formula}' divided by zero")]
    DivisionByZero { formula: FieldRef },
    #[error("formula dependency cycle: {path:?}")]
    Cycle { path: Vec<FieldRef> },
    #[error("calculation for '{field}' produced a non-finite result")]
    NonFiniteResult { field: FieldRef },
}

/// Calculate every numeric literal and formula in deterministic field order.
///
/// # Errors
///
/// Returns a [`CalculationError`] for missing or non-numeric references,
/// dependency cycles, division by zero, or non-finite results.
pub fn calculate(document: &Document) -> Result<Calculation, CalculationError> {
    let mut evaluator = Evaluator::new(document);

    for (entity_id, entity) in &document.entities {
        for (field_id, value) in &entity.fields {
            if matches!(value, Value::Number(_) | Value::Formula(_)) {
                evaluator.value_for(&FieldRef {
                    entity: entity_id.clone(),
                    field: field_id.clone(),
                })?;
            }
        }
    }

    Ok(Calculation {
        values: evaluator.values,
        dependencies: evaluator.dependencies,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VisitState {
    Visiting,
    Complete,
}

struct Evaluator<'document> {
    document: &'document Document,
    values: BTreeMap<FieldRef, f64>,
    dependencies: BTreeMap<FieldRef, BTreeSet<FieldRef>>,
    states: BTreeMap<FieldRef, VisitState>,
    stack: Vec<FieldRef>,
}

impl<'document> Evaluator<'document> {
    fn new(document: &'document Document) -> Self {
        Self {
            document,
            values: BTreeMap::new(),
            dependencies: BTreeMap::new(),
            states: BTreeMap::new(),
            stack: Vec::new(),
        }
    }

    fn value_for(&mut self, field: &FieldRef) -> Result<f64, CalculationError> {
        match self.states.get(field) {
            Some(VisitState::Complete) => {
                return Ok(self.values[field]);
            }
            Some(VisitState::Visiting) => {
                let cycle_start = self
                    .stack
                    .iter()
                    .position(|candidate| candidate == field)
                    .unwrap_or(0);
                let mut path = self.stack[cycle_start..].to_vec();
                path.push(field.clone());
                return Err(CalculationError::Cycle { path });
            }
            None => {}
        }

        let value = self.lookup_value(field)?.clone();
        self.states.insert(field.clone(), VisitState::Visiting);
        self.stack.push(field.clone());

        let result = match value {
            Value::Number(number) => Self::ensure_finite(field, number)?,
            Value::Formula(expression) => {
                self.dependencies.entry(field.clone()).or_default();
                let result = self.evaluate_expression(field, &expression)?;
                Self::ensure_finite(field, result)?
            }
            Value::Text(_) | Value::Boolean(_) | Value::Reference(_) => {
                return Err(CalculationError::NonNumericReference {
                    reference: field.clone(),
                });
            }
        };

        self.stack.pop();
        self.states.insert(field.clone(), VisitState::Complete);
        self.values.insert(field.clone(), result);
        Ok(result)
    }

    fn lookup_value(&self, field: &FieldRef) -> Result<&Value, CalculationError> {
        self.document
            .entities
            .get(&field.entity)
            .and_then(|entity| entity.fields.get(&field.field))
            .ok_or_else(|| CalculationError::MissingReference {
                reference: field.clone(),
            })
    }

    fn evaluate_expression(
        &mut self,
        formula: &FieldRef,
        expression: &Expression,
    ) -> Result<f64, CalculationError> {
        match expression {
            Expression::Number(number) => Self::ensure_finite(formula, *number),
            Expression::Reference(reference) => {
                self.dependencies
                    .entry(formula.clone())
                    .or_default()
                    .insert(reference.clone());
                self.value_for(reference)
            }
            Expression::Add { left, right } => {
                let left = self.evaluate_expression(formula, left)?;
                let right = self.evaluate_expression(formula, right)?;
                Self::ensure_finite(formula, left + right)
            }
            Expression::Subtract { left, right } => {
                let left = self.evaluate_expression(formula, left)?;
                let right = self.evaluate_expression(formula, right)?;
                Self::ensure_finite(formula, left - right)
            }
            Expression::Multiply { left, right } => {
                let left = self.evaluate_expression(formula, left)?;
                let right = self.evaluate_expression(formula, right)?;
                Self::ensure_finite(formula, left * right)
            }
            Expression::Divide { left, right } => {
                let left = self.evaluate_expression(formula, left)?;
                let right = self.evaluate_expression(formula, right)?;
                if right == 0.0 {
                    return Err(CalculationError::DivisionByZero {
                        formula: formula.clone(),
                    });
                }
                Self::ensure_finite(formula, left / right)
            }
            Expression::Minimum { left, right } => {
                let left = self.evaluate_expression(formula, left)?;
                let right = self.evaluate_expression(formula, right)?;
                Self::ensure_finite(formula, left.min(right))
            }
            Expression::Maximum { left, right } => {
                let left = self.evaluate_expression(formula, left)?;
                let right = self.evaluate_expression(formula, right)?;
                Self::ensure_finite(formula, left.max(right))
            }
        }
    }

    fn ensure_finite(field: &FieldRef, value: f64) -> Result<f64, CalculationError> {
        if value.is_finite() {
            Ok(value)
        } else {
            Err(CalculationError::NonFiniteResult {
                field: field.clone(),
            })
        }
    }
}
