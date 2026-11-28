use super::unsupported_formula_function;

#[test]
fn only_date_operands_accept_the_exact_blank_guard() {
    for formula in [
        "IFERROR(YEAR(IF(ISBLANK(A2),NA(),A2)),0)",
        "DAYS( if ( isblank ( A2 ), na ( ), a2 ),IF(ISBLANK(B2),NA(),B2))",
    ] {
        assert!(!unsupported_formula_function(formula, true), "{formula}");
    }
    for formula in [
        "NA()",
        "IF(ISBLANK(A2),NA(),A2)",
        "YEAR(IF(ISBLANK(A2),NA(),B2))",
        "WEEKDAY(A2,IF(ISBLANK(B2),NA(),B2))",
        "YEAR(IF(ISBLANK(A2),NA(),A2))+NA()",
    ] {
        assert!(unsupported_formula_function(formula, true), "{formula}");
    }
}
