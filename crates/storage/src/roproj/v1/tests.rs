use super::NumberV1;

#[test]
fn storage_number_conversion_normalizes_zero_and_rejects_nonfinite() {
    assert_eq!(NumberV1(-0.0).into_semantic().unwrap().to_bits(), 0);
    assert!(NumberV1(f64::INFINITY).into_semantic().is_err());
}
