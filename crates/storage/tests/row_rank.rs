use std::{collections::BTreeMap, fmt::Write as _, fs, path::Path};

use sha2::{Digest, Sha256};
use tachiko_semantic_core as semantic;
use tachiko_semantic_core::{Document, Entity, RowRank, Schema};
use tachiko_storage::{
    CanonicalRoProjectV3, FormatError, decode_roproj_v3, encode_portable_package_v1,
    encode_roproj_v1, encode_roproj_v2, encode_roproj_v3, to_canonical_string,
};

#[path = "../../../acceptance/portable-package-v3/document.rs"]
mod fixture;

fn ranked_document() -> Document {
    let mut document = Document::empty("rank-doc", "Ranks");
    document.schemas.insert(
        "s".into(),
        Schema {
            id: "s".into(),
            key: "s".into(),
            display_name: None,
            field_order: None,
            fields: BTreeMap::new(),
        },
    );
    for (id, rank) in [("row-03", "z"), ("row-05", "A")] {
        document.entities.insert(
            id.into(),
            Entity {
                id: id.into(),
                key: id.into(),
                schema: "s".into(),
                rank: Some(RowRank::new(rank).unwrap()),
                fields: BTreeMap::new(),
            },
        );
    }
    document
}

fn replace_rank(tree: &CanonicalRoProjectV3, rank: &str) -> Vec<(String, Vec<u8>)> {
    tree.files()
        .iter()
        .map(|file| {
            let content = if file.path() == "entities/a.jsonl" {
                String::from_utf8(file.bytes().to_vec())
                    .unwrap()
                    .replacen("\"rank\":\"z\"", rank, 1)
                    .into_bytes()
            } else {
                file.bytes().to_vec()
            };
            (file.path().to_owned(), content)
        })
        .collect()
}

#[test]
fn rank_v3_canonical_bytes_and_strict_reader() {
    let document = ranked_document();
    let tree = encode_roproj_v3(&document).unwrap();
    assert_eq!(
        tree.file("entities/a.jsonl").unwrap(),
        include_bytes!("fixtures/row-rank/a.jsonl")
    );
    assert_eq!(decode_roproj_v3(&tree).unwrap(), document);
    let files = tree
        .files()
        .iter()
        .map(|f| (f.path().to_owned(), f.bytes().to_vec()))
        .collect();
    assert_eq!(CanonicalRoProjectV3::try_from_files(files).unwrap(), tree);
    for invalid in [
        "\"rank\":null".to_owned(),
        "\"rank\":\"\"".to_owned(),
        "\"rank\":\"A0\"".to_owned(),
        "\"rank\":\"a-b\"".to_owned(),
        "\"rank\":\"é\"".to_owned(),
        format!("\"rank\":\"{}\"", "A".repeat(65)),
        "\"rank\":\"z\",\"rank\":\"A\"".to_owned(),
        "\"rank\":\"z\",\"unknown\":1".to_owned(),
        "\"rank\":42".to_owned(),
    ] {
        assert!(
            CanonicalRoProjectV3::try_from_files(replace_rank(&tree, &invalid)).is_err(),
            "{invalid}"
        );
    }
    let mut longest = document;
    longest.entities.get_mut("row-03").unwrap().rank = Some(RowRank::new("A".repeat(64)).unwrap());
    let tree = encode_roproj_v3(&longest).unwrap();
    assert_eq!(decode_roproj_v3(&tree).unwrap(), longest);
}

#[test]
fn unranked_v3_and_seed_bytes_unchanged() {
    let document = fixture::document();
    assert!(!document.has_row_ranks());
    let tree = encode_roproj_v3(&document).unwrap();
    let files: Vec<_> = tree
        .files()
        .iter()
        .map(|f| (f.path().to_owned(), f.bytes().to_vec()))
        .collect();
    let golden_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/research/fixtures/issue-519-portable-package-v1-v3");
    for (path, content) in files {
        assert_eq!(
            content,
            fs::read(golden_root.join("input.roproj").join(path)).unwrap()
        );
    }
    let hex = fs::read_to_string(golden_root.join("golden.hex")).unwrap();
    let golden: Vec<u8> = hex
        .trim_end()
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    assert_eq!(encode_portable_package_v1(&tree).unwrap(), golden);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest =
        fs::read_to_string(root.join("acceptance/date-definition-save-v3/SEED.SHA256SUMS"))
            .unwrap();
    for line in manifest.lines() {
        let (hash, path) = line.split_once("  ").unwrap();
        assert_eq!(
            hex(&Sha256::digest(fs::read(root.join(path)).unwrap())),
            hash,
            "{path}"
        );
    }
}

#[test]
fn ranked_documents_refuse_frozen_writers_and_v1_history() {
    let document = ranked_document();
    assert!(matches!(
        to_canonical_string(&document),
        Err(FormatError::UnsupportedRowRank)
    ));
    assert!(matches!(
        encode_roproj_v1(&document),
        Err(FormatError::UnsupportedRowRank)
    ));
    assert!(matches!(
        encode_roproj_v2(&document),
        Err(FormatError::UnsupportedRowRank)
    ));
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").unwrap();
        output
    })
}
