//! Research-only executable evidence for Issue #175.
//!
//! This module is compiled only for `tachiko-storage` tests. It deliberately
//! does not expose a production admission API or alter shipping behavior.

use std::{
    collections::BTreeMap,
    hint::black_box,
    time::{Duration, Instant},
};

use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, EntityKey, Expression, FieldDefinition, FieldId,
    FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AdmissionWork {
    source_bytes: usize,
    strict_json_bytes: usize,
    canonical_render_bytes: usize,
    entity_records: usize,
    formula_ast_nodes: usize,
    reference_edges: usize,
    formula_dependency_edges: usize,
}

/// Research A1: prove exact v1 bytes incrementally and construct one complete
/// semantic document directly. Unlike A0 it neither retains a canonical tree
/// in its result nor re-encodes or decodes the complete document a second time.
fn admit_one_pass_exact(
    files: &[(String, Vec<u8>)],
) -> Result<(Document, AdmissionWork), FormatError> {
    require_exact_paths(files)?;
    let source_bytes = files.iter().map(|(_, bytes)| bytes.len()).sum();
    let mut strict_input_bytes = files[0].1.len() + files[1].1.len();

    // ADR-0017/0023: manifest selection precedes all body interpretation.
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
    super::super::super::check_document(&document)?;
    validate_semantic_expression_limits(&document)?;

    let (formula_ast_nodes, reference_edges, formula_dependency_edges) =
        semantic_work_counts(&document);
    Ok((
        document,
        AdmissionWork {
            source_bytes,
            // `inspect_roproj` performs syntax and duplicate-member passes;
            // DTO deserialization is a third complete traversal of each JSON
            // unit. JSONL separator LFs are representation checks, not JSON.
            strict_json_bytes: strict_input_bytes * 3,
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

fn semantic_work_counts(document: &Document) -> (usize, usize, usize) {
    let mut formula_ast_nodes = 0;
    let mut reference_edges = 0;
    let mut formula_dependency_edges = 0;
    for entity in document.entities.values() {
        for value in entity.fields.values() {
            match value {
                Value::Reference(_) => reference_edges += 1,
                Value::Formula(expression) => {
                    let mut stack = vec![expression];
                    while let Some(node) = stack.pop() {
                        formula_ast_nodes += 1;
                        match node {
                            Expression::Reference(_) => formula_dependency_edges += 1,
                            Expression::Add { left, right }
                            | Expression::Subtract { left, right }
                            | Expression::Multiply { left, right }
                            | Expression::Divide { left, right }
                            | Expression::Minimum { left, right }
                            | Expression::Maximum { left, right } => {
                                stack.push(right);
                                stack.push(left);
                            }
                            Expression::Number(_) => {}
                        }
                    }
                }
                Value::Number(_) | Value::Text(_) | Value::Boolean(_) => {}
            }
        }
    }
    (formula_ast_nodes, reference_edges, formula_dependency_edges)
}

fn owned_files(tree: &CanonicalRoProjectV1) -> Vec<(String, Vec<u8>)> {
    tree.files()
        .iter()
        .map(|file| (file.path().to_owned(), file.bytes().to_vec()))
        .collect()
}

fn current_a0(files: &[(String, Vec<u8>)]) -> Result<Document, FormatError> {
    let tree = CanonicalRoProjectV1::try_from_files(files.to_vec())?;
    decode(&tree)
}

fn mixed_document(entity_count: usize, text_bytes: usize) -> Document {
    let schema_id = SchemaId::from("issue-175-schema");
    let base = FieldId::from("base");
    let label = FieldId::from("label");
    let enabled = FieldId::from("enabled");
    let link = FieldId::from("link");
    let computed = FieldId::from("computed");
    let fields = BTreeMap::from([
        (base.clone(), field(&base, "base", FieldType::Number)),
        (label.clone(), field(&label, "label", FieldType::Text)),
        (
            enabled.clone(),
            field(&enabled, "enabled", FieldType::Boolean),
        ),
        (
            link.clone(),
            field(
                &link,
                "link",
                FieldType::Reference {
                    schema: schema_id.clone(),
                },
            ),
        ),
        (
            computed.clone(),
            field(&computed, "computed", FieldType::Number),
        ),
    ]);
    let entities = (0..entity_count)
        .map(|index| {
            let id = EntityId::from(format!("issue-175-entity-{index:08}"));
            let far_index = if index == 0 { 0 } else { index / 2 };
            let far_id = EntityId::from(format!("issue-175-entity-{far_index:08}"));
            let repeated = "界\\\"\n".repeat(text_bytes);
            let text = repeated.chars().take(text_bytes).collect();
            let numeric_index = u32::try_from(index).unwrap();
            let values = BTreeMap::from([
                (
                    base.clone(),
                    Value::Number(Number::new(f64::from(numeric_index) + 1.0).unwrap()),
                ),
                (label.clone(), Value::Text(text)),
                (enabled.clone(), Value::Boolean(index % 2 == 0)),
                (link.clone(), Value::Reference(far_id)),
                (
                    computed.clone(),
                    Value::Formula(Expression::Add {
                        left: Box::new(Expression::Reference(FieldRef::new(
                            id.clone(),
                            base.clone(),
                        ))),
                        right: Box::new(Expression::Number(Number::new(1.0).unwrap())),
                    }),
                ),
            ]);
            (
                id.clone(),
                Entity {
                    id,
                    key: EntityKey::from(format!("entity_{index:08}")),
                    schema: schema_id.clone(),
                    fields: values,
                },
            )
        })
        .collect();
    Document {
        id: DocumentId::from("issue-175-document"),
        title: "Issue 175 mixed fixture".to_owned(),
        schemas: BTreeMap::from([(
            schema_id.clone(),
            Schema {
                id: schema_id,
                key: SchemaKey::from("records"),
                fields,
            },
        )]),
        entities,
    }
}

fn field(id: &FieldId, key: &str, field_type: FieldType) -> FieldDefinition {
    FieldDefinition {
        id: id.clone(),
        key: FieldKey::from(key),
        field_type,
        required: true,
    }
}

fn percentile(samples: &mut [Duration], percentile: usize) -> Duration {
    assert!(!samples.is_empty());
    assert!((1..=100).contains(&percentile));
    samples.sort_unstable();
    let index = (samples.len() * percentile).div_ceil(100) - 1;
    samples[index]
}

#[test]
fn issue_175_a1_matches_a0_semantics_without_whole_tree_reencoding() {
    let document = mixed_document(257, 37);
    let tree = encode(&document).unwrap();
    let files = owned_files(&tree);

    let a0 = current_a0(&files).unwrap();
    let (a1, work) = admit_one_pass_exact(&files).unwrap();

    assert_eq!(a1, a0);
    assert_eq!(a1, document);
    assert_eq!(work.entity_records, 257);
    assert_eq!(work.formula_ast_nodes, 257 * 3);
    assert_eq!(work.reference_edges, 257);
    assert_eq!(work.formula_dependency_edges, 257);
}

#[test]
#[ignore = "run explicitly in release mode to record Issue #175 A0/A1 evidence"]
fn issue_175_a0_a1_release_baseline() {
    let entity_counts = std::env::var("TACHIKO_ISSUE_175_ENTITY_COUNTS")
        .unwrap_or_else(|_| "1000,10000".to_owned());
    let repetitions = std::env::var("TACHIKO_ISSUE_175_REPETITIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(20);
    println!(
        "arm,workload,entities,fields,source_bytes,strict_json_bytes,canonical_render_bytes,entity_records,formula_ast_nodes,reference_edges,formula_dependency_edges,repetitions,p50_us,p95_us"
    );
    for entity_count in entity_counts
        .split(',')
        .map(|value| value.parse::<usize>().unwrap())
    {
        let document = mixed_document(entity_count, 64);
        let tree = encode(&document).unwrap();
        let files = owned_files(&tree);
        let (_, work) = admit_one_pass_exact(&files).unwrap();
        let mut a0_samples = Vec::with_capacity(repetitions);
        let mut a1_samples = Vec::with_capacity(repetitions);
        for _ in 0..repetitions {
            let start = Instant::now();
            black_box(current_a0(black_box(&files)).unwrap());
            a0_samples.push(start.elapsed());
            let start = Instant::now();
            black_box(admit_one_pass_exact(black_box(&files)).unwrap());
            a1_samples.push(start.elapsed());
        }
        emit_baseline_row("A0", entity_count, repetitions, work, &mut a0_samples);
        emit_baseline_row("A1", entity_count, repetitions, work, &mut a1_samples);
    }
}

fn emit_baseline_row(
    arm: &str,
    entity_count: usize,
    repetitions: usize,
    work: AdmissionWork,
    samples: &mut [Duration],
) {
    let p50 = percentile(samples, 50).as_micros();
    let p95 = percentile(samples, 95).as_micros();
    // A0 strictly decodes the tree once during admission, re-encodes it, then
    // decodes it again for ordinary load. Its raw strict-parser traversal is
    // therefore twice A1's; physical host reads are measured separately.
    let strict_json_bytes = if arm == "A0" {
        work.strict_json_bytes * 2
    } else {
        work.strict_json_bytes
    };
    println!(
        "{arm},mixed,{entity_count},{},{},{strict_json_bytes},{},{},{},{},{},{repetitions},{p50},{p95}",
        entity_count * 5,
        work.source_bytes,
        work.canonical_render_bytes,
        work.entity_records,
        work.formula_ast_nodes,
        work.reference_edges,
        work.formula_dependency_edges,
    );
}
