//! Native admission and no-clobber publication for portable packages.

use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use super::{
    PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES, PortablePackageError, VerifiedPortablePackageV1,
    v1::{decode, encode, payload_root},
};
use crate::{
    FormatError,
    roproj::{CanonicalRoProjectV1, publish_roproj},
};

static NEXT_STAGING_FILE: AtomicU64 = AtomicU64::new(0);

/// Read and completely verify a package-v1 artifact within the declared bound.
///
/// # Errors
///
/// Returns host read failures or the pure package decoder's stable failures.
pub fn read_portable_package(
    path: impl AsRef<Path>,
) -> Result<VerifiedPortablePackageV1, FormatError> {
    let path = path.as_ref();
    let source = read_bounded(path, PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES)?;
    decode(&source)
}

/// Publish canonical package-v1 bytes for an already-admitted payload tree.
///
/// The complete package is encoded before a sibling staging file is created.
/// Final publication uses the platform's atomic no-replace rename operation.
///
/// # Errors
///
/// Returns package capacity/resource failures, destination conflicts, or
/// publication failures without exposing a partial destination.
pub fn publish_portable_package(
    path: impl AsRef<Path>,
    tree: &CanonicalRoProjectV1,
) -> Result<(), FormatError> {
    let path = path.as_ref();
    let bytes = encode(tree)?;
    require_destination_absent(path)?;
    let (staging, mut file) = create_staging_file(path)?;
    if let Err(source) = file.write_all(&bytes) {
        drop(file);
        remove_staging_file(&staging)?;
        return Err(PortablePackageError::PublicationFailed {
            path: staging,
            source,
        }
        .into());
    }
    drop(file);
    finish_staged_file(&staging, path)
}

/// Publish a package snapshot while proving the output is outside its source.
///
/// # Errors
///
/// Returns [`FormatError::PathOverlap`] for a destination that could mutate the
/// source tree, otherwise the errors from [`publish_portable_package`].
pub fn publish_portable_package_from_roproj(
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
    tree: &CanonicalRoProjectV1,
) -> Result<(), FormatError> {
    let source = source.as_ref();
    let destination = destination.as_ref();
    crate::roproj::host::ensure_destination_outside_source(source, destination)?;
    publish_portable_package(destination, tree)
}

/// Read an exact canonical `.roproj/v1` source and publish a portable package.
///
/// # Errors
///
/// Noncanonical or unreadable source trees map to the stable
/// `portable_package.source_not_canonical` meaning. Publication is atomic and
/// never replaces a destination.
pub fn pack_roproj(
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<(), FormatError> {
    let source = source.as_ref();
    let destination = destination.as_ref();
    crate::roproj::host::ensure_destination_outside_source(source, destination)?;
    let tree = read_portable_package_source(source)?;
    publish_portable_package(destination, &tree)
}

/// Read the exact canonical source family accepted by package-v1 pack/compare.
///
/// # Errors
///
/// Every host, representation, or semantic rejection maps to the stable
/// `portable_package.source_not_canonical` meaning.
pub fn read_portable_package_source(
    source: impl AsRef<Path>,
) -> Result<CanonicalRoProjectV1, FormatError> {
    crate::roproj::host::read_canonical_roproj_bounded(
        source,
        PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES,
    )
    .map_err(|error| match error {
        FormatError::PortablePackage(PortablePackageError::ResourceLimit { .. }) => error,
        other => map_source_error(&other),
    })
}

/// Publish the exact payload of an already-verified package to an absent tree.
///
/// # Errors
///
/// Returns portable-package destination/publication meanings. Package decoding
/// has already completed and no package metadata is copied into the tree.
pub fn publish_unpacked_roproj(
    destination: impl AsRef<Path>,
    package: &VerifiedPortablePackageV1,
) -> Result<(), FormatError> {
    let destination = destination.as_ref();
    publish_roproj(destination, package.tree())
        .map_err(|error| map_tree_publication_error(destination, error))
}

/// Verify a package and publish its exact canonical payload to an absent tree.
///
/// # Errors
///
/// Returns read/verification failures before publication and portable-package
/// destination/publication failures at the host boundary.
pub fn unpack_roproj(
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<(), FormatError> {
    let package = read_portable_package(source)?;
    publish_unpacked_roproj(destination, &package)
}

/// Compare one verified package root with one exact canonical tracked source.
///
/// This operation is read-only. Equal roots are returned; differing roots map
/// to `portable_package.source_mismatch` without synchronizing either input.
///
/// # Errors
///
/// Returns package validation, canonical tracked-source, or source-mismatch
/// failures.
pub fn compare_portable_package_with_roproj(
    package_path: impl AsRef<Path>,
    tracked_path: impl AsRef<Path>,
) -> Result<[u8; 32], FormatError> {
    let package = read_portable_package(package_path)?;
    let tracked = read_portable_package_source(tracked_path)?;
    compare_verified_package_with_roproj(&package, &tracked)
}

/// Compare an already-verified package snapshot with an admitted source tree.
///
/// # Errors
///
/// Returns only the stable source-mismatch failure when roots differ.
pub fn compare_verified_package_with_roproj(
    package: &VerifiedPortablePackageV1,
    tracked: &CanonicalRoProjectV1,
) -> Result<[u8; 32], FormatError> {
    let tracked_root = payload_root(tracked);
    if package.payload_root() != tracked_root {
        return Err(PortablePackageError::SourceMismatch {
            package_root: encode_hex(&package.payload_root()),
            tracked_root: encode_hex(&tracked_root),
        }
        .into());
    }
    Ok(tracked_root)
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, FormatError> {
    let file = File::open(path).map_err(|source| FormatError::Read {
        path: path.to_owned(),
        source,
    })?;
    let read_limit = u64::try_from(limit)
        .expect("package limit fits u64")
        .saturating_add(1);
    let mut source = Vec::new();
    file.take(read_limit)
        .read_to_end(&mut source)
        .map_err(|source| FormatError::Read {
            path: path.to_owned(),
            source,
        })?;
    if source.len() > limit {
        return Err(PortablePackageError::ResourceLimit {
            resource: "archive bytes",
            limit,
            actual: source.len(),
        }
        .into());
    }
    Ok(source)
}

fn require_destination_absent(path: &Path) -> Result<(), FormatError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(PortablePackageError::DestinationExists {
            path: path.to_owned(),
        }
        .into()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(PortablePackageError::PublicationFailed {
            path: path.to_owned(),
            source,
        }
        .into()),
    }
}

fn create_staging_file(destination: &Path) -> Result<(PathBuf, File), FormatError> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let basename =
        destination
            .file_name()
            .ok_or_else(|| PortablePackageError::PublicationFailed {
                path: destination.to_owned(),
                source: io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "publication destination has no file name",
                ),
            })?;
    loop {
        let sequence = NEXT_STAGING_FILE.fetch_add(1, Ordering::Relaxed);
        let mut staging_name = OsString::from(".");
        staging_name.push(basename);
        staging_name.push(format!(
            ".tachiko-package-stage-{}-{sequence}",
            std::process::id()
        ));
        let staging = parent.join(staging_name);
        match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&staging)
        {
            Ok(file) => return Ok((staging, file)),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {}
            Err(source) => {
                return Err(PortablePackageError::PublicationFailed {
                    path: staging,
                    source,
                }
                .into());
            }
        }
    }
}

fn finish_staged_file(staging: &Path, destination: &Path) -> Result<(), FormatError> {
    match renamore::rename_exclusive(staging, destination) {
        Ok(()) => Ok(()),
        Err(source) => {
            let kind = source.kind();
            if let Err(cleanup) = fs::remove_file(staging) {
                return Err(PortablePackageError::PublicationFailed {
                    path: staging.to_owned(),
                    source: cleanup,
                }
                .into());
            }
            if kind == io::ErrorKind::AlreadyExists {
                Err(PortablePackageError::DestinationExists {
                    path: destination.to_owned(),
                }
                .into())
            } else {
                Err(PortablePackageError::PublicationFailed {
                    path: destination.to_owned(),
                    source,
                }
                .into())
            }
        }
    }
}

fn remove_staging_file(staging: &Path) -> Result<(), FormatError> {
    fs::remove_file(staging).map_err(|source| {
        PortablePackageError::PublicationFailed {
            path: staging.to_owned(),
            source,
        }
        .into()
    })
}

fn map_source_error(error: &FormatError) -> FormatError {
    PortablePackageError::SourceNotCanonical {
        message: error.to_string(),
    }
    .into()
}

fn map_tree_publication_error(destination: &Path, error: FormatError) -> FormatError {
    match error {
        FormatError::AlreadyExists { .. } => PortablePackageError::DestinationExists {
            path: destination.to_owned(),
        }
        .into(),
        FormatError::Write { source, .. } => PortablePackageError::PublicationFailed {
            path: destination.to_owned(),
            source,
        }
        .into(),
        other => PortablePackageError::PublicationFailed {
            path: destination.to_owned(),
            source: io::Error::other(other.to_string()),
        }
        .into(),
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::atomic::Ordering};

    use super::{NEXT_STAGING_FILE, finish_staged_file};

    #[test]
    fn exclusive_file_publication_preserves_a_destination_created_by_a_racer() {
        let sequence = NEXT_STAGING_FILE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "tachiko-package-file-race-{}-{sequence}",
            std::process::id()
        ));
        let staging = root.join("stage");
        let destination = root.join("destination");
        fs::create_dir(&root).unwrap();
        fs::write(&staging, b"candidate").unwrap();
        fs::write(&destination, b"racer").unwrap();

        assert!(matches!(
            finish_staged_file(&staging, &destination),
            Err(crate::FormatError::PortablePackage(
                crate::PortablePackageError::DestinationExists { .. }
            ))
        ));
        assert!(!staging.exists());
        assert_eq!(fs::read(&destination).unwrap(), b"racer");

        fs::remove_dir_all(root).unwrap();
    }
}
