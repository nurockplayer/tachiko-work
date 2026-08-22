use tachiko_storage::{FormatError, from_bytes, from_str};

const MINIMAL_V1: &str = r#"{
  "format_version": 1,
  "id": "doc",
  "title": "Document",
  "schemas": {},
  "entities": {}
}"#;

#[test]
fn invalid_utf8_is_distinct_from_invalid_json() {
    let error = from_bytes(b"{\"format_version\":1,\"title\":\xff}").unwrap_err();

    assert!(matches!(error, FormatError::InvalidUtf8 { .. }));
}

#[test]
fn invalid_json_is_reported_before_version_dispatch() {
    let error = from_bytes(br#"{"format_version": 2, "future": [}"#).unwrap_err();

    assert!(matches!(error, FormatError::InvalidJson { .. }));
}

#[test]
fn duplicate_members_are_rejected_at_every_depth_after_escape_decoding() {
    let cases = [
        (
            "top-level",
            r#"{"format_version":1,"format_version":1,"id":"doc","title":"Document","schemas":{},"entities":{}}"#,
        ),
        (
            "nested",
            r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"kind":{"id":"kind","id":"kind","fields":{}}},"entities":{}}"#,
        ),
        (
            "escaped-equivalent",
            r#"{"format_version":1,"id":"doc","title":"Document","schemas":{},"entities":{},"\u0069d":"doc"}"#,
        ),
        (
            "unsupported-body",
            r#"{"format_version":2,"future":{"a":1,"\u0061":2}}"#,
        ),
    ];

    for (name, source) in cases {
        let error = from_str(source).unwrap_err();
        assert!(
            matches!(error, FormatError::DuplicateMember { .. }),
            "{name}: {error:?}"
        );
    }
}

#[test]
fn missing_version_is_distinct() {
    for source in [
        r#"{"id":"doc","title":"Document","schemas":{},"entities":{}}"#,
        "[]",
    ] {
        let error = from_str(source).unwrap_err();
        assert!(matches!(error, FormatError::VersionMissing), "{error:?}");
    }
}

#[test]
fn malformed_versions_are_distinct_from_missing_and_unsupported_versions() {
    for version in [
        r#""1""#,
        "1.0",
        "1e0",
        "0",
        "-1",
        "4294967296",
        "1844674407370955161600000000000000000000000",
        "null",
        "true",
    ] {
        let source = format!(r#"{{"format_version":{version},"future_only":true}}"#);
        let error = from_str(&source).unwrap_err();
        assert!(
            matches!(error, FormatError::VersionMalformed),
            "{version}: {error:?}"
        );
    }

    let huge_version = format!("1{}", "0".repeat(400));
    let source = format!(r#"{{"format_version":{huge_version},"future_only":true}}"#);
    let error = from_str(&source).unwrap_err();
    assert!(
        matches!(error, FormatError::VersionMalformed),
        "huge integer: {error:?}"
    );
}

#[test]
fn unsupported_version_wins_before_v1_body_interpretation() {
    let error =
        from_str(r#"{"format_version":2,"future_only":{"unknown_v2_member":true}}"#).unwrap_err();

    assert!(matches!(
        error,
        FormatError::UnsupportedVersion {
            found: 2,
            supported
        } if supported == 1
    ));
}

#[test]
fn unsupported_version_does_not_apply_v1_number_limits_to_the_future_body() {
    let huge_number = format!("1{}", "0".repeat(400));
    let source = format!(r#"{{"format_version":2,"future_only":{huge_number}}}"#);

    let error = from_str(&source).unwrap_err();

    assert!(matches!(
        error,
        FormatError::UnsupportedVersion { found: 2, .. }
    ));
}

#[test]
fn malformed_version_wins_before_v1_body_interpretation() {
    let error = from_str(r#"{"format_version":"1","future_only":true}"#).unwrap_err();

    assert!(matches!(error, FormatError::VersionMalformed));
}

#[test]
fn supported_v1_rejects_unknown_members_recursively() {
    let cases = [
        (
            "document",
            r#"{"format_version":1,"id":"doc","title":"Document","schemas":{},"entities":{},"extra":true}"#,
        ),
        (
            "schema",
            r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{},"extra":true}},"entities":{}}"#,
        ),
        (
            "field-definition",
            r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{"n":{"field_type":{"type":"number"},"required":false,"extra":true}}}},"entities":{}}"#,
        ),
        (
            "field-type",
            r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{"n":{"field_type":{"type":"number","extra":true},"required":false}}}},"entities":{}}"#,
        ),
        (
            "entity",
            r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{}}},"entities":{"e":{"id":"e","schema":"s","fields":{},"extra":true}}}"#,
        ),
        (
            "value",
            r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{"n":{"field_type":{"type":"number"},"required":true}}}},"entities":{"e":{"id":"e","schema":"s","fields":{"n":{"kind":"number","value":1,"extra":true}}}}}"#,
        ),
        (
            "expression",
            r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{"n":{"field_type":{"type":"number"},"required":true}}}},"entities":{"e":{"id":"e","schema":"s","fields":{"n":{"kind":"formula","value":{"op":"number","args":1,"extra":true}}}}}}"#,
        ),
        (
            "field-reference",
            r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{"n":{"field_type":{"type":"number"},"required":true}}}},"entities":{"e":{"id":"e","schema":"s","fields":{"n":{"kind":"formula","value":{"op":"reference","args":{"entity":"e","field":"n","extra":true}}}}}}}"#,
        ),
        (
            "binary-expression-arguments",
            r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{"n":{"field_type":{"type":"number"},"required":true}}}},"entities":{"e":{"id":"e","schema":"s","fields":{"n":{"kind":"formula","value":{"op":"add","args":{"left":{"op":"number","args":1},"right":{"op":"number","args":2},"extra":true}}}}}}}"#,
        ),
    ];

    for (name, source) in cases {
        let error = from_str(source).unwrap_err();
        assert!(
            matches!(error, FormatError::InvalidRepresentation { .. }),
            "{name}: {error:?}"
        );
    }
}

#[test]
fn v1_basic_field_types_reject_a_present_null_schema_member() {
    for field_type in ["number", "text", "boolean"] {
        let source = format!(
            r#"{{"format_version":1,"id":"doc","title":"Document","schemas":{{"s":{{"id":"s","fields":{{"f":{{"field_type":{{"type":"{field_type}","schema":null}},"required":false}}}}}}}},"entities":{{}}}}"#
        );

        let error = from_str(&source).unwrap_err();
        assert!(
            matches!(error, FormatError::InvalidRepresentation { .. }),
            "{field_type}: {error:?}"
        );
    }
}

#[test]
fn v1_rejects_schema_and_entity_map_key_id_mismatches() {
    let cases = [
        r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"map-key":{"id":"nested-id","fields":{}}},"entities":{}}"#,
        r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{}}},"entities":{"map-key":{"id":"nested-id","schema":"s","fields":{}}}}"#,
    ];

    for source in cases {
        let error = from_str(source).unwrap_err();
        assert!(matches!(error, FormatError::InvalidRepresentation { .. }));
    }
}

#[test]
fn v1_rejects_unresolvable_schema_and_field_relationships() {
    let cases = [
        r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{"target":{"field_type":{"type":"reference","schema":"missing"},"required":false}}}},"entities":{}}"#,
        r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{}}},"entities":{"e":{"id":"e","schema":"missing","fields":{}}}}"#,
        r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{}}},"entities":{"e":{"id":"e","schema":"s","fields":{"missing":{"kind":"number","value":1}}}}}"#,
    ];

    for source in cases {
        let error = from_str(source).unwrap_err();
        assert!(matches!(error, FormatError::InvalidRepresentation { .. }));
    }
}

#[test]
fn semantically_invalid_v1_is_distinct_from_invalid_representation() {
    let source = r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{"target":{"field_type":{"type":"reference","schema":"s"},"required":true}}}},"entities":{"e":{"id":"e","schema":"s","fields":{"target":{"kind":"reference","value":"missing"}}}}}"#;

    let error = from_str(source).unwrap_err();

    assert!(matches!(error, FormatError::InvalidDocument { .. }));
}

#[test]
fn supported_v1_number_outside_historical_f64_is_invalid_representation() {
    let huge_number = format!("1{}", "0".repeat(400));
    let source = r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{"n":{"field_type":{"type":"number"},"required":true}}}},"entities":{"e":{"id":"e","schema":"s","fields":{"n":{"kind":"number","value":HUGE_NUMBER}}}}}"#
        .replace("HUGE_NUMBER", &huge_number);

    let error = from_str(&source).unwrap_err();

    assert!(matches!(error, FormatError::InvalidRepresentation { .. }));
}

#[test]
fn valid_minimal_v1_still_decodes() {
    let document = from_str(MINIMAL_V1).unwrap();

    assert_eq!(document.id.as_str(), "doc");
}
