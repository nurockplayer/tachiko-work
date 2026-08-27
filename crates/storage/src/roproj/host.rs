//! Native filesystem admission and publication for `.roproj/v1`.

use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use tachiko_semantic_core::Document;

use super::v1::{
    CanonicalRoProjectV1, ROPROJ_V1_PATHS, canonicalize_unordered, decode, dispatch_manifest,
    encode,
};
use crate::FormatError;

static NEXT_STAGING_DIRECTORY: AtomicU64 = AtomicU64::new(0);

/// Read an exact canonical `.roproj/v1` tree from an ordinary directory.
///
/// Manifest dispatch completes before schema or entity bodies are interpreted,
/// and the pure canonical constructor remains the final byte authority.
///
/// # Errors
///
/// Returns host read errors, layout/representation failures, explicit manifest
/// dispatch errors, or the canonical codec's DTO and semantic failures.
pub fn read_canonical_roproj(path: impl AsRef<Path>) -> Result<CanonicalRoProjectV1, FormatError> {
    let root = path.as_ref();
    require_directory(root, "canonical .roproj root")?;
    require_exact_root_entries(root)?;

    let manifest_path = root.join(ROPROJ_V1_PATHS[0]);
    let manifest = read_file(&manifest_path)?;
    dispatch_manifest(&manifest)?;

    let entities = root.join("entities");
    require_exact_entity_entries(&entities)?;

    let mut files = Vec::with_capacity(ROPROJ_V1_PATHS.len());
    files.push((ROPROJ_V1_PATHS[0].to_owned(), manifest));
    for relative in ROPROJ_V1_PATHS.iter().skip(1) {
        files.push(((*relative).to_owned(), read_file(&root.join(relative))?));
    }
    CanonicalRoProjectV1::try_from_files(files)
}

/// Load a semantic document from an exact canonical `.roproj/v1` directory.
///
/// Ordinary load deliberately does not admit the bounded non-canonical family.
///
/// # Errors
///
/// Returns exact canonical admission or semantic decode errors.
pub fn load_roproj(path: impl AsRef<Path>) -> Result<Document, FormatError> {
    let tree = read_canonical_roproj(path)?;
    decode(&tree)
}

/// Admit the bounded non-canonical `.roproj/v1` family and return a fresh tree.
///
/// This explicit operation never mutates the source. Paths and shard names do
/// not contribute identity; decoded stable IDs are the sole identity authority.
///
/// # Errors
///
/// Returns host/layout failures, explicit manifest dispatch errors, strict DTO
/// failures, duplicate identities, or semantic validation failures.
pub fn canonicalize_roproj(path: impl AsRef<Path>) -> Result<CanonicalRoProjectV1, FormatError> {
    let root = path.as_ref();
    require_directory(root, ".roproj root")?;
    require_exact_root_entries(root)?;

    let manifest = read_file(&root.join("manifest.json"))?;
    dispatch_manifest(&manifest)?;
    let schemas = read_file(&root.join("schemas.json"))?;
    let mut records = Vec::new();
    collect_entity_records(&root.join("entities"), &mut records)?;
    canonicalize_unordered(&manifest, &schemas, records)
}

/// Publish an exact canonical `.roproj/v1` tree without replacing a destination.
///
/// The complete tree is written to a unique sibling staging directory and made
/// visible at `path` only after every file write succeeds.
///
/// # Errors
///
/// Returns validation/representation errors before touching the destination,
/// [`FormatError::AlreadyExists`] for every pre-existing destination, or a
/// host write error. An ordinary error never leaves the staging directory.
pub fn publish_roproj(
    path: impl AsRef<Path>,
    tree: &CanonicalRoProjectV1,
) -> Result<(), FormatError> {
    let path = path.as_ref();
    let validated = CanonicalRoProjectV1::try_from_files(
        tree.files()
            .iter()
            .map(|file| (file.path().to_owned(), file.bytes().to_vec()))
            .collect(),
    )?;

    match fs::symlink_metadata(path) {
        Ok(_) => {
            return Err(FormatError::AlreadyExists {
                path: path.to_owned(),
            });
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(FormatError::Write {
                path: path.to_owned(),
                source,
            });
        }
    }

    let staging = create_staging_directory(path)?;
    if let Err(error) = write_staging_tree(&staging, &validated) {
        remove_staging_directory(&staging)?;
        return Err(error);
    }
    finish_staged_publication(&staging, path)
}

/// Encode and publish a semantic document as canonical `.roproj/v1`.
///
/// # Errors
///
/// Returns semantic/encoding failures before touching the destination and
/// otherwise the publication errors documented by [`publish_roproj`].
pub fn materialize_roproj(path: impl AsRef<Path>, document: &Document) -> Result<(), FormatError> {
    let tree = encode(document)?;
    publish_roproj(path, &tree)
}

fn create_staging_directory(destination: &Path) -> Result<PathBuf, FormatError> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let basename =
        destination
            .file_name()
            .ok_or_else(|| FormatError::InvalidRoProjectRepresentation {
                message: format!(
                    "publication destination '{}' has no file name",
                    destination.display()
                ),
            })?;

    loop {
        let sequence = NEXT_STAGING_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let mut staging_name = OsString::from(".");
        staging_name.push(basename);
        staging_name.push(format!(".tachiko-stage-{}-{sequence}", std::process::id()));
        let staging = parent.join(staging_name);
        match fs::create_dir(&staging) {
            Ok(()) => return Ok(staging),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {}
            Err(source) => {
                return Err(FormatError::Write {
                    path: staging,
                    source,
                });
            }
        }
    }
}

fn write_staging_tree(staging: &Path, tree: &CanonicalRoProjectV1) -> Result<(), FormatError> {
    let entities = staging.join("entities");
    fs::create_dir(&entities).map_err(|source| FormatError::Write {
        path: entities,
        source,
    })?;
    for file in tree.files() {
        let path = staging.join(file.path());
        fs::write(&path, file.bytes()).map_err(|source| FormatError::Write { path, source })?;
    }
    Ok(())
}

fn finish_staged_publication(staging: &Path, destination: &Path) -> Result<(), FormatError> {
    match fs::rename(staging, destination) {
        Ok(()) => Ok(()),
        Err(source) => {
            let error = if source.kind() == io::ErrorKind::AlreadyExists {
                FormatError::AlreadyExists {
                    path: destination.to_owned(),
                }
            } else {
                FormatError::Write {
                    path: destination.to_owned(),
                    source,
                }
            };
            remove_staging_directory(staging)?;
            Err(error)
        }
    }
}

fn remove_staging_directory(staging: &Path) -> Result<(), FormatError> {
    fs::remove_dir_all(staging).map_err(|source| FormatError::Write {
        path: staging.to_owned(),
        source,
    })
}

fn require_exact_root_entries(root: &Path) -> Result<(), FormatError> {
    let mut found_manifest = false;
    let mut found_schemas = false;
    let mut found_entities = false;
    for entry in read_directory(root)? {
        let name = entry.file_name();
        if name == "manifest.json" {
            found_manifest = true;
        } else if name == "schemas.json" {
            found_schemas = true;
        } else if name == "entities" {
            found_entities = true;
        } else {
            return invalid_layout(
                &entry.path(),
                "unknown top-level child in canonical .roproj/v1",
            );
        }
    }
    if !found_manifest {
        return invalid_layout(
            &root.join("manifest.json"),
            "required regular file is missing",
        );
    }
    if !found_schemas {
        return invalid_layout(
            &root.join("schemas.json"),
            "required regular file is missing",
        );
    }
    if !found_entities {
        return invalid_layout(
            &root.join("entities"),
            "required ordinary directory is missing",
        );
    }
    require_regular_file(&root.join("manifest.json"), "manifest.json")?;
    require_regular_file(&root.join("schemas.json"), "schemas.json")?;
    require_directory(&root.join("entities"), "entities directory")
}

fn require_exact_entity_entries(entities: &Path) -> Result<(), FormatError> {
    let mut present = [false; 16];
    for entry in read_directory(entities)? {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            return invalid_layout(&entry.path(), "entity shard name is not canonical UTF-8");
        };
        let Some(index) = ROPROJ_V1_PATHS
            .iter()
            .skip(2)
            .position(|path| path.strip_prefix("entities/") == Some(name.as_str()))
        else {
            return invalid_layout(
                &entry.path(),
                "unknown child in canonical .roproj/v1 entities directory",
            );
        };
        require_regular_file(&entry.path(), "canonical entity shard")?;
        present[index] = true;
    }
    for (index, is_present) in present.into_iter().enumerate() {
        if !is_present {
            return invalid_layout(
                &entities.join(ROPROJ_V1_PATHS[index + 2].trim_start_matches("entities/")),
                "required canonical entity shard is missing",
            );
        }
    }
    Ok(())
}

fn collect_entity_records(
    directory: &Path,
    records: &mut Vec<(String, Vec<u8>)>,
) -> Result<usize, FormatError> {
    let mut accepted_files = 0;
    for entry in read_directory(directory)? {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|source| FormatError::Read {
            path: path.clone(),
            source,
        })?;
        if metadata.file_type().is_file() {
            if path.extension().and_then(|extension| extension.to_str()) != Some("jsonl") {
                return invalid_layout(&path, "entity input file must have the .jsonl extension");
            }
            accepted_files += 1;
            append_jsonl_records(&path, records)?;
        } else if metadata.file_type().is_dir() {
            let descendants = collect_entity_records(&path, records)?;
            if descendants == 0 {
                return invalid_layout(
                    &path,
                    "nested entity directory has no accepted regular .jsonl descendant",
                );
            }
            accepted_files += descendants;
        } else {
            return invalid_layout(
                &path,
                "entity input must be an ordinary directory or regular .jsonl file",
            );
        }
    }
    Ok(accepted_files)
}

fn append_jsonl_records(
    path: &Path,
    records: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), FormatError> {
    let bytes = read_file(path)?;
    if bytes.is_empty() {
        return Ok(());
    }
    let body =
        bytes
            .strip_suffix(b"\n")
            .ok_or_else(|| FormatError::InvalidRoProjectRepresentation {
                message: format!(
                    "entity input '{}' must terminate every physical record with LF",
                    path.display()
                ),
            })?;
    for (index, record) in body.split(|byte| *byte == b'\n').enumerate() {
        if record.is_empty() {
            return invalid_layout(path, "entity input contains a blank JSONL record");
        }
        records.push((format!("{}:{}", path.display(), index + 1), record.to_vec()));
    }
    Ok(())
}

fn require_directory(path: &Path, kind: &str) -> Result<(), FormatError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| FormatError::Read {
        path: path.to_owned(),
        source,
    })?;
    if metadata.file_type().is_dir() {
        Ok(())
    } else {
        invalid_layout(path, &format!("{kind} must be an ordinary directory"))
    }
}

fn require_regular_file(path: &Path, kind: &str) -> Result<(), FormatError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| FormatError::Read {
        path: path.to_owned(),
        source,
    })?;
    if metadata.file_type().is_file() {
        Ok(())
    } else {
        invalid_layout(path, &format!("{kind} must be an ordinary regular file"))
    }
}

fn read_directory(path: &Path) -> Result<Vec<fs::DirEntry>, FormatError> {
    let mut entries = fs::read_dir(path)
        .map_err(|source| FormatError::Read {
            path: path.to_owned(),
            source,
        })?
        .map(|entry| {
            entry.map_err(|source| FormatError::Read {
                path: path.to_owned(),
                source,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    Ok(entries)
}

fn read_file(path: &Path) -> Result<Vec<u8>, FormatError> {
    fs::read(path).map_err(|source| FormatError::Read {
        path: path.to_owned(),
        source,
    })
}

fn invalid_layout<T>(path: &Path, message: &str) -> Result<T, FormatError> {
    Err(FormatError::InvalidRoProjectRepresentation {
        message: format!("invalid .roproj layout at '{}': {message}", path.display()),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{NEXT_STAGING_DIRECTORY, finish_staged_publication};
    use std::sync::atomic::Ordering;

    #[test]
    fn failed_final_publication_removes_an_existing_staging_tree() {
        let sequence = NEXT_STAGING_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "tachiko-storage-staging-cleanup-{}-{sequence}",
            std::process::id()
        ));
        let staging = root.join("stage");
        let destination = root.join("destination");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&staging).unwrap();
        fs::write(staging.join("partial"), b"partial").unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("preserve"), b"preserve").unwrap();

        assert!(finish_staged_publication(&staging, &destination).is_err());
        assert!(!staging.exists());
        assert_eq!(fs::read(destination.join("preserve")).unwrap(), b"preserve");

        fs::remove_dir_all(root).unwrap();
    }
}
