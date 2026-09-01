use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs::OpenOptions,
    io::{self, Write},
    path::{Path, PathBuf},
};

use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use tachiko_storage::{
    FormatError, canonicalize_roproj, compare_verified_package_with_roproj, decode_roproj_v1, load,
    load_roproj, materialize_roproj, publish_canonicalized_roproj,
    publish_portable_package_from_roproj, publish_unpacked_roproj, read_portable_package,
    read_portable_package_source, to_canonical_string,
};
use tachiko_workspace_engine::analysis_operations::{
    AnalysisDefinition, AnalysisDerivation, AnalysisFailure, AnalysisGroupKey, AnalysisOutcome,
    AnalysisPredicate, AnalysisPredicateOperator, AnalysisProjection, AnalysisResultRequest,
    AnalysisResultValue, MetricIncompleteReason, NumericAggregateOutcome, PredicateOperand,
};
use tachiko_workspace_engine::{
    CalculationFailure, Document, EditPreview, ExpressionComplexityError, FieldAddress, FieldKind,
    FieldRef, IdGenerator, MergeConflict, ReferenceFailure, SemanticChange, SemanticIdKind,
    StarterTemplate, ValidationReport, WorkspaceError, WorkspaceMergeOutcome,
    analyze_changes as analyze_semantic_changes, analyze_field as analyze_semantic_field,
    analyze_validation as analyze_semantic_validation, calculate_fields, compare_documents,
    create_document, duplicate_entity, explain_field, inspect_document,
    merge_documents as merge_semantic_documents, overview, remove_entity, rename_entity,
    runtime_export, set_formula, set_scalar, validate as validate_semantics,
};
use tachiko_workspace_engine::{
    formula_operations::{
        FormulaCalculationOutcome, FormulaOperationError, FormulaReasoningOutcome,
        FormulaReasoningResult, NumberOverride, ScenarioOutcome, ScenarioOverrideFailure,
        ScenarioRequest, ScenarioResult, ScenarioTargetOutcome, SemanticValueKind,
        ValidatorConfiguration,
    },
    patch_lifecycle::{
        AuthorizationDomainId, AuthorizationPolicyVersion, DocumentScopeId, Grant, GrantId,
        GrantRequirement, OperationFamily, PatchLifecycle, PatchLifecycleError, PolicyMeaningId,
        PrincipalId, PrincipalKind, ScopedSemanticSubject, SemanticApiContract, SemanticRevision,
        SemanticScope, TrustedInstant,
    },
};
use thiserror::Error;
use uuid::Uuid;

struct UuidV7Generator {
    document_override: Option<String>,
}

impl UuidV7Generator {
    fn new(document_override: Option<String>) -> Self {
        Self { document_override }
    }
}

impl IdGenerator for UuidV7Generator {
    fn generate(&mut self, kind: SemanticIdKind) -> String {
        if kind == SemanticIdKind::Document {
            if let Some(id) = self.document_override.take() {
                return id;
            }
        }
        Uuid::now_v7().to_string()
    }
}

#[derive(Debug, Error)]
pub enum CommandError {
    #[error(transparent)]
    Format(#[from] FormatError),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error("merge conflicts:\n{conflicts}")]
    MergeConflicts { conflicts: String },
    #[error("invalid field '{value}'; expected entity.field (for example, iron_sword.damage)")]
    InvalidFieldReference { value: String },
    #[error("analysis target '{value}' does not exist")]
    MissingAnalysisTarget { value: String },
    #[error("invalid analysis {kind} '{value}'; expected {expected}")]
    InvalidAnalysisSyntax {
        kind: &'static str,
        value: String,
        expected: &'static str,
    },
    #[error("invalid scenario override '{value}'; expected entity-id.field-id=number")]
    InvalidScenarioOverride { value: String },
    #[error(transparent)]
    FormulaOperation(#[from] FormulaOperationError),
    #[error(transparent)]
    AnalysisOperation(
        #[from] tachiko_workspace_engine::analysis_operations::AnalysisOperationError,
    ),
    #[error(transparent)]
    PatchLifecycle(#[from] PatchLifecycleError),
    #[error("output '{}' is the same as the input; choose a new path", path.display())]
    SameInputOutput { path: PathBuf },
    #[error(
        "output '{}' is inside directory input '{}'; choose a path outside the source",
        output.display(),
        input.display()
    )]
    OutputInsideDirectoryInput { input: PathBuf, output: PathBuf },
    #[error("'{}' already exists; refusing to overwrite it", path.display())]
    AlreadyExists { path: PathBuf },
    #[error("failed to create '{}': {source}", path.display())]
    Create { path: PathBuf, source: io::Error },
    #[error("failed to write '{}': {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
    #[error("could not encode command output: {0}")]
    Output(#[from] serde_json::Error),
}

fn load_read_source(path: &Path) -> Result<Document, CommandError> {
    if path.is_dir() {
        Ok(load_roproj(path)?)
    } else {
        Ok(load(path)?)
    }
}

pub fn init(
    path: &Path,
    id: Option<String>,
    title: Option<String>,
    template: StarterTemplate,
) -> Result<String, CommandError> {
    let title = title.unwrap_or_else(|| default_document_title(path));
    let mut generator = UuidV7Generator::new(id);
    let document = create_document(template, title, &mut generator)?;
    let encoded = to_canonical_string(&document)?;
    write_new(path, encoded.as_bytes())?;

    let summary = overview(&document)?;
    let mut output = format!(
        "initialized {}\n{} entities · {} formulas · ready to explore\n\nNext:\n  tachiko show {}\n",
        path.display(),
        summary.entity_count,
        summary.formula_count,
        path.display()
    );
    if template == StarterTemplate::GameBalance {
        let _ = writeln!(
            output,
            "  tachiko explain {} iron_sword.dps",
            path.display()
        );
    } else {
        output.push_str("  define schemas and entities in the canonical .ro document\n");
    }
    Ok(output)
}

pub fn validate(path: &Path) -> Result<String, CommandError> {
    let document = load_read_source(path)?;
    validate_semantics(&document)?;
    Ok(format!("valid {}\n", path.display()))
}

pub fn materialize_roproject(input: &Path, output: &Path) -> Result<String, CommandError> {
    ensure_distinct_paths(input, output)?;
    let document = load(input)?;
    validate_semantics(&document)?;
    materialize_roproj(output, &document)?;
    Ok(format!("materialized {}\n", output.display()))
}

pub fn validate_roproject(path: &Path) -> Result<String, CommandError> {
    let document = load_roproj(path)?;
    validate_semantics(&document)?;
    Ok(format!("valid {}\n", path.display()))
}

pub fn canonicalize_roproject(input: &Path, output: &Path) -> Result<String, CommandError> {
    let tree = canonicalize_roproj(input)?;
    let document = decode_roproj_v1(&tree)?;
    validate_semantics(&document)?;
    publish_canonicalized_roproj(input, output, &tree)?;
    Ok(format!("canonicalized {}\n", output.display()))
}

pub fn pack_roproject(input: &Path, output: &Path) -> Result<String, CommandError> {
    ensure_distinct_paths(input, output)?;
    let tree = read_portable_package_source(input)?;
    let document = decode_roproj_v1(&tree)?;
    validate_semantics(&document)?;
    publish_portable_package_from_roproj(input, output, &tree)?;
    Ok(format!("packed {}\n", output.display()))
}

pub fn unpack_roproject(input: &Path, output: &Path) -> Result<String, CommandError> {
    ensure_distinct_paths(input, output)?;
    let package = read_portable_package(input)?;
    let document = decode_roproj_v1(package.tree())?;
    validate_semantics(&document)?;
    publish_unpacked_roproj(output, &package)?;
    Ok(format!("unpacked {}\n", output.display()))
}

pub fn compare_roproject_package(
    package_path: &Path,
    tracked_path: &Path,
) -> Result<String, CommandError> {
    let package = read_portable_package(package_path)?;
    let package_document = decode_roproj_v1(package.tree())?;
    validate_semantics(&package_document)?;
    let tracked = read_portable_package_source(tracked_path)?;
    let tracked_document = decode_roproj_v1(&tracked)?;
    validate_semantics(&tracked_document)?;
    compare_verified_package_with_roproj(&package, &tracked)?;
    Ok(format!(
        "consistent {} {}\n",
        package_path.display(),
        tracked_path.display()
    ))
}

pub fn calculate_document(path: &Path) -> Result<String, CommandError> {
    let document = load_read_source(path)?;
    let output: BTreeMap<_, _> = calculate_fields(&document)?
        .into_iter()
        .map(|field| (field.address.to_string(), field.value))
        .collect();
    canonical_output(&output)
}

pub fn show(path: &Path) -> Result<String, CommandError> {
    let document = load_read_source(path)?;
    let view = overview(&document)?;
    let mut output = format!(
        "{} · {} schemas · {} entities · {} formulas\ndocument id: {}\n",
        document.title, view.schema_count, view.entity_count, view.formula_count, document.id
    );

    for entity in view.entities {
        let _ = writeln!(
            output,
            "\n{} · {} ({}) [{}]",
            entity.schema, entity.label, entity.key, entity.id
        );
        for field in entity.fields {
            let qualifier = match field.kind {
                FieldKind::Input => String::new(),
                FieldKind::Reference { target_schema } => {
                    format!(" (reference → {target_schema})")
                }
                FieldKind::Formula => " (formula)".to_owned(),
            };
            let _ = writeln!(
                output,
                "  {} [{}]: {}{qualifier}",
                field.key, field.id, field.display_value
            );
        }
    }
    Ok(output)
}

pub fn explain(path: &Path, field: &str) -> Result<String, CommandError> {
    let document = load_read_source(path)?;
    let field = parse_field_ref(field)?;
    let explanation = explain_field(&document, &field)?;
    let mut output = format!("{} = {}\n", explanation.address, explanation.display_value);

    if let Some(expression) = &explanation.expression {
        let _ = writeln!(output, "formula: {expression}");
    }
    if !explanation.dependencies.is_empty() {
        output.push_str("depends on:\n");
        for address in &explanation.dependency_addresses {
            let _ = writeln!(output, "  - {address}");
        }
    }
    if !explanation.affected_formulas.is_empty() {
        output.push_str("affects:\n");
        for affected in &explanation.affected_formulas {
            let _ = writeln!(
                output,
                "  - {} = {}",
                affected.address, affected.display_value
            );
        }
    }
    if explanation.expression.is_none()
        && explanation.dependencies.is_empty()
        && explanation.affected_formulas.is_empty()
    {
        output.push_str("stored input; no formulas depend on this field\n");
    }
    Ok(output)
}

pub fn analyze_document(path: &Path, source_state: Option<String>) -> Result<String, CommandError> {
    let document = load_read_source(path)?;
    canonical_output(&inspect_document(
        &document,
        analysis_source_label(path, source_state),
    ))
}

pub fn analyze_field(
    path: &Path,
    field: &str,
    source_state: Option<String>,
) -> Result<String, CommandError> {
    let document = load_read_source(path)?;
    let address = parse_field_ref(field)?;
    let target =
        document
            .resolve_field(&address)
            .map_err(|_| CommandError::MissingAnalysisTarget {
                value: field.to_owned(),
            })?;
    canonical_output(&analyze_semantic_field(
        &document,
        analysis_source_label(path, source_state),
        &target,
    )?)
}

pub fn analyze_changes(
    before_path: &Path,
    after_path: &Path,
    before_state: Option<String>,
    after_state: Option<String>,
) -> Result<String, CommandError> {
    let before = load_read_source(before_path)?;
    let after = load_read_source(after_path)?;
    let analysis = analyze_semantic_changes(
        &before,
        analysis_source_label(before_path, before_state),
        &after,
        analysis_source_label(after_path, after_state),
    )?;
    let changes = analysis
        .changes
        .iter()
        .map(semantic_change_output)
        .collect::<Vec<_>>();
    canonical_output(&json!({
        "before": analysis.before,
        "after": analysis.after,
        "changes": changes,
        "affected_schemas": analysis.affected_schemas,
        "affected_entities": analysis.affected_entities,
        "affected_fields": analysis.affected_fields,
    }))
}

pub fn analyze_validation(
    path: &Path,
    source_state: Option<String>,
) -> Result<String, CommandError> {
    let document = load_read_source(path)?;
    canonical_output(&analyze_semantic_validation(
        &document,
        analysis_source_label(path, source_state),
    ))
}

/// Run the provisional bounded typed Analysis Query adapter.
///
/// All request syntax and the disclosure-independent envelope are admitted
/// before either source is loaded. Semantic failures returned by the engine
/// remain successful structured JSON so callers never receive a partial
/// projection.
pub fn analyze_query(
    path: &Path,
    schema: &str,
    entities: &[String],
    predicates: &[String],
    group_by: Option<&str>,
    results: &[String],
    compare: Option<&Path>,
) -> Result<String, CommandError> {
    let definition = parse_analysis_definition(schema, entities, predicates, group_by, results)?;
    definition
        .admit_envelope()
        .map_err(tachiko_workspace_engine::analysis_operations::AnalysisOperationError::from)?;

    let document = load_read_source(path)?;
    let (lifecycle, scope, principal, revision) =
        local_formula_query(&document, OperationFamily::AnalysisQuery)?;
    let source = (&revision, ValidatorConfiguration::WorkspaceFull);
    let output = if let Some(compare_path) = compare {
        let comparison = load_read_source(compare_path)?;
        let (comparison_lifecycle, comparison_scope, comparison_principal, comparison_revision) =
            local_formula_query(&comparison, OperationFamily::AnalysisQuery)?;
        let result = lifecycle.query_analysis_pair(
            &scope,
            &document,
            source,
            &comparison_lifecycle,
            &comparison_scope,
            &comparison,
            (&comparison_revision, ValidatorConfiguration::WorkspaceFull),
            &definition,
            &principal,
            &comparison_principal,
            TrustedInstant::new(1),
        )?;
        analysis_pair_output(&result)
    } else {
        let result = lifecycle.query_analysis(
            &scope,
            &document,
            source,
            &definition,
            &principal,
            TrustedInstant::new(1),
        )?;
        analysis_result_output(&result)
    };
    canonical_output(&output)
}

fn parse_analysis_definition(
    schema: &str,
    entities: &[String],
    predicates: &[String],
    group_by: Option<&str>,
    results: &[String],
) -> Result<AnalysisDefinition, CommandError> {
    let schema = tachiko_workspace_engine::SchemaId::from(parse_analysis_id("schema", schema)?);
    let narrowing = if entities.is_empty() {
        None
    } else {
        Some(
            entities
                .iter()
                .map(|entity| {
                    parse_analysis_id("entity", entity)
                        .map(tachiko_workspace_engine::EntityId::from)
                })
                .collect::<Result<Vec<_>, _>>()?,
        )
    };
    let predicates = predicates
        .iter()
        .map(|predicate| parse_analysis_predicate(predicate))
        .collect::<Result<Vec<_>, _>>()?;
    let group_by = group_by
        .map(|field| {
            parse_analysis_id("group-by", field).map(tachiko_workspace_engine::FieldId::from)
        })
        .transpose()?;
    let results = results
        .iter()
        .map(|result| parse_analysis_result(result))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AnalysisDefinition::new(
        schema, narrowing, predicates, group_by, results,
    ))
}

fn parse_analysis_id(kind: &'static str, value: &str) -> Result<String, CommandError> {
    if value.is_empty() {
        return Err(CommandError::InvalidAnalysisSyntax {
            kind,
            value: value.to_owned(),
            expected: "a non-empty stable ID",
        });
    }
    Ok(value.to_owned())
}

pub fn set_document(
    input: &Path,
    field: &str,
    value: &str,
    output: &Path,
) -> Result<String, CommandError> {
    ensure_distinct_paths(input, output)?;
    ensure_output_outside_directory_source(input, output)?;
    let document = load_read_source(input)?;
    let field = parse_field_ref(field)?;
    let preview = set_scalar(&document, &field, value)?;
    let encoded = to_canonical_string(&preview.document)?;
    write_new(output, encoded.as_bytes())?;

    Ok(format!(
        "{}\nwrote {}\n\nNext:\n  tachiko diff {} {}\n",
        preview.diff.render_text().trim_end(),
        output.display(),
        input.display(),
        output.display()
    ))
}

pub fn duplicate_entity_document(
    input: &Path,
    source: &str,
    target: &str,
    output: &Path,
) -> Result<String, CommandError> {
    ensure_distinct_paths(input, output)?;
    let document = load(input)?;
    let mut generator = UuidV7Generator::new(None);
    let preview = duplicate_entity(&document, source, target, &mut generator)?;
    write_edit_preview(&preview, output)?;

    Ok(format!(
        "{}duplicated {source} as {target}\nwrote {}\n\nNext:\n  tachiko show {}\n",
        preview.diff.render_text(),
        output.display(),
        output.display()
    ))
}

pub fn rename_entity_document(
    input: &Path,
    source: &str,
    target: &str,
    output: &Path,
) -> Result<String, CommandError> {
    ensure_distinct_paths(input, output)?;
    let document = load(input)?;
    let preview = rename_entity(&document, source, target)?;
    write_edit_preview(&preview, output)?;

    Ok(format!(
        "{}renamed {source} -> {target}\nwrote {}\n\nNext:\n  tachiko show {}\n",
        preview.diff.render_text(),
        output.display(),
        output.display()
    ))
}

pub fn remove_entity_document(
    input: &Path,
    entity: &str,
    output: &Path,
) -> Result<String, CommandError> {
    ensure_distinct_paths(input, output)?;
    let document = load(input)?;
    let preview = remove_entity(&document, entity)?;
    write_edit_preview(&preview, output)?;

    Ok(format!(
        "{}removed {entity}\nwrote {}\n\nNext:\n  tachiko show {}\n",
        preview.diff.render_text(),
        output.display(),
        output.display()
    ))
}

pub fn inspect_formula(path: &Path, field: &str) -> Result<String, CommandError> {
    let document = load_read_source(path)?;
    let target = parse_stable_field_ref(field)?;
    let (lifecycle, scope, principal, revision) =
        local_formula_query(&document, OperationFamily::FormulaReasoning)?;
    let result = lifecycle.query_formula_reasoning(
        &scope,
        &document,
        (&revision, ValidatorConfiguration::WorkspaceFull),
        &target,
        &principal,
        TrustedInstant::new(1),
    )?;
    canonical_output(&formula_reasoning_output(&result))
}

pub fn run_formula_scenario(
    path: &Path,
    overrides: &[String],
    targets: &[String],
) -> Result<String, CommandError> {
    let overrides = overrides
        .iter()
        .map(|value| parse_number_override(value))
        .collect::<Result<Vec<_>, _>>()?;
    let targets = targets
        .iter()
        .map(|value| parse_stable_field_ref(value))
        .collect::<Result<Vec<_>, _>>()?;
    let request = ScenarioRequest::new(overrides, targets);
    request
        .admit_envelope()
        .map_err(FormulaOperationError::from)?;
    let document = load_read_source(path)?;
    let (lifecycle, scope, principal, revision) =
        local_formula_query(&document, OperationFamily::NumberOverrideScenario)?;
    let result = lifecycle.query_number_override_scenario(
        &scope,
        &document,
        (&revision, ValidatorConfiguration::WorkspaceFull),
        &request,
        &principal,
        TrustedInstant::new(1),
    )?;
    canonical_output(&scenario_output(&result))
}

pub fn set_formula_document(
    input: &Path,
    field: &str,
    expression: &str,
    output: &Path,
) -> Result<String, CommandError> {
    ensure_distinct_paths(input, output)?;
    let document = load(input)?;
    let field = parse_field_ref(field)?;
    let preview = set_formula(&document, &field, expression)?;
    write_edit_preview(&preview, output)?;

    Ok(format!(
        "{}wrote {}\n\nNext:\n  tachiko explain {} {}\n",
        preview.diff.render_text(),
        output.display(),
        output.display(),
        field
    ))
}

pub fn diff_documents(before: &Path, after: &Path) -> Result<String, CommandError> {
    let before = load_read_source(before)?;
    let after = load_read_source(after)?;
    Ok(compare_documents(&before, &after)?.render_text())
}

pub fn merge_documents(
    base_path: &Path,
    ours_path: &Path,
    theirs_path: &Path,
    output: &Path,
) -> Result<String, CommandError> {
    let base = load(base_path)?;
    let ours = load(ours_path)?;
    let theirs = load(theirs_path)?;
    let preview = match merge_semantic_documents(&base, &ours, &theirs)? {
        WorkspaceMergeOutcome::Merged(preview) => preview,
        WorkspaceMergeOutcome::Conflicted(conflicts) => {
            return Err(CommandError::MergeConflicts {
                conflicts: render_merge_conflicts(&conflicts),
            });
        }
    };
    let impact = preview.diff.render_text();
    let encoded = to_canonical_string(&preview.document)?;
    write_new(output, encoded.as_bytes())?;

    Ok(format!("{impact}wrote {}\n", output.display()))
}

pub fn export(input: &Path, output: &Path) -> Result<String, CommandError> {
    let document = load_read_source(input)?;
    ensure_output_outside_directory_source(input, output)?;
    let exported = runtime_export(&document)?;
    let encoded = canonical_output(&exported)?;
    write_new(output, encoded.as_bytes())?;
    Ok(format!("exported {}\n", output.display()))
}

fn default_document_title(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("document")
        .to_owned()
}

fn analysis_source_label(path: &Path, source_state: Option<String>) -> String {
    source_state.unwrap_or_else(|| path.display().to_string())
}

fn semantic_change_output(change: &SemanticChange) -> serde_json::Value {
    match change {
        SemanticChange::DocumentIdChanged { before, after } => {
            json!({ "kind": "document_id_changed", "before": before, "after": after })
        }
        SemanticChange::DocumentTitleChanged { before, after } => {
            json!({ "kind": "document_title_changed", "before": before, "after": after })
        }
        SemanticChange::SchemaAdded { .. }
        | SemanticChange::SchemaRemoved { .. }
        | SemanticChange::SchemaKeyChanged { .. }
        | SemanticChange::SchemaFieldAdded { .. }
        | SemanticChange::SchemaFieldRemoved { .. }
        | SemanticChange::SchemaFieldChanged { .. }
        | SemanticChange::FieldKeyChanged { .. } => schema_change_output(change),
        SemanticChange::EntityAdded { entity } => {
            json!({ "kind": "entity_added", "entity": entity })
        }
        SemanticChange::EntityRemoved { entity } => {
            json!({ "kind": "entity_removed", "entity": entity })
        }
        SemanticChange::EntityKeyChanged {
            entity,
            before,
            after,
        } => json!({
            "kind": "entity_key_changed",
            "entity": entity,
            "before": before,
            "after": after,
        }),
        SemanticChange::EntitySchemaChanged {
            entity,
            before,
            after,
        } => json!({
            "kind": "entity_schema_changed",
            "entity": entity,
            "before": before,
            "after": after,
        }),
        SemanticChange::FieldAdded { .. }
        | SemanticChange::FieldRemoved { .. }
        | SemanticChange::FieldChanged { .. }
        | SemanticChange::FormulaImpact { .. } => field_change_output(change),
    }
}

fn schema_change_output(change: &SemanticChange) -> serde_json::Value {
    match change {
        SemanticChange::SchemaAdded { schema, definition } => {
            json!({ "kind": "schema_added", "schema": schema, "definition": definition })
        }
        SemanticChange::SchemaRemoved { schema, definition } => {
            json!({ "kind": "schema_removed", "schema": schema, "definition": definition })
        }
        SemanticChange::SchemaKeyChanged {
            schema,
            before,
            after,
        } => json!({
            "kind": "schema_key_changed",
            "schema": schema,
            "before": before,
            "after": after,
        }),
        SemanticChange::SchemaFieldAdded {
            schema,
            field,
            definition,
        } => json!({
            "kind": "schema_field_added",
            "schema": schema,
            "field": field,
            "definition": definition,
        }),
        SemanticChange::SchemaFieldRemoved {
            schema,
            field,
            definition,
        } => json!({
            "kind": "schema_field_removed",
            "schema": schema,
            "field": field,
            "definition": definition,
        }),
        SemanticChange::SchemaFieldChanged {
            schema,
            field,
            before,
            after,
        } => json!({
            "kind": "schema_field_changed",
            "schema": schema,
            "field": field,
            "before": before,
            "after": after,
        }),
        SemanticChange::FieldKeyChanged {
            schema,
            field,
            before,
            after,
        } => json!({
            "kind": "field_key_changed",
            "schema": schema,
            "field": field,
            "before": before,
            "after": after,
        }),
        _ => unreachable!("schema change adapter called for non-schema change"),
    }
}

fn field_change_output(change: &SemanticChange) -> serde_json::Value {
    match change {
        SemanticChange::FieldAdded { field, value } => {
            json!({ "kind": "field_added", "field": field, "value": value })
        }
        SemanticChange::FieldRemoved { field, value } => {
            json!({ "kind": "field_removed", "field": field, "value": value })
        }
        SemanticChange::FieldChanged {
            field,
            before,
            after,
        } => json!({
            "kind": "field_changed",
            "field": field,
            "before": before,
            "after": after,
        }),
        SemanticChange::FormulaImpact {
            field,
            before,
            after,
            causes,
        } => json!({
            "kind": "formula_impact",
            "field": field,
            "before": before,
            "after": after,
            "causes": causes,
        }),
        _ => unreachable!("field change adapter called for non-field change"),
    }
}

fn local_formula_query(
    document: &Document,
    family: OperationFamily,
) -> Result<
    (
        PatchLifecycle,
        DocumentScopeId,
        PrincipalId,
        SemanticRevision,
    ),
    CommandError,
> {
    // This executable is the local read host: successful source loading is its
    // policy input for a fresh, read-only Delegated occurrence. The client
    // occurrence never self-selects Human kind and receives no mutation grant.
    let occurrence = Uuid::now_v7().to_string();
    let scope = DocumentScopeId::from(format!("cli-document-{occurrence}"));
    let authority = PrincipalId::from(format!("cli-host-authority-{occurrence}"));
    let principal = PrincipalId::from(format!("cli-query-client-{occurrence}"));
    let revision = semantic_revision_for(document)?;
    let document_subject =
        ScopedSemanticSubject::new(scope.clone(), document.id.clone(), SemanticScope::Document);
    let mut lifecycle = PatchLifecycle::new(
        AuthorizationDomainId::from(format!("cli-domain-{occurrence}")),
        scope.clone(),
        document.id.clone(),
        SemanticApiContract::from("tachiko-semantic-api-provisional"),
        AuthorizationPolicyVersion::from("cli-local-query-policy-v1"),
        PolicyMeaningId::from("cli-local-readable-source-query"),
    );
    lifecycle.register_principal(authority.clone(), PrincipalKind::Human)?;
    lifecycle.register_principal(principal.clone(), PrincipalKind::Delegated)?;
    lifecycle.provision_grant(Grant::new(
        GrantId::from(format!("cli-query-grant-{occurrence}")),
        authority,
        principal.clone(),
        vec![GrantRequirement::query(family, document_subject)],
        None,
    ))?;
    Ok((lifecycle, scope, principal, revision))
}

fn semantic_revision_for(document: &Document) -> Result<SemanticRevision, CommandError> {
    let digest = Sha256::digest(to_canonical_string(document)?.as_bytes());
    let mut revision = String::from("cli-semantic-sha256:");
    for byte in digest {
        let _ = write!(revision, "{byte:02x}");
    }
    Ok(SemanticRevision::from(revision))
}

fn formula_reasoning_output(result: &FormulaReasoningResult) -> serde_json::Value {
    let outcome = match &result.outcome {
        FormulaReasoningOutcome::Formula(facts) => json!({
            "kind": "formula",
            "target": facts.target,
            "expression": facts.expression,
            "direct_inputs": facts.direct_inputs,
            "direct_dependents": facts.direct_dependents,
            "affected_subjects": facts.affected_subjects,
            "calculation": calculation_output(&facts.calculation),
            "validation": validation_output(facts.validation_report.as_ref()),
        }),
        FormulaReasoningOutcome::UnresolvedTarget { target } => {
            json!({ "kind": "unresolved_target", "target": target })
        }
        FormulaReasoningOutcome::UnsupportedKind { target, actual } => json!({
            "kind": "unsupported_kind",
            "target": target,
            "actual": value_kind_name(*actual),
        }),
    };
    json!({
        "document": result.document,
        "source_revision": result.context.source_revision().as_str(),
        "validator_configuration": validator_name(result.context.validator_configuration()),
        "outcome": outcome,
    })
}

fn scenario_output(result: &ScenarioResult) -> serde_json::Value {
    let outcome = match &result.outcome {
        ScenarioOutcome::InvalidOverrides(failures) => json!({
            "kind": "invalid_overrides",
            "failures": failures.iter().map(override_failure_output).collect::<Vec<_>>(),
        }),
        ScenarioOutcome::Evaluated(evaluation) => json!({
            "kind": "evaluated",
            "baseline_validation": validation_output(evaluation.baseline_validation.as_ref()),
            "candidate_validation": validation_output(evaluation.candidate_validation.as_ref()),
            "impact": evaluation.impact.as_ref().map(|impact| json!({
                "changed_fields": impact.changed_fields,
                "affected_fields": impact.affected_fields,
            })),
            "targets": evaluation.targets.iter().map(|target| json!({
                "target": target.target,
                "outcome": scenario_target_output(&target.outcome),
            })).collect::<Vec<_>>(),
        }),
    };
    json!({
        "document": result.document,
        "source_revision": result.context.source_revision().as_str(),
        "validator_configuration": validator_name(result.context.validator_configuration()),
        "normalized_overrides": result.normalized_overrides.iter().map(|item| json!({
            "target": item.target,
            "value": item.value,
        })).collect::<Vec<_>>(),
        "outcome": outcome,
    })
}

fn scenario_target_output(outcome: &ScenarioTargetOutcome) -> serde_json::Value {
    match outcome {
        ScenarioTargetOutcome::Formula(comparison) => json!({
            "kind": "formula",
            "expression": comparison.expression,
            "direct_inputs": comparison.direct_inputs,
            "direct_dependents": comparison.direct_dependents,
            "baseline": calculation_output(&comparison.baseline),
            "candidate": calculation_output(&comparison.candidate),
        }),
        ScenarioTargetOutcome::UnresolvedTarget => json!({ "kind": "unresolved_target" }),
        ScenarioTargetOutcome::UnsupportedKind { actual } => {
            json!({ "kind": "unsupported_kind", "actual": value_kind_name(*actual) })
        }
        ScenarioTargetOutcome::DisclosureDenied => json!({ "kind": "disclosure_denied" }),
    }
}

fn override_failure_output(failure: &ScenarioOverrideFailure) -> serde_json::Value {
    match failure {
        ScenarioOverrideFailure::UnresolvedTarget { target } => {
            json!({ "kind": "unresolved_target", "target": target })
        }
        ScenarioOverrideFailure::UnsupportedKind { target, actual } => json!({
            "kind": "unsupported_kind",
            "target": target,
            "actual": value_kind_name(*actual),
        }),
    }
}

fn calculation_output(outcome: &FormulaCalculationOutcome) -> serde_json::Value {
    match outcome {
        FormulaCalculationOutcome::Value(value) => json!({ "kind": "value", "value": value }),
        FormulaCalculationOutcome::Failure(failure) => {
            json!({ "kind": "failure", "failure": calculation_failure_output(failure) })
        }
        FormulaCalculationOutcome::Unavailable => json!({ "kind": "unavailable" }),
    }
}

fn calculation_failure_output(failure: &CalculationFailure) -> serde_json::Value {
    match failure {
        CalculationFailure::InvalidExpression { error } => json!({
            "kind": "invalid_expression",
            "error": expression_complexity_name(*error),
        }),
        CalculationFailure::InvalidReferences { targets } => json!({
            "kind": "invalid_references",
            "targets": targets.iter().map(|(target, reason)| json!({
                "target": target,
                "reason": reference_failure_name(*reason),
            })).collect::<Vec<_>>(),
        }),
        CalculationFailure::Cycle { members } => json!({
            "kind": "cycle",
            "members": members,
        }),
        CalculationFailure::FailedDependencies { dependencies } => json!({
            "kind": "failed_dependencies",
            "dependencies": dependencies,
        }),
        CalculationFailure::DivisionByZero => json!({ "kind": "division_by_zero" }),
        CalculationFailure::NonFiniteResult => json!({ "kind": "non_finite_result" }),
    }
}

const fn expression_complexity_name(error: ExpressionComplexityError) -> &'static str {
    match error {
        ExpressionComplexityError::NodeLimit => "node_limit",
        ExpressionComplexityError::DepthLimit => "depth_limit",
        ExpressionComplexityError::CanonicalLengthLimit => "canonical_length_limit",
    }
}

const fn reference_failure_name(failure: ReferenceFailure) -> &'static str {
    match failure {
        ReferenceFailure::Missing => "missing",
        ReferenceFailure::NonNumeric => "non_numeric",
    }
}

fn validation_output(report: Option<&ValidationReport>) -> serde_json::Value {
    report.map_or(serde_json::Value::Null, |report| {
        json!({
            "is_valid": report.is_valid(),
            "diagnostics": report.diagnostics(),
        })
    })
}

fn parse_analysis_predicate(value: &str) -> Result<AnalysisPredicate, CommandError> {
    let parts = value.splitn(4, ':').collect::<Vec<_>>();
    if parts.len() != 4 || parts.iter().any(|part| part.is_empty()) {
        return Err(CommandError::InvalidAnalysisSyntax {
            kind: "predicate",
            value: value.to_owned(),
            expected: "field:operator:type:value",
        });
    }
    let field = tachiko_workspace_engine::FieldId::from(parts[0]);
    let operator = match parts[1] {
        "=" | "==" | "eq" => AnalysisPredicateOperator::Equal,
        "!=" | "ne" => AnalysisPredicateOperator::NotEqual,
        "<" | "lt" => AnalysisPredicateOperator::LessThan,
        "<=" | "lte" => AnalysisPredicateOperator::LessThanOrEqual,
        ">" | "gt" => AnalysisPredicateOperator::GreaterThan,
        ">=" | "gte" => AnalysisPredicateOperator::GreaterThanOrEqual,
        _ => {
            return Err(CommandError::InvalidAnalysisSyntax {
                kind: "predicate",
                value: value.to_owned(),
                expected: "field:operator:type:value (operator: =, !=, <, <=, >, >=)",
            });
        }
    };
    let operand = match parts[2] {
        "number" => parts[3]
            .parse::<f64>()
            .ok()
            .and_then(|n| tachiko_workspace_engine::Number::new(n).ok())
            .map(PredicateOperand::Number)
            .ok_or_else(|| CommandError::InvalidAnalysisSyntax {
                kind: "predicate",
                value: value.to_owned(),
                expected: "a finite number operand",
            })?,
        "text" => PredicateOperand::Text(parts[3].to_owned()),
        "boolean" => match parts[3] {
            "true" => PredicateOperand::Boolean(true),
            "false" => PredicateOperand::Boolean(false),
            _ => {
                return Err(CommandError::InvalidAnalysisSyntax {
                    kind: "predicate",
                    value: value.to_owned(),
                    expected: "boolean value true or false",
                });
            }
        },
        "reference" => {
            PredicateOperand::Reference(tachiko_workspace_engine::EntityId::from(parts[3]))
        }
        _ => {
            return Err(CommandError::InvalidAnalysisSyntax {
                kind: "predicate",
                value: value.to_owned(),
                expected: "type number, text, boolean, or reference",
            });
        }
    };
    Ok(AnalysisPredicate::new(field, operator, operand))
}

fn parse_analysis_result(value: &str) -> Result<AnalysisResultRequest, CommandError> {
    Ok(match value {
        "membership" => AnalysisResultRequest::Membership,
        "count" => AnalysisResultRequest::Count,
        value if value.starts_with("min:") => {
            AnalysisResultRequest::Minimum(parse_analysis_result_field(value, "min:")?)
        }
        value if value.starts_with("max:") => {
            AnalysisResultRequest::Maximum(parse_analysis_result_field(value, "max:")?)
        }
        value if value.starts_with("observations:") => AnalysisResultRequest::Observations(
            parse_analysis_result_field(value, "observations:")?,
        ),
        _ => {
            return Err(CommandError::InvalidAnalysisSyntax {
                kind: "result",
                value: value.to_owned(),
                expected: "membership, count, min:FIELD, max:FIELD, or observations:FIELD",
            });
        }
    })
}

fn parse_analysis_result_field(
    value: &str,
    prefix: &str,
) -> Result<tachiko_workspace_engine::FieldId, CommandError> {
    let field = value.strip_prefix(prefix).unwrap_or_default();
    if field.is_empty() {
        return Err(CommandError::InvalidAnalysisSyntax {
            kind: "result",
            value: value.to_owned(),
            expected: "a non-empty stable field ID",
        });
    }
    Ok(tachiko_workspace_engine::FieldId::from(field))
}

fn analysis_result_output(
    result: &tachiko_workspace_engine::analysis_operations::AnalysisQueryResult,
) -> serde_json::Value {
    let mut output = analysis_lineage_output(&result.lineage);
    if let serde_json::Value::Object(fields) = &mut output {
        fields.insert(
            "outcome".to_owned(),
            analysis_outcome_output(&result.outcome),
        );
    }
    output
}

fn analysis_pair_output(
    result: &tachiko_workspace_engine::analysis_operations::PairedAnalysisQueryResult,
) -> serde_json::Value {
    let mut output = analysis_lineage_output(&result.lineage);
    if let serde_json::Value::Object(fields) = &mut output {
        fields.insert("first".to_owned(), analysis_outcome_output(&result.first));
        fields.insert("second".to_owned(), analysis_outcome_output(&result.second));
    }
    output
}

fn analysis_lineage_output(
    lineage: &tachiko_workspace_engine::analysis_operations::AnalysisLineage,
) -> serde_json::Value {
    json!({
        "sources": lineage.sources.iter().map(|source| json!({ "document": source.document, "source_revision": source.source_revision.as_str(), "validator_configuration": validator_name(source.validator_configuration) })).collect::<Vec<_>>(),
        "normalized_definition": normalized_analysis_definition_output(&lineage.normalized_definition),
        "formula_calculation_used": lineage.formula_calculation_used,
        "derivations": lineage.derivations.iter().map(analysis_derivation_output).collect::<Vec<_>>(),
    })
}

fn normalized_analysis_definition_output(
    definition: &tachiko_workspace_engine::analysis_operations::NormalizedAnalysisDefinition,
) -> serde_json::Value {
    json!({
        "schema": definition.schema,
        "narrowing": definition.narrowing.as_ref().map(|entities| entities.iter().collect::<Vec<_>>()),
        "predicates": definition.predicates.iter().map(|p| json!({ "field": p.field, "operator": analysis_operator_name(p.operator), "operand": analysis_operand_output(&p.operand) })).collect::<Vec<_>>(),
        "group_by": definition.group_by,
        "results": definition.results.iter().map(analysis_result_request_output).collect::<Vec<_>>(),
    })
}

fn analysis_operand_output(operand: &PredicateOperand) -> serde_json::Value {
    match operand {
        PredicateOperand::Number(v) => json!({ "type": "number", "value": v }),
        PredicateOperand::Text(v) => json!({ "type": "text", "value": v }),
        PredicateOperand::Boolean(v) => json!({ "type": "boolean", "value": v }),
        PredicateOperand::Reference(v) => json!({ "type": "reference", "value": v }),
    }
}

fn analysis_result_request_output(request: &AnalysisResultRequest) -> serde_json::Value {
    match request {
        AnalysisResultRequest::Membership => json!({ "kind": "membership" }),
        AnalysisResultRequest::Count => json!({ "kind": "count" }),
        AnalysisResultRequest::Minimum(f) => json!({ "kind": "minimum", "field": f }),
        AnalysisResultRequest::Maximum(f) => json!({ "kind": "maximum", "field": f }),
        AnalysisResultRequest::Observations(f) => json!({ "kind": "observations", "field": f }),
    }
}

fn analysis_derivation_output(derivation: &AnalysisDerivation) -> serde_json::Value {
    match derivation {
        AnalysisDerivation::Predicate(f) => json!({ "kind": "predicate", "field": f }),
        AnalysisDerivation::GroupedBy(f) => json!({ "kind": "grouped_by", "field": f }),
        AnalysisDerivation::Membership => json!({ "kind": "membership" }),
        AnalysisDerivation::Count => json!({ "kind": "count" }),
        AnalysisDerivation::Minimum(f) => json!({ "kind": "minimum", "field": f }),
        AnalysisDerivation::Maximum(f) => json!({ "kind": "maximum", "field": f }),
        AnalysisDerivation::Observations(f) => json!({ "kind": "observations", "field": f }),
    }
}

fn analysis_outcome_output(outcome: &AnalysisOutcome) -> serde_json::Value {
    match outcome {
        AnalysisOutcome::Complete(p) => {
            json!({ "kind": "complete", "projection": analysis_projection_output(p) })
        }
        AnalysisOutcome::Failure(f) => {
            json!({ "kind": "failure", "failure": analysis_failure_output(f) })
        }
    }
}

fn analysis_projection_output(projection: &AnalysisProjection) -> serde_json::Value {
    match projection {
        AnalysisProjection::Ungrouped(b) => {
            json!({ "kind": "ungrouped", "bucket": analysis_bucket_output(b) })
        }
        AnalysisProjection::Grouped(groups) => {
            json!({ "kind": "grouped", "groups": groups.iter().map(|g| json!({ "key": analysis_group_key_output(&g.key), "bucket": analysis_bucket_output(&g.bucket) })).collect::<Vec<_>>() })
        }
    }
}

fn analysis_group_key_output(key: &AnalysisGroupKey) -> serde_json::Value {
    match key {
        AnalysisGroupKey::Number(v) => json!({ "type": "number", "value": v }),
        AnalysisGroupKey::Text(v) => json!({ "type": "text", "value": v }),
        AnalysisGroupKey::Boolean(v) => json!({ "type": "boolean", "value": v }),
        AnalysisGroupKey::Reference(v) => json!({ "type": "reference", "value": v }),
    }
}

fn analysis_bucket_output(
    bucket: &tachiko_workspace_engine::analysis_operations::AnalysisBucket,
) -> serde_json::Value {
    json!({ "values": bucket.values.iter().map(analysis_result_value_output).collect::<Vec<_>>() })
}

fn analysis_result_value_output(value: &AnalysisResultValue) -> serde_json::Value {
    match value {
        AnalysisResultValue::Membership(es) => json!({ "kind": "membership", "entities": es }),
        AnalysisResultValue::Count(n) => json!({ "kind": "count", "value": n }),
        AnalysisResultValue::Minimum { field, outcome } => {
            json!({ "kind": "minimum", "field": field, "outcome": numeric_aggregate_output(outcome) })
        }
        AnalysisResultValue::Maximum { field, outcome } => {
            json!({ "kind": "maximum", "field": field, "outcome": numeric_aggregate_output(outcome) })
        }
        AnalysisResultValue::Observations { field, values } => {
            json!({ "kind": "observations", "field": field, "values": values.iter().map(|(e, v)| json!({ "entity": e, "value": v })).collect::<Vec<_>>() })
        }
    }
}

fn numeric_aggregate_output(outcome: &NumericAggregateOutcome) -> serde_json::Value {
    match outcome {
        NumericAggregateOutcome::Value(v) => json!({ "kind": "value", "value": v }),
        NumericAggregateOutcome::Empty => json!({ "kind": "empty" }),
    }
}

fn analysis_failure_output(failure: &AnalysisFailure) -> serde_json::Value {
    match failure {
        AnalysisFailure::UnresolvedSchema { schema } => {
            json!({ "kind": "unresolved_schema", "schema": schema })
        }
        AnalysisFailure::UnresolvedField { role, field } => {
            json!({ "kind": "unresolved_field", "role": analysis_field_role_name(*role), "field": field })
        }
        AnalysisFailure::UnresolvedNarrowingEntity { entity } => {
            json!({ "kind": "unresolved_narrowing_entity", "entity": entity })
        }
        AnalysisFailure::WrongDomainNarrowingEntity {
            entity,
            expected,
            actual,
        } => {
            json!({ "kind": "wrong_domain_narrowing_entity", "entity": entity, "expected": expected, "actual": actual })
        }
        AnalysisFailure::IncoherentCandidateIdentity { key, entity } => {
            json!({ "kind": "incoherent_candidate_identity", "key": key, "entity": entity })
        }
        AnalysisFailure::InvalidPredicateType { field, declared } => {
            json!({ "kind": "invalid_predicate_type", "field": field, "declared": declared })
        }
        AnalysisFailure::InvalidMetricType { field, declared } => {
            json!({ "kind": "invalid_metric_type", "field": field, "declared": declared })
        }
        AnalysisFailure::InvalidPredicateValue {
            entity,
            field,
            actual,
        } => {
            json!({ "kind": "invalid_predicate_value", "entity": entity, "field": field, "actual": analysis_value_kind_name(*actual) })
        }
        AnalysisFailure::MissingGroupValue { entity, field } => {
            json!({ "kind": "missing_group_value", "entity": entity, "field": field })
        }
        AnalysisFailure::FormulaGroupingUnsupported { field } => {
            json!({ "kind": "formula_grouping_unsupported", "field": field })
        }
        AnalysisFailure::InvalidGroupValue {
            entity,
            field,
            actual,
        } => {
            json!({ "kind": "invalid_group_value", "entity": entity, "field": field, "actual": analysis_value_kind_name(*actual) })
        }
        AnalysisFailure::CalculationFailed { field, failure } => {
            json!({ "kind": "calculation_failed", "field": field, "failure": failure.as_ref().map(calculation_failure_output) })
        }
        AnalysisFailure::MetricIncomplete {
            entity,
            field,
            reason,
        } => {
            json!({ "kind": "metric_incomplete", "entity": entity, "field": field, "reason": metric_incomplete_reason_output(reason) })
        }
        AnalysisFailure::ResultTooLarge { collection, limit } => {
            json!({ "kind": "result_too_large", "collection": analysis_collection_kind_name(*collection), "limit": limit })
        }
    }
}

fn metric_incomplete_reason_output(reason: &MetricIncompleteReason) -> serde_json::Value {
    match reason {
        MetricIncompleteReason::Missing => json!({ "kind": "missing" }),
        MetricIncompleteReason::WrongKind(k) => {
            json!({ "kind": "wrong_kind", "actual": analysis_value_kind_name(*k) })
        }
    }
}

const fn analysis_operator_name(operator: AnalysisPredicateOperator) -> &'static str {
    match operator {
        AnalysisPredicateOperator::Equal => "equal",
        AnalysisPredicateOperator::NotEqual => "not_equal",
        AnalysisPredicateOperator::LessThan => "less_than",
        AnalysisPredicateOperator::LessThanOrEqual => "less_than_or_equal",
        AnalysisPredicateOperator::GreaterThan => "greater_than",
        AnalysisPredicateOperator::GreaterThanOrEqual => "greater_than_or_equal",
    }
}
const fn analysis_field_role_name(
    role: tachiko_workspace_engine::analysis_operations::AnalysisFieldRole,
) -> &'static str {
    match role {
        tachiko_workspace_engine::analysis_operations::AnalysisFieldRole::Predicate => "predicate",
        tachiko_workspace_engine::analysis_operations::AnalysisFieldRole::Group => "group",
        tachiko_workspace_engine::analysis_operations::AnalysisFieldRole::Metric => "metric",
    }
}
const fn analysis_value_kind_name(
    kind: tachiko_workspace_engine::analysis_operations::AnalysisValueKind,
) -> &'static str {
    match kind {
        tachiko_workspace_engine::analysis_operations::AnalysisValueKind::Number => "number",
        tachiko_workspace_engine::analysis_operations::AnalysisValueKind::Formula => "formula",
        tachiko_workspace_engine::analysis_operations::AnalysisValueKind::Text => "text",
        tachiko_workspace_engine::analysis_operations::AnalysisValueKind::Boolean => "boolean",
        tachiko_workspace_engine::analysis_operations::AnalysisValueKind::Reference => "reference",
    }
}
const fn analysis_collection_kind_name(
    kind: tachiko_workspace_engine::analysis_operations::AnalysisCollectionKind,
) -> &'static str {
    match kind {
        tachiko_workspace_engine::analysis_operations::AnalysisCollectionKind::Membership => {
            "membership"
        }
        tachiko_workspace_engine::analysis_operations::AnalysisCollectionKind::Groups => "groups",
        tachiko_workspace_engine::analysis_operations::AnalysisCollectionKind::Observations => {
            "observations"
        }
    }
}

const fn validator_name(configuration: ValidatorConfiguration) -> &'static str {
    match configuration {
        ValidatorConfiguration::WorkspaceFull => "workspace_full",
    }
}

const fn value_kind_name(kind: SemanticValueKind) -> &'static str {
    match kind {
        SemanticValueKind::Number => "number",
        SemanticValueKind::Formula => "formula",
        SemanticValueKind::Text => "text",
        SemanticValueKind::Boolean => "boolean",
        SemanticValueKind::Reference => "reference",
    }
}

fn parse_stable_field_ref(value: &str) -> Result<FieldRef, CommandError> {
    let address = parse_field_ref(value)?;
    Ok(FieldRef::new(
        address.entity.as_str(),
        address.field.as_str(),
    ))
}

fn parse_number_override(value: &str) -> Result<NumberOverride, CommandError> {
    let Some((target, number)) = value.split_once('=') else {
        return Err(CommandError::InvalidScenarioOverride {
            value: value.to_owned(),
        });
    };
    let target =
        parse_stable_field_ref(target).map_err(|_| CommandError::InvalidScenarioOverride {
            value: value.to_owned(),
        })?;
    let value_number =
        number
            .parse::<f64>()
            .map_err(|_| CommandError::InvalidScenarioOverride {
                value: value.to_owned(),
            })?;
    Ok(NumberOverride::new(target, value_number))
}

fn parse_field_ref(value: &str) -> Result<FieldAddress, CommandError> {
    let Some((entity, field)) = value.split_once('.') else {
        return Err(CommandError::InvalidFieldReference {
            value: value.to_owned(),
        });
    };
    if entity.is_empty() || field.is_empty() || field.contains('.') {
        return Err(CommandError::InvalidFieldReference {
            value: value.to_owned(),
        });
    }
    Ok(FieldAddress::new(entity, field))
}

fn canonical_output(value: &impl Serialize) -> Result<String, CommandError> {
    let mut output = serde_json::to_string_pretty(value)?;
    output.push('\n');
    Ok(output)
}

fn ensure_distinct_paths(input: &Path, output: &Path) -> Result<(), CommandError> {
    if input == output {
        return Err(CommandError::SameInputOutput {
            path: input.to_owned(),
        });
    }
    Ok(())
}

fn ensure_output_outside_directory_source(input: &Path, output: &Path) -> Result<(), CommandError> {
    if !input.is_dir() {
        return Ok(());
    }

    let output_parent = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let is_inside = match (input.canonicalize(), output_parent.canonicalize()) {
        (Ok(input), Ok(output_parent)) => output_parent.starts_with(input),
        _ => output.starts_with(input),
    };
    if is_inside {
        return Err(CommandError::OutputInsideDirectoryInput {
            input: input.to_owned(),
            output: output.to_owned(),
        });
    }
    Ok(())
}

fn write_edit_preview(preview: &EditPreview, output: &Path) -> Result<(), CommandError> {
    let encoded = to_canonical_string(&preview.document)?;
    write_new(output, encoded.as_bytes())
}

fn write_new(path: &Path, contents: &[u8]) -> Result<(), CommandError> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                CommandError::AlreadyExists {
                    path: path.to_owned(),
                }
            } else {
                CommandError::Create {
                    path: path.to_owned(),
                    source,
                }
            }
        })?;
    file.write_all(contents)
        .map_err(|source| CommandError::Write {
            path: path.to_owned(),
            source,
        })
}

fn render_merge_conflicts(conflicts: &[MergeConflict]) -> String {
    let mut output = String::new();
    for conflict in conflicts {
        let _ = writeln!(
            output,
            "  {:?} / {:?} / {:?}",
            conflict.target(),
            conflict.facet(),
            conflict.kind()
        );
        let _ = writeln!(output, "    base: {:?}", conflict.base());
        let _ = writeln!(output, "    left: {:?}", conflict.left());
        let _ = writeln!(output, "    right: {:?}", conflict.right());
    }
    output.trim_end().to_owned()
}
