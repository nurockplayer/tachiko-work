use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs::OpenOptions,
    io::{self, Write},
    path::{Path, PathBuf},
};

use serde::Serialize;
use tachiko_storage::{FormatError, load, to_canonical_string};
use tachiko_workspace_engine::{
    EditPreview, FieldAddress, FieldKind, IdGenerator, MergeConflict, SemanticIdKind,
    StarterTemplate, WorkspaceError, WorkspaceMergeOutcome, calculate_fields, compare_documents,
    create_document, duplicate_entity, explain_field, merge_documents as merge_semantic_documents,
    overview, remove_entity, rename_entity, runtime_export, set_formula, set_scalar,
    validate as validate_semantics,
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
    #[error("output '{}' is the same as the input; choose a new path", path.display())]
    SameInputOutput { path: PathBuf },
    #[error("'{}' already exists; refusing to overwrite it", path.display())]
    AlreadyExists { path: PathBuf },
    #[error("failed to create '{}': {source}", path.display())]
    Create { path: PathBuf, source: io::Error },
    #[error("failed to write '{}': {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
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
    validate_semantics(&document)?;
    Ok(format!("valid {}\n", path.display()))
}

pub fn calculate_document(path: &Path) -> Result<String, CommandError> {
    let document = load(path)?;
    let output: BTreeMap<_, _> = calculate_fields(&document)?
        .into_iter()
        .map(|field| (field.address.to_string(), field.value))
        .collect();
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
    let document = load(input)?;
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
