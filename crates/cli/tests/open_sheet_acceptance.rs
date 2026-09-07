use std::{path::Path, process::Command};

#[test]
fn open_sheet_export_acceptance() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("CLI crate lives inside repository crates");
    let output = Command::new("python3")
        .arg(root.join("experiments/open-sheet-export/native_ci.py"))
        .env("TACHIKO_BIN", env!("CARGO_BIN_EXE_tachiko"))
        .current_dir(root)
        .output()
        .expect("stdlib Python 3 must be available for the native acceptance harness");
    assert!(
        output.status.success(),
        "native open-sheet acceptance failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
