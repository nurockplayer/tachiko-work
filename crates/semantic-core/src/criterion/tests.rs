use super::{CriterionCell, CriterionKey, CriterionTest};
use crate::{ComparisonOperator, Date};

#[test]
fn relational_criteria_refuse_text_and_booleans() {
    assert!(CriterionTest::parse(">abc").is_err());
    assert!(CriterionTest::parse(">=true").is_err());
    assert!(CriterionTest::parse(">").is_err());
    assert!(CriterionTest::parse(">=5").is_ok());
    assert!(CriterionTest::parse(">2026-01-01").is_ok());
}

#[test]
fn wildcards_match_whole_cells_case_insensitively() {
    let test = CriterionTest::parse("A*").expect("valid criterion");
    assert!(test.matches(Some(CriterionCell::Text("abc"))));
    assert!(!test.matches(Some(CriterionCell::Text("ba"))));

    let test = CriterionTest::parse("?b").expect("valid criterion");
    assert!(test.matches(Some(CriterionCell::Text("AB"))));
    assert!(!test.matches(Some(CriterionCell::Text("aab"))));

    let test = CriterionTest::parse("~*").expect("valid criterion");
    assert!(test.matches(Some(CriterionCell::Text("*"))));
    assert!(!test.matches(Some(CriterionCell::Text("a"))));
}

#[test]
fn blanks_match_empty_text_and_not_equal_includes_them() {
    let empty = CriterionTest::parse("").expect("valid criterion");
    assert!(empty.matches(None));
    assert!(empty.matches(Some(CriterionCell::Text(""))));
    assert!(!empty.matches(Some(CriterionCell::Text("x"))));

    let not_empty = CriterionTest::parse("<>").expect("valid criterion");
    assert!(!not_empty.matches(None));
    assert!(not_empty.matches(Some(CriterionCell::Text("x"))));

    let not_x = CriterionTest::parse("<>x").expect("valid criterion");
    assert!(not_x.matches(None));
    assert!(not_x.matches(Some(CriterionCell::Text("y"))));
    assert!(!not_x.matches(Some(CriterionCell::Text("X"))));
}

#[test]
fn numeric_text_cells_compare_to_number_operands() {
    let test = CriterionTest::parse("5").expect("valid criterion");
    assert!(test.matches(Some(CriterionCell::Text("5"))));
    assert!(!test.matches(Some(CriterionCell::Text(" 5"))));
    assert!(!test.matches(Some(CriterionCell::Text("five"))));
}

#[test]
fn dates_and_booleans_compare_by_value() {
    let test = CriterionTest::parse("2026-01-01").expect("valid criterion");
    assert!(test.matches(Some(CriterionCell::Date(
        Date::parse("2026-01-01").expect("valid date")
    ))));
    assert!(test.matches(Some(CriterionCell::Text("2026-01-01"))));

    let test = CriterionTest::parse("TRUE").expect("valid criterion");
    assert!(test.matches(Some(CriterionCell::Boolean(true))));
    assert!(test.matches(Some(CriterionCell::Text("true"))));
}

/// Every text a test may be parsed from, plus the constructors that do not
/// go through [`CriterionTest::parse`].
fn tests() -> Vec<CriterionTest> {
    let mut tests = texts()
        .iter()
        .map(|text| CriterionTest::parse(text).expect("valid criterion"))
        .collect::<Vec<_>>();
    for operator in [
        ComparisonOperator::Equal,
        ComparisonOperator::NotEqual,
        ComparisonOperator::Less,
        ComparisonOperator::LessOrEqual,
        ComparisonOperator::Greater,
        ComparisonOperator::GreaterOrEqual,
    ] {
        tests.push(CriterionTest::number(operator, 1.0));
        tests.push(CriterionTest::number(operator, 0.0));
        tests.push(CriterionTest::blank(operator));
    }
    tests.push(CriterionTest::equal_number(1.0));
    tests.push(CriterionTest::equal_number(-0.0));
    tests.push(CriterionTest::equal_boolean(true));
    tests.push(CriterionTest::equal_boolean(false));
    tests.push(CriterionTest::equal_date(
        Date::parse("2026-01-05").expect("valid date"),
    ));
    tests
}

fn texts() -> Vec<String> {
    [
        "0",
        "-0",
        "1",
        "1.0",
        "1e0",
        "+1",
        "1.5",
        "abc",
        "ABC",
        "ÀB",
        "àb",
        "İ",
        "i\u{307}",
        "*",
        "~",
        "",
        "true",
        "TRUE",
        "2026-01-05",
        "2026-1-5",
        "=abc",
        "a*",
        "a?",
        "~*",
        "<>abc",
        ">1",
        "<=1",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

/// Every cell a criterion column may hold, including a blank.
fn cells() -> Vec<Option<CriterionCell<'static>>> {
    let mut cells = [
        "0",
        "-0",
        "1",
        "1.0",
        "1e0",
        "+1",
        "1.5",
        "abc",
        "ABC",
        "ÀB",
        "àb",
        "İ",
        "i\u{307}",
        "*",
        "~",
        "",
        "true",
        "TRUE",
        "2026-01-05",
        "2026-1-5",
    ]
    .into_iter()
    .map(|text| Some(CriterionCell::Text(text)))
    .collect::<Vec<_>>();
    for number in [0.0, -0.0, 1.0, 1.5] {
        cells.push(Some(CriterionCell::Number(number)));
    }
    cells.push(Some(CriterionCell::Boolean(true)));
    cells.push(Some(CriterionCell::Boolean(false)));
    cells.push(Some(CriterionCell::Date(
        Date::parse("2026-01-05").expect("valid date"),
    )));
    cells.push(None);
    cells
}

#[test]
fn a_test_that_has_keys_is_never_missed_by_them() {
    let mut explained = 0_usize;
    let mut cross_type = 0_usize;
    for test in &tests() {
        let Some(keys) = test.keys() else {
            continue;
        };
        for cell in &cells() {
            if !test.matches(*cell) {
                continue;
            }
            let cell_keys = cell.map_or_else(Vec::new, CriterionCell::keys);
            let Some(key) = keys.iter().find(|key| cell_keys.contains(key)) else {
                panic!("{test:?} matches {cell:?} but none of {keys:?} is in {cell_keys:?}");
            };
            explained += 1;
            // A match the cell's own type alone would not explain: numeric
            // text under a number operand, or a text spelling of a
            // boolean or a date.
            let own_type_only = matches!(
                (cell, key),
                (Some(CriterionCell::Number(_)), CriterionKey::Number(_))
                    | (Some(CriterionCell::Text(_)), CriterionKey::Text(_))
                    | (Some(CriterionCell::Boolean(_)), CriterionKey::Boolean(_))
                    | (Some(CriterionCell::Date(_)), CriterionKey::Date(_))
            );
            if !own_type_only {
                cross_type += 1;
            }
        }
    }
    assert!(
        explained > 20,
        "only {explained} matched pairs were explained by a key"
    );
    assert!(
        cross_type > 0,
        "the text spellings of numbers, booleans and dates must match"
    );
}

#[test]
fn a_keyed_test_and_a_scanning_test_say_so() {
    // Equality with a literal has keys; relational, wildcard, blank and
    // `<>` criteria have to read every cell.
    for text in ["abc", "1", "2026-01-05", "TRUE"] {
        assert!(
            CriterionTest::parse(text)
                .expect("valid criterion")
                .keys()
                .is_some(),
            "{text}"
        );
    }
    for text in ["", "*", "abc*", "a?", "~*"] {
        assert!(
            CriterionTest::parse(text)
                .expect("valid criterion")
                .keys()
                .is_none(),
            "{text:?}"
        );
    }
    for text in [">1", "<=1", "<>abc"] {
        assert!(
            CriterionTest::parse(text)
                .expect("valid criterion")
                .keys()
                .is_none(),
            "{text}"
        );
    }
    assert!(
        CriterionTest::blank(ComparisonOperator::Equal)
            .keys()
            .is_none()
    );
}
