//! Reproducible native/wasm32 evidence for Issue #96.
//!
//! This is a research probe, not a production reader or a proposed contract.
//! It records the current `tachiko-storage` result classes for hostile inputs
//! before a representation version can be trusted.

#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use std::{env, hint::black_box, process::ExitCode, time::Instant};

use tachiko_storage::{FormatError, V2_MAX_INPUT_BYTES, V2_MAX_NUMBER_TOKEN_BYTES, from_bytes};

const OK: u32 = 0;
const INVALID_UTF8: u32 = 1;
const INVALID_JSON: u32 = 2;
const DUPLICATE_MEMBER: u32 = 3;
const VERSION_MISSING: u32 = 4;
const VERSION_MALFORMED: u32 = 5;
const UNSUPPORTED_VERSION: u32 = 6;
const INPUT_LIMIT: u32 = 7;
const NUMBER_TOKEN_LIMIT: u32 = 8;
const INVALID_REPRESENTATION: u32 = 9;
const MIGRATION_FAILED: u32 = 10;
const INVALID_DOCUMENT: u32 = 11;
const OTHER: u32 = 255;

const FIXED_CASE_COUNT: u32 = 15;

#[unsafe(no_mangle)]
pub extern "C" fn issue96_case_count() -> u32 {
    FIXED_CASE_COUNT
}

#[unsafe(no_mangle)]
pub extern "C" fn issue96_case_class(index: u32) -> u32 {
    let input = fixed_input(index);
    classify(from_bytes(black_box(&input)))
}

#[unsafe(no_mangle)]
pub extern "C" fn issue96_case_bytes(index: u32) -> u32 {
    u32::try_from(fixed_input(index).len()).expect("fixed probe inputs fit in u32")
}

fn classify<T>(result: Result<T, FormatError>) -> u32 {
    match result {
        Ok(_) => OK,
        Err(FormatError::InvalidUtf8 { .. }) => INVALID_UTF8,
        Err(FormatError::InvalidJson { .. }) => INVALID_JSON,
        Err(FormatError::DuplicateMember { .. }) => DUPLICATE_MEMBER,
        Err(FormatError::VersionMissing) => VERSION_MISSING,
        Err(FormatError::VersionMalformed) => VERSION_MALFORMED,
        Err(FormatError::UnsupportedVersion { .. }) => UNSUPPORTED_VERSION,
        Err(FormatError::ResourceLimit {
            resource: "input", ..
        }) => INPUT_LIMIT,
        Err(FormatError::ResourceLimit {
            resource: "number token",
            ..
        }) => NUMBER_TOKEN_LIMIT,
        Err(FormatError::InvalidRepresentation { .. }) => INVALID_REPRESENTATION,
        Err(FormatError::MigrationFailed { .. }) => MIGRATION_FAILED,
        Err(FormatError::InvalidDocument { .. }) => INVALID_DOCUMENT,
        Err(_) => OTHER,
    }
}

fn fixed_input(index: u32) -> Vec<u8> {
    let over = V2_MAX_INPUT_BYTES + 1;
    match index {
        0 => minimal_v1().into_bytes(),
        1 => minimal_v2().into_bytes(),
        2 => malformed_json(over).into_bytes(),
        3 => duplicate_member(over).into_bytes(),
        4 => missing_version(over).into_bytes(),
        5 => malformed_version(over).into_bytes(),
        6 => unsupported_version(over).into_bytes(),
        7 => invalid_utf8(over),
        8 => valid_v1(over).into_bytes(),
        9 => valid_v2(V2_MAX_INPUT_BYTES).into_bytes(),
        10 => valid_v2(over).into_bytes(),
        11 => v2_number(V2_MAX_NUMBER_TOKEN_BYTES + 1).into_bytes(),
        12 => unsupported_number(V2_MAX_NUMBER_TOKEN_BYTES + 1).into_bytes(),
        13 => duplicate_then_malformed(over).into_bytes(),
        14 => deeply_nested(1_024).into_bytes(),
        _ => Vec::new(),
    }
}

fn padded(prefix: &str, suffix: &str, target: usize, fill: char) -> String {
    assert!(fill.len_utf8() == 1);
    let filler = target
        .checked_sub(prefix.len() + suffix.len())
        .expect("target is large enough for probe framing");
    let mut input = String::with_capacity(target);
    input.push_str(prefix);
    input.extend(std::iter::repeat_n(fill, filler));
    input.push_str(suffix);
    assert_eq!(input.len(), target);
    input
}

fn minimal_v1() -> String {
    r#"{"format_version":1,"id":"doc","title":"x","schemas":{},"entities":{}}"#.to_owned()
}

fn minimal_v2() -> String {
    r#"{"format_version":2,"id":"doc","title":"x","schemas":{},"entities":{}}"#.to_owned()
}

fn valid_v1(target: usize) -> String {
    padded(
        r#"{"format_version":1,"id":"doc","title":""#,
        r#"","schemas":{},"entities":{}}"#,
        target,
        'x',
    )
}

fn valid_v2(target: usize) -> String {
    let mut input = minimal_v2();
    input.extend(std::iter::repeat_n(' ', target - input.len()));
    assert_eq!(input.len(), target);
    input
}

fn malformed_json(target: usize) -> String {
    padded(r#"{"format_version":2,"future":""#, "", target, 'x')
}

fn duplicate_member(target: usize) -> String {
    padded(
        r#"{"format_version":3,"future":""#,
        r#"","a":1,"\u0061":2}"#,
        target,
        'x',
    )
}

fn duplicate_then_malformed(target: usize) -> String {
    padded(
        r#"{"format_version":3,"a":1,"\u0061":2,"future":""#,
        "",
        target,
        'x',
    )
}

fn missing_version(target: usize) -> String {
    padded(r#"{"future":""#, r#""}"#, target, 'x')
}

fn malformed_version(target: usize) -> String {
    padded(r#"{"format_version":"2","future":""#, r#""}"#, target, 'x')
}

fn unsupported_version(target: usize) -> String {
    padded(r#"{"format_version":3,"future":""#, r#""}"#, target, 'x')
}

fn invalid_utf8(target: usize) -> Vec<u8> {
    let mut input = vec![b' '; target];
    input[target - 1] = 0xff;
    input
}

fn v2_number(token_bytes: usize) -> String {
    let token = format!("1{}", "0".repeat(token_bytes - 1));
    format!(r#"{{"format_version":2,"future":{token}}}"#)
}

fn unsupported_number(token_bytes: usize) -> String {
    let token = format!("1{}", "0".repeat(token_bytes - 1));
    format!(r#"{{"format_version":3,"future":{token}}}"#)
}

fn deeply_nested(depth: usize) -> String {
    format!(
        "{{\"format_version\":3,\"future\":{}0{}}}",
        "[".repeat(depth),
        "]".repeat(depth)
    )
}

fn huge_member_name(target: usize) -> String {
    padded(r#"{"format_version":3,""#, r#"":0}"#, target, 'x')
}

fn many_members(count: usize) -> String {
    let mut input = String::from(r#"{"format_version":3,"future":{"#);
    for index in 0..count {
        if index != 0 {
            input.push(',');
        }
        input.push_str(&format!(r#""k{index:016x}":0"#));
    }
    input.push_str("}}");
    input
}

fn named_input(name: &str, magnitude: usize) -> Option<Vec<u8>> {
    let input = match name {
        "valid-v1" => valid_v1(magnitude).into_bytes(),
        "valid-v2" => valid_v2(magnitude).into_bytes(),
        "malformed-json" => malformed_json(magnitude).into_bytes(),
        "duplicate" => duplicate_member(magnitude).into_bytes(),
        "duplicate-then-malformed" => duplicate_then_malformed(magnitude).into_bytes(),
        "missing-version" => missing_version(magnitude).into_bytes(),
        "malformed-version" => malformed_version(magnitude).into_bytes(),
        "unsupported-version" => unsupported_version(magnitude).into_bytes(),
        "invalid-utf8" => invalid_utf8(magnitude),
        "huge-member-name" => huge_member_name(magnitude).into_bytes(),
        "many-members" => many_members(magnitude).into_bytes(),
        "deep" => deeply_nested(magnitude).into_bytes(),
        "v2-number" => v2_number(magnitude).into_bytes(),
        "unsupported-number" => unsupported_number(magnitude).into_bytes(),
        _ => return None,
    };
    Some(input)
}

fn class_name(class: u32) -> &'static str {
    match class {
        OK => "Ok",
        INVALID_UTF8 => "InvalidUtf8",
        INVALID_JSON => "InvalidJson",
        DUPLICATE_MEMBER => "DuplicateMember",
        VERSION_MISSING => "VersionMissing",
        VERSION_MALFORMED => "VersionMalformed",
        UNSUPPORTED_VERSION => "UnsupportedVersion",
        INPUT_LIMIT => "ResourceLimit(input)",
        NUMBER_TOKEN_LIMIT => "ResourceLimit(number token)",
        INVALID_REPRESENTATION => "InvalidRepresentation",
        MIGRATION_FAILED => "MigrationFailed",
        INVALID_DOCUMENT => "InvalidDocument",
        _ => "Other",
    }
}

fn main() -> ExitCode {
    let mut arguments = env::args().skip(1);
    let Some(name) = arguments.next() else {
        for index in 0..issue96_case_count() {
            println!(
                "{index}|{}|{}",
                issue96_case_class(index),
                issue96_case_bytes(index)
            );
        }
        return ExitCode::SUCCESS;
    };
    let Some(magnitude) = arguments.next().and_then(|value| value.parse().ok()) else {
        eprintln!("usage: issue-96-pre-version-envelope CASE MAGNITUDE");
        return ExitCode::FAILURE;
    };
    let Some(input) = named_input(&name, magnitude) else {
        eprintln!("unknown case: {name}");
        return ExitCode::FAILURE;
    };
    let started = Instant::now();
    let class = classify(from_bytes(black_box(&input)));
    println!(
        "case={name} input_bytes={} class={} elapsed_us={}",
        input.len(),
        class_name(class),
        started.elapsed().as_micros()
    );
    ExitCode::SUCCESS
}
