use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use tachiko_semantic_core::{
    Document, Entity, EntityId, FieldDefinition, FieldId, FieldKey, FieldType, Number, Schema,
    SchemaId, SchemaKey, Value,
};
use tachiko_storage::{
    FormatError, canonicalize_roproj, decode_roproj_v1, encode_roproj_v1, load_roproj,
    materialize_roproj, publish_roproj, read_canonical_roproj,
};

static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let sequence = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tachiko-storage-roproj-host-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    #[cfg(unix)]
    fn new_short() -> Self {
        let sequence = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let path = Path::new("/tmp").join(format!("tw-rp-{}-{sequence}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn publication_never_overwrites_or_leaves_partial_destination() {
    let temp = TempDir::new();
    let destination = temp.path().join("project.roproj");
    let tree = encode_roproj_v1(&Document::empty("doc-empty", "Empty")).unwrap();

    publish_roproj(&destination, &tree).unwrap();
    let before = read_tree_bytes(&destination);
    let error = publish_roproj(&destination, &tree).unwrap_err();

    assert!(matches!(
        error,
        FormatError::AlreadyExists { path } if path == destination
    ));
    assert_eq!(read_tree_bytes(&destination), before);
    assert_eq!(before.len(), 18);
}

#[test]
fn publication_refuses_preexisting_files_directories_and_symlinks() {
    let temp = TempDir::new();
    let tree = encode_roproj_v1(&Document::empty("doc-empty", "Empty")).unwrap();

    let file = temp.path().join("existing-file.roproj");
    fs::write(&file, b"preserve").unwrap();
    let directory = temp.path().join("existing-directory.roproj");
    fs::create_dir(&directory).unwrap();
    for destination in [&file, &directory] {
        assert!(matches!(
            publish_roproj(destination, &tree),
            Err(FormatError::AlreadyExists { .. })
        ));
    }
    assert_eq!(fs::read(file).unwrap(), b"preserve");
    assert_eq!(fs::read_dir(directory).unwrap().count(), 0);

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let linked = temp.path().join("existing-symlink.roproj");
        symlink(temp.path().join("missing-target"), &linked).unwrap();
        assert!(matches!(
            publish_roproj(&linked, &tree),
            Err(FormatError::AlreadyExists { .. })
        ));
        assert!(
            fs::symlink_metadata(linked)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
}

#[test]
fn exact_canonical_read_and_ordinary_load_return_the_original_document() {
    let temp = TempDir::new();
    let destination = temp.path().join("project.roproj");
    let document = Document::empty("doc-empty", "Empty");
    let expected = encode_roproj_v1(&document).unwrap();
    publish_roproj(&destination, &expected).unwrap();

    assert_eq!(read_canonical_roproj(&destination).unwrap(), expected);
    assert_eq!(load_roproj(&destination).unwrap(), document);
}

#[test]
fn invalid_semantic_materialization_fails_before_touching_the_destination() {
    let temp = TempDir::new();
    let destination = temp.path().join("invalid.roproj");
    let invalid = Document::empty("", "Invalid");

    let error = materialize_roproj(&destination, &invalid).unwrap_err();

    assert!(matches!(error, FormatError::InvalidDocument { .. }));
    assert!(!destination.exists());
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
}

#[test]
fn publication_host_error_leaves_no_destination_or_staging_debris() {
    let temp = TempDir::new();
    let parent_file = temp.path().join("not-a-directory");
    fs::write(&parent_file, b"preserve").unwrap();
    let destination = parent_file.join("project.roproj");
    let tree = encode_roproj_v1(&Document::empty("doc-empty", "Empty")).unwrap();

    assert!(matches!(
        publish_roproj(&destination, &tree),
        Err(FormatError::Write { .. })
    ));

    assert_eq!(fs::read(&parent_file).unwrap(), b"preserve");
    assert!(!destination.exists());
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
}

#[test]
fn host_workflow_operates_without_git_discovery() {
    let temp = TempDir::new();
    let destination = temp.path().join("standalone.roproj");
    let document = Document::empty("standalone", "No Git");

    materialize_roproj(&destination, &document).unwrap();

    assert_eq!(load_roproj(&destination).unwrap(), document);
}

#[cfg(unix)]
#[test]
fn canonical_reader_rejects_a_symlink_root() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new();
    let ordinary = temp.path().join("ordinary.roproj");
    let linked = temp.path().join("linked.roproj");
    materialize_roproj(&ordinary, &Document::empty("doc-empty", "Empty")).unwrap();
    symlink(&ordinary, &linked).unwrap();

    assert!(matches!(
        read_canonical_roproj(&linked),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
}

#[cfg(unix)]
#[test]
fn canonical_reader_rejects_a_symlink_entities_directory() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new();
    let project = temp.path().join("project.roproj");
    let backing = temp.path().join("entities-backing");
    materialize_roproj(&project, &Document::empty("doc-empty", "Empty")).unwrap();
    fs::rename(project.join("entities"), &backing).unwrap();
    symlink(&backing, project.join("entities")).unwrap();

    assert!(matches!(
        read_canonical_roproj(&project),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
}

#[cfg(unix)]
#[test]
fn canonical_reader_rejects_a_symlink_file() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new();
    let project = temp.path().join("project.roproj");
    let backing = temp.path().join("schemas-backing.json");
    materialize_roproj(&project, &Document::empty("doc-empty", "Empty")).unwrap();
    fs::rename(project.join("schemas.json"), &backing).unwrap();
    symlink(&backing, project.join("schemas.json")).unwrap();

    assert!(matches!(
        read_canonical_roproj(&project),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
}

#[test]
fn canonical_reader_rejects_extra_paths() {
    let temp = TempDir::new();
    let project = temp.path().join("project.roproj");
    materialize_roproj(&project, &Document::empty("doc-empty", "Empty")).unwrap();
    fs::write(project.join("extra.json"), b"{}\n").unwrap();

    assert!(matches!(
        read_canonical_roproj(&project),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
}

#[test]
fn canonical_reader_rejects_missing_paths() {
    let temp = TempDir::new();
    let project = temp.path().join("project.roproj");
    materialize_roproj(&project, &Document::empty("doc-empty", "Empty")).unwrap();
    fs::remove_file(project.join("entities/f.jsonl")).unwrap();

    assert!(matches!(
        read_canonical_roproj(&project),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
}

#[test]
fn canonical_reader_rejects_wrong_node_types() {
    let temp = TempDir::new();
    let file_project = temp.path().join("file-project.roproj");
    materialize_roproj(&file_project, &Document::empty("doc-file", "Wrong File")).unwrap();
    fs::remove_file(file_project.join("manifest.json")).unwrap();
    fs::create_dir(file_project.join("manifest.json")).unwrap();

    let directory_project = temp.path().join("directory-project.roproj");
    materialize_roproj(
        &directory_project,
        &Document::empty("doc-directory", "Wrong Directory"),
    )
    .unwrap();
    fs::remove_dir_all(directory_project.join("entities")).unwrap();
    fs::write(directory_project.join("entities"), b"not a directory").unwrap();

    for project in [&file_project, &directory_project] {
        assert!(matches!(
            read_canonical_roproj(project),
            Err(FormatError::InvalidRoProjectRepresentation { .. })
        ));
    }
}

#[test]
fn canonical_reader_rejects_nested_entity_directories() {
    let temp = TempDir::new();
    let project = temp.path().join("project.roproj");
    materialize_roproj(&project, &Document::empty("doc-empty", "Empty")).unwrap();
    fs::create_dir(project.join("entities/nested")).unwrap();
    fs::write(project.join("entities/nested/entities.jsonl"), b"").unwrap();

    assert!(matches!(
        read_canonical_roproj(&project),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
}

#[test]
fn canonical_reader_and_ordinary_load_reject_noncanonical_bytes() {
    let temp = TempDir::new();
    let project = temp.path().join("project.roproj");
    materialize_roproj(&project, &Document::empty("doc-empty", "Empty")).unwrap();
    fs::write(
        project.join("manifest.json"),
        br#"{"format_version":1,"format":"tachiko.roproj","document":{"title":"Empty","id":"doc-empty"}}"#,
    )
    .unwrap();

    assert!(matches!(
        read_canonical_roproj(&project),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
    assert!(matches!(
        load_roproj(&project),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
}

#[test]
fn canonical_reader_manifest_error_precedes_malformed_bodies() {
    let temp = TempDir::new();
    let project = temp.path().join("project.roproj");
    materialize_roproj(&project, &Document::empty("doc-empty", "Empty")).unwrap();
    fs::write(
        project.join("manifest.json"),
        br#"{"format":"tachiko.roproj","format_version":2,"document":{"id":"doc-empty","title":"Empty"}}"#,
    )
    .unwrap();
    fs::write(project.join("schemas.json"), b"not JSON").unwrap();
    fs::write(project.join("entities/0.jsonl"), b"also not JSON\n").unwrap();

    assert!(matches!(
        read_canonical_roproj(&project),
        Err(FormatError::UnsupportedRoProjectVersion {
            found: 2,
            supported: 1
        })
    ));
}

#[test]
fn bounded_canonicalizer_reorders_and_rehomes_without_mutating_source() {
    let temp = TempDir::new();
    let source = temp.path().join("noncanonical.roproj");
    write_noncanonical_project(&source);
    let before = read_tree_bytes(&source);
    let expected = encode_roproj_v1(&bounded_document()).unwrap();

    assert!(load_roproj(&source).is_err());
    let canonical = canonicalize_roproj(&source).unwrap();

    assert_eq!(canonical, expected);
    assert_eq!(read_tree_bytes(&source), before);
    assert!(
        canonical
            .file("entities/6.jsonl")
            .unwrap()
            .starts_with(b"{\"id\":\"entity-a\"")
    );
    assert!(
        canonical
            .file("entities/b.jsonl")
            .unwrap()
            .starts_with(b"{\"id\":\"entity-b\"")
    );
}

#[test]
fn canonicalizer_manifest_error_precedes_malformed_bodies() {
    let temp = TempDir::new();
    let source = temp.path().join("noncanonical.roproj");
    write_noncanonical_project(&source);
    fs::write(
        source.join("manifest.json"),
        br#"{"format":"tachiko.roproj","format_version":2,"document":{"id":"doc","title":"Unsupported"}}"#,
    )
    .unwrap();
    fs::write(source.join("schemas.json"), b"not JSON").unwrap();
    fs::write(
        source.join("entities/nested/wrong-shard.jsonl"),
        b"not JSON\n",
    )
    .unwrap();

    assert!(matches!(
        canonicalize_roproj(&source),
        Err(FormatError::UnsupportedRoProjectVersion {
            found: 2,
            supported: 1
        })
    ));
}

#[cfg(unix)]
#[test]
fn canonicalizer_rejects_nested_symlinks() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new();
    let source = temp.path().join("noncanonical.roproj");
    write_noncanonical_project(&source);
    symlink(
        source.join("entities/extra-empty.jsonl"),
        source.join("entities/nested/linked.jsonl"),
    )
    .unwrap();

    assert!(matches!(
        canonicalize_roproj(&source),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
}

#[cfg(unix)]
#[test]
fn canonicalizer_rejects_root_entities_and_top_level_file_symlinks() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new();

    let ordinary = temp.path().join("ordinary.roproj");
    write_noncanonical_project(&ordinary);
    let linked_root = temp.path().join("linked-root.roproj");
    symlink(&ordinary, &linked_root).unwrap();

    let linked_entities = temp.path().join("linked-entities.roproj");
    write_noncanonical_project(&linked_entities);
    let entities_backing = temp.path().join("entities-backing");
    fs::rename(linked_entities.join("entities"), &entities_backing).unwrap();
    symlink(&entities_backing, linked_entities.join("entities")).unwrap();

    let linked_file = temp.path().join("linked-file.roproj");
    write_noncanonical_project(&linked_file);
    let schemas_backing = temp.path().join("schemas-backing.json");
    fs::rename(linked_file.join("schemas.json"), &schemas_backing).unwrap();
    symlink(&schemas_backing, linked_file.join("schemas.json")).unwrap();

    for source in [&linked_root, &linked_entities, &linked_file] {
        assert!(matches!(
            canonicalize_roproj(source),
            Err(FormatError::InvalidRoProjectRepresentation { .. })
        ));
    }
}

#[test]
fn canonicalizer_rejects_unknown_top_level_children() {
    let temp = TempDir::new();
    let source = temp.path().join("noncanonical.roproj");
    write_noncanonical_project(&source);
    fs::create_dir(source.join("assets")).unwrap();

    assert!(matches!(
        canonicalize_roproj(&source),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
}

#[test]
fn canonicalizer_rejects_wrong_required_top_level_node_types() {
    let temp = TempDir::new();
    let source = temp.path().join("wrong-file.roproj");
    write_noncanonical_project(&source);
    fs::remove_file(source.join("manifest.json")).unwrap();
    fs::create_dir(source.join("manifest.json")).unwrap();

    let entities_source = temp.path().join("wrong-directory.roproj");
    write_noncanonical_project(&entities_source);
    fs::remove_dir_all(entities_source.join("entities")).unwrap();
    fs::write(entities_source.join("entities"), b"not a directory").unwrap();

    for project in [&source, &entities_source] {
        assert!(matches!(
            canonicalize_roproj(project),
            Err(FormatError::InvalidRoProjectRepresentation { .. })
        ));
    }
}

#[test]
fn filesystem_rejection_precedence_is_lexically_deterministic() {
    let temp = TempDir::new();
    let source = temp.path().join("noncanonical.roproj");
    write_noncanonical_project(&source);
    fs::write(source.join("z-unknown"), b"").unwrap();
    fs::write(source.join("a-unknown"), b"").unwrap();

    let error = canonicalize_roproj(&source).unwrap_err();
    let FormatError::InvalidRoProjectRepresentation { message } = error else {
        panic!("unexpected error: {error:?}");
    };
    assert!(message.contains("a-unknown"), "{message}");
}

#[test]
fn canonicalizer_rejects_non_jsonl_entity_files() {
    let temp = TempDir::new();
    let source = temp.path().join("noncanonical.roproj");
    write_noncanonical_project(&source);
    fs::write(source.join("entities/notes.txt"), b"ignored?").unwrap();

    assert!(matches!(
        canonicalize_roproj(&source),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
}

#[test]
fn canonicalizer_rejects_dead_nested_directories() {
    let temp = TempDir::new();
    let source = temp.path().join("noncanonical.roproj");
    write_noncanonical_project(&source);
    fs::create_dir(source.join("entities/dead")).unwrap();

    assert!(matches!(
        canonicalize_roproj(&source),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
}

#[cfg(unix)]
#[test]
fn canonicalizer_rejects_nonregular_entity_nodes() {
    use std::os::unix::net::UnixListener;

    let temp = TempDir::new_short();
    let source = temp.path().join("noncanonical.roproj");
    write_noncanonical_project(&source);
    let _socket = UnixListener::bind(source.join("entities/socket.jsonl")).unwrap();

    assert!(matches!(
        canonicalize_roproj(&source),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
}

#[test]
fn canonicalizer_rejects_invalid_physical_jsonl_records() {
    let temp = TempDir::new();
    for case in ["unterminated", "blank", "multiple", "embedded-lf"] {
        let source = temp.path().join(format!("{case}.roproj"));
        write_noncanonical_project(&source);
        let path = source.join("entities/nested/wrong-shard.jsonl");
        let mut bytes = fs::read(&path).unwrap();
        match case {
            "unterminated" => {
                bytes.pop();
            }
            "blank" => bytes.push(b'\n'),
            "multiple" => {
                bytes.pop();
                bytes.extend_from_slice(b" {}\n");
            }
            "embedded-lf" => bytes.insert(10, b'\n'),
            _ => unreachable!(),
        }
        fs::write(path, bytes).unwrap();

        assert!(
            matches!(
                canonicalize_roproj(&source),
                Err(FormatError::InvalidRoProjectRepresentation { .. }
                    | FormatError::InvalidRoProjectJson { .. })
            ),
            "case {case} must fail"
        );
    }
}

#[test]
fn canonicalizer_rejects_recursive_duplicate_unknown_members_and_tags() {
    let temp = TempDir::new();
    for case in ["duplicate", "unknown-member", "unknown-tag"] {
        let source = temp.path().join(format!("{case}.roproj"));
        write_noncanonical_project(&source);
        let path = source.join("entities/nested/wrong-shard.jsonl");
        let entity = fs::read_to_string(&path).unwrap();
        let mutated = match case {
            "duplicate" => entity.replacen(
                "\"id\": \"entity-b\"",
                "\"id\": \"entity-b\", \"\\u0069d\": \"entity-b\"",
                1,
            ),
            "unknown-member" => entity.replacen(
                "\"id\": \"entity-b\"",
                "\"id\": \"entity-b\", \"extra\": true",
                1,
            ),
            "unknown-tag" => entity.replacen("\"kind\": \"text\"", "\"kind\": \"mystery\"", 1),
            _ => unreachable!(),
        };
        assert_ne!(mutated, entity, "fixture mutation for {case}");
        fs::write(path, mutated).unwrap();

        assert!(
            canonicalize_roproj(&source).is_err(),
            "case {case} must fail"
        );
    }
}

#[test]
fn canonicalizer_rejects_duplicates_in_each_declared_id_scope() {
    let temp = TempDir::new();

    let duplicate_schema = temp.path().join("duplicate-schema.roproj");
    write_noncanonical_project(&duplicate_schema);
    let schemas_path = duplicate_schema.join("schemas.json");
    let schemas = fs::read_to_string(&schemas_path).unwrap();
    let prefix = schemas.trim_end().strip_suffix(']').unwrap();
    fs::write(
        schemas_path,
        format!("{prefix},{{\"id\":\"schema-a\",\"key\":\"again\",\"fields\":[]}}]\n"),
    )
    .unwrap();

    let duplicate_field = temp.path().join("duplicate-field.roproj");
    write_noncanonical_project(&duplicate_field);
    let schemas_path = duplicate_field.join("schemas.json");
    let schemas = fs::read_to_string(&schemas_path).unwrap();
    let mutated = schemas.replacen("\"id\":\"field-z\"", "\"id\":\"field-a\"", 1);
    assert_ne!(mutated, schemas);
    fs::write(schemas_path, mutated).unwrap();

    let duplicate_entity = temp.path().join("duplicate-entity.roproj");
    write_noncanonical_project(&duplicate_entity);
    fs::copy(
        duplicate_entity.join("entities/nested/wrong-shard.jsonl"),
        duplicate_entity.join("entities/duplicate.jsonl"),
    )
    .unwrap();

    for source in [&duplicate_schema, &duplicate_field, &duplicate_entity] {
        assert!(matches!(
            canonicalize_roproj(source),
            Err(FormatError::InvalidRoProjectRepresentation { .. })
        ));
    }
}

#[test]
fn canonicalizer_proves_duplicate_ids_before_semantic_conversion() {
    let temp = TempDir::new();
    let source = temp.path().join("duplicate-before-conversion.roproj");
    write_noncanonical_project(&source);
    fs::remove_dir_all(source.join("entities")).unwrap();
    fs::create_dir(source.join("entities")).unwrap();
    fs::write(
        source.join("entities/a.jsonl"),
        b"{\"id\":\"duplicate\",\"key\":\"first\",\"schema\":\"schema-a\",\"fields\":{\"field-a\":{\"kind\":\"number\",\"value\":1}}}\n",
    )
    .unwrap();
    fs::write(
        source.join("entities/z.jsonl"),
        b"{\"id\":\"duplicate\",\"key\":\"second\",\"schema\":\"\",\"fields\":{\"field-a\":{\"kind\":\"number\",\"value\":2}}}\n",
    )
    .unwrap();

    let error = canonicalize_roproj(&source).unwrap_err();
    let FormatError::InvalidRoProjectRepresentation { message } = error else {
        panic!("unexpected error: {error:?}");
    };
    assert!(
        message.contains("duplicate entity id 'duplicate'"),
        "{message}"
    );
}

#[test]
fn canonicalizer_proves_aggregate_uniqueness_before_any_semantic_conversion() {
    let temp = TempDir::new();
    let source = temp.path().join("aggregate-uniqueness-first.roproj");
    write_noncanonical_project(&source);
    fs::write(
        source.join("schemas.json"),
        b"[{\"id\":\"schema-a\",\"key\":\"record\",\"fields\":[{\"id\":\"field-ref\",\"key\":\"target\",\"field_type\":{\"type\":\"reference\",\"schema\":\"\"},\"required\":false}]}]\n",
    )
    .unwrap();
    fs::remove_dir_all(source.join("entities")).unwrap();
    fs::create_dir(source.join("entities")).unwrap();
    fs::write(
        source.join("entities/a.jsonl"),
        b"{\"id\":\"duplicate\",\"key\":\"first\",\"schema\":\"missing-schema\",\"fields\":{}}\n",
    )
    .unwrap();
    fs::write(
        source.join("entities/z.jsonl"),
        b"{\"id\":\"duplicate\",\"key\":\"second\",\"schema\":\"missing-schema\",\"fields\":{}}\n",
    )
    .unwrap();

    let error = canonicalize_roproj(&source).unwrap_err();
    let FormatError::InvalidRoProjectRepresentation { message } = error else {
        panic!("unexpected error: {error:?}");
    };
    assert!(
        message.contains("duplicate entity id 'duplicate'"),
        "{message}"
    );
}

#[test]
fn canonicalizer_allows_equal_spellings_in_different_id_types() {
    let temp = TempDir::new();
    let source = temp.path().join("cross-type-equality.roproj");
    fs::create_dir(&source).unwrap();
    fs::write(
        source.join("manifest.json"),
        b"{\"format\":\"tachiko.roproj\",\"format_version\":1,\"document\":{\"id\":\"same\",\"title\":\"Same\"}}\n",
    )
    .unwrap();
    fs::write(
        source.join("schemas.json"),
        b"[{\"id\":\"same\",\"key\":\"schema\",\"fields\":[{\"id\":\"same\",\"key\":\"field\",\"field_type\":{\"type\":\"number\"},\"required\":true}]}]\n",
    )
    .unwrap();
    fs::create_dir(source.join("entities")).unwrap();
    fs::write(
        source.join("entities/anything.jsonl"),
        b"{\"id\":\"same\",\"key\":\"entity\",\"schema\":\"same\",\"fields\":{\"same\":{\"kind\":\"number\",\"value\":1}}}\n",
    )
    .unwrap();

    let document = decode_roproj_v1(&canonicalize_roproj(&source).unwrap()).unwrap();

    assert_eq!(document.id.as_str(), "same");
    assert_eq!(document.schemas["same"].id.as_str(), "same");
    assert_eq!(document.schemas["same"].fields["same"].id.as_str(), "same");
    assert_eq!(document.entities["same"].id.as_str(), "same");
}

#[test]
fn canonicalizer_numeric_bridge_preserves_rounding_and_subnormal_bits() {
    let temp = TempDir::new();
    let source = temp.path().join("numeric-bridge.roproj");
    fs::create_dir(&source).unwrap();
    fs::write(
        source.join("manifest.json"),
        b"{\"format\":\"tachiko.roproj\",\"format_version\":1,\"document\":{\"id\":\"doc-numeric\",\"title\":\"Numeric\"}}\n",
    )
    .unwrap();
    fs::write(
        source.join("schemas.json"),
        b"[{\"id\":\"schema-numeric\",\"key\":\"numeric\",\"fields\":[{\"id\":\"field-round\",\"key\":\"round\",\"field_type\":{\"type\":\"number\"},\"required\":true},{\"id\":\"field-subnormal\",\"key\":\"subnormal\",\"field_type\":{\"type\":\"number\"},\"required\":true}]}]\n",
    )
    .unwrap();
    fs::create_dir(source.join("entities")).unwrap();
    fs::write(
        source.join("entities/noncanonical.jsonl"),
        b"{\"id\":\"entity-numeric\",\"key\":\"numeric\",\"schema\":\"schema-numeric\",\"fields\":{\"field-round\":{\"value\":1424953923781206.25,\"kind\":\"number\"},\"field-subnormal\":{\"value\":4.9406564584124654e-324,\"kind\":\"number\"}}}\n",
    )
    .unwrap();
    let expected = numeric_bridge_document();

    let canonical = canonicalize_roproj(&source).unwrap();
    let document = decode_roproj_v1(&canonical).unwrap();

    assert_eq!(canonical, encode_roproj_v1(&expected).unwrap());
    assert_eq!(document, expected);
    let Value::Number(rounding) = document.entities["entity-numeric"].fields["field-round"] else {
        panic!("rounding field changed value kind")
    };
    let Value::Number(subnormal) = document.entities["entity-numeric"].fields["field-subnormal"]
    else {
        panic!("subnormal field changed value kind")
    };
    assert_eq!(rounding.to_bits(), 0x4314_3ff3_c1cb_0959);
    assert_eq!(subnormal.to_bits(), 1);
}

#[test]
fn canonicalizer_rejects_invalid_semantic_content() {
    let temp = TempDir::new();
    let source = temp.path().join("invalid-semantic.roproj");
    write_noncanonical_project(&source);
    let path = source.join("entities/nested/wrong-shard.jsonl");
    let entity = fs::read_to_string(&path).unwrap();
    let mutated = entity.replacen(
        "\"schema\": \"schema-a\"",
        "\"schema\": \"missing-schema\"",
        1,
    );
    assert_ne!(mutated, entity);
    fs::write(path, mutated).unwrap();

    assert!(matches!(
        canonicalize_roproj(&source),
        Err(FormatError::InvalidDocument { .. })
    ));
}

fn read_tree_bytes(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    collect_tree_bytes(root, root, &mut files);
    files
}

fn collect_tree_bytes(root: &Path, directory: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_tree_bytes(root, &path, files);
        } else {
            files.insert(
                path.strip_prefix(root).unwrap().to_owned(),
                fs::read(path).unwrap(),
            );
        }
    }
}

fn write_noncanonical_project(root: &Path) {
    fs::create_dir(root).unwrap();
    fs::write(
        root.join("manifest.json"),
        b"{ \"document\": { \"title\": \"Bounded\", \"id\": \"doc-bounded\" }, \"format_version\": 1, \"format\": \"tachiko.roproj\" }\n",
    )
    .unwrap();
    fs::write(
        root.join("schemas.json"),
        br#"[
 {"fields":[],"key":"unused","id":"schema-z"},
 {"key":"record","fields":[
   {"required":false,"field_type":{"type":"text"},"key":"label","id":"field-z"},
   {"required":true,"field_type":{"type":"number"},"key":"amount","id":"field-a"}
 ],"id":"schema-a"}
]
"#,
    )
    .unwrap();
    fs::create_dir(root.join("entities")).unwrap();
    fs::create_dir(root.join("entities/nested")).unwrap();
    fs::write(
        root.join("entities/nested/wrong-shard.jsonl"),
        concat!(
            " { \"fields\": { \"field-z\": { \"value\": \"beta\", \"kind\": \"text\" }, \"field-a\": { \"value\": 1.0, \"kind\": \"number\" } }, \"schema\": \"schema-a\", \"key\": \"beta\", \"id\": \"entity-b\" } \n",
            "{\"key\":\"alpha\",\"id\":\"entity-a\",\"fields\":{\"field-a\":{\"kind\":\"number\",\"value\":2e0}},\"schema\":\"schema-a\"}\n"
        ),
    )
    .unwrap();
    fs::write(root.join("entities/extra-empty.jsonl"), b"").unwrap();
}

fn bounded_document() -> Document {
    Document {
        id: "doc-bounded".into(),
        title: "Bounded".to_owned(),
        schemas: BTreeMap::from([
            (
                SchemaId::from("schema-a"),
                Schema {
                    id: "schema-a".into(),
                    key: SchemaKey::from("record"),
                    fields: BTreeMap::from([
                        (
                            FieldId::from("field-a"),
                            FieldDefinition {
                                id: "field-a".into(),
                                key: FieldKey::from("amount"),
                                field_type: FieldType::Number,
                                required: true,
                            },
                        ),
                        (
                            FieldId::from("field-z"),
                            FieldDefinition {
                                id: "field-z".into(),
                                key: FieldKey::from("label"),
                                field_type: FieldType::Text,
                                required: false,
                            },
                        ),
                    ]),
                },
            ),
            (
                SchemaId::from("schema-z"),
                Schema {
                    id: "schema-z".into(),
                    key: SchemaKey::from("unused"),
                    fields: BTreeMap::new(),
                },
            ),
        ]),
        entities: BTreeMap::from([
            (
                EntityId::from("entity-a"),
                Entity {
                    id: "entity-a".into(),
                    key: "alpha".into(),
                    schema: "schema-a".into(),
                    fields: BTreeMap::from([(
                        FieldId::from("field-a"),
                        Value::Number(Number::new(2.0).unwrap()),
                    )]),
                },
            ),
            (
                EntityId::from("entity-b"),
                Entity {
                    id: "entity-b".into(),
                    key: "beta".into(),
                    schema: "schema-a".into(),
                    fields: BTreeMap::from([
                        (
                            FieldId::from("field-a"),
                            Value::Number(Number::new(1.0).unwrap()),
                        ),
                        (FieldId::from("field-z"), Value::Text("beta".to_owned())),
                    ]),
                },
            ),
        ]),
    }
}

fn numeric_bridge_document() -> Document {
    Document {
        id: "doc-numeric".into(),
        title: "Numeric".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("schema-numeric"),
            Schema {
                id: "schema-numeric".into(),
                key: SchemaKey::from("numeric"),
                fields: BTreeMap::from([
                    (
                        FieldId::from("field-round"),
                        FieldDefinition {
                            id: "field-round".into(),
                            key: FieldKey::from("round"),
                            field_type: FieldType::Number,
                            required: true,
                        },
                    ),
                    (
                        FieldId::from("field-subnormal"),
                        FieldDefinition {
                            id: "field-subnormal".into(),
                            key: FieldKey::from("subnormal"),
                            field_type: FieldType::Number,
                            required: true,
                        },
                    ),
                ]),
            },
        )]),
        entities: BTreeMap::from([(
            EntityId::from("entity-numeric"),
            Entity {
                id: "entity-numeric".into(),
                key: "numeric".into(),
                schema: "schema-numeric".into(),
                fields: BTreeMap::from([
                    (
                        FieldId::from("field-round"),
                        Value::Number(Number::new(f64::from_bits(0x4314_3ff3_c1cb_0959)).unwrap()),
                    ),
                    (
                        FieldId::from("field-subnormal"),
                        Value::Number(Number::new(f64::from_bits(1)).unwrap()),
                    ),
                ]),
            },
        )]),
    }
}
