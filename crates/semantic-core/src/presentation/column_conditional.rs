use crate::{ColumnCriterion, ConditionalAggregate, Document};

use super::{PresentationError, check_column};

pub(super) fn validate_column_conditional(
    document: &Document,
    aggregate: &ConditionalAggregate,
    criteria: &[ColumnCriterion],
) -> Result<(), PresentationError> {
    match aggregate {
        ConditionalAggregate::Sum(column) | ConditionalAggregate::Average(column) => {
            check_column(document, column)?;
        }
        ConditionalAggregate::Count => {}
    }
    for criterion in criteria {
        check_column(document, &criterion.column)?;
    }
    Ok(())
}
