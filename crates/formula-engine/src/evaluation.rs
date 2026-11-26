use std::borrow::Cow;
mod lookup;

use std::collections::{BTreeMap, BTreeSet};

use tachiko_semantic_core::{
    ColumnAggregateFunction, ColumnCriterion, ColumnRef, ComparisonOperator, ConditionalAggregate,
    CriterionCell, CriterionKey, CriterionTest, CriterionTestKey, Date, DatePartKind, Document,
    Entity, EntityId, Expression, FieldRef, FieldType, Number, Value,
};

use crate::{CalculatedValue, CalculationFailure, FailureMap, ValueStore, column_cells};

#[cfg(test)]
use conditional_walks::{bypass_caches, record_row, record_walk};

/// In a normal build the walk counters and the forced-walk switch do not exist.
#[cfg(not(test))]
const fn bypass_caches() -> bool {
    false
}
#[cfg(not(test))]
const fn record_row() {}
#[cfg(not(test))]
const fn record_walk(_candidates: bool) {}

/// One pass's read of one column, in entity order.
struct ColumnValues {
    /// The column's first cell that already carries a failure, in entity order.
    first_failed: Option<FieldRef>,
    /// The first cell with no entry in the pass's value map.
    first_missing: Option<FieldRef>,
    /// The present values, in entity order.
    present: Vec<f64>,
    /// The plain running sum of `present` in that same order.
    total: f64,
}

/// One conditional criterion column's index for this pass.
struct CriterionIndex {
    /// The entities filed under each cell key, in entity order.
    entries: BTreeMap<CriterionKey, Vec<EntityId>>,
    /// A cell whose match cannot be decided from its key: the full walk.
    scan: bool,
}

/// The memo identity of one conditional aggregate. The rows a conditional
/// matches depend on the aggregate and on each criterion's column and resolved
/// test, never on the formula that computes it.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ConditionalKey {
    aggregate: AggregateKey,
    criteria: Vec<(ColumnRef, CriterionTestKey)>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum AggregateKey {
    Count,
    Sum(ColumnRef),
    Average(ColumnRef),
}

impl AggregateKey {
    fn new(aggregate: &ConditionalAggregate) -> Self {
        match aggregate {
            ConditionalAggregate::Count => Self::Count,
            ConditionalAggregate::Sum(column) => Self::Sum(column.clone()),
            ConditionalAggregate::Average(column) => Self::Average(column.clone()),
        }
    }
}

/// One calculation pass's read of each column, filled on first use.
///
/// A column's formula members are its static dependencies, so all of them have
/// resolved before the first reader of the column is ready, and nothing writes
/// to them later in the pass: an entry cannot go stale. A retained update reads
/// the same invariant from the other side, where members outside the dirty set
/// are already resolved.
#[derive(Default)]
pub(crate) struct ColumnCache {
    entries: BTreeMap<ColumnRef, ColumnValues>,
    indexes: BTreeMap<ColumnRef, CriterionIndex>,
    conditionals: BTreeMap<ConditionalKey, Result<Number, CalculationFailure>>,
    lookup_indexes: BTreeMap<ColumnRef, BTreeMap<CriterionKey, EntityId>>,
}

impl ColumnCache {
    /// One criterion column's key index, built at most once per pass, in entity
    /// order and under the same schema filter the row walk applies.
    fn criterion_index(
        &mut self,
        document: &Document,
        values: &impl ValueStore,
        column: &ColumnRef,
    ) {
        if self.indexes.contains_key(column) {
            return;
        }
        let mut index = CriterionIndex {
            entries: BTreeMap::new(),
            scan: false,
        };
        for (entity_id, entity) in &document.entities {
            if entity.schema != column.schema {
                continue;
            }
            let Some(value) = entity.fields.get(&column.field) else {
                continue;
            };
            let cell = match value {
                Value::Number(_) | Value::Formula(_) => {
                    let reference = FieldRef::new(entity_id.clone(), column.field.clone());
                    let Some(value) = values.number_value(&reference) else {
                        // The walk fails at this row whatever its key says.
                        index.scan = true;
                        continue;
                    };
                    CriterionCell::Number(value.get())
                }
                Value::Text(text) => CriterionCell::Text(text),
                Value::Boolean(boolean) => CriterionCell::Boolean(*boolean),
                Value::Date(date) => CriterionCell::Date(*date),
                Value::Reference(_) => {
                    index.scan = true;
                    continue;
                }
            };
            for key in cell.keys() {
                index
                    .entries
                    .entry(key)
                    .or_default()
                    .push(entity_id.clone());
            }
        }
        self.indexes.insert(column.clone(), index);
    }

    fn lookup_index(&mut self, document: &Document, values: &impl ValueStore, column: &ColumnRef) {
        if self.lookup_indexes.contains_key(column) {
            return;
        }
        let auto =
            document.schemas[&column.schema].fields[&column.field].field_type == FieldType::Auto;
        let mut index = BTreeMap::new();
        for (entity_id, entity) in &document.entities {
            if entity.schema != column.schema {
                continue;
            }
            let calculated;
            let cell = match entity.fields.get(&column.field) {
                Some(Value::Formula(_)) if auto => {
                    let reference = FieldRef::new(entity_id.clone(), column.field.clone());
                    let Some(value) = values.calculated_value(&reference) else {
                        continue;
                    };
                    calculated = value;
                    match &calculated {
                        CalculatedValue::Number(number) => CriterionCell::Number(number.get()),
                        CalculatedValue::Text(text) => CriterionCell::Text(text),
                        CalculatedValue::Boolean(value) => CriterionCell::Boolean(*value),
                        CalculatedValue::Date(value) => CriterionCell::Date(*value),
                    }
                }
                Some(Value::Number(_) | Value::Formula(_)) => {
                    let reference = FieldRef::new(entity_id.clone(), column.field.clone());
                    let Some(value) = values.number_value(&reference) else {
                        continue;
                    };
                    CriterionCell::Number(value.get())
                }
                Some(Value::Text(text)) => CriterionCell::Text(text),
                Some(Value::Boolean(value)) => CriterionCell::Boolean(*value),
                Some(Value::Date(value)) => CriterionCell::Date(*value),
                None | Some(Value::Reference(_)) => continue,
            };
            index
                .entry(exact_key(cell))
                .or_insert_with(|| entity_id.clone());
        }
        self.lookup_indexes.insert(column.clone(), index);
        #[cfg(test)]
        LOOKUP_INDEX_BUILDS.with(|builds| builds.set(builds.get() + 1));
    }

    pub(crate) fn fill(
        &mut self,
        document: &Document,
        values: &impl ValueStore,
        failures: &FailureMap,
        column: &ColumnRef,
    ) {
        if self.entries.contains_key(column) {
            return;
        }
        let mut first_failed = None;
        let mut first_missing = None;
        let mut present = Vec::new();
        let mut total = 0.0;
        for reference in column_cells(document, column) {
            if first_failed.is_none() && failures.contains_key(&reference) {
                first_failed = Some(reference.clone());
            }
            match values.number_value(&reference) {
                Some(value) => {
                    present.push(value.get());
                    total += value.get();
                }
                None if first_missing.is_none() => first_missing = Some(reference),
                None => {}
            }
        }
        self.entries.insert(
            column.clone(),
            ColumnValues {
                first_failed,
                first_missing,
                present,
                total,
            },
        );
    }

    fn entry(&self, column: &ColumnRef) -> &ColumnValues {
        self.entries
            .get(column)
            .expect("every column a formula names is filled before evaluation")
    }
}

pub(crate) struct Evaluation<'a, V: ValueStore> {
    pub(crate) document: &'a Document,
    pub(crate) owner: &'a FieldRef,
    pub(crate) values: &'a V,
    // Conditional aggregates memoise their result and index their criteria
    // columns as the pass goes, so the cache is this evaluation's alone.
    pub(crate) columns: &'a mut ColumnCache,
}

impl<V: ValueStore> Evaluation<'_, V> {
    pub(crate) fn calculated(
        &mut self,
        expression: &Expression,
    ) -> Result<CalculatedValue, CalculationFailure> {
        match expression {
            Expression::Reference(reference) | Expression::AnchoredReference(reference) => {
                self.calculated_reference(reference)
            }
            Expression::SameRowReference(field) | Expression::AnchoredSameRowReference(field) => {
                self.calculated_reference(&FieldRef::new(self.owner.entity.clone(), field.clone()))
            }
            Expression::If {
                condition,
                then_value,
                else_value,
            } => {
                let condition = self.condition(condition)?;
                self.calculated(if condition { then_value } else { else_value })
            }
            Expression::IfError { value, fallback } => match self.calculated(value) {
                Err(
                    CalculationFailure::DivisionByZero
                    | CalculationFailure::TypeMismatch
                    | CalculationFailure::NonFiniteResult
                    | CalculationFailure::EmptyAggregate
                    | CalculationFailure::LookupMissingKey
                    | CalculationFailure::FailedDependencies { .. },
                ) => self.calculated(fallback),
                result => result,
            },
            Expression::Compare { .. }
            | Expression::And(_)
            | Expression::Or(_)
            | Expression::Not(_)
            | Expression::IsBlank(_) => self.condition(expression).map(CalculatedValue::Boolean),
            Expression::Add { left, right } | Expression::Subtract { left, right } => {
                let left = arithmetic_operand(self.calculated(left)?)?;
                let right = arithmetic_operand(self.calculated(right)?)?;
                let subtract = matches!(expression, Expression::Subtract { .. });
                match (left, right) {
                    (CalculatedValue::Number(a), CalculatedValue::Number(b)) => {
                        number_result(if subtract {
                            a.get() - b.get()
                        } else {
                            a.get() + b.get()
                        })
                        .map(CalculatedValue::Number)
                    }
                    (CalculatedValue::Date(a), CalculatedValue::Date(b)) if subtract => {
                        number_result(f64::from(days_from_civil(a) - days_from_civil(b)))
                            .map(CalculatedValue::Number)
                    }
                    (CalculatedValue::Date(date), CalculatedValue::Number(days)) => {
                        shift_date(date, if subtract { -days.get() } else { days.get() })
                            .map(CalculatedValue::Date)
                    }
                    (CalculatedValue::Number(days), CalculatedValue::Date(date)) if !subtract => {
                        shift_date(date, days.get()).map(CalculatedValue::Date)
                    }
                    _ => Err(CalculationFailure::TypeMismatch),
                }
            }
            _ => self
                .number_expression(expression)
                .map(CalculatedValue::Number),
        }
    }

    fn calculated_reference(
        &self,
        reference: &FieldRef,
    ) -> Result<CalculatedValue, CalculationFailure> {
        if let Some(value) = self.values.calculated_value(reference) {
            Ok(value)
        } else if super::field_type(self.document, reference) == Some(&FieldType::Auto) {
            match self.document.entities[&reference.entity]
                .fields
                .get(&reference.field)
            {
                Some(Value::Text(text)) => Ok(CalculatedValue::Text(text.clone())),
                Some(Value::Boolean(value)) => Ok(CalculatedValue::Boolean(*value)),
                None => Ok(CalculatedValue::Number(Number::default())),
                Some(
                    Value::Formula(_) | Value::Number(_) | Value::Date(_) | Value::Reference(_),
                ) => Err(CalculationFailure::FailedDependencies {
                    dependencies: BTreeSet::from([reference.clone()]),
                }),
            }
        } else {
            self.reference(reference).map(CalculatedValue::Number)
        }
    }
    // Static dependencies still govern scheduling, cycles, and invalidation.
    // Failure propagation follows only the branches that are actually read;
    // a column read names its first failed cell in entity order. The
    // differential oracle keeps this expanded per-formula form.
    #[cfg(test)]
    pub(crate) fn required_references(
        &mut self,
        expression: &Expression,
        failures: &FailureMap,
    ) -> BTreeSet<FieldRef> {
        let mut references = BTreeSet::new();
        let mut pending = vec![expression];
        while let Some(node) = pending.pop() {
            match node {
                Expression::Reference(reference) | Expression::AnchoredReference(reference) => {
                    references.insert(reference.clone());
                }
                Expression::SameRowReference(field)
                | Expression::AnchoredSameRowReference(field) => {
                    references.insert(FieldRef::new(self.owner.entity.clone(), field.clone()));
                }
                Expression::ColumnTotal(column) | Expression::ColumnAggregate { column, .. } => {
                    references.extend(first_failed_cell(self.document, column, failures));
                }
                // A conditional aggregate reads every cell of every column it
                // names, matched row or not, so any failed contributor fails
                // the formula. That is stricter than Excel and visible.
                Expression::ColumnConditional {
                    aggregate,
                    criteria,
                } => {
                    if let ConditionalAggregate::Sum(column)
                    | ConditionalAggregate::Average(column) = aggregate.as_ref()
                    {
                        references.extend(first_failed_cell(self.document, column, failures));
                    }
                    for criterion in criteria {
                        references.extend(first_failed_cell(
                            self.document,
                            &criterion.column,
                            failures,
                        ));
                        if let Some(expression) = &criterion.value {
                            match expression.as_ref() {
                                Expression::Reference(reference)
                                | Expression::AnchoredReference(reference) => {
                                    references.insert(reference.clone());
                                }
                                Expression::SameRowReference(field)
                                | Expression::AnchoredSameRowReference(field) => {
                                    references.insert(FieldRef::new(
                                        self.owner.entity.clone(),
                                        field.clone(),
                                    ));
                                }
                                other => pending.push(other),
                            }
                        }
                    }
                }
                Expression::Lookup {
                    key,
                    key_column,
                    result_column,
                    if_missing,
                } => {
                    pending.push(key);
                    for column in [key_column, result_column] {
                        references.extend(first_failed_cell(self.document, column, failures));
                    }
                    if let Some(value) = if_missing {
                        if matches!(self.lookup_row(key, key_column), Ok(None)) {
                            pending.push(value);
                        }
                    }
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
                Expression::IfError { value, fallback } => {
                    pending.push(value);
                    if matches!(
                        self.calculated(value),
                        Err(CalculationFailure::DivisionByZero
                            | CalculationFailure::TypeMismatch
                            | CalculationFailure::NonFiniteResult
                            | CalculationFailure::EmptyAggregate
                            | CalculationFailure::LookupMissingKey
                            | CalculationFailure::FailedDependencies { .. })
                    ) {
                        pending.push(fallback);
                    }
                }
                // ISBLANK observes presence, not the referenced calculated value.
                Expression::IsBlank(_) => {}
                _ => pending.extend(node.children().into_iter().rev()),
            }
        }
        references
    }

    /// The failed references the expression actually reads: the same branch
    /// rules as the expanded reference walk, but its columns answered from the
    /// pass cache instead of expanded per cell.
    pub(crate) fn failed_required_references(
        &mut self,
        expression: &Expression,
        failures: &FailureMap,
        cell_failures: &super::CellFailures,
    ) -> BTreeSet<FieldRef> {
        let is_failed = |reference: &FieldRef| {
            failures.contains_key(reference) || cell_failures.contains_key(reference)
        };
        let mut references = BTreeSet::new();
        let mut pending = vec![expression];
        while let Some(node) = pending.pop() {
            match node {
                Expression::Reference(reference) | Expression::AnchoredReference(reference) => {
                    if is_failed(reference) {
                        references.insert(reference.clone());
                    }
                }
                Expression::SameRowReference(field)
                | Expression::AnchoredSameRowReference(field) => {
                    let reference = FieldRef::new(self.owner.entity.clone(), field.clone());
                    if is_failed(&reference) {
                        references.insert(reference);
                    }
                }
                Expression::ColumnTotal(column) | Expression::ColumnAggregate { column, .. } => {
                    references.extend(self.columns.entry(column).first_failed.iter().cloned());
                }
                // A conditional aggregate reads every cell of every column it
                // names, matched row or not, so any failed contributor fails
                // the formula. That is stricter than Excel and visible.
                Expression::ColumnConditional {
                    aggregate,
                    criteria,
                } => {
                    if let ConditionalAggregate::Sum(column)
                    | ConditionalAggregate::Average(column) = aggregate.as_ref()
                    {
                        references.extend(self.columns.entry(column).first_failed.iter().cloned());
                    }
                    for criterion in criteria {
                        references.extend(
                            self.columns
                                .entry(&criterion.column)
                                .first_failed
                                .iter()
                                .cloned(),
                        );
                        pending.extend(criterion.value.as_deref());
                    }
                }
                Expression::Lookup {
                    key,
                    key_column,
                    result_column,
                    if_missing,
                } => {
                    pending.push(key);
                    for column in [key_column, result_column] {
                        references.extend(self.columns.entry(column).first_failed.iter().cloned());
                    }
                    if let Some(value) = if_missing {
                        if matches!(self.lookup_row(key, key_column), Ok(None)) {
                            pending.push(value);
                        }
                    }
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
                Expression::IfError { value, fallback } => {
                    pending.push(value);
                    if matches!(
                        self.calculated(value),
                        Err(CalculationFailure::DivisionByZero
                            | CalculationFailure::TypeMismatch
                            | CalculationFailure::NonFiniteResult
                            | CalculationFailure::EmptyAggregate
                            | CalculationFailure::LookupMissingKey
                            | CalculationFailure::FailedDependencies { .. })
                    ) {
                        pending.push(fallback);
                    }
                }
                // ISBLANK observes presence, not the referenced calculated value.
                Expression::IsBlank(_) => {}
                _ => pending.extend(node.children().into_iter().rev()),
            }
        }
        references
    }

    pub(crate) fn number(&mut self, expression: &Expression) -> Result<Number, CalculationFailure> {
        match arithmetic_operand(self.calculated(expression)?)? {
            CalculatedValue::Number(number) => Ok(number),
            CalculatedValue::Date(_) | CalculatedValue::Text(_) | CalculatedValue::Boolean(_) => {
                Err(CalculationFailure::TypeMismatch)
            }
        }
    }

    #[allow(clippy::too_many_lines)] // Closed expression dispatch.
    fn number_expression(&mut self, expression: &Expression) -> Result<Number, CalculationFailure> {
        match expression {
            Expression::Number(number) => Ok(*number),
            Expression::Reference(reference) | Expression::AnchoredReference(reference) => {
                self.reference(reference)
            }
            Expression::SameRowReference(field) | Expression::AnchoredSameRowReference(field) => {
                self.reference(&FieldRef::new(self.owner.entity.clone(), field.clone()))
            }
            Expression::If {
                condition,
                then_value,
                else_value,
            } => {
                let condition = self.condition(condition)?;
                self.number(if condition { then_value } else { else_value })
            }
            Expression::IfError { value, fallback } => match self.number(value) {
                Err(
                    CalculationFailure::DivisionByZero
                    | CalculationFailure::TypeMismatch
                    | CalculationFailure::NonFiniteResult
                    | CalculationFailure::EmptyAggregate
                    | CalculationFailure::LookupMissingKey
                    | CalculationFailure::FailedDependencies { .. },
                ) => self.number(fallback),
                result => result,
            },
            Expression::ColumnTotal(column) => {
                // Blank cells are skipped, and the pass cache added the values
                // in entity order, so the total is bit-identical to a walk.
                let values = self.columns.entry(column);
                if let Some(reference) = &values.first_missing {
                    return Err(CalculationFailure::FailedDependencies {
                        dependencies: BTreeSet::from([reference.clone()]),
                    });
                }
                number_result(values.total)
            }
            Expression::ColumnAggregate { function, column } => {
                let values = self.columns.entry(column);
                if let Some(reference) = &values.first_missing {
                    return Err(CalculationFailure::FailedDependencies {
                        dependencies: BTreeSet::from([reference.clone()]),
                    });
                }
                let count = value_count(values.present.len());
                if values.present.is_empty() && *function != ColumnAggregateFunction::Count {
                    return Err(CalculationFailure::EmptyAggregate);
                }
                number_result(match function {
                    ColumnAggregateFunction::Average => average(&values.present),
                    ColumnAggregateFunction::Count => count,
                    ColumnAggregateFunction::Minimum => {
                        values.present.iter().copied().fold(f64::INFINITY, f64::min)
                    }
                    ColumnAggregateFunction::Maximum => values
                        .present
                        .iter()
                        .copied()
                        .fold(f64::NEG_INFINITY, f64::max),
                })
            }
            Expression::ColumnConditional {
                aggregate,
                criteria,
            } => self.conditional(aggregate, criteria),
            Expression::DatePart { part, operand } => {
                let date = self.date(operand)?;
                number_result(f64::from(match part {
                    DatePartKind::Year => date.year(),
                    DatePartKind::Month => u16::from(date.month()),
                    DatePartKind::Day => u16::from(date.day()),
                }))
            }
            Expression::Weekday {
                operand,
                return_type,
            } => {
                let sunday = (days_from_civil(self.date(operand)?) + 4).rem_euclid(7);
                let result = match return_type {
                    1 => sunday + 1,
                    2 => (sunday + 6) % 7 + 1,
                    3 => (sunday + 6) % 7,
                    _ => return Err(CalculationFailure::TypeMismatch),
                };
                number_result(f64::from(result))
            }
            Expression::Days { end, start } => number_result(f64::from(
                days_from_civil(self.date(end)?) - days_from_civil(self.date(start)?),
            )),
            Expression::Lookup {
                key,
                key_column,
                result_column,
                if_missing,
            } => {
                if let Some(entity) = self.lookup_row(key, key_column)? {
                    self.lookup_result_number(&FieldRef::new(entity, result_column.field.clone()))
                } else if let Some(value) = if_missing {
                    self.number(value)
                } else {
                    Err(CalculationFailure::LookupMissingKey)
                }
            }
            Expression::Round { value, digits } => round(self.number(value)?, *digits),
            Expression::Abs(value) => number_result(self.number(value)?.get().abs()),
            Expression::RoundUp { value, digits } => {
                directed_round(self.number(value)?, *digits, true)
            }
            Expression::RoundDown { value, digits } => {
                directed_round(self.number(value)?, *digits, false)
            }
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

    fn conditional(
        &mut self,
        aggregate: &ConditionalAggregate,
        criteria: &[ColumnCriterion],
    ) -> Result<Number, CalculationFailure> {
        if criteria.is_empty() {
            return Err(CalculationFailure::TypeMismatch);
        }
        let mut tests = Vec::with_capacity(criteria.len());
        for criterion in criteria {
            tests.push(self.criterion_test(criterion)?);
        }
        let bypass = bypass_caches();
        let memo_key = (!bypass).then(|| ConditionalKey {
            aggregate: AggregateKey::new(aggregate),
            criteria: criteria
                .iter()
                .zip(&tests)
                .map(|(criterion, test)| (criterion.column.clone(), test.key()))
                .collect(),
        });
        if let Some(key) = &memo_key {
            if let Some(result) = self.columns.conditionals.get(key) {
                return result.clone();
            }
        }
        let keys = if bypass { None } else { tests[0].keys() };
        if keys.is_some() {
            self.columns
                .criterion_index(self.document, self.values, &criteria[0].column);
        }
        let candidates = keys
            .as_deref()
            .and_then(|keys| self.candidates(&criteria[0].column, keys));
        let result = self.conditional_over(aggregate, criteria, &tests, candidates.as_deref());
        if let Some(key) = memo_key {
            self.columns.conditionals.insert(key, result.clone());
        }
        result
    }

    /// The entities the first criterion cannot reject, in entity order: `None`
    /// when its column holds a cell that may fail.
    ///
    /// A row the first criterion rejects without an error is dropped before
    /// any later criterion reads it, so a superset of the rows it matches, in
    /// entity order, gives the same matched rows, errors and sum order.
    fn candidates(&self, column: &ColumnRef, keys: &[CriterionKey]) -> Option<Cow<'_, [EntityId]>> {
        let index = &self.columns.indexes[column];
        if index.scan {
            return None;
        }
        let lists = keys
            .iter()
            .filter_map(|key| index.entries.get(key))
            .collect::<Vec<_>>();
        Some(match lists.as_slice() {
            [] => Cow::Borrowed(&[]),
            [only] => Cow::Borrowed(only.as_slice()),
            [left, right] => Cow::Owned(merge_ids(left, right)),
            _ => unreachable!("a test has at most two keys"),
        })
    }

    /// The rows one conditional reads, in entity order: the candidates when
    /// the first criterion names them, every row of the schema otherwise.
    fn conditional_over(
        &self,
        aggregate: &ConditionalAggregate,
        criteria: &[ColumnCriterion],
        tests: &[CriterionTest],
        candidates: Option<&[EntityId]>,
    ) -> Result<Number, CalculationFailure> {
        record_walk(candidates.is_some());
        let document = self.document;
        let mut values = Vec::new();
        let mut matched = 0_usize;
        if let Some(candidates) = candidates {
            for entity in candidates.iter().map(|id| &document.entities[id]) {
                if self.conditional_row(aggregate, criteria, tests, entity, &mut values)? {
                    matched += 1;
                }
            }
        } else {
            let schema = &criteria[0].column.schema;
            for entity in document
                .entities
                .values()
                .filter(|entity| entity.schema == *schema)
            {
                if self.conditional_row(aggregate, criteria, tests, entity, &mut values)? {
                    matched += 1;
                }
            }
        }
        number_result(match aggregate {
            ConditionalAggregate::Count => value_count(matched),
            ConditionalAggregate::Sum(_) => values.iter().copied().sum(),
            ConditionalAggregate::Average(_) => {
                if values.is_empty() {
                    return Err(CalculationFailure::EmptyAggregate);
                }
                average(&values)
            }
        })
    }

    /// One row of a conditional: whether every criterion reads it as a match,
    /// adding its aggregate value when they all do.
    fn conditional_row(
        &self,
        aggregate: &ConditionalAggregate,
        criteria: &[ColumnCriterion],
        tests: &[CriterionTest],
        entity: &Entity,
        values: &mut Vec<f64>,
    ) -> Result<bool, CalculationFailure> {
        record_row();
        for (criterion, test) in criteria.iter().zip(tests) {
            let cell = self.criterion_cell(entity, &criterion.column.field)?;
            if !test.matches(cell) {
                return Ok(false);
            }
        }
        if let ConditionalAggregate::Sum(column) | ConditionalAggregate::Average(column) = aggregate
        {
            if entity.fields.contains_key(&column.field) {
                let reference = FieldRef::new(entity.id.clone(), column.field.clone());
                let Some(value) = self.values.number_value(&reference) else {
                    return Err(CalculationFailure::FailedDependencies {
                        dependencies: BTreeSet::from([reference]),
                    });
                };
                values.push(value.get());
            }
        }
        Ok(true)
    }

    fn criterion_test(
        &mut self,
        criterion: &ColumnCriterion,
    ) -> Result<CriterionTest, CalculationFailure> {
        match (criterion.operator, criterion.value.as_deref()) {
            (ComparisonOperator::Equal, None) => {
                Ok(CriterionTest::blank(ComparisonOperator::Equal))
            }
            (ComparisonOperator::NotEqual, None) => {
                Ok(CriterionTest::blank(ComparisonOperator::NotEqual))
            }
            (
                ComparisonOperator::Equal,
                Some(Expression::Reference(reference) | Expression::AnchoredReference(reference)),
            ) => self.reference_criterion_test(reference),
            (
                ComparisonOperator::Equal,
                Some(
                    Expression::SameRowReference(field)
                    | Expression::AnchoredSameRowReference(field),
                ),
            ) => self
                .reference_criterion_test(&FieldRef::new(self.owner.entity.clone(), field.clone())),
            (_, Some(Expression::Number(number))) => {
                Ok(CriterionTest::number(criterion.operator, number.get()))
            }
            (ComparisonOperator::Equal, Some(other)) => {
                Ok(CriterionTest::equal_number(self.number(other)?.get()))
            }
            _ => Err(CalculationFailure::TypeMismatch),
        }
    }

    fn reference_criterion_test(
        &self,
        reference: &FieldRef,
    ) -> Result<CriterionTest, CalculationFailure> {
        let cell = self
            .document
            .entities
            .get(&reference.entity)
            .and_then(|entity| entity.fields.get(&reference.field));
        match cell {
            // Excel compares a blank referenced cell as zero.
            None => Ok(CriterionTest::equal_number(0.0)),
            Some(Value::Number(_) | Value::Formula(_)) => {
                let Some(value) = self.values.number_value(reference) else {
                    return Err(CalculationFailure::FailedDependencies {
                        dependencies: BTreeSet::from([reference.clone()]),
                    });
                };
                Ok(CriterionTest::equal_number(value.get()))
            }
            Some(Value::Text(text)) => {
                CriterionTest::parse(text).map_err(|_| CalculationFailure::TypeMismatch)
            }
            Some(Value::Boolean(boolean)) => Ok(CriterionTest::equal_boolean(*boolean)),
            Some(Value::Date(date)) => Ok(CriterionTest::equal_date(*date)),
            Some(Value::Reference(_)) => Err(CalculationFailure::TypeMismatch),
        }
    }

    fn criterion_cell<'a>(
        &self,
        entity: &'a tachiko_semantic_core::Entity,
        field: &tachiko_semantic_core::FieldId,
    ) -> Result<Option<CriterionCell<'a>>, CalculationFailure> {
        match entity.fields.get(field) {
            None => Ok(None),
            Some(Value::Number(_) | Value::Formula(_)) => {
                let reference = FieldRef::new(entity.id.clone(), field.clone());
                let Some(value) = self.values.number_value(&reference) else {
                    return Err(CalculationFailure::FailedDependencies {
                        dependencies: BTreeSet::from([reference]),
                    });
                };
                Ok(Some(CriterionCell::Number(value.get())))
            }
            Some(Value::Text(text)) => Ok(Some(CriterionCell::Text(text))),
            Some(Value::Boolean(boolean)) => Ok(Some(CriterionCell::Boolean(*boolean))),
            Some(Value::Date(date)) => Ok(Some(CriterionCell::Date(*date))),
            Some(Value::Reference(_)) => Err(CalculationFailure::TypeMismatch),
        }
    }

    fn binary(
        &mut self,
        left: &Expression,
        right: &Expression,
        operation: impl FnOnce(f64, f64) -> Result<Number, CalculationFailure>,
    ) -> Result<Number, CalculationFailure> {
        let left = self.number(left)?;
        let right = self.number(right)?;
        operation(left.get(), right.get())
    }

    fn date(&self, operand: &Expression) -> Result<Date, CalculationFailure> {
        let reference = match operand {
            Expression::Reference(reference) | Expression::AnchoredReference(reference) => {
                reference.clone()
            }
            Expression::SameRowReference(field) | Expression::AnchoredSameRowReference(field) => {
                FieldRef::new(self.owner.entity.clone(), field.clone())
            }
            _ => return Err(CalculationFailure::TypeMismatch),
        };
        if let Some(value) = self.values.calculated_value(&reference) {
            return match value {
                CalculatedValue::Date(date) => Ok(date),
                CalculatedValue::Number(_)
                | CalculatedValue::Text(_)
                | CalculatedValue::Boolean(_) => Err(CalculationFailure::TypeMismatch),
            };
        }
        match self
            .document
            .entities
            .get(&reference.entity)
            .and_then(|entity| entity.fields.get(&reference.field))
        {
            Some(Value::Date(date)) => Ok(*date),
            Some(Value::Formula(_)) | None => Err(CalculationFailure::FailedDependencies {
                dependencies: BTreeSet::from([reference]),
            }),
            Some(_) => Err(CalculationFailure::TypeMismatch),
        }
    }

    fn reference(&self, reference: &FieldRef) -> Result<Number, CalculationFailure> {
        if let Some(value) = self.values.number_value(reference) {
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

    fn condition(&mut self, expression: &Expression) -> Result<bool, CalculationFailure> {
        match expression {
            Expression::Compare {
                operator,
                left,
                right,
            } => {
                if super::field_type(self.document, self.owner) == Some(&FieldType::Auto)
                    || [left, right].into_iter().any(|operand| {
                        operand
                            .reference_uses(self.owner)
                            .into_iter()
                            .any(|(target, _)| {
                                super::field_type(self.document, &target) == Some(&FieldType::Auto)
                            })
                    })
                {
                    let left = self.comparison_operand(left)?;
                    let right = self.comparison_operand(right)?;
                    compare(*operator, left.as_ref(), right.as_ref())
                } else {
                    let left = self.number(left)?;
                    let right = self.number(right)?;
                    compare_order(*operator, Some(left.cmp(&right)))
                }
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
                    Expression::Reference(reference) | Expression::AnchoredReference(reference) => {
                        reference.clone()
                    }
                    Expression::SameRowReference(field)
                    | Expression::AnchoredSameRowReference(field) => {
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
            _ => match self.calculated(expression)? {
                CalculatedValue::Boolean(value) => Ok(value),
                CalculatedValue::Number(number) => Ok(number.get() != 0.0),
                CalculatedValue::Date(_) => Ok(true),
                CalculatedValue::Text(_) => Err(CalculationFailure::TypeMismatch),
            },
        }
    }

    fn comparison_operand(
        &mut self,
        expression: &Expression,
    ) -> Result<Option<CalculatedValue>, CalculationFailure> {
        let reference = match expression {
            Expression::Reference(reference) | Expression::AnchoredReference(reference) => {
                Some(reference.clone())
            }
            Expression::SameRowReference(field) | Expression::AnchoredSameRowReference(field) => {
                Some(FieldRef::new(self.owner.entity.clone(), field.clone()))
            }
            _ => None,
        };
        if let Some(reference) = reference {
            if super::field_type(self.document, &reference) == Some(&FieldType::Auto)
                && !self.document.entities[&reference.entity]
                    .fields
                    .contains_key(&reference.field)
            {
                return Ok(None);
            }
        }
        self.calculated(expression).map(Some)
    }
}

fn arithmetic_operand(value: CalculatedValue) -> Result<CalculatedValue, CalculationFailure> {
    match value {
        CalculatedValue::Text(text) => {
            let text = text.trim_matches(' ');
            if !text
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'+' | b'-' | b'.' | b'e' | b'E'))
            {
                return Err(CalculationFailure::TypeMismatch);
            }
            let number = text
                .parse::<f64>()
                .map_err(|_| CalculationFailure::TypeMismatch)?;
            number_result(number).map(CalculatedValue::Number)
        }
        CalculatedValue::Boolean(value) => {
            number_result(f64::from(u8::from(value))).map(CalculatedValue::Number)
        }
        CalculatedValue::Date(_) | CalculatedValue::Number(_) => Ok(value),
    }
}

fn compare(
    operator: ComparisonOperator,
    left: Option<&CalculatedValue>,
    right: Option<&CalculatedValue>,
) -> Result<bool, CalculationFailure> {
    fn blank_order(value: &CalculatedValue) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering::{Equal, Less};
        match value {
            CalculatedValue::Number(number) => Some(Number::default().cmp(number)),
            CalculatedValue::Boolean(value) => Some(false.cmp(value)),
            CalculatedValue::Text(text) => Some(if text.is_empty() { Equal } else { Less }),
            CalculatedValue::Date(_) => None,
        }
    }
    let ordering = match (left, right) {
        (None, None) => Some(std::cmp::Ordering::Equal),
        (None, Some(right)) => blank_order(right),
        (Some(left), None) => blank_order(left).map(std::cmp::Ordering::reverse),
        (Some(CalculatedValue::Number(left)), Some(CalculatedValue::Number(right))) => {
            Some(left.cmp(right))
        }
        (Some(CalculatedValue::Boolean(left)), Some(CalculatedValue::Boolean(right))) => {
            Some(left.cmp(right))
        }
        (Some(CalculatedValue::Date(left)), Some(CalculatedValue::Date(right))) => {
            Some(left.cmp(right))
        }
        (Some(CalculatedValue::Text(_)), Some(CalculatedValue::Text(_))) => {
            return Err(CalculationFailure::TypeMismatch);
        }
        (Some(_), Some(_)) => None,
    };
    compare_order(operator, ordering)
}

fn compare_order(
    operator: ComparisonOperator,
    order: Option<std::cmp::Ordering>,
) -> Result<bool, CalculationFailure> {
    use std::cmp::Ordering::{Equal, Greater, Less};
    Ok(match operator {
        ComparisonOperator::Equal => order == Some(Equal),
        ComparisonOperator::NotEqual => order != Some(Equal),
        ComparisonOperator::Less => order.ok_or(CalculationFailure::TypeMismatch)? == Less,
        ComparisonOperator::LessOrEqual => {
            order.ok_or(CalculationFailure::TypeMismatch)? != Greater
        }
        ComparisonOperator::Greater => order.ok_or(CalculationFailure::TypeMismatch)? == Greater,
        ComparisonOperator::GreaterOrEqual => {
            order.ok_or(CalculationFailure::TypeMismatch)? != Less
        }
    })
}

// Howard Hinnant's civil-date algorithm, with an arbitrary 1970-01-01 epoch.
#[must_use]
pub fn days_from_civil(date: Date) -> i32 {
    let mut year = i32::from(date.year());
    let month = i32::from(date.month());
    if month <= 2 {
        year -= 1;
    }
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let shifted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + i32::from(date.day()) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn shift_date(date: Date, offset: f64) -> Result<Date, CalculationFailure> {
    if offset.fract() != 0.0 {
        return Err(CalculationFailure::TypeMismatch);
    }
    let day = f64::from(days_from_civil(date)) + offset;
    if !(-719_162.0..=2_932_896.0).contains(&day) {
        return Err(CalculationFailure::NonFiniteResult);
    }
    #[allow(clippy::cast_possible_truncation)] // Integral and bounded above.
    let day = day as i32 + 719_468;
    let era = day.div_euclid(146_097);
    let day_of_era = day - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i32::from(month <= 2);
    Date::new(
        u16::try_from(year).map_err(|_| CalculationFailure::NonFiniteResult)?,
        u8::try_from(month).map_err(|_| CalculationFailure::NonFiniteResult)?,
        u8::try_from(day).map_err(|_| CalculationFailure::NonFiniteResult)?,
    )
    .map_err(|_| CalculationFailure::NonFiniteResult)
}

/// The first cell of `column`, in entity order, that already carries a failure.
#[cfg(test)]
fn first_failed_cell(
    document: &Document,
    column: &ColumnRef,
    failures: &FailureMap,
) -> Option<FieldRef> {
    column_cells(document, column).find(|cell| failures.contains_key(cell))
}

/// Merge two entity-id lists, each already in entity order, keeping that order.
/// A cell carries at most one key per kind, so the lists never share an id.
fn merge_ids(left: &[EntityId], right: &[EntityId]) -> Vec<EntityId> {
    use std::cmp::Ordering;

    let mut merged = Vec::with_capacity(left.len() + right.len());
    let (mut left_index, mut right_index) = (0_usize, 0_usize);
    while left_index < left.len() || right_index < right.len() {
        let ordering = match (left.get(left_index), right.get(right_index)) {
            (Some(left_id), Some(right_id)) => left_id.cmp(right_id),
            (Some(_), None) => Ordering::Less,
            (None, _) => Ordering::Greater,
        };
        if ordering != Ordering::Greater {
            merged.push(left[left_index].clone());
            left_index += 1;
        }
        if ordering != Ordering::Less {
            merged.push(right[right_index].clone());
            right_index += 1;
        }
    }
    merged
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
    number_result(round_decimal(value, usize::from(digits), None))
}

fn directed_round(value: Number, digits: u8, away: bool) -> Result<Number, CalculationFailure> {
    if digits > 10 {
        return Err(CalculationFailure::TypeMismatch);
    }
    let value = value.get();
    if value.fract() == 0.0 {
        return number_result(value);
    }
    number_result(round_decimal(value, usize::from(digits), Some(away)))
}

/// Round half away from zero on the decimal the user sees, not on the binary64
/// product: `1.005 * 100.0` is `100.49999999999999`, which would round down,
/// while spreadsheets give `ROUND(1.005, 2) = 1.01`. Rust's `Display` prints
/// the shortest decimal that round-trips to the same binary64 value, so the
/// digit after the kept places decides, then the result is parsed back.
fn round_decimal(value: f64, digits: usize, directed: Option<bool>) -> f64 {
    let text = format!("{}", value.abs());
    let (integer, fraction) = text.split_once('.').unwrap_or((text.as_str(), ""));
    if fraction.len() <= digits {
        return value;
    }
    let mut kept = integer
        .bytes()
        .chain(fraction.bytes().take(digits))
        .collect::<Vec<_>>();
    let increment = match directed {
        None => fraction.as_bytes()[digits] >= b'5',
        Some(away) => {
            away && fraction.as_bytes()[digits..]
                .iter()
                .any(|digit| *digit != b'0')
        }
    };
    if increment {
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

/// The exactly rounded sum over the count, so cancelling values keep their
/// remainder. A sum beyond f64 is taken again in two parts: values that scale
/// exactly are summed at 2^-64 (the count stays below 2^64, so this part
/// cannot overflow), and the rest unscaled, so tiny values are not lost.
///
/// Callers supply a nonempty slice of finite values.
#[must_use]
pub fn average(values: &[f64]) -> f64 {
    let count = value_count(values.len());
    let sum = exact_sum(values.iter().copied());
    if sum.is_finite() {
        return sum / count;
    }
    let scale = 2f64.powi(64);
    let exact = f64::MIN_POSITIVE * scale;
    let large = exact_sum(
        values
            .iter()
            .filter(|value| value.abs() >= exact)
            .map(|value| value / scale),
    );
    let small = exact_sum(values.iter().copied().filter(|value| value.abs() < exact));
    // Unscale before dividing whenever that cannot overflow, so a large part
    // that cancels to a tiny remainder does not underflow in the division.
    let large = if large.abs() < f64::MAX / scale {
        large * scale / count
    } else {
        large / count * scale
    };
    large + small / count
}

// Columns hold far fewer than 2^32 rows, so the count converts exactly.
fn value_count(len: usize) -> f64 {
    f64::from(u32::try_from(len).unwrap_or(u32::MAX))
}

/// Shewchuk's exact summation with a correctly rounded result (the algorithm
/// of Python's `math.fsum`): non-overlapping partials hold the sum exactly.
/// A non-finite result means the sum left f64.
#[must_use]
#[allow(clippy::float_cmp)] // exactness tests on error terms are the algorithm
pub fn exact_sum(values: impl Iterator<Item = f64>) -> f64 {
    let mut partials: Vec<f64> = Vec::new();
    for mut value in values {
        let mut kept = 0;
        for index in 0..partials.len() {
            let mut partial = partials[index];
            if value.abs() < partial.abs() {
                std::mem::swap(&mut value, &mut partial);
            }
            let high = value + partial;
            let low = partial - (high - value);
            if low != 0.0 {
                partials[kept] = low;
                kept += 1;
            }
            value = high;
        }
        if !value.is_finite() {
            // The sum left f64. Its NaN error terms would otherwise be kept
            // and grow the partials with every later value.
            return value;
        }
        partials.truncate(kept);
        partials.push(value);
    }
    let Some(mut high) = partials.pop() else {
        return 0.0;
    };
    let mut low = 0.0;
    while let Some(partial) = partials.pop() {
        let previous = high;
        high = previous + partial;
        low = partial - (high - previous);
        if low != 0.0 {
            break;
        }
    }
    // Round half to even across the remaining partials.
    if partials
        .last()
        .is_some_and(|next| (low < 0.0 && *next < 0.0) || (low > 0.0 && *next > 0.0))
    {
        let doubled = low * 2.0;
        let rounded = high + doubled;
        if doubled == rounded - high {
            high = rounded;
        }
    }
    high
}

// The first key is always the cell's own typed key. SUMIF's additional
// numeric key for numeric text is deliberately excluded from exact lookup.
fn exact_key(cell: CriterionCell<'_>) -> CriterionKey {
    cell.keys()
        .into_iter()
        .next()
        .expect("a scalar cell has its own typed key")
}

fn number_result(value: f64) -> Result<Number, CalculationFailure> {
    Number::new(value).map_err(|_| CalculationFailure::NonFiniteResult)
}

#[cfg(test)]
mod conditional_tests;

/// Counts the rows a conditional walk reads and which walk ran, so a test can
/// bound one calculation's work and prove the path it took.
#[cfg(test)]
mod conditional_walks {
    use std::cell::Cell;

    thread_local! {
        static ROWS: Cell<usize> = const { Cell::new(0) };
        static CANDIDATE_WALKS: Cell<usize> = const { Cell::new(0) };
        static FULL_WALKS: Cell<usize> = const { Cell::new(0) };
        static BYPASS: Cell<bool> = const { Cell::new(false) };
    }

    pub(crate) fn record_row() {
        ROWS.with(|rows| rows.set(rows.get() + 1));
    }

    pub(crate) fn record_walk(candidates: bool) {
        let walks = if candidates {
            &CANDIDATE_WALKS
        } else {
            &FULL_WALKS
        };
        walks.with(|walks| walks.set(walks.get() + 1));
    }

    /// The rows read since the last call, with the candidate and full walks.
    pub(crate) fn take() -> (usize, usize, usize) {
        (
            ROWS.with(|rows| rows.replace(0)),
            CANDIDATE_WALKS.with(|walks| walks.replace(0)),
            FULL_WALKS.with(|walks| walks.replace(0)),
        )
    }

    pub(crate) fn bypass_caches() -> bool {
        BYPASS.with(Cell::get)
    }

    pub(crate) fn set_bypass(bypass: bool) {
        BYPASS.with(|flag| flag.set(bypass));
    }
}

#[cfg(test)]
thread_local! {
    static LOOKUP_INDEX_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod lookup_tests;
