//! Minimal research-only bridge for cross-crate Issue #175 oracle tests.

use std::collections::{BTreeMap, BTreeSet};

use tachiko_semantic_core::{Document, DocumentId, EntityId};

use super::{
    CanonicalRoProjectV1, EntityV1, ExpressionV1, ROPROJ_V1_MAX_JSON_NESTING, ROPROJ_V1_PATHS,
    SchemaV1, ValueV1, decode, decode_json_file, decode_manifest, deserialize_roproj,
    ensure_increasing, invalid_representation, map_frontend_error, render_manifest, render_schemas,
    require_id, schemas_into_semantic, shard_index, utf8, validate_semantic_expression_limits,
    write_entity,
};
use crate::{FormatError, strict_json::inspect_roproj};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AdmissionWork {
    pub(crate) source_bytes: usize,
    pub(crate) nesting_scan_bytes: usize,
    pub(crate) json_parser_deserializer_bytes: usize,
    pub(crate) canonical_render_bytes: usize,
    pub(crate) entity_records: usize,
    pub(crate) formula_ast_nodes: usize,
    pub(crate) reference_edges: usize,
    pub(crate) formula_dependency_edges: usize,
}

/// Run current A0 and the research A1 over one byte-identical canonical tree.
///
/// # Errors
///
/// Returns the exact storage admission error from canonical encoding, A0, or
/// A1. This helper exists only when the `issue-175-research` feature is enabled.
#[cfg_attr(test, allow(dead_code))]
pub fn issue_175_admit_a0_a1(document: &Document) -> Result<(Document, Document), FormatError> {
    let tree = super::encode(document)?;
    let files = tree
        .files()
        .iter()
        .map(|file| (file.path().to_owned(), file.bytes().to_vec()))
        .collect::<Vec<_>>();
    let a0_tree = CanonicalRoProjectV1::try_from_files(files.clone())?;
    let a0 = decode(&a0_tree)?;
    let (a1, _) = admit_one_pass_exact(&files, false)?;
    Ok((a0, a1))
}

pub(crate) fn admit_one_pass_exact(
    files: &[(String, Vec<u8>)],
    collect_work: bool,
) -> Result<(Document, AdmissionWork), FormatError> {
    require_exact_paths(files)?;
    let source_bytes = files.iter().map(|(_, bytes)| bytes.len()).sum();
    let mut strict_input_bytes = files[0].1.len() + files[1].1.len();

    let manifest = decode_manifest(&files[0].1)?;
    if render_manifest(&manifest)?.as_bytes() != files[0].1 {
        return invalid_representation("manifest.json is not canonical .roproj/v1".to_owned());
    }
    let schema_dtos: Vec<SchemaV1> = decode_json_file(ROPROJ_V1_PATHS[1], &files[1].1)?;
    if render_schemas(&schema_dtos)?.as_bytes() != files[1].1 {
        return invalid_representation("schemas.json is not canonical .roproj/v1".to_owned());
    }
    let schemas = schemas_into_semantic(schema_dtos)?;

    let mut entities = BTreeMap::new();
    let mut entity_records = 0;
    let mut formula_ast_nodes = 0;
    let mut reference_edges = 0;
    let mut formula_dependency_edges = 0;
    for (shard, (path, bytes)) in files.iter().enumerate().skip(2) {
        if bytes.is_empty() {
            continue;
        }
        let source = utf8(path, bytes)?;
        let records = source.strip_suffix('\n').ok_or_else(|| {
            FormatError::InvalidRoProjectRepresentation {
                message: format!("nonempty entity shard '{path}' must end with one LF"),
            }
        })?;
        let mut previous_id: Option<String> = None;
        for (record_index, record) in records.split('\n').enumerate() {
            if record.is_empty() {
                return invalid_representation(format!(
                    "entity shard '{path}' contains a blank JSONL record"
                ));
            }
            let record_path = format!("{path}:{}", record_index + 1);
            strict_input_bytes += record.len();
            inspect_roproj(record, ROPROJ_V1_MAX_JSON_NESTING)
                .map_err(|error| map_frontend_error(&record_path, error))?;
            let dto: EntityV1 = deserialize_roproj(&record_path, record)?;
            if collect_work {
                let (nodes, references, dependencies) = dto_work_counts(&dto);
                formula_ast_nodes += nodes;
                reference_edges += references;
                formula_dependency_edges += dependencies;
            }
            require_id("entity id", &dto.id)?;
            ensure_increasing("entity", previous_id.as_deref(), &dto.id)?;
            previous_id = Some(dto.id.clone());
            if shard_index(&dto.id) != shard - 2 {
                return invalid_representation(format!(
                    "entity '{}' is in wrong shard '{path}'",
                    dto.id
                ));
            }
            let mut canonical_record = String::with_capacity(record.len());
            write_entity(&mut canonical_record, &dto)?;
            if canonical_record != record {
                return invalid_representation(format!(
                    "entity record '{record_path}' is not canonical .roproj/v1"
                ));
            }
            let id_text = dto.id.clone();
            let id = EntityId::from(id_text.clone());
            if entities.insert(id, dto.into_semantic()?).is_some() {
                return invalid_representation(format!("duplicate entity id '{id_text}'"));
            }
            entity_records += 1;
        }
    }

    let document = Document {
        id: DocumentId::from(manifest.document.id),
        title: manifest.document.title,
        schemas,
        entities,
    };
    crate::check_document(&document)?;
    validate_semantic_expression_limits(&document)?;
    Ok((
        document,
        AdmissionWork {
            source_bytes,
            nesting_scan_bytes: strict_input_bytes,
            json_parser_deserializer_bytes: strict_input_bytes * 3,
            canonical_render_bytes: source_bytes,
            entity_records,
            formula_ast_nodes,
            reference_edges,
            formula_dependency_edges,
        },
    ))
}

fn require_exact_paths(files: &[(String, Vec<u8>)]) -> Result<(), FormatError> {
    if files.len() != ROPROJ_V1_PATHS.len() {
        return invalid_representation(format!(
            "canonical tree requires {} files, found {}",
            ROPROJ_V1_PATHS.len(),
            files.len()
        ));
    }
    for (index, ((path, _), expected)) in files.iter().zip(ROPROJ_V1_PATHS).enumerate() {
        if path != expected {
            return invalid_representation(format!(
                "canonical path {index} must be '{expected}', found '{path}'"
            ));
        }
    }
    Ok(())
}

pub(crate) fn dto_work_counts(entity: &EntityV1) -> (usize, usize, usize) {
    let mut formula_ast_nodes = 0;
    let mut reference_edges = 0;
    let mut formula_dependency_edges = 0;
    for value in entity.fields.values() {
        match value {
            ValueV1::Reference(_) => reference_edges += 1,
            ValueV1::Formula(expression) => {
                let (dependencies, nodes) = expression_dependencies(expression);
                formula_ast_nodes += nodes;
                formula_dependency_edges += dependencies;
            }
            ValueV1::Number(_) | ValueV1::Text(_) | ValueV1::Boolean(_) => {}
        }
    }
    (formula_ast_nodes, reference_edges, formula_dependency_edges)
}

fn expression_dependencies(expression: &ExpressionV1) -> (usize, usize) {
    let mut dependencies = BTreeSet::new();
    let mut stack = vec![expression];
    let mut nodes = 0;
    while let Some(node) = stack.pop() {
        nodes += 1;
        match node {
            ExpressionV1::Reference(reference) => {
                dependencies.insert((&reference.entity, &reference.field));
            }
            ExpressionV1::Add(arguments)
            | ExpressionV1::Subtract(arguments)
            | ExpressionV1::Multiply(arguments)
            | ExpressionV1::Divide(arguments)
            | ExpressionV1::Minimum(arguments)
            | ExpressionV1::Maximum(arguments) => {
                stack.push(&arguments.right);
                stack.push(&arguments.left);
            }
            ExpressionV1::Number(_) => {}
        }
    }
    (dependencies.len(), nodes)
}
