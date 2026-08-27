use tachiko_semantic_core::Document;
use tachiko_storage::{ROPROJ_V1_PATHS, encode_roproj_v1};

const EXPECTED_EMPTY_MANIFEST: &[u8] = br#"{
  "format": "tachiko.roproj",
  "format_version": 1,
  "document": {
    "id": "doc-empty",
    "title": "Empty"
  }
}
"#;

#[test]
fn empty_document_emits_the_normative_eighteen_file_tree() {
    let tree = encode_roproj_v1(&Document::empty("doc-empty", "Empty")).unwrap();
    assert_eq!(tree.files().len(), 18);
    assert_eq!(tree.file("manifest.json").unwrap(), EXPECTED_EMPTY_MANIFEST);
    assert_eq!(tree.file("schemas.json").unwrap(), b"[]\n");
    for path in ROPROJ_V1_PATHS.iter().skip(2) {
        assert_eq!(tree.file(path).unwrap(), b"");
    }
}
