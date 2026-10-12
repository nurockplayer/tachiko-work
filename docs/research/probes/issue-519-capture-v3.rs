//! Reproduction harness: copy to crates/storage/tests/issue_519_capture.rs,
//! run the targeted test with the shared target directory, then remove the copy.
use tachiko_semantic_core as semantic;
#[path = "../../../acceptance/portable-package-v3/document.rs"]
mod fixture;

#[test]
fn capture_accepted_v3_input() {
    let mut control = fixture::document();
    for schema in control.schemas.values_mut() {
        for field in schema.fields.values_mut() {
            field.constraint = semantic::FieldConstraint::None;
        }
    }
    for (directory, document) in [
        ("input.roproj", fixture::document()),
        ("runtime-control.roproj", control),
    ] {
        let tree = tachiko_storage::encode_roproj_v3(&document).unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/research/fixtures/issue-519-portable-package-v1-v3")
            .join(directory);
        std::fs::create_dir_all(root.join("entities")).unwrap();
        for file in tree.files() {
            std::fs::write(root.join(file.path()), file.bytes()).unwrap();
        }
        assert_eq!(tachiko_storage::decode_roproj_v3(&tree).unwrap(), document);
    }
}
