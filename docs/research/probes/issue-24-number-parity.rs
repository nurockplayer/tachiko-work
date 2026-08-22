//! Reproducible native/wasm32 probe for Issue #24's proposed Number contract.
//!
//! This is research evidence, not the production formula implementation. It
//! deliberately models only the proposed finite-binary64 boundary, semantic
//! zero normalization, `min`/`max`, and current arithmetic operators.

use std::hint::black_box;

const VALUE: u32 = 0;
const DIVISION_BY_ZERO: u32 = 1;
const NON_FINITE_RESULT: u32 = 2;
const CASE_COUNT: u32 = 16;

#[unsafe(no_mangle)]
pub extern "C" fn issue24_case_count() -> u32 {
    CASE_COUNT
}

#[unsafe(no_mangle)]
pub extern "C" fn issue24_case_kind(index: u32) -> u32 {
    probe(index).0
}

#[unsafe(no_mangle)]
pub extern "C" fn issue24_case_bits(index: u32) -> u64 {
    probe(index).1
}

fn probe(index: u32) -> (u32, u64) {
    match index {
        0 => value(black_box(-0.0)),
        1 => value(semantic_min(black_box(-0.0), black_box(0.0))),
        2 => value(semantic_max(black_box(-0.0), black_box(0.0))),
        3 => semantic_divide(black_box(1.0), black_box(-0.0)),
        4 => value(black_box(f64::MAX) + black_box(f64::MAX)),
        5 => value(black_box(f64::from_bits(1))),
        6 => value(black_box(f64::MIN_POSITIVE) / black_box(2.0)),
        7 => value(black_box(f64::from_bits(1)) / black_box(2.0)),
        8 => value(black_box(-f64::from_bits(1)) / black_box(2.0)),
        9 => value(black_box(1.0) + black_box(f64::EPSILON / 2.0)),
        10 => value(black_box(1.0) + black_box(f64::EPSILON)),
        11 => value(semantic_min(
            black_box(-f64::from_bits(1)),
            black_box(f64::from_bits(1)),
        )),
        12 => value(semantic_max(
            black_box(-f64::from_bits(1)),
            black_box(f64::from_bits(1)),
        )),
        13 => semantic_divide(black_box(0.0), black_box(0.0)),
        14 => value(black_box(1.0) - black_box(1.0)),
        15 => value(black_box(-1.0) * black_box(0.0)),
        _ => (NON_FINITE_RESULT, 0),
    }
}

fn semantic_min(left: f64, right: f64) -> f64 {
    let left = normalize_zero(left);
    let right = normalize_zero(right);
    normalize_zero(if left <= right { left } else { right })
}

fn semantic_max(left: f64, right: f64) -> f64 {
    let left = normalize_zero(left);
    let right = normalize_zero(right);
    normalize_zero(if left >= right { left } else { right })
}

fn semantic_divide(left: f64, right: f64) -> (u32, u64) {
    if right == 0.0 {
        (DIVISION_BY_ZERO, 0)
    } else {
        value(left / right)
    }
}

fn value(result: f64) -> (u32, u64) {
    if result.is_finite() {
        (VALUE, normalize_zero(result).to_bits())
    } else {
        (NON_FINITE_RESULT, 0)
    }
}

fn normalize_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

#[allow(dead_code)]
fn main() {
    for index in 0..issue24_case_count() {
        println!(
            "{index}:{}:{:016x}",
            issue24_case_kind(index),
            issue24_case_bits(index)
        );
    }
}
