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
use tachiko_semantic_core::{Document, FieldRef, Value};
use tachiko_storage::{FormatError, load, to_canonical_string};
use tachiko_workflow::{
    FieldKind, StarterTemplate, WorkflowError, create_document, explain_field, overview, set_scalar,
};
use thiserror::Error;

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
    let id = id.unwrap_or_else(|| default_document_id(path));
    let title = title.unwrap_or_else(|| id.clone());
    let document = create_document(template, id, title);
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
    let output: BTreeMap<_, _> = calculation
        .values()
        .iter()
        .map(|(field, value)| (field.to_string(), value))
        .collect();
    canonical_output(&output)
}

pub fn show(path: &Path) -> Result<String, CommandError> {
    let document = load(path)?;
    let view = overview(&document)?;
    let mut output = format!(
        "{}\n{} · {} schemas · {} entities · {} formulas\n",
        document.title, document.id, view.schema_count, view.entity_count, view.formula_count
    );

    for entity in view.entities {
        let _ = writeln!(
            output,
            "\n{} · {} [{}]",
            entity.schema, entity.label, entity.id
        );
        for field in entity.fields {
            let qualifier = match field.kind {
                FieldKind::Input => String::new(),
                FieldKind::Reference { target_schema } => {
                    format!(" (reference → {target_schema})")
                }
                FieldKind::Formula => " (formula)".to_owned(),
            };
            let _ = writeln!(output, "  {}: {}{qualifier}", field.id, field.display_value);
        }
    }
    Ok(output)
}

pub fn explain(path: &Path, field: &str) -> Result<String, CommandError> {
    let document = load(path)?;
    let field = parse_field_ref(field)?;
    let explanation = explain_field(&document, &field)?;
    let mut output = format!("{} = {}\n", explanation.field, explanation.display_value);

    if let Some(expression) = &explanation.expression {
        let _ = writeln!(output, "formula: {expression}");
    }
    if !explanation.dependencies.is_empty() {
        output.push_str("depends on:\n");
        for dependency in &explanation.dependencies {
            let _ = writeln!(output, "  - {dependency}");
        }
    }
    if !explanation.affected_formulas.is_empty() {
        output.push_str("affects:\n");
        for affected in &explanation.affected_formulas {
            let _ = writeln!(
                output,
                "  - {} = {}",
                affected.field, affected.display_value
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

pub fn diff_documents(before: &Path, after: &Path) -> Result<String, CommandError> {
    let before = load(before)?;
    let after = load(after)?;
    Ok(diff(&before, &after)?.render_text())
}

pub fn export(input: &Path, output: &Path) -> Result<String, CommandError> {
    let document = load(input)?;
    let calculation = calculate(&document)?;
    let exported = ExportDocument::new(&document, &calculation)?;
    let encoded = canonical_output(&exported)?;
    write_new(output, encoded.as_bytes())?;
    Ok(format!("exported {}\n", output.display()))
}

fn default_document_id(path: &Path) -> String {
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

fn parse_field_ref(value: &str) -> Result<FieldRef, CommandError> {
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
    Ok(FieldRef::new(entity, field))
}

fn canonical_output(value: &impl Serialize) -> Result<String, CommandError> {
    let mut output = serde_json::to_string_pretty(value)?;
    output.push('\n');
    Ok(output)
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
            let mut fields = BTreeMap::new();
            for (field_id, value) in &entity.fields {
                let field_ref = FieldRef {
                    entity: entity_id.clone(),
                    field: field_id.clone(),
                };
                fields.insert(
                    field_id.to_string(),
                    export_value(value, &field_ref, calculation)?,
                );
            }
            entities.insert(
                entity_id.to_string(),
                ExportEntity {
                    schema: entity.schema.to_string(),
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

const RUNTIME_EXPORT_VERSION: u32 = 1;

#[derive(Serialize)]
struct ExportEntity {
    schema: String,
    fields: BTreeMap<String, JsonValue>,
}

fn export_value(
    value: &Value,
    field: &FieldRef,
    calculation: &Calculation,
) -> Result<JsonValue, CommandError> {
    match value {
        Value::Number(number) => Ok(json!(number)),
        Value::Text(text) => Ok(json!(text)),
        Value::Boolean(boolean) => Ok(json!(boolean)),
        Value::Reference(entity) => Ok(json!({ "reference": entity.as_str() })),
        Value::Formula(_) => calculation
            .value(field)
            .map(|number| json!(number))
            .ok_or_else(|| CommandError::MissingCalculation {
                field: field.clone(),
            }),
    }
}
