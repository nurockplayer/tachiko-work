//! Semantic document contracts for Tachiko Work.

mod model;
mod validation;

pub use model::{
    Document, DocumentId, Entity, EntityId, Expression, FieldDefinition, FieldId, FieldRef,
    FieldType, Schema, SchemaId, Value,
};
pub use validation::{Diagnostic, DiagnosticCode, validate_document};
