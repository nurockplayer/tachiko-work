use std::{path::PathBuf, process::Command};

#[test]
fn static_named_package_reopens_in_fresh_native_processes() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../acceptance/native-display-names/named.ro");
    for command in ["validate", "calculate"] {
        let output = Command::new(env!("CARGO_BIN_EXE_tachiko"))
            .arg(command)
            .arg(&fixture)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{command}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let tree = fixture.parent().unwrap().join("tree");
    let output = Command::new(env!("CARGO_BIN_EXE_tachiko"))
        .arg("validate")
        .arg(tree)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
