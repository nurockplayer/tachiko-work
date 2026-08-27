//! Portable `.ro` package profiles layered over canonical `.roproj` bytes.

mod host;
mod v1;

use std::{io, path::PathBuf};

use tachiko_semantic_core::Diagnostic;
use thiserror::Error;

pub use host::{
    compare_portable_package_with_roproj, compare_verified_package_with_roproj, pack_roproj,
    publish_portable_package, publish_portable_package_from_roproj, publish_unpacked_roproj,
    read_portable_package, read_portable_package_source, unpack_roproj,
};
pub use v1::{
    PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES, VerifiedPortablePackageV1,
    decode as decode_portable_package_v1, encode as encode_portable_package_v1,
    payload_root as portable_package_payload_root,
};

/// Stable portable-package failure meanings from the v1 package contract.
#[derive(Debug, Error)]
pub enum PortablePackageError {
    #[error("portable_package.invalid_container: {message}")]
    InvalidContainer { message: String },
    #[error("portable_package.invalid_manifest: {message}")]
    InvalidManifest { message: String },
    #[error("portable_package.unsupported_version: unsupported package version {found}")]
    UnsupportedVersion { found: String },
    #[error("portable_package.noncanonical_container: {message}")]
    NonCanonicalContainer { message: String },
    #[error("portable_package.entry_set_mismatch: {message}")]
    EntrySetMismatch { message: String },
    #[error("portable_package.crc_mismatch: stored size or CRC disagrees for '{path}'")]
    CrcMismatch { path: String },
    #[error("portable_package.integrity_mismatch: payload root does not match package.json")]
    IntegrityMismatch,
    #[error(
        "portable_package.payload_manifest_mismatch: inner .roproj manifest disagrees with package claims"
    )]
    PayloadManifestMismatch,
    #[error("portable_package.noncanonical_payload: {message}")]
    NonCanonicalPayload { message: String },
    #[error("portable_package.invalid_semantic_payload: {diagnostics:?}")]
    InvalidSemanticPayload { diagnostics: Vec<Diagnostic> },
    #[error("portable_package.source_not_canonical: {message}")]
    SourceNotCanonical { message: String },
    #[error("portable_package.capacity_exceeded: {resource} exceeds ordinary ZIP32 capacity")]
    CapacityExceeded { resource: &'static str },
    #[error("portable_package.resource_limit: {resource} limit is {limit}, found {actual}")]
    ResourceLimit {
        resource: &'static str,
        limit: usize,
        actual: usize,
    },
    #[error("portable_package.destination_exists: '{}' already exists", path.display())]
    DestinationExists { path: PathBuf },
    #[error("portable_package.publication_failed: failed to publish '{}': {source}", path.display())]
    PublicationFailed {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(
        "portable_package.source_mismatch: package root {package_root} differs from tracked root {tracked_root}"
    )]
    SourceMismatch {
        package_root: String,
        tracked_root: String,
    },
}
