use tachiko_semantic_core::{ColumnCriterion, ConditionalAggregate, CriterionTest, Number};

use super::{average, number_result, value_count};
use crate::CalculationFailure;

#[derive(Clone, Copy)]
pub(super) struct ConditionalCriteria<'a> {
    pub(super) criteria: &'a [ColumnCriterion],
    pub(super) tests: &'a [CriterionTest],
}

pub(super) fn reduce(
    aggregate: &ConditionalAggregate,
    matched: usize,
    values: &[f64],
) -> Result<Number, CalculationFailure> {
    number_result(match aggregate {
        ConditionalAggregate::Count => value_count(matched),
        ConditionalAggregate::Sum(_) => values.iter().copied().sum(),
        ConditionalAggregate::Average(_) => {
            if values.is_empty() {
                return Err(CalculationFailure::EmptyAggregate);
            }
            average(values)
        }
    })
}
