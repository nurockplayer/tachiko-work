use std::{path::Path, process::Command};

#[test]
fn open_sheet_handoff_acceptance() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("CLI crate lives inside repository crates");
    let output = Command::new("node")
        .arg("--test")
        .arg(root.join("experiments/open-sheet-handoff/acceptance.test.mjs"))
        .env("TACHIKO_BIN", env!("CARGO_BIN_EXE_tachiko"))
        .current_dir(root)
        .output()
        .expect("Node must be available for the public-package acceptance harness");
    assert!(
        output.status.success(),
        "open-sheet handoff acceptance failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
