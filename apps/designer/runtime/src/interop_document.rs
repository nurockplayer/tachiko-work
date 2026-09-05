//! Private import composition and revision-pinned cleanup for the declared
//! Designer spreadsheet profile. Semantic publication remains in the runtime.

use super::{
    DesignerError, DesignerRuntime, FieldProjection, FieldTarget, ScalarEditInput,
    StoredValueProjection,
};
use serde::{Deserialize, Serialize};
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
}

impl DesignerRuntime {
    pub(super) fn preview_cleanup(
        &mut self,
        expected_revision: &str,
        operation: &CleanupOperation,
    ) -> Result<CleanupPreview, DesignerError> {
        self.check_revision(expected_revision)?;
        self.prepare_cleanup(expected_revision, operation)
    }
}

use super::{
    MAX_FIELD_QUERY_TARGETS, MAX_PROFILE_STRING_BYTES, MAX_PROJECTION_BYTES, MAX_TABLE_FIELDS,
    MAX_TABLE_ROWS, PREFLIGHT_OCCURRENCE, PublicationProjection, designer_lifecycle,
    ensure_projection_size, field_target, stored_value_projection, tracker_error,
};
use std::collections::BTreeSet;
use tachiko_workspace_engine::patch_lifecycle::{
    ProposalRequest, SemanticPatchBody, SemanticRevision,
};
use tachiko_workspace_engine::{
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
        lifecycle.preview(
            snapshot.document_scope(),
            snapshot.document(),
            snapshot.revision(),
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
        let publication = self.publish_commands(expected_revision, commands)?;
        self.pending_cleanup = None;
        self.undo.clear();
        self.redo.clear();
        Ok(publication)
    }
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

fn convert_value(value: &Value, kind: &FieldType) -> Result<Value, DesignerError> {
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
    CellStyle, FidelityCategory, FidelityFinding, MAX_COLUMNS, MAX_DATA_ROWS, MAX_SHEETS,
    SourceCell, SourceColumn, SourceSheet, SourceValue, SourceWorkbook,
};
use super::{IdGenerator, OpenedProjection, SemanticIdKind};
use std::collections::BTreeMap;
use tachiko_workspace_engine::{
    Entity, EntityKey, Expression, FieldDefinition, FieldKey, Schema, SchemaId, SchemaKey,
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
    pub width: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InteropRowMetadata {
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
pub fn import_workbook(
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
            sheet_index,
            runtime.session.export_snapshot().document(),
        )?;
        pending.insert(target, (source, references));
    }
    while !pending.is_empty() {
        let next = pending
            .iter()
            .find(|(_, (_, references))| {
                references
                    .iter()
                    .all(|reference| !pending.contains_key(reference))
            })
            .map(|(target, _)| target.clone())
            .ok_or_else(|| tracker_error("source formulas contain a dependency cycle"))?;
        let (source, _) = pending
            .remove(&next)
            .ok_or_else(|| tracker_error("pending import formula is unavailable"))?;
        let revision = runtime.current_revision().to_owned();
        runtime.update_formula(&revision, &field_target(&next), &source)?;
    }
    let runtime = DesignerRuntime::from_document(
        runtime.session.export_snapshot().document().clone(),
        occurrence_id,
    )?;
    validate_import_metadata(runtime.session.export_snapshot().document(), &metadata)?;
    // Admission covers the final typed candidate, including user-added columns
    // and bound formulas. Reuse the same representation proof as saved-project
    // inspection and export before exposing or installing this occurrence.
    runtime.export_workbook(runtime.current_revision(), &metadata)?;
    let bootstrap = runtime.bootstrap_projection();
    let table = runtime.query_table(&bootstrap.default_collection)?;
    ledger.push(FidelityFinding {
        category: FidelityCategory::Converted, code: "stable_reference_binding".into(), location: "workbook".into(),
        message: "Worksheet coordinates and selected types were accepted as stable semantic identities; formula export uses absolute references to those identities.".into(), blocking: false,
    });
    let projection = ImportedProjection {
        opened: OpenedProjection { bootstrap, table },
        metadata,
        ledger,
    };
    ensure_projection_size(&projection)?;
    Ok((runtime, projection))
}

fn validate_selection(
    workbook: &SourceWorkbook,
    selection: &ImportSelection,
) -> Result<(), DesignerError> {
    if workbook.ledger.iter().any(|finding| finding.blocking) {
        return Err(tracker_error("blocking fidelity findings prevent import"));
    }
    if workbook.sheets.is_empty()
        || workbook.sheets.len() > MAX_SHEETS
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
            || sheet.columns.len() + selection.extra_columns[index].len() > MAX_COLUMNS
            || sheet.rows.len() > MAX_DATA_ROWS
            || selection.column_types[index].len() != sheet.columns.len()
        {
            return Err(tracker_error(
                "import sheet names, columns, rows, or selected types are invalid",
            ));
        }
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
    if formulas > super::MAX_FORMULAS {
        return Err(tracker_error(
            "source formulas exceed the declared bounded profile",
        ));
    }
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

#[allow(clippy::too_many_arguments)] // One sheet builder shares one bounded import transaction.
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
        id: schema_id.clone(),
        key: SchemaKey::from(format!("sheet_{}", index + 1)),
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
        .map(|(column, kind)| (column.name.clone(), column.width, *kind))
        .chain(
            selection.extra_columns[index]
                .iter()
                .map(|column| (column.name.clone(), None, column.field_type)),
        );
    for (column, (name, width, kind)) in columns.enumerate() {
        let id = FieldId::from(ids.generate(SemanticIdKind::Field));
        schema.fields.insert(
            id.clone(),
            FieldDefinition {
                id: id.clone(),
                key: FieldKey::from(format!("column_{}", column + 1)),
                field_type: kind.field_type(),
                required: false,
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
            styles.push(cell.style.clone());
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

fn quoted_sheet_name(source: &str, cursor: &mut usize) -> Result<String, DesignerError> {
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

fn translate_a1(
    source: &str,
    metadata: &InteropMetadata,
    current_sheet: usize,
    document: &Document,
) -> Result<(String, BTreeSet<FieldRef>), DesignerError> {
    if source.len() > MAX_PROFILE_STRING_BYTES {
        return Err(tracker_error(
            "source formula exceeds the bounded authoring limit",
        ));
    }
    let source = source.strip_prefix('=').unwrap_or(source);
    let mut output = String::new();
    let mut references = BTreeSet::new();
    let mut cursor = 0;
    while cursor < source.len() {
        let ch = source[cursor..]
            .chars()
            .next()
            .expect("cursor stays on characters");
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
            if sheet_name.is_none()
                && !token.contains('!')
                && (token.eq_ignore_ascii_case("MIN") || token.eq_ignore_ascii_case("MAX"))
            {
                output.push_str(&token.to_ascii_lowercase());
                continue;
            }
            let (sheet_name, address) = if let Some(name) = sheet_name {
                (Some(name), token)
            } else if let Some((name, address)) = token.split_once('!') {
                (Some(name.to_owned()), address)
            } else {
                (None, token)
            };
            let sheet_index = if let Some(name) = sheet_name {
                metadata
                    .sheets
                    .iter()
                    .position(|sheet| sheet.name.eq_ignore_ascii_case(&name))
                    .ok_or_else(|| tracker_error("formula worksheet name is unavailable"))?
            } else {
                current_sheet
            };
            if cursor == start {
                return Err(tracker_error("invalid source formula token"));
            }
            let (column, sheet_row) = parse_a1_address(address)?;
            let row = sheet_row
                .checked_sub(usize::from(metadata.sheets[sheet_index].has_header))
                .ok_or_else(|| tracker_error("formula cannot bind a header cell"))?;
            let reference = metadata_target(metadata, sheet_index, row, column)?;
            require_copy_number_for_import(document, &reference)?;
            let entity = &document.entities[&reference.entity];
            let field = &document.schemas[&entity.schema].fields[&reference.field];
            output.push('[');
            output.push_str(entity.key.as_str());
            output.push('.');
            output.push_str(field.key.as_str());
            output.push(']');
            references.insert(reference);
        } else if ch.is_ascii_digit() || ch == '.' {
            // Keep numeric spelling for the existing Rust parser, including
            // scientific notation; never reinterpret an exponent as A1.
            let start = cursor;
            cursor += ch.len_utf8();
            while cursor < source.len()
                && (source.as_bytes()[cursor].is_ascii_digit() || source.as_bytes()[cursor] == b'.')
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
            output.push(ch);
            cursor += ch.len_utf8();
        }
    }
    Ok((output, references))
}

fn require_copy_number_for_import(
    document: &Document,
    reference: &FieldRef,
) -> Result<(), DesignerError> {
    if declared_type(document, reference)? != &FieldType::Number
        || !matches!(
            stored_value(document, reference)?,
            Some(Value::Number(_) | Value::Formula(_))
        )
    {
        return Err(tracker_error(
            "source formula reference must resolve to a present numeric field",
        ));
    }
    Ok(())
}

fn parse_a1_address(address: &str) -> Result<(usize, usize), DesignerError> {
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

/// Check private source presentation mappings against exact canonical identity.
/// Deleted source rows may remain as source metadata; they never recreate data.
///
/// # Errors
/// Rejects duplicate, foreign, unbounded, or incomplete schema/field/row maps.
pub fn validate_import_metadata(
    document: &Document,
    metadata: &InteropMetadata,
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
    ensure_projection_size(metadata)?;
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
            || sheet.columns.len() > MAX_COLUMNS
            || sheet.columns.len() != schema.fields.len()
            || sheet.rows.len() > MAX_DATA_ROWS
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
        self.check_revision(expected_revision)?;
        let snapshot = self.session.export_snapshot();
        validate_import_metadata(snapshot.document(), metadata)?;
        let document = snapshot.document();
        let mut addresses = BTreeMap::new();
        for sheet in &metadata.sheets {
            let present = sheet.rows.iter().filter(|row| {
                document
                    .entities
                    .contains_key(&EntityId::from(row.entity_id.clone()))
            });
            for (row_index, row) in present.enumerate() {
                for (column, definition) in sheet.columns.iter().enumerate() {
                    addresses.insert(
                        FieldRef::new(row.entity_id.clone(), definition.field_id.clone()),
                        format!(
                            "'{}'!${}${}",
                            sheet.name.replace('\'', "''"),
                            column_letters(column),
                            row_index + 1 + usize::from(sheet.has_header)
                        ),
                    );
                }
            }
        }
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
                        let formula = export_expression(expression, &addresses)?;
                        if formula.len() > MAX_PROFILE_STRING_BYTES {
                            return Err(tracker_error(
                                "export formula exceeds the bounded reimport authoring profile",
                            ));
                        }
                        (SourceValue::Number { value: number }, Some(formula))
                    } else {
                        (export_scalar(value)?, None)
                    };
                    cells.push(SourceCell {
                        value,
                        formula,
                        style: row.styles[column].clone(),
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
                        width: column.width,
                    })
                    .collect(),
                rows,
            });
        }
        let workbook = SourceWorkbook { sheets, ledger: vec![FidelityFinding {
            category: FidelityCategory::Converted, code: "bound_formula_absolute_a1".into(), location: "workbook".into(),
            message: "Export is rebuilt from current stable identities and authoritative values. Formula references use current absolute worksheet coordinates; deleted rows are omitted.".into(), blocking: false,
        }] };
        super::interop_adapter::validate_output(&workbook)
            .map_err(|error| tracker_error(&error.0))?;
        Ok(workbook)
    }
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
    let (candidate, opened) = super::admit_project(input, PREFLIGHT_OCCURRENCE)?;
    candidate.export_workbook(candidate.current_revision(), metadata)?;
    Ok(opened)
}

fn column_letters(mut index: usize) -> String {
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

fn export_scalar(value: Option<&Value>) -> Result<SourceValue, DesignerError> {
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

fn export_expression(
    expression: &Expression,
    addresses: &BTreeMap<FieldRef, String>,
) -> Result<String, DesignerError> {
    match expression {
        Expression::Number(number) => Ok(number.get().to_string()),
        Expression::Reference(reference) => addresses
            .get(reference)
            .cloned()
            .ok_or_else(|| tracker_error("formula stable target is outside the export mapping")),
        Expression::Add { left, right } => export_binary(left, "+", right, addresses),
        Expression::Subtract { left, right } => export_binary(left, "-", right, addresses),
        Expression::Multiply { left, right } => export_binary(left, "*", right, addresses),
        Expression::Divide { left, right } => export_binary(left, "/", right, addresses),
        Expression::Minimum { left, right } => Ok(format!(
            "MIN({},{})",
            export_expression(left, addresses)?,
            export_expression(right, addresses)?
        )),
        Expression::Maximum { left, right } => Ok(format!(
            "MAX({},{})",
            export_expression(left, addresses)?,
            export_expression(right, addresses)?
        )),
    }
}
fn export_binary(
    left: &Expression,
    operator: &str,
    right: &Expression,
    addresses: &BTreeMap<FieldRef, String>,
) -> Result<String, DesignerError> {
    Ok(format!(
        "({}{}{})",
        export_expression(left, addresses)?,
        operator,
        export_expression(right, addresses)?
    ))
}
