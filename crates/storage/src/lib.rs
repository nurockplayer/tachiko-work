//! Versioned `.ro` serialization for Tachiko Work.

mod direct_ro;
mod legacy_direct_ro;
mod migration;
mod roproj;
mod strict_json;

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    str::Utf8Error,
};

use direct_ro::v2::{CodecError as V2CodecError, DocumentV2};
use strict_json::{FrontendError, VersionToken, inspect};
use tachiko_semantic_core::{Diagnostic, Document, validate_document};
use thiserror::Error;

pub use roproj::{
    CanonicalRoProjectFile, CanonicalRoProjectV1, ROPROJ_V1_FORMAT_VERSION, ROPROJ_V1_PATHS,
    decode_roproj_v1, encode_roproj_v1,
};

pub const LEGACY_FORMAT_VERSION: u32 = 1;
pub const FORMAT_VERSION: u32 = 2;
pub const SUPPORTED_FORMAT_VERSIONS: &[u32] = &[LEGACY_FORMAT_VERSION, FORMAT_VERSION];

/// Complete-input limit for the normal direct-JSON admission profile.
pub const NORMAL_DIRECT_JSON_MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
/// Maximum lexical length of one RFC 8259 number token in direct-ro/v2.
pub const V2_MAX_NUMBER_TOKEN_BYTES: usize = 256;

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
    #[error("unsupported .ro format version {found}; this build supports through {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },
    #[error("invalid .roproj UTF-8 in '{path}': {source}")]
    InvalidRoProjectUtf8 {
        path: String,
        #[source]
        source: Utf8Error,
    },
    #[error("invalid .roproj JSON in '{path}': {source}")]
    InvalidRoProjectJson {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("duplicate .roproj JSON member '{member}' in '{path}'")]
    DuplicateRoProjectMember { path: String, member: String },
    #[error("missing required .roproj format discriminator")]
    RoProjectFormatMissing,
    #[error("malformed .roproj format discriminator")]
    RoProjectFormatMalformed,
    #[error("missing required .roproj format version")]
    RoProjectVersionMissing,
    #[error("malformed .roproj format version")]
    RoProjectVersionMalformed,
    #[error("unsupported .roproj format version {found}; this build supports version {supported}")]
    UnsupportedRoProjectVersion { found: u32, supported: u32 },
    #[error("invalid .roproj representation: {message}")]
    InvalidRoProjectRepresentation { message: String },
    #[error("invalid direct .ro representation: {message}")]
    InvalidRepresentation {
        message: String,
        #[source]
        source: Option<serde_json::Error>,
    },
    #[error("direct-JSON {resource} limit exceeded: maximum {limit} bytes, found {actual} bytes")]
    ResourceLimit {
        resource: &'static str,
        limit: usize,
        actual: usize,
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

/// Serialize a valid semantic document into canonical direct-ro/v2 UTF-8 JSON.
///
/// # Errors
///
/// Returns [`FormatError::InvalidDocument`] for semantic diagnostics,
/// [`FormatError::ResourceLimit`] when the canonical v2 representation exceeds
/// the normal direct-JSON complete-input profile or its v2-specific number-token
/// profile, or a representation error if conversion or canonical encoding
/// fails.
pub fn to_canonical_string(document: &Document) -> Result<String, FormatError> {
    check_document(document)?;
    let dto = DocumentV2::from_semantic(document).map_err(map_v2_encode_error)?;
    let encoded = direct_ro::v2::encode(&dto).map_err(map_v2_encode_error)?;
    enforce_normal_direct_json_input_limit(encoded.as_bytes())?;
    enforce_v2_number_token_limit(&encoded)?;
    Ok(encoded)
}

/// Parse and validate a supported, versioned `.ro` JSON document.
///
/// Legacy v1 is migrated deterministically in memory. Reading never modifies
/// the durable source; any durable v2 materialization remains an explicit save.
///
/// # Errors
///
/// Returns an error when JSON is malformed or ambiguous, the format version is
/// missing, malformed, or unsupported, migration fails, the recognized
/// representation is invalid, or semantic validation fails.
pub fn from_str(source: &str) -> Result<Document, FormatError> {
    from_bytes(source.as_bytes())
}

/// Parse and validate a versioned `.ro` JSON document from bytes.
///
/// # Errors
///
/// Returns a machine-distinguishable [`FormatError`] at the first failed stage
/// of the storage reader pipeline.
pub fn from_bytes(source: &[u8]) -> Result<Document, FormatError> {
    let (source_text, version) = inspect_envelope(source)?;
    let document = match version {
        LEGACY_FORMAT_VERSION => {
            let legacy = decode_v1_dto_for_migration(source)?;
            migration::legacy_v1_to_v2(legacy)
                .map_err(|error| FormatError::MigrationFailed { message: error.0 })?
                .into_semantic()
                .map_err(map_v2_decode_error)?
        }
        FORMAT_VERSION => {
            enforce_v2_number_token_limit(source_text)?;
            let dto = decode_v2_dto(source_text)?;
            dto.validate().map_err(map_v2_decode_error)?;
            dto.into_semantic().map_err(map_v2_decode_error)?
        }
        _ => unreachable!("supported direct .ro versions are dispatched exhaustively"),
    };
    check_document(&document)?;
    Ok(document)
}

/// Decode and validate the immutable legacy v1 DTO without crossing into the
/// current semantic model.
///
/// Explicit migration consumes this crate-internal seam so historical bytes
/// pass the complete strict reader and exact v1 dispatch first.
pub(crate) fn decode_v1_dto_for_migration(
    source: &[u8],
) -> Result<legacy_direct_ro::v1::DocumentV1, FormatError> {
    let (source, version) = inspect_envelope(source)?;
    if version != LEGACY_FORMAT_VERSION {
        return Err(FormatError::UnsupportedVersion {
            found: version,
            supported: FORMAT_VERSION,
        });
    }
    let document: legacy_direct_ro::v1::DocumentV1 = serde_json::from_str(source)
        .map_err(legacy_direct_ro::v1::CodecError::Json)
        .map_err(map_v1_decode_error)?;
    document.validate().map_err(map_v1_decode_error)?;
    Ok(document)
}

/// Re-encode a valid frozen v1 document using the immutable v1 canonical codec.
///
/// This compatibility helper does not convert through current semantic types
/// and does not write or migrate durable state.
///
/// # Errors
///
/// Returns strict v1 decoding or canonical encoding failures.
pub fn canonicalize_legacy_v1(source: &[u8]) -> Result<String, FormatError> {
    let dto = decode_v1_dto_for_migration(source)?;
    legacy_direct_ro::v1::encode_dto(&dto).map_err(map_v1_encode_error)
}

/// Load a validated semantic document from a UTF-8 `.ro` file.
///
/// # Errors
///
/// Returns [`FormatError::Read`] when the path cannot be read and otherwise
/// propagates parsing, compatibility, migration, and validation errors.
pub fn load(path: impl AsRef<Path>) -> Result<Document, FormatError> {
    let path = path.as_ref();
    let source = fs::read(path).map_err(|source| FormatError::Read {
        path: path.to_owned(),
        source,
    })?;
    from_bytes(&source)
}

/// Save a semantic document using canonical direct-ro/v2 encoding.
///
/// The destination is created atomically with respect to existence: an
/// existing path is never truncated or overwritten.
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

fn inspect_envelope(source: &[u8]) -> Result<(&str, u32), FormatError> {
    enforce_normal_direct_json_input_limit(source)?;
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
    Ok((source, version))
}

fn enforce_normal_direct_json_input_limit(source: &[u8]) -> Result<(), FormatError> {
    if source.len() > NORMAL_DIRECT_JSON_MAX_INPUT_BYTES {
        return Err(FormatError::ResourceLimit {
            resource: "input",
            limit: NORMAL_DIRECT_JSON_MAX_INPUT_BYTES,
            actual: source.len(),
        });
    }
    Ok(())
}

fn enforce_v2_number_token_limit(source: &str) -> Result<(), FormatError> {
    let bytes = source.as_bytes();
    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if byte == b'"' {
            in_string = true;
            index += 1;
            continue;
        }
        if byte == b'-' || byte.is_ascii_digit() {
            let start = index;
            index += 1;
            while index < bytes.len()
                && matches!(bytes[index], b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-')
            {
                index += 1;
            }
            let actual = index - start;
            if actual > V2_MAX_NUMBER_TOKEN_BYTES {
                return Err(FormatError::ResourceLimit {
                    resource: "number token",
                    limit: V2_MAX_NUMBER_TOKEN_BYTES,
                    actual,
                });
            }
            continue;
        }
        index += 1;
    }
    Ok(())
}

fn decode_v2_dto(source: &str) -> Result<DocumentV2, FormatError> {
    let mut value: serde_json::Value = serde_json::from_str(source)
        .map_err(V2CodecError::Json)
        .map_err(map_v2_decode_error)?;
    normalize_v2_number_tokens(&mut value, true)?;
    serde_json::from_value(value)
        .map_err(V2CodecError::Json)
        .map_err(map_v2_decode_error)
}

fn normalize_v2_number_tokens(
    value: &mut serde_json::Value,
    root_object: bool,
) -> Result<(), FormatError> {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                normalize_v2_number_tokens(value, false)?;
            }
        }
        serde_json::Value::Object(members) => {
            for (name, value) in members {
                if root_object && name == "format_version" {
                    continue;
                }
                normalize_v2_number_tokens(value, false)?;
            }
        }
        serde_json::Value::Number(number) => {
            let token = number.to_string();
            let parsed = token
                .parse::<f64>()
                .map_err(|_| FormatError::InvalidRepresentation {
                    message: format!("number token '{token}' cannot be converted to binary64"),
                    source: None,
                })?;
            if !parsed.is_finite() {
                return Err(FormatError::InvalidRepresentation {
                    message: format!(
                        "number token '{token}' converts to a non-finite binary64 value"
                    ),
                    source: None,
                });
            }
            let parsed = if parsed == 0.0 { 0.0 } else { parsed };
            *number = serde_json::Number::from_f64(parsed).ok_or_else(|| {
                FormatError::InvalidRepresentation {
                    message: format!(
                        "number token '{token}' cannot be represented as finite binary64"
                    ),
                    source: None,
                }
            })?;
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::String(_) => {}
    }
    Ok(())
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
        FrontendError::NestingLimit { limit } => FormatError::InvalidRepresentation {
            message: format!("JSON nesting exceeds representation limit {limit}"),
            source: None,
        },
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

fn map_v2_decode_error(error: V2CodecError) -> FormatError {
    match error {
        V2CodecError::Json(source) => FormatError::InvalidRepresentation {
            message: source.to_string(),
            source: Some(source),
        },
        V2CodecError::InvalidRepresentation(message) => FormatError::InvalidRepresentation {
            message,
            source: None,
        },
    }
}

fn map_v2_encode_error(error: V2CodecError) -> FormatError {
    match error {
        V2CodecError::Json(source) => FormatError::Json(source),
        V2CodecError::InvalidRepresentation(message) => FormatError::InvalidRepresentation {
            message,
            source: None,
        },
    }
}
