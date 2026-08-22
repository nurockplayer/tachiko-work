use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs::OpenOptions,
    io::{self, Write},
    path::{Path, PathBuf},
};

use serde::Serialize;
use serde_json::{Value as JsonValue, json};
use tachiko_diff_engine::{DiffError, diff};
use tachiko_formula_engine::{Calculation, CalculationError, calculate};
use tachiko_merge_engine::{MergeConflict, MergeError, MergeOutcome, merge};
use tachiko_semantic_core::{AddressIndex, Document, FieldAddress, FieldRef, Value};
use tachiko_storage::{FormatError, load, to_canonical_string};
use tachiko_workflow::{
    EditPreview, FieldKind, IdGenerator, SemanticIdKind, StarterTemplate, WorkflowError,
    create_document, duplicate_entity, explain_field, overview, remove_entity, rename_entity,
    set_formula, set_scalar,
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
    Calculation(#[from] CalculationError),
    #[error(transparent)]
    Diff(#[from] DiffError),
    #[error(transparent)]
    Workflow(#[from] WorkflowError),
    #[error(transparent)]
    Merge(#[from] MergeError),
    #[error("merge conflicts:\n{conflicts}")]
    MergeConflicts { conflicts: String },
    #[error("invalid field '{value}'; expected entity.field (for example, iron_sword.damage)")]
    InvalidFieldReference { value: String },
    #[error("output '{}' is the same as the input; choose a new path", path.display())]
    SameInputOutput { path: PathBuf },
    #[error("'{}' already exists; refusing to overwrite it", path.display())]
    AlreadyExists { path: PathBuf },
    #[error("failed to create '{}': {source}", path.display())]
    Create { path: PathBuf, source: io::Error },
    #[error("failed to write '{}': {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
    #[error("calculation did not produce a value for '{field}'")]
    MissingCalculation { field: FieldRef },
    #[error("could not encode command output: {0}")]
    Output(#[from] serde_json::Error),
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
    let document = load(path)?;
    calculate(&document)?;
    Ok(format!("valid {}\n", path.display()))
}

pub fn calculate_document(path: &Path) -> Result<String, CommandError> {
    let document = load(path)?;
    let calculation = calculate(&document)?;
    let index = AddressIndex::build(&document).map_err(WorkflowError::from)?;
    let output: BTreeMap<_, _> = calculation
        .values()
        .iter()
        .map(|(field, value)| {
            index
                .field_address(&document, field)
                .map(|address| (address.to_string(), value))
                .map_err(WorkflowError::from)
        })
        .collect::<Result<_, _>>()?;
    canonical_output(&output)
}

pub fn show(path: &Path) -> Result<String, CommandError> {
    let document = load(path)?;
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
    let document = load(path)?;
    let field = parse_field_ref(field)?;
    let explanation = explain_field(&document, &field)?;
    let index = AddressIndex::build(&document).map_err(WorkflowError::from)?;
    let mut output = format!("{} = {}\n", explanation.address, explanation.display_value);

    if let Some(expression) = &explanation.expression {
        let _ = writeln!(output, "formula: {expression}");
    }
    if !explanation.dependencies.is_empty() {
        output.push_str("depends on:\n");
        for dependency in &explanation.dependencies {
            let address = index
                .field_address(&document, dependency)
                .map_err(WorkflowError::from)?;
            let _ = writeln!(output, "  - {address}");
        }
    }
    if !explanation.affected_formulas.is_empty() {
        output.push_str("affects:\n");
        for affected in &explanation.affected_formulas {
            let address = index
                .field_address(&document, &affected.field)
                .map_err(WorkflowError::from)?;
            let _ = writeln!(output, "  - {} = {}", address, affected.display_value);
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

pub fn set_document(
    input: &Path,
    field: &str,
    value: &str,
    output: &Path,
) -> Result<String, CommandError> {
    if input == output {
        return Err(CommandError::SameInputOutput {
            path: input.to_owned(),
        });
    }
    let document = load(input)?;
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
    let before = load(before)?;
    let after = load(after)?;
    Ok(diff(&before, &after)?.render_text())
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
    let merged = match merge(&base, &ours, &theirs)? {
        MergeOutcome::Merged(document) => document,
        MergeOutcome::Conflicted(conflicts) => {
            return Err(CommandError::MergeConflicts {
                conflicts: render_merge_conflicts(&conflicts),
            });
        }
    };
    let impact = diff(&base, &merged)?.render_text();
    let encoded = to_canonical_string(&merged)?;
    write_new(output, encoded.as_bytes())?;

    Ok(format!("{impact}wrote {}\n", output.display()))
}

pub fn export(input: &Path, output: &Path) -> Result<String, CommandError> {
    let document = load(input)?;
    let calculation = calculate(&document)?;
    let exported = ExportDocument::new(&document, &calculation)?;
    let encoded = canonical_output(&exported)?;
    write_new(output, encoded.as_bytes())?;
    Ok(format!("exported {}\n", output.display()))
}

fn default_document_title(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("document");
    let mut identifier = String::new();
    let mut pending_separator = false;
    for character in stem.chars() {
        if character.is_ascii_alphanumeric() {
            if pending_separator && !identifier.is_empty() {
                identifier.push('-');
            }
            identifier.push(character.to_ascii_lowercase());
            pending_separator = false;
        } else if matches!(character, '_' | '-') {
            if pending_separator && !identifier.is_empty() {
                identifier.push('-');
            }
            identifier.push(character);
            pending_separator = false;
        } else {
            pending_separator = true;
        }
    }
    while identifier.ends_with(['-', '_']) {
        identifier.pop();
    }
    if identifier.is_empty() {
        "document".to_owned()
    } else {
        identifier
    }
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
        let _ = writeln!(output, "  {}", conflict.path);
        let _ = writeln!(output, "    base: {:?}", conflict.base);
        let _ = writeln!(output, "    ours: {:?}", conflict.ours);
        let _ = writeln!(output, "    theirs: {:?}", conflict.theirs);
    }
    output.trim_end().to_owned()
}

#[derive(Serialize)]
struct ExportDocument {
    format_version: u32,
    document_id: String,
    title: String,
    entities: BTreeMap<String, ExportEntity>,
}

impl ExportDocument {
    fn new(document: &Document, calculation: &Calculation) -> Result<Self, CommandError> {
        let mut entities = BTreeMap::new();
        for (entity_id, entity) in &document.entities {
            let schema = &document.schemas[&entity.schema];
            let mut fields = BTreeMap::new();
            for (field_id, value) in &entity.fields {
                let field_ref = FieldRef {
                    entity: entity_id.clone(),
                    field: field_id.clone(),
                };
                fields.insert(
                    schema.fields[field_id].key.to_string(),
                    export_value(document, value, &field_ref, calculation)?,
                );
            }
            entities.insert(
                entity.key.to_string(),
                ExportEntity {
                    schema: schema.key.to_string(),
                    fields,
                },
            );
        }

        Ok(Self {
            format_version: RUNTIME_EXPORT_VERSION,
            document_id: document.id.to_string(),
            title: document.title.clone(),
            entities,
        })
    }
}

const RUNTIME_EXPORT_VERSION: u32 = 2;

#[derive(Serialize)]
struct ExportEntity {
    schema: String,
    fields: BTreeMap<String, JsonValue>,
}

fn export_value(
    document: &Document,
    value: &Value,
    field: &FieldRef,
    calculation: &Calculation,
) -> Result<JsonValue, CommandError> {
    match value {
        Value::Number(number) => Ok(json!(number)),
        Value::Text(text) => Ok(json!(text)),
        Value::Boolean(boolean) => Ok(json!(boolean)),
        Value::Reference(entity) => Ok(json!({
            "reference": document.entities[entity].key.as_str()
        })),
        Value::Formula(_) => calculation
            .value(field)
            .map(|number| json!(number))
            .ok_or_else(|| CommandError::MissingCalculation {
                field: field.clone(),
            }),
    }
}
