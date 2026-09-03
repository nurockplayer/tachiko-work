use tachiko_semantic_core::{Date, Value};

fn date(input: &str) -> Date {
    input.parse().expect("test date must be valid")
}

#[test]
fn date_admission_enforces_proleptic_gregorian_boundaries() {
    for valid in ["0001-01-01", "1900-02-28", "2000-02-29", "9999-12-31"] {
        assert_eq!(date(valid).to_string(), valid);
    }

    for invalid in [
        "0000-01-01",
        "10000-01-01",
        "1900-02-29",
        "2001-02-29",
        "2024-04-31",
        "2024-00-01",
        "2024-01-00",
        "2024/01/01",
        " 2024-01-01",
        "2024-1-01",
    ] {
        assert!(
            invalid.parse::<Date>().is_err(),
            "{invalid} must be rejected"
        );
    }
}

#[test]
fn date_has_deterministic_order_and_canonical_serde_round_trip() {
    let earlier = date("2024-02-29");
    let later = date("2025-01-01");
    assert!(earlier < later);
    assert_eq!(earlier.year(), 2024);
    assert_eq!(earlier.month(), 2);
    assert_eq!(earlier.day(), 29);
    assert_eq!(earlier.to_canonical_string(), "2024-02-29");
    assert_eq!(serde_json::to_string(&earlier).unwrap(), r#""2024-02-29""#);
    assert_eq!(
        serde_json::from_str::<Date>(r#""2024-02-29""#).unwrap(),
        earlier
    );
    assert!(serde_json::from_str::<Date>(r#""1900-02-29""#).is_err());
}

#[test]
fn date_is_a_closed_semantic_field_and_value_variant() {
    let value = Value::Date(date("2024-02-29"));
    assert!(matches!(value, Value::Date(_)));
    assert_eq!(value, Value::Date(date("2024-02-29")));
}
