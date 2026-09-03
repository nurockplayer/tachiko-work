//! Provisional subject-first field capability discovery for Issue #268.
//!
//! This projection is deliberately finite and operation-family specific. It
//! reports semantic applicability from the same workspace rules used by
//! mutation and formula paths; it does not define a type registry, generic
//! operation DTO, conversion catalogue, authorization grant, or UI model.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::formula_operations::{
    formula_reasoning_target_is_applicable, number_override_target_is_applicable,
};
use crate::patch_lifecycle::{
    DisclosureRequirement, OperationFamily, PatchLifecycle, PatchLifecycleError, PrincipalId,
    ScopedSemanticSubject, SemanticRevision, SemanticScope, TrustedInstant,
};
use crate::resident_session::ResidentSnapshot;
use crate::{
    Document, DocumentId, FieldRef, FieldType, SemanticValueKind, Value, WorkspaceError,
    field_definition, field_value_input_rule, formula_update_target_rule, semantic_value_kind,
};

/// Whether a discovered family is read-only or can describe a semantic edit.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldCapabilityKind {
    Query,
    Edit,
}

/// Typed input expected by one discovered family.
///
/// `TypedValue` carries only the finite current semantic value kind. For a
/// Reference field, the declared `FieldType` on [`FieldCapabilities`] carries
/// the target schema contract; this projection never enumerates valid targets.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldCapabilityInput {
    TypedValue { kind: SemanticValueKind },
    Formula,
    Number,
    None,
}

/// Machine-readable semantic applicability for one family/input projection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldCapabilityApplicability {
    Applicable,
    Inapplicable {
        reason: FieldCapabilityInapplicability,
    },
}

/// Disclosure-safe reasons for a known family not applying to the field.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldCapabilityInapplicability {
    FormulaEdit,
    TypeMismatch {
        expected: FieldType,
        actual: SemanticValueKind,
    },
    NonNumericFormulaField,
    NotFormula,
    UnsupportedValueKind {
        actual: SemanticValueKind,
    },
}

/// One finite operation-family capability projection for a stable field.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FieldCapability {
    pub family: OperationFamily,
    pub kind: FieldCapabilityKind,
    pub input: FieldCapabilityInput,
    pub applicability: FieldCapabilityApplicability,
}

/// All v1 field-local capability projections visible after authorization.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FieldCapabilities {
    pub field: FieldRef,
    pub declared_type: FieldType,
    pub current_value_kind: SemanticValueKind,
    pub capabilities: Vec<FieldCapability>,
}

/// Exact source-revision evidence for a field capability Query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldCapabilityQueryContext {
    source_revision: SemanticRevision,
}

impl FieldCapabilityQueryContext {
    fn trusted(source_revision: SemanticRevision) -> Self {
        Self { source_revision }
    }

    #[must_use]
    pub fn source_revision(&self) -> &SemanticRevision {
        &self.source_revision
    }
}

/// Disclosure-safe result for a stable field target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldCapabilityQueryOutcome {
    Field(FieldCapabilities),
    UnresolvedTarget { field: FieldRef },
}

/// Field capability Query result tied to one exact semantic context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldCapabilityQueryResult {
    pub document: DocumentId,
    pub context: FieldCapabilityQueryContext,
    pub outcome: FieldCapabilityQueryOutcome,
}

/// Project the bounded field capability facts without authorization.
///
/// This is an internal semantic projection used by the authorized lifecycle
/// Query below and by parity tests. Callers crossing a client boundary must
/// use [`PatchLifecycle::query_field_capabilities`], which performs disclosure
/// authorization before invoking this projection.
///
/// # Errors
///
/// Returns a typed lookup error when the stable field, its entity, or its
/// schema definition is absent.
pub(crate) fn describe_field_capabilities(
    document: &Document,
    field: &FieldRef,
) -> Result<FieldCapabilities, WorkspaceError> {
    let entity =
        document
            .entities
            .get(&field.entity)
            .ok_or_else(|| WorkspaceError::MissingEntityId {
                entity: field.entity.clone(),
            })?;
    let existing = entity
        .fields
        .get(&field.field)
        .ok_or_else(|| WorkspaceError::MissingField {
            field: field.clone(),
        })?;
    let definition = field_definition(document, field)?;
    let declared_type = definition.field_type.clone();
    let current_value_kind = semantic_value_kind(existing);

    let mut capabilities = Vec::with_capacity(8);
    for input_kind in [
        SemanticValueKind::Number,
        SemanticValueKind::Text,
        SemanticValueKind::Boolean,
        SemanticValueKind::Date,
        SemanticValueKind::Reference,
    ] {
        capabilities.push(FieldCapability {
            family: OperationFamily::SetFieldValue,
            kind: FieldCapabilityKind::Edit,
            input: FieldCapabilityInput::TypedValue { kind: input_kind },
            applicability: field_value_applicability(field, existing, input_kind, &declared_type)?,
        });
    }

    capabilities.push(FieldCapability {
        family: OperationFamily::FormulaUpdate,
        kind: FieldCapabilityKind::Edit,
        input: FieldCapabilityInput::Formula,
        applicability: formula_update_applicability(document, field)?,
    });
    capabilities.push(FieldCapability {
        family: OperationFamily::FormulaReasoning,
        kind: FieldCapabilityKind::Query,
        input: FieldCapabilityInput::None,
        applicability: if formula_reasoning_target_is_applicable(existing) {
            FieldCapabilityApplicability::Applicable
        } else {
            FieldCapabilityApplicability::Inapplicable {
                reason: FieldCapabilityInapplicability::NotFormula,
            }
        },
    });
    capabilities.push(FieldCapability {
        family: OperationFamily::NumberOverrideScenario,
        kind: FieldCapabilityKind::Query,
        input: FieldCapabilityInput::Number,
        applicability: if number_override_target_is_applicable(existing) {
            FieldCapabilityApplicability::Applicable
        } else {
            FieldCapabilityApplicability::Inapplicable {
                reason: FieldCapabilityInapplicability::UnsupportedValueKind {
                    actual: current_value_kind,
                },
            }
        },
    });

    Ok(FieldCapabilities {
        field: field.clone(),
        declared_type,
        current_value_kind,
        capabilities,
    })
}

fn field_value_applicability(
    field: &FieldRef,
    existing: &Value,
    input_kind: SemanticValueKind,
    field_type: &FieldType,
) -> Result<FieldCapabilityApplicability, WorkspaceError> {
    match field_value_input_rule(field, existing, input_kind, field_type) {
        Ok(()) => Ok(FieldCapabilityApplicability::Applicable),
        Err(WorkspaceError::FormulaEdit { .. }) => Ok(FieldCapabilityApplicability::Inapplicable {
            reason: FieldCapabilityInapplicability::FormulaEdit,
        }),
        Err(WorkspaceError::TypeMismatch { .. }) => {
            Ok(FieldCapabilityApplicability::Inapplicable {
                reason: FieldCapabilityInapplicability::TypeMismatch {
                    expected: field_type.clone(),
                    actual: input_kind,
                },
            })
        }
        Err(error) => Err(error),
    }
}

fn formula_update_applicability(
    document: &Document,
    field: &FieldRef,
) -> Result<FieldCapabilityApplicability, WorkspaceError> {
    match formula_update_target_rule(document, field) {
        Ok(()) => Ok(FieldCapabilityApplicability::Applicable),
        Err(WorkspaceError::NonNumericFormulaField { .. }) => {
            Ok(FieldCapabilityApplicability::Inapplicable {
                reason: FieldCapabilityInapplicability::NonNumericFormulaField,
            })
        }
        Err(error) => Err(error),
    }
}

impl PatchLifecycle {
    /// Query the existing semantic capabilities for one stable field target.
    ///
    /// Authorization is performed before target/type/value classification.
    /// The host-provided [`ResidentSnapshot`] keeps the document, occurrence,
    /// and opaque revision paired for the whole Query.
    /// The discovery Query itself does not imply Query, Propose, Execute, or
    /// Approval authority for any family in the returned projection; each
    /// later operation re-evaluates its own live rules and grants.
    ///
    /// # Errors
    ///
    /// Returns disclosure denial before semantic facts when the caller lacks
    /// the independent field-capability Query grants for both a resolvable
    /// field instance and its corresponding schema definition. A target that
    /// is safely unresolved after authorized document-scope disclosure is
    /// returned as a structured unresolved outcome.
    pub fn query_field_capabilities(
        &self,
        snapshot: &ResidentSnapshot,
        target: &FieldRef,
        principal: &PrincipalId,
        now: TrustedInstant,
    ) -> Result<FieldCapabilityQueryResult, PatchLifecycleError> {
        let document_scope = snapshot.document_scope();
        let document = snapshot.document();
        self.require_document(document_scope, document)?;
        self.require_active_principal(principal)?;
        let context = FieldCapabilityQueryContext::trusted(snapshot.revision().clone());
        let requirements = match self.field_scope(document, target) {
            Ok(entity_field_scope) => {
                let schema_field_scope = match entity_field_scope.subject() {
                    SemanticScope::EntityField { schema, field, .. } => ScopedSemanticSubject::new(
                        entity_field_scope.document_scope().clone(),
                        entity_field_scope.document().clone(),
                        SemanticScope::SchemaField {
                            schema: schema.clone(),
                            field: field.clone(),
                        },
                    ),
                    _ => unreachable!("field_scope must return an EntityField scope"),
                };
                BTreeSet::from([
                    DisclosureRequirement {
                        family: OperationFamily::FieldCapabilityDiscovery,
                        scope: entity_field_scope,
                    },
                    DisclosureRequirement {
                        family: OperationFamily::FieldCapabilityDiscovery,
                        scope: schema_field_scope,
                    },
                ])
            }
            Err(_) => BTreeSet::from([DisclosureRequirement {
                family: OperationFamily::FieldCapabilityDiscovery,
                scope: ScopedSemanticSubject::new(
                    document_scope.clone(),
                    document.id.clone(),
                    SemanticScope::Document,
                ),
            }]),
        };
        self.authorize_query(principal, &requirements, now)
            .map_err(|error| match error {
                PatchLifecycleError::InsufficientCapability { .. } => {
                    PatchLifecycleError::DisclosureDenied
                }
                other => other,
            })?;

        let outcome = match describe_field_capabilities(document, target) {
            Ok(capabilities) => FieldCapabilityQueryOutcome::Field(capabilities),
            Err(
                WorkspaceError::MissingEntityId { .. }
                | WorkspaceError::MissingSchema { .. }
                | WorkspaceError::MissingField { .. },
            ) => FieldCapabilityQueryOutcome::UnresolvedTarget {
                field: target.clone(),
            },
            Err(error) => {
                return Err(PatchLifecycleError::CommandRejected {
                    source: Box::new(error),
                });
            }
        };
        Ok(FieldCapabilityQueryResult {
            document: document.id.clone(),
            context,
            outcome,
        })
    }
}
