//! Versioned `.ro` serialization for Tachiko Work.

use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tachiko_semantic_core::{
    Diagnostic, Document, DocumentId, Entity, EntityId, Schema, SchemaId, validate_document,
};
use thiserror::Error;

pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("invalid .ro JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported .ro format version {found}; this build supports {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },
    #[error("semantic document is invalid: {diagnostics:?}")]
    InvalidDocument { diagnostics: Vec<Diagnostic> },
    #[error("'{}' already exists; refusing to overwrite it", path.display())]
    AlreadyExists { path: PathBuf },
    #[error("failed to read '{}': {source}", path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("failed to write '{}': {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
}

#[derive(Deserialize)]
struct VersionProbe {
    format_version: u32,
}

#[derive(Serialize)]
struct DocumentV1Ref<'document> {
    format_version: u32,
    id: &'document DocumentId,
    title: &'document str,
    schemas: &'document BTreeMap<SchemaId, Schema>,
    entities: &'document BTreeMap<EntityId, Entity>,
}

impl<'document> From<&'document Document> for DocumentV1Ref<'document> {
    fn from(document: &'document Document) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            id: &document.id,
            title: &document.title,
            schemas: &document.schemas,
            entities: &document.entities,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentV1 {
    format_version: u32,
    id: DocumentId,
    title: String,
    schemas: BTreeMap<SchemaId, Schema>,
    entities: BTreeMap<EntityId, Entity>,
}

impl DocumentV1 {
    fn into_document(self) -> Document {
        let Self {
            format_version: _,
            id,
            title,
            schemas,
            entities,
        } = self;
        Document {
            id,
            title,
            schemas,
            entities,
        }
    }
}

/// Serialize a valid version-1 document into canonical UTF-8 JSON.
///
/// # Errors
///
/// Returns [`FormatError::InvalidDocument`] for semantic diagnostics or
/// [`FormatError::Json`] when JSON encoding fails.
pub fn to_canonical_string(document: &Document) -> Result<String, FormatError> {
    check_document(document)?;

    let mut encoded = serde_json::to_string_pretty(&DocumentV1Ref::from(document))?;
    encoded.push('\n');
    Ok(encoded)
}

/// Parse and validate a versioned `.ro` JSON document.
///
/// # Errors
///
/// Returns an error when JSON is malformed or unknown, the format version is
/// unsupported, or semantic validation fails.
pub fn from_str(source: &str) -> Result<Document, FormatError> {
    let version = serde_json::from_str::<VersionProbe>(source)?.format_version;
    check_version(version)?;

    let wire_document = serde_json::from_str::<DocumentV1>(source)?;
    check_version(wire_document.format_version)?;
    let document = wire_document.into_document();
    check_document(&document)?;
    Ok(document)
}

/// Load a validated semantic document from a UTF-8 `.ro` file.
///
/// # Errors
///
/// Returns [`FormatError::Read`] when the path cannot be read and otherwise
/// propagates parsing, compatibility, and semantic validation errors.
pub fn load(path: impl AsRef<Path>) -> Result<Document, FormatError> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|source| FormatError::Read {
        path: path.to_owned(),
        source,
    })?;
    from_str(&source)
}

/// Save a semantic document using canonical version-1 `.ro` encoding.
///
/// # Errors
///
/// Returns serialization/validation errors, [`FormatError::AlreadyExists`] when
/// the destination exists, or [`FormatError::Write`] when a new destination
/// cannot be created or written.
pub fn save(path: impl AsRef<Path>, document: &Document) -> Result<(), FormatError> {
    let path = path.as_ref();
    let encoded = to_canonical_string(document)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                FormatError::AlreadyExists {
                    path: path.to_owned(),
                }
            } else {
                FormatError::Write {
                    path: path.to_owned(),
                    source,
                }
            }
        })?;
    file.write_all(encoded.as_bytes())
        .map_err(|source| FormatError::Write {
            path: path.to_owned(),
            source,
        })
}

fn check_version(version: u32) -> Result<(), FormatError> {
    if version == FORMAT_VERSION {
        Ok(())
    } else {
        Err(FormatError::UnsupportedVersion {
            found: version,
            supported: FORMAT_VERSION,
        })
    }
}

fn check_document(document: &Document) -> Result<(), FormatError> {
    let diagnostics = validate_document(document);
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(FormatError::InvalidDocument { diagnostics })
    }
}
