//! Private first-party Designer adapter over the Rust-authoritative workspace runtime.
//!
//! This crate is app-local composition code. Its DTOs and WASM ABI are
//! provisional delivery mechanics, not a public Semantic API or SDK contract.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    mem::{size_of, size_of_val},
};

use serde::{Deserialize, Serialize};
use tachiko_storage::{
    CanonicalRoProjectAdmissionError, CanonicalRoProjectV1, FormatError, ROPROJ_V1_PATHS,
    encode_roproj_v1, from_bytes, to_canonical_string,
};
use tachiko_workspace_engine::{
    CalculationFailure, Date, Document, Entity, EntityId, EntityKey, Expression, FieldDefinition,
    FieldId, FieldKey, FieldRef, FieldType, IdGenerator, Number, Schema, SchemaId, SchemaKey,
    SemanticIdKind, StarterTemplate, Value, WorkspaceError, analyze_field, create_document,
    formula_operations::{FormulaCalculationOutcome, FormulaUpdateRequest},
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

pub mod interop_adapter;
mod interop_document;
mod interop_number_format;
use interop_document::PendingCleanup;
pub use interop_document::{
    CleanupChange, CleanupOperation, CleanupPreview, ImportColumnSpec, ImportFieldType,
    ImportSelection, ImportedProjection, InteropMetadata, NativeTrackerExportPresentation,
    NativeTrackerExportRow, SpreadsheetExportProjection, import_workbook, inspect_imported_project,
    validate_import_metadata,
};

#[cfg(target_arch = "wasm32")]
mod wasm;

const MAX_COLLECTIONS: usize = 32;
const MAX_TABLE_FIELDS: usize = 32;
const MAX_TABLE_ROWS: usize = 128;
const MAX_TOTAL_ENTITIES: usize = 1024;
const MAX_FIELD_QUERY_TARGETS: usize = 1024;
const MAX_FORMULAS: usize = 32;
const MAX_FORMULA_PROFILE_NODES: usize = 256;
const MAX_PROFILE_STRING_BYTES: usize = 4_096;
const MAX_PROJECTION_BYTES: usize = 65_536;
// The stock Tracker's 128 fixed scalar rows carry repeated field projections
// and may contain one bounded collection of Text. This is a private delivery
// profile, derived from the existing two 64 KiB input/profile budgets; generic
// tables, field batches, and source admission remain at 64 KiB.
const MAX_NATIVE_TRACKER_PROJECTION_BYTES: usize = 4 * MAX_PROJECTION_BYTES;
const MAX_WIDTH_FINITE_JSON_NUMBER: f64 = -f64::MIN_POSITIVE;
pub(crate) const MAX_WIRE_REQUEST_BYTES: usize = 65_536;
pub(crate) const MAX_PROJECT_TRANSFER_BYTES: usize = 64 * 1024 * 1024;
const DESIGNER_PRINCIPAL: &str = "designer-human";
const PREFLIGHT_OCCURRENCE: &str = "00000000-0000-4000-8000-000000000000";
/// Private record discriminator for frozen canonical `.roproj/v1` bundles.
const PROJECT_BUNDLE_V1_MAGIC: &[u8; 8] = b"TWDPROJ1";
/// Private record discriminator for direct-ro/v2 project records.
///
/// This is an app-host envelope, not a `.roproj/v2` or public storage format.
const PROJECT_RECORD_V2_MAGIC: &[u8; 8] = b"TWDPROJ2";
const MOONFALL_BOOLEAN_FIXTURE_COLLECTION: &str = "weapons";

/// App-private requests accepted by the Designer runtime adapter.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum DesignerRequest {
    Bootstrap {
        occurrence_id: String,
    },
    NewTracker {
        occurrence_id: String,
    },
    NewBudget {
        occurrence_id: String,
    },
    EditCells {
        expected_revision: String,
        edits: Vec<CellEdit>,
    },
    PasteCells {
        expected_revision: String,
        collection: String,
        start_entity: Option<String>,
        start_field: String,
        rows: Vec<Vec<String>>,
    },
    AppendRow {
        expected_revision: String,
        collection: String,
    },
    RemoveRows {
        expected_revision: String,
        entities: Vec<String>,
    },
    Undo {
        expected_revision: String,
    },
    Redo {
        expected_revision: String,
    },
    QueryTable {
        collection: String,
    },
    QueryFields {
        expected_revision: String,
        fields: Vec<FieldTarget>,
    },
    EditScalar {
        expected_revision: String,
        target: FieldTarget,
        input: ScalarEditInput,
    },
    PreviewCleanup {
        expected_revision: String,
        operation: CleanupOperation,
    },
    CommitCleanup {
        expected_revision: String,
        preview_id: String,
    },
    CopyFormula {
        expected_revision: String,
        source: FieldTarget,
        destinations: Vec<FieldTarget>,
        fixed_references: Vec<FieldTarget>,
        relative_rows: bool,
        relative_columns: bool,
    },
    FormulaUpdate {
        expected_revision: String,
        target: FieldTarget,
        source: String,
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
    CleanupPreview(CleanupPreview),
    ImportPreview(Box<interop_adapter::SourceWorkbook>),
    Imported(Box<ImportedProjection>),
    SpreadsheetExported(SpreadsheetExportProjection),
    ProjectExported(ProjectExportProjection),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct OpenedProjection {
    pub bootstrap: BootstrapProjection,
    pub table: TableProjection,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BootstrapProjection {
    pub title: String,
    pub revision: String,
    pub default_collection: String,
    pub collections: Vec<CollectionSummary>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracker_profile: Option<bool>,
    pub collection: CollectionSummary,
    pub columns: Vec<ColumnProjection>,
    pub rows: Vec<RowProjection>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CellEdit {
    pub target: FieldTarget,
    pub input: ScalarEditInput,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ColumnProjection {
    pub id: String,
    pub key: String,
    pub field_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dropdown_options: Option<Vec<String>>,
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
    pub editable_scalar: Option<ScalarKind>,
}

/// Rust-authoritative directly stored scalar kinds supported by this private table path.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalarKind {
    Number,
    Text,
    Boolean,
    Date,
}

/// Private typed input for one directly stored scalar edit.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScalarEditInput {
    Number { input: String },
    Text { value: String },
    Boolean { value: bool },
    Date { value: String },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StoredValueProjection {
    Number { value: f64 },
    Text { value: String },
    Boolean { value: bool },
    Date { value: Date },
    Reference { entity: String },
}

impl StoredValueProjection {
    #[must_use]
    pub const fn number(&self) -> Option<f64> {
        match self {
            Self::Number { value } => Some(*value),
            Self::Text { .. }
            | Self::Boolean { .. }
            | Self::Date { .. }
            | Self::Reference { .. } => None,
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
    #[error("this canonical project is outside the bounded Designer profile: {message}")]
    UnsupportedProject { message: String },
    #[error("the trusted host occurrence identity is invalid")]
    InvalidOccurrenceIdentity,
    #[error("collection '{collection}' is not available in this Designer slice")]
    MissingCollection { collection: String },
    #[error("collection '{collection}' exceeds the bounded table profile")]
    CollectionTooLarge { collection: String },
    #[error("field query requested {requested} targets; the bounded maximum is {maximum}")]
    FieldQueryTooLarge { requested: usize, maximum: usize },
    #[error("projection is {actual} bytes; the bounded maximum is {maximum}")]
    ProjectionTooLarge { actual: usize, maximum: usize },
    #[error("formula projection is unavailable for '{field}'")]
    MissingFormulaProjection { field: FieldRef },
    #[error("Designer lifecycle failed: {0}")]
    Lifecycle(#[from] PatchLifecycleError),
    #[error("'{input}' is not a finite Number")]
    InvalidNumberInput { input: String },
    #[error("'{input}' is not a valid Date (expected YYYY-MM-DD)")]
    InvalidDateInput { input: String },
    #[error("field '{field}' is not editable through this directly stored scalar path")]
    UnsupportedScalarEdit { field: FieldRef },
    #[error("requested revision '{requested}' is stale; current revision is '{current}'")]
    StaleQuery { requested: String, current: String },
    #[error("successful publication did not yield matching invalidation facts")]
    MissingInvalidation,
    #[error("tracker operation rejected: {message}")]
    InvalidTrackerOperation { message: String },
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
            Self::Lifecycle(PatchLifecycleError::NoChange) => ("no_change", Vec::new()),
            Self::Lifecycle(PatchLifecycleError::CommandRejected { .. }) => {
                ("edit_rejected", Vec::new())
            }
            Self::InvalidTrackerOperation { .. } => ("invalid_tracker_operation", Vec::new()),
            Self::InvalidNumberInput { .. } => ("invalid_number", Vec::new()),
            Self::InvalidDateInput { .. } => ("invalid_date", Vec::new()),
            Self::UnsupportedScalarEdit { .. } => ("unsupported_edit", Vec::new()),
            Self::Storage(_)
            | Self::InvalidProjectWorkspace { .. }
            | Self::InvalidProjectTransfer { .. } => ("invalid_project", Vec::new()),
            Self::ProjectTransferTooLarge { .. } => ("project_too_large", Vec::new()),
            Self::UnsupportedProject { .. } => ("unsupported_project", Vec::new()),
            Self::InvalidOccurrenceIdentity => ("invalid_occurrence", Vec::new()),
            Self::MissingCollection { .. } => ("missing_collection", Vec::new()),
            Self::CollectionTooLarge { .. }
            | Self::FieldQueryTooLarge { .. }
            | Self::ProjectionTooLarge { .. } => ("query_too_large", Vec::new()),
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
    default_collection: String,
    collections: Vec<CollectionSummary>,
    collection_specs: BTreeMap<String, CollectionSpec>,
    formula_sources: BTreeMap<FieldRef, String>,
    session: ResidentWorkspaceSession,
    lifecycle: PatchLifecycle,
    principal: PrincipalId,
    clock: DesignerClock,
    proposal_serial: u64,
    row_serial: usize,
    row_namespace: String,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    pending_cleanup: Option<PendingCleanup>,
}

#[derive(Clone)]
struct HistoryEntry {
    forward: Vec<SemanticCommand>,
    inverse: Vec<SemanticCommand>,
}

#[derive(Clone)]
struct CollectionSpec {
    direct_tracker_rows: bool,
    summary: CollectionSummary,
    columns: Vec<ColumnSpec>,
    entities: Vec<tachiko_workspace_engine::EntityId>,
}

#[derive(Clone)]
struct ColumnSpec {
    id: FieldId,
    key: String,
    field_type: FieldType,
    required: bool,
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
        let mut document = create_document(
            StarterTemplate::GameBalance,
            "Moonfall Balance",
            &mut generator,
        )?;
        add_moonfall_boolean_fixture(&mut document)?;
        Self::from_document(document, occurrence_id)
    }

    /// Construct a fresh bounded Designer occurrence from an already-admitted
    /// semantic document. Storage admission remains outside this constructor.
    /// The trusted host must supply a fresh UUID for every new occurrence,
    /// including reopening the same project after close or process restart.
    ///
    /// # Errors
    ///
    /// Returns an app-profile or shared workspace failure before any existing
    /// occurrence is replaced.
    pub fn from_document(document: Document, occurrence_id: &str) -> Result<Self, DesignerError> {
        ensure_cheap_document_profile(&document)?;
        validate(&document).map_err(|source| DesignerError::InvalidProjectWorkspace { source })?;
        let collection_specs = collection_specs(&document);
        let default_collection = select_default_collection(&collection_specs)?;
        let collections = collection_specs
            .values()
            .map(|collection| collection.summary.clone())
            .collect::<Vec<_>>();
        let title = document.title.clone();
        let document_scope = document_scope(occurrence_id, &document)?;
        ensure_static_profile(&title, &default_collection, &collections, &collection_specs)?;
        let formula_sources = formula_sources(&document)?;
        let principal = PrincipalId::from(DESIGNER_PRINCIPAL);
        let lifecycle = designer_lifecycle(&document_scope, &document, &principal)?;
        let row_serial = document
            .entities
            .keys()
            .filter_map(|id| {
                id.as_str()
                    .strip_prefix("tracker_row_")
                    .and_then(|suffix| suffix.split('_').next()?.parse::<usize>().ok())
            })
            .max()
            .unwrap_or(0);
        let session = ResidentWorkspaceSession::new(document_scope.clone(), document);
        let runtime = Self {
            title,
            document_scope,
            default_collection,
            collections,
            collection_specs,
            formula_sources,
            session,
            lifecycle,
            principal,
            clock: DesignerClock::default(),
            proposal_serial: 0,
            row_serial,
            row_namespace: occurrence_id.to_owned(),
            undo: Vec::new(),
            redo: Vec::new(),
            pending_cleanup: None,
        };
        runtime.ensure_supported_project()?;
        Ok(runtime)
    }

    /// Execute one private adapter request without exposing the canonical document.
    ///
    /// # Errors
    ///
    /// Returns a typed adapter or workspace failure.
    #[allow(clippy::too_many_lines)] // Exhaustive private request dispatch.
    pub fn handle(&mut self, request: DesignerRequest) -> Result<DesignerResponse, DesignerError> {
        match request {
            DesignerRequest::NewTracker { occurrence_id } => {
                let candidate = Self::tracker(&occurrence_id)?;
                let opened = OpenedProjection {
                    bootstrap: candidate.bootstrap_projection(),
                    table: candidate.query_table("tracker")?,
                };
                ensure_opened_projection_size(&opened)?;
                *self = candidate;
                Ok(DesignerResponse::Opened(Box::new(opened)))
            }
            DesignerRequest::NewBudget { occurrence_id } => {
                let candidate = Self::budget(&occurrence_id)?;
                let opened = OpenedProjection {
                    bootstrap: candidate.bootstrap_projection(),
                    table: candidate.query_table(&candidate.default_collection)?,
                };
                ensure_opened_projection_size(&opened)?;
                *self = candidate;
                Ok(DesignerResponse::Opened(Box::new(opened)))
            }
            DesignerRequest::EditCells {
                expected_revision,
                edits,
            } => Ok(DesignerResponse::Published(
                self.edit_cells(&expected_revision, &edits)?,
            )),
            DesignerRequest::PasteCells {
                expected_revision,
                collection,
                start_entity,
                start_field,
                rows,
            } => Ok(DesignerResponse::Published(self.paste_cells(
                &expected_revision,
                &collection,
                start_entity.as_deref(),
                &start_field,
                &rows,
            )?)),
            DesignerRequest::AppendRow {
                expected_revision,
                collection,
            } => Ok(DesignerResponse::Published(
                self.append_row(&expected_revision, &collection)?,
            )),
            DesignerRequest::RemoveRows {
                expected_revision,
                entities,
            } => Ok(DesignerResponse::Published(
                self.remove_rows(&expected_revision, &entities)?,
            )),
            DesignerRequest::Undo { expected_revision } => Ok(DesignerResponse::Published(
                self.history_edit(&expected_revision, false)?,
            )),
            DesignerRequest::Redo { expected_revision } => Ok(DesignerResponse::Published(
                self.history_edit(&expected_revision, true)?,
            )),
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
            DesignerRequest::EditScalar {
                expected_revision,
                target,
                input,
            } => Ok(DesignerResponse::Published(self.edit_scalar(
                &expected_revision,
                &target,
                &input,
            )?)),
            DesignerRequest::PreviewCleanup {
                expected_revision,
                operation,
            } => Ok(DesignerResponse::CleanupPreview(
                self.preview_cleanup(&expected_revision, &operation)?,
            )),
            DesignerRequest::CommitCleanup {
                expected_revision,
                preview_id,
            } => Ok(DesignerResponse::Published(
                self.commit_cleanup(&expected_revision, &preview_id)?,
            )),
            DesignerRequest::CopyFormula {
                expected_revision,
                source,
                destinations,
                fixed_references,
                relative_rows,
                relative_columns,
            } => Ok(DesignerResponse::Published(self.copy_formula(
                &expected_revision,
                &source,
                &destinations,
                &fixed_references,
                relative_rows,
                relative_columns,
            )?)),
            DesignerRequest::FormulaUpdate {
                expected_revision,
                target,
                source,
            } => Ok(DesignerResponse::Published(self.update_formula(
                &expected_revision,
                &target,
                &source,
            )?)),
        }
    }

    fn bootstrap_projection(&self) -> BootstrapProjection {
        BootstrapProjection {
            title: self.title.clone(),
            revision: self.session.revision().as_str().to_owned(),
            default_collection: self.default_collection.clone(),
            collections: self.collections.clone(),
        }
    }

    fn ensure_supported_project(&self) -> Result<(), DesignerError> {
        let mut post_edit_fields = BTreeSet::new();
        for collection in &self.collections {
            let table = self.query_table(&collection.key).map_err(|error| {
                DesignerError::UnsupportedProject {
                    message: error.to_string(),
                }
            })?;
            // Stock Tracker operations refresh the exact bounded table after a
            // publication. Its fixed scalar columns have no formulas, so the
            // generic all-editable-fields refresh is neither reachable nor a
            // valid capacity model for this profile.
            if table.tracker_profile == Some(true) {
                continue;
            }
            post_edit_fields.extend(
                table
                    .rows
                    .iter()
                    .flat_map(|row| &row.fields)
                    .filter(|field| field.editable_scalar.is_some() || field.formula.is_some())
                    .map(|field| field.target.clone()),
            );
        }
        let mut post_edit_refresh = self
            .query_fields(
                self.current_revision(),
                &post_edit_fields.into_iter().collect::<Vec<_>>(),
            )
            .map_err(|error| DesignerError::UnsupportedProject {
                message: format!("the worst-case post-edit refresh is not bounded: {error}"),
            })?;
        "resident/18446744073709551615".clone_into(&mut post_edit_refresh.revision);
        for field in &mut post_edit_refresh.fields {
            if let Some(StoredValueProjection::Number { value }) = &mut field.stored {
                *value = MAX_WIDTH_FINITE_JSON_NUMBER;
            }
            if let Some(CalculationProjection::Value { value }) = &mut field.calculated {
                *value = MAX_WIDTH_FINITE_JSON_NUMBER;
            }
        }
        ensure_projection_size(&post_edit_refresh).map_err(|error| {
            DesignerError::UnsupportedProject {
                message: format!("the worst-case post-edit refresh is not bounded: {error}"),
            }
        })?;
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
        let bytes = if document_contains_date(snapshot.document()) {
            encode_project_record_v2(snapshot.document())?
        } else {
            let tree = encode_roproj_v1(snapshot.document())?;
            encode_project_bundle_v1(&tree)?
        };
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
        let projection = TableProjection {
            tracker_profile: is_tracker_spec(spec).then_some(true),
            revision: field_query.revision().as_str().to_owned(),
            collection: spec.summary.clone(),
            columns: spec
                .columns
                .iter()
                .map(|column| ColumnProjection {
                    id: column.id.to_string(),
                    key: column.key.clone(),
                    field_type: field_type_name(&column.field_type).to_owned(),
                    dropdown_options: (column.field_type == FieldType::Boolean)
                        .then(|| vec!["true".to_owned(), "false".to_owned()]),
                })
                .collect(),
            rows,
        };
        ensure_table_projection_size(&projection)?;
        Ok(projection)
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
            editable_scalar: scalar_kind(field.stored_value.as_ref()),
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
        let projection = FieldBatchProjection {
            revision: query.revision().as_str().to_owned(),
            fields: query
                .value()
                .iter()
                .map(|field| self.project_field(field))
                .collect(),
        };
        ensure_projection_size(&projection)?;
        Ok(projection)
    }

    fn edit_scalar(
        &mut self,
        expected_revision: &str,
        target: &FieldTarget,
        input: &ScalarEditInput,
    ) -> Result<PublicationProjection, DesignerError> {
        let publication = self.edit_cells(
            expected_revision,
            &[CellEdit {
                target: target.clone(),
                input: input.clone(),
            }],
        )?;
        // Generic publications are not represented in the Tracker action history.
        // Invalidate both semantic directions only after accepted publication.
        self.undo.clear();
        self.redo.clear();
        Ok(publication)
    }

    // The trusted host supplies a fresh UUID per occurrence. Callers compare
    // the complete opaque token; its spelling is not a semantic or security API.
    fn next_proposal_id(&mut self) -> Result<ProposalId, DesignerError> {
        let serial = self
            .proposal_serial
            .checked_add(1)
            .ok_or_else(|| tracker_error("proposal identity counter exhausted"))?;
        self.proposal_serial = serial;
        Ok(ProposalId::from(format!(
            "designer-proposal/{}/{serial}",
            self.row_namespace
        )))
    }

    fn update_formula(
        &mut self,
        expected_revision: &str,
        target: &FieldTarget,
        source: &str,
    ) -> Result<PublicationProjection, DesignerError> {
        let snapshot = self.session.export_snapshot();
        // This app-private human request owns its complete lifecycle. Keep
        // admitted-but-unpublishable candidates and finished proposal evidence
        // request-local rather than retaining them in the resident session.
        let mut lifecycle = designer_lifecycle(
            snapshot.document_scope(),
            snapshot.document(),
            &self.principal,
        )?;
        let proposal_id = self.next_proposal_id()?;
        lifecycle.propose_formula_update(
            snapshot.document_scope(),
            snapshot.document(),
            snapshot.revision(),
            FormulaUpdateRequest::new(
                proposal_id.clone(),
                SemanticRevision::from(expected_revision.to_owned()),
                target.as_field_ref(),
                source,
                self.principal.clone(),
            ),
            self.clock.tick(),
        )?;
        let preview = lifecycle.preview(
            snapshot.document_scope(),
            snapshot.document(),
            snapshot.revision(),
            &proposal_id,
            &self.principal,
            self.clock.tick(),
        )?;
        // Use the admitted bound command from this exact authorized preview.
        // The full open-time profile must pass before publication, including
        // formula count, aggregate strings and every bounded projection.
        let SemanticPatchBody::Command(SemanticCommand::FormulaUpdate(command)) =
            preview.proposal.exact_change().body()
        else {
            return Err(tracker_error(
                "formula proposal did not contain one admitted formula command",
            ));
        };
        let mut candidate = snapshot.document().clone();
        let entity = candidate
            .entities
            .get_mut(&command.target().entity)
            .ok_or_else(|| tracker_error("admitted formula target is unavailable"))?;
        entity.fields.insert(
            command.target().field.clone(),
            Value::Formula(command.expression().clone()),
        );
        Self::from_document(candidate, PREFLIGHT_OCCURRENCE)?;
        let execute_now = self.clock.tick();
        let (receipt, invalidation) = {
            let mut publication = self.session.publication_authority(&mut self.clock);
            let receipt = lifecycle.execute(
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
        self.refresh_structure();
        self.formula_sources = formula_sources(self.session.export_snapshot().document())?;
        // Formula publication is generic semantic publication, not Tracker history.
        self.undo.clear();
        self.redo.clear();
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

    /// Resolve private copy gestures once against the exact canonical snapshot.
    /// View order and display names never participate in formula meaning.
    fn copy_formula(
        &mut self,
        expected: &str,
        source: &FieldTarget,
        destinations: &[FieldTarget],
        fixed_references: &[FieldTarget],
        relative_rows: bool,
        relative_columns: bool,
    ) -> Result<PublicationProjection, DesignerError> {
        self.check_revision(expected)?;
        if destinations.is_empty()
            || destinations.len() > MAX_FIELD_QUERY_TARGETS
            || fixed_references.len() > MAX_FIELD_QUERY_TARGETS
        {
            return Err(tracker_error("formula copy range is empty or too large"));
        }
        let snapshot = self.session.export_snapshot();
        let document = snapshot.document();
        let source = source.as_field_ref();
        let entity = document
            .entities
            .get(&source.entity)
            .ok_or_else(|| tracker_error("formula source entity is unavailable"))?;
        let Some(Value::Formula(expression)) = entity.fields.get(&source.field) else {
            return Err(tracker_error("formula copy requires a formula source"));
        };
        let spec = self
            .collection_specs
            .values()
            .find(|spec| spec.entities.contains(&source.entity))
            .ok_or_else(|| tracker_error("formula source collection is unavailable"))?;
        let (source_row, source_column) = copy_position(spec, &source)?;
        let fixed = fixed_references
            .iter()
            .map(FieldTarget::as_field_ref)
            .collect::<BTreeSet<_>>();
        let mut references = BTreeSet::new();
        let mut reference_probe = expression.clone();
        map_copy_references(&mut reference_probe, &mut |reference| {
            references.insert(reference.clone());
            Ok(())
        })?;
        if fixed.len() != fixed_references.len() || !fixed.is_subset(&references) {
            return Err(tracker_error(
                "fixed references must be unique source dependencies",
            ));
        }
        let mut seen = BTreeSet::new();
        let mut forward = Vec::new();
        for destination in destinations {
            let target = destination.as_field_ref();
            if !seen.insert(target.clone()) {
                return Err(tracker_error(
                    "duplicate formula copy targets are unsupported",
                ));
            }
            let (row, column) = copy_position(spec, &target)?;
            require_copy_number(document, &target)?;
            let old = &document.entities[&target.entity].fields[&target.field];
            let mut copied = expression.clone();
            map_copy_references(&mut copied, &mut |reference| {
                if fixed.contains(reference) || !spec.entities.contains(&reference.entity) {
                    return require_copy_number(document, reference);
                }
                let (reference_row, reference_column) = copy_position(spec, reference)?;
                let shifted_row = copy_index(reference_row, source_row, row, relative_rows)?;
                let shifted_column =
                    copy_index(reference_column, source_column, column, relative_columns)?;
                *reference = FieldRef::new(
                    spec.entities.get(shifted_row).cloned().ok_or_else(|| {
                        tracker_error("relative reference row is outside the collection")
                    })?,
                    spec.columns
                        .get(shifted_column)
                        .map(|column| column.id.clone())
                        .ok_or_else(|| {
                            tracker_error("relative reference column is outside the collection")
                        })?,
                );
                require_copy_number(document, reference)
            })?;
            let value = Value::Formula(copied);
            if &value != old {
                forward.push(SemanticCommand::set_field_value(target.clone(), value));
            }
        }
        if forward.is_empty() {
            return Err(PatchLifecycleError::NoChange.into());
        }
        let publication = self.publish_commands(expected, forward)?;
        // Match source-based formula authoring: generic formula mutations are
        // outside scalar history, whose Core contract cannot restore a literal
        // over a formula. Rejected copies retain both history directions.
        self.undo.clear();
        self.redo.clear();
        Ok(publication)
    }

    fn publish_commands(
        &mut self,
        expected_revision: &str,
        commands: Vec<SemanticCommand>,
    ) -> Result<PublicationProjection, DesignerError> {
        self.check_revision(expected_revision)?;
        let mut candidate = self.session.export_snapshot().document().clone();
        for command in &commands {
            match command {
                SemanticCommand::SetFieldValue { field, value } => {
                    if let Some(entity) = candidate.entities.get_mut(&field.entity) {
                        entity.fields.insert(field.field.clone(), value.clone());
                    }
                }
                SemanticCommand::AppendEntity { entity } => {
                    candidate.entities.insert(entity.id.clone(), entity.clone());
                }
                SemanticCommand::RemoveEntity { entity } => {
                    candidate.entities.remove(entity);
                }
                SemanticCommand::FormulaUpdate(_) => {
                    return Err(tracker_error("unsupported history command"));
                }
            }
        }
        if validate(&candidate).is_ok() {
            Self::from_document(candidate, PREFLIGHT_OCCURRENCE)?;
        }
        let snapshot = self.session.export_snapshot();
        let proposal_id = self.next_proposal_id()?;
        let body = SemanticPatchBody::atomic_batch(commands)?;
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
        self.refresh_structure();
        self.formula_sources = formula_sources(self.session.export_snapshot().document())?;
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

    fn refresh_structure(&mut self) {
        self.row_serial = self.row_serial.max(
            self.session
                .export_snapshot()
                .document()
                .entities
                .keys()
                .filter_map(|id| {
                    id.as_str()
                        .strip_prefix("tracker_row_")
                        .and_then(|suffix| suffix.split('_').next()?.parse::<usize>().ok())
                })
                .max()
                .unwrap_or(0),
        );
        self.collection_specs = collection_specs(self.session.export_snapshot().document());
        self.collections = self
            .collection_specs
            .values()
            .map(|spec| spec.summary.clone())
            .collect();
    }

    /// Create a bounded operational tracker using existing scalar schema authority.
    ///
    /// # Errors
    /// Returns an admission error without replacing an existing occurrence.
    pub fn tracker(occurrence_id: &str) -> Result<Self, DesignerError> {
        let mut document = Document::empty(occurrence_id, "Driver Tracker");
        let schema = Schema {
            id: SchemaId::from("tracker"),
            key: SchemaKey::from("tracker"),
            fields: [
                ("task", FieldType::Text),
                ("estimate", FieldType::Number),
                ("done", FieldType::Boolean),
            ]
            .into_iter()
            .map(|(key, field_type)| {
                let id = FieldId::from(key);
                (
                    id.clone(),
                    FieldDefinition {
                        id,
                        key: FieldKey::from(key),
                        field_type,
                        required: true,
                    },
                )
            })
            .collect(),
        };
        document.schemas.insert(schema.id.clone(), schema);
        Self::from_document(document, occurrence_id)
    }

    /// Create a small two-collection monthly budget for the bounded Driver slice.
    ///
    /// Collections remain projections of the semantic document: formula references
    /// bind to entity and field identities rather than a collection name or order.
    ///
    /// # Errors
    ///
    /// Returns an admission error without replacing an existing occurrence.
    #[allow(clippy::too_many_lines)] // Keep the bounded fixture readable as one document.
    pub fn budget(occurrence_id: &str) -> Result<Self, DesignerError> {
        let mut document = Document::empty(occurrence_id, "Monthly Budget");
        let budget_items = SchemaId::from("budget_items");
        let budget_summary = SchemaId::from("budget_summary");
        let item_schema = budget_schema(
            budget_items.clone(),
            "budget_items",
            [
                ("name", FieldType::Text),
                ("due_date", FieldType::Date),
                ("planned", FieldType::Number),
                ("actual", FieldType::Number),
                ("variance", FieldType::Number),
            ],
        );
        let summary_schema = budget_schema(
            budget_summary.clone(),
            "budget_summary",
            [
                ("label", FieldType::Text),
                ("month", FieldType::Date),
                ("planned_total", FieldType::Number),
                ("actual_total", FieldType::Number),
                ("remaining", FieldType::Number),
            ],
        );
        document.schemas.insert(budget_items.clone(), item_schema);
        document
            .schemas
            .insert(budget_summary.clone(), summary_schema);
        document.entities.extend([
            budget_item(
                "rent",
                "Rent",
                "2026-09-01",
                1200.0,
                1200.0,
                Expression::Subtract {
                    left: Box::new(Expression::Reference(FieldRef::new("rent", "actual"))),
                    right: Box::new(Expression::Reference(FieldRef::new("rent", "planned"))),
                },
                budget_items.clone(),
            )?,
            budget_item(
                "utilities",
                "Utilities",
                "2026-09-15",
                180.0,
                160.0,
                Expression::Subtract {
                    left: Box::new(Expression::Reference(FieldRef::new("utilities", "actual"))),
                    right: Box::new(Expression::Reference(FieldRef::new("utilities", "planned"))),
                },
                budget_items,
            )?,
            (
                EntityId::from("monthly_summary"),
                Entity {
                    id: EntityId::from("monthly_summary"),
                    key: EntityKey::from("monthly_summary"),
                    schema: budget_summary,
                    fields: BTreeMap::from([
                        (
                            FieldId::from("label"),
                            Value::Text("September 2026".to_owned()),
                        ),
                        (
                            FieldId::from("month"),
                            Value::Date(Date::parse("2026-09-01").map_err(|_| {
                                DesignerError::InvalidDateInput {
                                    input: "2026-09-01".to_owned(),
                                }
                            })?),
                        ),
                        (
                            FieldId::from("planned_total"),
                            Value::Formula(Expression::Add {
                                left: Box::new(Expression::Reference(FieldRef::new(
                                    "rent", "planned",
                                ))),
                                right: Box::new(Expression::Reference(FieldRef::new(
                                    "utilities",
                                    "planned",
                                ))),
                            }),
                        ),
                        (
                            FieldId::from("actual_total"),
                            Value::Formula(Expression::Add {
                                left: Box::new(Expression::Reference(FieldRef::new(
                                    "rent", "actual",
                                ))),
                                right: Box::new(Expression::Reference(FieldRef::new(
                                    "utilities",
                                    "actual",
                                ))),
                            }),
                        ),
                        (
                            FieldId::from("remaining"),
                            Value::Formula(Expression::Subtract {
                                left: Box::new(Expression::Reference(FieldRef::new(
                                    "monthly_summary",
                                    "planned_total",
                                ))),
                                right: Box::new(Expression::Reference(FieldRef::new(
                                    "monthly_summary",
                                    "actual_total",
                                ))),
                            }),
                        ),
                    ]),
                },
            ),
        ]);
        Self::from_document(document, occurrence_id)
    }

    fn check_revision(&self, expected: &str) -> Result<(), DesignerError> {
        if expected != self.current_revision() {
            return Err(DesignerError::StaleQuery {
                requested: expected.to_owned(),
                current: self.current_revision().to_owned(),
            });
        }
        Ok(())
    }

    fn record_edit(
        &mut self,
        expected: &str,
        forward: Vec<SemanticCommand>,
        inverse: Vec<SemanticCommand>,
    ) -> Result<PublicationProjection, DesignerError> {
        if forward.is_empty() {
            return Err(PatchLifecycleError::NoChange.into());
        }
        let publication = self.publish_commands(expected, forward.clone())?;
        if self.undo.len() == 64 {
            self.undo.remove(0);
        }
        self.undo.push(HistoryEntry { forward, inverse });
        self.redo.clear();
        Ok(publication)
    }

    fn edit_cells(
        &mut self,
        expected: &str,
        edits: &[CellEdit],
    ) -> Result<PublicationProjection, DesignerError> {
        self.check_revision(expected)?;
        if edits.is_empty() || edits.len() > MAX_FIELD_QUERY_TARGETS {
            return Err(tracker_error("the edit range is empty or too large"));
        }
        let snapshot = self.session.export_snapshot();
        let mut seen = BTreeSet::new();
        let mut forward = Vec::new();
        let mut inverse = Vec::new();
        for edit in edits {
            let field = edit.target.as_field_ref();
            if !seen.insert(field.clone()) {
                return Err(tracker_error("duplicate cell targets are unsupported"));
            }
            let old = snapshot
                .document()
                .entities
                .get(&field.entity)
                .and_then(|entity| entity.fields.get(&field.field))
                .ok_or_else(|| DesignerError::UnsupportedScalarEdit {
                    field: field.clone(),
                })?;
            let value = parse_scalar(old, &edit.input, &field)?;
            if &value != old {
                forward.push(SemanticCommand::set_field_value(field.clone(), value));
                inverse.push(SemanticCommand::set_field_value(field, old.clone()));
            }
        }
        self.record_edit(expected, forward, inverse)
    }

    fn history_edit(
        &mut self,
        expected: &str,
        redo: bool,
    ) -> Result<PublicationProjection, DesignerError> {
        self.check_revision(expected)?;
        let entry = if redo {
            self.redo.last()
        } else {
            self.undo.last()
        }
        .cloned()
        .ok_or_else(|| tracker_error("no operation is available in this history direction"))?;
        let result = self.publish_commands(
            expected,
            if redo {
                entry.forward.clone()
            } else {
                entry.inverse.clone()
            },
        )?;
        if redo {
            self.redo.pop();
            self.undo.push(entry);
        } else {
            self.undo.pop();
            self.redo.push(entry);
        }
        Ok(result)
    }

    fn tracker_spec(&self, collection: &str) -> Result<&CollectionSpec, DesignerError> {
        let spec = self
            .collection_specs
            .get(collection)
            .ok_or_else(|| tracker_error("collection is unavailable"))?;
        if !is_tracker_spec(spec) {
            return Err(tracker_error(
                "row maintenance is available for the bounded tracker schema",
            ));
        }
        Ok(spec)
    }

    fn append_row(
        &mut self,
        expected: &str,
        collection: &str,
    ) -> Result<PublicationProjection, DesignerError> {
        self.paste_cells(
            expected,
            collection,
            None,
            "task",
            &[vec![String::new(), "0".to_owned(), "false".to_owned()]],
        )
    }

    fn remove_rows(
        &mut self,
        expected: &str,
        entities: &[String],
    ) -> Result<PublicationProjection, DesignerError> {
        self.check_revision(expected)?;
        self.tracker_spec("tracker")?;
        if entities.is_empty() || entities.len() > MAX_TABLE_ROWS {
            return Err(tracker_error("the row selection is empty or too large"));
        }
        let snapshot = self.session.export_snapshot();
        let mut seen = BTreeSet::new();
        let mut forward = Vec::new();
        let mut inverse = Vec::new();
        for id in entities {
            if !seen.insert(id) {
                return Err(tracker_error("duplicate row targets are unsupported"));
            }
            let entity = snapshot
                .document()
                .entities
                .get(&EntityId::from(id.clone()))
                .filter(|entity| entity.schema.as_str() == "tracker")
                .ok_or_else(|| tracker_error("row is unavailable"))?;
            forward.push(SemanticCommand::RemoveEntity {
                entity: entity.id.clone(),
            });
            inverse.push(SemanticCommand::AppendEntity {
                entity: entity.clone(),
            });
        }
        self.record_edit(expected, forward, inverse)
    }

    fn paste_cells(
        &mut self,
        expected: &str,
        collection: &str,
        start_entity: Option<&str>,
        start_field: &str,
        rows: &[Vec<String>],
    ) -> Result<PublicationProjection, DesignerError> {
        self.check_revision(expected)?;
        let spec = self.tracker_spec(collection)?;
        let fields = ["task", "estimate", "done"];
        let column = fields
            .iter()
            .position(|field| *field == start_field)
            .ok_or_else(|| tracker_error("starting field is unavailable"))?;
        let start = start_entity.map_or(Ok(spec.entities.len()), |id| {
            spec.entities
                .iter()
                .position(|entity| entity.as_str() == id)
                .ok_or_else(|| tracker_error("starting row is unavailable"))
        })?;
        if rows.is_empty()
            || start.saturating_add(rows.len()) > MAX_TABLE_ROWS
            || rows[0].is_empty()
            || rows
                .iter()
                .any(|row| row.len() != rows[0].len() || row.len() + column > fields.len())
        {
            return Err(tracker_error(
                "paste must be a nonempty rectangular range within 128 rows and three typed columns",
            ));
        }
        if self.row_serial > usize::MAX - MAX_TOTAL_ENTITIES - MAX_TABLE_ROWS {
            return Err(tracker_error("row identity allocation is exhausted"));
        }
        let snapshot = self.session.export_snapshot();
        let document = snapshot.document();
        let mut forward = Vec::new();
        let mut inverse = Vec::new();
        let mut allocated = BTreeSet::new();
        for (offset, row) in rows.iter().enumerate() {
            let existing = spec
                .entities
                .get(start + offset)
                .and_then(|id| document.entities.get(id));
            let mut entity = existing.cloned().unwrap_or_else(|| {
                tracker_row(
                    document,
                    self.row_serial,
                    &self.row_namespace,
                    &mut allocated,
                )
            });
            for (offset, text) in row.iter().enumerate() {
                let field = FieldRef::new(entity.id.clone(), fields[column + offset]);
                let old = entity
                    .fields
                    .get(&field.field)
                    .ok_or_else(|| tracker_error("tracker row has a missing required value"))?;
                let input = match old {
                    Value::Text(_) => ScalarEditInput::Text {
                        value: text.clone(),
                    },
                    Value::Number(_) => ScalarEditInput::Number {
                        input: text.clone(),
                    },
                    Value::Boolean(_) => ScalarEditInput::Boolean {
                        value: match text.as_str() {
                            "true" => true,
                            "false" => false,
                            _ => {
                                return Err(tracker_error(
                                    "Boolean paste accepts exactly true or false",
                                ));
                            }
                        },
                    },
                    _ => return Err(tracker_error("paste value type is unsupported")),
                };
                let value = parse_scalar(old, &input, &field)?;
                if existing.is_some() && &value != old {
                    forward.push(SemanticCommand::set_field_value(
                        field.clone(),
                        value.clone(),
                    ));
                    inverse.push(SemanticCommand::set_field_value(field.clone(), old.clone()));
                }
                entity.fields.insert(field.field, value);
            }
            if existing.is_none() {
                inverse.push(SemanticCommand::RemoveEntity {
                    entity: entity.id.clone(),
                });
                forward.push(SemanticCommand::AppendEntity { entity });
            }
        }
        inverse.reverse();
        self.record_edit(expected, forward, inverse)
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
/// `occurrence_id` is a trusted host identity, not an identifier read from the
/// project. The host must generate a fresh UUID on every open, even when the
/// same project is reopened. Reusing an occurrence UUID violates this private
/// authority contract; disposed occurrence identities are not persisted here.
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
    let (candidate, opened) = admit_project(input, occurrence_id)?;
    *runtime = Some(candidate);
    Ok(opened)
}

/// Inspect a fully admitted project without replacing any resident occurrence.
///
/// # Errors
/// Returns the same storage, profile, and projection failures as project open.
/// The temporary admission occurrence is discarded together with its history.
pub fn inspect_project(input: &[u8]) -> Result<OpenedProjection, DesignerError> {
    let (_, opened) = admit_project(input, PREFLIGHT_OCCURRENCE)?;
    Ok(opened)
}

fn admit_project(
    input: &[u8],
    occurrence_id: &str,
) -> Result<(DesignerRuntime, OpenedProjection), DesignerError> {
    let document = decode_project_bundle(input)?;
    let candidate = DesignerRuntime::from_document(document, occurrence_id)?;
    let bootstrap = candidate.bootstrap_projection();
    let table = candidate.query_table(&bootstrap.default_collection)?;
    let opened = OpenedProjection { bootstrap, table };
    ensure_opened_projection_size(&opened)?;
    Ok((candidate, opened))
}

/// Destroy the current semantic occurrence without touching durable host data.
pub fn close_project(runtime: &mut Option<DesignerRuntime>) {
    *runtime = None;
}

fn ensure_projection_size(projection: &impl Serialize) -> Result<(), DesignerError> {
    ensure_projection_size_with_limit(projection, MAX_PROJECTION_BYTES)
}

fn ensure_projection_size_with_limit(
    projection: &impl Serialize,
    maximum: usize,
) -> Result<(), DesignerError> {
    let actual = serde_json::to_vec(projection)
        .map_err(|error| DesignerError::UnsupportedProject {
            message: format!("a Designer projection could not be encoded: {error}"),
        })?
        .len();
    if actual > maximum {
        return Err(DesignerError::ProjectionTooLarge { actual, maximum });
    }
    Ok(())
}

fn ensure_table_projection_size(table: &TableProjection) -> Result<(), DesignerError> {
    ensure_projection_size_with_limit(
        table,
        if table.tracker_profile == Some(true) {
            MAX_NATIVE_TRACKER_PROJECTION_BYTES
        } else {
            MAX_PROJECTION_BYTES
        },
    )
}

fn ensure_opened_projection_size(opened: &OpenedProjection) -> Result<(), DesignerError> {
    ensure_projection_size_with_limit(
        opened,
        if opened.table.tracker_profile == Some(true) {
            MAX_NATIVE_TRACKER_PROJECTION_BYTES
        } else {
            MAX_PROJECTION_BYTES
        },
    )
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn ensure_wire_reply_size(reply: &DesignerWireReply) -> Result<(), DesignerError> {
    let maximum = match reply {
        DesignerWireReply::Ok {
            response: DesignerResponse::Opened(opened),
        } if opened.table.tracker_profile == Some(true) => MAX_NATIVE_TRACKER_PROJECTION_BYTES,
        DesignerWireReply::Ok {
            response: DesignerResponse::Table(table),
        } if table.tracker_profile == Some(true) => MAX_NATIVE_TRACKER_PROJECTION_BYTES,
        _ => MAX_PROJECTION_BYTES,
    };
    ensure_projection_size_with_limit(reply, maximum)
}

fn budget_schema<const COUNT: usize>(
    id: SchemaId,
    key: &str,
    fields: [(&str, FieldType); COUNT],
) -> Schema {
    Schema {
        id,
        key: SchemaKey::from(key),
        fields: fields
            .into_iter()
            .map(|(key, field_type)| {
                let id = FieldId::from(key);
                (
                    id.clone(),
                    FieldDefinition {
                        id,
                        key: FieldKey::from(key),
                        field_type,
                        required: true,
                    },
                )
            })
            .collect(),
    }
}

fn budget_item(
    id: &str,
    name: &str,
    due_date: &str,
    planned: f64,
    actual: f64,
    variance: Expression,
    schema: SchemaId,
) -> Result<(EntityId, Entity), DesignerError> {
    let entity_id = EntityId::from(id);
    let date = Date::parse(due_date).map_err(|_| DesignerError::InvalidDateInput {
        input: due_date.to_owned(),
    })?;
    let planned = Number::new(planned).map_err(|_| DesignerError::InvalidNumberInput {
        input: planned.to_string(),
    })?;
    let actual = Number::new(actual).map_err(|_| DesignerError::InvalidNumberInput {
        input: actual.to_string(),
    })?;
    Ok((
        entity_id.clone(),
        Entity {
            id: entity_id,
            key: EntityKey::from(id),
            schema,
            fields: BTreeMap::from([
                (FieldId::from("name"), Value::Text(name.to_owned())),
                (FieldId::from("due_date"), Value::Date(date)),
                (FieldId::from("planned"), Value::Number(planned)),
                (FieldId::from("actual"), Value::Number(actual)),
                (FieldId::from("variance"), Value::Formula(variance)),
            ]),
        },
    ))
}

fn encode_project_bundle_v1(tree: &CanonicalRoProjectV1) -> Result<Vec<u8>, DesignerError> {
    let total = tree.files().iter().try_fold(
        PROJECT_BUNDLE_V1_MAGIC.len() + size_of::<u32>(),
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
    output.extend_from_slice(PROJECT_BUNDLE_V1_MAGIC);
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

fn encode_project_record_v2(document: &Document) -> Result<Vec<u8>, DesignerError> {
    let canonical = to_canonical_string(document)?;
    let total = PROJECT_RECORD_V2_MAGIC
        .len()
        .saturating_add(canonical.len());
    enforce_project_transfer_limit(total)?;
    let mut output = Vec::with_capacity(total);
    output.extend_from_slice(PROJECT_RECORD_V2_MAGIC);
    output.extend_from_slice(canonical.as_bytes());
    Ok(output)
}

fn decode_project_bundle(input: &[u8]) -> Result<Document, DesignerError> {
    enforce_project_transfer_limit(input.len())?;
    if input.starts_with(PROJECT_RECORD_V2_MAGIC) {
        let payload = &input[PROJECT_RECORD_V2_MAGIC.len()..];
        if payload.is_empty() {
            return Err(DesignerError::InvalidProjectTransfer {
                message: "TWDPROJ2 transfer is missing direct-ro/v2 bytes".to_owned(),
            });
        }
        return from_bytes(payload).map_err(DesignerError::from);
    }
    let mut cursor = ProjectBundleCursor::new(input);
    if cursor.take(PROJECT_BUNDLE_V1_MAGIC.len())? != PROJECT_BUNDLE_V1_MAGIC {
        return Err(DesignerError::InvalidProjectTransfer {
            message: "missing private TWD project record discriminator".to_owned(),
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
    match CanonicalRoProjectV1::try_from_files_with_profile(files, ensure_cheap_document_profile) {
        Ok((_, document)) => Ok(document),
        Err(CanonicalRoProjectAdmissionError::Format(error)) => Err(error.into()),
        Err(CanonicalRoProjectAdmissionError::Profile(error)) => Err(error),
    }
}

fn document_contains_date(document: &Document) -> bool {
    document
        .schemas
        .values()
        .flat_map(|schema| schema.fields.values())
        .any(|field| field.field_type == FieldType::Date)
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
            if let Some(occurrence_id) = match &request {
                DesignerRequest::NewTracker { occurrence_id }
                | DesignerRequest::NewBudget { occurrence_id } => Some(occurrence_id),
                _ => None,
            } {
                let result = (match &request {
                    DesignerRequest::NewTracker { .. } => DesignerRuntime::tracker(occurrence_id),
                    DesignerRequest::NewBudget { .. } => DesignerRuntime::budget(occurrence_id),
                    _ => unreachable!("only new project requests enter this branch"),
                })
                .and_then(|candidate| {
                    let opened = OpenedProjection {
                        bootstrap: candidate.bootstrap_projection(),
                        table: candidate.query_table(&candidate.default_collection)?,
                    };
                    ensure_opened_projection_size(&opened)?;
                    *runtime = Some(candidate);
                    Ok(DesignerResponse::Opened(Box::new(opened)))
                });
                return encode_reply(&match result {
                    Ok(response) => DesignerWireReply::Ok { response },
                    Err(error) => DesignerWireReply::Error {
                        error: error.failure_projection(
                            runtime
                                .as_ref()
                                .map_or("unavailable", DesignerRuntime::current_revision),
                        ),
                    },
                });
            }
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

fn is_tracker_spec(spec: &CollectionSpec) -> bool {
    spec.direct_tracker_rows
        && spec.summary.id == "tracker"
        && spec.summary.key == "tracker"
        && spec.columns.len() == 3
        && spec.columns.iter().all(|column| {
            column.required
                && column.key == column.id.as_str()
                && matches!(
                    (column.id.as_str(), &column.field_type),
                    ("task", FieldType::Text)
                        | ("estimate", FieldType::Number)
                        | ("done", FieldType::Boolean)
                )
        })
}

fn copy_position(spec: &CollectionSpec, field: &FieldRef) -> Result<(usize, usize), DesignerError> {
    let row = spec
        .entities
        .iter()
        .position(|entity| entity == &field.entity);
    let column = spec
        .columns
        .iter()
        .position(|column| column.id == field.field);
    row.zip(column)
        .ok_or_else(|| tracker_error("formula copy target must belong to the source collection"))
}

fn copy_index(
    index: usize,
    source: usize,
    destination: usize,
    relative: bool,
) -> Result<usize, DesignerError> {
    if !relative {
        return Ok(index);
    }
    index
        .checked_add(destination)
        .and_then(|index| index.checked_sub(source))
        .ok_or_else(|| tracker_error("relative reference is outside the collection"))
}

fn require_copy_number(document: &Document, field: &FieldRef) -> Result<(), DesignerError> {
    let entity = document
        .entities
        .get(&field.entity)
        .ok_or_else(|| tracker_error("formula reference entity is unavailable"))?;
    let numeric = document
        .schemas
        .get(&entity.schema)
        .and_then(|schema| schema.fields.get(&field.field))
        .is_some_and(|definition| definition.field_type == FieldType::Number);
    if !numeric
        || !matches!(
            entity.fields.get(&field.field),
            Some(Value::Number(_) | Value::Formula(_))
        )
    {
        return Err(tracker_error(
            "formula copy requires present numeric targets and references",
        ));
    }
    Ok(())
}

fn map_copy_references(
    expression: &mut Expression,
    visit: &mut impl FnMut(&mut FieldRef) -> Result<(), DesignerError>,
) -> Result<(), DesignerError> {
    match expression {
        Expression::Number(_) => Ok(()),
        Expression::Reference(reference) => visit(reference),
        Expression::Add { left, right }
        | Expression::Subtract { left, right }
        | Expression::Multiply { left, right }
        | Expression::Divide { left, right }
        | Expression::Minimum { left, right }
        | Expression::Maximum { left, right } => {
            map_copy_references(left, visit)?;
            map_copy_references(right, visit)
        }
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
                    direct_tracker_rows: entities.iter().all(|id| {
                        let fields = &document.entities[id].fields;
                        fields.len() == 3
                            && fields.iter().all(|(field, value)| {
                                matches!(
                                    (field.as_str(), value),
                                    ("task", Value::Text(_))
                                        | ("estimate", Value::Number(_))
                                        | ("done", Value::Boolean(_))
                                )
                            })
                    }),
                    summary,
                    columns: {
                        let mut fields = schema.fields.values().collect::<Vec<_>>();
                        if schema.key.as_str() == "tracker" {
                            fields.sort_by_key(|field| match field.id.as_str() {
                                "task" => 0,
                                "estimate" => 1,
                                "done" => 2,
                                _ => 3,
                            });
                        }
                        fields.into_iter()
                    }
                    .map(|field| ColumnSpec {
                        id: field.id.clone(),
                        key: field.key.to_string(),
                        field_type: field.field_type.clone(),
                        required: field.required,
                    })
                    .collect(),
                    entities,
                },
            )
        })
        .collect()
}

fn select_default_collection(
    collection_specs: &BTreeMap<String, CollectionSpec>,
) -> Result<String, DesignerError> {
    collection_specs
        .values()
        .max_by(|left, right| {
            left.summary
                .entity_count
                .cmp(&right.summary.entity_count)
                .then_with(|| left.columns.len().cmp(&right.columns.len()))
                .then_with(|| right.summary.key.cmp(&left.summary.key))
        })
        .map(|collection| collection.summary.key.clone())
        .ok_or_else(|| DesignerError::UnsupportedProject {
            message: "the project does not contain a collection to display".to_owned(),
        })
}

fn ensure_cheap_document_profile(document: &Document) -> Result<(), DesignerError> {
    let mut profile_string_bytes = 0usize;
    ensure_profile_string(
        &mut profile_string_bytes,
        "document identity",
        document.id.as_str(),
    )?;
    ensure_profile_string(&mut profile_string_bytes, "document title", &document.title)?;
    if document.schemas.len() > MAX_COLLECTIONS {
        return Err(DesignerError::UnsupportedProject {
            message: format!(
                "the project advertises {} collections; the bounded maximum is {MAX_COLLECTIONS}",
                document.schemas.len()
            ),
        });
    }
    for (schema_id, schema) in &document.schemas {
        ensure_profile_string(
            &mut profile_string_bytes,
            "schema map identity",
            schema_id.as_str(),
        )?;
        ensure_profile_string(
            &mut profile_string_bytes,
            "schema identity",
            schema.id.as_str(),
        )?;
        ensure_profile_string(&mut profile_string_bytes, "schema key", schema.key.as_str())?;
        if schema.fields.len() > MAX_TABLE_FIELDS {
            return Err(DesignerError::UnsupportedProject {
                message: DesignerError::CollectionTooLarge {
                    collection: schema.key.to_string(),
                }
                .to_string(),
            });
        }
        for (field_id, field) in &schema.fields {
            ensure_profile_string(
                &mut profile_string_bytes,
                "field map identity",
                field_id.as_str(),
            )?;
            ensure_profile_string(
                &mut profile_string_bytes,
                "field identity",
                field.id.as_str(),
            )?;
            ensure_profile_string(&mut profile_string_bytes, "field key", field.key.as_str())?;
            if let FieldType::Reference { schema } = &field.field_type {
                ensure_profile_string(
                    &mut profile_string_bytes,
                    "reference schema identity",
                    schema.as_str(),
                )?;
            }
        }
    }
    ensure_cheap_entity_profile(document, &mut profile_string_bytes)
}

fn ensure_cheap_entity_profile(
    document: &Document,
    profile_string_bytes: &mut usize,
) -> Result<(), DesignerError> {
    if document.entities.len() > MAX_TOTAL_ENTITIES {
        return Err(DesignerError::UnsupportedProject {
            message: format!(
                "the project contains {} entities; the bounded maximum is {MAX_TOTAL_ENTITIES}",
                document.entities.len()
            ),
        });
    }
    let mut collection_profiles = BTreeMap::new();
    for (entity_id, entity) in &document.entities {
        ensure_profile_string(
            profile_string_bytes,
            "entity map identity",
            entity_id.as_str(),
        )?;
        ensure_profile_string(profile_string_bytes, "entity identity", entity.id.as_str())?;
        ensure_profile_string(profile_string_bytes, "entity key", entity.key.as_str())?;
        ensure_profile_string(
            profile_string_bytes,
            "entity schema identity",
            entity.schema.as_str(),
        )?;
        if entity.fields.len() > MAX_TABLE_FIELDS {
            return Err(DesignerError::UnsupportedProject {
                message: format!(
                    "entity '{}' contains {} stored fields; the bounded maximum is {MAX_TABLE_FIELDS}",
                    entity.id,
                    entity.fields.len()
                ),
            });
        }
        let profile = collection_profiles
            .entry(entity.schema.clone())
            .or_insert((0usize, 0usize));
        for (field_id, value) in &entity.fields {
            ensure_profile_string(
                profile_string_bytes,
                "stored field identity",
                field_id.as_str(),
            )?;
            match value {
                Value::Reference(entity) => {
                    ensure_profile_string(
                        profile_string_bytes,
                        "stored reference identity",
                        entity.as_str(),
                    )?;
                }
                Value::Formula(expression) => {
                    ensure_formula_reference_profile(expression, profile_string_bytes)?;
                }
                Value::Text(text) => {
                    ensure_stored_text_profile(text)?;
                    profile.1 = profile.1.saturating_add(text.len());
                    if profile.1 > MAX_PROJECTION_BYTES {
                        return Err(DesignerError::UnsupportedProject {
                            message: format!(
                                "a collection contains more than the bounded {MAX_PROJECTION_BYTES}-byte stored-text projection maximum"
                            ),
                        });
                    }
                }
                Value::Number(_) | Value::Boolean(_) | Value::Date(_) => {}
            }
        }
        profile.0 = profile.0.saturating_add(1);
        if profile.0 > MAX_TABLE_ROWS {
            return Err(DesignerError::UnsupportedProject {
                message: format!(
                    "a collection exceeds the bounded maximum of {MAX_TABLE_ROWS} entities"
                ),
            });
        }
    }
    let formula_count = document
        .entities
        .values()
        .flat_map(|entity| entity.fields.values())
        .filter(|value| matches!(value, Value::Formula(_)))
        .count();
    if formula_count > MAX_FORMULAS {
        return Err(DesignerError::UnsupportedProject {
            message: format!(
                "the project contains {formula_count} formulas; the bounded maximum is {MAX_FORMULAS}"
            ),
        });
    }
    Ok(())
}

fn ensure_profile_string(
    profile_string_bytes: &mut usize,
    label: &str,
    value: &str,
) -> Result<(), DesignerError> {
    if value.len() > MAX_PROFILE_STRING_BYTES {
        return Err(DesignerError::UnsupportedProject {
            message: format!(
                "the {label} exceeds the bounded {MAX_PROFILE_STRING_BYTES}-byte maximum"
            ),
        });
    }
    *profile_string_bytes = profile_string_bytes.saturating_add(value.len());
    if *profile_string_bytes > MAX_PROJECTION_BYTES {
        return Err(DesignerError::UnsupportedProject {
            message: format!(
                "aggregate project profile strings exceed the bounded {MAX_PROJECTION_BYTES}-byte projection maximum"
            ),
        });
    }
    Ok(())
}

fn ensure_stored_text_profile(value: &str) -> Result<(), DesignerError> {
    if value.len() > MAX_PROJECTION_BYTES {
        return Err(DesignerError::UnsupportedProject {
            message: format!(
                "stored text exceeds the bounded {MAX_PROJECTION_BYTES}-byte projection maximum"
            ),
        });
    }
    Ok(())
}

fn ensure_formula_reference_profile(
    expression: &Expression,
    profile_string_bytes: &mut usize,
) -> Result<(), DesignerError> {
    let mut pending = vec![expression];
    let mut visited = 0usize;
    while let Some(expression) = pending.pop() {
        visited = visited.saturating_add(1);
        if visited > MAX_FORMULA_PROFILE_NODES {
            return Err(DesignerError::UnsupportedProject {
                message: format!(
                    "a formula exceeds the bounded {MAX_FORMULA_PROFILE_NODES}-node maximum"
                ),
            });
        }
        match expression {
            Expression::Reference(reference) => {
                ensure_profile_string(
                    profile_string_bytes,
                    "formula reference entity identity",
                    reference.entity.as_str(),
                )?;
                ensure_profile_string(
                    profile_string_bytes,
                    "formula reference field identity",
                    reference.field.as_str(),
                )?;
            }
            Expression::Add { left, right }
            | Expression::Subtract { left, right }
            | Expression::Multiply { left, right }
            | Expression::Divide { left, right }
            | Expression::Minimum { left, right }
            | Expression::Maximum { left, right } => {
                pending.push(right);
                pending.push(left);
            }
            Expression::Number(_) => {}
        }
    }
    Ok(())
}

fn ensure_static_profile(
    title: &str,
    default_collection: &str,
    collections: &[CollectionSummary],
    collection_specs: &BTreeMap<String, CollectionSpec>,
) -> Result<(), DesignerError> {
    if collections.len() > MAX_COLLECTIONS {
        return Err(DesignerError::UnsupportedProject {
            message: format!(
                "the project advertises {} collections; the bounded maximum is {MAX_COLLECTIONS}",
                collections.len()
            ),
        });
    }
    if !collection_specs.contains_key(default_collection) {
        return Err(DesignerError::UnsupportedProject {
            message: format!("the selected '{default_collection}' collection is unavailable"),
        });
    }
    for collection in collection_specs.values() {
        if collection.columns.len() > MAX_TABLE_FIELDS || collection.entities.len() > MAX_TABLE_ROWS
        {
            return Err(DesignerError::UnsupportedProject {
                message: DesignerError::CollectionTooLarge {
                    collection: collection.summary.key.clone(),
                }
                .to_string(),
            });
        }
    }
    let bootstrap = BootstrapProjection {
        title: title.to_owned(),
        revision: "resident/0".to_owned(),
        default_collection: default_collection.to_owned(),
        collections: collections.to_vec(),
    };
    ensure_projection_size(&bootstrap).map_err(|error| DesignerError::UnsupportedProject {
        message: error.to_string(),
    })?;

    let entities = collection_specs
        .values()
        .flat_map(|collection| collection.entities.iter().map(ToString::to_string))
        .collect::<Vec<_>>();
    let fields = collection_specs
        .values()
        .flat_map(|collection| {
            collection.entities.iter().flat_map(|entity| {
                collection
                    .columns
                    .iter()
                    .map(|column| field_target(&FieldRef::new(entity.clone(), column.id.clone())))
            })
        })
        .collect::<Vec<_>>();
    // The stock Tracker is a fixed, direct three-scalar-field surface: it has
    // no formulas, so a Tracker publication can invalidate its changed stored
    // fields but cannot also invalidate a second, derived-calculation copy of
    // every field. Keep the generic profile conservative; otherwise a valid
    // 128-row Tracker is incorrectly rejected before any export can occur.
    let native_tracker = collection_specs.len() == 1
        && collection_specs
            .values()
            .next()
            .is_some_and(is_tracker_spec);
    let publication = PublicationProjection {
        base_revision: "resident/18446744073709551615".to_owned(),
        resulting_revision: "resident/18446744073709551615".to_owned(),
        entities,
        fields: fields.clone(),
        affected_calculations: (!native_tracker).then_some(fields).unwrap_or_default(),
    };
    ensure_projection_size_with_limit(
        &publication,
        if native_tracker {
            MAX_NATIVE_TRACKER_PROJECTION_BYTES
        } else {
            MAX_PROJECTION_BYTES
        },
    )
    .map_err(|error| DesignerError::UnsupportedProject {
        message: format!("the worst-case publication projection is not bounded: {error}"),
    })
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
            let source = analyze_field(document, "designer-formula-projection", &field)?
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
        [
            (OperationFamily::SetFieldValue, MutationClass::Value),
            (OperationFamily::SetFieldValue, MutationClass::Formula),
            (OperationFamily::FormulaUpdate, MutationClass::Formula),
            (OperationFamily::AppendEntity, MutationClass::Structure),
            (OperationFamily::RemoveEntity, MutationClass::Structure),
            (OperationFamily::RemoveEntity, MutationClass::Destructive),
            (OperationFamily::RemoveEntity, MutationClass::Formula),
        ]
        .into_iter()
        .flat_map(|(family, class)| {
            [
                Ok(GrantRequirement::query(family, scope.clone())),
                GrantRequirement::mutation(
                    AuthorizationAction::Propose,
                    family,
                    class,
                    scope.clone(),
                ),
                GrantRequirement::mutation(
                    AuthorizationAction::Execute,
                    family,
                    class,
                    scope.clone(),
                ),
            ]
        })
        .collect::<Result<Vec<_>, _>>()?,
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

fn add_moonfall_boolean_fixture(document: &mut Document) -> Result<(), DesignerError> {
    let field = FieldId::from("enabled");
    let weapons = document
        .schemas
        .values_mut()
        .find(|schema| schema.key.as_str() == MOONFALL_BOOLEAN_FIXTURE_COLLECTION)
        .ok_or_else(|| DesignerError::UnsupportedProject {
            message: "the Moonfall weapons schema is unavailable".to_owned(),
        })?;
    if weapons.fields.contains_key(&field) {
        return Err(DesignerError::UnsupportedProject {
            message: "the Moonfall Boolean fixture field is already present".to_owned(),
        });
    }
    weapons.fields.insert(
        field.clone(),
        FieldDefinition {
            id: field.clone(),
            key: FieldKey::from("enabled"),
            field_type: FieldType::Boolean,
            required: true,
        },
    );
    let iron_sword = document
        .entities
        .values_mut()
        .find(|entity| entity.key.as_str() == "iron_sword")
        .ok_or_else(|| DesignerError::UnsupportedProject {
            message: "the Moonfall iron_sword entity is unavailable".to_owned(),
        })?;
    iron_sword.fields.insert(field, Value::Boolean(true));
    Ok(())
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
        Value::Date(value) => StoredValueProjection::Date { value: *value },
        Value::Reference(entity) => StoredValueProjection::Reference {
            entity: entity.to_string(),
        },
        Value::Formula(_) => unreachable!("formula definitions are projected separately"),
    }
}

const fn scalar_kind(value: Option<&Value>) -> Option<ScalarKind> {
    match value {
        Some(Value::Number(_)) => Some(ScalarKind::Number),
        Some(Value::Text(_)) => Some(ScalarKind::Text),
        Some(Value::Boolean(_)) => Some(ScalarKind::Boolean),
        Some(Value::Date(_)) => Some(ScalarKind::Date),
        Some(Value::Reference(_) | Value::Formula(_)) | None => None,
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
        FieldType::Date => "date",
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

fn tracker_row(
    document: &Document,
    row_serial: usize,
    namespace: &str,
    allocated: &mut BTreeSet<EntityId>,
) -> Entity {
    let mut serial = row_serial.saturating_add(1);
    loop {
        let id = EntityId::from(format!("tracker_row_{serial:04}_{namespace}"));
        let key = EntityKey::from(format!("row_{serial:04}"));
        if !document.entities.contains_key(&id)
            && !allocated.contains(&id)
            && !document.entities.values().any(|entity| entity.key == key)
        {
            allocated.insert(id.clone());
            break Entity {
                id,
                key,
                schema: SchemaId::from("tracker"),
                fields: [
                    (FieldId::from("task"), Value::Text(String::new())),
                    (
                        FieldId::from("estimate"),
                        Value::Number(Number::new(0.0).expect("zero is finite")),
                    ),
                    (FieldId::from("done"), Value::Boolean(false)),
                ]
                .into_iter()
                .collect(),
            };
        }
        serial += 1;
    }
}

fn tracker_error(message: &str) -> DesignerError {
    DesignerError::InvalidTrackerOperation {
        message: message.to_owned(),
    }
}

fn parse_scalar(
    old: &Value,
    input: &ScalarEditInput,
    field: &FieldRef,
) -> Result<Value, DesignerError> {
    match (old, input) {
        (Value::Number(_), ScalarEditInput::Number { input }) => input
            .parse::<f64>()
            .ok()
            .and_then(|value| Number::new(value).ok())
            .map(Value::Number)
            .ok_or_else(|| DesignerError::InvalidNumberInput {
                input: input.clone(),
            }),
        (Value::Text(_), ScalarEditInput::Text { value }) => Ok(Value::Text(value.clone())),
        (Value::Boolean(_), ScalarEditInput::Boolean { value }) => Ok(Value::Boolean(*value)),
        (Value::Date(_), ScalarEditInput::Date { value }) => Date::parse(value)
            .map(Value::Date)
            .map_err(|_| DesignerError::InvalidDateInput {
                input: value.clone(),
            }),
        _ => Err(DesignerError::UnsupportedScalarEdit {
            field: field.clone(),
        }),
    }
}
#[cfg(test)]
mod tests {
    use super::{DesignerError, DesignerRuntime, MAX_WIDTH_FINITE_JSON_NUMBER, ProposalId};

    #[test]
    fn formula_requests_do_not_retain_admitted_rejections_or_completed_proposals() {
        let base = DesignerRuntime::budget("00000000-0000-4000-8000-000000000000").unwrap();
        let mut document = base.session.export_snapshot().document().clone();
        document
            .schemas
            .retain(|id, _| id.as_str() == "budget_items");
        document
            .schemas
            .values_mut()
            .for_each(|schema| schema.fields.retain(|id, _| id.as_str() == "planned"));
        let template = document.entities[&super::EntityId::from("rent")].clone();
        document.entities.clear();
        for index in 0..33 {
            let mut entity = template.clone();
            entity.id = super::EntityId::from(format!("r{index:02}"));
            entity.key = super::EntityKey::from(entity.id.to_string());
            let number = super::Number::new(1.0).unwrap();
            let value = if index < 32 {
                super::Value::Formula(super::Expression::Number(number))
            } else {
                super::Value::Number(number)
            };
            entity.fields = [(super::FieldId::from("planned"), value)]
                .into_iter()
                .collect();
            document.entities.insert(entity.id.clone(), entity);
        }
        let mut runtime =
            DesignerRuntime::from_document(document, "00000000-0000-4000-8000-000000000000")
                .unwrap();
        let before = runtime.export_project("resident/0").unwrap().bytes;
        for serial in 1..=32 {
            let error = runtime
                .update_formula("resident/0", &"r32.planned".into(), "3")
                .unwrap_err();
            assert!(matches!(error, DesignerError::UnsupportedProject { .. }));
            assert!(
                runtime
                    .lifecycle
                    .proposal_history(&ProposalId::from(format!(
                        "designer-proposal/{}/{serial}",
                        runtime.row_namespace
                    )))
                    .is_err()
            );
            assert_eq!(runtime.export_project("resident/0").unwrap().bytes, before);
        }
        let result = runtime
            .update_formula("resident/0", &"r00.planned".into(), "1201")
            .unwrap();
        assert_eq!(result.resulting_revision, "resident/1");
        assert!(
            runtime
                .lifecycle
                .proposal_history(&ProposalId::from(format!(
                    "designer-proposal/{}/33",
                    runtime.row_namespace
                )))
                .is_err()
        );
        assert_eq!(
            runtime
                .query_fields("resident/1", &["r00.planned".into()])
                .unwrap()
                .fields[0]
                .calculated
                .as_ref()
                .and_then(super::CalculationProjection::number),
            Some(1201.0)
        );
    }

    #[test]
    fn exhausted_proposal_identity_counter_never_reuses_or_publishes() {
        let mut runtime = DesignerRuntime::budget("00000000-0000-4000-8000-000000000000").unwrap();
        let operation = super::CleanupOperation::Convert {
            source: "utilities.planned".into(),
            destination: "utilities.actual".into(),
        };
        let pending = runtime.preview_cleanup("resident/0", &operation).unwrap();
        let before = runtime.export_project("resident/0").unwrap().bytes;
        runtime.proposal_serial = u64::MAX - 1;
        let last = runtime.next_proposal_id().unwrap();
        assert_ne!(last.as_str(), pending.preview_id);
        for _ in 0..2 {
            assert!(runtime.next_proposal_id().is_err());
            assert!(runtime.preview_cleanup("resident/0", &operation).is_err());
            assert!(
                runtime
                    .update_formula("resident/0", &"rent.planned".into(), "1300")
                    .is_err()
            );
            assert!(
                runtime
                    .publish_commands(
                        "resident/0",
                        vec![super::SemanticCommand::SetFieldValue {
                            field: super::FieldRef::new("utilities", "actual"),
                            value: super::Value::Number(super::Number::new(170.0).unwrap()),
                        }]
                    )
                    .is_err()
            );
            assert_eq!(runtime.proposal_serial, u64::MAX);
            assert_eq!(
                runtime.pending_cleanup.as_ref().unwrap().preview_id,
                pending.preview_id
            );
            assert_eq!(runtime.export_project("resident/0").unwrap().bytes, before);
            assert!(runtime.undo.is_empty());
            assert!(runtime.redo.is_empty());
        }
    }

    #[test]
    fn worst_case_refresh_number_uses_the_maximum_finite_json_width() {
        assert_eq!(
            serde_json::to_string(&MAX_WIDTH_FINITE_JSON_NUMBER)
                .expect("finite Number must serialize")
                .len(),
            24
        );
    }
}
