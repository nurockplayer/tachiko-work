use sha2::{Digest, Sha256};
use std::{fmt::Write as _, fs, path::PathBuf};
use tachiko_storage::*;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").unwrap();
        output
    })
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../acceptance/native-display-names")
}
fn files() -> Vec<(String, Vec<u8>)> {
    ROPROJ_V3_PATHS
        .into_iter()
        .map(|path| {
            (
                path.to_owned(),
                fs::read(root().join("tree").join(path)).unwrap(),
            )
        })
        .collect()
}

#[test]
fn independent_named_tree_package_hashes_crcs_and_repack() {
    let expected = fs::read(root().join("named.ro")).unwrap();
    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(root().join("expected.json")).unwrap()).unwrap();
    let tree = CanonicalRoProjectV3::try_from_files(files()).unwrap();
    let document = decode_roproj_v3(&tree).unwrap();
    assert_eq!(encode_roproj_v3(&document).unwrap(), tree);
    assert_eq!(encode_portable_package_v1(&tree).unwrap(), expected);
    let verified = decode_portable_package_v1(&expected).unwrap();
    assert_eq!(verified.tree(), &tree);
    assert_eq!(
        encode_portable_package_v1(verified.tree()).unwrap(),
        expected
    );
    assert_eq!(receipt["length"].as_u64().unwrap(), expected.len() as u64);
    assert_eq!(
        hex(&Sha256::digest(&expected)),
        receipt["sha256"].as_str().unwrap()
    );
    let root_hex = hex(&verified.payload_root());
    assert_eq!(root_hex, receipt["payload_root_sha256"].as_str().unwrap());
    let mut cursor = 0;
    for _ in 0..20 {
        assert_eq!(&expected[cursor..cursor + 4], b"PK\x03\x04");
        let crc = u32::from_le_bytes(expected[cursor + 14..cursor + 18].try_into().unwrap());
        let size =
            u32::from_le_bytes(expected[cursor + 18..cursor + 22].try_into().unwrap()) as usize;
        let name_len =
            u16::from_le_bytes(expected[cursor + 26..cursor + 28].try_into().unwrap()) as usize;
        let name = std::str::from_utf8(&expected[cursor + 30..cursor + 30 + name_len]).unwrap();
        assert_eq!(
            format!("{crc:08x}"),
            receipt["crcs"][name].as_str().unwrap()
        );
        cursor += 30 + name_len + size;
    }
    assert_eq!(from_bytes(&expected).unwrap(), document);
}

#[test]
fn v3_optional_members_are_closed_nonnull_and_already_trimmed() {
    for invalid in [
        "null",
        "42",
        "true",
        "[]",
        "{}",
        "\"\"",
        "\" Item \"",
        "\"a\\u0000b\"",
        "\"a\\u0085b\"",
        "\"a\\u2028b\"",
        "\"a\\u2029b\"",
    ] {
        for member in ["My Expenses", "Item Name"] {
            let mut files = files();
            let schemas = String::from_utf8(files[1].1.clone()).unwrap();
            files[1].1 = schemas
                .replace(
                    &format!("\"display_name\": \"{member}\""),
                    &format!("\"display_name\": {invalid}"),
                )
                .into_bytes();
            assert!(
                CanonicalRoProjectV3::try_from_files(files).is_err(),
                "{member} {invalid}"
            );
        }
    }
    for member in ["My Expenses", "Item Name"] {
        let mut files = files();
        let schemas = String::from_utf8(files[1].1.clone()).unwrap();
        files[1].1 = schemas
            .replace(
                &format!("\"display_name\": \"{member}\""),
                "\"label\": \"unknown\"",
            )
            .into_bytes();
        assert!(CanonicalRoProjectV3::try_from_files(files).is_err());
    }
}

#[test]
fn old_target_writers_refuse_names_without_creating_an_output() {
    let document = from_bytes(&fs::read(root().join("named.ro")).unwrap()).unwrap();
    assert!(matches!(
        to_canonical_string(&document),
        Err(FormatError::UnsupportedDisplayName)
    ));
    assert!(matches!(
        encode_roproj_v1(&document),
        Err(FormatError::UnsupportedDisplayName)
    ));
    assert!(matches!(
        encode_roproj_v2(&document),
        Err(FormatError::UnsupportedDisplayName)
    ));
    let output = std::env::temp_dir().join(format!("work529-refused-{}.ro", std::process::id()));
    assert!(!output.exists());
    assert!(matches!(
        save(&output, &document),
        Err(FormatError::UnsupportedDisplayName)
    ));
    assert!(!output.exists());
}

#[test]
fn name_uniqueness_is_admitted_even_without_entities() {
    let mut files = files();
    let schemas = String::from_utf8(files[1].1.clone()).unwrap();
    files[1].1 = schemas
        .replace(
            "\"display_name\": \"Item Name\"",
            "\"display_name\": \"項目\"",
        )
        .into_bytes();
    assert!(CanonicalRoProjectV3::try_from_files(files).is_err());
}
