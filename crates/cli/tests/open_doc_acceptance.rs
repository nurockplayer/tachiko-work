use std::{path::Path, process::Command};

#[test]
fn open_doc_research_projection_acceptance() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root must exist");
    let output = Command::new("node")
        .current_dir(repository)
        .arg("--test")
        .arg("docs/research/probes/open-doc/acceptance.test.mjs")
        .env("TACHIKO_BIN", env!("CARGO_BIN_EXE_tachiko"))
        .output()
        .expect("Node.js is required for the research acceptance harness");
    assert!(
        output.status.success(),
        "open-doc acceptance failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
