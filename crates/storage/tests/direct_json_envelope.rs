use std::{fmt::Write as _, panic::catch_unwind};

use tachiko_storage::{
    FormatError, NORMAL_DIRECT_JSON_MAX_INPUT_BYTES, V2_MAX_NUMBER_TOKEN_BYTES,
    canonicalize_legacy_v1, from_bytes, from_str,
};

fn padded(prefix: &str, suffix: &str, target: usize) -> String {
    let filler = target
        .checked_sub(prefix.len() + suffix.len())
        .expect("target must fit the JSON framing");
    let mut input = String::with_capacity(target);
    input.push_str(prefix);
    input.extend(std::iter::repeat_n('x', filler));
    input.push_str(suffix);
    assert_eq!(input.len(), target);
    input
}

fn valid_v1(target: usize) -> String {
    padded(
        r#"{"format_version":1,"id":"doc","title":""#,
        r#"","schemas":{},"entities":{}}"#,
        target,
    )
}

fn valid_v2(target: usize) -> String {
    padded(
        r#"{"format_version":2,"id":"doc","title":""#,
        r#"","schemas":{},"entities":{}}"#,
        target,
    )
}

fn malformed_json(target: usize) -> String {
    padded(r#"{"format_version":2,"future":""#, "", target)
}

fn duplicate_member(target: usize) -> String {
    padded(
        r#"{"format_version":3,"padding":""#,
        r#"","future":{"a":1,"\u0061":2}}"#,
        target,
    )
}

fn duplicate_then_malformed(target: usize) -> String {
    padded(
        r#"{"format_version":3,"a":1,"\u0061":2,"future":""#,
        "",
        target,
    )
}

fn missing_version(target: usize) -> String {
    padded(r#"{"future":""#, r#""}"#, target)
}

fn malformed_version(target: usize) -> String {
    padded(r#"{"format_version":"2","future":""#, r#""}"#, target)
}

fn unsupported_version(target: usize) -> String {
    padded(r#"{"format_version":3,"future":""#, r#""}"#, target)
}

fn invalid_utf8(target: usize) -> Vec<u8> {
    let mut input = vec![b' '; target];
    input[target - 1] = 0xff;
    input
}

fn many_members(count: usize) -> String {
    let mut input = String::from(r#"{"format_version":3,"future":{"#);
    for index in 0..count {
        if index != 0 {
            input.push(',');
        }
        write!(input, r#""k{index:016x}":0"#).unwrap();
    }
    input.push_str("}}");
    input
}

fn assert_input_limit(error: &FormatError, actual: usize) {
    assert!(matches!(
        error,
        FormatError::ResourceLimit {
            resource: "input",
            limit: NORMAL_DIRECT_JSON_MAX_INPUT_BYTES,
            actual: found,
        } if *found == actual
    ));
}

#[test]
fn normal_profile_admits_exact_boundary_for_v1_and_v2() {
    for source in [
        valid_v1(NORMAL_DIRECT_JSON_MAX_INPUT_BYTES),
        valid_v2(NORMAL_DIRECT_JSON_MAX_INPUT_BYTES),
    ] {
        assert_eq!(source.len(), NORMAL_DIRECT_JSON_MAX_INPUT_BYTES);
        assert!(from_bytes(source.as_bytes()).is_ok());
    }
}

#[test]
fn normal_profile_rejects_valid_v1_and_v2_one_byte_over() {
    let oversized = NORMAL_DIRECT_JSON_MAX_INPUT_BYTES + 1;
    for source in [valid_v1(oversized), valid_v2(oversized)] {
        assert_input_limit(&from_bytes(source.as_bytes()).unwrap_err(), oversized);
    }
}

#[test]
fn oversized_resource_limit_precedes_every_latent_format_error() {
    let oversized = NORMAL_DIRECT_JSON_MAX_INPUT_BYTES + 1;
    let cases = [
        ("invalid UTF-8", invalid_utf8(oversized)),
        ("invalid JSON", malformed_json(oversized).into_bytes()),
        (
            "recursive escaped-equivalent duplicate",
            duplicate_member(oversized).into_bytes(),
        ),
        (
            "duplicate followed by invalid JSON",
            duplicate_then_malformed(oversized).into_bytes(),
        ),
        ("missing version", missing_version(oversized).into_bytes()),
        (
            "malformed version",
            malformed_version(oversized).into_bytes(),
        ),
        (
            "unsupported version",
            unsupported_version(oversized).into_bytes(),
        ),
    ];

    for (name, source) in cases {
        let error = from_bytes(&source).unwrap_err();
        assert!(
            matches!(
                error,
                FormatError::ResourceLimit {
                    resource: "input",
                    limit: NORMAL_DIRECT_JSON_MAX_INPUT_BYTES,
                    actual,
                } if actual == oversized
            ),
            "{name}: {error:?}"
        );
    }
}

#[test]
fn admitted_input_keeps_the_existing_strict_precedence() {
    assert!(matches!(
        from_bytes(b"{\"format_version\":1,\"title\":\xff}").unwrap_err(),
        FormatError::InvalidUtf8 { .. }
    ));
    assert!(matches!(
        from_str(r#"{"format_version":3,"a":1,"\u0061":2,"future":[}"#).unwrap_err(),
        FormatError::InvalidJson { .. }
    ));
    assert!(matches!(
        from_str(r#"{"format_version":3,"future":{"a":1,"\u0061":2}}"#).unwrap_err(),
        FormatError::DuplicateMember { .. }
    ));
    assert!(matches!(
        from_str(r#"{"future":true}"#).unwrap_err(),
        FormatError::VersionMissing
    ));
    assert!(matches!(
        from_str(r#"{"format_version":"2","future":true}"#).unwrap_err(),
        FormatError::VersionMalformed
    ));
    assert!(matches!(
        from_str(r#"{"format_version":3,"future":true}"#).unwrap_err(),
        FormatError::UnsupportedVersion { found: 3, .. }
    ));
}

#[test]
fn ordinary_legacy_canonicalization_uses_the_same_normal_profile() {
    let exact = valid_v1(NORMAL_DIRECT_JSON_MAX_INPUT_BYTES);
    assert!(canonicalize_legacy_v1(exact.as_bytes()).is_ok());

    let oversized = valid_v1(NORMAL_DIRECT_JSON_MAX_INPUT_BYTES + 1);
    assert_input_limit(
        &canonicalize_legacy_v1(oversized.as_bytes()).unwrap_err(),
        oversized.len(),
    );
}

#[test]
fn admitted_unsupported_body_is_never_interpreted_as_v2() {
    let token = format!("1{}", "0".repeat(1024 * 1024 - 1));
    let source = format!(r#"{{"format_version":3,"future":{token}}}"#);

    assert!(matches!(
        from_str(&source).unwrap_err(),
        FormatError::UnsupportedVersion { found: 3, .. }
    ));
}

#[test]
fn admitted_hostile_shapes_remain_bounded_by_existing_fail_closed_rules() {
    let huge_member_name = padded(
        r#"{"format_version":3,""#,
        r#"":0}"#,
        NORMAL_DIRECT_JSON_MAX_INPUT_BYTES,
    );
    let many_members = many_members(381_000);
    assert!(many_members.len() <= NORMAL_DIRECT_JSON_MAX_INPUT_BYTES);
    let deeply_nested = format!(
        "{{\"format_version\":3,\"future\":{}0{}}}",
        "[".repeat(10_000),
        "]".repeat(10_000)
    );
    let oversized_number = format!("1{}", "0".repeat(V2_MAX_NUMBER_TOKEN_BYTES));
    let v2_number = format!(r#"{{"format_version":2,"future":{oversized_number}}}"#);

    let result = catch_unwind(|| from_bytes(huge_member_name.as_bytes()))
        .expect("huge member name unwound")
        .unwrap_err();
    assert!(matches!(
        result,
        FormatError::UnsupportedVersion { found: 3, .. }
    ));
    let result = catch_unwind(|| from_bytes(many_members.as_bytes()))
        .expect("many members unwound")
        .unwrap_err();
    assert!(matches!(
        result,
        FormatError::UnsupportedVersion { found: 3, .. }
    ));
    let result = catch_unwind(|| from_bytes(deeply_nested.as_bytes()))
        .expect("deep nesting unwound")
        .unwrap_err();
    assert!(matches!(result, FormatError::InvalidJson { .. }));
    let result = catch_unwind(|| from_bytes(v2_number.as_bytes()))
        .expect("v2 number token unwound")
        .unwrap_err();
    assert!(matches!(
        result,
        FormatError::ResourceLimit {
            resource: "number token",
            limit: V2_MAX_NUMBER_TOKEN_BYTES,
            actual,
        } if actual == V2_MAX_NUMBER_TOKEN_BYTES + 1
    ));
}
