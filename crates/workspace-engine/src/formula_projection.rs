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
