use tachiko_storage::{FORMAT_VERSION, FormatError, canonicalize_legacy_v1, from_str};

#[test]
fn canonical_minimal_v1_has_exact_frozen_bytes() {
    let source = r#"{"format_version":1,"id":"doc","title":"Document","schemas":{},"entities":{}}"#;
    let expected = "{\n  \"format_version\": 1,\n  \"id\": \"doc\",\n  \"title\": \"Document\",\n  \"schemas\": {},\n  \"entities\": {}\n}\n";

    assert_eq!(canonicalize_legacy_v1(source.as_bytes()).unwrap(), expected);
}

#[test]
fn canonical_v1_helper_reports_the_current_build_version_ceiling() {
    let v2 = r#"{"format_version":2,"id":"doc","title":"Document","schemas":{},"entities":{}}"#;

    assert!(matches!(
        canonicalize_legacy_v1(v2.as_bytes()).unwrap_err(),
        FormatError::UnsupportedVersion { found: 2, supported }
            if supported == FORMAT_VERSION
    ));
}

#[test]
fn every_v1_field_value_and_expression_discriminator_remains_frozen() {
    let fixture = include_str!("fixtures/all-v1-shapes.ro");
    let encoded = canonicalize_legacy_v1(fixture.as_bytes()).unwrap();

    assert_eq!(encoded, fixture);
    for (member, discriminator) in [
        ("type", "number"),
        ("type", "text"),
        ("type", "boolean"),
        ("type", "reference"),
        ("kind", "number"),
        ("kind", "text"),
        ("kind", "boolean"),
        ("kind", "reference"),
        ("kind", "formula"),
        ("op", "number"),
        ("op", "reference"),
        ("op", "add"),
        ("op", "subtract"),
        ("op", "multiply"),
        ("op", "divide"),
        ("op", "minimum"),
        ("op", "maximum"),
    ] {
        assert!(
            encoded.contains(&format!("\"{member}\": \"{discriminator}\"")),
            "missing {member} discriminator {discriminator}"
        );
    }
}

#[test]
fn canonical_v1_preserves_unicode_scalar_sequences_without_normalization() {
    let fixture = include_str!("fixtures/all-v1-shapes.ro");
    let encoded = canonicalize_legacy_v1(fixture.as_bytes()).unwrap();
    let migrated = from_str(&encoded).unwrap();

    assert!(encoded.contains("Café | Cafe\u{301}"));
    assert!(encoded.contains("日本語 😀"));
    assert_eq!(migrated.title, "Café | Cafe\u{301}");
    assert_ne!("é".as_bytes(), "e\u{301}".as_bytes());
}

#[test]
fn noncanonical_json_reencodes_to_the_unique_v1_layout() {
    let noncanonical = "{\r\n\t\"entities\": {}, \"schemas\": {},\r\n\"title\":\"\\u65e5\",\"id\":\"doc\",\"format_version\":1}\r\n";
    let expected = "{\n  \"format_version\": 1,\n  \"id\": \"doc\",\n  \"title\": \"日\",\n  \"schemas\": {},\n  \"entities\": {}\n}\n";

    let encoded = canonicalize_legacy_v1(noncanonical.as_bytes()).unwrap();

    assert_eq!(encoded, expected);
    assert!(!encoded.starts_with('\u{feff}'));
    assert!(!encoded.contains('\r'));
    assert!(encoded.lines().all(|line| !line.ends_with([' ', '\t'])));
    assert!(encoded.ends_with('\n'));
    assert!(!encoded.ends_with("\n\n"));
}

#[test]
fn alternative_legal_string_escapes_have_one_v1_spelling() {
    let escaped =
        r#"{"format_version":1,"id":"doc","title":"\u65e5\/work","schemas":{},"entities":{}}"#;
    let literal = r#"{"format_version":1,"id":"doc","title":"日/work","schemas":{},"entities":{}}"#;

    let escaped_output = canonicalize_legacy_v1(escaped.as_bytes()).unwrap();
    let literal_output = canonicalize_legacy_v1(literal.as_bytes()).unwrap();

    assert_eq!(escaped_output, literal_output);
    assert!(literal_output.contains("日/work"));
    assert!(!literal_output.contains("\\/"));
}

#[test]
fn every_legacy_id_map_uses_ascii_lexicographic_order() {
    let source = r#"{
      "entities": {
        "z-entity": {"fields":{"z-field":{"kind":"text","value":"z"},"a-field":{"kind":"text","value":"a"}},"schema":"z-schema","id":"z-entity"},
        "a-entity": {"fields":{},"schema":"a-schema","id":"a-entity"}
      },
      "schemas": {
        "z-schema": {"fields":{"z-field":{"required":false,"field_type":{"type":"text"}},"a-field":{"required":false,"field_type":{"type":"text"}}},"id":"z-schema"},
        "a-schema": {"fields":{},"id":"a-schema"}
      },
      "title":"Ordering","id":"doc","format_version":1
    }"#;
    let encoded = canonicalize_legacy_v1(source.as_bytes()).unwrap();
    let schemas = &encoded
        [encoded.find("  \"schemas\": {").unwrap()..encoded.find("  \"entities\": {").unwrap()];
    let entities = &encoded[encoded.find("  \"entities\": {").unwrap()..];

    assert!(
        schemas.find("    \"a-schema\": {").unwrap() < schemas.find("    \"z-schema\": {").unwrap()
    );
    assert!(
        schemas.find("        \"a-field\": {").unwrap()
            < schemas.find("        \"z-field\": {").unwrap()
    );
    assert!(
        entities.find("    \"a-entity\": {").unwrap()
            < entities.find("    \"z-entity\": {").unwrap()
    );
    assert!(
        entities.find("        \"a-field\": {").unwrap()
            < entities.find("        \"z-field\": {").unwrap()
    );
}

#[test]
fn checked_in_legacy_examples_remain_canonical_and_byte_stable() {
    for fixture in [
        include_str!("../../../examples/game-balance/game-balance.ro"),
        include_str!("../../../examples/game-balance/buffed-sword.ro"),
    ] {
        assert_eq!(canonicalize_legacy_v1(fixture.as_bytes()).unwrap(), fixture);
    }
}
