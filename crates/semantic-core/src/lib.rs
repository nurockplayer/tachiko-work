//! Semantic document contracts for Tachiko Work.

mod address;
mod diagnostic;
mod keyed_grouped_sum;
mod model;
mod validation;

pub use address::{AddressIndex, AddressIndexError};
pub use diagnostic::{
    Diagnostic, DiagnosticCode, DiagnosticFact, DiagnosticLocation, DiagnosticProvider,
    DiagnosticSeverity, SemanticSubject, StableDiagnosticObservation,
};
pub use keyed_grouped_sum::{
    KeyedGroupedSumBindingRole, KeyedGroupedSumDefinition, KeyedGroupedSumDefinitionError,
    KeyedGroupedSumDefinitionId, KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding,
    validate_keyed_grouped_sum_definitions,
};
pub use model::{
    Date, Document, DocumentId, Entity, EntityId, EntityKey, Expression, FieldAddress,
    FieldDefinition, FieldId, FieldKey, FieldRef, FieldType, InvalidDate, InvalidNumber,
    MAX_EXPRESSION_DEPTH, MAX_EXPRESSION_NODES, Number, Schema, SchemaId, SchemaKey, Value,
};
#[cfg(feature = "issue-175-research")]
pub use validation::validate_document_cancellable;
pub use validation::{is_valid_identifier, validate_document, validate_document_core};
