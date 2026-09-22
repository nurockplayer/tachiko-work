use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

struct OwnedTempDirectory(PathBuf);

impl OwnedTempDirectory {
    fn new(prefix: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{nonce}", std::process::id()));
        fs::create_dir(&path).unwrap_or_else(|error| {
            panic!(
                "must create an exclusively owned temporary directory {}: {error}",
                path.display()
            )
        });
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for OwnedTempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn tree_snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn visit(root: &Path, directory: &Path, entries: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry in fs::read_dir(directory).unwrap_or_else(|error| {
            panic!(
                "must read temporary cache directory {}: {error}",
                directory.display()
            )
        }) {
            let entry = entry.expect("temporary cache directory entries must be readable");
            let path = entry.path();
            let kind = entry
                .file_type()
                .expect("temporary cache entry type must be readable");
            if kind.is_dir() {
                visit(root, &path, entries);
            } else if kind.is_file() {
                entries.push((
                    path.strip_prefix(root)
                        .expect("temporary cache entry must remain inside its root")
                        .to_path_buf(),
                    fs::read(&path).expect("temporary cache file must be readable"),
                ));
            } else {
                panic!(
                    "temporary cache must not contain non-file entries: {}",
                    path.display()
                );
            }
        }
    }

    let mut entries = Vec::new();
    visit(root, root, &mut entries);
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries
}

#[test]
fn open_sheet_export_acceptance() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("CLI crate lives inside repository crates");
    let cache = OwnedTempDirectory::new("tachiko-open-sheet-native-ci-pycache");
    fs::write(cache.path().join("sentinel"), b"preserve-owned-cache")
        .expect("must seed the exclusively owned Python cache directory");
    let before = tree_snapshot(cache.path());

    let output = Command::new("python3")
        .arg(root.join("experiments/open-sheet-export/native_ci.py"))
        .env("TACHIKO_BIN", env!("CARGO_BIN_EXE_tachiko"))
        // Do not let an inherited setting make this regression pass if the
        // launcher-local bytecode suppression is accidentally removed.
        .env_remove("PYTHONDONTWRITEBYTECODE")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        // A removed suppression writes only into this test-owned directory,
        // never into a caller's checkout or cache.
        .env("PYTHONPYCACHEPREFIX", cache.path())
        .current_dir(root)
        .output()
        .expect("stdlib Python 3 must be available for the native acceptance harness");
    assert!(
        output.status.success(),
        "native open-sheet acceptance failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        tree_snapshot(cache.path()),
        before,
        "native open-sheet acceptance must not write Python bytecode"
    );
}
