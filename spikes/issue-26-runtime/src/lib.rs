//! Provisional executable evidence for Issue #26 runtime ownership.
//!
//! This crate is deliberately outside the production Cargo workspace. Its JSON
//! types and snapshot encoding are spike evidence, not a stable SDK contract.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tachiko_workspace_engine::{
    Document, DocumentId, Entity, EntityId, EntityKey, Expression, FieldAddress, FieldDefinition,
    FieldId, FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, SemanticChange,
    SemanticDiff, Value, WorkspaceError, WorkspaceMergeOutcome, calculate_fields, merge_documents,
    overview, set_scalar, validate,
};
use thiserror::Error;

#[cfg(target_arch = "wasm32")]
mod wasm;

/// A provisional command that crosses the spike boundary.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    Overview,
    Calculate,
    SetScalar {
        address: FieldAddress,
        input: String,
    },
    Merge {
        base: Document,
        theirs: Document,
    },
}

/// One calculated projection returned to a frontend cache.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CalculatedProjection {
    pub field: FieldRef,
    pub address: String,
    pub value: f64,
}

/// One current semantic value suitable for a revision-keyed frontend cache.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProjectionValue {
    Number { value: f64 },
    Text { value: String },
    Boolean { value: bool },
    Reference { entity: EntityId },
    Formula { expression: Expression },
}

/// A stable-subject patch. `None` removes the field from a frontend cache.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProjectionPatch {
    pub field: FieldRef,
    pub value: Option<ProjectionValue>,
}

/// Provisional result payloads. None of these types are a public SDK promise.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandResult {
    Overview {
        schema_count: usize,
        entity_count: usize,
        formula_count: usize,
    },
    Calculation {
        calculated: Vec<CalculatedProjection>,
    },
    Mutation {
        change_count: usize,
        diff_text: String,
        patches: Vec<ProjectionPatch>,
    },
    Merge {
        merged: bool,
        conflict_count: usize,
        change_count: usize,
        diff_text: String,
        patches: Vec<ProjectionPatch>,
    },
}

/// A revisioned response from the Rust-owned resident aggregate.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CommandResponse {
    pub revision: u64,
    pub result: CommandResult,
}

/// Stateless comparison result that necessarily carries the next full snapshot.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SnapshotExecution {
    pub snapshot: Vec<u8>,
    pub response: CommandResponse,
}

/// The complete provisional adapter request vocabulary used by native and WASM.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WireRequest {
    GenerateSynthetic {
        entity_count: usize,
    },
    OpenSynthetic {
        entity_count: usize,
    },
    Open {
        document: Document,
    },
    Execute {
        command: Command,
    },
    Snapshot,
    ExecuteSnapshot {
        document: Document,
        command: Command,
    },
}

/// Successful provisional adapter payloads.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WireResult {
    Generated {
        document: Document,
    },
    Opened {
        revision: u64,
    },
    Command {
        response: CommandResponse,
    },
    Snapshot {
        document: Document,
    },
    SnapshotExecution {
        document: Document,
        response: CommandResponse,
    },
}

/// A transport-only reply. The error string deliberately does not define #23.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WireReply {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<WireResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Error)]
pub enum SpikeError {
    #[error("could not decode the provisional semantic snapshot: {0}")]
    SnapshotDecode(#[source] serde_json::Error),
    #[error("could not encode the provisional semantic snapshot: {0}")]
    SnapshotEncode(#[source] serde_json::Error),
    #[error("workspace operation failed: {0}")]
    Workspace(#[from] WorkspaceError),
    #[error("synthetic projects require at least one entity")]
    EmptySyntheticProject,
    #[error("synthetic project entity count {entity_count} exceeds the spike generator limit")]
    SyntheticProjectTooLarge { entity_count: usize },
    #[error("synthetic fixture produced an invalid number")]
    InvalidSyntheticNumber,
    #[error("resident runtime is not open")]
    RuntimeNotOpen,
}

/// One authoritative semantic aggregate retained in Rust.
pub struct ResidentRuntime {
    document: Document,
    revision: u64,
}

impl ResidentRuntime {
    /// Open one provisional snapshot and validate it before making it resident.
    ///
    /// # Errors
    ///
    /// Returns a decode or workspace validation/calculation error.
    pub fn open(snapshot: &[u8]) -> Result<Self, SpikeError> {
        let document = serde_json::from_slice(snapshot).map_err(SpikeError::SnapshotDecode)?;
        Self::from_document(document)
    }

    /// Make an already-decoded aggregate resident after shared validation.
    ///
    /// # Errors
    ///
    /// Returns the current workspace validation/calculation error.
    pub fn from_document(document: Document) -> Result<Self, SpikeError> {
        validate(&document)?;
        Ok(Self {
            document,
            revision: 0,
        })
    }

    /// Execute a semantic command against the resident aggregate.
    ///
    /// # Errors
    ///
    /// Returns the existing workspace-engine failure without publishing a new
    /// revision when a command is rejected.
    pub fn execute(&mut self, command: Command) -> Result<CommandResponse, SpikeError> {
        let result = match command {
            Command::Overview => {
                let overview = overview(&self.document)?;
                CommandResult::Overview {
                    schema_count: overview.schema_count,
                    entity_count: overview.entity_count,
                    formula_count: overview.formula_count,
                }
            }
            Command::Calculate => CommandResult::Calculation {
                calculated: calculated_projection(&self.document)?,
            },
            Command::SetScalar { address, input } => {
                let preview = set_scalar(&self.document, &address, &input)?;
                let patches = projection_patch(&preview.diff);
                let result = CommandResult::Mutation {
                    change_count: preview.diff.changes().len(),
                    diff_text: preview.diff.render_text(),
                    patches,
                };
                self.document = preview.document;
                self.revision += 1;
                result
            }
            Command::Merge { base, theirs } => {
                match merge_documents(&base, &self.document, &theirs)? {
                    WorkspaceMergeOutcome::Merged(preview) => {
                        let patches = projection_patch(&preview.diff);
                        let result = CommandResult::Merge {
                            merged: true,
                            conflict_count: 0,
                            change_count: preview.diff.changes().len(),
                            diff_text: preview.diff.render_text(),
                            patches,
                        };
                        self.document = preview.document;
                        self.revision += 1;
                        result
                    }
                    WorkspaceMergeOutcome::Conflicted(conflicts) => CommandResult::Merge {
                        merged: false,
                        conflict_count: conflicts.len(),
                        change_count: 0,
                        diff_text: String::new(),
                        patches: Vec::new(),
                    },
                }
            }
        };

        Ok(CommandResponse {
            revision: self.revision,
            result,
        })
    }

    /// Serialize the complete resident aggregate for host persistence or the
    /// explicit whole-document comparison path.
    ///
    /// # Errors
    ///
    /// Returns a provisional snapshot encoding error.
    pub fn snapshot(&self) -> Result<Vec<u8>, SpikeError> {
        serde_json::to_vec(&self.document).map_err(SpikeError::SnapshotEncode)
    }

    /// Borrow the authoritative resident document inside the Rust adapter.
    #[must_use]
    pub const fn document(&self) -> &Document {
        &self.document
    }
}

/// Execute one command by decoding and returning the whole document.
///
/// # Errors
///
/// Returns the same decode, semantic, or encode errors as the resident path.
pub fn execute_snapshot(
    snapshot: &[u8],
    command: Command,
) -> Result<SnapshotExecution, SpikeError> {
    let mut runtime = ResidentRuntime::open(snapshot)?;
    let response = runtime.execute(command)?;
    let snapshot = runtime.snapshot()?;
    Ok(SnapshotExecution { snapshot, response })
}

/// Process one complete adapter message without exposing Rust memory layouts.
#[must_use]
pub fn process_wire_request(runtime: &mut Option<ResidentRuntime>, input: &[u8]) -> Vec<u8> {
    let reply = serde_json::from_slice::<WireRequest>(input).map_or_else(
        |error| WireReply {
            ok: false,
            result: None,
            error: Some(format!("invalid spike request: {error}")),
        },
        |request| match handle_wire_request(runtime, request) {
            Ok(result) => WireReply {
                ok: true,
                result: Some(result),
                error: None,
            },
            Err(error) => WireReply {
                ok: false,
                result: None,
                error: Some(error.to_string()),
            },
        },
    );
    serde_json::to_vec(&reply)
        .unwrap_or_else(|_| br#"{"ok":false,"error":"spike reply encoding failed"}"#.to_vec())
}

fn handle_wire_request(
    runtime: &mut Option<ResidentRuntime>,
    request: WireRequest,
) -> Result<WireResult, SpikeError> {
    match request {
        WireRequest::GenerateSynthetic { entity_count } => Ok(WireResult::Generated {
            document: synthetic_document(entity_count)?,
        }),
        WireRequest::OpenSynthetic { entity_count } => {
            *runtime = Some(ResidentRuntime::from_document(synthetic_document(
                entity_count,
            )?)?);
            Ok(WireResult::Opened { revision: 0 })
        }
        WireRequest::Open { document } => {
            *runtime = Some(ResidentRuntime::from_document(document)?);
            Ok(WireResult::Opened { revision: 0 })
        }
        WireRequest::Execute { command } => {
            let runtime = runtime.as_mut().ok_or(SpikeError::RuntimeNotOpen)?;
            Ok(WireResult::Command {
                response: runtime.execute(command)?,
            })
        }
        WireRequest::Snapshot => {
            let runtime = runtime.as_ref().ok_or(SpikeError::RuntimeNotOpen)?;
            Ok(WireResult::Snapshot {
                document: runtime.document().clone(),
            })
        }
        WireRequest::ExecuteSnapshot { document, command } => {
            let mut snapshot_runtime = ResidentRuntime::from_document(document)?;
            let response = snapshot_runtime.execute(command)?;
            Ok(WireResult::SnapshotExecution {
                document: snapshot_runtime.document().clone(),
                response,
            })
        }
    }
}

fn calculated_projection(document: &Document) -> Result<Vec<CalculatedProjection>, SpikeError> {
    calculate_fields(document).map_or_else(
        |error| Err(error.into()),
        |fields| {
            Ok(fields
                .into_iter()
                .map(|field| CalculatedProjection {
                    field: field.field,
                    address: field.address.to_string(),
                    value: field.value.get(),
                })
                .collect())
        },
    )
}

fn projection_patch(diff: &SemanticDiff) -> Vec<ProjectionPatch> {
    let mut patches = BTreeMap::new();
    for change in diff.changes() {
        match change {
            SemanticChange::FieldAdded { field, value } => {
                patches.insert(field.clone(), Some(projection_value(value)));
            }
            SemanticChange::FieldRemoved { field, .. } => {
                patches.insert(field.clone(), None);
            }
            SemanticChange::FieldChanged { field, after, .. } => {
                patches.insert(field.clone(), Some(projection_value(after)));
            }
            SemanticChange::FormulaImpact { field, after, .. } => {
                patches.insert(
                    field.clone(),
                    Some(ProjectionValue::Number { value: after.get() }),
                );
            }
            _ => {}
        }
    }
    patches
        .into_iter()
        .map(|(field, value)| ProjectionPatch { field, value })
        .collect()
}

fn projection_value(value: &Value) -> ProjectionValue {
    match value {
        Value::Number(value) => ProjectionValue::Number { value: value.get() },
        Value::Text(value) => ProjectionValue::Text {
            value: value.clone(),
        },
        Value::Boolean(value) => ProjectionValue::Boolean { value: *value },
        Value::Reference(entity) => ProjectionValue::Reference {
            entity: entity.clone(),
        },
        Value::Formula(expression) => ProjectionValue::Formula {
            expression: expression.clone(),
        },
    }
}

/// Build a deterministic project with one independent formula per entity.
///
/// # Errors
///
/// Returns an error for zero entities or if the generated aggregate does not
/// satisfy the current workspace-engine contract.
pub fn synthetic_document(entity_count: usize) -> Result<Document, SpikeError> {
    if entity_count == 0 {
        return Err(SpikeError::EmptySyntheticProject);
    }
    if u32::try_from(entity_count).is_err() {
        return Err(SpikeError::SyntheticProjectTooLarge { entity_count });
    }

    let schema_id = SchemaId::from("synthetic-schema-id");
    let base_field = FieldId::from("synthetic-base-field-id");
    let multiplier_field = FieldId::from("synthetic-multiplier-field-id");
    let computed_field = FieldId::from("synthetic-computed-field-id");
    let label_field = FieldId::from("synthetic-label-field-id");
    let fields = synthetic_fields(
        &base_field,
        &multiplier_field,
        &computed_field,
        &label_field,
    );
    let schemas = BTreeMap::from([(
        schema_id.clone(),
        Schema {
            id: schema_id.clone(),
            key: SchemaKey::from("synthetic_records"),
            fields,
        },
    )]);

    let mut entities = BTreeMap::new();
    for index in 0..entity_count {
        let numeric_index = u32::try_from(index)
            .map_err(|_| SpikeError::SyntheticProjectTooLarge { entity_count })?;
        let id = EntityId::from(format!("synthetic-entity-{index:06}"));
        let key = EntityKey::from(format!("entity_{index:04}"));
        let values = BTreeMap::from([
            (
                base_field.clone(),
                Value::Number(
                    Number::new(f64::from(numeric_index) + 1.0)
                        .map_err(|_| SpikeError::InvalidSyntheticNumber)?,
                ),
            ),
            (
                multiplier_field.clone(),
                Value::Number(Number::new(2.0).map_err(|_| SpikeError::InvalidSyntheticNumber)?),
            ),
            (
                computed_field.clone(),
                Value::Formula(Expression::Multiply {
                    left: Box::new(Expression::Reference(FieldRef::new(
                        id.clone(),
                        base_field.clone(),
                    ))),
                    right: Box::new(Expression::Reference(FieldRef::new(
                        id.clone(),
                        multiplier_field.clone(),
                    ))),
                }),
            ),
            (label_field.clone(), Value::Text(format!("Record {index}"))),
        ]);
        entities.insert(
            id.clone(),
            Entity {
                id,
                key,
                schema: schema_id.clone(),
                fields: values,
            },
        );
    }

    let document = Document {
        id: DocumentId::from("synthetic-document-id"),
        title: format!("Issue 26 synthetic {entity_count}"),
        schemas,
        entities,
    };
    validate(&document)?;
    Ok(document)
}

fn synthetic_fields(
    base_field: &FieldId,
    multiplier_field: &FieldId,
    computed_field: &FieldId,
    label_field: &FieldId,
) -> BTreeMap<FieldId, FieldDefinition> {
    BTreeMap::from([
        (
            base_field.clone(),
            FieldDefinition {
                id: base_field.clone(),
                key: FieldKey::from("base"),
                field_type: FieldType::Number,
                required: true,
            },
        ),
        (
            multiplier_field.clone(),
            FieldDefinition {
                id: multiplier_field.clone(),
                key: FieldKey::from("multiplier"),
                field_type: FieldType::Number,
                required: true,
            },
        ),
        (
            computed_field.clone(),
            FieldDefinition {
                id: computed_field.clone(),
                key: FieldKey::from("computed"),
                field_type: FieldType::Number,
                required: true,
            },
        ),
        (
            label_field.clone(),
            FieldDefinition {
                id: label_field.clone(),
                key: FieldKey::from("label"),
                field_type: FieldType::Text,
                required: true,
            },
        ),
    ])
}
