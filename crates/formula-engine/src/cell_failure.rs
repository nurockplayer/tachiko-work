use super::{CalculationFailure, CellFailures, FieldRef};

/// A calculated error published in an Auto cell, with the cell where it began.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CellFailure {
    pub code: CellFailureCode,
    pub origin: FieldRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CellFailureCode {
    DivisionByZero,
    Value,
    Number,
    Reference,
    NotAvailable,
}

impl CellFailureCode {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::DivisionByZero => "division_by_zero",
            Self::Value => "value",
            Self::Number => "number",
            Self::Reference => "reference",
            Self::NotAvailable => "lookup_missing_key",
        }
    }

    #[must_use]
    pub const fn display(self) -> &'static str {
        match self {
            Self::DivisionByZero => "#DIV/0!",
            Self::Value => "#VALUE!",
            Self::Number => "#NUM!",
            Self::Reference => "#REF!",
            Self::NotAvailable => "#N/A",
        }
    }
}

impl CellFailure {
    pub(super) fn from_error(
        owner: &FieldRef,
        failure: &CalculationFailure,
        cell_failures: &CellFailures,
    ) -> Self {
        let code = match failure {
            CalculationFailure::DivisionByZero | CalculationFailure::EmptyAggregate => {
                CellFailureCode::DivisionByZero
            }
            CalculationFailure::TypeMismatch => CellFailureCode::Value,
            CalculationFailure::NonFiniteResult => CellFailureCode::Number,
            CalculationFailure::FailedDependencies { dependencies } => {
                let origin = dependencies
                    .first()
                    .expect("a failed dependency names its target");
                return cell_failures.get(origin).cloned().unwrap_or_else(|| Self {
                    code: CellFailureCode::Reference,
                    origin: origin.clone(),
                });
            }
            CalculationFailure::LookupMissingKey => CellFailureCode::NotAvailable,
            CalculationFailure::InvalidExpression { .. }
            | CalculationFailure::InvalidReferences { .. }
            | CalculationFailure::Cycle { .. }
            | CalculationFailure::InvalidColumn { .. } => {
                unreachable!("formula definition failures precede evaluation")
            }
        };
        Self {
            code,
            origin: owner.clone(),
        }
    }
}
