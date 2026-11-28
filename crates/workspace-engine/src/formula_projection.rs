use tachiko_formula_engine::{project_expression, project_expression_with_index};
use tachiko_semantic_core::{AddressIndex, Document, Expression, FieldRef};

use super::WorkspaceError;

pub(super) fn indexed(
    document: &Document,
    index: &AddressIndex,
    field: FieldRef,
    expression: &Expression,
) -> Result<(), WorkspaceError> {
    project_expression_with_index(document, index, &field, expression)
        .map_err(|source| WorkspaceError::FormulaProjection { field, source })?;
    Ok(())
}

pub(super) fn unindexed(
    document: &Document,
    field: FieldRef,
    expression: &Expression,
) -> Result<(), WorkspaceError> {
    project_expression(document, &field, expression)
        .map_err(|source| WorkspaceError::FormulaProjection { field, source })?;
    Ok(())
}

pub(super) fn local_failure_diagnostic(
    failure: &super::CalculationFailure,
    formula: &super::FieldRef,
    subject: super::SemanticSubject,
    path: String,
) -> super::Diagnostic {
    use super::{CalculationFailure, diagnostic_codes, formula_diagnostic};
    let (code, message) = match failure {
        CalculationFailure::DeletedReference => (
            diagnostic_codes::FORMULA_INVALID_REFERENCES,
            "reads a deleted reference",
        ),
        CalculationFailure::LookupMissingKey => (
            diagnostic_codes::FORMULA_LOOKUP_MISSING_KEY,
            "has no matching lookup key",
        ),
        CalculationFailure::EmptyAggregate => (
            diagnostic_codes::FORMULA_AGGREGATE_EMPTY,
            "reads an empty aggregate",
        ),
        CalculationFailure::DivisionByZero => (
            diagnostic_codes::FORMULA_DIVISION_BY_ZERO,
            "divided by zero",
        ),
        CalculationFailure::NonFiniteResult => (
            diagnostic_codes::FORMULA_NON_FINITE_RESULT,
            "produced a non-finite result",
        ),

        _ => unreachable!("structural and dependency failures are projected separately"),
    };
    formula_diagnostic(
        code,
        vec![subject],
        path,
        format!("formula '{formula}' {message}"),
    )
}
