#[test]
fn translate_a1_import_keeps_original_input_length_bound() {
    let source = format!("=1{}", " ".repeat(super::MAX_PROFILE_STRING_BYTES - 1));
    let metadata = super::InteropMetadata {
        version: 1,
        sheets: Vec::new(),
    };
    let document = tachiko_workspace_engine::Document::empty("bound", "Bound");
    let owner = tachiko_workspace_engine::FieldRef::new("entity", "field");
    assert!(super::translate_a1(&source, &metadata, &[], &[], 0, &owner, &document).is_err());
    assert!(super::translate_a1(&source[1..], &metadata, &[], &[], 0, &owner, &document).is_ok());
}
