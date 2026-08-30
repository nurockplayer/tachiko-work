//! Research-only executable evidence for Issue #175.
//!
//! This module is compiled only for `tachiko-storage` tests. It deliberately
//! does not expose a production admission API or alter shipping behavior.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    hint::black_box,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use sha2::Digest as _;
use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, EntityKey, Expression, FieldDefinition, FieldId,
    FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

use super::*;

static NEXT_RESEARCH_TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AdmissionWork {
    source_bytes: usize,
    nesting_scan_bytes: usize,
    strict_json_bytes: usize,
    canonical_render_bytes: usize,
    entity_records: usize,
    formula_ast_nodes: usize,
    reference_edges: usize,
    formula_dependency_edges: usize,
}

#[derive(Clone, Copy, Debug)]
struct HostAdmissionTimings {
    source_known: Duration,
    first_source_preview: Option<Duration>,
    semantic_current: Duration,
}

struct ResearchTempDirectory(PathBuf);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Directory {
    source_fingerprint: String,
    document_id: String,
    document_title: String,
    schemas: Vec<DirectorySchema>,
    entities: Vec<DirectoryEntity>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DirectorySchema {
    id: String,
    key: String,
    fields: Vec<DirectoryField>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DirectoryField {
    id: String,
    key: String,
    field_type: FieldTypeFact,
    required: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum FieldTypeFact {
    Number,
    Text,
    Boolean,
    Reference { schema: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DirectoryEntity {
    id: String,
    key: String,
    schema: String,
    locator: RecordLocator,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RecordLocator {
    path: String,
    offset: u64,
    length: u64,
    record_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ValueKindFact {
    Number,
    Text,
    Boolean,
    Reference,
    Formula,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
struct IndexedFieldRef {
    entity: String,
    field: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FieldPresenceFact {
    field: IndexedFieldRef,
    kind: ValueKindFact,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct EntityReferenceFact {
    source: IndexedFieldRef,
    target_entity: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FormulaDependencyFact {
    source: IndexedFieldRef,
    targets: Vec<IndexedFieldRef>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct StructuralIndex {
    directory: Directory,
    field_presence: Vec<FieldPresenceFact>,
    references: Vec<EntityReferenceFact>,
    formula_dependencies: Vec<FormulaDependencyFact>,
    reverse_formula_dependencies: Vec<FormulaDependencyFact>,
}

#[derive(Clone, Debug)]
struct SpineScan {
    directory: Directory,
    structural: Option<StructuralIndex>,
    work: AdmissionWork,
    scan_time: Duration,
    serialized_bytes: usize,
}

const SIDECAR_FORMAT: &str = "tachiko.issue-175.spine-sidecar";
const SIDECAR_VERSION: u32 = 1;
const SIDECAR_ALGORITHM: &str = "roproj-v1-structural-index/sha256-framed-v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum SourceBinding {
    DirtyFilesystem {
        source_sha256: String,
    },
    GitSnapshot {
        commit: String,
        tree: String,
        blobs: Vec<GitBlobBinding>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct GitBlobBinding {
    path: String,
    mode: String,
    object_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SidecarEnvelope {
    format: String,
    version: u32,
    algorithm: String,
    binding: SourceBinding,
    payload_sha256: String,
    payload_json: String,
}

enum SidecarOpen {
    Reused(StructuralIndex),
    FellBackToExactAdmission(Document),
}

#[derive(Debug)]
struct BoundedMaterialization {
    entities: BTreeMap<EntityId, Entity>,
    materialized_payload_bytes: usize,
    full_fingerprint_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BoundedProof<T> {
    Exact(T),
    RequiresFullAdmission(&'static str),
}

impl ResearchTempDirectory {
    fn new() -> Self {
        let sequence = NEXT_RESEARCH_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tachiko-issue-175-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ResearchTempDirectory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

/// Research A1: prove exact v1 bytes incrementally and construct one complete
/// semantic document directly. Unlike A0 it neither retains a canonical tree
/// in its result nor re-encodes or decodes the complete document a second time.
fn admit_one_pass_exact(
    files: &[(String, Vec<u8>)],
    collect_work: bool,
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
    super::super::super::check_document(&document)?;
    validate_semantic_expression_limits(&document)?;

    Ok((
        document,
        AdmissionWork {
            source_bytes,
            // `inspect_roproj` performs one explicit nesting preflight plus
            // syntax and duplicate-member parser passes. DTO deserialization
            // is a third parser traversal. JSONL separator LFs are layout
            // checks, not JSON input.
            nesting_scan_bytes: strict_input_bytes,
            strict_json_bytes: strict_input_bytes * 3,
            canonical_render_bytes: source_bytes,
            entity_records,
            formula_ast_nodes,
            reference_edges,
            formula_dependency_edges,
        },
    ))
}

/// Research A1 host path: start from the same exact filesystem tree as A0,
/// retain only manifest/schema bytes and one entity record at a time, and
/// publish semantic authority only after complete exact admission.
#[allow(clippy::too_many_lines)]
fn admit_one_pass_host(
    root: &Path,
    collect_work: bool,
) -> Result<(Document, AdmissionWork, HostAdmissionTimings), FormatError> {
    admit_one_pass_host_controlled(root, collect_work, None, None)
}

#[allow(clippy::too_many_lines)]
fn admit_one_pass_host_controlled(
    root: &Path,
    collect_work: bool,
    cancel: Option<&AtomicBool>,
    records_completed: Option<&AtomicUsize>,
) -> Result<(Document, AdmissionWork, HostAdmissionTimings), FormatError> {
    let started = Instant::now();
    super::super::host::require_exact_root_entries(root)?;

    let manifest_bytes = super::super::host::read_file(&root.join(ROPROJ_V1_PATHS[0]))?;
    let manifest = decode_manifest(&manifest_bytes)?;
    if render_manifest(&manifest)?.as_bytes() != manifest_bytes {
        return invalid_representation("manifest.json is not canonical .roproj/v1".to_owned());
    }
    let source_known = started.elapsed();

    super::super::host::require_exact_entity_entries(&root.join("entities"))?;
    let schemas_bytes = super::super::host::read_file(&root.join(ROPROJ_V1_PATHS[1]))?;
    let schema_dtos: Vec<SchemaV1> = decode_json_file(ROPROJ_V1_PATHS[1], &schemas_bytes)?;
    if render_schemas(&schema_dtos)?.as_bytes() != schemas_bytes {
        return invalid_representation("schemas.json is not canonical .roproj/v1".to_owned());
    }
    let schemas = schemas_into_semantic(schema_dtos)?;

    let mut source_bytes = manifest_bytes.len() + schemas_bytes.len();
    let mut strict_input_bytes = source_bytes;
    let mut entities = BTreeMap::new();
    let mut entity_records = 0;
    let mut formula_ast_nodes = 0;
    let mut reference_edges = 0;
    let mut formula_dependency_edges = 0;
    let mut first_source_preview = None;

    for (shard, relative) in ROPROJ_V1_PATHS.iter().enumerate().skip(2) {
        let path = root.join(relative);
        let file = File::open(&path).map_err(|source| FormatError::Read {
            path: path.clone(),
            source,
        })?;
        let mut reader = BufReader::new(file);
        let mut line = Vec::new();
        let mut previous_id: Option<String> = None;
        let mut record_index = 0;
        loop {
            line.clear();
            let read = reader
                .read_until(b'\n', &mut line)
                .map_err(|source| FormatError::Read {
                    path: path.clone(),
                    source,
                })?;
            if read == 0 {
                break;
            }
            if cancel.is_some_and(|token| token.load(Ordering::Relaxed)) {
                return invalid_representation(
                    "research A1 admission cancelled before SemanticCurrent".to_owned(),
                );
            }
            source_bytes += read;
            let Some(record_bytes) = line.strip_suffix(b"\n") else {
                return invalid_representation(format!(
                    "nonempty entity shard '{relative}' must end with one LF"
                ));
            };
            if record_bytes.is_empty() {
                return invalid_representation(format!(
                    "entity shard '{relative}' contains a blank JSONL record"
                ));
            }
            record_index += 1;
            strict_input_bytes += record_bytes.len();
            let record_path = format!("{relative}:{record_index}");
            let record = utf8(&record_path, record_bytes)?;
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
                    "entity '{}' is in wrong shard '{relative}'",
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
            first_source_preview.get_or_insert_with(|| started.elapsed());

            let id_text = dto.id.clone();
            let id = EntityId::from(id_text.clone());
            if entities.insert(id, dto.into_semantic()?).is_some() {
                return invalid_representation(format!("duplicate entity id '{id_text}'"));
            }
            entity_records += 1;
            if let Some(records_completed) = records_completed {
                records_completed.store(entity_records, Ordering::Relaxed);
            }
            if entity_records % 64 == 0 {
                std::thread::yield_now();
            }
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
    let semantic_current = started.elapsed();
    Ok((
        document,
        AdmissionWork {
            source_bytes,
            nesting_scan_bytes: strict_input_bytes,
            strict_json_bytes: strict_input_bytes * 3,
            canonical_render_bytes: source_bytes,
            entity_records,
            formula_ast_nodes,
            reference_edges,
            formula_dependency_edges,
        },
        HostAdmissionTimings {
            source_known,
            first_source_preview,
            semantic_current,
        },
    ))
}

fn source_preview(root: &Path) -> Result<(String, String, String), FormatError> {
    for relative in ROPROJ_V1_PATHS.iter().skip(2) {
        let path = root.join(relative);
        let mut reader = BufReader::new(File::open(&path).map_err(|source| FormatError::Read {
            path: path.clone(),
            source,
        })?);
        let mut line = Vec::new();
        if reader
            .read_until(b'\n', &mut line)
            .map_err(|source| FormatError::Read {
                path: path.clone(),
                source,
            })?
            == 0
        {
            continue;
        }
        let record_bytes = line.strip_suffix(b"\n").ok_or_else(|| {
            FormatError::InvalidRoProjectRepresentation {
                message: format!("source preview '{relative}' lacks final LF"),
            }
        })?;
        let record = utf8(relative, record_bytes)?;
        inspect_roproj(record, ROPROJ_V1_MAX_JSON_NESTING)
            .map_err(|error| map_frontend_error(relative, error))?;
        let dto: EntityV1 = deserialize_roproj(relative, record)?;
        let mut canonical = String::new();
        write_entity(&mut canonical, &dto)?;
        if canonical != record || shard_index(&dto.id) != shard_index_from_path(relative)? {
            return invalid_representation(
                "source preview record is not exact canonical source".to_owned(),
            );
        }
        return Ok((dto.id, dto.key, dto.schema));
    }
    invalid_representation("source preview found no entity record".to_owned())
}

fn shard_index_from_path(path: &str) -> Result<usize, FormatError> {
    ROPROJ_V1_PATHS
        .iter()
        .skip(2)
        .position(|candidate| candidate == &path)
        .ok_or_else(|| FormatError::InvalidRoProjectRepresentation {
            message: format!("unknown canonical shard path '{path}'"),
        })
}

fn foreground_preview_samples(root: &Path, count: usize) -> Vec<Duration> {
    (0..count)
        .map(|_| {
            let started = Instant::now();
            black_box(source_preview(black_box(root)).unwrap());
            started.elapsed()
        })
        .collect()
}

fn current_rss_bytes() -> usize {
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .unwrap();
    assert!(output.status.success());
    let kibibytes = String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .parse::<usize>()
        .unwrap();
    kibibytes * 1024
}

fn emit_steady_rss() {
    println!("ISSUE175_STEADY_RSS_BYTES={}", current_rss_bytes());
}

fn parse_child_rss(stdout: &[u8], stderr: &[u8]) -> (usize, usize) {
    let stdout = String::from_utf8_lossy(stdout);
    let steady = stdout
        .lines()
        .find_map(|line| line.strip_prefix("ISSUE175_STEADY_RSS_BYTES="))
        .unwrap()
        .parse::<usize>()
        .unwrap();
    let stderr = String::from_utf8_lossy(stderr);
    let peak = stderr
        .lines()
        .find(|line| line.contains("maximum resident set size"))
        .and_then(|line| line.split_whitespace().next())
        .unwrap()
        .parse::<usize>()
        .unwrap();
    (steady, peak)
}

#[allow(clippy::too_many_lines)]
fn scan_spine_host(root: &Path, retain_structural: bool) -> Result<SpineScan, FormatError> {
    let started = Instant::now();
    super::super::host::require_exact_root_entries(root)?;
    let mut fingerprint = sha2::Sha256::new();

    let manifest_bytes = super::super::host::read_file(&root.join(ROPROJ_V1_PATHS[0]))?;
    hash_frame(&mut fingerprint, ROPROJ_V1_PATHS[0], &manifest_bytes);
    let manifest = decode_manifest(&manifest_bytes)?;
    if render_manifest(&manifest)?.as_bytes() != manifest_bytes {
        return invalid_representation("manifest.json is not canonical .roproj/v1".to_owned());
    }
    super::super::host::require_exact_entity_entries(&root.join("entities"))?;

    let schemas_bytes = super::super::host::read_file(&root.join(ROPROJ_V1_PATHS[1]))?;
    hash_frame(&mut fingerprint, ROPROJ_V1_PATHS[1], &schemas_bytes);
    let schema_dtos: Vec<SchemaV1> = decode_json_file(ROPROJ_V1_PATHS[1], &schemas_bytes)?;
    if render_schemas(&schema_dtos)?.as_bytes() != schemas_bytes {
        return invalid_representation("schemas.json is not canonical .roproj/v1".to_owned());
    }
    let schemas = schemas_into_semantic(schema_dtos)?;
    let directory_schemas = directory_schemas(&schemas);
    let schema_fields = schema_field_lookup(&directory_schemas);

    let mut source_bytes = manifest_bytes.len() + schemas_bytes.len();
    let mut strict_input_bytes = source_bytes;
    let mut directory_entities = Vec::new();
    let mut field_presence = Vec::new();
    let mut references = Vec::new();
    let mut formula_dependencies = BTreeMap::new();
    let mut entity_schemas = BTreeMap::new();
    let mut entity_keys = BTreeSet::new();
    let mut entity_records = 0;
    let mut formula_ast_nodes = 0;
    let mut reference_edges = 0;
    let mut formula_dependency_edges = 0;

    for (shard, relative) in ROPROJ_V1_PATHS.iter().enumerate().skip(2) {
        let path = root.join(relative);
        let file_length = std::fs::metadata(&path)
            .map_err(|source| FormatError::Read {
                path: path.clone(),
                source,
            })?
            .len();
        hash_frame_header(&mut fingerprint, relative, file_length);
        let file = File::open(&path).map_err(|source| FormatError::Read {
            path: path.clone(),
            source,
        })?;
        let mut reader = BufReader::new(file);
        let mut line = Vec::new();
        let mut previous_id: Option<String> = None;
        let mut offset = 0_u64;
        let mut record_index = 0;
        loop {
            line.clear();
            let read = reader
                .read_until(b'\n', &mut line)
                .map_err(|source| FormatError::Read {
                    path: path.clone(),
                    source,
                })?;
            if read == 0 {
                break;
            }
            fingerprint.update(&line);
            source_bytes += read;
            let Some(record_bytes) = line.strip_suffix(b"\n") else {
                return invalid_representation(format!(
                    "nonempty entity shard '{relative}' must end with one LF"
                ));
            };
            if record_bytes.is_empty() {
                return invalid_representation(format!(
                    "entity shard '{relative}' contains a blank JSONL record"
                ));
            }
            record_index += 1;
            strict_input_bytes += record_bytes.len();
            let record_path = format!("{relative}:{record_index}");
            let record = utf8(&record_path, record_bytes)?;
            inspect_roproj(record, ROPROJ_V1_MAX_JSON_NESTING)
                .map_err(|error| map_frontend_error(&record_path, error))?;
            let dto: EntityV1 = deserialize_roproj(&record_path, record)?;
            require_id("entity id", &dto.id)?;
            ensure_increasing("entity", previous_id.as_deref(), &dto.id)?;
            previous_id = Some(dto.id.clone());
            if shard_index(&dto.id) != shard - 2 {
                return invalid_representation(format!(
                    "entity '{}' is in wrong shard '{relative}'",
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

            let (nodes, entity_references, dependencies) = dto_work_counts(&dto);
            formula_ast_nodes += nodes;
            reference_edges += entity_references;
            formula_dependency_edges += dependencies;
            if !entity_keys.insert((dto.schema.clone(), dto.key.clone())) {
                return invalid_representation(format!(
                    "duplicate entity key '{}' in schema '{}'",
                    dto.key, dto.schema
                ));
            }
            if entity_schemas
                .insert(dto.id.clone(), dto.schema.clone())
                .is_some()
            {
                return invalid_representation(format!("duplicate entity id '{}'", dto.id));
            }
            directory_entities.push(DirectoryEntity {
                id: dto.id.clone(),
                key: dto.key.clone(),
                schema: dto.schema.clone(),
                locator: RecordLocator {
                    path: (*relative).to_owned(),
                    offset,
                    length: u64::try_from(record_bytes.len()).unwrap(),
                    record_sha256: sha256_hex(record_bytes),
                },
            });
            if retain_structural {
                retain_structural_facts(
                    &dto,
                    &mut field_presence,
                    &mut references,
                    &mut formula_dependencies,
                );
            }
            // Conversion validates stable IDs, finite Number values, and the
            // complete bound formula depth/node contract before payload drop.
            drop(dto.into_semantic()?);
            entity_records += 1;
            offset += u64::try_from(read).unwrap();
        }
        if offset != file_length {
            return invalid_representation(format!(
                "entity shard '{relative}' changed during structural scan"
            ));
        }
    }

    let source_fingerprint = hex_digest(&fingerprint.finalize());
    let directory = Directory {
        source_fingerprint,
        document_id: manifest.document.id,
        document_title: manifest.document.title,
        schemas: directory_schemas,
        entities: directory_entities,
    };
    let structural = if retain_structural {
        validate_structural_coverage(
            &directory,
            &schema_fields,
            &entity_schemas,
            &field_presence,
            &references,
            &formula_dependencies,
        )?;
        let reverse_formula_dependencies = reverse_edges(&formula_dependencies);
        Some(StructuralIndex {
            directory: directory.clone(),
            field_presence,
            references,
            formula_dependencies: dependency_facts(&formula_dependencies),
            reverse_formula_dependencies: dependency_facts(&reverse_formula_dependencies),
        })
    } else {
        None
    };
    let serialized_bytes = if let Some(structural) = &structural {
        serde_json::to_vec(structural)?.len()
    } else {
        serde_json::to_vec(&directory)?.len()
    };
    Ok(SpineScan {
        directory,
        structural,
        work: AdmissionWork {
            source_bytes,
            nesting_scan_bytes: strict_input_bytes,
            strict_json_bytes: strict_input_bytes * 3,
            canonical_render_bytes: source_bytes,
            entity_records,
            formula_ast_nodes,
            reference_edges,
            formula_dependency_edges,
        },
        scan_time: started.elapsed(),
        serialized_bytes,
    })
}

fn hash_frame(hasher: &mut sha2::Sha256, path: &str, bytes: &[u8]) {
    hash_frame_header(hasher, path, u64::try_from(bytes.len()).unwrap());
    hasher.update(bytes);
}

fn fingerprint_source(root: &Path) -> Result<(String, usize, Duration), FormatError> {
    let started = Instant::now();
    super::super::host::require_exact_root_entries(root)?;
    let manifest = super::super::host::read_file(&root.join(ROPROJ_V1_PATHS[0]))?;
    dispatch_manifest(&manifest)?;
    super::super::host::require_exact_entity_entries(&root.join("entities"))?;
    let mut hasher = sha2::Sha256::new();
    let mut bytes_read = 0;
    for relative in ROPROJ_V1_PATHS {
        let bytes = if relative == ROPROJ_V1_PATHS[0] {
            manifest.clone()
        } else {
            super::super::host::read_file(&root.join(relative))?
        };
        bytes_read += bytes.len();
        hash_frame(&mut hasher, relative, &bytes);
    }
    Ok((
        hex_digest(&hasher.finalize()),
        bytes_read,
        started.elapsed(),
    ))
}

fn encode_sidecar(index: &StructuralIndex, binding: SourceBinding) -> Result<Vec<u8>, FormatError> {
    let payload_json = serde_json::to_string(index)?;
    let envelope = SidecarEnvelope {
        format: SIDECAR_FORMAT.to_owned(),
        version: SIDECAR_VERSION,
        algorithm: SIDECAR_ALGORITHM.to_owned(),
        binding,
        payload_sha256: sha256_hex(payload_json.as_bytes()),
        payload_json,
    };
    Ok(serde_json::to_vec(&envelope)?)
}

fn decode_sidecar(
    bytes: &[u8],
    expected_binding: &SourceBinding,
) -> Result<StructuralIndex, String> {
    let source = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    inspect_roproj(source, ROPROJ_V1_MAX_JSON_NESTING)
        .map_err(|error| format!("sidecar strict JSON failure: {error:?}"))?;
    let envelope: SidecarEnvelope =
        serde_json::from_str(source).map_err(|error| error.to_string())?;
    if envelope.format != SIDECAR_FORMAT
        || envelope.version != SIDECAR_VERSION
        || envelope.algorithm != SIDECAR_ALGORITHM
    {
        return Err("unknown or incompatible sidecar format/algorithm".to_owned());
    }
    if &envelope.binding != expected_binding {
        return Err("sidecar source binding is stale or mismatched".to_owned());
    }
    if sha256_hex(envelope.payload_json.as_bytes()) != envelope.payload_sha256 {
        return Err("sidecar payload integrity mismatch".to_owned());
    }
    let index: StructuralIndex =
        serde_json::from_str(&envelope.payload_json).map_err(|error| error.to_string())?;
    if &(SourceBinding::DirtyFilesystem {
        source_sha256: index.directory.source_fingerprint.clone(),
    }) == expected_binding
        || matches!(expected_binding, SourceBinding::GitSnapshot { .. })
    {
        Ok(index)
    } else {
        Err("sidecar payload/source fingerprint mismatch".to_owned())
    }
}

fn open_sidecar_or_fallback(
    root: &Path,
    bytes: &[u8],
    expected_binding: &SourceBinding,
) -> Result<SidecarOpen, FormatError> {
    match decode_sidecar(bytes, expected_binding) {
        Ok(index) => Ok(SidecarOpen::Reused(index)),
        Err(_) => admit_one_pass_host(root, false)
            .map(|(document, _, _)| SidecarOpen::FellBackToExactAdmission(document)),
    }
}

fn materialize_entities_pinned_dirty(
    root: &Path,
    index: &StructuralIndex,
    entity_ids: &BTreeSet<String>,
) -> Result<BoundedMaterialization, FormatError> {
    let (before, before_bytes, _) = fingerprint_source(root)?;
    if before != index.directory.source_fingerprint {
        return invalid_representation(
            "source revision changed before bounded materialization".to_owned(),
        );
    }
    let locators = index
        .directory
        .entities
        .iter()
        .map(|entity| (entity.id.as_str(), entity))
        .collect::<BTreeMap<_, _>>();
    let mut entities = BTreeMap::new();
    let mut materialized_payload_bytes = 0;
    for id in entity_ids {
        let entry = locators.get(id.as_str()).ok_or_else(|| {
            FormatError::InvalidRoProjectRepresentation {
                message: format!("bounded entity '{id}' is not in the exact Directory"),
            }
        })?;
        let path = root.join(&entry.locator.path);
        let mut file = File::open(&path).map_err(|source| FormatError::Read {
            path: path.clone(),
            source,
        })?;
        file.seek(SeekFrom::Start(entry.locator.offset))
            .map_err(|source| FormatError::Read {
                path: path.clone(),
                source,
            })?;
        let length = usize::try_from(entry.locator.length).map_err(|_| {
            FormatError::InvalidRoProjectRepresentation {
                message: format!("bounded locator length does not fit usize for '{id}'"),
            }
        })?;
        let mut record_bytes = vec![0_u8; length];
        file.read_exact(&mut record_bytes)
            .map_err(|source| FormatError::Read {
                path: path.clone(),
                source,
            })?;
        if sha256_hex(&record_bytes) != entry.locator.record_sha256 {
            return invalid_representation(format!("bounded record digest mismatch for '{id}'"));
        }
        let record_path = format!("{}@{}", entry.locator.path, entry.locator.offset);
        let record = utf8(&record_path, &record_bytes)?;
        inspect_roproj(record, ROPROJ_V1_MAX_JSON_NESTING)
            .map_err(|error| map_frontend_error(&record_path, error))?;
        let dto: EntityV1 = deserialize_roproj(&record_path, record)?;
        let mut canonical_record = String::with_capacity(record.len());
        write_entity(&mut canonical_record, &dto)?;
        if canonical_record != record || dto.id != *id {
            return invalid_representation(format!(
                "bounded locator does not resolve exact canonical entity '{id}'"
            ));
        }
        let entity_id = EntityId::from(id.clone());
        entities.insert(entity_id, dto.into_semantic()?);
        materialized_payload_bytes += record_bytes.len();
    }
    let (after, after_bytes, _) = fingerprint_source(root)?;
    if after != before {
        return invalid_representation(
            "source revision changed during bounded materialization".to_owned(),
        );
    }
    Ok(BoundedMaterialization {
        entities,
        materialized_payload_bytes,
        full_fingerprint_bytes: before_bytes + after_bytes,
    })
}

fn dependency_entity_closure(
    index: &StructuralIndex,
    requested: &IndexedFieldRef,
) -> BTreeSet<String> {
    let dependencies = index
        .formula_dependencies
        .iter()
        .map(|fact| (fact.source.clone(), fact.targets.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut entities = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut pending = vec![requested.clone()];
    while let Some(field) = pending.pop() {
        if !visited.insert(field.clone()) {
            continue;
        }
        entities.insert(field.entity.clone());
        if let Some(targets) = dependencies.get(&field) {
            pending.extend(targets.iter().cloned());
        }
    }
    entities
}

fn reverse_dependent_closure(
    index: &StructuralIndex,
    changed: &IndexedFieldRef,
) -> BTreeSet<IndexedFieldRef> {
    let reverse = index
        .reverse_formula_dependencies
        .iter()
        .map(|fact| (fact.source.clone(), fact.targets.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut affected = BTreeSet::new();
    let mut pending = vec![changed.clone()];
    while let Some(field) = pending.pop() {
        if let Some(targets) = reverse.get(&field) {
            for target in targets {
                if affected.insert(target.clone()) {
                    pending.push(target.clone());
                }
            }
        }
    }
    affected
}

fn bounded_formula_proof() -> BoundedProof<()> {
    BoundedProof::RequiresFullAdmission(
        "Structural Index does not prove the Accepted atomic full formula outcome",
    )
}

fn exact_scalar_search_proof() -> BoundedProof<()> {
    BoundedProof::RequiresFullAdmission(
        "exact scalar/full-text search values are absent from the Structural Index",
    )
}

fn initialize_git_snapshot(repo: &Path, document: &Document) -> PathBuf {
    std::fs::create_dir(repo).unwrap();
    run_git(repo, &["init", "-q"]);
    let project = repo.join("project.roproj");
    super::super::host::materialize_roproj(&project, document).unwrap();
    run_git(repo, &["add", "project.roproj"]);
    run_git(
        repo,
        &[
            "-c",
            "user.name=Tachiko Research",
            "-c",
            "user.email=research@tachiko.invalid",
            "commit",
            "-qm",
            "immutable issue 175 snapshot",
        ],
    );
    project
}

fn git_snapshot_binding(repo: &Path) -> (SourceBinding, Duration) {
    let started = Instant::now();
    let commit = git_output(repo, &["rev-parse", "HEAD"]);
    let tree = git_output(repo, &["rev-parse", "HEAD:project.roproj"]);
    let listing = git_output(repo, &["ls-tree", "-r", "HEAD", "--", "project.roproj"]);
    let blobs = listing
        .lines()
        .map(|line| {
            let (metadata, path) = line.split_once('\t').unwrap();
            let mut metadata = metadata.split_whitespace();
            let mode = metadata.next().unwrap().to_owned();
            assert_eq!(metadata.next(), Some("blob"));
            let object_id = metadata.next().unwrap().to_owned();
            GitBlobBinding {
                path: path.strip_prefix("project.roproj/").unwrap().to_owned(),
                mode,
                object_id,
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(blobs.len(), ROPROJ_V1_PATHS.len());
    (
        SourceBinding::GitSnapshot {
            commit,
            tree,
            blobs,
        },
        started.elapsed(),
    )
}

fn run_git(repo: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(arguments)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_output(repo: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(arguments)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn id_navigation_proof(index: &StructuralIndex, id: &str) -> BoundedProof<bool> {
    BoundedProof::Exact(
        index
            .directory
            .entities
            .iter()
            .any(|entity| entity.id == id),
    )
}

fn hash_frame_header(hasher: &mut sha2::Sha256, path: &str, length: u64) {
    hasher.update(u64::try_from(path.len()).unwrap().to_be_bytes());
    hasher.update(path.as_bytes());
    hasher.update(length.to_be_bytes());
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex_digest(&sha2::Sha256::digest(bytes))
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").unwrap();
    }
    output
}

fn directory_schemas(schemas: &BTreeMap<SchemaId, Schema>) -> Vec<DirectorySchema> {
    schemas
        .values()
        .map(|schema| DirectorySchema {
            id: schema.id.to_string(),
            key: schema.key.to_string(),
            fields: schema
                .fields
                .values()
                .map(|field| DirectoryField {
                    id: field.id.to_string(),
                    key: field.key.to_string(),
                    field_type: field_type_fact(&field.field_type),
                    required: field.required,
                })
                .collect(),
        })
        .collect()
}

fn field_type_fact(field_type: &FieldType) -> FieldTypeFact {
    match field_type {
        FieldType::Number => FieldTypeFact::Number,
        FieldType::Text => FieldTypeFact::Text,
        FieldType::Boolean => FieldTypeFact::Boolean,
        FieldType::Reference { schema } => FieldTypeFact::Reference {
            schema: schema.to_string(),
        },
    }
}

fn schema_field_lookup(
    schemas: &[DirectorySchema],
) -> BTreeMap<(String, String), (FieldTypeFact, bool)> {
    schemas
        .iter()
        .flat_map(|schema| {
            schema.fields.iter().map(|field| {
                (
                    (schema.id.clone(), field.id.clone()),
                    (field.field_type.clone(), field.required),
                )
            })
        })
        .collect()
}

fn retain_structural_facts(
    entity: &EntityV1,
    presence: &mut Vec<FieldPresenceFact>,
    references: &mut Vec<EntityReferenceFact>,
    dependencies: &mut BTreeMap<IndexedFieldRef, BTreeSet<IndexedFieldRef>>,
) {
    for (field, value) in &entity.fields {
        let source = IndexedFieldRef {
            entity: entity.id.clone(),
            field: field.clone(),
        };
        presence.push(FieldPresenceFact {
            field: source.clone(),
            kind: value_kind(value),
        });
        match value {
            ValueV1::Reference(target_entity) => references.push(EntityReferenceFact {
                source,
                target_entity: target_entity.clone(),
            }),
            ValueV1::Formula(expression) => {
                dependencies.insert(source, expression_dependencies(expression));
            }
            ValueV1::Number(_) | ValueV1::Text(_) | ValueV1::Boolean(_) => {}
        }
    }
}

fn value_kind(value: &ValueV1) -> ValueKindFact {
    match value {
        ValueV1::Number(_) => ValueKindFact::Number,
        ValueV1::Text(_) => ValueKindFact::Text,
        ValueV1::Boolean(_) => ValueKindFact::Boolean,
        ValueV1::Reference(_) => ValueKindFact::Reference,
        ValueV1::Formula(_) => ValueKindFact::Formula,
    }
}

fn expression_dependencies(expression: &ExpressionV1) -> BTreeSet<IndexedFieldRef> {
    let mut dependencies = BTreeSet::new();
    let mut stack = vec![expression];
    while let Some(node) = stack.pop() {
        match node {
            ExpressionV1::Reference(reference) => {
                dependencies.insert(IndexedFieldRef {
                    entity: reference.entity.clone(),
                    field: reference.field.clone(),
                });
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
    dependencies
}

fn reverse_edges(
    dependencies: &BTreeMap<IndexedFieldRef, BTreeSet<IndexedFieldRef>>,
) -> BTreeMap<IndexedFieldRef, BTreeSet<IndexedFieldRef>> {
    let mut reverse = BTreeMap::<IndexedFieldRef, BTreeSet<IndexedFieldRef>>::new();
    for (source, targets) in dependencies {
        for target in targets {
            reverse
                .entry(target.clone())
                .or_default()
                .insert(source.clone());
        }
    }
    reverse
}

fn dependency_facts(
    dependencies: &BTreeMap<IndexedFieldRef, BTreeSet<IndexedFieldRef>>,
) -> Vec<FormulaDependencyFact> {
    dependencies
        .iter()
        .map(|(source, targets)| FormulaDependencyFact {
            source: source.clone(),
            targets: targets.iter().cloned().collect(),
        })
        .collect()
}

#[allow(clippy::too_many_lines)]
fn validate_structural_coverage(
    directory: &Directory,
    schema_fields: &BTreeMap<(String, String), (FieldTypeFact, bool)>,
    entity_schemas: &BTreeMap<String, String>,
    presence: &[FieldPresenceFact],
    references: &[EntityReferenceFact],
    dependencies: &BTreeMap<IndexedFieldRef, BTreeSet<IndexedFieldRef>>,
) -> Result<(), FormatError> {
    let present = presence
        .iter()
        .map(|fact| (fact.field.clone(), fact.kind.clone()))
        .collect::<BTreeMap<_, _>>();
    for entity in &directory.entities {
        if !directory
            .schemas
            .iter()
            .any(|schema| schema.id == entity.schema)
        {
            return invalid_representation(format!(
                "entity '{}' references missing schema '{}'",
                entity.id, entity.schema
            ));
        }
        for ((schema, field), (_, required)) in schema_fields {
            if schema == &entity.schema
                && *required
                && !present.contains_key(&IndexedFieldRef {
                    entity: entity.id.clone(),
                    field: field.clone(),
                })
            {
                return invalid_representation(format!(
                    "entity '{}' is missing required field '{field}'",
                    entity.id
                ));
            }
        }
    }
    for fact in presence {
        let schema = entity_schemas.get(&fact.field.entity).ok_or_else(|| {
            FormatError::InvalidRoProjectRepresentation {
                message: format!("missing indexed entity '{}'", fact.field.entity),
            }
        })?;
        let (expected, _) = schema_fields
            .get(&(schema.clone(), fact.field.field.clone()))
            .ok_or_else(|| FormatError::InvalidRoProjectRepresentation {
                message: format!(
                    "entity '{}' contains undeclared field '{}'",
                    fact.field.entity, fact.field.field
                ),
            })?;
        if !kind_matches_type(&fact.kind, expected) {
            return invalid_representation(format!(
                "field '{}.{}' has a structurally incompatible value kind",
                fact.field.entity, fact.field.field
            ));
        }
    }
    for reference in references {
        let source_schema = &entity_schemas[&reference.source.entity];
        let (FieldTypeFact::Reference { schema: expected }, _) =
            &schema_fields[&(source_schema.clone(), reference.source.field.clone())]
        else {
            return invalid_representation(format!(
                "field '{}.{}' stores a reference under a non-reference declaration",
                reference.source.entity, reference.source.field
            ));
        };
        let target_schema = entity_schemas
            .get(&reference.target_entity)
            .ok_or_else(|| FormatError::InvalidRoProjectRepresentation {
                message: format!(
                    "reference target '{}' does not exist",
                    reference.target_entity
                ),
            })?;
        if target_schema != expected {
            return invalid_representation(format!(
                "reference target '{}' has wrong schema",
                reference.target_entity
            ));
        }
    }
    for (source, targets) in dependencies {
        let source_schema = &entity_schemas[&source.entity];
        if schema_fields[&(source_schema.clone(), source.field.clone())].0 != FieldTypeFact::Number
        {
            return invalid_representation(format!(
                "formula field '{}.{}' is not declared Number",
                source.entity, source.field
            ));
        }
        for target in targets {
            let target_schema = entity_schemas.get(&target.entity).ok_or_else(|| {
                FormatError::InvalidRoProjectRepresentation {
                    message: format!("formula target entity '{}' does not exist", target.entity),
                }
            })?;
            let Some((target_type, _)) =
                schema_fields.get(&(target_schema.clone(), target.field.clone()))
            else {
                return invalid_representation(format!(
                    "formula target '{}.{}' does not exist",
                    target.entity, target.field
                ));
            };
            if target_type != &FieldTypeFact::Number {
                return invalid_representation(format!(
                    "formula target '{}.{}' is not numeric",
                    target.entity, target.field
                ));
            }
        }
    }
    Ok(())
}

fn kind_matches_type(kind: &ValueKindFact, field_type: &FieldTypeFact) -> bool {
    matches!(
        (kind, field_type),
        (
            ValueKindFact::Number | ValueKindFact::Formula,
            FieldTypeFact::Number
        ) | (ValueKindFact::Text, FieldTypeFact::Text)
            | (ValueKindFact::Boolean, FieldTypeFact::Boolean)
            | (ValueKindFact::Reference, FieldTypeFact::Reference { .. })
    )
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

fn dto_work_counts(entity: &EntityV1) -> (usize, usize, usize) {
    let mut formula_ast_nodes = 0;
    let mut reference_edges = 0;
    let mut formula_dependency_edges = 0;
    for value in entity.fields.values() {
        match value {
            ValueV1::Reference(_) => reference_edges += 1,
            ValueV1::Formula(expression) => {
                let mut stack = vec![expression];
                while let Some(node) = stack.pop() {
                    formula_ast_nodes += 1;
                    match node {
                        ExpressionV1::Reference(_) => formula_dependency_edges += 1,
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
            }
            ValueV1::Number(_) | ValueV1::Text(_) | ValueV1::Boolean(_) => {}
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
    current_a0_owned(files.to_vec())
}

fn current_a0_owned(files: Vec<(String, Vec<u8>)>) -> Result<Document, FormatError> {
    let tree = CanonicalRoProjectV1::try_from_files(files)?;
    decode(&tree)
}

fn mixed_document(entity_count: usize, text_char_count: usize) -> Document {
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
            let repeated = "界\\\"\n".repeat(text_char_count);
            let text = repeated.chars().take(text_char_count).collect();
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

fn dependency_chain_document(entity_count: usize, cycle: bool) -> Document {
    let schema_id = SchemaId::from("issue-175-chain-schema");
    let value_field = FieldId::from("value");
    let entities = (0..entity_count)
        .map(|index| {
            let id = EntityId::from(format!("issue-175-chain-{index:08}"));
            let value = if index == 0 && !cycle {
                Value::Number(Number::new(1.0).unwrap())
            } else {
                let target = if index == 0 {
                    entity_count - 1
                } else {
                    index - 1
                };
                Value::Formula(Expression::Reference(FieldRef::new(
                    format!("issue-175-chain-{target:08}"),
                    value_field.clone(),
                )))
            };
            (
                id.clone(),
                Entity {
                    id,
                    key: EntityKey::from(format!("chain_{index:08}")),
                    schema: schema_id.clone(),
                    fields: BTreeMap::from([(value_field.clone(), value)]),
                },
            )
        })
        .collect();
    Document {
        id: DocumentId::from("issue-175-chain-document"),
        title: if cycle {
            "Issue 175 cross-cold cycle"
        } else {
            "Issue 175 deep dependency chain"
        }
        .to_owned(),
        schemas: BTreeMap::from([(
            schema_id.clone(),
            Schema {
                id: schema_id,
                key: SchemaKey::from("chain"),
                fields: BTreeMap::from([(
                    value_field.clone(),
                    field(&value_field, "value", FieldType::Number),
                )]),
            },
        )]),
        entities,
    }
}

#[derive(Clone, Copy, Debug)]
enum WorkloadShape {
    PayloadUnicodeLong,
    ReferenceHeavy,
    FormulaEdgeHeavy,
    FormulaAstHeavy,
    DeepDependencyChain,
    WideFanOut,
    WideFanIn,
    CrossShardCycle,
    WideFieldPresence,
    MixedRealistic,
}

impl WorkloadShape {
    const ALL: [Self; 10] = [
        Self::PayloadUnicodeLong,
        Self::ReferenceHeavy,
        Self::FormulaEdgeHeavy,
        Self::FormulaAstHeavy,
        Self::DeepDependencyChain,
        Self::WideFanOut,
        Self::WideFanIn,
        Self::CrossShardCycle,
        Self::WideFieldPresence,
        Self::MixedRealistic,
    ];

    const fn name(self) -> &'static str {
        match self {
            Self::PayloadUnicodeLong => "payload_unicode_long_record",
            Self::ReferenceHeavy => "reference_heavy",
            Self::FormulaEdgeHeavy => "formula_edge_heavy",
            Self::FormulaAstHeavy => "formula_ast_heavy",
            Self::DeepDependencyChain => "deep_dependency_chain",
            Self::WideFanOut => "wide_fan_out",
            Self::WideFanIn => "wide_fan_in",
            Self::CrossShardCycle => "cross_shard_cycle",
            Self::WideFieldPresence => "wide_field_presence",
            Self::MixedRealistic => "mixed_realistic_game_data",
        }
    }

    fn document(self, entity_count: usize) -> Document {
        match self {
            Self::PayloadUnicodeLong => payload_document(entity_count, 2_048),
            Self::ReferenceHeavy => reference_document(entity_count, 16),
            Self::FormulaEdgeHeavy => formula_profile_document(entity_count, 16, 0),
            Self::FormulaAstHeavy => formula_profile_document(entity_count, 0, 128),
            Self::DeepDependencyChain => dependency_chain_document(entity_count, false),
            Self::WideFanOut => fan_out_document(entity_count),
            Self::WideFanIn => formula_profile_document(entity_count, 64, 0),
            Self::CrossShardCycle => dependency_chain_document(entity_count, true),
            Self::WideFieldPresence => wide_field_document(entity_count, 64),
            Self::MixedRealistic => mixed_document(entity_count, 128),
        }
    }
}

fn payload_document(entity_count: usize, text_chars: usize) -> Document {
    let schema_id = SchemaId::from("issue-175-payload-schema");
    let payload = FieldId::from("payload");
    let text = "界\\\"\n"
        .repeat(text_chars)
        .chars()
        .take(text_chars)
        .collect::<String>();
    let entities = (0..entity_count)
        .map(|index| {
            let id = EntityId::from(format!("issue-175-payload-{index:08}"));
            (
                id.clone(),
                Entity {
                    id,
                    key: EntityKey::from(format!("payload_{index:08}")),
                    schema: schema_id.clone(),
                    fields: BTreeMap::from([(payload.clone(), Value::Text(text.clone()))]),
                },
            )
        })
        .collect();
    single_schema_document(
        "issue-175-payload-document",
        "Issue 175 payload / Unicode / long JSONL",
        schema_id,
        "payloads",
        BTreeMap::from([(payload.clone(), field(&payload, "payload", FieldType::Text))]),
        entities,
    )
}

fn reference_document(entity_count: usize, reference_fields: usize) -> Document {
    let schema_id = SchemaId::from("issue-175-reference-schema");
    let fields = (0..reference_fields)
        .map(|index| {
            let id = FieldId::from(format!("reference-{index:03}"));
            (
                id.clone(),
                field(
                    &id,
                    &format!("reference_{index:03}"),
                    FieldType::Reference {
                        schema: schema_id.clone(),
                    },
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let entities = (0..entity_count)
        .map(|index| {
            let id = EntityId::from(format!("issue-175-reference-{index:08}"));
            let values = fields
                .keys()
                .enumerate()
                .map(|(offset, field)| {
                    let target = (index + offset * 97) % entity_count;
                    (
                        field.clone(),
                        Value::Reference(EntityId::from(format!(
                            "issue-175-reference-{target:08}"
                        ))),
                    )
                })
                .collect();
            (
                id.clone(),
                Entity {
                    id,
                    key: EntityKey::from(format!("reference_{index:08}")),
                    schema: schema_id.clone(),
                    fields: values,
                },
            )
        })
        .collect();
    single_schema_document(
        "issue-175-reference-document",
        "Issue 175 reference heavy",
        schema_id,
        "references",
        fields,
        entities,
    )
}

fn formula_profile_document(
    entity_count: usize,
    edges_per_formula: usize,
    literal_leaves: usize,
) -> Document {
    let schema_id = SchemaId::from("issue-175-formula-schema");
    let base = FieldId::from("base");
    let computed = FieldId::from("computed");
    let entities = (0..entity_count)
        .map(|index| {
            let id = EntityId::from(format!("issue-175-formula-{index:08}"));
            let leaves = if edges_per_formula > 0 {
                (0..edges_per_formula)
                    .map(|offset| {
                        let target = (index + offset * 97) % entity_count;
                        Expression::Reference(FieldRef::new(
                            format!("issue-175-formula-{target:08}"),
                            base.clone(),
                        ))
                    })
                    .collect()
            } else {
                (0..literal_leaves)
                    .map(|_| Expression::Number(Number::new(1.0).unwrap()))
                    .collect()
            };
            (
                id.clone(),
                Entity {
                    id,
                    key: EntityKey::from(format!("formula_{index:08}")),
                    schema: schema_id.clone(),
                    fields: BTreeMap::from([
                        (base.clone(), Value::Number(Number::new(1.0).unwrap())),
                        (computed.clone(), Value::Formula(balanced_sum(leaves))),
                    ]),
                },
            )
        })
        .collect();
    single_schema_document(
        "issue-175-formula-document",
        "Issue 175 independent formula dimensions",
        schema_id,
        "formulas",
        BTreeMap::from([
            (base.clone(), field(&base, "base", FieldType::Number)),
            (
                computed.clone(),
                field(&computed, "computed", FieldType::Number),
            ),
        ]),
        entities,
    )
}

fn balanced_sum(mut nodes: Vec<Expression>) -> Expression {
    assert!(!nodes.is_empty());
    while nodes.len() > 1 {
        let mut next = Vec::with_capacity(nodes.len().div_ceil(2));
        let mut iterator = nodes.into_iter();
        while let Some(left) = iterator.next() {
            next.push(match iterator.next() {
                Some(right) => Expression::Add {
                    left: Box::new(left),
                    right: Box::new(right),
                },
                None => left,
            });
        }
        nodes = next;
    }
    nodes.pop().unwrap()
}

fn fan_out_document(entity_count: usize) -> Document {
    let mut document = formula_profile_document(entity_count, 1, 0);
    for entity in document.entities.values_mut() {
        entity.fields.insert(
            FieldId::from("computed"),
            Value::Formula(Expression::Reference(FieldRef::new(
                "issue-175-formula-00000000",
                "base",
            ))),
        );
    }
    document.title = "Issue 175 wide fan-out".to_owned();
    document
}

fn wide_field_document(entity_count: usize, field_count: usize) -> Document {
    let schema_id = SchemaId::from("issue-175-wide-schema");
    let fields = (0..field_count)
        .map(|index| {
            let id = FieldId::from(format!("wide-{index:03}"));
            let field_type = match index % 3 {
                0 => FieldType::Number,
                1 => FieldType::Text,
                _ => FieldType::Boolean,
            };
            (
                id.clone(),
                field(&id, &format!("wide_{index:03}"), field_type),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let entities = (0..entity_count)
        .map(|index| {
            let id = EntityId::from(format!("issue-175-wide-{index:08}"));
            let values = fields
                .iter()
                .enumerate()
                .map(|(offset, (field, definition))| {
                    let numeric_offset = u32::try_from(offset).unwrap();
                    let value = match definition.field_type {
                        FieldType::Number => {
                            Value::Number(Number::new(f64::from(numeric_offset)).unwrap())
                        }
                        FieldType::Text => Value::Text(format!("wide value {index}/{offset}")),
                        FieldType::Boolean => Value::Boolean((index + offset) % 2 == 0),
                        FieldType::Reference { .. } => unreachable!(),
                    };
                    (field.clone(), value)
                })
                .collect();
            (
                id.clone(),
                Entity {
                    id,
                    key: EntityKey::from(format!("wide_{index:08}")),
                    schema: schema_id.clone(),
                    fields: values,
                },
            )
        })
        .collect();
    single_schema_document(
        "issue-175-wide-document",
        "Issue 175 wide field presence",
        schema_id,
        "wide",
        fields,
        entities,
    )
}

fn single_schema_document(
    document_id: &str,
    title: &str,
    schema_id: SchemaId,
    schema_key: &str,
    fields: BTreeMap<FieldId, FieldDefinition>,
    entities: BTreeMap<EntityId, Entity>,
) -> Document {
    Document {
        id: DocumentId::from(document_id),
        title: title.to_owned(),
        schemas: BTreeMap::from([(
            schema_id.clone(),
            Schema {
                id: schema_id,
                key: SchemaKey::from(schema_key),
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

fn ids_in_shard(prefix: &str, shard: usize, count: usize) -> Vec<EntityId> {
    let mut ids = Vec::with_capacity(count);
    for candidate in 0_u64.. {
        let id = format!("{prefix}-{candidate:016x}");
        if shard_index(&id) == shard {
            ids.push(EntityId::from(id));
            if ids.len() == count {
                break;
            }
        }
    }
    ids.sort_unstable_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
    ids
}

fn late_invalid_pressure_document(entity_count: usize) -> Document {
    assert!(entity_count >= 3);
    let ids = ids_in_shard("issue-175-late", 15, entity_count);
    let cold_schema_id = SchemaId::from("issue-175-late-cold-schema");
    let hot_schema_id = SchemaId::from("issue-175-late-hot-schema");
    let base = FieldId::from("base");
    let link = FieldId::from("link");
    let computed = FieldId::from("computed");
    let cold_entity_id = ids[0].clone();
    let hot_target_id = ids[1].clone();
    let schemas = BTreeMap::from([
        (
            cold_schema_id.clone(),
            Schema {
                id: cold_schema_id.clone(),
                key: SchemaKey::from("late_cold"),
                fields: BTreeMap::from([(base.clone(), field(&base, "base", FieldType::Number))]),
            },
        ),
        (
            hot_schema_id.clone(),
            Schema {
                id: hot_schema_id.clone(),
                key: SchemaKey::from("late_hot"),
                fields: BTreeMap::from([
                    (base.clone(), field(&base, "base", FieldType::Number)),
                    (
                        link.clone(),
                        field(
                            &link,
                            "link",
                            FieldType::Reference {
                                schema: hot_schema_id.clone(),
                            },
                        ),
                    ),
                    (
                        computed.clone(),
                        field(&computed, "computed", FieldType::Number),
                    ),
                ]),
            },
        ),
    ]);
    let entities = ids
        .into_iter()
        .enumerate()
        .map(|(index, id)| {
            if id == cold_entity_id {
                return (
                    id.clone(),
                    Entity {
                        id,
                        key: EntityKey::from(format!("late_{index:08}")),
                        schema: cold_schema_id.clone(),
                        fields: BTreeMap::from([(
                            base.clone(),
                            Value::Number(Number::new(1.0).unwrap()),
                        )]),
                    },
                );
            }
            let fields = BTreeMap::from([
                (
                    base.clone(),
                    Value::Number(Number::new(f64::from(u32::try_from(index).unwrap())).unwrap()),
                ),
                (link.clone(), Value::Reference(hot_target_id.clone())),
                (
                    computed.clone(),
                    Value::Formula(Expression::Reference(FieldRef {
                        entity: hot_target_id.clone(),
                        field: base.clone(),
                    })),
                ),
            ]);
            (
                id.clone(),
                Entity {
                    id,
                    key: EntityKey::from(format!("late_{index:08}")),
                    schema: hot_schema_id.clone(),
                    fields,
                },
            )
        })
        .collect();
    Document {
        id: DocumentId::from("issue-175-late-document"),
        title: "Issue 175 physically late invalid pressure".to_owned(),
        schemas,
        entities,
    }
}

fn percentile(samples: &mut [Duration], percentile: usize) -> Duration {
    assert!(!samples.is_empty());
    assert!((1..=100).contains(&percentile));
    samples.sort_unstable();
    let index = (samples.len() * percentile).div_ceil(100) - 1;
    samples[index]
}

#[allow(clippy::assertions_on_constants)]
fn require_release_profile() {
    assert!(
        !cfg!(debug_assertions),
        "Issue #175 measurements require `cargo test --release`"
    );
}

#[test]
fn issue_175_a1_matches_a0_semantics_without_whole_tree_reencoding() {
    let document = mixed_document(257, 37);
    let tree = encode(&document).unwrap();
    let files = owned_files(&tree);

    let a0 = current_a0(&files).unwrap();
    let (a1, work) = admit_one_pass_exact(&files, true).unwrap();

    assert_eq!(a1, a0);
    assert_eq!(a1, document);
    assert_eq!(work.entity_records, 257);
    assert_eq!(work.formula_ast_nodes, 257 * 3);
    assert_eq!(work.reference_edges, 257);
    assert_eq!(work.formula_dependency_edges, 257);
}

#[test]
fn issue_175_a1_preserves_a0_rejection_classes() {
    let tree = encode(&mixed_document(257, 37)).unwrap();
    let canonical = owned_files(&tree);

    let mut invalid_path = canonical.clone();
    invalid_path[2].0 = "entities/unknown.jsonl".to_owned();
    assert_rejection_parity("invalid path", &invalid_path);

    let mut wrong_shard = canonical.clone();
    let source = wrong_shard
        .iter()
        .enumerate()
        .skip(2)
        .find(|(_, (_, bytes))| !bytes.is_empty())
        .map(|(index, _)| index)
        .unwrap();
    let target = if source == 2 { 3 } else { 2 };
    wrong_shard[target].1 = std::mem::take(&mut wrong_shard[source].1);
    assert_rejection_parity("wrong shard", &wrong_shard);

    let mut noncanonical = canonical.clone();
    mutate_first_entity_record(&mut noncanonical, |record| format!("{record} "));
    assert_rejection_parity("noncanonical JSONL", &noncanonical);

    let mut duplicate = canonical.clone();
    let shard = duplicate
        .iter_mut()
        .skip(2)
        .find(|(_, bytes)| !bytes.is_empty())
        .unwrap();
    let source = std::str::from_utf8(&shard.1).unwrap();
    let last = source
        .trim_end_matches('\n')
        .split('\n')
        .next_back()
        .unwrap()
        .to_owned();
    shard.1.extend_from_slice(last.as_bytes());
    shard.1.push(b'\n');
    assert_rejection_parity("duplicate entity id", &duplicate);

    let mut semantic_invalid = canonical.clone();
    mutate_first_entity_dto(&mut semantic_invalid, |entity| {
        entity.fields.remove("base");
    });
    assert_rejection_parity("semantic invalidity", &semantic_invalid);

    let mut formula_limit = canonical;
    mutate_first_entity_dto(&mut formula_limit, |entity| {
        entity
            .fields
            .insert("computed".to_owned(), ValueV1::Formula(balanced_formula(8)));
    });
    assert_rejection_parity("formula node limit", &formula_limit);
}

#[test]
fn issue_175_late_invalid_pressure_is_physically_last_in_final_shard() {
    let document = late_invalid_pressure_document(17);
    let canonical = owned_files(&encode(&document).unwrap());
    assert!(canonical[2..17].iter().all(|(_, bytes)| bytes.is_empty()));
    assert_eq!(canonical[17].0, "entities/f.jsonl");
    assert_eq!(
        std::str::from_utf8(&canonical[17].1)
            .unwrap()
            .lines()
            .count(),
        17
    );

    let hot_key = document
        .entities
        .values()
        .find(|entity| entity.schema.as_str() == "issue-175-late-hot-schema")
        .unwrap()
        .key
        .as_str()
        .to_owned();
    let cold_id = document
        .entities
        .values()
        .find(|entity| entity.schema.as_str() == "issue-175-late-cold-schema")
        .unwrap()
        .id
        .as_str()
        .to_owned();

    let mut duplicate_id = canonical.clone();
    let last = last_record(&duplicate_id[17].1).as_bytes().to_vec();
    duplicate_id[17].1.extend_from_slice(&last);
    duplicate_id[17].1.push(b'\n');
    assert_rejection_parity("duplicate id in physically final record", &duplicate_id);

    let mut duplicate_key = canonical.clone();
    mutate_last_entity_dto(&mut duplicate_key, 15, |entity| entity.key = hot_key);
    assert_rejection_parity("duplicate key in physically final record", &duplicate_key);

    let mut missing_reference = canonical.clone();
    mutate_last_entity_dto(&mut missing_reference, 15, |entity| {
        entity.fields.insert(
            "link".to_owned(),
            ValueV1::Reference("issue-175-missing-target".to_owned()),
        );
    });
    assert_rejection_parity(
        "missing reference in physically final record",
        &missing_reference,
    );

    let mut wrong_schema_reference = canonical.clone();
    mutate_last_entity_dto(&mut wrong_schema_reference, 15, |entity| {
        entity
            .fields
            .insert("link".to_owned(), ValueV1::Reference(cold_id));
    });
    assert_rejection_parity(
        "wrong-schema reference in physically final record",
        &wrong_schema_reference,
    );

    let mut order_violation = canonical.clone();
    swap_final_records(&mut order_violation[17].1);
    assert_rejection_parity("final-record unsigned UTF-8 ordering", &order_violation);

    let mut wrong_shard = canonical.clone();
    let final_record = take_last_record(&mut wrong_shard[17].1);
    wrong_shard[16].1 = final_record;
    wrong_shard[16].1.push(b'\n');
    assert_rejection_parity("final-record wrong shard", &wrong_shard);

    let mut duplicate_member = canonical.clone();
    mutate_last_entity_record(&mut duplicate_member, 15, |record| {
        record.replacen("{\"id\":", "{\"id\":\"duplicate\",\"id\":", 1)
    });
    assert_rejection_parity("final-record duplicate JSON member", &duplicate_member);

    let mut unknown_member = canonical.clone();
    mutate_last_entity_record(&mut unknown_member, 15, |record| {
        format!("{},\"unknown\":true}}", record.strip_suffix('}').unwrap())
    });
    assert_rejection_parity("final-record unknown member", &unknown_member);

    let mut malformed_formula = canonical;
    mutate_last_entity_record(&mut malformed_formula, 15, |record| {
        record.replacen("\"op\":\"reference\"", "\"op\":\"unknown\"", 1)
    });
    assert_rejection_parity("final-record malformed formula", &malformed_formula);
}

#[test]
fn issue_175_workload_matrix_varies_structure_and_payload_independently() {
    for shape in WorkloadShape::ALL {
        let document = shape.document(128);
        let tree = encode(&document).unwrap();
        let files = owned_files(&tree);
        let a0 = current_a0(&files).unwrap();
        let (a1, _) = admit_one_pass_exact(&files, true).unwrap();
        assert_eq!(a0, document, "A0 drift for {}", shape.name());
        assert_eq!(a1, document, "A1 drift for {}", shape.name());

        let temp = ResearchTempDirectory::new();
        let root = temp.path().join("matrix.roproj");
        super::super::host::materialize_roproj(&root, &document).unwrap();
        let directory = scan_spine_host(&root, false).unwrap();
        let structural = scan_spine_host(&root, true).unwrap();
        assert_eq!(directory.directory.entities.len(), 128);
        assert_eq!(directory.directory, structural.directory);
    }

    let edge = WorkloadShape::FormulaEdgeHeavy.document(128);
    let ast = WorkloadShape::FormulaAstHeavy.document(128);
    let edge_work = admit_one_pass_exact(&owned_files(&encode(&edge).unwrap()), true)
        .unwrap()
        .1;
    let ast_work = admit_one_pass_exact(&owned_files(&encode(&ast).unwrap()), true)
        .unwrap()
        .1;
    assert!(edge_work.formula_dependency_edges > ast_work.formula_dependency_edges);
    assert!(ast_work.formula_ast_nodes > edge_work.formula_ast_nodes);
}

fn assert_rejection_parity(label: &str, files: &[(String, Vec<u8>)]) {
    let a0 = current_a0(files).unwrap_err();
    let a1 = admit_one_pass_exact(files, false).unwrap_err();
    assert_eq!(
        std::mem::discriminant(&a0),
        std::mem::discriminant(&a1),
        "A0/A1 rejection class drift for {label}: A0={a0}; A1={a1}"
    );
    if a0.to_string() != a1.to_string() {
        assert!(
            matches!(
                (&a0, &a1),
                (
                    FormatError::InvalidRoProjectRepresentation { message: a0_message },
                    FormatError::InvalidRoProjectRepresentation { message: a1_message },
                ) if a0_message == "tree bytes are not canonical .roproj/v1"
                    && a1_message.contains("not canonical .roproj/v1")
            ),
            "unexpected A0/A1 rejection detail drift for {label}: A0={a0}; A1={a1}"
        );
    }
}

fn mutate_first_entity_record(
    files: &mut [(String, Vec<u8>)],
    mutation: impl FnOnce(&str) -> String,
) {
    let (_, bytes) = files
        .iter_mut()
        .skip(2)
        .find(|(_, bytes)| !bytes.is_empty())
        .unwrap();
    let newline = bytes.iter().position(|byte| *byte == b'\n').unwrap();
    let record = std::str::from_utf8(&bytes[..newline]).unwrap();
    let mutated = mutation(record);
    let mut replacement = mutated.into_bytes();
    replacement.push(b'\n');
    replacement.extend_from_slice(&bytes[newline + 1..]);
    *bytes = replacement;
}

fn mutate_first_entity_dto(files: &mut [(String, Vec<u8>)], mutation: impl FnOnce(&mut EntityV1)) {
    mutate_first_entity_record(files, |record| {
        let mut entity: EntityV1 = deserialize_roproj("research mutation", record).unwrap();
        mutation(&mut entity);
        let mut rendered = String::new();
        write_entity(&mut rendered, &entity).unwrap();
        rendered
    });
}

fn last_record(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes)
        .unwrap()
        .strip_suffix('\n')
        .unwrap()
        .split('\n')
        .next_back()
        .unwrap()
}

fn take_last_record(bytes: &mut Vec<u8>) -> Vec<u8> {
    let final_start = bytes[..bytes.len() - 1]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index + 1);
    let record = bytes[final_start..bytes.len() - 1].to_vec();
    bytes.truncate(final_start);
    record
}

fn mutate_last_entity_record(
    files: &mut [(String, Vec<u8>)],
    shard: usize,
    mutation: impl FnOnce(&str) -> String,
) {
    let bytes = &mut files[shard + 2].1;
    let record = take_last_record(bytes);
    let mutated = mutation(std::str::from_utf8(&record).unwrap());
    bytes.extend_from_slice(mutated.as_bytes());
    bytes.push(b'\n');
}

fn mutate_last_entity_dto(
    files: &mut [(String, Vec<u8>)],
    shard: usize,
    mutation: impl FnOnce(&mut EntityV1),
) {
    mutate_last_entity_record(files, shard, |record| {
        let mut entity: EntityV1 = deserialize_roproj("research mutation", record).unwrap();
        mutation(&mut entity);
        let mut rendered = String::new();
        write_entity(&mut rendered, &entity).unwrap();
        rendered
    });
}

fn swap_final_records(bytes: &mut Vec<u8>) {
    let source = std::str::from_utf8(bytes).unwrap();
    let mut records = source
        .strip_suffix('\n')
        .unwrap()
        .split('\n')
        .collect::<Vec<_>>();
    let final_index = records.len() - 1;
    records.swap(final_index - 1, final_index);
    *bytes = records.join("\n").into_bytes();
    bytes.push(b'\n');
}

fn balanced_formula(depth: usize) -> ExpressionV1 {
    if depth == 0 {
        ExpressionV1::Number(NumberV1(1.0))
    } else {
        ExpressionV1::Add(BinaryArgumentsV1 {
            left: Box::new(balanced_formula(depth - 1)),
            right: Box::new(balanced_formula(depth - 1)),
        })
    }
}

#[test]
fn issue_175_streaming_host_a1_matches_current_host_a0() {
    let temp = ResearchTempDirectory::new();
    let root = temp.path().join("mixed-smoke.roproj");
    let expected = mixed_document(257, 37);
    super::super::host::materialize_roproj(&root, &expected).unwrap();

    let a0 = super::super::host::load_roproj(&root).unwrap();
    let (a1, work, timings) = admit_one_pass_host(&root, true).unwrap();

    assert_eq!(a0, expected);
    assert_eq!(a1, expected);
    assert_eq!(work.entity_records, 257);
    assert!(timings.source_known <= timings.semantic_current);
    assert!(timings.first_source_preview.unwrap() <= timings.semantic_current);
}

#[test]
fn issue_175_progressive_source_preview_is_non_authoritative_and_cancellable() {
    let temp = ResearchTempDirectory::new();
    let root = temp.path().join("progressive.roproj");
    let expected = mixed_document(257, 37);
    super::super::host::materialize_roproj(&root, &expected).unwrap();
    let preview = source_preview(&root).unwrap();
    assert!(expected.entities.contains_key(preview.0.as_str()));

    let cancel = AtomicBool::new(true);
    let records = AtomicUsize::new(0);
    let error =
        admit_one_pass_host_controlled(&root, false, Some(&cancel), Some(&records)).unwrap_err();
    assert!(matches!(
        error,
        FormatError::InvalidRoProjectRepresentation { message }
            if message.contains("cancelled before SemanticCurrent")
    ));
    assert_eq!(records.load(Ordering::Relaxed), 0);
}

#[test]
fn issue_175_directory_and_structural_index_are_distinct_exact_layers() {
    let temp = ResearchTempDirectory::new();
    let root = temp.path().join("mixed-smoke.roproj");
    let expected = mixed_document(257, 37);
    super::super::host::materialize_roproj(&root, &expected).unwrap();

    let directory = scan_spine_host(&root, false).unwrap();
    let structural = scan_spine_host(&root, true).unwrap();
    let index = structural.structural.as_ref().unwrap();

    assert!(directory.structural.is_none());
    assert_eq!(directory.directory, index.directory);
    assert_eq!(index.directory.entities.len(), 257);
    assert_eq!(index.field_presence.len(), 257 * 5);
    assert_eq!(index.references.len(), 257);
    assert_eq!(index.formula_dependencies.len(), 257);
    assert_eq!(index.reverse_formula_dependencies.len(), 257);
    assert!(structural.serialized_bytes > directory.serialized_bytes);
    assert_eq!(directory.work.source_bytes, structural.work.source_bytes);
}

#[test]
fn issue_175_dirty_sidecar_reuse_is_bound_and_fails_closed() {
    let temp = ResearchTempDirectory::new();
    let root = temp.path().join("source.roproj");
    let expected = mixed_document(257, 37);
    super::super::host::materialize_roproj(&root, &expected).unwrap();
    let scan = scan_spine_host(&root, true).unwrap();
    let index = scan.structural.unwrap();
    let binding = SourceBinding::DirtyFilesystem {
        source_sha256: fingerprint_source(&root).unwrap().0,
    };
    let sidecar = encode_sidecar(&index, binding.clone()).unwrap();

    assert_eq!(decode_sidecar(&sidecar, &binding).unwrap(), index);
    assert!(matches!(
        open_sidecar_or_fallback(&root, &sidecar, &binding).unwrap(),
        SidecarOpen::Reused(reused) if reused == index
    ));

    let mut corrupted: SidecarEnvelope = serde_json::from_slice(&sidecar).unwrap();
    corrupted.payload_json.push(' ');
    let corrupted = serde_json::to_vec(&corrupted).unwrap();
    assert!(matches!(
        open_sidecar_or_fallback(&root, &corrupted, &binding).unwrap(),
        SidecarOpen::FellBackToExactAdmission(document) if document == expected
    ));

    let mut incompatible: SidecarEnvelope = serde_json::from_slice(&sidecar).unwrap();
    incompatible.algorithm = "unknown-algorithm".to_owned();
    let incompatible = serde_json::to_vec(&incompatible).unwrap();
    assert!(matches!(
        open_sidecar_or_fallback(&root, &incompatible, &binding).unwrap(),
        SidecarOpen::FellBackToExactAdmission(document) if document == expected
    ));

    let other_root = temp.path().join("other.roproj");
    let other = mixed_document(258, 37);
    super::super::host::materialize_roproj(&other_root, &other).unwrap();
    let other_binding = SourceBinding::DirtyFilesystem {
        source_sha256: fingerprint_source(&other_root).unwrap().0,
    };
    assert!(matches!(
        open_sidecar_or_fallback(&other_root, &sidecar, &other_binding).unwrap(),
        SidecarOpen::FellBackToExactAdmission(document) if document == other
    ));
}

#[test]
fn issue_175_git_sidecar_binds_exact_immutable_snapshot_objects() {
    let temp = ResearchTempDirectory::new();
    let repo = temp.path().join("repo");
    let expected = mixed_document(257, 37);
    let project = initialize_git_snapshot(&repo, &expected);
    let scan = scan_spine_host(&project, true).unwrap();
    let index = scan.structural.unwrap();
    let (binding, _) = git_snapshot_binding(&repo);
    let SourceBinding::GitSnapshot { blobs, .. } = &binding else {
        unreachable!();
    };
    assert_eq!(
        blobs
            .iter()
            .map(|blob| blob.path.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(ROPROJ_V1_PATHS)
    );
    assert!(blobs.iter().all(|blob| blob.mode == "100644"));
    let sidecar = encode_sidecar(&index, binding.clone()).unwrap();
    assert_eq!(decode_sidecar(&sidecar, &binding).unwrap(), index);

    let dirty_binding = SourceBinding::DirtyFilesystem {
        source_sha256: index.directory.source_fingerprint.clone(),
    };
    assert!(decode_sidecar(&sidecar, &dirty_binding).is_err());
}

#[test]
fn issue_175_bounded_materialization_is_revision_pinned_and_never_guesses_formula_truth() {
    let temp = ResearchTempDirectory::new();
    let root = temp.path().join("chain.roproj");
    let complete = dependency_chain_document(257, false);
    super::super::host::materialize_roproj(&root, &complete).unwrap();
    let index = scan_spine_host(&root, true).unwrap().structural.unwrap();
    let requested = IndexedFieldRef {
        entity: "issue-175-chain-00000256".to_owned(),
        field: "value".to_owned(),
    };

    assert_eq!(
        id_navigation_proof(&index, &requested.entity),
        BoundedProof::Exact(true)
    );
    assert_eq!(
        exact_scalar_search_proof(),
        BoundedProof::RequiresFullAdmission(
            "exact scalar/full-text search values are absent from the Structural Index"
        )
    );
    let closure = dependency_entity_closure(&index, &requested);
    assert_eq!(closure.len(), 257);
    let bounded = materialize_entities_pinned_dirty(&root, &index, &closure).unwrap();
    assert_eq!(bounded.entities.len(), complete.entities.len());
    for (id, entity) in &bounded.entities {
        assert_eq!(entity, &complete.entities[id]);
    }
    let changed = IndexedFieldRef {
        entity: "issue-175-chain-00000000".to_owned(),
        field: "value".to_owned(),
    };
    assert_eq!(reverse_dependent_closure(&index, &changed).len(), 256);
    assert!(matches!(
        bounded_formula_proof(),
        BoundedProof::RequiresFullAdmission(_)
    ));
    assert!(bounded.full_fingerprint_bytes >= index.directory.entities.len());
    assert!(bounded.materialized_payload_bytes > 0);

    let cycle_root = temp.path().join("cycle.roproj");
    let cycle = dependency_chain_document(257, true);
    super::super::host::materialize_roproj(&cycle_root, &cycle).unwrap();
    let cycle_index = scan_spine_host(&cycle_root, true)
        .unwrap()
        .structural
        .unwrap();
    assert_eq!(
        dependency_entity_closure(&cycle_index, &requested).len(),
        257
    );
    assert!(matches!(
        bounded_formula_proof(),
        BoundedProof::RequiresFullAdmission(_)
    ));

    let replacement = temp.path().join("replacement.roproj");
    let mut changed_revision = complete.clone();
    changed_revision.title.push_str(" changed");
    super::super::host::materialize_roproj(&replacement, &changed_revision).unwrap();
    std::fs::copy(
        replacement.join("manifest.json"),
        root.join("manifest.json"),
    )
    .unwrap();
    let error =
        materialize_entities_pinned_dirty(&root, &index, &BTreeSet::from([requested.entity]))
            .unwrap_err();
    assert!(matches!(
        error,
        FormatError::InvalidRoProjectRepresentation { message }
            if message.contains("source revision changed")
    ));
}

#[test]
#[ignore = "run explicitly in release mode to record Issue #175 A0/A1 evidence"]
fn issue_175_a0_a1_release_baseline() {
    require_release_profile();
    let entity_counts = std::env::var("TACHIKO_ISSUE_175_ENTITY_COUNTS")
        .unwrap_or_else(|_| "1000,10000".to_owned());
    let repetitions = std::env::var("TACHIKO_ISSUE_175_REPETITIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(20);
    println!(
        "arm,workload,entities,fields,source_bytes,nesting_scan_bytes,strict_json_bytes,canonical_render_bytes,entity_records,formula_ast_nodes,reference_edges,formula_dependency_edges,repetitions,p50_us,p95_us"
    );
    for entity_count in entity_counts
        .split(',')
        .map(|value| value.parse::<usize>().unwrap())
    {
        let document = mixed_document(entity_count, 64);
        let tree = encode(&document).unwrap();
        let files = owned_files(&tree);
        let (_, work) = admit_one_pass_exact(&files, true).unwrap();
        let mut a0_samples = Vec::with_capacity(repetitions);
        let mut a1_samples = Vec::with_capacity(repetitions);
        for _ in 0..repetitions {
            let a0_input = files.clone();
            let start = Instant::now();
            black_box(current_a0_owned(black_box(a0_input)).unwrap());
            a0_samples.push(start.elapsed());
            let start = Instant::now();
            black_box(admit_one_pass_exact(black_box(&files), false).unwrap());
            a1_samples.push(start.elapsed());
        }
        emit_baseline_row("A0", entity_count, repetitions, work, &mut a0_samples);
        emit_baseline_row("A1", entity_count, repetitions, work, &mut a1_samples);
    }
}

#[test]
#[ignore = "run explicitly in release mode to record Issue #175 host-open evidence"]
fn issue_175_a0_a1_host_open_warm_raw_samples() {
    require_release_profile();
    let entity_counts = std::env::var("TACHIKO_ISSUE_175_ENTITY_COUNTS")
        .unwrap_or_else(|_| "1000,10000".to_owned());
    let repetitions = std::env::var("TACHIKO_ISSUE_175_REPETITIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(30);
    println!(
        "arm,workload,cache_state,entities,fields,source_bytes,physical_read_bytes,nesting_scan_bytes,strict_json_bytes,canonical_render_bytes,entity_records,formula_ast_nodes,reference_edges,formula_dependency_edges,repetition,order,source_known_us,first_source_preview_us,semantic_current_us"
    );
    for entity_count in entity_counts
        .split(',')
        .map(|value| value.parse::<usize>().unwrap())
    {
        let temp = ResearchTempDirectory::new();
        let root = temp.path().join("mixed-smoke.roproj");
        let expected = mixed_document(entity_count, 64);
        super::super::host::materialize_roproj(&root, &expected).unwrap();
        let (_, work, _) = admit_one_pass_host(&root, true).unwrap();

        // Explicit warmup. This test reports OS-cache-warm evidence only; it
        // makes no claim about controlled cold page-cache behavior.
        black_box(super::super::host::load_roproj(&root).unwrap());
        black_box(admit_one_pass_host(&root, false).unwrap());

        for repetition in 0..repetitions {
            if repetition % 2 == 0 {
                run_a0_host_sample(&root, &expected, entity_count, repetition, "A0-first", work);
                run_a1_host_sample(&root, &expected, entity_count, repetition, "A0-first", work);
            } else {
                run_a1_host_sample(&root, &expected, entity_count, repetition, "A1-first", work);
                run_a0_host_sample(&root, &expected, entity_count, repetition, "A1-first", work);
            }
        }
    }
}

#[test]
#[ignore = "run explicitly in release mode to record Issue #175 workload matrix"]
fn issue_175_full_workload_matrix_raw_samples() {
    require_release_profile();
    let entity_counts = std::env::var("TACHIKO_ISSUE_175_MATRIX_ENTITIES")
        .unwrap_or_else(|_| "1000,4000,16000".to_owned());
    let repetitions = std::env::var("TACHIKO_ISSUE_175_REPETITIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(20);
    println!(
        "arm,workload,cache_state,entities,fields,source_sha256,source_bytes,nesting_scan_bytes,strict_json_bytes,canonical_render_bytes,entity_records,formula_ast_nodes,reference_edges,formula_dependency_edges,serialized_spine_bytes,repetition,source_known_us,first_preview_us,semantic_or_scan_current_us"
    );
    for entity_count in entity_counts
        .split(',')
        .map(|value| value.parse::<usize>().unwrap())
    {
        for shape in WorkloadShape::ALL {
            let temp = ResearchTempDirectory::new();
            let root = temp.path().join("matrix.roproj");
            let document = shape.document(entity_count);
            let field_count = document
                .entities
                .values()
                .map(|entity| entity.fields.len())
                .sum::<usize>();
            super::super::host::materialize_roproj(&root, &document).unwrap();
            let initial_scan = scan_spine_host(&root, true).unwrap();
            let work = initial_scan.work;
            let source_fingerprint = initial_scan.directory.source_fingerprint;
            for repetition in 0..repetitions {
                // Deterministically rotate arm order to distribute filesystem
                // and allocator order effects across paired warm samples.
                for slot in 0..4 {
                    run_matrix_arm(
                        (slot + repetition) % 4,
                        &root,
                        shape,
                        entity_count,
                        field_count,
                        repetition,
                        work,
                        &source_fingerprint,
                    );
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_matrix_arm(
    arm: usize,
    root: &Path,
    shape: WorkloadShape,
    entity_count: usize,
    field_count: usize,
    repetition: usize,
    work: AdmissionWork,
    source_fingerprint: &str,
) {
    match arm {
        0 => {
            let started = Instant::now();
            black_box(super::super::host::load_roproj(root).unwrap());
            emit_matrix_sample(
                "A0",
                shape,
                entity_count,
                field_count,
                repetition,
                work,
                source_fingerprint,
                0,
                None,
                None,
                started.elapsed(),
            );
        }
        1 => {
            let (_, _, timings) = admit_one_pass_host(root, false).unwrap();
            emit_matrix_sample(
                "A1",
                shape,
                entity_count,
                field_count,
                repetition,
                work,
                source_fingerprint,
                0,
                Some(timings.source_known),
                timings.first_source_preview,
                timings.semantic_current,
            );
        }
        2 | 3 => {
            let retain_structural = arm == 3;
            let scan = scan_spine_host(root, retain_structural).unwrap();
            emit_matrix_sample(
                if retain_structural {
                    "C-structural"
                } else {
                    "C-directory"
                },
                shape,
                entity_count,
                field_count,
                repetition,
                scan.work,
                source_fingerprint,
                scan.serialized_bytes,
                None,
                None,
                scan.scan_time,
            );
        }
        _ => unreachable!(),
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_matrix_sample(
    arm: &str,
    shape: WorkloadShape,
    entity_count: usize,
    field_count: usize,
    repetition: usize,
    work: AdmissionWork,
    source_fingerprint: &str,
    serialized_spine_bytes: usize,
    source_known: Option<Duration>,
    first_preview: Option<Duration>,
    current: Duration,
) {
    let strict_json_bytes = if arm == "A0" {
        work.strict_json_bytes * 2
    } else {
        work.strict_json_bytes
    };
    let nesting_scan_bytes = if arm == "A0" {
        work.nesting_scan_bytes * 2
    } else {
        work.nesting_scan_bytes
    };
    println!(
        "{arm},{},os_cache_warm,{entity_count},{field_count},{source_fingerprint},{},{nesting_scan_bytes},{strict_json_bytes},{},{},{},{},{},{serialized_spine_bytes},{repetition},{},{},{}",
        shape.name(),
        work.source_bytes,
        work.canonical_render_bytes,
        work.entity_records,
        work.formula_ast_nodes,
        work.reference_edges,
        work.formula_dependency_edges,
        source_known.map_or_else(String::new, |value| value.as_micros().to_string()),
        first_preview.map_or_else(String::new, |value| value.as_micros().to_string()),
        current.as_micros(),
    );
}

#[test]
#[ignore = "run explicitly in release mode to record Issue #175 B evidence"]
fn issue_175_progressive_background_interference_and_cancellation() {
    require_release_profile();
    let entity_count = std::env::var("TACHIKO_ISSUE_175_BACKGROUND_ENTITIES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(16_000);
    let foreground_count = std::env::var("TACHIKO_ISSUE_175_FOREGROUND_REQUESTS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(200);
    let repetitions = std::env::var("TACHIKO_ISSUE_175_REPETITIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(20);
    let temp = ResearchTempDirectory::new();
    let root = temp.path().join("progressive.roproj");
    super::super::host::materialize_roproj(&root, &mixed_document(entity_count, 64)).unwrap();
    black_box(source_preview(&root).unwrap());
    println!(
        "arm,workload,cache_state,entities,repetition,foreground_requests,baseline_p50_us,baseline_p95_us,baseline_p99_us,baseline_min_us,baseline_max_us,background_p50_us,background_p95_us,background_p99_us,background_min_us,background_max_us,foreground_p95_ratio_ppm,baseline_elapsed_us,background_foreground_elapsed_us,background_semantic_current_us,cancelled_after_records,cancellation_latency_us"
    );
    for repetition in 0..repetitions {
        let baseline_started = Instant::now();
        let baseline = foreground_preview_samples(&root, foreground_count);
        let baseline_elapsed = baseline_started.elapsed();

        let worker_root = root.clone();
        let background_started = Instant::now();
        let worker = std::thread::spawn(move || admit_one_pass_host(&worker_root, false));
        let interfered_started = Instant::now();
        let interfered = foreground_preview_samples(&root, foreground_count);
        let interfered_elapsed = interfered_started.elapsed();
        let background = worker.join().unwrap().unwrap();
        let background_time = background_started.elapsed();
        let baseline_distribution = latency_distribution(&baseline);
        let interfered_distribution = latency_distribution(&interfered);
        let regression_ppm = interfered_distribution
            .1
            .as_nanos()
            .saturating_mul(1_000_000)
            .checked_div(baseline_distribution.1.as_nanos())
            .unwrap_or(0);

        let cancel = Arc::new(AtomicBool::new(false));
        let records = Arc::new(AtomicUsize::new(0));
        let worker_root = root.clone();
        let worker_cancel = Arc::clone(&cancel);
        let worker_records = Arc::clone(&records);
        let cancellation_worker = std::thread::spawn(move || {
            admit_one_pass_host_controlled(
                &worker_root,
                false,
                Some(&worker_cancel),
                Some(&worker_records),
            )
        });
        while records.load(Ordering::Relaxed) < 64 {
            std::thread::yield_now();
        }
        let cancellation_started = Instant::now();
        cancel.store(true, Ordering::Relaxed);
        let cancellation = cancellation_worker.join().unwrap();
        let cancellation_time = cancellation_started.elapsed();
        assert!(cancellation.is_err());
        println!(
            "B-progressive,mixed_smoke,os_cache_warm,{entity_count},{repetition},{foreground_count},{},{},{},{},{},{},{},{},{},{},{regression_ppm},{},{},{},{},{}",
            baseline_distribution.0.as_micros(),
            baseline_distribution.1.as_micros(),
            baseline_distribution.2.as_micros(),
            baseline_distribution.3.as_micros(),
            baseline_distribution.4.as_micros(),
            interfered_distribution.0.as_micros(),
            interfered_distribution.1.as_micros(),
            interfered_distribution.2.as_micros(),
            interfered_distribution.3.as_micros(),
            interfered_distribution.4.as_micros(),
            baseline_elapsed.as_micros(),
            interfered_elapsed.as_micros(),
            background
                .2
                .semantic_current
                .as_micros()
                .max(background_time.as_micros()),
            records.load(Ordering::Relaxed),
            cancellation_time.as_micros(),
        );
    }
}

fn latency_distribution(
    samples: &[Duration],
) -> (Duration, Duration, Duration, Duration, Duration) {
    let mut sorted = samples.to_vec();
    let p50 = percentile(&mut sorted, 50);
    let p95 = percentile(&mut sorted, 95);
    let p99 = percentile(&mut sorted, 99);
    (p50, p95, p99, sorted[0], sorted[sorted.len() - 1])
}

#[test]
#[ignore = "run explicitly in release mode to record Issue #175 C evidence"]
fn issue_175_directory_structural_raw_samples() {
    require_release_profile();
    let entity_counts = std::env::var("TACHIKO_ISSUE_175_ENTITY_COUNTS")
        .unwrap_or_else(|_| "1000,10000".to_owned());
    let repetitions = std::env::var("TACHIKO_ISSUE_175_REPETITIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(30);
    println!(
        "arm,workload,cache_state,entities,fields,source_bytes,nesting_scan_bytes,strict_json_bytes,entity_records,formula_ast_nodes,reference_edges,formula_dependency_edges,serialized_spine_bytes,spine_source_ratio_ppm,repetition,scan_us"
    );
    for entity_count in entity_counts
        .split(',')
        .map(|value| value.parse::<usize>().unwrap())
    {
        let temp = ResearchTempDirectory::new();
        let root = temp.path().join("mixed-smoke.roproj");
        let expected = mixed_document(entity_count, 64);
        super::super::host::materialize_roproj(&root, &expected).unwrap();
        black_box(scan_spine_host(&root, true).unwrap());
        for repetition in 0..repetitions {
            for (arm, retain_structural) in [("C-directory", false), ("C-structural", true)] {
                let scan = scan_spine_host(black_box(&root), retain_structural).unwrap();
                println!(
                    "{arm},mixed_smoke,os_cache_warm,{entity_count},{},{},{},{},{},{},{},{},{},{},{repetition},{}",
                    entity_count * 5,
                    scan.work.source_bytes,
                    scan.work.nesting_scan_bytes,
                    scan.work.strict_json_bytes,
                    scan.work.entity_records,
                    scan.work.formula_ast_nodes,
                    scan.work.reference_edges,
                    scan.work.formula_dependency_edges,
                    scan.serialized_bytes,
                    scan.serialized_bytes.saturating_mul(1_000_000) / scan.work.source_bytes,
                    scan.scan_time.as_micros(),
                );
            }
        }
    }
}

#[test]
#[ignore = "run explicitly in release mode to record Issue #175 E1 evidence"]
fn issue_175_dirty_sidecar_raw_samples() {
    require_release_profile();
    let entity_counts = std::env::var("TACHIKO_ISSUE_175_ENTITY_COUNTS")
        .unwrap_or_else(|_| "1000,10000".to_owned());
    let repetitions = std::env::var("TACHIKO_ISSUE_175_REPETITIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(30);
    println!(
        "arm,workload,cache_state,entities,source_bytes,sidecar_bytes,repetition,hash_us,sidecar_decode_us,total_validated_reuse_us,full_a1_us"
    );
    for entity_count in entity_counts
        .split(',')
        .map(|value| value.parse::<usize>().unwrap())
    {
        let temp = ResearchTempDirectory::new();
        let root = temp.path().join("source.roproj");
        let expected = mixed_document(entity_count, 64);
        super::super::host::materialize_roproj(&root, &expected).unwrap();
        let scan = scan_spine_host(&root, true).unwrap();
        let index = scan.structural.unwrap();
        let binding = SourceBinding::DirtyFilesystem {
            source_sha256: scan.directory.source_fingerprint,
        };
        let sidecar = encode_sidecar(&index, binding.clone()).unwrap();
        black_box(fingerprint_source(&root).unwrap());
        black_box(decode_sidecar(&sidecar, &binding).unwrap());
        for repetition in 0..repetitions {
            let (_, _, hash_time) = fingerprint_source(black_box(&root)).unwrap();
            let decode_started = Instant::now();
            black_box(decode_sidecar(black_box(&sidecar), &binding).unwrap());
            let decode_time = decode_started.elapsed();
            let full_started = Instant::now();
            black_box(admit_one_pass_host(black_box(&root), false).unwrap());
            let full_time = full_started.elapsed();
            println!(
                "E1-dirty-sidecar,mixed_smoke,os_cache_warm,{entity_count},{},{},{repetition},{},{},{},{}",
                scan.work.source_bytes,
                sidecar.len(),
                hash_time.as_micros(),
                decode_time.as_micros(),
                (hash_time + decode_time).as_micros(),
                full_time.as_micros(),
            );
        }
    }
}

#[test]
#[ignore = "run explicitly in release mode to record Issue #175 E2 evidence"]
fn issue_175_git_sidecar_raw_samples() {
    require_release_profile();
    let entity_count = std::env::var("TACHIKO_ISSUE_175_GIT_ENTITIES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(10_000);
    let repetitions = std::env::var("TACHIKO_ISSUE_175_REPETITIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(30);
    let temp = ResearchTempDirectory::new();
    let repo = temp.path().join("repo");
    let document = mixed_document(entity_count, 64);
    let project = initialize_git_snapshot(&repo, &document);
    let scan = scan_spine_host(&project, true).unwrap();
    let index = scan.structural.unwrap();
    let (binding, _) = git_snapshot_binding(&repo);
    let sidecar = encode_sidecar(&index, binding.clone()).unwrap();
    println!(
        "arm,workload,cache_state,entities,source_bytes,sidecar_bytes,repetition,git_identity_us,sidecar_decode_us,total_validated_reuse_us,full_a1_us"
    );
    for repetition in 0..repetitions {
        let (_, identity_time) = git_snapshot_binding(black_box(&repo));
        let decode_started = Instant::now();
        black_box(decode_sidecar(black_box(&sidecar), &binding).unwrap());
        let decode_time = decode_started.elapsed();
        let full_started = Instant::now();
        black_box(admit_one_pass_host(black_box(&project), false).unwrap());
        let full_time = full_started.elapsed();
        println!(
            "E2-git-sidecar,mixed_smoke,os_cache_warm,{entity_count},{},{},{repetition},{},{},{},{}",
            scan.work.source_bytes,
            sidecar.len(),
            identity_time.as_micros(),
            decode_time.as_micros(),
            (identity_time + decode_time).as_micros(),
            full_time.as_micros(),
        );
    }
}

#[test]
#[ignore = "run explicitly in release mode to record Issue #175 D evidence"]
fn issue_175_bounded_materialization_raw_samples() {
    require_release_profile();
    let entity_counts =
        std::env::var("TACHIKO_ISSUE_175_ENTITY_COUNTS").unwrap_or_else(|_| "1000,4000".to_owned());
    let repetitions = std::env::var("TACHIKO_ISSUE_175_REPETITIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(30);
    println!(
        "arm,workload,cache_state,entities,closure_entities,source_bytes,materialized_payload_bytes,full_fingerprint_bytes,repetition,materialize_us,proof"
    );
    for entity_count in entity_counts
        .split(',')
        .map(|value| value.parse::<usize>().unwrap())
    {
        let temp = ResearchTempDirectory::new();
        let root = temp.path().join("chain.roproj");
        let document = dependency_chain_document(entity_count, false);
        super::super::host::materialize_roproj(&root, &document).unwrap();
        let scan = scan_spine_host(&root, true).unwrap();
        let index = scan.structural.unwrap();
        let requested = IndexedFieldRef {
            entity: format!("issue-175-chain-{:08}", entity_count - 1),
            field: "value".to_owned(),
        };
        let closure = dependency_entity_closure(&index, &requested);
        black_box(materialize_entities_pinned_dirty(&root, &index, &closure).unwrap());
        for repetition in 0..repetitions {
            let started = Instant::now();
            let bounded =
                materialize_entities_pinned_dirty(black_box(&root), &index, &closure).unwrap();
            let duration = started.elapsed();
            println!(
                "D-bounded,deep_dependency_chain,os_cache_warm,{entity_count},{},{},{},{},{repetition},{},requires_full_admission",
                closure.len(),
                scan.work.source_bytes,
                bounded.materialized_payload_bytes,
                bounded.full_fingerprint_bytes,
                duration.as_micros(),
            );
        }
    }
}

#[test]
#[ignore = "internal fresh-process child for Issue #175 RSS evidence"]
fn issue_175_rss_child() {
    require_release_profile();
    let root = PathBuf::from(std::env::var("TACHIKO_ISSUE_175_RSS_ROOT").unwrap());
    let arm = std::env::var("TACHIKO_ISSUE_175_RSS_ARM").unwrap();
    match arm.as_str() {
        "baseline" => emit_steady_rss(),
        "A0" => {
            let document = super::super::host::load_roproj(&root).unwrap();
            emit_steady_rss();
            black_box(&document);
        }
        "A1" => {
            let document = admit_one_pass_host(&root, false).unwrap();
            emit_steady_rss();
            black_box(&document);
        }
        "directory" => {
            let directory = scan_spine_host(&root, false).unwrap();
            emit_steady_rss();
            black_box(&directory);
        }
        "structural" => {
            let structural = scan_spine_host(&root, true).unwrap();
            emit_steady_rss();
            black_box(&structural);
        }
        "structural_hot_payload" => {
            let structural = scan_spine_host(&root, true).unwrap();
            let index = structural.structural.as_ref().unwrap();
            let first = index.directory.entities.first().unwrap().id.clone();
            let hot =
                materialize_entities_pinned_dirty(&root, index, &BTreeSet::from([first])).unwrap();
            emit_steady_rss();
            black_box((&structural, &hot));
        }
        "structural_plus_document" => {
            let structural = scan_spine_host(&root, true).unwrap();
            let document = admit_one_pass_host(&root, false).unwrap();
            emit_steady_rss();
            black_box((&structural, &document));
        }
        _ => panic!("unknown Issue #175 RSS arm '{arm}'"),
    }
}

#[test]
#[ignore = "run explicitly on macOS in release mode to record Issue #175 F RSS evidence"]
fn issue_175_fresh_process_rss_samples() {
    require_release_profile();
    assert_eq!(std::env::consts::OS, "macos", "RSS units are macOS bytes");
    let entity_count = std::env::var("TACHIKO_ISSUE_175_RSS_ENTITIES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(10_000);
    let repetitions = std::env::var("TACHIKO_ISSUE_175_RSS_REPETITIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(5);
    let temp = ResearchTempDirectory::new();
    let root = temp.path().join("rss.roproj");
    super::super::host::materialize_roproj(&root, &mixed_document(entity_count, 64)).unwrap();
    let executable = std::env::current_exe().unwrap();
    println!("arm,workload,cache_state,entities,repetition,steady_rss_bytes,peak_rss_bytes");
    for repetition in 0..repetitions {
        for arm in [
            "baseline",
            "A0",
            "A1",
            "directory",
            "structural",
            "structural_hot_payload",
            "structural_plus_document",
        ] {
            let output = Command::new("/usr/bin/time")
                .arg("-l")
                .arg(&executable)
                .args([
                    "--exact",
                    "roproj::v1::issue_175_research::issue_175_rss_child",
                    "--ignored",
                    "--nocapture",
                ])
                .env("TACHIKO_ISSUE_175_RSS_ROOT", &root)
                .env("TACHIKO_ISSUE_175_RSS_ARM", arm)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "RSS child failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let (steady, peak) = parse_child_rss(&output.stdout, &output.stderr);
            println!("{arm},mixed_smoke,os_cache_warm,{entity_count},{repetition},{steady},{peak}");
        }
    }
}

fn run_a0_host_sample(
    root: &Path,
    expected: &Document,
    entity_count: usize,
    repetition: usize,
    order: &str,
    work: AdmissionWork,
) {
    let started = Instant::now();
    let document = super::super::host::load_roproj(black_box(root)).unwrap();
    let semantic_current = started.elapsed();
    assert_eq!(&document, expected);
    emit_host_sample(
        "A0",
        entity_count,
        repetition,
        order,
        work,
        None,
        None,
        semantic_current,
    );
}

fn run_a1_host_sample(
    root: &Path,
    expected: &Document,
    entity_count: usize,
    repetition: usize,
    order: &str,
    work: AdmissionWork,
) {
    let (document, _, timings) = admit_one_pass_host(black_box(root), false).unwrap();
    assert_eq!(&document, expected);
    emit_host_sample(
        "A1",
        entity_count,
        repetition,
        order,
        work,
        Some(timings.source_known),
        timings.first_source_preview,
        timings.semantic_current,
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_host_sample(
    arm: &str,
    entity_count: usize,
    repetition: usize,
    order: &str,
    work: AdmissionWork,
    source_known: Option<Duration>,
    first_source_preview: Option<Duration>,
    semantic_current: Duration,
) {
    let strict_json_bytes = if arm == "A0" {
        work.strict_json_bytes * 2
    } else {
        work.strict_json_bytes
    };
    let nesting_scan_bytes = if arm == "A0" {
        work.nesting_scan_bytes * 2
    } else {
        work.nesting_scan_bytes
    };
    println!(
        "{arm},mixed_smoke_host,os_cache_warm,{entity_count},{},{},{},{nesting_scan_bytes},{strict_json_bytes},{},{},{},{},{},{repetition},{order},{},{},{}",
        entity_count * 5,
        work.source_bytes,
        work.source_bytes,
        work.canonical_render_bytes,
        work.entity_records,
        work.formula_ast_nodes,
        work.reference_edges,
        work.formula_dependency_edges,
        source_known.map_or_else(String::new, |value| value.as_micros().to_string()),
        first_source_preview.map_or_else(String::new, |value| value.as_micros().to_string()),
        semantic_current.as_micros(),
    );
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
    let nesting_scan_bytes = if arm == "A0" {
        work.nesting_scan_bytes * 2
    } else {
        work.nesting_scan_bytes
    };
    println!(
        "{arm},mixed_smoke,{entity_count},{},{},{nesting_scan_bytes},{strict_json_bytes},{},{},{},{},{},{repetitions},{p50},{p95}",
        entity_count * 5,
        work.source_bytes,
        work.canonical_render_bytes,
        work.entity_records,
        work.formula_ast_nodes,
        work.reference_edges,
        work.formula_dependency_edges,
    );
}
