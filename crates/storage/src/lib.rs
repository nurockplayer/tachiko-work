//! Versioned `.ro` serialization for Tachiko Work.

mod legacy_direct_ro;
mod strict_json;

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    str::Utf8Error,
};

use strict_json::{FrontendError, VersionToken, inspect};
use tachiko_semantic_core::{Diagnostic, Document, validate_document};
use thiserror::Error;

pub const FORMAT_VERSION: u32 = 1;
pub const SUPPORTED_FORMAT_VERSIONS: &[u32] = &[FORMAT_VERSION];

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("invalid .ro UTF-8: {source}")]
    InvalidUtf8 {
        #[source]
        source: Utf8Error,
    },
    #[error("invalid .ro JSON: {source}")]
    InvalidJson {
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to encode .ro JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("duplicate JSON member '{member}'")]
    DuplicateMember { member: String },
    #[error("missing required .ro format version")]
    VersionMissing,
    #[error("malformed .ro format version")]
    VersionMalformed,
    #[error("unsupported .ro format version {found}; this build supports {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },
    #[error("invalid version-1 .ro representation: {message}")]
    InvalidRepresentation {
        message: String,
        #[source]
        source: Option<serde_json::Error>,
    },
    #[error(".ro migration failed: {message}")]
    MigrationFailed { message: String },
    #[error("semantic document is invalid: {diagnostics:?}")]
    InvalidDocument { diagnostics: Vec<Diagnostic> },
    #[error("'{}' already exists; refusing to overwrite it", path.display())]
    AlreadyExists { path: PathBuf },
    #[error("failed to read '{}': {source}", path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("failed to write '{}': {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
}

/// Serialize a valid version-1 document into canonical UTF-8 JSON.
///
/// # Errors
///
/// Returns [`FormatError::InvalidDocument`] for semantic diagnostics or
/// [`FormatError::Json`] when JSON encoding fails.
pub fn to_canonical_string(document: &Document) -> Result<String, FormatError> {
    check_document(document)?;
    legacy_direct_ro::v1::encode(document).map_err(map_v1_encode_error)
}

/// Parse and validate a versioned `.ro` JSON document.
///
/// # Errors
///
/// Returns an error when JSON is malformed or ambiguous, the format version is
/// missing, malformed, or unsupported, the recognized representation is
/// invalid, or semantic validation fails.
pub fn from_str(source: &str) -> Result<Document, FormatError> {
    from_bytes(source.as_bytes())
}

/// Parse and validate a versioned `.ro` JSON document from bytes.
///
/// This byte-oriented entry point preserves the required distinction between
/// invalid UTF-8 and invalid JSON before version dispatch.
///
/// # Errors
///
/// Returns a machine-distinguishable [`FormatError`] at the first failed stage
/// of the storage reader pipeline.
pub fn from_bytes(source: &[u8]) -> Result<Document, FormatError> {
    let source =
        std::str::from_utf8(source).map_err(|source| FormatError::InvalidUtf8 { source })?;
    let inspection = inspect(source).map_err(map_frontend_error)?;
    let version = match inspection.version {
        None => return Err(FormatError::VersionMissing),
        Some(VersionToken::Unsigned(version)) => {
            u32::try_from(version).map_err(|_| FormatError::VersionMalformed)?
        }
        Some(VersionToken::Other) => return Err(FormatError::VersionMalformed),
    };
    if version == 0 {
        return Err(FormatError::VersionMalformed);
    }
    check_version(version)?;

    let document = match version {
        FORMAT_VERSION => legacy_direct_ro::v1::decode(source).map_err(map_v1_decode_error)?,
        _ => unreachable!("supported versions are dispatched exhaustively"),
    };
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
    let source = fs::read(path).map_err(|source| FormatError::Read {
        path: path.to_owned(),
        source,
    })?;
    from_bytes(&source)
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
    if SUPPORTED_FORMAT_VERSIONS.contains(&version) {
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

fn map_frontend_error(error: FrontendError) -> FormatError {
    match error {
        FrontendError::InvalidJson(source) => FormatError::InvalidJson { source },
        FrontendError::DuplicateMember(member) => FormatError::DuplicateMember { member },
    }
}

fn map_v1_decode_error(error: legacy_direct_ro::v1::CodecError) -> FormatError {
    match error {
        legacy_direct_ro::v1::CodecError::Json(source) => FormatError::InvalidRepresentation {
            message: source.to_string(),
            source: Some(source),
        },
        legacy_direct_ro::v1::CodecError::InvalidRepresentation(message) => {
            FormatError::InvalidRepresentation {
                message,
                source: None,
            }
        }
    }
}

fn map_v1_encode_error(error: legacy_direct_ro::v1::CodecError) -> FormatError {
    match error {
        legacy_direct_ro::v1::CodecError::Json(source) => FormatError::Json(source),
        legacy_direct_ro::v1::CodecError::InvalidRepresentation(message) => {
            FormatError::InvalidRepresentation {
                message,
                source: None,
            }
        }
    }
}
