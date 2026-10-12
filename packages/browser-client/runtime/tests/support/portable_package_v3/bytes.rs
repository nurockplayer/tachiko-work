//! Checked-in independent bytes; this module never invokes the package encoder.
use super::FIXTURE_ROOT;
use std::{fs, path::PathBuf};

pub fn vector(name: &str) -> Vec<u8> {
    let hex = fs::read_to_string(PathBuf::from(FIXTURE_ROOT).join(format!("{name}.hex"))).unwrap();
    let hex = hex.strip_suffix('\n').unwrap();
    assert!(
        hex.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    assert_eq!(hex.len() % 2, 0);
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

pub fn input_files() -> Vec<(String, Vec<u8>)> {
    tree_files("input.roproj")
}
pub fn control_files() -> Vec<(String, Vec<u8>)> {
    tree_files("runtime-control.roproj")
}
fn tree_files(directory: &str) -> Vec<(String, Vec<u8>)> {
    let paths = ["manifest.json", "schemas.json", "definitions.json"]
        .into_iter()
        .map(str::to_owned)
        .chain((0..16).map(|n| format!("entities/{n:x}.jsonl")));
    paths
        .map(|p| {
            let bytes = fs::read(PathBuf::from(FIXTURE_ROOT).join(directory).join(&p)).unwrap();
            (p, bytes)
        })
        .collect()
}
