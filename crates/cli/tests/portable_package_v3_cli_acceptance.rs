//! #519 public CLI acceptance, including the explicit migration remedy.
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
use tachiko_storage::{encode_roproj_v1, encode_roproj_v2};
use tachiko_workspace_engine::Document;
const FIXTURE_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/research/fixtures/issue-519-portable-package-v1-v3"
);
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "issue-519-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tachiko"))
        .args(args)
        .output()
        .unwrap()
}
fn files() -> Vec<(String, Vec<u8>)> {
    ["manifest.json", "schemas.json", "definitions.json"]
        .into_iter()
        .map(str::to_owned)
        .chain((0..16).map(|n| format!("entities/{n:x}.jsonl")))
        .map(|p| {
            let b = fs::read(Path::new(FIXTURE_ROOT).join("input.roproj").join(&p)).unwrap();
            (p, b)
        })
        .collect()
}
fn write_tree(root: &Path, files: &[(String, Vec<u8>)]) {
    fs::create_dir_all(root.join("entities")).unwrap();
    for (p, b) in files {
        fs::write(root.join(p), b).unwrap();
    }
}
fn assert_tree(root: &Path, files: &[(String, Vec<u8>)]) {
    for (p, b) in files {
        assert_eq!(&fs::read(root.join(p)).unwrap(), b);
    }
}
fn golden() -> Vec<u8> {
    let hex = fs::read_to_string(Path::new(FIXTURE_ROOT).join("golden.hex")).unwrap();
    hex.trim_end()
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
#[test]
fn ac1_cli_pack_unpack_compare_match_the_independent_golden() {
    let temp = Temp::new();
    let source = temp.0.join("source.roproj");
    let package = temp.0.join("package.ro");
    let restored = temp.0.join("restored.roproj");
    let repacked = temp.0.join("repacked.ro");
    let files = files();
    write_tree(&source, &files);
    let packed = run(&[
        "roproj",
        "pack",
        source.to_str().unwrap(),
        package.to_str().unwrap(),
    ]);
    assert_tree(&source, &files);
    assert!(
        packed.status.success(),
        "{}",
        String::from_utf8_lossy(&packed.stderr)
    );
    assert_eq!(fs::read(&package).unwrap(), golden());
    let unpacked = run(&[
        "roproj",
        "unpack",
        package.to_str().unwrap(),
        restored.to_str().unwrap(),
    ]);
    assert!(
        unpacked.status.success(),
        "{}",
        String::from_utf8_lossy(&unpacked.stderr)
    );
    assert_tree(&restored, &files);
    let packed = run(&[
        "roproj",
        "pack",
        restored.to_str().unwrap(),
        repacked.to_str().unwrap(),
    ]);
    assert!(
        packed.status.success(),
        "{}",
        String::from_utf8_lossy(&packed.stderr)
    );
    assert_eq!(fs::read(repacked).unwrap(), golden());
    let compared = run(&[
        "roproj",
        "compare-package",
        package.to_str().unwrap(),
        source.to_str().unwrap(),
    ]);
    assert!(
        compared.status.success(),
        "{}",
        String::from_utf8_lossy(&compared.stderr)
    );
    assert!(String::from_utf8_lossy(&compared.stdout).contains("consistent"));
    assert_tree(&source, &files);
}
#[test]
fn ac3_cli_v1_source_refusal_names_explicit_migrate_path() {
    old_source(1);
}
#[test]
fn ac3_cli_v2_source_refusal_names_explicit_migrate_path() {
    old_source(2);
}
fn old_source(version: u32) {
    let temp = Temp::new();
    let document = Document::empty("old-519", "Old source");
    let files = match version {
        1 => encode_roproj_v1(&document)
            .unwrap()
            .files()
            .iter()
            .map(|f| (f.path().to_owned(), f.bytes().to_vec()))
            .collect::<Vec<_>>(),
        2 => encode_roproj_v2(&document)
            .unwrap()
            .files()
            .iter()
            .map(|f| (f.path().to_owned(), f.bytes().to_vec()))
            .collect(),
        _ => unreachable!(),
    };
    let source = temp.0.join("old.roproj");
    let destination = temp.0.join("refused.ro");
    write_tree(&source, &files);
    let result = run(&[
        "roproj",
        "pack",
        source.to_str().unwrap(),
        destination.to_str().unwrap(),
    ]);
    assert_tree(&source, &files);
    assert!(
        !result.status.success(),
        "old source was implicitly packaged"
    );
    assert!(!destination.exists());
    let diagnostic = String::from_utf8_lossy(&result.stderr);
    assert!(
        diagnostic.contains("portable_package.unsupported_payload_representation"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("migrate"),
        "refusal must name the existing explicit migration path: {diagnostic}"
    );
}
