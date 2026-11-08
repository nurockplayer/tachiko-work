//! Semantic document contracts for Tachiko Work.

mod address;
mod criterion;
mod diagnostic;
mod display_name;
mod input_classification;
pub mod instrumentation;
mod keyed_grouped_sum;
mod model;
mod presentation;
mod validation;

pub use address::{AddressIndex, AddressIndexError};
pub use criterion::{
    CriterionCell, CriterionKey, CriterionOperandKind, CriterionTest, CriterionTestKey,
    InvalidCriterion, MAX_CONDITIONAL_CRITERIA,
};
pub use diagnostic::{
    Diagnostic, DiagnosticCode, DiagnosticFact, DiagnosticLocation, DiagnosticProvider,
    DiagnosticSeverity, SemanticSubject, StableDiagnosticObservation,
};
pub use display_name::{
    DisplayNameFailure, MAX_DISPLAY_NAME_BYTES, display_name_failures, is_display_name_whitespace,
    trim_display_name,
};
pub use input_classification::{ClassifiedDateDisplay, ClassifiedInput, DateOrder, classify_input};
pub use keyed_grouped_sum::{
    KeyedGroupedSumBindingRole, KeyedGroupedSumDefinition, KeyedGroupedSumDefinitionError,
    KeyedGroupedSumDefinitionId, KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding,
    validate_keyed_grouped_sum_definitions,
};
pub use model::{
    ColumnAddress, ColumnAggregateFunction, ColumnCriterion, ColumnRef, ComparisonOperator,
    ConditionalAggregate, Date, Document, DocumentId, Entity, EntityId, EntityKey, Expression,
    FieldAddress, FieldConstraint, FieldDefinition, FieldId, FieldKey, FieldRef, FieldType,
    FormulaExpressionType, InvalidDate, InvalidNumber, MAX_EXPRESSION_DEPTH, MAX_EXPRESSION_NODES,
    Number, NumberFormat, NumberFormatError, NumberFormatKind, Schema, SchemaId, SchemaKey, Value,
};
pub use presentation::{
    BorderSide, CellAlignment, CellStyle, ConditionalRule, DateFormat, FooterAggregate,
    MAX_RULES_PER_TABLE, MAX_STYLES, PresentationError, PresentationStore, RuleId, StyleId,
    TablePresentation, prune_presentation, validate_presentation,
};
#[cfg(feature = "issue-175-research")]
pub use validation::validate_document_cancellable;
pub use validation::{
    is_valid_identifier, validate_complete_formula_constraints, validate_document,
    validate_document_core, validate_entity_field_core, validate_formula_value_constraint,
};
