use super::{FrontendError, VersionToken, inspect, inspect_roproj, inspect_top_level};

#[test]
fn records_a_root_unsigned_format_version() {
    let inspection = inspect(r#"{"format_version":1,"future":{"nested":true}}"#).unwrap();

    assert!(matches!(
        inspection.version,
        Some(VersionToken::Unsigned(ref version)) if version == "1"
    ));
}

#[test]
fn reports_duplicates_after_decoding_escapes_at_any_depth() {
    let error = inspect(r#"{"format_version":2,"future":{"a":1,"\u0061":2}}"#).unwrap_err();

    assert!(matches!(error, FrontendError::DuplicateMember(member) if member == "a"));
}

#[test]
fn top_level_inspection_ignores_nested_duplicates_only() {
    assert!(
        inspect_top_level(
            r#"{"format":"future","format_version":2,"future":{"a":1,"a":2}}"#,
            16,
        )
        .is_ok()
    );
    assert!(matches!(
        inspect_top_level(r#"{"format_version":2,"format_version":3}"#, 16),
        Err(FrontendError::DuplicateMember(member)) if member == "format_version"
    ));
    let inspection = inspect_top_level(
        r#"{"format":"future","format_version":18446744073709551616}"#,
        16,
    )
    .unwrap();
    assert_eq!(inspection.format.as_deref(), Some("future"));
    assert!(matches!(
        inspection.version,
        Some(VersionToken::Unsigned(ref version))
            if version == "18446744073709551616"
    ));
}

#[test]
fn invalid_json_wins_over_an_earlier_duplicate() {
    let error = inspect(r#"{"format_version":1,"format_version":1,"future":[}"#).unwrap_err();

    assert!(matches!(error, FrontendError::InvalidJson(_)));
}

#[test]
fn distinguishes_unsigned_versions_from_other_json_representations() {
    for source in [
        r#"{"format_version":"1"}"#,
        r#"{"format_version":1.0}"#,
        r#"{"format_version":1e0}"#,
        r#"{"format_version":-0}"#,
    ] {
        let inspection = inspect(source).unwrap();
        assert!(matches!(inspection.version, Some(VersionToken::Other)));
    }

    assert!(matches!(
        inspect(r#"{"format_version":0}"#).unwrap().version,
        Some(VersionToken::Unsigned(ref version)) if version == "0"
    ));
}

#[test]
fn ignores_versions_outside_the_root_object_and_accepts_only_one_value() {
    assert!(
        inspect(r#"[{"format_version":1}]"#)
            .unwrap()
            .version
            .is_none()
    );
    assert!(matches!(
        inspect(r#"{"format_version":1} trailing"#),
        Err(FrontendError::InvalidJson(_))
    ));
}

#[test]
fn roproj_inspection_bounds_nesting_but_ignores_string_contents() {
    assert!(inspect_roproj(r#"{"text":"[[{{\\\"}}]]"}"#, 1).is_ok());
    assert!(matches!(
        inspect_roproj(r#"{"nested":[{}]}"#, 2),
        Err(FrontendError::NestingLimit {
            limit: 2,
            actual: 3,
        })
    ));
}

#[test]
fn roproj_inspection_keeps_syntax_precedence_and_deep_escape_duplicates() {
    let malformed = r#"{"a":1,"a":2,"nested":[}"#;
    assert!(matches!(
        inspect_roproj(malformed, 132),
        Err(FrontendError::InvalidJson(_))
    ));

    let mut source = String::new();
    for _ in 0..64 {
        source.push_str("{\"nested\":");
    }
    source.push_str(r#"{"a":1,"\u0061":2}"#);
    source.push_str(&"}".repeat(64));
    assert!(matches!(
        inspect_roproj(&source, 65),
        Err(FrontendError::DuplicateMember(member)) if member == "a"
    ));
}
