//! #522 Steward acceptance: current canonical `.roproj/v3` trees on the CLI.
//!
//! Expected contents come from the checked-in #519 fixture text, not from a
//! same-codec round trip.
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
            "issue-522-cli-{}-{}",
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
fn fixture_files() -> Vec<(String, Vec<u8>)> {
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
/// Every file and directory entry under `root`, with regular-file bytes.
fn snapshot(root: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    let mut entries = Vec::new();
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                entries.push((path.clone(), None));
                pending.push(path);
            } else {
                entries.push((path.clone(), Some(fs::read(&path).unwrap())));
            }
        }
    }
    entries.sort();
    entries
}
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
fn assert_success(output: &Output) -> String {
    assert!(output.status.success(), "{}", stderr(output));
    String::from_utf8_lossy(&output.stdout).into_owned()
}
fn assert_validates(tree: &Path) {
    let path = tree.to_str().unwrap();
    let before = snapshot(tree);
    for args in [vec!["validate", path], vec!["roproj", "validate", path]] {
        let stdout = assert_success(&run(&args));
        assert!(stdout.starts_with("valid "), "{stdout}");
        assert_eq!(snapshot(tree), before, "validation mutated {path}");
    }
}
/// Contents independently read from the fixture's schemas.json and shard text.
fn assert_fixture_contents(show: &str) {
    for expected in [
        "document id: import_00000000-0000-4000-8000-000000000001_0001",
        "1 schemas · 2 entities · 0 formulas",
        "(sheet_1_row_1) [import_00000000-0000-4000-8000-000000000001_0008]",
        "(sheet_1_row_2) [import_00000000-0000-4000-8000-000000000001_0009]",
        "column_1 [import_00000000-0000-4000-8000-000000000001_0003]: PEN",
        "column_2 [import_00000000-0000-4000-8000-000000000001_0004]: Paper",
        "column_3 [import_00000000-0000-4000-8000-000000000001_0005]: 5",
        "column_5 [import_00000000-0000-4000-8000-000000000001_0007]: 2026-09-28",
        "column_5 [import_00000000-0000-4000-8000-000000000001_0007]: 2026-09-27",
    ] {
        assert!(show.contains(expected), "missing {expected:?} in:\n{show}");
    }
}

#[test]
fn ac1_both_validate_entrypoints_accept_the_canonical_v3_fixture_tree() {
    let temp = Temp::new();
    let source = temp.0.join("source.roproj");
    write_tree(&source, &fixture_files());
    assert_validates(&source);
}

#[test]
fn ac2_cli_unpack_output_validates_and_reads_back_independent_contents() {
    let temp = Temp::new();
    let source = temp.0.join("source.roproj");
    let package = temp.0.join("package.ro");
    let unpacked = temp.0.join("unpacked.roproj");
    write_tree(&source, &fixture_files());
    assert_success(&run(&[
        "roproj",
        "pack",
        source.to_str().unwrap(),
        package.to_str().unwrap(),
    ]));
    assert_success(&run(&[
        "roproj",
        "unpack",
        package.to_str().unwrap(),
        unpacked.to_str().unwrap(),
    ]));
    assert_validates(&unpacked);
    let before = snapshot(&unpacked);
    let shown = assert_success(&run(&["show", unpacked.to_str().unwrap()]));
    assert_fixture_contents(&shown);
    assert_eq!(snapshot(&unpacked), before);
    let shown = assert_success(&run(&["show", source.to_str().unwrap()]));
    assert_fixture_contents(&shown);
}

#[test]
fn ac3_invalid_v3_trees_are_refused_without_mutation() {
    let base = fixture_files();
    let mut cases = Vec::new();
    let replace = |path: &str, bytes: Vec<u8>| {
        let mut files = base.clone();
        files.iter_mut().find(|(p, _)| p == path).unwrap().1 = bytes;
        files
    };
    let manifest = String::from_utf8(base[0].1.clone()).unwrap();
    cases.push((
        "unknown-version",
        replace(
            "manifest.json",
            manifest
                .replace("\"format_version\": 3", "\"format_version\": 4")
                .into_bytes(),
        ),
    ));
    cases.push((
        "malformed-manifest",
        replace(
            "manifest.json",
            b"{\"format\": \"tachiko.roproj\",".to_vec(),
        ),
    ));
    let schemas = String::from_utf8(base[1].1.clone()).unwrap();
    cases.push((
        "noncanonical-schemas",
        replace(
            "schemas.json",
            schemas.replace("\n  ", "\n    ").into_bytes(),
        ),
    ));
    cases.push((
        "missing-definitions",
        base.iter()
            .filter(|(p, _)| p != "definitions.json")
            .cloned()
            .collect(),
    ));
    let mut extra = base.clone();
    extra.push(("notes.txt".to_owned(), b"extra".to_vec()));
    cases.push(("extra-entry", extra));

    for (name, files) in cases {
        let temp = Temp::new();
        let tree = temp.0.join(format!("{name}.roproj"));
        write_tree(&tree, &files);
        let before = snapshot(&tree);
        let path = tree.to_str().unwrap();
        for args in [vec!["validate", path], vec!["roproj", "validate", path]] {
            let output = run(&args);
            assert!(!output.status.success(), "{name}: {args:?} was admitted");
            assert_eq!(snapshot(&tree), before, "{name}: refusal mutated source");
        }
    }
}

#[test]
fn ac4_existing_v1_trees_keep_validating_and_v2_is_not_silently_admitted() {
    let document = Document::empty("old-522", "Old source");
    let v1 = encode_roproj_v1(&document).unwrap();
    let v2 = encode_roproj_v2(&document).unwrap();
    for (version, files) in [
        (
            1,
            v1.files()
                .iter()
                .map(|f| (f.path().to_owned(), f.bytes().to_vec()))
                .collect::<Vec<_>>(),
        ),
        (
            2,
            v2.files()
                .iter()
                .map(|f| (f.path().to_owned(), f.bytes().to_vec()))
                .collect(),
        ),
    ] {
        let temp = Temp::new();
        let tree = temp.0.join(format!("v{version}.roproj"));
        write_tree(&tree, &files);
        if version == 1 {
            assert_validates(&tree);
            continue;
        }
        let before = snapshot(&tree);
        let path = tree.to_str().unwrap();
        for args in [vec!["validate", path], vec!["roproj", "validate", path]] {
            assert!(!run(&args).status.success(), "v2 tree was admitted");
            assert_eq!(snapshot(&tree), before);
        }
    }
}
