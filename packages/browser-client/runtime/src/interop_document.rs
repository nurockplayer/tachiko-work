//! Private import composition and revision-pinned cleanup for the declared
//! Designer spreadsheet profile. Semantic publication remains in the runtime.

use super::{
    DesignerError, DesignerRuntime, FieldProjection, FieldTarget, HistoryAction, HistoryEntry,
    ScalarEditInput, StoredValueProjection,
};
use serde::{Deserialize, Serialize};
use tachiko_formula_engine::{
    CanonicalAuthoringProjectionError, ExpressionComplexityError, bind_expression,
    parse_expression, project_expression, validate_expression_structure,
};

fn interop_admission_error(error: super::interop_adapter::InteropError) -> DesignerError {
    match error {
        super::interop_adapter::InteropError::Invalid(message)
        | super::interop_adapter::InteropError::InvalidText(message) => {
            super::tracker_error(&message)
        }
        super::interop_adapter::InteropError::Capacity(capacity) => {
            DesignerError::ImportCapacityExceeded {
                capacity,
                at_least: false,
            }
        }
        super::interop_adapter::InteropError::CapacityLowerBound(capacity) => {
            DesignerError::ImportCapacityExceeded {
                capacity,
                at_least: true,
            }
        }
    }
}

use tachiko_workspace_engine::patch_lifecycle::SemanticCommand;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CleanupOperation {
    Trim {
        fields: Vec<FieldTarget>,
    },
    Replace {
        fields: Vec<FieldTarget>,
        find: String,
        replacement: String,
    },
    Split {
        source: FieldTarget,
        destinations: Vec<FieldTarget>,
        separator: String,
    },
    Convert {
        source: FieldTarget,
        destination: FieldTarget,
    },
    Fill {
        fields: Vec<FieldTarget>,
        input: ScalarEditInput,
    },
    Deduplicate {
        entities: Vec<String>,
        key_fields: Vec<String>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CleanupChange {
    pub target: FieldTarget,
    pub before: Option<FieldProjection>,
    pub after: Option<StoredValueProjection>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CleanupPreview {
    pub preview_id: String,
    pub revision: String,
    pub changes: Vec<CleanupChange>,
    pub removed_entities: Vec<String>,
}

pub(super) struct PendingCleanup {
    pub(super) preview_id: String,
    pub(super) revision: String,
    pub(super) commands: Vec<SemanticCommand>,
    pub(super) inverse: Vec<SemanticCommand>,
}

impl DesignerRuntime {
    pub(super) fn preview_cleanup(
        &mut self,
        expected_revision: &str,
        operation: &CleanupOperation,
    ) -> Result<CleanupPreview, DesignerError> {
        self.check_revision(expected_revision)?;
        if self.text_capacity && matches!(operation, CleanupOperation::Deduplicate { .. }) {
            return Err(tracker_error(
                "capacity Text projects cannot deduplicate rows",
            ));
        }
        self.prepare_cleanup(expected_revision, operation)
    }
}

use super::{
    CollectionSpec, MAX_FIELD_QUERY_TARGETS, MAX_PROFILE_STRING_BYTES, MAX_PROJECTION_BYTES,
    MAX_TABLE_FIELDS, MAX_TABLE_ROWS, PREFLIGHT_OCCURRENCE, PublicationProjection,
    designer_lifecycle, ensure_projection_size, field_target, stored_value_projection,
    tracker_error,
};
use std::collections::BTreeSet;
use tachiko_workspace_engine::patch_lifecycle::{
    ProposalRequest, SemanticPatchBody, SemanticRevision,
};
use tachiko_workspace_engine::{
    ColumnAggregateFunction, ColumnCriterion, ColumnRef, ComparisonOperator, ConditionalAggregate,
    Date, Document, EntityId, FieldId, FieldRef, FieldType, Number, Value,
};

impl DesignerRuntime {
    fn prepare_cleanup(
        &mut self,
        expected_revision: &str,
        operation: &CleanupOperation,
    ) -> Result<CleanupPreview, DesignerError> {
        let snapshot = self.session.export_snapshot();
        let commands = cleanup_commands(snapshot.document(), operation)?;
        if commands.is_empty() {
            return Err(super::PatchLifecycleError::NoChange.into());
        }
        let inverse = cleanup_inverse(snapshot.document(), &commands)?;
        let mut candidate = snapshot.document().clone();
        for command in &commands {
            match command {
                SemanticCommand::SetFieldValue { field, value } => {
                    let entity = candidate
                        .entities
                        .get_mut(&field.entity)
                        .ok_or_else(|| tracker_error("cleanup entity is unavailable"))?;
                    entity.fields.insert(field.field.clone(), value.clone());
                }
                SemanticCommand::RemoveEntity { entity } => {
                    candidate.entities.remove(entity);
                }
                _ => return Err(tracker_error("unsupported cleanup command")),
            }
        }
        // Preview must prove that the exact final candidate remains reopenable.
        Self::from_document(candidate, PREFLIGHT_OCCURRENCE)?;
        let mut lifecycle = designer_lifecycle(
            snapshot.document_scope(),
            snapshot.document(),
            &self.principal,
        )?;
        let proposal_id = self.next_proposal_id()?;
        let preview_id = proposal_id.as_str().to_owned();
        lifecycle.propose(
            snapshot.document_scope(),
            snapshot.document(),
            snapshot.revision(),
            ProposalRequest::new(
                proposal_id.clone(),
                SemanticRevision::from(expected_revision.to_owned()),
                SemanticPatchBody::atomic_batch(commands.clone())?,
                self.principal.clone(),
            ),
            self.clock.tick(),
        )?;
        super::designer_preview(
            &mut lifecycle,
            &snapshot,
            &proposal_id,
            &self.principal,
            self.clock.tick(),
        )?;
        let (changes, removed_entities) =
            cleanup_changes(self, expected_revision, snapshot.document(), &commands)?;
        let preview = CleanupPreview {
            preview_id: preview_id.clone(),
            revision: expected_revision.to_owned(),
            changes,
            removed_entities,
        };
        ensure_projection_size(&preview)?;
        // Repeated previews replace one bounded pending plan; they never append
        // proposals or histories to resident semantic state.
        self.pending_cleanup = Some(PendingCleanup {
            preview_id,
            revision: expected_revision.to_owned(),
            commands,
            inverse,
        });
        Ok(preview)
    }

    pub(super) fn commit_cleanup(
        &mut self,
        expected_revision: &str,
        preview_id: &str,
    ) -> Result<PublicationProjection, DesignerError> {
        self.check_revision(expected_revision)?;
        let pending = self
            .pending_cleanup
            .as_ref()
            .filter(|pending| {
                pending.preview_id == preview_id && pending.revision == expected_revision
            })
            .ok_or_else(|| tracker_error("cleanup preview is absent or no longer current"))?;
        let commands = pending.commands.clone();
        let inverse = pending.inverse.clone();
        let publication = self.publish_commands(expected_revision, commands.clone())?;
        self.pending_cleanup = None;
        self.record_history_entry(HistoryEntry {
            forward: HistoryAction::Commands(commands),
            inverse: HistoryAction::Commands(inverse),
        });
        Ok(publication)
    }
}

fn cleanup_inverse(
    document: &Document,
    commands: &[SemanticCommand],
) -> Result<Vec<SemanticCommand>, DesignerError> {
    let mut inverse = Vec::with_capacity(commands.len());
    for command in commands {
        match command {
            SemanticCommand::SetFieldValue { field, .. } => {
                if let Some(value) = document
                    .entities
                    .get(&field.entity)
                    .and_then(|entity| entity.fields.get(&field.field))
                {
                    inverse.push(SemanticCommand::set_field_value(
                        field.clone(),
                        value.clone(),
                    ));
                } else {
                    inverse.push(SemanticCommand::UnsetField {
                        field: field.clone(),
                    });
                }
            }
            SemanticCommand::RemoveEntity { entity } => {
                let record = document
                    .entities
                    .get(entity)
                    .ok_or_else(|| tracker_error("cleanup entity is unavailable"))?;
                inverse.push(SemanticCommand::AppendEntity {
                    entity: record.clone(),
                });
            }
            _ => return Err(tracker_error("unsupported cleanup command")),
        }
    }
    inverse.reverse();
    Ok(inverse)
}

fn cleanup_changes(
    runtime: &DesignerRuntime,
    expected_revision: &str,
    document: &Document,
    commands: &[SemanticCommand],
) -> Result<(Vec<CleanupChange>, Vec<String>), DesignerError> {
    let mut changes = Vec::new();
    let mut removed_entities = Vec::new();
    for command in commands {
        match command {
            SemanticCommand::SetFieldValue { field, value } => {
                let before = if document.entities[&field.entity]
                    .fields
                    .contains_key(&field.field)
                {
                    Some(
                        runtime
                            .query_fields(expected_revision, &[field_target(field)])?
                            .fields
                            .remove(0),
                    )
                } else {
                    None
                };
                changes.push(CleanupChange {
                    target: field_target(field),
                    before,
                    after: Some(stored_value_projection(value)),
                });
            }
            SemanticCommand::RemoveEntity { entity } => {
                removed_entities.push(entity.to_string());
                let record = &document.entities[entity];
                let fields = record
                    .fields
                    .keys()
                    .map(|field| field_target(&FieldRef::new(entity.clone(), field.clone())))
                    .collect::<Vec<_>>();
                for before in runtime.query_fields(expected_revision, &fields)?.fields {
                    changes.push(CleanupChange {
                        target: before.target.clone(),
                        before: Some(before),
                        after: None,
                    });
                }
            }
            _ => return Err(tracker_error("unsupported cleanup command")),
        }
    }
    Ok((changes, removed_entities))
}

fn cleanup_commands(
    document: &Document,
    operation: &CleanupOperation,
) -> Result<Vec<SemanticCommand>, DesignerError> {
    match operation {
        CleanupOperation::Trim { fields } => {
            text_updates(document, fields, |value| Ok(value.trim().to_owned()))
        }
        CleanupOperation::Replace {
            fields,
            find,
            replacement,
        } => {
            if find.is_empty()
                || find.len() > MAX_PROFILE_STRING_BYTES
                || replacement.len() > MAX_PROFILE_STRING_BYTES
            {
                return Err(tracker_error(
                    "literal replacement requires a nonempty bounded search and replacement",
                ));
            }
            text_updates(document, fields, |value| {
                let matches = value.matches(find).count();
                let length = value
                    .len()
                    .checked_sub(matches * find.len())
                    .and_then(|length| {
                        matches
                            .checked_mul(replacement.len())
                            .and_then(|added| length.checked_add(added))
                    });
                if length.is_none_or(|length| length > MAX_PROJECTION_BYTES) {
                    return Err(tracker_error("replacement exceeds the bounded text size"));
                }
                Ok(value.replace(find, replacement))
            })
        }
        CleanupOperation::Split {
            source,
            destinations,
            separator,
        } => {
            if separator.is_empty() || separator.len() > MAX_PROFILE_STRING_BYTES {
                return Err(tracker_error("split requires a nonempty bounded separator"));
            }
            let source_ref = source.as_field_ref();
            let text = require_text(document, &source_ref)?;
            let targets = unique_targets(destinations)?;
            if targets.contains(&source_ref) {
                return Err(tracker_error("split retains its source field"));
            }
            let parts = text
                .split(separator)
                .take(destinations.len() + 1)
                .collect::<Vec<_>>();
            if parts.len() != destinations.len() {
                return Err(tracker_error(
                    "split output count must exactly match selected destinations",
                ));
            }
            let mut commands = Vec::new();
            for (destination, part) in destinations.iter().zip(parts) {
                append_value_command(
                    document,
                    destination.as_field_ref(),
                    Value::Text(part.to_owned()),
                    &mut commands,
                )?;
            }
            Ok(commands)
        }
        CleanupOperation::Convert {
            source,
            destination,
        } => {
            if source == destination {
                return Err(tracker_error("type conversion retains its source field"));
            }
            let source = stored_value(document, &source.as_field_ref())?
                .ok_or_else(|| tracker_error("conversion source is missing"))?;
            let target = destination.as_field_ref();
            let kind = declared_type(document, &target)?;
            let value = convert_value(source, kind)?;
            let mut commands = Vec::new();
            append_value_command(document, target, value, &mut commands)?;
            Ok(commands)
        }
        CleanupOperation::Fill { fields, input } => {
            let targets = unique_targets(fields)?;
            let value = input_value(input)?;
            let mut commands = Vec::new();
            for target in targets {
                if stored_value(document, &target)?.is_some() {
                    continue;
                }
                append_value_command(document, target, value.clone(), &mut commands)?;
            }
            Ok(commands)
        }
        CleanupOperation::Deduplicate {
            entities,
            key_fields,
        } => deduplicate(document, entities, key_fields),
    }
}

fn unique_targets(fields: &[FieldTarget]) -> Result<BTreeSet<FieldRef>, DesignerError> {
    if fields.is_empty() || fields.len() > MAX_FIELD_QUERY_TARGETS {
        return Err(tracker_error("cleanup range is empty or too large"));
    }
    let targets = fields
        .iter()
        .map(FieldTarget::as_field_ref)
        .collect::<BTreeSet<_>>();
    if targets.len() != fields.len() {
        return Err(tracker_error("duplicate cleanup targets are unsupported"));
    }
    Ok(targets)
}

fn stored_value<'a>(
    document: &'a Document,
    field: &FieldRef,
) -> Result<Option<&'a Value>, DesignerError> {
    declared_type(document, field)?;
    Ok(document.entities[&field.entity].fields.get(&field.field))
}

fn declared_type<'a>(
    document: &'a Document,
    field: &FieldRef,
) -> Result<&'a FieldType, DesignerError> {
    let entity = document
        .entities
        .get(&field.entity)
        .ok_or_else(|| tracker_error("cleanup entity is unavailable"))?;
    document
        .schemas
        .get(&entity.schema)
        .and_then(|schema| schema.fields.get(&field.field))
        .map(|field| &field.field_type)
        .ok_or_else(|| tracker_error("cleanup field definition is unavailable"))
}

fn require_text<'a>(document: &'a Document, field: &FieldRef) -> Result<&'a str, DesignerError> {
    match stored_value(document, field)? {
        Some(Value::Text(text)) => Ok(text),
        _ => Err(tracker_error("cleanup requires an existing Text source")),
    }
}

fn text_updates(
    document: &Document,
    fields: &[FieldTarget],
    mut transform: impl FnMut(&str) -> Result<String, DesignerError>,
) -> Result<Vec<SemanticCommand>, DesignerError> {
    let targets = unique_targets(fields)?;
    let mut commands = Vec::new();
    for target in targets {
        let text = require_text(document, &target)?;
        append_value_command(
            document,
            target,
            Value::Text(transform(text)?),
            &mut commands,
        )?;
    }
    Ok(commands)
}

fn append_value_command(
    document: &Document,
    field: FieldRef,
    value: Value,
    commands: &mut Vec<SemanticCommand>,
) -> Result<(), DesignerError> {
    let kind = declared_type(document, &field)?;
    let matching = matches!(
        (kind, &value),
        (FieldType::Text, Value::Text(_))
            | (FieldType::Number, Value::Number(_))
            | (FieldType::Boolean, Value::Boolean(_))
            | (FieldType::Date, Value::Date(_))
    );
    if !matching {
        return Err(tracker_error(
            "cleanup output does not match the existing destination type",
        ));
    }
    let old = stored_value(document, &field)?;
    if matches!(old, Some(Value::Formula(_))) {
        return Err(tracker_error("cleanup cannot replace a formula"));
    }
    if old != Some(&value) {
        commands.push(SemanticCommand::set_field_value(field, value));
    }
    Ok(())
}

fn input_value(input: &ScalarEditInput) -> Result<Value, DesignerError> {
    match input {
        ScalarEditInput::Clear => Err(tracker_error("cleanup fill requires a typed stored value")),
        ScalarEditInput::Text { value } => Ok(Value::Text(value.clone())),
        ScalarEditInput::Number { input } => {
            convert_value(&Value::Text(input.clone()), &FieldType::Number)
        }
        ScalarEditInput::Date { value } => {
            convert_value(&Value::Text(value.clone()), &FieldType::Date)
        }
        ScalarEditInput::Boolean { value } => Ok(Value::Boolean(*value)),
    }
}

pub(crate) fn convert_value(value: &Value, kind: &FieldType) -> Result<Value, DesignerError> {
    let text = match value {
        Value::Text(text) => text.clone(),
        Value::Number(number) => number.get().to_string(),
        Value::Boolean(value) => value.to_string(),
        Value::Date(date) => date.to_string(),
        Value::Formula(_) | Value::Reference(_) => {
            return Err(tracker_error("conversion requires a stored scalar source"));
        }
    };
    match kind {
        FieldType::Text => Ok(Value::Text(text)),
        FieldType::Number => text
            .parse::<f64>()
            .ok()
            .and_then(|number| Number::new(number).ok())
            .map(Value::Number)
            .ok_or_else(|| {
                tracker_error("conversion requires an unambiguous finite decimal Number")
            }),
        FieldType::Boolean => match text.as_str() {
            "true" => Ok(Value::Boolean(true)),
            "false" => Ok(Value::Boolean(false)),
            _ => Err(tracker_error(
                "Boolean conversion accepts only true or false",
            )),
        },
        FieldType::Date => Date::parse(&text)
            .map(Value::Date)
            .map_err(|_| tracker_error("Date conversion requires Gregorian YYYY-MM-DD")),
        FieldType::Reference { .. } => Err(tracker_error(
            "conversion target type is outside the cleanup profile",
        )),
    }
}

fn deduplicate(
    document: &Document,
    entities: &[String],
    key_fields: &[String],
) -> Result<Vec<SemanticCommand>, DesignerError> {
    if entities.is_empty()
        || entities.len() > MAX_TABLE_ROWS
        || key_fields.is_empty()
        || key_fields.len() > MAX_TABLE_FIELDS
    {
        return Err(tracker_error(
            "deduplication requires bounded rows and key fields",
        ));
    }
    let selected = entities
        .iter()
        .map(|entity| EntityId::from(entity.clone()))
        .collect::<BTreeSet<_>>();
    let keys = key_fields
        .iter()
        .map(|field| FieldId::from(field.clone()))
        .collect::<BTreeSet<_>>();
    if selected.len() != entities.len() || keys.len() != key_fields.len() {
        return Err(tracker_error(
            "duplicate row or field selectors are unsupported",
        ));
    }
    let mut schema = None;
    let mut seen = Vec::new();
    let mut commands = Vec::new();
    // Keep the first canonical stable-ID row; display sorting never changes
    // which semantic entity survives a deduplication request.
    for id in selected {
        let entity = document
            .entities
            .get(&id)
            .ok_or_else(|| tracker_error("deduplication row is unavailable"))?;
        if schema.is_some_and(|schema| schema != &entity.schema) {
            return Err(tracker_error(
                "deduplication rows must belong to one collection",
            ));
        }
        schema = Some(&entity.schema);
        let values = keys
            .iter()
            .map(|key| {
                stored_value(document, &FieldRef::new(id.clone(), key.clone()))
                    .map(Option::<&Value>::cloned)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if seen.contains(&values) {
            commands.push(SemanticCommand::RemoveEntity { entity: id });
        } else {
            seen.push(values);
        }
    }
    Ok(commands)
}

use super::interop_adapter::{
    CellStyle, CriterionKind, FidelityCategory, FidelityFinding, MAX_COLUMNS, MAX_DATA_ROWS,
    MAX_NATIVE_BUDGET_COLUMNS, MAX_NATIVE_BUDGET_DATA_ROWS, MAX_NATIVE_BUDGET_SHEETS, MAX_SHEETS,
    MAX_TEXT_CAPACITY_COLUMNS, OutputProfile, SourceCell, SourceColumn, SourceLayout, SourceSheet,
    SourceValue, SourceWorkbook, SummaryAggregate, SummaryRow, call_argument, conditional_shape,
    criterion_literal, split_arguments, whole_column_argument, writable_criterion,
};
use super::{IdGenerator, OpenedProjection, SemanticIdKind};
use crate::translate_a1::{A1RefusalCode, A1TokenSpan};
use std::collections::BTreeMap;
use tachiko_workspace_engine::{
    CriterionOperandKind, CriterionTest, Entity, EntityKey, Expression, FieldDefinition, FieldKey,
    Schema, SchemaId, SchemaKey,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportFieldType {
    Text,
    Number,
    Boolean,
    Date,
}

impl ImportFieldType {
    fn field_type(self) -> FieldType {
        match self {
            Self::Text => FieldType::Text,
            Self::Number => FieldType::Number,
            Self::Boolean => FieldType::Boolean,
            Self::Date => FieldType::Date,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImportColumnSpec {
    pub name: String,
    pub field_type: ImportFieldType,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImportSelection {
    pub column_types: Vec<Vec<ImportFieldType>>,
    pub extra_columns: Vec<Vec<ImportColumnSpec>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InteropMetadata {
    pub version: u32,
    pub sheets: Vec<InteropSheetMetadata>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InteropSheetMetadata {
    pub schema_id: String,
    pub name: String,
    pub has_header: bool,
    pub columns: Vec<InteropColumnMetadata>,
    pub rows: Vec<InteropRowMetadata>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InteropColumnMetadata {
    pub field_id: String,
    pub name: String,
    #[serde(default, deserialize_with = "metadata_width")]
    pub width: Option<f64>,
}

// Internally tagged operations buffer JSON numbers with arbitrary_precision.
// Decode through Number so both integer and fractional widths survive buffering.
fn metadata_width<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<f64>, D::Error> {
    Option::<serde_json::Number>::deserialize(deserializer)?
        .map(|number| {
            number
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| serde::de::Error::custom("metadata width must be finite"))
        })
        .transpose()
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InteropRowMetadata {
    pub entity_id: String,
    pub styles: Vec<CellStyle>,
}

/// Private, outbound-only presentation binding for the stock Driver Tracker.
///
/// This deliberately is not import metadata: Tracker retains its own outbound
/// geometry and presentation binding independently of shared import capacity.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeTrackerExportPresentation {
    pub version: u32,
    pub rows: Vec<NativeTrackerExportRow>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeTrackerExportRow {
    pub entity_id: String,
    /// One style per column, in the table's projected column order at the
    /// export revision (as `queryTable` returns it; R4a moves change it).
    pub styles: Vec<CellStyle>,
}

/// Private, outbound-only presentation binding for the native Budget escape.
///
/// The view list is presentation state: multiple views may point at the same
/// canonical collection. Collection rows are kept separately so aliases can
/// never duplicate the authoritative worksheet data or change its identity
/// mapping.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBudgetExportPresentation {
    pub version: u32,
    pub active_view: String,
    pub views: Vec<NativeBudgetExportView>,
    pub collections: Vec<NativeBudgetExportCollection>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBudgetExportView {
    pub id: String,
    pub name: String,
    pub collection_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBudgetExportCollection {
    pub collection_id: String,
    pub rows: Vec<NativeBudgetExportRow>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBudgetExportRow {
    pub entity_id: String,
    pub styles: Vec<CellStyle>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ImportedProjection {
    pub opened: OpenedProjection,
    pub metadata: InteropMetadata,
    pub ledger: Vec<FidelityFinding>,
}

struct ImportIds<'a> {
    namespace: &'a str,
    serial: usize,
}
impl IdGenerator for ImportIds<'_> {
    fn generate(&mut self, _kind: SemanticIdKind) -> String {
        self.serial += 1;
        format!("import_{}_{:04}", self.namespace, self.serial)
    }
}

/// Build one fully admitted candidate without installing a resident occurrence.
///
/// # Errors
/// Rejects blocked fidelity, selection mismatches, invalid scalar conversions,
/// unsupported formulas, cycles, or any normal Designer admission failure.
/// A workbook that exceeds the projection profile refuses with the typed
/// `projection_bytes` capacity limit instead of a raw projection error.
pub fn import_workbook(
    workbook: &SourceWorkbook,
    selection: &ImportSelection,
    occurrence_id: &str,
) -> Result<(DesignerRuntime, ImportedProjection), DesignerError> {
    import_workbook_candidate(workbook, selection, occurrence_id).map_err(import_capacity_refusal)
}

/// Name the ordinary projection bound as an import capacity refusal, so the
/// host can say why the workbook did not open.
fn import_capacity_refusal(error: DesignerError) -> DesignerError {
    let (actual, maximum) = match error {
        DesignerError::ProjectionTooLarge { actual, maximum }
        | DesignerError::PostEditRefreshTooLarge { actual, maximum } => (actual, maximum),
        error => return error,
    };
    DesignerError::ImportCapacityExceeded {
        capacity: super::CapacityExceeded {
            limit: super::CapacityLimit::ProjectionBytes,
            maximum,
            attempted: actual,
            collection: None,
        },
        at_least: false,
    }
}

fn import_workbook_candidate(
    workbook: &SourceWorkbook,
    selection: &ImportSelection,
    occurrence_id: &str,
) -> Result<(DesignerRuntime, ImportedProjection), DesignerError> {
    validate_selection(workbook, selection)?;
    let mut ids = ImportIds {
        namespace: occurrence_id,
        serial: 0,
    };
    let mut document = Document::empty(ids.generate(SemanticIdKind::Document), "Imported workbook");
    let mut metadata = InteropMetadata {
        version: 1,
        sheets: Vec::new(),
    };
    let mut formulas = Vec::new();
    let mut ledger = workbook.ledger.clone();
    for (sheet_index, sheet) in workbook.sheets.iter().enumerate() {
        build_import_sheet(
            sheet_index,
            sheet,
            selection,
            &mut ids,
            &mut document,
            &mut metadata,
            &mut formulas,
            &mut ledger,
        )?;
    }
    // Formula slots are private temporary Numbers only. No candidate is
    // exposed until all source formulas bind and calculate through Rust.
    let mut runtime = DesignerRuntime::from_document(document, PREFLIGHT_OCCURRENCE)?;
    let mut pending = BTreeMap::new();
    for (sheet_index, row, column, source) in formulas {
        let target = metadata_target(&metadata, sheet_index, row, column)?;
        let (source, references) = translate_a1(
            &source,
            &metadata,
            &workbook.sheets,
            sheet_index,
            &target,
            runtime.session.export_snapshot().document(),
        )?;
        pending.insert(target, (source, references, (sheet_index, row, column)));
    }
    let export_maps = export_a1_maps(runtime.session.export_snapshot().document(), &metadata);
    while !pending.is_empty() {
        let next = pending
            .iter()
            .find(|(_, (_, references, _))| {
                references
                    .iter()
                    .all(|reference| !pending.contains_key(reference))
            })
            .map(|(target, _)| target.clone())
            .ok_or_else(|| tracker_error("source formulas contain a dependency cycle"))?;
        let (source, _, (sheet_index, row, column)) = pending
            .remove(&next)
            .ok_or_else(|| tracker_error("pending import formula is unavailable"))?;
        let sheet = &workbook.sheets[sheet_index];
        let location = format!(
            "{}!{}{}",
            sheet.name,
            super::interop_adapter::column_name(column),
            sheet.layout.rows_above_header as usize + usize::from(sheet.has_header) + row + 1
        );
        bind_import_formula(
            &mut runtime,
            &next,
            &source,
            &sheet.rows[row][column],
            &location,
            &export_maps,
            &mut ledger,
        )?;
    }
    let document = runtime.session.export_snapshot().into_document();
    // The temporary occurrence has finished binding formulas. Its detached
    // snapshot owns the complete document; release the old runtime before
    // validating and projecting the final occurrence.
    drop(runtime);
    let runtime = DesignerRuntime::from_document(document, occurrence_id)?;
    if runtime.text_capacity && !super::interop_adapter::text_capacity_shape(workbook) {
        return Err(tracker_error(
            "Text capacity requires the complete original source profile",
        ));
    }
    normalize_import_date_styles(workbook, &mut metadata, runtime.text_capacity);
    validate_import_metadata_for_profile(
        runtime.session.export_snapshot().document(),
        &metadata,
        runtime.text_capacity,
    )?;
    finish_import(runtime, workbook, metadata, ledger, occurrence_id)
}

fn finish_import(
    mut runtime: DesignerRuntime,
    workbook: &SourceWorkbook,
    mut metadata: InteropMetadata,
    mut ledger: Vec<FidelityFinding>,
    occurrence_id: &str,
) -> Result<(DesignerRuntime, ImportedProjection), DesignerError> {
    // Admission covers the final typed candidate, including user-added columns
    // and bound formulas. Reuse the same representation proof as saved-project
    // inspection and export before exposing or installing this occurrence.
    let exported = match runtime.export_workbook(runtime.current_revision(), &metadata) {
        Ok(exported) => exported,
        Err(error)
            if !runtime.text_capacity && super::interop_adapter::text_capacity_shape(workbook) =>
        {
            let candidate = DesignerRuntime::from_document_profile(
                runtime.session.export_snapshot().into_document(),
                occurrence_id,
                true,
            )?;
            normalize_import_date_styles(workbook, &mut metadata, candidate.text_capacity);
            let exported = candidate
                .export_workbook(candidate.current_revision(), &metadata)
                .map_err(|_| error)?;
            runtime = candidate;
            exported
        }
        Err(error) => return Err(error),
    };
    if runtime.text_capacity {
        let profile = runtime.imported_output_profile();
        let encoded = super::interop_adapter::export_xlsx_for_profile(&exported, profile)
            .map_err(interop_admission_error)?;
        super::interop_adapter::import_xlsx(&encoded).map_err(interop_admission_error)?;
        super::interop_adapter::export_csv_for_profile(&exported.sheets[0], profile)
            .map_err(interop_admission_error)?;
    }
    drop(exported);
    let bootstrap = runtime.bootstrap_projection();
    let table = runtime.query_table(&bootstrap.default_collection)?;
    ledger.push(FidelityFinding {
        category: FidelityCategory::Converted, code: "stable_reference_binding".into(), location: "workbook".into(),
        message: "Worksheet coordinates and selected types were accepted as stable semantic identities; same-row references export as relative A1 so a spreadsheet fill-down follows the row, and other references as absolute coordinates to those identities.".into(), blocking: false,
    });
    let mut projection = ImportedProjection {
        opened: OpenedProjection { bootstrap, table },
        metadata,
        ledger,
    };
    if runtime.text_capacity {
        super::ensure_complete_projection_size(
            "imported",
            &projection,
            super::MAX_TEXT_CAPACITY_PROJECTION_BYTES,
        )?;
    } else if let Err(original_error) =
        super::ensure_complete_projection_size("imported", &projection, super::MAX_PROJECTION_BYTES)
    {
        // Promotion is candidate-local and requires complete actual source
        // presentation. A generic import that already fits keeps its profile.
        if !super::interop_adapter::text_capacity_shape(workbook) {
            return Err(original_error);
        }
        let candidate = DesignerRuntime::from_document_profile(
            runtime.session.export_snapshot().into_document(),
            occurrence_id,
            true,
        )?;
        normalize_import_date_styles(workbook, &mut projection.metadata, candidate.text_capacity);
        let exported =
            candidate.export_workbook(candidate.current_revision(), &projection.metadata)?;
        let bytes = super::interop_adapter::export_xlsx_for_profile(
            &exported,
            candidate.imported_output_profile(),
        )
        .map_err(interop_admission_error)?;
        super::interop_adapter::import_xlsx(&bytes).map_err(interop_admission_error)?;
        projection.opened = OpenedProjection {
            bootstrap: candidate.bootstrap_projection(),
            table: candidate.query_table(&candidate.default_collection)?,
        };
        super::ensure_complete_projection_size(
            "imported",
            &projection,
            super::MAX_TEXT_CAPACITY_PROJECTION_BYTES,
        )?;
        runtime = candidate;
    }
    Ok((runtime, projection))
}

fn validate_selection(
    workbook: &SourceWorkbook,
    selection: &ImportSelection,
) -> Result<(), DesignerError> {
    let capacity = super::interop_adapter::output_profile(workbook) == OutputProfile::TextCapacity;
    let capacity_selection = workbook.sheets.first().is_some_and(|sheet| {
        selection.column_types.len() == 1
            && selection.column_types[0].len() == sheet.columns.len()
            && selection.column_types[0].iter().all(|kind| {
                matches!(
                    kind,
                    ImportFieldType::Text
                        | ImportFieldType::Number
                        | ImportFieldType::Date
                        | ImportFieldType::Boolean
                )
            })
            && selection.extra_columns == vec![Vec::new()]
    });
    if capacity && (!super::interop_adapter::text_capacity_shape(workbook) || !capacity_selection) {
        return Err(tracker_error(
            "8,406-row capacity requires selected Text, Number, Date or Boolean fields and no extra columns",
        ));
    }
    super::interop_adapter::validate_import_geometry(workbook).map_err(interop_admission_error)?;
    if workbook.ledger.iter().any(|finding| finding.blocking) {
        return Err(tracker_error("blocking fidelity findings prevent import"));
    }
    if workbook.sheets.is_empty()
        || selection.column_types.len() != workbook.sheets.len()
        || selection.extra_columns.len() != workbook.sheets.len()
    {
        return Err(tracker_error(
            "import sheet and selection dimensions do not match the declared profile",
        ));
    }
    let mut names = BTreeSet::new();
    let mut formulas = 0;
    for (index, sheet) in workbook.sheets.iter().enumerate() {
        check_label(&sheet.name)?;
        if !names.insert(sheet.name.to_ascii_lowercase())
            || sheet.columns.is_empty()
            || selection.column_types[index].len() != sheet.columns.len()
        {
            return Err(tracker_error(
                "import sheet names, columns, rows, or selected types are invalid",
            ));
        }
        super::interop_adapter::check_import_limit(
            super::CapacityLimit::TableFields,
            if capacity {
                MAX_TEXT_CAPACITY_COLUMNS
            } else {
                MAX_COLUMNS
            },
            sheet.columns.len() + selection.extra_columns[index].len(),
            Some(&sheet.name),
            false,
        )
        .map_err(interop_admission_error)?;
        let mut columns = BTreeSet::new();
        for name in sheet.columns.iter().map(|column| &column.name).chain(
            selection.extra_columns[index]
                .iter()
                .map(|column| &column.name),
        ) {
            check_label(name)?;
            if !columns.insert(name) {
                return Err(tracker_error("import column labels must be unique"));
            }
        }
        for row in &sheet.rows {
            if row.len() > sheet.columns.len() {
                return Err(tracker_error("source row exceeds declared columns"));
            }
            formulas += row.iter().filter(|cell| cell.formula.is_some()).count();
        }
    }
    super::interop_adapter::check_import_limit(
        super::CapacityLimit::Formulas,
        super::capacity::DEFAULT_LIMITS.formulas,
        formulas,
        None,
        false,
    )
    .map_err(interop_admission_error)?;
    if serde_json::to_vec(workbook)
        .map_err(|_| tracker_error("source workbook is not encodable"))?
        .len()
        > super::interop_adapter::MAX_EXPANDED_BYTES
    {
        return Err(tracker_error(
            "source workbook exceeds the bounded expanded profile",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)] // One sheet builder shares one bounded import transaction.
fn build_import_sheet(
    index: usize,
    source: &SourceSheet,
    selection: &ImportSelection,
    ids: &mut ImportIds<'_>,
    document: &mut Document,
    metadata: &mut InteropMetadata,
    formulas: &mut Vec<(usize, usize, usize, String)>,
    ledger: &mut Vec<FidelityFinding>,
) -> Result<(), DesignerError> {
    let schema_id = SchemaId::from(ids.generate(SemanticIdKind::Schema));
    let mut schema = Schema {
        field_order: None,
        id: schema_id.clone(),
        key: SchemaKey::from(format!("sheet_{}", index + 1)),
        display_name: None,
        fields: BTreeMap::new(),
    };
    let mut sheet = InteropSheetMetadata {
        schema_id: schema_id.to_string(),
        name: source.name.clone(),
        has_header: source.has_header,
        columns: Vec::new(),
        rows: Vec::new(),
    };
    let columns = source
        .columns
        .iter()
        .zip(&selection.column_types[index])
        .map(|(column, kind)| {
            (
                column.name.clone(),
                column.width,
                *kind,
                column.number_format.clone(),
            )
        })
        .chain(
            selection.extra_columns[index]
                .iter()
                .map(|column| (column.name.clone(), None, column.field_type, None)),
        );
    for (column, (name, width, kind, number_format)) in columns.enumerate() {
        if kind.field_type() != FieldType::Number && number_format.is_some() {
            ledger.push(FidelityFinding {
                category: FidelityCategory::LossyOnExport,
                code: "number_format_not_applied".into(),
                location: format!("{}:column {}", source.name, column + 1),
                message: "The selected field type is not Number, so the imported XLSX display format was not stored.".into(),
                blocking: false,
            });
        }
        let number_format = (kind.field_type() == FieldType::Number)
            .then_some(number_format)
            .flatten();
        let id = FieldId::from(ids.generate(SemanticIdKind::Field));
        schema.fields.insert(
            id.clone(),
            FieldDefinition {
                id: id.clone(),
                key: FieldKey::from(format!("column_{}", column + 1)),
                display_name: None,
                field_type: kind.field_type(),
                required: false,
                constraint: tachiko_workspace_engine::FieldConstraint::None,
                number_format,
            },
        );
        sheet.columns.push(InteropColumnMetadata {
            field_id: id.to_string(),
            name,
            width,
        });
    }
    for (row_index, cells) in source.rows.iter().enumerate() {
        let entity_id = EntityId::from(ids.generate(SemanticIdKind::Entity));
        let mut entity = Entity {
            id: entity_id.clone(),
            key: EntityKey::from(format!("sheet_{}_row_{}", index + 1, row_index + 1)),
            schema: schema_id.clone(),
            fields: BTreeMap::new(),
        };
        let mut styles = Vec::new();
        for (column, cell) in cells.iter().enumerate() {
            let kind = selection.column_types[index][column].field_type();
            let value = if let Some(formula) = &cell.formula {
                if kind != FieldType::Number {
                    return Err(tracker_error(
                        "source formulas require explicit Number column selection",
                    ));
                }
                formulas.push((index, row_index, column, formula.clone()));
                Some(Value::Number(Number::new(0.0).expect("zero is finite")))
            } else {
                let value = source_value(&cell.value)?;
                let converted = value
                    .as_ref()
                    .map(|value| convert_value(value, &kind))
                    .transpose()?;
                if converted != value {
                    ledger.push(FidelityFinding { category: FidelityCategory::Converted, code: "explicit_column_type".into(), location: format!("{}:{}:{}", source.name, row_index + 1, column + 1), message: "Source scalar converted using the explicitly selected column type; original source remains available.".into(), blocking: false });
                }
                converted
            };
            if let Some(value) = value {
                entity
                    .fields
                    .insert(FieldId::from(sheet.columns[column].field_id.clone()), value);
            }
            let mut style = cell.style.clone();
            if source.columns[column].number_format.is_some()
                && (cell.formula.is_some()
                    || matches!(cell.value, SourceValue::Number { .. } | SourceValue::Empty))
            {
                // The promoted column format now owns numeric presentation in
                // this column, blanks included (a number typed there later is
                // the column's); retaining the source copy would bring a later
                // cleared format back on export.
                style.number_format = None;
            }
            styles.push(style);
        }
        styles.resize_with(sheet.columns.len(), CellStyle::default);
        sheet.rows.push(InteropRowMetadata {
            entity_id: entity_id.to_string(),
            styles,
        });
        document.entities.insert(entity_id, entity);
    }
    document.schemas.insert(schema_id, schema);
    metadata.sheets.push(sheet);
    Ok(())
}

fn normalize_import_date_styles(
    workbook: &SourceWorkbook,
    metadata: &mut InteropMetadata,
    text_capacity: bool,
) {
    if !text_capacity {
        return;
    }
    // The capacity writer supplies this Date format; only the admitted
    // runtime profile may require default styles in its metadata.
    for (sheet, source) in metadata.sheets.iter_mut().zip(&workbook.sheets) {
        for (row, cells) in sheet.rows.iter_mut().zip(&source.rows) {
            for (style, cell) in row.styles.iter_mut().zip(cells) {
                if matches!(cell.value, SourceValue::Date { .. })
                    && *style == super::interop_adapter::capacity_date_style()
                {
                    *style = CellStyle::default();
                }
            }
        }
    }
}

fn source_value(value: &SourceValue) -> Result<Option<Value>, DesignerError> {
    Ok(match value {
        SourceValue::Empty => None,
        SourceValue::Text { value } => Some(Value::Text(value.clone())),
        SourceValue::Number { value } => Some(Value::Number(
            Number::new(*value).map_err(|_| tracker_error("source Number is not finite"))?,
        )),
        SourceValue::Boolean { value } => Some(Value::Boolean(*value)),
        SourceValue::Date { value } => Some(Value::Date(
            Date::parse(value).map_err(|_| tracker_error("source Date is invalid"))?,
        )),
    })
}

fn metadata_target(
    metadata: &InteropMetadata,
    sheet: usize,
    row: usize,
    column: usize,
) -> Result<FieldRef, DesignerError> {
    let sheet = metadata
        .sheets
        .get(sheet)
        .ok_or_else(|| tracker_error("formula worksheet is unavailable"))?;
    Ok(FieldRef::new(
        sheet
            .rows
            .get(row)
            .ok_or_else(|| tracker_error("formula row is outside source data"))?
            .entity_id
            .clone(),
        sheet
            .columns
            .get(column)
            .ok_or_else(|| tracker_error("formula column is outside source data"))?
            .field_id
            .clone(),
    ))
}

pub(crate) fn quoted_sheet_name(source: &str, cursor: &mut usize) -> Result<String, DesignerError> {
    *cursor += 1;
    let mut name = String::new();
    loop {
        let next = source[*cursor..]
            .chars()
            .next()
            .ok_or_else(|| tracker_error("unterminated quoted worksheet name"))?;
        *cursor += next.len_utf8();
        if next == '\'' {
            if source[*cursor..].starts_with('\'') {
                name.push('\'');
                *cursor += 1;
            } else {
                break;
            }
        } else {
            name.push(next);
        }
    }
    if !source[*cursor..].starts_with('!') {
        return Err(tracker_error(
            "quoted formula token is not a worksheet reference",
        ));
    }
    *cursor += 1;
    Ok(name)
}

/// What one cell compares as when it's a criterion: a Text cell's type comes
/// from its content, as the engine reads it, and any other cell's from its field.
pub(crate) fn criterion_type(
    field_type: Option<&FieldType>,
    value: Option<&Value>,
) -> Option<FieldType> {
    let Some(Value::Text(text)) = value else {
        return field_type.cloned();
    };
    match CriterionTest::parse(text).map(|test| test.operand_kind()) {
        Ok(CriterionOperandKind::Number) => Some(FieldType::Number),
        Ok(CriterionOperandKind::Date) => Some(FieldType::Date),
        _ => field_type.cloned(),
    }
}

/// The A1 address of every exported cell and one whole-column range per
/// exported column, as the imported-workbook export writes them.
fn export_a1_maps(
    document: &Document,
    metadata: &InteropMetadata,
) -> (
    BTreeMap<FieldRef, ExportCell>,
    BTreeMap<ColumnRef, ExportColumn>,
) {
    let mut addresses = BTreeMap::new();
    // One whole-column range per exported column, so a column total keeps
    // meaning every exported row instead of the rows present today.
    let mut columns = BTreeMap::new();
    for sheet in &metadata.sheets {
        let present: Vec<_> = sheet
            .rows
            .iter()
            .filter(|row| {
                document
                    .entities
                    .contains_key(&EntityId::from(row.entity_id.clone()))
            })
            .collect();
        let first = 1 + usize::from(sheet.has_header);
        let last = present.len() + usize::from(sheet.has_header);
        let quoted = sheet.name.replace('\'', "''");
        for (column, definition) in sheet.columns.iter().enumerate() {
            let letters = column_letters(column);
            // An exported table without rows maps its columns to no range.
            let range = if present.is_empty() {
                String::new()
            } else {
                format!("'{quoted}'!${letters}${first}:${letters}${last}")
            };
            let field_type = document
                .schemas
                .get(sheet.schema_id.as_str())
                .and_then(|schema| schema.fields.get(definition.field_id.as_str()))
                .map(|field| field.field_type.clone());
            columns.insert(
                ColumnRef::new(sheet.schema_id.clone(), definition.field_id.clone()),
                ExportColumn::new(range, field_type),
            );
        }
        for (row_index, row) in present.iter().enumerate() {
            for (column, definition) in sheet.columns.iter().enumerate() {
                addresses.insert(
                    FieldRef::new(row.entity_id.clone(), definition.field_id.clone()),
                    ExportCell::new(
                        quoted.clone(),
                        column_letters(column),
                        row_index + 1 + usize::from(sheet.has_header),
                        criterion_type(
                            document
                                .schemas
                                .get(sheet.schema_id.as_str())
                                .and_then(|schema| schema.fields.get(definition.field_id.as_str()))
                                .map(|field| &field.field_type),
                            document
                                .entities
                                .get(row.entity_id.as_str())
                                .and_then(|entity| entity.fields.get(definition.field_id.as_str())),
                        ),
                    ),
                );
            }
        }
    }
    (addresses, columns)
}

// Bind and guard without publishing an unguarded intermediate calculation.
fn bind_import_formula(
    runtime: &mut DesignerRuntime,
    target: &FieldRef,
    source: &str,
    cell: &SourceCell,
    location: &str,
    export_maps: &(
        BTreeMap<FieldRef, ExportCell>,
        BTreeMap<ColumnRef, ExportColumn>,
    ),
    ledger: &mut Vec<FidelityFinding>,
) -> Result<(), DesignerError> {
    let snapshot = runtime.session.export_snapshot();
    let unbound = parse_expression(source).map_err(|error| {
        tachiko_workspace_engine::WorkspaceError::InvalidFormula {
            field: target.clone(),
            input: source.into(),
            source: error,
        }
    })?;
    let expression = bind_expression(snapshot.document(), target, &unbound).map_err(|error| {
        tachiko_workspace_engine::WorkspaceError::FormulaBinding {
            field: target.clone(),
            source: Box::new(error),
        }
    })?;
    // Guarding can outgrow any engine bound (nodes, depth or canonical text);
    // each keeps Excel's cached value rather than failing the whole import.
    // The export round trip in `finish_import` writes the same formula as A1
    // text, which has its own length bound. `update_formula` re-parses the
    // projected text, whose parse tree counts nodes (such as ROUND digits)
    // the bound tree does not, so the parser itself decides.
    let projected = match excel_extremum(expression, target, snapshot.document()) {
        Ok(expression) => match project_expression(snapshot.document(), target, &expression) {
            Ok(source)
                if parse_expression(&source).is_err()
                    || export_expression(&expression, target, &export_maps.0, &export_maps.1)
                        .is_ok_and(|text| text.len() > MAX_PROFILE_STRING_BYTES) =>
            {
                None
            }
            Ok(source) => Some(source),
            Err(CanonicalAuthoringProjectionError::Complexity(_)) => None,
            Err(error) => return Err(tracker_error(&error.to_string())),
        },
        Err(_) => None,
    };
    drop(snapshot);
    let Some(source) = projected else {
        return keep_import_cache(runtime, target, cell, location, ledger);
    };
    let revision = runtime.current_revision().to_owned();
    runtime.update_formula(&revision, &field_target(target), &source)?;
    Ok(())
}

/// An imported formula Tachiko cannot hold after guarding keeps Excel's cached
/// Number in its temporary formula slot.
fn keep_import_cache(
    runtime: &mut DesignerRuntime,
    target: &FieldRef,
    cell: &SourceCell,
    location: &str,
    ledger: &mut Vec<FidelityFinding>,
) -> Result<(), DesignerError> {
    let mut value = cell.value.clone();
    let mut formula = cell
        .formula
        .as_ref()
        .map(|source| source.strip_prefix('=').unwrap_or(source).to_owned());
    super::interop_adapter::keep_cached_value(
        &mut value,
        &mut formula,
        location,
        ledger,
        Some("guarding blank cells for MIN/MAX makes the formula larger than Tachiko allows"),
    );
    if let Some(finding) = ledger.last().filter(|finding| finding.blocking) {
        return Err(DesignerError::ImportedSpreadsheetExport {
            finding: finding.clone(),
        });
    }
    let SourceValue::Number { value } = value else {
        return Err(tracker_error("MIN/MAX requires a numeric cached value"));
    };
    // The formula slot already holds a temporary Number; a cached value
    // equal to it needs no edit (which would refuse as no change).
    let unchanged = runtime
        .session
        .export_snapshot()
        .document()
        .entities
        .get(&target.entity)
        .and_then(|entity| entity.fields.get(&target.field))
        .is_some_and(|stored| {
            Number::new(value).is_ok_and(|value| stored == &Value::Number(value))
        });
    if unchanged {
        return Ok(());
    }
    let revision = runtime.current_revision().to_owned();
    runtime.edit_cells(
        &revision,
        &[super::CellEdit {
            target: field_target(target),
            input: ScalarEditInput::Number {
                input: value.to_string(),
            },
        }],
    )?;
    Ok(())
}

// Excel skips blank direct references in MIN/MAX; Tachiko reads them as zero.
// This conversion is only applied to newly imported formulas.
pub(super) fn excel_extremum(
    mut expression: Expression,
    owner: &FieldRef,
    document: &Document,
) -> Result<Expression, ExpressionComplexityError> {
    fn rewrite(
        expression: &mut Expression,
        owner: &FieldRef,
        document: &Document,
        present: &std::collections::BTreeSet<FieldRef>,
    ) -> Result<(), ExpressionComplexityError> {
        // A reference an enclosing ISBLANK guard proved present is not blank
        // in the else branch, so already guarded formulas stay unchanged.
        if let Expression::If {
            condition,
            then_value,
            else_value,
        } = expression
        {
            rewrite(condition, owner, document, present)?;
            rewrite(then_value, owner, document, present)?;
            let present = guarded_present(condition, owner, present);
            return rewrite(else_value, owner, document, &present);
        }
        for child in expression.children_mut() {
            rewrite(child, owner, document, present)?;
        }
        let (Expression::Minimum { left, right } | Expression::Maximum { left, right }) =
            expression
        else {
            return Ok(());
        };
        let blankable = |argument: &Expression| {
            direct_reference(argument, owner)
                .filter(|reference| !present.contains(reference))
                .is_some_and(|reference| {
                    document
                        .entities
                        .get(&reference.entity)
                        .and_then(|entity| document.schemas.get(&entity.schema))
                        .and_then(|schema| schema.fields.get(&reference.field))
                        .is_some_and(|field| {
                            field.field_type == FieldType::Number && !field.required
                        })
                })
        };
        let left_blankable = blankable(left);
        let right_blankable = blankable(right);
        if !left_blankable && !right_blankable {
            return Ok(());
        }
        let left = left.clone();
        let right = right.clone();
        let mut inner = expression.clone();
        if right_blankable {
            inner = Expression::If {
                condition: Box::new(Expression::IsBlank(right.clone())),
                then_value: left.clone(),
                else_value: Box::new(inner),
            };
        }
        if left_blankable {
            inner = Expression::If {
                condition: Box::new(Expression::IsBlank(left)),
                then_value: right,
                else_value: Box::new(inner),
            };
        }
        // Check every expansion before its parent can duplicate it again.
        // The engine owns MAX_EXPRESSION_NODES (256), not this adapter.
        validate_expression_structure(&inner)?;
        *expression = inner;
        Ok(())
    }
    rewrite(
        &mut expression,
        owner,
        document,
        &std::collections::BTreeSet::new(),
    )?;
    validate_expression_structure(&expression)?;
    Ok(expression)
}

#[cfg(test)]
mod extremum_tests {
    use super::{
        BTreeMap, Document, Entity, Expression, FieldDefinition, FieldRef, FieldType, Number,
        Schema, Value, excel_extremum,
    };

    #[test]
    fn required_references_stay_plain_and_optional_same_row_references_are_guarded() {
        let mut document = Document::empty("extremum", "Extremum");
        document.schemas.insert(
            "items".into(),
            Schema {
                id: "items".into(),
                key: "items".into(),
                display_name: None,
                field_order: None,
                fields: BTreeMap::from([(
                    "n".into(),
                    FieldDefinition {
                        id: "n".into(),
                        key: "n".into(),
                        display_name: None,
                        number_format: None,
                        field_type: FieldType::Number,
                        required: true,
                        constraint: tachiko_workspace_engine::FieldConstraint::None,
                    },
                )]),
            },
        );
        document.entities.insert(
            "row".into(),
            Entity {
                id: "row".into(),
                key: "row".into(),
                schema: "items".into(),
                fields: BTreeMap::from([("n".into(), Value::Number(Number::new(3.0).unwrap()))]),
            },
        );
        let owner = FieldRef::new("row", "result");
        for reference in [
            Expression::Reference(FieldRef::new("row", "n")),
            Expression::SameRowReference("n".into()),
        ] {
            let expression = Expression::Minimum {
                left: Box::new(reference),
                right: Box::new(Expression::Number(Number::new(5.0).unwrap())),
            };
            assert_eq!(
                excel_extremum(expression.clone(), &owner, &document).unwrap(),
                expression
            );
        }
        document
            .schemas
            .get_mut("items")
            .unwrap()
            .fields
            .get_mut("n")
            .unwrap()
            .required = false;
        let expression = Expression::Maximum {
            left: Box::new(Expression::SameRowReference("n".into())),
            right: Box::new(Expression::Number(Number::new(5.0).unwrap())),
        };
        assert!(matches!(
            excel_extremum(expression, &owner, &document).unwrap(),
            Expression::If { .. }
        ));
    }
}

// Both explicit A1 boundaries share tokenization; geometry owns address resolution.
pub(super) trait A1Geometry {
    fn sheet(&self, name: &str) -> Result<usize, DesignerError>;
    fn cell(
        &mut self,
        sheet: usize,
        address: &str,
        document: &Document,
        references: &mut BTreeSet<FieldRef>,
    ) -> Result<String, DesignerError>;
    fn aggregate(
        &mut self,
        argument: &str,
        current: usize,
        aggregate: SummaryAggregate,
        document: &Document,
        references: &mut BTreeSet<FieldRef>,
    ) -> Result<String, DesignerError>;
    /// One column argument of a conditional aggregate, as `[schema.field:column]`.
    /// `numeric` is true for the value column.
    fn column(
        &mut self,
        argument: &str,
        current: usize,
        numeric: bool,
        document: &Document,
        references: &mut BTreeSet<FieldRef>,
    ) -> Result<String, DesignerError>;
    /// Start one conditional call so `column` can enforce its same-rows rule
    /// across that call's column arguments.
    fn begin_conditional(&mut self) {}
    fn record_token(
        &mut self,
        _start: usize,
        _end: usize,
        _output_start: usize,
        _output_end: usize,
    ) {
    }
    fn unknown_function(&self) -> DesignerError {
        tracker_error("unsupported source function")
    }
    fn locate(&self, error: DesignerError, _start: usize, _end: usize) -> DesignerError {
        error
    }
    fn operator(&self, _ch: char) -> Result<(), DesignerError> {
        Ok(())
    }
}

struct ImportA1Geometry<'a> {
    metadata: &'a InteropMetadata,
    sources: &'a [SourceSheet],
    owner: &'a FieldRef,
    /// The first conditional column's sheet and row span, for the same-rows rule.
    conditional_span: Option<(usize, usize, usize)>,
}

impl A1Geometry for ImportA1Geometry<'_> {
    fn sheet(&self, name: &str) -> Result<usize, DesignerError> {
        self.metadata
            .sheets
            .iter()
            .position(|sheet| sheet.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| tracker_error("formula worksheet name is unavailable"))
    }
    fn aggregate(
        &mut self,
        argument: &str,
        current: usize,
        aggregate: SummaryAggregate,
        document: &Document,
        references: &mut BTreeSet<FieldRef>,
    ) -> Result<String, DesignerError> {
        let (sheet, column, _, _) = whole_column_argument(argument, current, self.sources, true)
            .ok_or_else(|| tracker_error("aggregate range must cover one whole data column"))?;
        column_source(
            aggregate.suffix(),
            column,
            sheet,
            self.metadata,
            document,
            references,
        )
    }
    fn column(
        &mut self,
        argument: &str,
        current: usize,
        numeric: bool,
        document: &Document,
        references: &mut BTreeSet<FieldRef>,
    ) -> Result<String, DesignerError> {
        let (sheet, column, first_row, last_row) =
            whole_column_argument(argument, current, self.sources, numeric).ok_or_else(|| {
                tracker_error("conditional range must cover one whole data column")
            })?;
        let span = (sheet, first_row, last_row);
        if self.conditional_span.is_some_and(|first| first != span) {
            return Err(tracker_error(
                "conditional aggregate columns must cover the same rows",
            ));
        }
        self.conditional_span = Some(span);
        column_source("column", column, sheet, self.metadata, document, references)
    }
    fn begin_conditional(&mut self) {
        self.conditional_span = None;
    }
    fn cell(
        &mut self,
        sheet_index: usize,
        address: &str,
        document: &Document,
        references: &mut BTreeSet<FieldRef>,
    ) -> Result<String, DesignerError> {
        let metadata = self.metadata;
        let sources = self.sources;
        let (column, sheet_row) = parse_a1_address(address)?;
        let layout = &sources
            .get(sheet_index)
            .ok_or_else(|| tracker_error("formula worksheet is unavailable"))?
            .layout;
        if let Some(summary) = layout
            .summary_row
            .as_ref()
            .filter(|summary| usize::try_from(summary.row).is_ok_and(|row| row == sheet_row + 1))
        {
            return column_source(
                summary_cell_aggregate(summary, column)?.suffix(),
                column,
                sheet_index,
                metadata,
                document,
                references,
            );
        }
        let above = usize::try_from(layout.rows_above_header)
            .map_err(|_| tracker_error("header offset exceeds the row bound"))?;
        let row = sheet_row
            .checked_sub(above + usize::from(metadata.sheets[sheet_index].has_header))
            .ok_or_else(|| tracker_error("formula cannot bind a header or title cell"))?;
        let reference = metadata_target(metadata, sheet_index, row, column)?;
        // A row-relative reference to the owner's own row keeps Excel's
        // fill-down meaning as a same-row Tachiko reference.
        let same_row = reference.entity == self.owner.entity && !row_is_absolute(address);
        // The shared binder checks numeric uses after translation, when it
        // can distinguish an ISBLANK presence probe from a numeric operand.
        declared_type(document, &reference)?;
        let entity = &document.entities[&reference.entity];
        let field = &document.schemas[&entity.schema].fields[&reference.field];
        references.insert(reference);
        Ok(format!(
            "[{}.{}]",
            if same_row { "" } else { entity.key.as_str() },
            field.key.as_str()
        ))
    }
}

fn translate_a1(
    source: &str,
    metadata: &InteropMetadata,
    sources: &[SourceSheet],
    current_sheet: usize,
    owner: &FieldRef,
    document: &Document,
) -> Result<(String, BTreeSet<FieldRef>), DesignerError> {
    // Import counts the optional leading equals sign in its existing input bound.
    if source.len() > MAX_PROFILE_STRING_BYTES {
        return Err(tracker_error(
            "source formula exceeds the bounded authoring limit",
        ));
    }
    translate_a1_with(
        source.strip_prefix('=').unwrap_or(source),
        &mut ImportA1Geometry {
            metadata,
            sources,
            owner,
            conditional_span: None,
        },
        current_sheet,
        document,
    )
}

pub(super) fn translate_a1_with(
    source: &str,
    geometry: &mut impl A1Geometry,
    current_sheet: usize,
    document: &Document,
) -> Result<(String, BTreeSet<FieldRef>), DesignerError> {
    translate_a1_inner(source, geometry, current_sheet, document, true)
}

#[allow(clippy::too_many_lines)] // One bounded token loop shared by import and the read-only kit call.
fn translate_a1_inner(
    source: &str,
    geometry: &mut impl A1Geometry,
    current_sheet: usize,
    document: &Document,
    record_tokens: bool,
) -> Result<(String, BTreeSet<FieldRef>), DesignerError> {
    if source.len() > MAX_PROFILE_STRING_BYTES {
        return Err(tracker_error(
            "source formula exceeds the bounded authoring limit",
        ));
    }
    let mut output = String::new();
    let mut references = BTreeSet::new();
    let mut cursor = 0;
    while cursor < source.len() {
        let ch = source[cursor..]
            .chars()
            .next()
            .expect("cursor stays on characters");
        let token_start = cursor;
        let output_start = output.len();
        let result = (|| -> Result<(), DesignerError> {
            if ch.is_alphabetic() || ch == '_' || ch == '$' || ch == '\'' {
                let start = cursor;
                let sheet_name = if ch == '\'' {
                    Some(quoted_sheet_name(source, &mut cursor)?)
                } else {
                    None
                };
                let token_start = cursor;
                while let Some(next) = source[cursor..].chars().next() {
                    if !(next.is_alphanumeric() || "_$!.".contains(next)) {
                        break;
                    }
                    cursor += next.len_utf8();
                }
                let token = &source[token_start..cursor];
                if sheet_name.is_none() && !token.contains('!') {
                    let name = token.to_ascii_uppercase();
                    let aggregate = match name.as_str() {
                        "SUM" => Some(SummaryAggregate::Total),
                        "AVERAGE" => Some(SummaryAggregate::Average),
                        "COUNT" => Some(SummaryAggregate::Count),
                        _ => None,
                    };
                    if let Some(aggregate) = aggregate {
                        let call = skip_spaces(source, cursor);
                        if source[call..].starts_with('(') {
                            let (argument, consumed) = call_argument(&source[call..])
                                .ok_or_else(|| tracker_error("unterminated source formula call"))?;
                            cursor = call + consumed;
                            output.push_str(&geometry.aggregate(
                                argument,
                                current_sheet,
                                aggregate,
                                document,
                                &mut references,
                            )?);
                            cursor = call + consumed;
                            return Ok(());
                        }
                    }
                    if matches!(
                        name.as_str(),
                        "MIN" | "MAX" | "IF" | "AND" | "OR" | "NOT" | "ROUND" | "ISBLANK"
                    ) {
                        output.push_str(&name.to_ascii_lowercase());
                        return Ok(());
                    }
                    if matches!(
                        name.as_str(),
                        "SUMIF" | "SUMIFS" | "AVERAGEIF" | "AVERAGEIFS" | "COUNTIF" | "COUNTIFS"
                    ) {
                        let call = skip_spaces(source, cursor);
                        if source[call..].starts_with('(') {
                            let (argument, consumed) = call_argument(&source[call..])
                                .ok_or_else(|| tracker_error("unterminated source formula call"))?;
                            output.push_str(&translate_conditional_call(
                                &name,
                                argument,
                                call + 1,
                                current_sheet,
                                geometry,
                                document,
                                &mut references,
                            )?);
                            cursor = call + consumed;
                            return Ok(());
                        }
                    }
                }
                if source[skip_spaces(source, cursor)..].starts_with('(') {
                    return Err(geometry.unknown_function());
                }
                let (sheet_name, address) = if let Some(name) = sheet_name {
                    (Some(name), token)
                } else if let Some((name, address)) = token.split_once('!') {
                    (Some(name.to_owned()), address)
                } else {
                    (None, token)
                };
                let sheet_index = if let Some(name) = sheet_name {
                    geometry.sheet(&name)?
                } else {
                    current_sheet
                };
                if cursor == start {
                    return Err(tracker_error("invalid source formula token"));
                }
                output.push_str(&geometry.cell(sheet_index, address, document, &mut references)?);
            } else if ch.is_ascii_digit() || ch == '.' {
                // Keep numeric spelling for the existing Rust parser, including
                // scientific notation; never reinterpret an exponent as A1.
                let start = cursor;
                cursor += ch.len_utf8();
                while cursor < source.len()
                    && (source.as_bytes()[cursor].is_ascii_digit()
                        || source.as_bytes()[cursor] == b'.')
                {
                    cursor += 1;
                }
                if cursor < source.len() && matches!(source.as_bytes()[cursor], b'e' | b'E') {
                    cursor += 1;
                    if cursor < source.len() && matches!(source.as_bytes()[cursor], b'+' | b'-') {
                        cursor += 1;
                    }
                    while cursor < source.len() && source.as_bytes()[cursor].is_ascii_digit() {
                        cursor += 1;
                    }
                }
                output.push_str(&source[start..cursor]);
            } else {
                geometry.operator(ch)?;
                output.push(ch);
                cursor += ch.len_utf8();
            }
            Ok(())
        })();
        // A conditional criterion is translated against its own slice, so an
        // already located refusal keeps its exact original-text span instead of
        // being widened to the whole call.
        result.map_err(|error| {
            if matches!(
                &error,
                DesignerError::A1Translation { span, .. } if span.start != 0 || span.end != 0
            ) {
                error
            } else {
                geometry.locate(error, token_start, cursor.max(token_start + ch.len_utf8()))
            }
        })?;
        if record_tokens {
            geometry.record_token(token_start, cursor, output_start, output.len());
        }
    }
    Ok((output, references))
}

/// Translate one `SUMIF`-family call to canonical Tachiko text, keeping the
/// caller's function spelling. `base` is the call argument's offset in the
/// formula being translated.
fn translate_conditional_call(
    name: &str,
    argument: &str,
    base: usize,
    current_sheet: usize,
    geometry: &mut impl A1Geometry,
    document: &Document,
    references: &mut BTreeSet<FieldRef>,
) -> Result<String, DesignerError> {
    let parts = split_arguments(argument)
        .ok_or_else(|| tracker_error("unterminated source formula call"))?;
    let shape = conditional_shape(name, parts.len()).ok_or_else(|| geometry.unknown_function())?;
    geometry.begin_conditional();
    let mut rendered = vec![String::new(); parts.len()];
    for &(index, numeric) in &shape.columns {
        rendered[index] = geometry.column(
            parts[index].1.trim(),
            current_sheet,
            numeric,
            document,
            references,
        )?;
    }
    for &(criterion_index, column_index) in &shape.criteria {
        let (offset, part) = parts[criterion_index];
        let text = part.trim();
        let start = base + offset + (part.len() - part.trim_start().len());
        let (criterion, kind) = translate_conditional_criterion(
            text,
            start,
            current_sheet,
            geometry,
            document,
            references,
        )?;
        // Excel compares dates by serial number, Tachiko by date, so a number
        // cannot test a date column and a date only tests a date column.
        let date = column_field_type(&rendered[column_index], document) == Some(FieldType::Date);
        if (kind == CriterionKind::Number && date) || (kind == CriterionKind::Date && !date) {
            return Err(DesignerError::A1Translation {
                code: A1RefusalCode::FormulaTypeMismatch,
                span: A1TokenSpan {
                    start,
                    end: start + text.len(),
                },
            });
        }
        rendered[criterion_index] = criterion;
    }
    Ok(format!("{name}({})", rendered.join(",")))
}

/// One criterion: a written literal stays verbatim when the formula parser
/// accepts it, and anything else must translate to a number or one cell. XLSX
/// import and export carry only those forms.
fn translate_conditional_criterion(
    text: &str,
    base: usize,
    current_sheet: usize,
    geometry: &mut impl A1Geometry,
    document: &Document,
    references: &mut BTreeSet<FieldRef>,
) -> Result<(String, CriterionKind), DesignerError> {
    let unsupported = || DesignerError::A1Translation {
        code: A1RefusalCode::UnsupportedCriterion,
        span: A1TokenSpan {
            start: base,
            end: base + text.len(),
        },
    };
    if criterion_literal(text) {
        let kind = writable_criterion(text).ok_or_else(unsupported)?;
        return Ok((text.to_owned(), kind));
    }
    let (translated, nested) = translate_a1_inner(text, geometry, current_sheet, document, false)
        .map_err(|error| offset_translation(error, base))?;
    let kind = translated_criterion_kind(&translated, &nested, document).ok_or_else(unsupported)?;
    references.extend(nested);
    Ok((translated, kind))
}

/// Shift a nested translation's source span to its place in the original text.
fn offset_translation(error: DesignerError, base: usize) -> DesignerError {
    match error {
        DesignerError::A1Translation { code, span } => DesignerError::A1Translation {
            code,
            span: A1TokenSpan {
                start: base + span.start,
                end: base + span.end,
            },
        },
        other => other,
    }
}

/// The value class of a translated criterion that is a number or exactly one
/// reference, or `None` for any other expression.
fn translated_criterion_kind(
    translated: &str,
    references: &BTreeSet<FieldRef>,
    document: &Document,
) -> Option<CriterionKind> {
    if writable_criterion(translated) == Some(CriterionKind::Number) {
        return Some(CriterionKind::Number);
    }
    let inner = translated.strip_prefix('[')?.strip_suffix(']')?;
    if inner.contains(['[', ']']) || references.len() != 1 {
        return None;
    }
    let reference = references.iter().next()?;
    let entity = document.entities.get(&reference.entity)?;
    let field_type = document
        .schemas
        .get(&entity.schema)
        .and_then(|schema| schema.fields.get(&reference.field))
        .map(|field| &field.field_type);
    Some(
        match criterion_type(field_type, entity.fields.get(&reference.field)) {
            Some(FieldType::Number) => CriterionKind::Number,
            Some(FieldType::Date) => CriterionKind::Date,
            _ => CriterionKind::Other,
        },
    )
}

/// The declared type of one translated column argument,
/// `[schema.field:column]`.
fn column_field_type(translated: &str, document: &Document) -> Option<FieldType> {
    let inner = translated.strip_prefix('[')?.strip_suffix(":column]")?;
    let (schema_key, field_key) = inner.split_once('.')?;
    document
        .schemas
        .values()
        .find(|schema| schema.key.as_str() == schema_key)?
        .fields
        .values()
        .find(|field| field.key.as_str() == field_key)
        .map(|field| field.field_type.clone())
}

fn skip_spaces(source: &str, mut cursor: usize) -> usize {
    while let Some(character) = source[cursor..].chars().next() {
        if !character.is_whitespace() {
            break;
        }
        cursor += character.len_utf8();
    }
    cursor
}

/// The aggregate a source formula reads from one imported summary-row cell.
fn summary_cell_aggregate(
    summary: &SummaryRow,
    column: usize,
) -> Result<SummaryAggregate, DesignerError> {
    summary
        .cells
        .iter()
        .find(|cell| usize::try_from(cell.column).is_ok_and(|index| index == column))
        .map(|cell| cell.aggregate)
        .ok_or_else(|| {
            tracker_error("formula refers to a cell of a summary row that is not imported")
        })
}

/// The semantic spelling of one sheet column with `suffix`, as
/// `[schema.field:total]` or `[schema.field:column]`, binding every row of that
/// column.
fn column_source(
    suffix: &str,
    column: usize,
    sheet_index: usize,
    metadata: &InteropMetadata,
    document: &Document,
    references: &mut BTreeSet<FieldRef>,
) -> Result<String, DesignerError> {
    let sheet = metadata
        .sheets
        .get(sheet_index)
        .ok_or_else(|| tracker_error("formula worksheet is unavailable"))?;
    let field_id = &sheet
        .columns
        .get(column)
        .ok_or_else(|| tracker_error("formula column is outside source data"))?
        .field_id;
    let schema = document
        .schemas
        .get(&SchemaId::from(sheet.schema_id.clone()))
        .ok_or_else(|| tracker_error("formula worksheet schema is unavailable"))?;
    let field = schema
        .fields
        .get(&FieldId::from(field_id.clone()))
        .ok_or_else(|| tracker_error("formula field is unavailable"))?;
    // Binding after every cell formula of the column keeps a formula column
    // bound before a total that reads it.
    for row in &sheet.rows {
        references.insert(FieldRef::new(row.entity_id.clone(), field_id.clone()));
    }
    Ok(format!(
        "[{}.{}:{}]",
        schema.key.as_str(),
        field.key.as_str(),
        suffix
    ))
}

/// Whether an A1 address anchors its row with `$`, so a copy keeps that row.
pub(super) fn row_is_absolute(address: &str) -> bool {
    address
        .trim_start_matches('$')
        .chars()
        .find(|character| !character.is_ascii_alphabetic())
        == Some('$')
}

pub(super) fn parse_a1_address(address: &str) -> Result<(usize, usize), DesignerError> {
    let address = address.strip_prefix('$').unwrap_or(address);
    let letters = address.bytes().take_while(u8::is_ascii_alphabetic).count();
    if letters == 0 {
        return Err(tracker_error(
            "formula reference is outside plain A1 profile",
        ));
    }
    let column = address[..letters]
        .bytes()
        .try_fold(0usize, |column, letter| {
            column.checked_mul(26).and_then(|column| {
                column.checked_add(usize::from(letter.to_ascii_uppercase() - b'A' + 1))
            })
        })
        .ok_or_else(|| tracker_error("formula column is out of bounds"))?;
    let row = address[letters..]
        .strip_prefix('$')
        .unwrap_or(&address[letters..]);
    if row.is_empty() || !row.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(tracker_error(
            "formula reference is outside plain A1 profile",
        ));
    }
    let row = row
        .parse::<usize>()
        .map_err(|_| tracker_error("formula row is out of bounds"))?;
    Ok((
        column
            .checked_sub(1)
            .ok_or_else(|| tracker_error("invalid A1 column"))?,
        row.checked_sub(1)
            .ok_or_else(|| tracker_error("invalid A1 row"))?,
    ))
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SpreadsheetExportProjection {
    pub revision: String,
    pub byte_length: usize,
    pub ledger: Vec<FidelityFinding>,
}

fn check_label(label: &str) -> Result<(), DesignerError> {
    if label.is_empty() || label.len() > MAX_PROFILE_STRING_BYTES || label.contains('\0') {
        return Err(tracker_error(
            "interop labels must be nonempty bounded text",
        ));
    }
    Ok(())
}

/// File output scope is selected before reconstruction; the neutral projection
/// remains available for source/save closure without claiming Excel fidelity.
#[derive(Clone, Copy)]
pub(crate) enum ImportedOutput<'a> {
    Projection,
    Csv(&'a str),
    Xlsx,
}

/// A durable column order (R4a) outranks the source presentation order.
fn sort_by_field_order(schema: &Schema, columns: &mut [InteropColumnMetadata]) {
    if let Some(order) = &schema.field_order {
        columns.sort_by_key(|column| order.iter().position(|id| id.as_str() == column.field_id));
    }
}

/// Reconcile source presentation with current stable identities. Source slots
/// remain useful for Undo; they never recreate removed fields or entities.
#[allow(clippy::too_many_lines)] // Keep profile-aware identity and presentation reconciliation in one ordered pass.
fn current_import_metadata(
    document: &Document,
    metadata: &InteropMetadata,
    output: ImportedOutput<'_>,
    capacity: bool,
) -> Result<(InteropMetadata, Vec<FidelityFinding>), DesignerError> {
    let refuse = |location: &str, message: &str| imported_export_error(location, message);
    validate_historical_envelope(document, metadata, capacity)?;
    if matches!(output, ImportedOutput::Projection) {
        validate_neutral_source_presentation(metadata)?;
    }
    let specs = super::collection_specs(document);
    let mut result = metadata.clone();
    let mut ledger = Vec::new();
    let mut schemas = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut entities = BTreeSet::new();
    for sheet in &mut result.sheets {
        let location = format!("collection {}", sheet.schema_id);
        check_label(&sheet.name).map_err(|error| refuse(&location, &error.to_string()))?;
        let schema = document
            .schemas
            .get(sheet.schema_id.as_str())
            .ok_or_else(|| refuse(&location, "Mapped schema is unavailable."))?;
        if !schemas.insert(sheet.schema_id.clone())
            || !names.insert(sheet.name.to_ascii_lowercase())
            || sheet.columns.is_empty()
            || sheet.columns.len()
                > if capacity {
                    MAX_TEXT_CAPACITY_COLUMNS
                } else {
                    MAX_COLUMNS
                }
        {
            return Err(refuse(
                &location,
                "Duplicate worksheet or invalid source column mapping.",
            ));
        }
        let fields = validate_historical_sheet(document, sheet, &mut entities)?;
        let native = specs
            .values()
            .find(|spec| spec.summary.id == sheet.schema_id)
            .is_some_and(|spec| spec.native_table_rows);
        if !native
            && (schema.fields.len() != sheet.columns.len()
                || schema
                    .fields
                    .keys()
                    .any(|field| !fields.contains(field.as_str()))
                || document
                    .entities
                    .values()
                    .filter(|entity| entity.schema == schema.id)
                    .any(|entity| !entities.contains(entity.id.as_str())))
        {
            return Err(refuse(
                &location,
                "Non-native worksheets require complete current identity mappings.",
            ));
        }
        if matches!(output, ImportedOutput::Csv(collection) if collection != sheet.schema_id) {
            continue;
        }
        let source_columns = sheet.columns.clone();
        // Keep surviving source columns in source order, then append new fields
        // in the runtime's deterministic current column order.
        sheet
            .columns
            .retain(|column| schema.fields.contains_key(column.field_id.as_str()));
        for field in schema
            .fields
            .values()
            .filter(|field| !fields.contains(field.id.as_str()))
        {
            if super::explicit_column_display_name(field).is_none() {
                return Err(refuse(
                    &format!("{location}/field {}", field.id),
                    "An unmapped field without an explicit display name has no recoverable spreadsheet header.",
                ));
            }
            sheet.columns.push(InteropColumnMetadata {
                field_id: field.id.to_string(),
                name: field.effective_name().to_owned(),
                width: None,
            });
        }
        sort_by_field_order(schema, &mut sheet.columns);
        if sheet.columns != source_columns
            || sheet
                .rows
                .iter()
                .any(|row| !document.entities.contains_key(row.entity_id.as_str()))
            || document
                .entities
                .values()
                .filter(|entity| entity.schema == schema.id)
                .any(|entity| !entities.contains(entity.id.as_str()))
        {
            ledger.push(FidelityFinding { category: FidelityCategory::Converted,
                code: "imported_current_structure".into(), location: location.clone(),
                message: "Surviving source columns and rows retain source order and identity-bound presentation. Added columns and rows append in current canonical order with default presentation; removed columns and deleted rows are omitted. Blank optional values are empty cells.".into(), blocking: false });
        }
        map_current_headers(
            schema,
            sheet,
            &mut ledger,
            matches!(output, ImportedOutput::Csv(_)),
        )?;
        reconcile_import_rows(document, sheet, &source_columns, &entities, capacity)?;
    }
    Ok((
        scope_import_metadata(document, result, output, capacity)?,
        ledger,
    ))
}

fn scope_import_metadata(
    document: &Document,
    mut result: InteropMetadata,
    output: ImportedOutput<'_>,
    capacity: bool,
) -> Result<InteropMetadata, DesignerError> {
    let refuse = imported_export_error;
    match output {
        ImportedOutput::Projection => validate_exact_import_metadata(document, &result, capacity)
            .map_err(|error| refuse("workbook", &error.to_string()))?,
        ImportedOutput::Csv(collection) => {
            result.sheets.retain(|sheet| sheet.schema_id == collection);
            if result.sheets.is_empty() {
                return Err(refuse(
                    "workbook",
                    "CSV export requires a known stable collection ID.",
                ));
            }
        }
        ImportedOutput::Xlsx => {}
    }
    Ok(result)
}

// Saved/source closure retains its historical admission rules, including unused
// historical presentation. File exports instead check only emitted presentation.
fn validate_neutral_source_presentation(metadata: &InteropMetadata) -> Result<(), DesignerError> {
    for sheet in &metadata.sheets {
        let location = format!("collection {}", sheet.schema_id);
        if sheet.columns.iter().any(|column| {
            column
                .width
                .is_some_and(|w| !w.is_finite() || w <= 0.0 || w > 255.0)
        }) {
            return Err(imported_export_error(&location, "Invalid source width."));
        }
        for style in sheet.rows.iter().flat_map(|row| &row.styles) {
            if [&style.number_format, &style.fill, &style.alignment]
                .into_iter()
                .flatten()
                .any(|text| text.len() > MAX_PROFILE_STRING_BYTES || text.contains('\0'))
            {
                return Err(imported_export_error(
                    &location,
                    "Source style exceeds the bounded profile.",
                ));
            }
        }
    }
    Ok(())
}

fn validate_historical_envelope(
    document: &Document,
    metadata: &InteropMetadata,
    capacity: bool,
) -> Result<(), DesignerError> {
    let refuse = imported_export_error;
    if metadata.version != 1
        || metadata.sheets.is_empty()
        || metadata.sheets.len() > MAX_SHEETS
        || metadata.sheets.len() != document.schemas.len()
    {
        return Err(refuse(
            "workbook",
            "Metadata must cover every current worksheet exactly once.",
        ));
    }
    super::ensure_projection_size_with_limit(
        metadata,
        if capacity {
            3 * 1024 * 1024
        } else {
            super::MAX_PROJECTION_BYTES
        },
    )
    .map_err(|error| refuse("workbook", &error.to_string()))?;
    Ok(())
}

fn reconcile_import_rows(
    document: &Document,
    sheet: &mut InteropSheetMetadata,
    source_columns: &[InteropColumnMetadata],
    entities: &BTreeSet<String>,
    capacity: bool,
) -> Result<(), DesignerError> {
    let location = format!("collection {}", sheet.schema_id);
    let schema = &document.schemas[sheet.schema_id.as_str()];
    let refuse = imported_export_error;
    sheet
        .rows
        .retain(|row| document.entities.contains_key(row.entity_id.as_str()));
    for row in &mut sheet.rows {
        row.styles = sheet
            .columns
            .iter()
            .map(|column| {
                source_columns
                    .iter()
                    .position(|old| old.field_id == column.field_id)
                    .map_or_else(CellStyle::default, |index| row.styles[index].clone())
            })
            .collect();
    }
    for entity in document
        .entities
        .values()
        .filter(|entity| entity.schema == schema.id && !entities.contains(entity.id.as_str()))
    {
        sheet.rows.push(InteropRowMetadata {
            entity_id: entity.id.to_string(),
            styles: vec![CellStyle::default(); sheet.columns.len()],
        });
    }
    if sheet.columns.is_empty()
        || sheet.columns.len()
            > if capacity {
                MAX_TEXT_CAPACITY_COLUMNS
            } else {
                MAX_COLUMNS
            }
        || sheet.rows.len()
            > if capacity {
                super::interop_adapter::MAX_TEXT_CAPACITY_ROWS
            } else {
                MAX_DATA_ROWS
            }
    {
        return Err(refuse(
            &location,
            "Current rows or columns exceed the existing metadata export profile; nothing is truncated.",
        ));
    }
    Ok(())
}

fn validate_historical_sheet(
    document: &Document,
    sheet: &InteropSheetMetadata,
    entities: &mut BTreeSet<String>,
) -> Result<BTreeSet<String>, DesignerError> {
    let location = format!("collection {}", sheet.schema_id);
    let refuse = imported_export_error;
    let schema = &document.schemas[sheet.schema_id.as_str()];
    let mut fields = BTreeSet::new();
    let mut labels = BTreeSet::new();
    for column in &sheet.columns {
        check_label(&column.field_id).map_err(|error| refuse(&location, &error.to_string()))?;
        check_label(&column.name).map_err(|error| refuse(&location, &error.to_string()))?;
        if !schema.fields.contains_key(column.field_id.as_str())
            && document.schemas.values().any(|other| {
                other.id != schema.id && other.fields.contains_key(column.field_id.as_str())
            })
        {
            return Err(refuse(
                &format!("{location}/field {}", column.field_id),
                "Mapped field belongs to another current schema; only absent historical IDs are tombstones.",
            ));
        }
        if !fields.insert(column.field_id.clone()) || !labels.insert(column.name.clone()) {
            return Err(refuse(&location, "Duplicate source field or header."));
        }
    }
    for row in &sheet.rows {
        check_label(&row.entity_id).map_err(|error| refuse(&location, &error.to_string()))?;
        if document
            .entities
            .get(row.entity_id.as_str())
            .is_some_and(|entity| entity.schema != schema.id)
        {
            return Err(refuse(
                &format!("{location}/entity {}", row.entity_id),
                "Mapped entity belongs to another current schema; only absent historical IDs are tombstones.",
            ));
        }
        if !entities.insert(row.entity_id.clone()) || row.styles.len() != sheet.columns.len() {
            return Err(refuse(
                &location,
                "Duplicate or foreign row, or invalid source style dimensions.",
            ));
        }
    }
    Ok(fields)
}

fn map_current_headers(
    schema: &tachiko_workspace_engine::Schema,
    sheet: &mut InteropSheetMetadata,
    ledger: &mut Vec<FidelityFinding>,
    csv: bool,
) -> Result<(), DesignerError> {
    let location = format!("collection {}", sheet.schema_id);
    let mut headers = BTreeSet::new();
    for column in &mut sheet.columns {
        let field = &schema.fields[column.field_id.as_str()];
        let source = super::explicit_column_display_name(field).unwrap_or(&column.name);
        let path = format!("{location}/field {}", field.id);
        let mapped = super::interop_adapter::native_tables_compatibility(
            if csv {
                super::interop_adapter::NativeTablesCheck::CsvHeader {
                    value: source,
                    used: &mut headers,
                    emitted: sheet.has_header,
                }
            } else {
                super::interop_adapter::NativeTablesCheck::Name {
                    value: source,
                    worksheet: false,
                    used: &mut headers,
                    allocate: true,
                }
            },
            &path,
        )
        .map_err(|finding| DesignerError::ImportedSpreadsheetExport { finding })?
        .name;
        if (!csv || sheet.has_header) && (mapped != source || mapped != column.name) {
            ledger.push(FidelityFinding {
                category: FidelityCategory::Converted,
                code: "imported_current_header".into(),
                location: path,
                message: format!(
                    "Current header {source:?} exports as {mapped:?}; the source header was {:?}.",
                    column.name
                ),
                blocking: false,
            });
        }
        column.name = mapped;
    }
    Ok(())
}

pub(crate) fn imported_export_error(location: &str, message: &str) -> DesignerError {
    DesignerError::ImportedSpreadsheetExport {
        finding: FidelityFinding {
            category: FidelityCategory::UnsupportedSafeDisabled,
            code: "imported_spreadsheet_unrepresentable".into(),
            location: location.into(),
            message: message.into(),
            blocking: true,
        },
    }
}

/// Admit historical import metadata against current content without requiring
/// deleted slots to be removed from the host's presentation record.
///
/// # Errors
/// Rejects invalid mappings or current content outside the metadata output profile.
pub fn validate_import_metadata(
    document: &Document,
    metadata: &InteropMetadata,
) -> Result<(), DesignerError> {
    // The detached admission chooses the same profile as import/reopen, including
    // projection limits; scalar shape alone cannot decide presentation rules.
    let runtime = DesignerRuntime::from_document(document.clone(), PREFLIGHT_OCCURRENCE)?;
    validate_import_metadata_for_profile(document, metadata, runtime.text_capacity)
}

fn validate_import_metadata_for_profile(
    document: &Document,
    metadata: &InteropMetadata,
    capacity: bool,
) -> Result<(), DesignerError> {
    current_import_metadata(document, metadata, ImportedOutput::Projection, capacity).map(|_| ())
}

/// Check private source presentation mappings against exact canonical identity.
/// Deleted source rows may remain as source metadata; they never recreate data.
///
/// # Errors
/// Rejects duplicate, foreign, unbounded, or incomplete schema/field/row maps.
fn validate_exact_import_metadata(
    document: &Document,
    metadata: &InteropMetadata,
    capacity: bool,
) -> Result<(), DesignerError> {
    validate_metadata_envelope(document, metadata, capacity)?;
    let mut schemas = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut rows = BTreeSet::new();
    for sheet in &metadata.sheets {
        check_label(&sheet.name)?;
        if !schemas.insert(sheet.schema_id.as_str())
            || !names.insert(sheet.name.to_ascii_lowercase())
        {
            return Err(tracker_error("duplicate interop worksheet mapping"));
        }
        let schema = document
            .schemas
            .get(&SchemaId::from(sheet.schema_id.clone()))
            .ok_or_else(|| tracker_error("interop schema is not present in the candidate"))?;
        if sheet.columns.is_empty()
            || sheet.columns.len()
                > if capacity {
                    MAX_TEXT_CAPACITY_COLUMNS
                } else {
                    MAX_COLUMNS
                }
            || sheet.columns.len() != schema.fields.len()
            || sheet.rows.len()
                > if capacity {
                    super::interop_adapter::MAX_TEXT_CAPACITY_ROWS
                } else {
                    MAX_DATA_ROWS
                }
        {
            return Err(tracker_error(
                "interop row/column mappings exceed bounds or omit declared fields",
            ));
        }
        let mut fields = BTreeSet::new();
        let mut labels = BTreeSet::new();
        for column in &sheet.columns {
            check_label(&column.name)?;
            if !fields.insert(column.field_id.as_str())
                || !labels.insert(column.name.as_str())
                || !schema
                    .fields
                    .contains_key(&FieldId::from(column.field_id.clone()))
                || column
                    .width
                    .is_some_and(|width| !width.is_finite() || width <= 0.0 || width > 255.0)
            {
                return Err(tracker_error(
                    "invalid interop field, label, or width mapping",
                ));
            }
        }
        for row in &sheet.rows {
            check_label(&row.entity_id)?;
            if !rows.insert(row.entity_id.as_str()) || row.styles.len() != sheet.columns.len() {
                return Err(tracker_error(
                    "duplicate interop row or invalid style dimensions",
                ));
            }
            if document
                .entities
                .get(&EntityId::from(row.entity_id.clone()))
                .is_some_and(|entity| entity.schema != schema.id)
            {
                return Err(tracker_error("interop row belongs to another schema"));
            }
            for style in &row.styles {
                for text in [&style.number_format, &style.fill, &style.alignment]
                    .into_iter()
                    .flatten()
                {
                    if text.len() > MAX_PROFILE_STRING_BYTES || text.contains('\0') {
                        return Err(tracker_error(
                            "interop style is outside the bounded text profile",
                        ));
                    }
                }
            }
        }
    }
    if document
        .entities
        .keys()
        .any(|id| !rows.contains(id.as_str()))
    {
        return Err(tracker_error(
            "canonical rows are missing from the interop identity mapping",
        ));
    }
    Ok(())
}

fn validate_metadata_envelope(
    document: &Document,
    metadata: &InteropMetadata,
    capacity: bool,
) -> Result<(), DesignerError> {
    if metadata.version != 1
        || metadata.sheets.is_empty()
        || metadata.sheets.len() > MAX_SHEETS
        || metadata.sheets.len() != document.schemas.len()
    {
        return Err(tracker_error(
            "interop metadata version or worksheet count is invalid",
        ));
    }
    super::ensure_projection_size_with_limit(
        metadata,
        if capacity {
            3 * 1024 * 1024
        } else {
            super::MAX_PROJECTION_BYTES
        },
    )?;
    if capacity {
        validate_capacity_metadata(document, metadata)?;
    }
    Ok(())
}

/// Capacity columns follow id order (the stored source) or the durable column
/// order (R4a), which reconciled exports use.
fn capacity_columns_in_order(sheet: &InteropSheetMetadata, schema: &Schema) -> bool {
    let columns = || sheet.columns.iter().map(|column| column.field_id.as_str());
    columns().eq(schema.fields.keys().map(FieldId::as_str))
        || columns().eq(schema.ordered_fields().map(|field| field.id.as_str()))
}

fn validate_capacity_metadata(
    document: &Document,
    metadata: &InteropMetadata,
) -> Result<(), DesignerError> {
    let sheet = metadata
        .sheets
        .first()
        .ok_or_else(|| tracker_error("capacity metadata sheet is missing"))?;
    let schema = document
        .schemas
        .values()
        .next()
        .ok_or_else(|| tracker_error("capacity schema is missing"))?;
    if metadata.sheets.len() != 1
        || !sheet
            .rows
            .iter()
            .map(|row| row.entity_id.as_str())
            .eq(document.entities.keys().map(EntityId::as_str))
        || !capacity_columns_in_order(sheet, schema)
    {
        return Err(tracker_error(
            "capacity metadata must preserve canonical row and column identity order",
        ));
    }
    if !sheet.has_header
        || !(1..=MAX_TEXT_CAPACITY_COLUMNS).contains(&sheet.columns.len())
        || sheet.columns.iter().any(|column| column.width.is_some())
        || sheet.rows.iter().any(|row| {
            row.styles
                .iter()
                .any(|style| *style != CellStyle::default())
        })
    {
        return Err(tracker_error(
            "capacity metadata requires a header, no widths and default styles",
        ));
    }
    let header = SourceSheet {
        name: sheet.name.clone(),
        has_header: true,
        columns: sheet
            .columns
            .iter()
            .map(|column| SourceColumn {
                name: column.name.clone(),
                width: None,
                number_format: None,
            })
            .collect(),
        rows: Vec::new(),
        layout: SourceLayout::default(),
    };
    let workbook = SourceWorkbook {
        sheets: vec![header],
        ledger: Vec::new(),
    };
    super::interop_adapter::validate_text_capacity(&workbook).map_err(interop_admission_error)
}

impl DesignerRuntime {
    /// Rebuild source workbook values and formulas from the exact live snapshot.
    ///
    /// # Errors
    /// Rejects stale revisions, invalid metadata, missing identity mappings, or
    /// any formula reference outside the declared spreadsheet export profile.
    pub fn export_workbook(
        &self,
        expected_revision: &str,
        metadata: &InteropMetadata,
    ) -> Result<SourceWorkbook, DesignerError> {
        self.export_imported_workbook(expected_revision, metadata, ImportedOutput::Projection)
    }

    /// The admitted runtime profile also governs encoding, even when rebuilt
    /// cells fit Shared geometry after a capacity promotion.
    pub(crate) fn imported_output_profile(&self) -> OutputProfile {
        if self.text_capacity {
            OutputProfile::TextCapacity
        } else {
            OutputProfile::Shared
        }
    }

    /// Reconcile once, then check every emitted cell for the selected format
    /// before shared profile validation or encoding can flatten its location.
    pub(crate) fn export_imported_workbook(
        &self,
        expected_revision: &str,
        metadata: &InteropMetadata,
        output: ImportedOutput<'_>,
    ) -> Result<SourceWorkbook, DesignerError> {
        self.check_revision(expected_revision)?;
        let snapshot = self.session.export_snapshot();
        let (metadata, ledger) =
            current_import_metadata(snapshot.document(), metadata, output, self.text_capacity)?;
        if self.text_capacity {
            validate_capacity_metadata(snapshot.document(), &metadata)
                .map_err(|error| imported_export_error("workbook", &error.to_string()))?;
        }
        let profile = self.imported_output_profile();
        let mut workbook = self
            .rebuild_workbook_from_metadata(
                expected_revision,
                snapshot.document(),
                &metadata,
                profile,
                matches!(output, ImportedOutput::Csv(_)),
            )
            .map_err(|error| imported_export_error("workbook", &error.to_string()))?;
        match output {
            ImportedOutput::Xlsx => {
                super::interop_adapter::validate_xlsx_compatibility(&workbook)
                    .map_err(|finding| DesignerError::ImportedSpreadsheetExport { finding })?;
                workbook
                    .ledger
                    .extend(super::interop_adapter::native_tables_xlsx_number_findings(
                        &workbook,
                    ));
            }
            ImportedOutput::Csv(_) => {
                if profile == OutputProfile::Shared && workbook.sheets[0].rows.len() > MAX_DATA_ROWS
                {
                    return Err(imported_export_error(
                        "workbook",
                        &format!("CSV output exceeds the {MAX_DATA_ROWS}-row shared profile."),
                    ));
                }
                super::interop_adapter::validate_native_tables_csv(&workbook.sheets[0])
                    .map_err(|finding| DesignerError::ImportedSpreadsheetExport { finding })?;
            }
            ImportedOutput::Projection => {}
        }
        let mut workbook = if matches!(output, ImportedOutput::Csv(_)) {
            // Formula definitions, worksheet labels, widths and styles never
            // enter CSV bytes. Do not run an XLSX validator on them.
            append_saved_export_findings(snapshot.document(), &mut workbook);
            workbook
        } else {
            finalize_export_workbook(snapshot.document(), workbook, profile)
                .map_err(|error| imported_export_error("workbook", &error.to_string()))?
        };
        workbook.ledger.extend(ledger);
        Ok(workbook)
    }

    /// Export the one stock Tracker profile without fabricating import provenance.
    ///
    /// The private presentation establishes only stable row order and supported
    /// cell styles. Rust derives the fixed three-field schema and every scalar
    /// from the authoritative snapshot, then uses the existing bounded writer.
    ///
    /// # Errors
    /// Returns stale, non-Tracker, incomplete/foreign presentation, scalar, or
    /// bounded-output failures without mutating the resident occurrence.
    pub fn export_native_tracker_workbook(
        &self,
        expected_revision: &str,
        presentation: &NativeTrackerExportPresentation,
    ) -> Result<SourceWorkbook, DesignerError> {
        self.check_revision(expected_revision)?;
        let snapshot = self.session.export_snapshot();
        let metadata = native_tracker_metadata(snapshot.document(), presentation)?;
        self.export_workbook_from_metadata(
            expected_revision,
            snapshot.document(),
            &metadata,
            OutputProfile::NativeTracker,
        )
    }

    /// Export the native Budget profile from stable collection/entity/field
    /// identities in the exact resident snapshot.
    ///
    /// Budget views are presentation aliases. The metadata builder emits one
    /// worksheet per canonical collection, while the existing bound-expression
    /// exporter projects formulas against the same snapshot without mutating
    /// the resident occurrence.
    ///
    /// # Errors
    /// Rejects stale revisions, incomplete or colliding presentation mappings,
    /// unsupported formulas, and any shared writer/profile failure.
    pub fn export_native_budget_workbook(
        &self,
        expected_revision: &str,
        presentation: &NativeBudgetExportPresentation,
    ) -> Result<SourceWorkbook, DesignerError> {
        self.check_revision(expected_revision)?;
        let metadata = native_budget_metadata(self, presentation)?;
        let snapshot = self.session.export_snapshot();
        self.export_workbook_from_metadata(
            expected_revision,
            snapshot.document(),
            &metadata,
            OutputProfile::Shared,
        )
    }

    /// Export only the active native Budget view as calculated scalar values.
    ///
    /// CSV has no worksheet or formula-reference surface, so its bounded
    /// projection admits only the active collection mapping. Inactive views
    /// and collection mappings remain outside this path; the full XLSX method
    /// above continues to perform complete-source admission.
    ///
    /// # Errors
    /// Returns stale, unavailable, incomplete/foreign active mappings, scalar,
    /// style, calculation, or bounded-output failures without mutating the
    /// resident occurrence.
    pub fn export_native_budget_csv_workbook(
        &self,
        expected_revision: &str,
        presentation: &NativeBudgetExportPresentation,
    ) -> Result<SourceWorkbook, DesignerError> {
        self.check_revision(expected_revision)?;
        let (active, collection, spec) = native_budget_active_csv_mapping(self, presentation)?;
        let snapshot = self.session.export_snapshot();
        let document = snapshot.document();
        let schema = document
            .schemas
            .get(&SchemaId::from(active.collection_id.clone()))
            .ok_or_else(|| tracker_error("native Budget active schema is unavailable"))?;
        if schema.id.as_str() != active.collection_id
            || spec.columns.is_empty()
            || spec.columns.len() > MAX_NATIVE_BUDGET_COLUMNS
            || schema.fields.len() != spec.columns.len()
        {
            return Err(tracker_error(
                "native Budget active schema identity changed during export",
            ));
        }
        let row_styles = native_budget_active_csv_styles(collection, spec)?;
        let rows =
            native_budget_active_csv_rows(self, expected_revision, document, spec, &row_styles)?;

        let workbook = SourceWorkbook {
            sheets: vec![SourceSheet {
                // CSV has no worksheet-name surface. Keep a bounded safe
                // internal name so shared cell/style validation does not
                // accidentally apply XLSX worksheet-name rules to CSV.
                name: "Budget".to_owned(),
                has_header: true,
                columns: spec
                    .columns
                    .iter()
                    .map(|column| SourceColumn {
                        name: column.key.clone(),
                        width: None,
                        number_format: None,
                    })
                    .collect(),
                rows,
                layout: SourceLayout::default(),
            }],
            ledger: vec![FidelityFinding {
                category: FidelityCategory::Converted,
                code: "native_budget_active_csv_values".into(),
                location: active.name.clone(),
                message: "CSV is rebuilt from the active Budget collection, entity, and field identities with authoritative calculated scalar values; inactive collections and worksheet names are not admitted.".into(),
                blocking: false,
            }],
        };
        finalize_export_workbook(document, workbook, OutputProfile::Shared)
    }

    fn export_workbook_from_metadata(
        &self,
        expected_revision: &str,
        document: &Document,
        metadata: &InteropMetadata,
        profile: OutputProfile,
    ) -> Result<SourceWorkbook, DesignerError> {
        let workbook = self.rebuild_workbook_from_metadata(
            expected_revision,
            document,
            metadata,
            profile,
            false,
        )?;
        finalize_export_workbook(document, workbook, profile)
    }

    #[allow(clippy::too_many_lines)] // One metadata-to-workbook rebuild, column formats included.
    fn rebuild_workbook_from_metadata(
        &self,
        expected_revision: &str,
        document: &Document,
        metadata: &InteropMetadata,
        profile: OutputProfile,
        values_only: bool,
    ) -> Result<SourceWorkbook, DesignerError> {
        self.check_revision(expected_revision)?;
        // The caller holds the exact immutable snapshot already used for
        // metadata admission. No second owned document is needed here.
        // Capacity admission excludes formulas, the sole consumers of A1
        // addresses. Other profiles retain their full reference index.
        let (addresses, columns) = if profile == OutputProfile::TextCapacity || values_only {
            (BTreeMap::new(), BTreeMap::new())
        } else {
            export_a1_maps(document, metadata)
        };
        let mut sheets = Vec::new();
        for sheet in &metadata.sheets {
            let mut rows = Vec::new();
            for row in &sheet.rows {
                let Some(entity) = document
                    .entities
                    .get(&EntityId::from(row.entity_id.clone()))
                else {
                    continue;
                };
                let mut cells = Vec::new();
                for (column, definition) in sheet.columns.iter().enumerate() {
                    let target = FieldRef::new(entity.id.clone(), definition.field_id.clone());
                    let value = entity.fields.get(&target.field);
                    let (value, formula) = if let Some(Value::Formula(expression)) = value {
                        let projection =
                            self.query_fields(expected_revision, &[field_target(&target)])?;
                        let number = projection.fields[0].calculated.as_ref().and_then(super::CalculationProjection::number).ok_or_else(|| tracker_error("formula export requires a complete authoritative Number result"))?;
                        let formula = if values_only {
                            None
                        } else {
                            Some(export_expression(
                                expression, &target, &addresses, &columns,
                            )?)
                        };
                        if formula
                            .as_ref()
                            .is_some_and(|formula| formula.len() > MAX_PROFILE_STRING_BYTES)
                        {
                            return Err(tracker_error(
                                "export formula exceeds the bounded reimport authoring profile",
                            ));
                        }
                        (SourceValue::Number { value: number }, formula)
                    } else {
                        (export_scalar(value)?, None)
                    };
                    cells.push(SourceCell {
                        value,
                        formula,
                        style: if values_only {
                            CellStyle::default()
                        } else {
                            row.styles[column].clone()
                        },
                    });
                }
                rows.push(cells);
            }
            sheets.push(SourceSheet {
                name: sheet.name.clone(),
                has_header: sheet.has_header,
                columns: sheet
                    .columns
                    .iter()
                    .map(|column| SourceColumn {
                        name: column.name.clone(),
                        width: if values_only { None } else { column.width },
                        number_format: if values_only || profile == OutputProfile::TextCapacity {
                            None
                        } else {
                            document
                                .schemas
                                .get(&SchemaId::from(sheet.schema_id.clone()))
                                .and_then(|schema| {
                                    schema.fields.get(&FieldId::from(column.field_id.clone()))
                                })
                                .and_then(|field| field.number_format.clone())
                        },
                    })
                    .collect(),
                rows,
                layout: SourceLayout::default(),
            });
        }
        let workbook = SourceWorkbook { sheets, ledger: vec![FidelityFinding {
            category: FidelityCategory::Converted, code: "bound_formula_absolute_a1".into(), location: "workbook".into(),
            message: "Export is rebuilt from current stable identities and authoritative values. Same-row references export as relative A1 so a spreadsheet fill-down follows the row; other formula references use current absolute worksheet coordinates. Deleted rows are omitted.".into(), blocking: false,
        }] };
        Ok(workbook)
    }
}

fn finalize_export_workbook(
    document: &Document,
    mut workbook: SourceWorkbook,
    profile: OutputProfile,
) -> Result<SourceWorkbook, DesignerError> {
    append_saved_export_findings(document, &mut workbook);
    super::interop_adapter::validate_output_for_profile(&workbook, profile)
        .map_err(interop_admission_error)?;
    Ok(workbook)
}

fn append_saved_export_findings(document: &Document, workbook: &mut SourceWorkbook) {
    if !document.keyed_grouped_sum_definitions.is_empty() {
        workbook.ledger.push(FidelityFinding {
            category: FidelityCategory::LossyOnExport,
            code: "saved_keyed_grouped_sum_definitions_not_exported".into(),
            location: "workbook".into(),
            message: "Saved grouped-summary definitions are not included in this spreadsheet export. Keep a Tachiko saved copy to preserve their bindings and recomputation.".into(),
            blocking: false,
        });
    }
}

fn native_budget_active_csv_mapping<'a>(
    runtime: &'a DesignerRuntime,
    presentation: &'a NativeBudgetExportPresentation,
) -> Result<
    (
        &'a NativeBudgetExportView,
        &'a NativeBudgetExportCollection,
        &'a CollectionSpec,
    ),
    DesignerError,
> {
    if presentation.version != 1
        || presentation.views.is_empty()
        || presentation.views.len() > super::MAX_COLLECTIONS
    {
        return Err(tracker_error(
            "native Budget CSV presentation is outside the bounded profile",
        ));
    }
    let mut active_views = presentation
        .views
        .iter()
        .filter(|view| view.id == presentation.active_view);
    let active = active_views
        .next()
        .ok_or_else(|| tracker_error("active Budget view is unavailable"))?;
    if active_views.next().is_some() {
        return Err(tracker_error("native Budget active view ID is ambiguous"));
    }
    check_label(&active.id)?;
    check_label(&active.name)?;
    check_label(&active.collection_id)?;

    let mut active_collections = presentation
        .collections
        .iter()
        .filter(|collection| collection.collection_id == active.collection_id);
    let collection = active_collections
        .next()
        .ok_or_else(|| tracker_error("active Budget collection mapping is unavailable"))?;
    if active_collections.next().is_some() {
        return Err(tracker_error(
            "native Budget active collection mapping is ambiguous",
        ));
    }
    check_label(&collection.collection_id)?;
    let spec = runtime
        .collection_specs
        .values()
        .find(|spec| spec.summary.id == active.collection_id)
        .ok_or_else(|| tracker_error("native Budget active collection is unavailable"))?;
    Ok((active, collection, spec))
}

fn native_budget_active_csv_styles(
    collection: &NativeBudgetExportCollection,
    spec: &CollectionSpec,
) -> Result<BTreeMap<String, Vec<CellStyle>>, DesignerError> {
    if spec.columns.len() > MAX_NATIVE_BUDGET_COLUMNS
        || collection.rows.len() != spec.entities.len()
        || collection.rows.len() > MAX_NATIVE_BUDGET_DATA_ROWS
    {
        return Err(tracker_error(
            "native Budget active collection mapping must cover every bounded source row exactly once",
        ));
    }
    let mut row_styles = BTreeMap::new();
    for row in &collection.rows {
        check_label(&row.entity_id)?;
        if row.styles.len() != spec.columns.len()
            || !spec
                .entities
                .iter()
                .any(|entity| entity.as_str() == row.entity_id)
            || row_styles
                .insert(row.entity_id.clone(), row.styles.clone())
                .is_some()
        {
            return Err(tracker_error(
                "native Budget active row mapping is foreign, duplicated, or has invalid styles",
            ));
        }
        for style in &row.styles {
            for text in [&style.number_format, &style.fill, &style.alignment]
                .into_iter()
                .flatten()
            {
                if text.len() > MAX_PROFILE_STRING_BYTES || text.contains('\0') {
                    return Err(tracker_error(
                        "native Budget active style is outside the bounded text profile",
                    ));
                }
            }
        }
    }
    if spec
        .entities
        .iter()
        .any(|entity| !row_styles.contains_key(entity.as_str()))
    {
        return Err(tracker_error(
            "native Budget active row mapping omits a canonical source row",
        ));
    }
    Ok(row_styles)
}

fn native_budget_active_csv_rows(
    runtime: &DesignerRuntime,
    expected_revision: &str,
    document: &Document,
    spec: &CollectionSpec,
    row_styles: &BTreeMap<String, Vec<CellStyle>>,
) -> Result<Vec<Vec<SourceCell>>, DesignerError> {
    let mut rows = Vec::with_capacity(spec.entities.len());
    for entity_id in &spec.entities {
        let entity = document
            .entities
            .get(entity_id)
            .ok_or_else(|| tracker_error("native Budget active source row is unavailable"))?;
        let styles = row_styles
            .get(entity_id.as_str())
            .ok_or_else(|| tracker_error("native Budget active row styles are unavailable"))?;
        let mut cells = Vec::with_capacity(spec.columns.len());
        for (column, style) in spec.columns.iter().zip(styles) {
            let field = FieldRef::new(entity.id.clone(), column.id.clone());
            let stored = entity.fields.get(&column.id);
            let value = if matches!(stored, Some(Value::Formula(_))) {
                let projection =
                    runtime.query_fields(expected_revision, &[field_target(&field)])?;
                let calculated = projection.fields.first().and_then(|field| {
                    field
                        .calculated
                        .as_ref()
                        .and_then(super::CalculationProjection::number)
                });
                let value = calculated.ok_or_else(|| {
                    tracker_error(
                        "native Budget active CSV requires a complete authoritative Number result",
                    )
                })?;
                SourceValue::Number { value }
            } else {
                export_scalar(stored)?
            };
            cells.push(SourceCell {
                value,
                formula: None,
                style: style.clone(),
            });
        }
        rows.push(cells);
    }
    Ok(rows)
}

/// Build the private native Budget identity mapping and deterministic sheet
/// order. View aliases are accepted only as presentation aliases; canonical
/// rows and fields are always derived from the resident runtime snapshot.
#[allow(clippy::too_many_lines)] // Keep the bounded identity-admission proof together.
fn native_budget_metadata(
    runtime: &DesignerRuntime,
    presentation: &NativeBudgetExportPresentation,
) -> Result<InteropMetadata, DesignerError> {
    if presentation.version != 1
        || presentation.views.is_empty()
        || presentation.views.len() > super::MAX_COLLECTIONS
        || presentation.collections.is_empty()
        || presentation.collections.len() > MAX_NATIVE_BUDGET_SHEETS
    {
        return Err(tracker_error(
            "native Budget export presentation is outside the bounded profile",
        ));
    }
    let active = presentation
        .views
        .iter()
        .find(|view| view.id == presentation.active_view)
        .ok_or_else(|| tracker_error("active Budget view is unavailable"))?;
    let mut view_ids = BTreeSet::new();
    let mut collection_views = BTreeMap::<String, (String, usize)>::new();
    for (index, view) in presentation.views.iter().enumerate() {
        check_label(&view.id)?;
        check_label(&view.name)?;
        check_label(&view.collection_id)?;
        if !view_ids.insert(view.id.as_str()) {
            return Err(tracker_error("native Budget view IDs must be unique"));
        }
        if !runtime
            .collection_specs
            .values()
            .any(|spec| spec.summary.id == view.collection_id)
        {
            return Err(tracker_error(
                "native Budget view references an unavailable collection",
            ));
        }
        // The first alias in the saved view order determines the user-facing
        // worksheet name for that canonical collection. Later aliases never
        // create another worksheet or retarget a formula.
        collection_views
            .entry(view.collection_id.clone())
            .or_insert_with(|| (view.name.clone(), index));
    }
    if !collection_views.contains_key(&active.collection_id) {
        return Err(tracker_error(
            "active Budget view has no canonical collection mapping",
        ));
    }
    let snapshot = runtime.session.export_snapshot();
    let document = snapshot.document();
    let mut row_styles = BTreeMap::<String, BTreeMap<String, Vec<CellStyle>>>::new();
    let mut mapped_collections = BTreeSet::new();
    for collection in &presentation.collections {
        check_label(&collection.collection_id)?;
        if !mapped_collections.insert(collection.collection_id.as_str()) {
            return Err(tracker_error(
                "native Budget collection mappings must be unique",
            ));
        }
        let spec = runtime
            .collection_specs
            .values()
            .find(|spec| spec.summary.id == collection.collection_id)
            .ok_or_else(|| {
                tracker_error("native Budget collection mapping references an unavailable source")
            })?;
        if spec.columns.len() > MAX_NATIVE_BUDGET_COLUMNS
            || collection.rows.len() != spec.entities.len()
            || collection.rows.len() > MAX_NATIVE_BUDGET_DATA_ROWS
        {
            return Err(tracker_error(
                "native Budget collection mapping must cover every bounded source row exactly once",
            ));
        }
        let mut rows = BTreeMap::new();
        for row in &collection.rows {
            check_label(&row.entity_id)?;
            if row.styles.len() != spec.columns.len()
                || !spec
                    .entities
                    .iter()
                    .any(|entity| entity.as_str() == row.entity_id)
                || rows
                    .insert(row.entity_id.clone(), row.styles.clone())
                    .is_some()
            {
                return Err(tracker_error(
                    "native Budget row mapping is foreign, duplicated, or has invalid styles",
                ));
            }
            for style in &row.styles {
                for text in [&style.number_format, &style.fill, &style.alignment]
                    .into_iter()
                    .flatten()
                {
                    if text.len() > super::MAX_PROFILE_STRING_BYTES || text.contains('\0') {
                        return Err(tracker_error(
                            "native Budget style is outside the bounded text profile",
                        ));
                    }
                }
            }
        }
        if spec
            .entities
            .iter()
            .any(|entity| !rows.contains_key(entity.as_str()))
        {
            return Err(tracker_error(
                "native Budget row mapping omits a canonical source row",
            ));
        }
        row_styles.insert(collection.collection_id.clone(), rows);
    }
    if mapped_collections.len() != runtime.collection_specs.len()
        || runtime
            .collection_specs
            .values()
            .any(|spec| !mapped_collections.contains(spec.summary.id.as_str()))
    {
        return Err(tracker_error(
            "native Budget collection mapping must cover every source collection",
        ));
    }
    if document.schemas.len() != runtime.collection_specs.len() {
        return Err(tracker_error(
            "native Budget source schemas changed during export admission",
        ));
    }

    let mut names = BTreeSet::new();
    let mut sheets = Vec::new();
    let mut ordered_collections = runtime
        .collection_specs
        .values()
        .map(|spec| {
            let collection_id = spec.summary.id.clone();
            let (name, view_index) = collection_views
                .get(&collection_id)
                .cloned()
                .unwrap_or_else(|| (spec.summary.key.clone(), usize::MAX));
            (collection_id, name, view_index)
        })
        .collect::<Vec<_>>();
    ordered_collections.sort_by(|(left_id, _, left_index), (right_id, _, right_index)| {
        left_index
            .cmp(right_index)
            .then_with(|| left_id.cmp(right_id))
    });
    for (collection_id, name, _) in ordered_collections {
        if !super::interop_adapter::valid_worksheet_name(&name)
            || name.starts_with('\'')
            || name.ends_with('\'')
            || !names.insert(name.to_lowercase())
        {
            return Err(tracker_error(
                "native Budget view names are invalid or collide across source collections",
            ));
        }
        let spec = runtime
            .collection_specs
            .values()
            .find(|spec| spec.summary.id == collection_id)
            .ok_or_else(|| tracker_error("native Budget worksheet source is unavailable"))?;
        let styles = row_styles
            .get(&collection_id)
            .ok_or_else(|| tracker_error("native Budget worksheet styles are unavailable"))?;
        let rows =
            spec.entities
                .iter()
                .map(|entity| -> Result<InteropRowMetadata, DesignerError> {
                    Ok(InteropRowMetadata {
                        entity_id: entity.to_string(),
                        styles: styles.get(entity.as_str()).cloned().ok_or_else(|| {
                            tracker_error("native Budget row styles are unavailable")
                        })?,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
        let schema = document
            .schemas
            .get(&SchemaId::from(collection_id.clone()))
            .ok_or_else(|| tracker_error("native Budget source schema is unavailable"))?;
        if schema.id.as_str() != collection_id {
            return Err(tracker_error(
                "native Budget schema identity changed during export",
            ));
        }
        sheets.push(InteropSheetMetadata {
            schema_id: collection_id,
            name,
            has_header: true,
            columns: spec
                .columns
                .iter()
                .map(|column| InteropColumnMetadata {
                    field_id: column.id.to_string(),
                    name: column.key.clone(),
                    width: None,
                })
                .collect(),
            rows,
        });
    }
    Ok(InteropMetadata { version: 1, sheets })
}

/// The stock Tracker columns in their durable order (R4a).
fn tracker_column_order(schema: &Schema) -> Vec<&str> {
    schema.field_order.as_ref().map_or_else(
        || vec!["task", "estimate", "done"],
        |order| order.iter().map(FieldId::as_str).collect(),
    )
}

fn native_tracker_metadata(
    document: &Document,
    presentation: &NativeTrackerExportPresentation,
) -> Result<InteropMetadata, DesignerError> {
    if presentation.version != 1 || presentation.rows.len() > MAX_TABLE_ROWS {
        return Err(tracker_error(
            "native Tracker export presentation is outside the 128-row profile",
        ));
    }
    let schema = document
        .schemas
        .get(&SchemaId::from("tracker"))
        .filter(|schema| document.schemas.len() == 1 && schema.key.to_string() == "tracker")
        .ok_or_else(|| tracker_error("native Tracker export requires the stock Tracker schema"))?;
    let fields = [
        ("task", FieldType::Text),
        ("estimate", FieldType::Number),
        ("done", FieldType::Boolean),
    ];
    if schema.fields.len() != fields.len()
        || fields.iter().any(|(id, field_type)| {
            schema
                .fields
                .get(&FieldId::from(*id))
                .is_none_or(|field| field.key.to_string() != *id || field.field_type != *field_type)
        })
        || document.entities.len() > MAX_TABLE_ROWS
    {
        return Err(tracker_error(
            "native Tracker export requires the stock task, estimate, done fields",
        ));
    }
    let mut seen = BTreeSet::new();
    for row in &presentation.rows {
        check_label(&row.entity_id)?;
        if !seen.insert(row.entity_id.as_str())
            || row.styles.len() != fields.len()
            || document
                .entities
                .get(&EntityId::from(row.entity_id.clone()))
                .is_none_or(|entity| entity.schema != schema.id)
        {
            return Err(tracker_error(
                "native Tracker export presentation has an invalid row mapping",
            ));
        }
        for style in &row.styles {
            for text in [&style.number_format, &style.fill, &style.alignment]
                .into_iter()
                .flatten()
            {
                if text.len() > MAX_PROFILE_STRING_BYTES || text.contains('\0') {
                    return Err(tracker_error(
                        "native Tracker export style is outside the bounded profile",
                    ));
                }
            }
        }
    }
    if presentation.rows.len() != document.entities.len()
        || document
            .entities
            .keys()
            .any(|id| !seen.contains(id.as_str()))
    {
        return Err(tracker_error(
            "native Tracker export must map every accepted row exactly once",
        ));
    }
    let order = tracker_column_order(schema);
    Ok(InteropMetadata {
        version: 1,
        sheets: vec![InteropSheetMetadata {
            schema_id: "tracker".to_owned(),
            name: "Tracker".to_owned(),
            has_header: true,
            columns: order
                .iter()
                .map(|id| InteropColumnMetadata {
                    field_id: (*id).to_owned(),
                    name: (*id).to_owned(),
                    width: None,
                })
                .collect(),
            rows: presentation
                .rows
                .iter()
                .map(|row| InteropRowMetadata {
                    entity_id: row.entity_id.clone(),
                    styles: row.styles.clone(),
                })
                .collect(),
        }],
    })
}

/// Inspect canonical bytes plus their private source mappings without replacing
/// the resident occurrence or its history.
///
/// # Errors
/// Returns canonical admission, mapping, or formula export failure.
pub fn inspect_imported_project(
    input: &[u8],
    metadata: &InteropMetadata,
) -> Result<OpenedProjection, DesignerError> {
    inspect_imported_project_with_profile(input, metadata).map(|(opened, _)| opened)
}

pub(crate) fn inspect_imported_project_with_profile(
    input: &[u8],
    metadata: &InteropMetadata,
) -> Result<(OpenedProjection, bool), DesignerError> {
    let (candidate, opened) = super::admit_imported_project(input, PREFLIGHT_OCCURRENCE, metadata)?;
    Ok((opened, candidate.text_capacity))
}

pub(crate) fn validate_saved_open_closure(
    candidate: &DesignerRuntime,
    metadata: &InteropMetadata,
) -> Result<(), DesignerError> {
    let workbook = candidate.export_workbook(candidate.current_revision(), metadata)?;
    let profile = candidate.imported_output_profile();
    let encoded = super::interop_adapter::export_xlsx_for_profile(&workbook, profile)
        .map_err(interop_admission_error)?;
    super::interop_adapter::import_xlsx(&encoded).map_err(interop_admission_error)?;
    for sheet in &workbook.sheets {
        super::interop_adapter::export_csv_for_profile(sheet, profile)
            .map_err(interop_admission_error)?;
    }
    Ok(())
}

pub(crate) fn validate_saved_open_capacity_metadata(
    document: &Document,
    metadata: &InteropMetadata,
) -> Result<(), DesignerError> {
    validate_capacity_metadata(document, metadata)
}

pub(crate) fn column_letters(mut index: usize) -> String {
    let mut reversed = Vec::new();
    loop {
        reversed.push(char::from(
            b'A' + u8::try_from(index % 26).expect("column remainder fits u8"),
        ));
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }
    reversed.into_iter().rev().collect()
}

/// One exported cell's address: its quoted worksheet name, column letters and
/// 1-based row number.
#[derive(Clone)]
pub(crate) struct ExportCell {
    sheet: String,
    column: String,
    row: usize,
    criterion_type: Option<FieldType>,
}

impl ExportCell {
    pub(crate) fn new(
        sheet: String,
        column: String,
        row: usize,
        criterion_type: Option<FieldType>,
    ) -> Self {
        Self {
            sheet,
            column,
            row,
            criterion_type,
        }
    }

    /// The absolute, sheet-qualified address of a stable identity.
    fn absolute(&self) -> String {
        format!("'{}'!${}${}", self.sheet, self.column, self.row)
    }

    /// The relative, unqualified address of the owner's own row, so a
    /// spreadsheet fill-down keeps Tachiko's same-row meaning.
    fn relative(&self) -> String {
        format!("{}{}", self.column, self.row)
    }
}

/// One exported column's whole range, with the declared type a conditional
/// criterion needs.
#[derive(Clone)]
pub(crate) struct ExportColumn {
    range: String,
    field_type: Option<FieldType>,
}

impl ExportColumn {
    pub(crate) fn new(range: String, field_type: Option<FieldType>) -> Self {
        Self { range, field_type }
    }
}

pub(crate) fn export_scalar(value: Option<&Value>) -> Result<SourceValue, DesignerError> {
    match value {
        None => Ok(SourceValue::Empty),
        Some(Value::Text(value)) => Ok(SourceValue::Text {
            value: value.clone(),
        }),
        Some(Value::Number(number)) => Ok(SourceValue::Number {
            value: number.get(),
        }),
        Some(Value::Boolean(value)) => Ok(SourceValue::Boolean { value: *value }),
        Some(Value::Date(value)) => Ok(SourceValue::Date {
            value: value.to_string(),
        }),
        Some(Value::Reference(_) | Value::Formula(_)) => {
            Err(tracker_error("export value is outside the scalar profile"))
        }
    }
}

pub(crate) fn export_expression(
    expression: &Expression,
    owner: &FieldRef,
    addresses: &BTreeMap<FieldRef, ExportCell>,
    columns: &BTreeMap<ColumnRef, ExportColumn>,
) -> Result<String, DesignerError> {
    export_in(
        expression,
        owner,
        addresses,
        columns,
        &std::collections::BTreeSet::new(),
    )
}

/// The field a direct reference names, resolved against the formula's owner.
fn direct_reference(argument: &Expression, owner: &FieldRef) -> Option<FieldRef> {
    match argument {
        Expression::Reference(reference) => Some(reference.clone()),
        Expression::SameRowReference(field) => {
            Some(FieldRef::new(owner.entity.clone(), field.clone()))
        }
        _ => None,
    }
}

/// The references known present in the else branch of `IF(condition, ...)`:
/// those already known, plus the one `ISBLANK` tested.
fn guarded_present(
    condition: &Expression,
    owner: &FieldRef,
    present: &std::collections::BTreeSet<FieldRef>,
) -> std::collections::BTreeSet<FieldRef> {
    let mut present = present.clone();
    if let Expression::IsBlank(argument) = condition {
        present.extend(direct_reference(argument, owner));
    }
    present
}

fn export_in(
    expression: &Expression,
    owner: &FieldRef,
    addresses: &BTreeMap<FieldRef, ExportCell>,
    columns: &BTreeMap<ColumnRef, ExportColumn>,
    present: &std::collections::BTreeSet<FieldRef>,
) -> Result<String, DesignerError> {
    match expression {
        Expression::Number(number) => Ok(number.get().to_string()),
        // Same row means same sheet, so an unqualified relative address keeps
        // the meaning through a spreadsheet fill-down.
        Expression::SameRowReference(field) => addresses
            .get(&FieldRef::new(owner.entity.clone(), field.clone()))
            .map(ExportCell::relative)
            .ok_or_else(|| tracker_error("formula stable target is outside the export mapping")),
        // A fixed identity keeps pointing at that identity when filled down.
        Expression::Reference(reference) => addresses
            .get(reference)
            .map(ExportCell::absolute)
            .ok_or_else(|| tracker_error("formula stable target is outside the export mapping")),
        Expression::ColumnTotal(column) => export_column("SUM", column, columns),
        Expression::ColumnAggregate { function, column } => {
            export_column_aggregate(*function, column, columns)
        }
        Expression::ColumnConditional {
            aggregate,
            criteria,
        } => export_conditional(aggregate, criteria, owner, addresses, columns, present),
        Expression::Add { left, right } => {
            export_binary(left, "+", right, owner, addresses, columns, present)
        }
        Expression::Subtract { left, right } => {
            export_binary(left, "-", right, owner, addresses, columns, present)
        }
        Expression::Multiply { left, right } => {
            export_binary(left, "*", right, owner, addresses, columns, present)
        }
        Expression::Divide { left, right } => {
            export_binary(left, "/", right, owner, addresses, columns, present)
        }
        Expression::Minimum { left, right } => Ok(format!(
            "MIN({},{})",
            export_extremum_argument(left, owner, addresses, columns, present)?,
            export_extremum_argument(right, owner, addresses, columns, present)?
        )),
        Expression::Maximum { left, right } => Ok(format!(
            "MAX({},{})",
            export_extremum_argument(left, owner, addresses, columns, present)?,
            export_extremum_argument(right, owner, addresses, columns, present)?
        )),
        Expression::Compare {
            operator,
            left,
            right,
        } => export_binary(
            left,
            operator.as_str(),
            right,
            owner,
            addresses,
            columns,
            present,
        ),
        Expression::If {
            condition,
            then_value,
            else_value,
        } => Ok(format!(
            "IF({},{},{})",
            export_in(condition, owner, addresses, columns, present)?,
            export_in(then_value, owner, addresses, columns, present)?,
            export_in(
                else_value,
                owner,
                addresses,
                columns,
                &guarded_present(condition, owner, present)
            )?
        )),
        Expression::And(conditions) => {
            export_function("AND", conditions, owner, addresses, columns, present)
        }
        Expression::Or(conditions) => {
            export_function("OR", conditions, owner, addresses, columns, present)
        }
        Expression::Not(condition) => Ok(format!(
            "NOT({})",
            export_in(condition, owner, addresses, columns, present)?
        )),
        Expression::Round { value, digits } => Ok(format!(
            "ROUND({},{digits})",
            export_in(value, owner, addresses, columns, present)?
        )),
        Expression::IsBlank(reference) => Ok(format!(
            "ISBLANK({})",
            export_in(reference, owner, addresses, columns, present)?
        )),
    }
}

/// Tachiko reads a blank optional Number as 0, but Excel's MIN and MAX skip a
/// blank cell argument; `(reference+0)` is 0 there too. A reference the
/// enclosing `IF(ISBLANK(...))` already proved present needs no wrapper.
fn export_extremum_argument(
    argument: &Expression,
    owner: &FieldRef,
    addresses: &BTreeMap<FieldRef, ExportCell>,
    columns: &BTreeMap<ColumnRef, ExportColumn>,
    present: &std::collections::BTreeSet<FieldRef>,
) -> Result<String, DesignerError> {
    let exported = export_in(argument, owner, addresses, columns, present)?;
    Ok(
        match direct_reference(argument, owner).filter(|reference| !present.contains(reference)) {
            Some(_) => format!("({exported}+0)"),
            None => exported,
        },
    )
}

/// A column total exports as the aggregate over the column's whole exported
/// range, the same meaning the semantic node keeps.
fn export_column(
    name: &str,
    column: &ColumnRef,
    columns: &BTreeMap<ColumnRef, ExportColumn>,
) -> Result<String, DesignerError> {
    let (range, _) = export_column_range(column, columns)?;
    Ok(format!("{name}({range})"))
}

fn export_column_aggregate(
    function: ColumnAggregateFunction,
    column: &ColumnRef,
    columns: &BTreeMap<ColumnRef, ExportColumn>,
) -> Result<String, DesignerError> {
    let name = match function {
        ColumnAggregateFunction::Average => "AVERAGE",
        ColumnAggregateFunction::Count => "COUNT",
        // Excel's range MIN/MAX is 0 over an all-blank column where Tachiko's
        // fails, and import refuses it, so neither exports.
        ColumnAggregateFunction::Minimum | ColumnAggregateFunction::Maximum => {
            return Err(tracker_error(
                "column minimum and maximum are outside the export mapping",
            ));
        }
    };
    export_column(name, column, columns)
}

/// The exported range of a column a formula reads, with the declared type the
/// conditional criterion rule needs.
fn export_column_range<'a>(
    column: &ColumnRef,
    columns: &'a BTreeMap<ColumnRef, ExportColumn>,
) -> Result<(&'a str, Option<FieldType>), DesignerError> {
    match columns.get(column) {
        Some(entry) if !entry.range.is_empty() => {
            Ok((entry.range.as_str(), entry.field_type.clone()))
        }
        // A column of a table outside this export, or of an exported table
        // without rows, has no range that would follow rows added later.
        _ => Err(tracker_error(
            "column totals are outside the export mapping",
        )),
    }
}

/// A conditional aggregate exports as Excel's plural `*IFS` call over whole
/// column ranges, the canonical spelling Tachiko reads back.
fn export_conditional(
    aggregate: &ConditionalAggregate,
    criteria: &[ColumnCriterion],
    owner: &FieldRef,
    addresses: &BTreeMap<FieldRef, ExportCell>,
    columns: &BTreeMap<ColumnRef, ExportColumn>,
    present: &std::collections::BTreeSet<FieldRef>,
) -> Result<String, DesignerError> {
    if criteria.is_empty() {
        return Err(tracker_error(
            "conditional aggregates without criteria are outside the export mapping",
        ));
    }
    let mut arguments = Vec::new();
    let name = match aggregate {
        ConditionalAggregate::Sum(column) => {
            arguments.push(export_column_range(column, columns)?.0.to_owned());
            "SUMIFS"
        }
        ConditionalAggregate::Average(column) => {
            arguments.push(export_column_range(column, columns)?.0.to_owned());
            "AVERAGEIFS"
        }
        ConditionalAggregate::Count => "COUNTIFS",
    };
    for criterion in criteria {
        let (range, field_type) = export_column_range(&criterion.column, columns)?;
        let criterion = export_criterion(
            criterion,
            field_type.as_ref(),
            owner,
            addresses,
            columns,
            present,
        )?;
        arguments.push(range.to_owned());
        arguments.push(criterion);
    }
    Ok(format!("{name}({})", arguments.join(",")))
}

/// One criterion as Excel text: a blank test is the empty literal, a written
/// number keeps its operator inside a literal, and an equality test against an
/// expression exports that expression.
fn export_criterion(
    criterion: &ColumnCriterion,
    column_type: Option<&FieldType>,
    owner: &FieldRef,
    addresses: &BTreeMap<FieldRef, ExportCell>,
    columns: &BTreeMap<ColumnRef, ExportColumn>,
    present: &std::collections::BTreeSet<FieldRef>,
) -> Result<String, DesignerError> {
    let cell_type = |reference: &FieldRef| {
        addresses
            .get(reference)
            .and_then(|address| address.criterion_type.as_ref())
    };
    // XLSX import carries a number or one cell, so nothing else may be written.
    let value_type = match criterion.value.as_deref() {
        None => None,
        Some(Expression::Number(_)) => Some(&FieldType::Number),
        Some(Expression::Reference(reference)) => cell_type(reference),
        Some(Expression::SameRowReference(field)) => {
            cell_type(&FieldRef::new(owner.entity.clone(), field.clone()))
        }
        Some(_) => {
            return Err(tracker_error(
                "a criterion expression is outside the export mapping",
            ));
        }
    };
    // Excel would compare dates by serial number, while Tachiko compares dates.
    let date_column = column_type == Some(&FieldType::Date);
    if date_column && value_type == Some(&FieldType::Number) {
        return Err(tracker_error(
            "a number criterion cannot test a date column",
        ));
    }
    if !date_column && value_type == Some(&FieldType::Date) {
        return Err(tracker_error("a date criterion only tests a date column"));
    }
    match (criterion.operator, criterion.value.as_deref()) {
        (ComparisonOperator::Equal, None) => Ok("\"\"".to_owned()),
        (ComparisonOperator::NotEqual, None) => Ok("\"<>\"".to_owned()),
        (ComparisonOperator::Equal, Some(value)) => {
            export_in(value, owner, addresses, columns, present)
        }
        (_, Some(Expression::Number(number))) => Ok(format!(
            "\"{}{}\"",
            criterion.operator.as_str(),
            number.get()
        )),
        _ => Err(tracker_error(
            "criterion is outside the exported spreadsheet formula grammar",
        )),
    }
}

fn export_function(
    name: &str,
    arguments: &[Expression],
    owner: &FieldRef,
    addresses: &BTreeMap<FieldRef, ExportCell>,
    columns: &BTreeMap<ColumnRef, ExportColumn>,
    present: &std::collections::BTreeSet<FieldRef>,
) -> Result<String, DesignerError> {
    let mut exported = Vec::with_capacity(arguments.len());
    for argument in arguments {
        exported.push(export_in(argument, owner, addresses, columns, present)?);
    }
    Ok(format!("{name}({})", exported.join(",")))
}
fn export_binary(
    left: &Expression,
    operator: &str,
    right: &Expression,
    owner: &FieldRef,
    addresses: &BTreeMap<FieldRef, ExportCell>,
    columns: &BTreeMap<ColumnRef, ExportColumn>,
    present: &std::collections::BTreeSet<FieldRef>,
) -> Result<String, DesignerError> {
    Ok(format!(
        "({}{}{})",
        export_in(left, owner, addresses, columns, present)?,
        operator,
        export_in(right, owner, addresses, columns, present)?
    ))
}

#[cfg(test)]
mod a1_import_boundary_tests {
    #[test]
    fn translate_a1_import_keeps_original_input_length_bound() {
        let source = format!("=1{}", " ".repeat(super::MAX_PROFILE_STRING_BYTES - 1));
        let metadata = super::InteropMetadata {
            version: 1,
            sheets: Vec::new(),
        };
        let document = tachiko_workspace_engine::Document::empty("bound", "Bound");
        let owner = tachiko_workspace_engine::FieldRef::new("entity", "field");
        assert!(super::translate_a1(&source, &metadata, &[], 0, &owner, &document).is_err());
        assert!(super::translate_a1(&source[1..], &metadata, &[], 0, &owner, &document).is_ok());
    }
}
