use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Barrier,
        atomic::{AtomicU64, Ordering},
    },
    thread,
};

use tachiko_semantic_core::Document;
use tachiko_storage::{
    FormatError, PortablePackageError, compare_portable_package_with_roproj, pack_roproj,
    read_canonical_roproj, read_portable_package, unpack_roproj,
};

static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let sequence = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tachiko-portable-package-host-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn pack_unpack_and_compare_preserve_exact_canonical_bytes() {
    let temp = TempDir::new();
    let source = temp.path().join("source.roproj");
    let first_package = temp.path().join("first.ro");
    let second_package = temp.path().join("second.ro");
    let restored = temp.path().join("restored.roproj");
    tachiko_storage::materialize_roproj(&source, &Document::empty("doc-portable", "Portable"))
        .unwrap();
    let expected = read_canonical_roproj(&source).unwrap();

    pack_roproj(&source, &first_package).unwrap();
    pack_roproj(&source, &second_package).unwrap();
    assert_eq!(
        fs::read(&first_package).unwrap(),
        fs::read(&second_package).unwrap()
    );
    assert_eq!(
        read_portable_package(&first_package).unwrap().tree(),
        &expected
    );

    unpack_roproj(&first_package, &restored).unwrap();
    assert_eq!(read_canonical_roproj(&restored).unwrap(), expected);
    assert_eq!(
        compare_portable_package_with_roproj(&first_package, &source).unwrap(),
        read_portable_package(&first_package)
            .unwrap()
            .payload_root()
    );
}

#[test]
fn pack_and_unpack_never_overwrite_existing_destinations() {
    let temp = TempDir::new();
    let source = temp.path().join("source.roproj");
    let package = temp.path().join("package.ro");
    tachiko_storage::materialize_roproj(&source, &Document::empty("doc", "Document")).unwrap();
    pack_roproj(&source, &package).unwrap();

    let existing_package = temp.path().join("existing.ro");
    fs::write(&existing_package, b"preserve package destination").unwrap();
    assert!(matches!(
        pack_roproj(&source, &existing_package),
        Err(FormatError::PortablePackage(
            PortablePackageError::DestinationExists { .. }
        ))
    ));
    assert_eq!(
        fs::read(&existing_package).unwrap(),
        b"preserve package destination"
    );

    let existing_tree = temp.path().join("existing.roproj");
    fs::create_dir(&existing_tree).unwrap();
    fs::write(existing_tree.join("marker"), b"preserve tree destination").unwrap();
    assert!(matches!(
        unpack_roproj(&package, &existing_tree),
        Err(FormatError::PortablePackage(
            PortablePackageError::DestinationExists { .. }
        ))
    ));
    assert_eq!(
        fs::read(existing_tree.join("marker")).unwrap(),
        b"preserve tree destination"
    );
}

#[test]
fn noncanonical_source_and_source_mismatch_are_read_only() {
    let temp = TempDir::new();
    let source = temp.path().join("source.roproj");
    let other = temp.path().join("other.roproj");
    let package = temp.path().join("package.ro");
    let rejected = temp.path().join("rejected.ro");
    tachiko_storage::materialize_roproj(&source, &Document::empty("doc", "Source")).unwrap();
    tachiko_storage::materialize_roproj(&other, &Document::empty("doc", "Other")).unwrap();
    pack_roproj(&source, &package).unwrap();
    let package_before = fs::read(&package).unwrap();
    let other_before = read_canonical_roproj(&other).unwrap();

    assert!(matches!(
        compare_portable_package_with_roproj(&package, &other),
        Err(FormatError::PortablePackage(
            PortablePackageError::SourceMismatch { .. }
        ))
    ));
    assert_eq!(fs::read(&package).unwrap(), package_before);
    assert_eq!(read_canonical_roproj(&other).unwrap(), other_before);

    fs::write(source.join("extra"), b"unknown").unwrap();
    assert!(matches!(
        pack_roproj(&source, &rejected),
        Err(FormatError::PortablePackage(
            PortablePackageError::SourceNotCanonical { .. }
        ))
    ));
    assert!(!rejected.exists());
}

#[test]
fn concurrent_unpack_uses_the_real_no_replace_primitive() {
    let temp = TempDir::new();
    let source = temp.path().join("source.roproj");
    let package = temp.path().join("package.ro");
    let destination = temp.path().join("raced.roproj");
    tachiko_storage::materialize_roproj(&source, &Document::empty("doc", "Race")).unwrap();
    pack_roproj(&source, &package).unwrap();

    let barrier = Arc::new(Barrier::new(3));
    let handles = (0..2)
        .map(|_| {
            let package = package.clone();
            let destination = destination.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                unpack_roproj(package, destination)
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    let results = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(
                result,
                Err(FormatError::PortablePackage(
                    PortablePackageError::DestinationExists { .. }
                ))
            ))
            .count(),
        1
    );
    assert_eq!(
        read_canonical_roproj(&destination).unwrap(),
        read_canonical_roproj(&source).unwrap()
    );
}
