//! Semantic document contracts for Tachiko Work.

mod address;
mod model;
mod validation;

pub use address::{AddressIndex, AddressIndexError};
pub use model::{
    Document, DocumentId, Entity, EntityId, EntityKey, Expression, FieldAddress, FieldDefinition,
    FieldId, FieldKey, FieldRef, FieldType, InvalidNumber, MAX_EXPRESSION_DEPTH,
    MAX_EXPRESSION_NODES, Number, Schema, SchemaId, SchemaKey, Value,
};
pub use validation::{Diagnostic, DiagnosticCode, is_valid_identifier, validate_document};
