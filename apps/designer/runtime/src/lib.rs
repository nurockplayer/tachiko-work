//! Private first-party Designer adapter over the Rust-authoritative workspace runtime.
//!
//! This crate is app-local composition code. Its DTOs and WASM ABI are
//! provisional delivery mechanics, not a public Semantic API or SDK contract.

use std::{
    collections::{BTreeMap, VecDeque},
    mem::{size_of, size_of_val},
};

use serde::{Deserialize, Serialize};
use tachiko_storage::{
    CanonicalRoProjectV1, FormatError, ROPROJ_V1_PATHS, decode_roproj_v1, encode_roproj_v1,
};
use tachiko_workspace_engine::{
    CalculationFailure, Document, FieldId, FieldRef, FieldType, IdGenerator, Number,
    SemanticIdKind, StarterTemplate, Value, WorkspaceError, analyze_field, create_document,
    formula_operations::FormulaCalculationOutcome,
    patch_lifecycle::{
        AuthorizationAction, AuthorizationDomainId, AuthorizationPolicyVersion, DocumentScopeId,
        Grant, GrantId, GrantRequirement, MutationClass, OperationFamily, PatchLifecycle,
        PatchLifecycleError, PolicyMeaningId, PrincipalId, PrincipalKind, ProposalId,
        ProposalRequest, ScopedSemanticSubject, SemanticApiContract, SemanticCommand,
        SemanticPatchBody, SemanticRevision, SemanticScope, TrustedInstant,
    },
    resident_session::{ResidentWorkspaceSession, TrustedPublicationTimeSource},
    validate,
};
use thiserror::Error;

#[cfg(target_arch = "wasm32")]
mod wasm;

const DEFAULT_COLLECTION: &str = "weapons";
const MAX_TABLE_FIELDS: usize = 32;
const MAX_TABLE_ROWS: usize = 32;
const MAX_FIELD_QUERY_TARGETS: usize = MAX_TABLE_FIELDS * MAX_TABLE_ROWS;
pub(crate) const MAX_WIRE_REQUEST_BYTES: usize = 65_536;
pub(crate) const MAX_PROJECT_TRANSFER_BYTES: usize = 64 * 1024 * 1024;
const DESIGNER_PRINCIPAL: &str = "designer-human";
const PROJECT_BUNDLE_MAGIC: &[u8; 8] = b"TWDPROJ1";

/// App-private requests accepted by the Designer runtime adapter.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum DesignerRequest {
    Bootstrap {
        occurrence_id: String,
    },
    QueryTable {
        collection: String,
    },
    QueryFields {
        expected_revision: String,
        fields: Vec<FieldTarget>,
    },
    EditNumber {
        expected_revision: String,
        target: FieldTarget,
        input: String,
    },
}

/// App-private responses returned by the Designer runtime adapter.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum DesignerResponse {
    Bootstrap(BootstrapProjection),
    Opened(Box<OpenedProjection>),
    Table(TableProjection),
    Fields(FieldBatchProjection),
    Published(PublicationProjection),
    ProjectExported(ProjectExportProjection),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct OpenedProjection {
    pub bootstrap: BootstrapProjection,
    pub table: TableProjection,
    pub control: ControlProjection,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ControlProjection {
    pub target: FieldTarget,
    pub value: f64,
    pub revision: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BootstrapProjection {
    pub title: String,
    pub revision: String,
    pub default_collection: String,
    pub collections: Vec<CollectionSummary>,
    pub control_field: FieldTarget,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CollectionSummary {
    pub id: String,
    pub key: String,
    pub entity_count: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TableProjection {
    pub revision: String,
    pub collection: CollectionSummary,
    pub columns: Vec<ColumnProjection>,
    pub rows: Vec<RowProjection>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ColumnProjection {
    pub id: String,
    pub key: String,
    pub field_type: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RowProjection {
    pub id: String,
    pub key: String,
    pub fields: Vec<FieldProjection>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FieldProjection {
    pub target: FieldTarget,
    pub address: String,
    pub stored: Option<StoredValueProjection>,
    pub formula: Option<FormulaProjection>,
    pub calculated: Option<CalculationProjection>,
    pub diagnostics: Vec<DiagnosticProjection>,
    pub editable_number: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StoredValueProjection {
    Number { value: f64 },
    Text { value: String },
    Boolean { value: bool },
    Reference { entity: String },
}

impl StoredValueProjection {
    #[must_use]
    pub const fn number(&self) -> Option<f64> {
        match self {
            Self::Number { value } => Some(*value),
            Self::Text { .. } | Self::Boolean { .. } | Self::Reference { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FormulaProjection {
    pub source: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CalculationProjection {
    Value { value: f64 },
    Failure { code: String, message: String },
    Unavailable,
}

impl CalculationProjection {
    #[must_use]
    pub const fn number(&self) -> Option<f64> {
        match self {
            Self::Value { value } => Some(*value),
            Self::Failure { .. } | Self::Unavailable => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DiagnosticProjection {
    pub code: String,
    pub message: String,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FailureProjection {
    pub code: String,
    pub message: String,
    pub current_revision: String,
    pub diagnostics: Vec<DiagnosticProjection>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DesignerWireReply {
    Ok { response: DesignerResponse },
    Error { error: FailureProjection },
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FieldTarget {
    pub entity: String,
    pub field: String,
}

impl From<&str> for FieldTarget {
    fn from(value: &str) -> Self {
        let (entity, field) = value
            .split_once('.')
            .expect("test and fixture field targets use entity.field spelling");
        Self {
            entity: entity.to_owned(),
            field: field.to_owned(),
        }
    }
}

impl FieldTarget {
    fn as_field_ref(&self) -> FieldRef {
        FieldRef::new(self.entity.clone(), self.field.clone())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FieldBatchProjection {
    pub revision: String,
    pub fields: Vec<FieldProjection>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PublicationProjection {
    pub base_revision: String,
    pub resulting_revision: String,
    pub entities: Vec<String>,
    pub fields: Vec<FieldTarget>,
    pub affected_calculations: Vec<FieldTarget>,
}

/// One exact-revision canonical project prepared for a host durability commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectExport {
    pub revision: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectExportProjection {
    pub revision: String,
    pub byte_length: usize,
}

#[derive(Debug, Error)]
pub enum DesignerError {
    #[error("Designer workspace operation failed: {0}")]
    Workspace(#[from] WorkspaceError),
    #[error("canonical project failed complete workspace validation: {source}")]
    InvalidProjectWorkspace {
        #[source]
        source: WorkspaceError,
    },
    #[error("canonical project admission failed: {0}")]
    Storage(#[from] FormatError),
    #[error("project transfer is invalid: {message}")]
    InvalidProjectTransfer { message: String },
    #[error(
        "project transfer exceeds the private {maximum}-byte host boundary (received {actual} bytes)"
    )]
    ProjectTransferTooLarge { actual: usize, maximum: usize },
    #[error("this canonical project is outside the bounded Moonfall Designer profile: {message}")]
    UnsupportedProject { message: String },
    #[error("the trusted host occurrence identity is invalid")]
    InvalidOccurrenceIdentity,
    #[error("collection '{collection}' is not available in this Designer slice")]
    MissingCollection { collection: String },
    #[error("collection '{collection}' exceeds the bounded table profile")]
    CollectionTooLarge { collection: String },
    #[error("field query requested {requested} targets; the bounded maximum is {maximum}")]
    FieldQueryTooLarge { requested: usize, maximum: usize },
    #[error("formula projection is unavailable for '{field}'")]
    MissingFormulaProjection { field: FieldRef },
    #[error("Designer lifecycle failed: {0}")]
    Lifecycle(#[from] PatchLifecycleError),
    #[error("'{input}' is not a finite Number")]
    InvalidNumberInput { input: String },
    #[error("field '{field}' is not a directly stored Number")]
    UnsupportedNumberEdit { field: FieldRef },
    #[error("requested revision '{requested}' is stale; current revision is '{current}'")]
    StaleQuery { requested: String, current: String },
    #[error("successful publication did not yield matching invalidation facts")]
    MissingInvalidation,
}

impl DesignerError {
    #[must_use]
    pub fn failure_projection(&self, current_revision: &str) -> FailureProjection {
        let (code, diagnostics) = match self {
            Self::Lifecycle(PatchLifecycleError::Stale) | Self::StaleQuery { .. } => {
                ("stale_revision", Vec::new())
            }
            Self::Lifecycle(PatchLifecycleError::ValidationFailed { report }) => (
                "validation_failed",
                report
                    .diagnostics()
                    .iter()
                    .map(diagnostic_projection)
                    .collect(),
            ),
            Self::Lifecycle(PatchLifecycleError::CommandRejected { .. }) => {
                ("edit_rejected", Vec::new())
            }
            Self::InvalidNumberInput { .. } => ("invalid_number", Vec::new()),
            Self::UnsupportedNumberEdit { .. } => ("unsupported_edit", Vec::new()),
            Self::Storage(_)
            | Self::InvalidProjectWorkspace { .. }
            | Self::InvalidProjectTransfer { .. } => ("invalid_project", Vec::new()),
            Self::ProjectTransferTooLarge { .. } => ("project_too_large", Vec::new()),
            Self::UnsupportedProject { .. } => ("unsupported_project", Vec::new()),
            Self::InvalidOccurrenceIdentity => ("invalid_occurrence", Vec::new()),
            Self::MissingCollection { .. } => ("missing_collection", Vec::new()),
            Self::CollectionTooLarge { .. } | Self::FieldQueryTooLarge { .. } => {
                ("query_too_large", Vec::new())
            }
            Self::Workspace(_)
            | Self::MissingFormulaProjection { .. }
            | Self::Lifecycle(_)
            | Self::MissingInvalidation => ("runtime_failure", Vec::new()),
        };
        FailureProjection {
            code: code.to_owned(),
            message: self.to_string(),
            current_revision: current_revision.to_owned(),
            diagnostics,
        }
    }
}

/// One Rust-authoritative occurrence composed for the first-party Web Designer.
pub struct DesignerRuntime {
    title: String,
    document_scope: DocumentScopeId,
    collections: Vec<CollectionSummary>,
    collection_specs: BTreeMap<String, CollectionSpec>,
    formula_sources: BTreeMap<FieldRef, String>,
    session: ResidentWorkspaceSession,
    lifecycle: PatchLifecycle,
    principal: PrincipalId,
    clock: DesignerClock,
    proposal_serial: u64,
}

#[derive(Clone)]
struct CollectionSpec {
    summary: CollectionSummary,
    columns: Vec<ColumnSpec>,
    entities: Vec<tachiko_workspace_engine::EntityId>,
}

#[derive(Clone)]
struct ColumnSpec {
    id: FieldId,
    key: String,
    field_type: FieldType,
}

impl DesignerRuntime {
    /// Create the bounded built-in Moonfall occurrence used by this first slice.
    ///
    /// # Errors
    ///
    /// Returns the shared workspace failure if the deterministic fixture does
    /// not satisfy current semantic and calculation authority.
    pub fn moonfall(occurrence_id: &str) -> Result<Self, DesignerError> {
        let mut generator = MoonfallIds::new();
        let document = create_document(
            StarterTemplate::GameBalance,
            "Moonfall Balance",
            &mut generator,
        )?;
        Self::from_document(document, occurrence_id)
    }

    /// Construct a fresh bounded Designer occurrence from an already-admitted
    /// semantic document. Storage admission remains outside this constructor.
    ///
    /// # Errors
    ///
    /// Returns an app-profile or shared workspace failure before any existing
    /// occurrence is replaced.
    pub fn from_document(document: Document, occurrence_id: &str) -> Result<Self, DesignerError> {
        validate(&document).map_err(|source| DesignerError::InvalidProjectWorkspace { source })?;
        let collection_specs = collection_specs(&document);
        let collections = collection_specs
            .values()
            .map(|collection| collection.summary.clone())
            .collect();
        let formula_sources = formula_sources(&document)?;
        let title = document.title.clone();
        let principal = PrincipalId::from(DESIGNER_PRINCIPAL);
        let document_scope = document_scope(occurrence_id, &document)?;
        let lifecycle = designer_lifecycle(&document_scope, &document, &principal)?;
        let session = ResidentWorkspaceSession::new(document_scope.clone(), document);
        let runtime = Self {
            title,
            document_scope,
            collections,
            collection_specs,
            formula_sources,
            session,
            lifecycle,
            principal,
            clock: DesignerClock::default(),
            proposal_serial: 0,
        };
        runtime.ensure_supported_project()?;
        Ok(runtime)
    }

    /// Execute one private adapter request without exposing the canonical document.
    ///
    /// # Errors
    ///
    /// Returns a typed adapter or workspace failure.
    pub fn handle(&mut self, request: DesignerRequest) -> Result<DesignerResponse, DesignerError> {
        match request {
            DesignerRequest::Bootstrap { .. } => {
                Ok(DesignerResponse::Bootstrap(self.bootstrap_projection()))
            }
            DesignerRequest::QueryTable { collection } => {
                Ok(DesignerResponse::Table(self.query_table(&collection)?))
            }
            DesignerRequest::QueryFields {
                expected_revision,
                fields,
            } => Ok(DesignerResponse::Fields(
                self.query_fields(&expected_revision, &fields)?,
            )),
            DesignerRequest::EditNumber {
                expected_revision,
                target,
                input,
            } => Ok(DesignerResponse::Published(self.edit_number(
                &expected_revision,
                &target,
                &input,
            )?)),
        }
    }

    fn bootstrap_projection(&self) -> BootstrapProjection {
        BootstrapProjection {
            title: self.title.clone(),
            revision: self.session.revision().as_str().to_owned(),
            default_collection: DEFAULT_COLLECTION.to_owned(),
            collections: self.collections.clone(),
            control_field: control_field(),
        }
    }

    fn ensure_supported_project(&self) -> Result<(), DesignerError> {
        for collection in &self.collections {
            self.query_table(&collection.key).map_err(|error| {
                DesignerError::UnsupportedProject {
                    message: error.to_string(),
                }
            })?;
        }
        let control = self
            .query_fields(self.current_revision(), &[control_field()])
            .map_err(|error| DesignerError::UnsupportedProject {
                message: error.to_string(),
            })?;
        if !matches!(
            control
                .fields
                .first()
                .and_then(|field| field.calculated.as_ref()),
            Some(CalculationProjection::Value { .. })
        ) {
            return Err(DesignerError::UnsupportedProject {
                message: "the required shop.upgrade_cost control formula is unavailable".to_owned(),
            });
        }
        Ok(())
    }

    /// Prepare the exact current resident snapshot as a canonical project.
    ///
    /// # Errors
    ///
    /// Returns a stale-revision, storage, or bounded-transfer failure without
    /// mutating the resident occurrence.
    pub fn export_project(&self, expected_revision: &str) -> Result<ProjectExport, DesignerError> {
        let current = self.current_revision();
        if current != expected_revision {
            return Err(DesignerError::StaleQuery {
                requested: expected_revision.to_owned(),
                current: current.to_owned(),
            });
        }
        let snapshot = self.session.export_snapshot();
        let tree = encode_roproj_v1(snapshot.document())?;
        let bytes = encode_project_bundle(&tree)?;
        Ok(ProjectExport {
            revision: snapshot.revision().as_str().to_owned(),
            bytes,
        })
    }

    fn query_table(&self, collection: &str) -> Result<TableProjection, DesignerError> {
        let spec = self.collection_specs.get(collection).ok_or_else(|| {
            DesignerError::MissingCollection {
                collection: collection.to_owned(),
            }
        })?;
        if spec.columns.len() > MAX_TABLE_FIELDS || spec.entities.len() > MAX_TABLE_ROWS {
            return Err(DesignerError::CollectionTooLarge {
                collection: collection.to_owned(),
            });
        }

        let entities = self.session.query_entities(&spec.entities)?;
        let targets = entities
            .value()
            .iter()
            .flat_map(|entity| {
                spec.columns
                    .iter()
                    .filter(|column| entity.fields.contains(&column.id))
                    .map(|column| FieldRef::new(entity.id.clone(), column.id.clone()))
            })
            .collect::<Vec<_>>();
        let field_query = self.session.query_fields(&targets)?;
        let fields = field_query
            .value()
            .iter()
            .map(|field| (field.field.clone(), self.project_field(field)))
            .collect::<BTreeMap<_, _>>();
        let rows = entities
            .value()
            .iter()
            .map(|entity| RowProjection {
                id: entity.id.to_string(),
                key: entity.key.to_string(),
                fields: spec
                    .columns
                    .iter()
                    .filter_map(|column| {
                        fields
                            .get(&FieldRef::new(entity.id.clone(), column.id.clone()))
                            .cloned()
                    })
                    .collect(),
            })
            .collect();
        Ok(TableProjection {
            revision: field_query.revision().as_str().to_owned(),
            collection: spec.summary.clone(),
            columns: spec
                .columns
                .iter()
                .map(|column| ColumnProjection {
                    id: column.id.to_string(),
                    key: column.key.clone(),
                    field_type: field_type_name(&column.field_type).to_owned(),
                })
                .collect(),
            rows,
        })
    }

    fn project_field(
        &self,
        field: &tachiko_workspace_engine::resident_session::ResidentFieldProjection,
    ) -> FieldProjection {
        let stored = field.stored_value.as_ref().map(stored_value_projection);
        let formula = field
            .formula_definition
            .as_ref()
            .map(|_| FormulaProjection {
                source: self
                    .formula_sources
                    .get(&field.field)
                    .cloned()
                    .unwrap_or_else(|| "Formula source unavailable".to_owned()),
            });
        FieldProjection {
            target: field_target(&field.field),
            address: field.presentation_address.to_string(),
            editable_number: matches!(field.stored_value, Some(Value::Number(_))),
            stored,
            formula,
            calculated: field.calculated_value.as_ref().map(calculation_projection),
            diagnostics: field
                .diagnostics
                .iter()
                .map(diagnostic_projection)
                .collect(),
        }
    }

    fn query_fields(
        &self,
        expected_revision: &str,
        fields: &[FieldTarget],
    ) -> Result<FieldBatchProjection, DesignerError> {
        if fields.len() > MAX_FIELD_QUERY_TARGETS {
            return Err(DesignerError::FieldQueryTooLarge {
                requested: fields.len(),
                maximum: MAX_FIELD_QUERY_TARGETS,
            });
        }
        let current = self.session.revision().as_str();
        if current != expected_revision {
            return Err(DesignerError::StaleQuery {
                requested: expected_revision.to_owned(),
                current: current.to_owned(),
            });
        }
        let requested = fields
            .iter()
            .map(FieldTarget::as_field_ref)
            .collect::<Vec<_>>();
        let query = self.session.query_fields(&requested)?;
        Ok(FieldBatchProjection {
            revision: query.revision().as_str().to_owned(),
            fields: query
                .value()
                .iter()
                .map(|field| self.project_field(field))
                .collect(),
        })
    }

    fn edit_number(
        &mut self,
        expected_revision: &str,
        target: &FieldTarget,
        input: &str,
    ) -> Result<PublicationProjection, DesignerError> {
        let field = target.as_field_ref();
        let current = self.session.query_fields(std::slice::from_ref(&field))?;
        if !matches!(current.value()[0].stored_value, Some(Value::Number(_))) {
            return Err(DesignerError::UnsupportedNumberEdit { field });
        }
        let parsed = input
            .parse::<f64>()
            .ok()
            .and_then(|value| Number::new(value).ok())
            .ok_or_else(|| DesignerError::InvalidNumberInput {
                input: input.to_owned(),
            })?;
        let snapshot = self.session.export_snapshot();
        self.proposal_serial = self.proposal_serial.saturating_add(1);
        let proposal_id = ProposalId::from(format!("designer-edit-{}", self.proposal_serial));
        let body = SemanticPatchBody::command(SemanticCommand::set_field_value(
            field,
            Value::Number(parsed),
        ));
        self.lifecycle.propose(
            snapshot.document_scope(),
            snapshot.document(),
            snapshot.revision(),
            ProposalRequest::new(
                proposal_id.clone(),
                SemanticRevision::from(expected_revision.to_owned()),
                body,
                self.principal.clone(),
            ),
            self.clock.tick(),
        )?;
        self.lifecycle.preview(
            snapshot.document_scope(),
            snapshot.document(),
            snapshot.revision(),
            &proposal_id,
            &self.principal,
            self.clock.tick(),
        )?;
        let execute_now = self.clock.tick();
        let (receipt, invalidation) = {
            let mut publication = self.session.publication_authority(&mut self.clock);
            let receipt = self.lifecycle.execute(
                &proposal_id,
                None,
                &self.principal,
                &mut publication,
                execute_now,
            )?;
            let invalidation = publication
                .projection_invalidation_for(
                    snapshot.document_scope(),
                    &receipt.base_revision,
                    &receipt.resulting_revision,
                )
                .ok_or(DesignerError::MissingInvalidation)?
                .clone();
            (receipt, invalidation)
        };
        Ok(PublicationProjection {
            base_revision: receipt.base_revision.as_str().to_owned(),
            resulting_revision: receipt.resulting_revision.as_str().to_owned(),
            entities: invalidation
                .entities
                .iter()
                .map(ToString::to_string)
                .collect(),
            fields: invalidation.fields.iter().map(field_target).collect(),
            affected_calculations: invalidation
                .affected_calculations
                .iter()
                .map(field_target)
                .collect(),
        })
    }

    fn current_revision(&self) -> &str {
        self.session.revision().as_str()
    }

    /// Return the trusted occurrence identity for adapter-level authority tests.
    #[must_use]
    pub fn occurrence_scope(&self) -> &str {
        self.document_scope.as_str()
    }
}

/// Fully admit a canonical project candidate before replacing the occurrence.
///
/// # Errors
///
/// Returns transfer, storage, or Designer-profile failures while leaving the
/// existing occurrence unchanged.
pub fn open_project(
    runtime: &mut Option<DesignerRuntime>,
    input: &[u8],
    occurrence_id: &str,
) -> Result<OpenedProjection, DesignerError> {
    let tree = decode_project_bundle(input)?;
    let document = decode_roproj_v1(&tree)?;
    let candidate = DesignerRuntime::from_document(document, occurrence_id)?;
    let bootstrap = candidate.bootstrap_projection();
    let table = candidate.query_table(&bootstrap.default_collection)?;
    let control_batch = candidate.query_fields(
        &bootstrap.revision,
        std::slice::from_ref(&bootstrap.control_field),
    )?;
    let control_value = control_batch
        .fields
        .first()
        .and_then(|field| field.calculated.as_ref())
        .and_then(CalculationProjection::number)
        .ok_or_else(|| DesignerError::UnsupportedProject {
            message: "the required shop.upgrade_cost control formula is unavailable".to_owned(),
        })?;
    let control = ControlProjection {
        target: bootstrap.control_field.clone(),
        value: control_value,
        revision: bootstrap.revision.clone(),
    };
    let opened = OpenedProjection {
        bootstrap,
        table,
        control,
    };
    *runtime = Some(candidate);
    Ok(opened)
}

/// Destroy the current semantic occurrence without touching durable host data.
pub fn close_project(runtime: &mut Option<DesignerRuntime>) {
    *runtime = None;
}

fn encode_project_bundle(tree: &CanonicalRoProjectV1) -> Result<Vec<u8>, DesignerError> {
    let total = tree.files().iter().try_fold(
        PROJECT_BUNDLE_MAGIC.len() + size_of::<u32>(),
        |total, file| {
            let path_length = u16::try_from(file.path().len()).map_err(|_| {
                DesignerError::InvalidProjectTransfer {
                    message: "a canonical path exceeds the private transfer profile".to_owned(),
                }
            })?;
            let byte_length = u32::try_from(file.bytes().len()).map_err(|_| {
                DesignerError::ProjectTransferTooLarge {
                    actual: file.bytes().len(),
                    maximum: MAX_PROJECT_TRANSFER_BYTES,
                }
            })?;
            Ok::<usize, DesignerError>(
                total
                    .saturating_add(size_of_val(&path_length))
                    .saturating_add(size_of_val(&byte_length))
                    .saturating_add(file.path().len())
                    .saturating_add(file.bytes().len()),
            )
        },
    )?;
    enforce_project_transfer_limit(total)?;
    let mut output = Vec::with_capacity(total);
    output.extend_from_slice(PROJECT_BUNDLE_MAGIC);
    output.extend_from_slice(
        &u32::try_from(tree.files().len())
            .expect("canonical project file count fits u32")
            .to_le_bytes(),
    );
    for file in tree.files() {
        output.extend_from_slice(
            &u16::try_from(file.path().len())
                .expect("canonical project paths fit u16")
                .to_le_bytes(),
        );
        output.extend_from_slice(
            &u32::try_from(file.bytes().len())
                .expect("bounded project file lengths fit u32")
                .to_le_bytes(),
        );
        output.extend_from_slice(file.path().as_bytes());
        output.extend_from_slice(file.bytes());
    }
    Ok(output)
}

fn decode_project_bundle(input: &[u8]) -> Result<CanonicalRoProjectV1, DesignerError> {
    enforce_project_transfer_limit(input.len())?;
    let mut cursor = ProjectBundleCursor::new(input);
    if cursor.take(PROJECT_BUNDLE_MAGIC.len())? != PROJECT_BUNDLE_MAGIC {
        return Err(DesignerError::InvalidProjectTransfer {
            message: "missing TWDPROJ1 transfer discriminator".to_owned(),
        });
    }
    let file_count = cursor.read_u32()? as usize;
    if file_count > 1_024 {
        return Err(DesignerError::InvalidProjectTransfer {
            message: format!("file count {file_count} exceeds the private transfer profile"),
        });
    }
    let mut files_by_path = BTreeMap::new();
    for _ in 0..file_count {
        let path_length = cursor.read_u16()? as usize;
        let byte_length = cursor.read_u32()? as usize;
        let path = std::str::from_utf8(cursor.take(path_length)?)
            .map_err(|_| DesignerError::InvalidProjectTransfer {
                message: "project path is not UTF-8".to_owned(),
            })?
            .to_owned();
        let bytes = cursor.take(byte_length)?.to_vec();
        if files_by_path.insert(path.clone(), bytes).is_some() {
            return Err(DesignerError::InvalidProjectTransfer {
                message: format!("duplicate project path '{path}'"),
            });
        }
    }
    if !cursor.is_finished() {
        return Err(DesignerError::InvalidProjectTransfer {
            message: "project transfer contains trailing bytes".to_owned(),
        });
    }
    let mut files = Vec::with_capacity(ROPROJ_V1_PATHS.len());
    for path in ROPROJ_V1_PATHS {
        let bytes =
            files_by_path
                .remove(path)
                .ok_or_else(|| DesignerError::InvalidProjectTransfer {
                    message: format!("project transfer is missing '{path}'"),
                })?;
        files.push((path.to_owned(), bytes));
    }
    if let Some(extra) = files_by_path.keys().next() {
        return Err(DesignerError::InvalidProjectTransfer {
            message: format!("project transfer contains unexpected path '{extra}'"),
        });
    }
    Ok(CanonicalRoProjectV1::try_from_files(files)?)
}

fn enforce_project_transfer_limit(actual: usize) -> Result<(), DesignerError> {
    if actual > MAX_PROJECT_TRANSFER_BYTES {
        return Err(DesignerError::ProjectTransferTooLarge {
            actual,
            maximum: MAX_PROJECT_TRANSFER_BYTES,
        });
    }
    Ok(())
}

struct ProjectBundleCursor<'input> {
    input: &'input [u8],
    offset: usize,
}

impl<'input> ProjectBundleCursor<'input> {
    const fn new(input: &'input [u8]) -> Self {
        Self { input, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'input [u8], DesignerError> {
        let end = self.offset.checked_add(length).ok_or_else(|| {
            DesignerError::InvalidProjectTransfer {
                message: "project transfer length overflowed".to_owned(),
            }
        })?;
        let bytes = self.input.get(self.offset..end).ok_or_else(|| {
            DesignerError::InvalidProjectTransfer {
                message: "project transfer ended before its declared lengths".to_owned(),
            }
        })?;
        self.offset = end;
        Ok(bytes)
    }

    fn read_u16(&mut self) -> Result<u16, DesignerError> {
        let bytes: [u8; 2] = self.take(size_of::<u16>())?.try_into().map_err(|_| {
            DesignerError::InvalidProjectTransfer {
                message: "invalid project path length".to_owned(),
            }
        })?;
        Ok(u16::from_le_bytes(bytes))
    }

    fn read_u32(&mut self) -> Result<u32, DesignerError> {
        let bytes: [u8; 4] = self.take(size_of::<u32>())?.try_into().map_err(|_| {
            DesignerError::InvalidProjectTransfer {
                message: "invalid project transfer length".to_owned(),
            }
        })?;
        Ok(u32::from_le_bytes(bytes))
    }

    const fn is_finished(&self) -> bool {
        self.offset == self.input.len()
    }
}

/// Process one complete private adapter message without exposing Rust layouts.
#[must_use]
pub fn process_wire_request(runtime: &mut Option<DesignerRuntime>, input: &[u8]) -> Vec<u8> {
    if input.len() > MAX_WIRE_REQUEST_BYTES {
        return request_too_large_reply(runtime.as_ref());
    }
    let reply = match serde_json::from_slice::<DesignerRequest>(input) {
        Ok(request) => {
            if runtime.is_none() {
                if let DesignerRequest::Bootstrap { occurrence_id } = &request {
                    match DesignerRuntime::moonfall(occurrence_id) {
                        Ok(created) => *runtime = Some(created),
                        Err(error) => {
                            return encode_reply(&DesignerWireReply::Error {
                                error: error.failure_projection("unavailable"),
                            });
                        }
                    }
                }
            }
            if let Some(runtime) = runtime.as_mut() {
                match runtime.handle(request) {
                    Ok(response) => DesignerWireReply::Ok { response },
                    Err(error) => DesignerWireReply::Error {
                        error: error.failure_projection(runtime.current_revision()),
                    },
                }
            } else {
                DesignerWireReply::Error {
                    error: FailureProjection {
                        code: "no_project_open".to_owned(),
                        message: "No Designer project is open.".to_owned(),
                        current_revision: "unavailable".to_owned(),
                        diagnostics: Vec::new(),
                    },
                }
            }
        }
        Err(error) => DesignerWireReply::Error {
            error: FailureProjection {
                code: "invalid_request".to_owned(),
                message: format!("The Designer request could not be decoded: {error}"),
                current_revision: runtime.as_ref().map_or_else(
                    || "unavailable".to_owned(),
                    |runtime| runtime.current_revision().to_owned(),
                ),
                diagnostics: Vec::new(),
            },
        },
    };
    encode_reply(&reply)
}

fn request_too_large_reply(runtime: Option<&DesignerRuntime>) -> Vec<u8> {
    encode_reply(&DesignerWireReply::Error {
        error: FailureProjection {
            code: "request_too_large".to_owned(),
            message: format!(
                "The Designer request exceeds the private {MAX_WIRE_REQUEST_BYTES}-byte bridge limit."
            ),
            current_revision: runtime.map_or_else(
                || "unavailable".to_owned(),
                |runtime| runtime.current_revision().to_owned(),
            ),
            diagnostics: Vec::new(),
        },
    })
}

fn document_scope(
    occurrence_id: &str,
    document: &Document,
) -> Result<DocumentScopeId, DesignerError> {
    if !is_canonical_uuid_v4(occurrence_id) {
        return Err(DesignerError::InvalidOccurrenceIdentity);
    }
    Ok(DocumentScopeId::from(format!(
        "designer-occurrence/{occurrence_id}/{}",
        document.id
    )))
}

fn is_canonical_uuid_v4(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes[8] == b'-'
        && bytes[13] == b'-'
        && bytes[18] == b'-'
        && bytes[23] == b'-'
        && bytes[14] == b'4'
        && matches!(bytes[19], b'8' | b'9' | b'a' | b'b')
        && bytes.iter().enumerate().all(|(index, byte)| {
            matches!(index, 8 | 13 | 18 | 23) || matches!(byte, b'0'..=b'9' | b'a'..=b'f')
        })
}

fn control_field() -> FieldTarget {
    FieldTarget {
        entity: "shop".to_owned(),
        field: "upgrade_cost".to_owned(),
    }
}

fn collection_specs(document: &Document) -> BTreeMap<String, CollectionSpec> {
    document
        .schemas
        .values()
        .map(|schema| {
            let entities = document
                .entities
                .values()
                .filter(|entity| entity.schema == schema.id)
                .map(|entity| entity.id.clone())
                .collect::<Vec<_>>();
            let summary = CollectionSummary {
                id: schema.id.to_string(),
                key: schema.key.to_string(),
                entity_count: entities.len(),
            };
            (
                schema.key.to_string(),
                CollectionSpec {
                    summary,
                    columns: schema
                        .fields
                        .values()
                        .map(|field| ColumnSpec {
                            id: field.id.clone(),
                            key: field.key.to_string(),
                            field_type: field.field_type.clone(),
                        })
                        .collect(),
                    entities,
                },
            )
        })
        .collect()
}

fn formula_sources(document: &Document) -> Result<BTreeMap<FieldRef, String>, DesignerError> {
    document
        .entities
        .values()
        .flat_map(|entity| {
            entity
                .fields
                .iter()
                .filter(|(_, value)| matches!(value, Value::Formula(_)))
                .map(|(field, _)| FieldRef::new(entity.id.clone(), field.clone()))
        })
        .map(|field| {
            let source = analyze_field(document, "moonfall-fixture", &field)?
                .formula_source
                .ok_or_else(|| DesignerError::MissingFormulaProjection {
                    field: field.clone(),
                })?;
            Ok((field, source))
        })
        .collect()
}

fn designer_lifecycle(
    document_scope: &DocumentScopeId,
    document: &Document,
    principal: &PrincipalId,
) -> Result<PatchLifecycle, DesignerError> {
    let authority = PrincipalId::from("designer-host-authority");
    let mut lifecycle = PatchLifecycle::new(
        AuthorizationDomainId::from("designer-local-domain"),
        document_scope.clone(),
        document.id.clone(),
        SemanticApiContract::from("designer-internal-semantic-api"),
        AuthorizationPolicyVersion::from("designer-policy-v1"),
        PolicyMeaningId::from("designer-policy-v1-meaning"),
    );
    lifecycle.register_principal(authority.clone(), PrincipalKind::Human)?;
    lifecycle.register_principal(principal.clone(), PrincipalKind::Human)?;
    let scope = ScopedSemanticSubject::new(
        document_scope.clone(),
        document.id.clone(),
        SemanticScope::Document,
    );
    lifecycle.provision_grant(Grant::new(
        GrantId::from("designer-number-edit"),
        authority,
        principal.clone(),
        vec![
            GrantRequirement::query(OperationFamily::SetFieldValue, scope.clone()),
            GrantRequirement::mutation(
                AuthorizationAction::Propose,
                OperationFamily::SetFieldValue,
                MutationClass::Value,
                scope.clone(),
            )?,
            GrantRequirement::mutation(
                AuthorizationAction::Execute,
                OperationFamily::SetFieldValue,
                MutationClass::Value,
                scope,
            )?,
        ],
        None,
    ))?;
    Ok(lifecycle)
}

pub(crate) fn encode_reply(reply: &DesignerWireReply) -> Vec<u8> {
    serde_json::to_vec(reply).unwrap_or_else(|_| {
        br#"{"status":"error","error":{"code":"runtime_failure","message":"The Designer reply could not be encoded.","current_revision":"unavailable","diagnostics":[]}}"#
            .to_vec()
    })
}

#[derive(Default)]
struct DesignerClock {
    current: u64,
}

impl DesignerClock {
    fn tick(&mut self) -> TrustedInstant {
        self.current = self.current.saturating_add(1);
        TrustedInstant::new(self.current)
    }
}

impl TrustedPublicationTimeSource for DesignerClock {
    fn now(&mut self) -> TrustedInstant {
        self.tick()
    }
}

fn field_target(field: &FieldRef) -> FieldTarget {
    FieldTarget {
        entity: field.entity.to_string(),
        field: field.field.to_string(),
    }
}

fn diagnostic_projection(
    diagnostic: &tachiko_workspace_engine::Diagnostic,
) -> DiagnosticProjection {
    DiagnosticProjection {
        code: diagnostic.code.as_str().to_owned(),
        message: diagnostic.message.clone(),
        path: diagnostic.path.to_string(),
    }
}

fn stored_value_projection(value: &Value) -> StoredValueProjection {
    match value {
        Value::Number(value) => StoredValueProjection::Number { value: value.get() },
        Value::Text(value) => StoredValueProjection::Text {
            value: value.clone(),
        },
        Value::Boolean(value) => StoredValueProjection::Boolean { value: *value },
        Value::Reference(entity) => StoredValueProjection::Reference {
            entity: entity.to_string(),
        },
        Value::Formula(_) => unreachable!("formula definitions are projected separately"),
    }
}

fn calculation_projection(outcome: &FormulaCalculationOutcome) -> CalculationProjection {
    match outcome {
        FormulaCalculationOutcome::Value(value) => {
            CalculationProjection::Value { value: value.get() }
        }
        FormulaCalculationOutcome::Failure(failure) => CalculationProjection::Failure {
            code: calculation_failure_code(failure).to_owned(),
            message: calculation_failure_message(failure).to_owned(),
        },
        FormulaCalculationOutcome::Unavailable => CalculationProjection::Unavailable,
    }
}

fn calculation_failure_code(failure: &CalculationFailure) -> &'static str {
    match failure {
        CalculationFailure::InvalidExpression { .. } => "invalid_expression",
        CalculationFailure::InvalidReferences { .. } => "invalid_references",
        CalculationFailure::Cycle { .. } => "cycle",
        CalculationFailure::FailedDependencies { .. } => "failed_dependencies",
        CalculationFailure::DivisionByZero => "division_by_zero",
        CalculationFailure::NonFiniteResult => "non_finite_result",
    }
}

fn calculation_failure_message(failure: &CalculationFailure) -> &'static str {
    match failure {
        CalculationFailure::InvalidExpression { .. } => "The formula structure is invalid.",
        CalculationFailure::InvalidReferences { .. } => {
            "The formula references an unavailable value."
        }
        CalculationFailure::Cycle { .. } => "The formula is part of a dependency cycle.",
        CalculationFailure::FailedDependencies { .. } => {
            "A formula dependency could not be calculated."
        }
        CalculationFailure::DivisionByZero => "The formula divides by zero.",
        CalculationFailure::NonFiniteResult => "The formula result is not a finite number.",
    }
}

const fn field_type_name(field_type: &FieldType) -> &'static str {
    match field_type {
        FieldType::Number => "number",
        FieldType::Text => "text",
        FieldType::Boolean => "boolean",
        FieldType::Reference { .. } => "reference",
    }
}

struct MoonfallIds {
    document: VecDeque<String>,
    schemas: VecDeque<String>,
    fields: VecDeque<String>,
    entities: VecDeque<String>,
}

impl MoonfallIds {
    fn new() -> Self {
        Self {
            document: VecDeque::from(["moonfall".to_owned()]),
            schemas: ["characters", "economy", "items", "weapons"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            fields: [
                "level",
                "name",
                "weapon",
                "currency",
                "gold_per_match",
                "matches_for_sword",
                "upgrade_cost",
                "category",
                "grants_weapon",
                "name",
                "price",
                "attack_interval",
                "damage",
                "dps",
                "name",
                "price",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            entities: ["alric", "iron_sword", "shop", "tempered_blade"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }
    }
}

impl IdGenerator for MoonfallIds {
    fn generate(&mut self, kind: SemanticIdKind) -> String {
        match kind {
            SemanticIdKind::Document => self.document.pop_front(),
            SemanticIdKind::Schema => self.schemas.pop_front(),
            SemanticIdKind::Field => self.fields.pop_front(),
            SemanticIdKind::Entity => self.entities.pop_front(),
        }
        .expect("Moonfall fixture IDs cover every generated semantic subject")
    }
}
