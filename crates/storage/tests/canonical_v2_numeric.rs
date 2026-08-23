use tachiko_semantic_core::Value;
use tachiko_storage::{FormatError, from_str, to_canonical_string};

const NUMERIC_GOLDEN: &str = include_str!("fixtures/direct-ro-v2-numeric-golden.ro");

const RFC8785_FINITE_VECTORS: &[(&str, u64, &str)] = &[
    ("0", 0x0000_0000_0000_0000, "0"),
    ("-0", 0x0000_0000_0000_0000, "0"),
    ("5e-324", 0x0000_0000_0000_0001, "5e-324"),
    ("-5e-324", 0x8000_0000_0000_0001, "-5e-324"),
    (
        "1.7976931348623157e+308",
        0x7fef_ffff_ffff_ffff,
        "1.7976931348623157e+308",
    ),
    (
        "-1.7976931348623157e+308",
        0xffef_ffff_ffff_ffff,
        "-1.7976931348623157e+308",
    ),
    (
        "9007199254740992",
        0x4340_0000_0000_0000,
        "9007199254740992",
    ),
    (
        "-9007199254740992",
        0xc340_0000_0000_0000,
        "-9007199254740992",
    ),
    (
        "295147905179352830000",
        0x4430_0000_0000_0000,
        "295147905179352830000",
    ),
    (
        "9.999999999999997e+22",
        0x44b5_2d02_c7e1_4af5,
        "9.999999999999997e+22",
    ),
    ("1e+23", 0x44b5_2d02_c7e1_4af6, "1e+23"),
    (
        "1.0000000000000001e+23",
        0x44b5_2d02_c7e1_4af7,
        "1.0000000000000001e+23",
    ),
    (
        "999999999999999700000",
        0x444b_1ae4_d6e2_ef4e,
        "999999999999999700000",
    ),
    (
        "999999999999999900000",
        0x444b_1ae4_d6e2_ef4f,
        "999999999999999900000",
    ),
    ("1e+21", 0x444b_1ae4_d6e2_ef50, "1e+21"),
    (
        "9.999999999999997e-7",
        0x3eb0_c6f7_a0b5_ed8c,
        "9.999999999999997e-7",
    ),
    ("0.000001", 0x3eb0_c6f7_a0b5_ed8d, "0.000001"),
    (
        "333333333.3333332",
        0x41b3_de43_5555_5553,
        "333333333.3333332",
    ),
    (
        "333333333.33333325",
        0x41b3_de43_5555_5554,
        "333333333.33333325",
    ),
    (
        "333333333.3333333",
        0x41b3_de43_5555_5555,
        "333333333.3333333",
    ),
    (
        "333333333.3333334",
        0x41b3_de43_5555_5556,
        "333333333.3333334",
    ),
    (
        "333333333.33333343",
        0x41b3_de43_5555_5557,
        "333333333.33333343",
    ),
    (
        "-0.0000033333333333333333",
        0xbecb_f647_612f_3696,
        "-0.0000033333333333333333",
    ),
    (
        "1424953923781206.2",
        0x4314_3ff3_c1cb_0959,
        "1424953923781206.2",
    ),
];

const ADDITIONAL_ADR0018_VECTORS: &[(&str, u64, &str)] = &[
    (
        "2.225073858507201e-308",
        0x000f_ffff_ffff_ffff,
        "2.225073858507201e-308",
    ),
    (
        "2.2250738585072014e-308",
        0x0010_0000_0000_0000,
        "2.2250738585072014e-308",
    ),
    (
        "9007199254740993",
        0x4340_0000_0000_0000,
        "9007199254740992",
    ),
    (
        "9007199254740995",
        0x4340_0000_0000_0002,
        "9007199254740996",
    ),
    (
        "-9007199254740993",
        0xc340_0000_0000_0000,
        "-9007199254740992",
    ),
    (
        "-9007199254740995",
        0xc340_0000_0000_0002,
        "-9007199254740996",
    ),
    (
        "1424953923781206.25",
        0x4314_3ff3_c1cb_0959,
        "1424953923781206.2",
    ),
    ("1e-4000", 0x0000_0000_0000_0000, "0"),
    ("-1e-4000", 0x0000_0000_0000_0000, "0"),
];

const ALTERNATE_SPELLINGS: &[(&str, u64, &str)] = &[
    ("-0.0", 0x0000_0000_0000_0000, "0"),
    ("0.0000010000", 0x3eb0_c6f7_a0b5_ed8d, "0.000001"),
    ("1.000000e-6", 0x3eb0_c6f7_a0b5_ed8d, "0.000001"),
    ("1000000000000000000000", 0x444b_1ae4_d6e2_ef50, "1e+21"),
    ("1.0e21", 0x444b_1ae4_d6e2_ef50, "1e+21"),
];

fn v2_number_source(number: &str) -> String {
    r#"{"format_version":2,"id":"doc","title":"Numeric conformance","schemas":{"schema":{"id":"schema","key":"schema","fields":{"number":{"id":"number","key":"number","field_type":{"type":"number"},"required":true}}}},"entities":{"entity":{"id":"entity","key":"entity","schema":"schema","fields":{"number":{"kind":"number","value":NUMBER}}}}}"#
        .replace("NUMBER", number)
}

fn assert_vector(input: &str, expected_bits: u64, canonical: &str) {
    let document = from_str(&v2_number_source(input)).unwrap();
    let Value::Number(number) = document.entities["entity"].fields["number"] else {
        panic!("numeric fixture must decode as Number")
    };
    assert_eq!(number.to_bits(), expected_bits, "input {input}");

    let encoded = to_canonical_string(&document).unwrap();
    assert!(
        encoded.contains(&format!("\"value\": {canonical}\n")),
        "input {input} encoded unexpectedly:\n{encoded}"
    );
    assert_eq!(
        to_canonical_string(&from_str(&encoded).unwrap()).unwrap(),
        encoded,
        "input {input} was not byte-stable after canonical re-encoding"
    );
}

#[test]
fn direct_ro_v2_numeric_golden_is_byte_stable() {
    let document = from_str(NUMERIC_GOLDEN).unwrap();
    assert_eq!(to_canonical_string(&document).unwrap(), NUMERIC_GOLDEN);
    assert_eq!(
        to_canonical_string(&from_str(NUMERIC_GOLDEN).unwrap()).unwrap(),
        NUMERIC_GOLDEN
    );
}

#[test]
fn direct_ro_v2_numeric_golden_detects_noncanonical_byte_drift() {
    let drifted = NUMERIC_GOLDEN.replacen(
        "          \"value\": 1e+21\n",
        "          \"value\": 1e21\n",
        1,
    );
    assert_ne!(drifted, NUMERIC_GOLDEN);

    let document = from_str(&drifted).unwrap();
    assert_eq!(to_canonical_string(&document).unwrap(), NUMERIC_GOLDEN);
}

#[test]
fn direct_ro_v2_covers_all_finite_rfc8785_appendix_b_vectors() {
    for &(input, bits, canonical) in RFC8785_FINITE_VECTORS {
        assert_vector(input, bits, canonical);
    }
}

#[test]
fn direct_ro_v2_covers_adr0018_threshold_subnormal_underflow_and_tie_vectors() {
    for &(input, bits, canonical) in ADDITIONAL_ADR0018_VECTORS {
        assert_vector(input, bits, canonical);
    }
}

#[test]
fn direct_ro_v2_canonicalizes_alternate_legal_number_spellings() {
    for &(input, bits, canonical) in ALTERNATE_SPELLINGS {
        assert_vector(input, bits, canonical);
    }
}

#[test]
fn direct_ro_v2_rejects_positive_and_negative_overflow_to_infinity() {
    for input in ["1e400", "-1e400"] {
        let error = from_str(&v2_number_source(input)).unwrap_err();
        assert!(
            matches!(error, FormatError::InvalidRepresentation { .. }),
            "{input}: {error:?}"
        );
    }
}
