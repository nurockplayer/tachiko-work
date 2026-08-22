use tachiko_formula_engine::{
    ExpressionComplexityError, FormulaParseError, UnboundExpression, format_unbound_expression,
    parse_expression, validate_unbound_expression_structure,
};
use tachiko_semantic_core::{FieldAddress, Number};

fn number(value: f64) -> UnboundExpression {
    UnboundExpression::Number(Number::new(value).unwrap())
}

fn reference(entity: &str, field: &str) -> UnboundExpression {
    UnboundExpression::Reference(FieldAddress::new(entity, field))
}

fn left_associative_sum(terms: usize) -> String {
    std::iter::repeat_n("1", terms)
        .collect::<Vec<_>>()
        .join("+")
}

fn left_associative_ast(terms: usize, value: f64) -> UnboundExpression {
    let mut expression = number(value);
    for _ in 1..terms {
        expression = UnboundExpression::Add {
            left: Box::new(expression),
            right: Box::new(number(value)),
        };
    }
    expression
}

fn balanced_sum(leaves: usize) -> String {
    balanced_sum_with_leaf(leaves, "1")
}

fn balanced_sum_with_leaf(leaves: usize, leaf: &str) -> String {
    if leaves == 1 {
        return leaf.to_owned();
    }
    let left = leaves / 2;
    let right = leaves - left;
    format!(
        "({}+{})",
        balanced_sum_with_leaf(left, leaf),
        balanced_sum_with_leaf(right, leaf)
    )
}

fn balanced_ast(leaves: usize) -> UnboundExpression {
    balanced_ast_with_leaf(leaves, &number(1.0))
}

fn balanced_ast_with_leaf(leaves: usize, leaf: &UnboundExpression) -> UnboundExpression {
    if leaves == 1 {
        return leaf.clone();
    }
    let left = leaves / 2;
    let right = leaves - left;
    UnboundExpression::Add {
        left: Box::new(balanced_ast_with_leaf(left, leaf)),
        right: Box::new(balanced_ast_with_leaf(right, leaf)),
    }
}

fn assert_parse_error(input: &str, position: usize, message: &str) {
    let error = parse_expression(input).expect_err("expression should be rejected");
    assert_eq!(
        error,
        FormulaParseError {
            position,
            message: message.to_owned(),
        }
    );
    assert_eq!(
        error.to_string(),
        format!("formula parse error at byte {position}: {message}")
    );
}

#[test]
fn precedence_and_left_associativity_map_exactly_to_the_ast() {
    assert_eq!(
        parse_expression("1 + 2 * 3 - 4 / 2").unwrap(),
        UnboundExpression::Subtract {
            left: Box::new(UnboundExpression::Add {
                left: Box::new(number(1.0)),
                right: Box::new(UnboundExpression::Multiply {
                    left: Box::new(number(2.0)),
                    right: Box::new(number(3.0)),
                }),
            }),
            right: Box::new(UnboundExpression::Divide {
                left: Box::new(number(4.0)),
                right: Box::new(number(2.0)),
            }),
        }
    );
    assert_eq!(
        parse_expression("8 / 4 / 2").unwrap(),
        UnboundExpression::Divide {
            left: Box::new(UnboundExpression::Divide {
                left: Box::new(number(8.0)),
                right: Box::new(number(4.0)),
            }),
            right: Box::new(number(2.0)),
        }
    );
}

#[test]
fn unary_signs_bind_before_arithmetic_and_non_literals_use_zero_subtraction() {
    assert_eq!(parse_expression("-1.5").unwrap(), number(-1.5));
    assert_eq!(parse_expression("+2e3").unwrap(), number(2_000.0));
    assert_eq!(
        parse_expression("-[weapon.damage] * 2").unwrap(),
        UnboundExpression::Multiply {
            left: Box::new(UnboundExpression::Subtract {
                left: Box::new(number(0.0)),
                right: Box::new(reference("weapon", "damage")),
            }),
            right: Box::new(number(2.0)),
        }
    );
    assert_eq!(
        parse_expression("--[weapon.damage]").unwrap(),
        UnboundExpression::Subtract {
            left: Box::new(number(0.0)),
            right: Box::new(UnboundExpression::Subtract {
                left: Box::new(number(0.0)),
                right: Box::new(reference("weapon", "damage")),
            }),
        }
    );
}

#[test]
fn parentheses_functions_references_and_ascii_whitespace_are_supported() {
    let parsed =
        parse_expression("\tmax( min(1, 2),\n([weapon-2.damage_per-hit] + .5) )\r").unwrap();

    assert_eq!(
        parsed,
        UnboundExpression::Maximum {
            left: Box::new(UnboundExpression::Minimum {
                left: Box::new(number(1.0)),
                right: Box::new(number(2.0)),
            }),
            right: Box::new(UnboundExpression::Add {
                left: Box::new(reference("weapon-2", "damage_per-hit")),
                right: Box::new(number(0.5)),
            }),
        }
    );
}

#[test]
fn decimal_and_scientific_literals_are_finite_f64_values() {
    for (input, expected) in [
        ("0", 0.0),
        ("1.", 1.0),
        (".5", 0.5),
        ("6.02e23", 6.02e23),
        ("1E-3", 0.001),
    ] {
        assert_eq!(parse_expression(input).unwrap(), number(expected));
    }
}

#[test]
fn canonical_numbers_preserve_precision_and_normalize_negative_zero() {
    for (input, expected) in [
        ("1e308", "1e308"),
        ("5e-324", "5e-324"),
        ("1.2345678901234567", "1.2345678901234567"),
        ("100", "100"),
        ("-0", "0"),
    ] {
        let expression = parse_expression(input).unwrap();
        let canonical = format_unbound_expression(&expression);

        assert_eq!(canonical, expected);
        let UnboundExpression::Number(original) = expression else {
            panic!("numeric input must parse to a number")
        };
        let UnboundExpression::Number(reparsed) = parse_expression(&canonical).unwrap() else {
            panic!("canonical number must reparse to a number")
        };
        assert_eq!(reparsed.to_bits(), original.to_bits());
    }
}

#[test]
fn canonical_formatter_covers_every_ast_shape() {
    let expression = UnboundExpression::Maximum {
        left: Box::new(UnboundExpression::Divide {
            left: Box::new(UnboundExpression::Add {
                left: Box::new(number(-2.0)),
                right: Box::new(reference("weapon", "damage")),
            }),
            right: Box::new(number(3.0)),
        }),
        right: Box::new(UnboundExpression::Minimum {
            left: Box::new(UnboundExpression::Subtract {
                left: Box::new(number(4.0)),
                right: Box::new(number(1.0)),
            }),
            right: Box::new(UnboundExpression::Multiply {
                left: Box::new(number(2.0)),
                right: Box::new(number(5.0)),
            }),
        }),
    };

    assert_eq!(
        format_unbound_expression(&expression),
        "max(((-2 + [weapon.damage]) / 3), min((4 - 1), (2 * 5)))"
    );
}

#[test]
fn canonical_format_round_trips_and_is_deterministic() {
    for input in [
        "1 + 2 * 3",
        "-(1 + [weapon.damage]) / max(.5, 1e-2)",
        "min([a-b.c_d], max(-0, +42))",
        "([weapon.damage])",
    ] {
        let parsed = parse_expression(input).unwrap();
        let canonical = format_unbound_expression(&parsed);
        let reparsed = parse_expression(&canonical).unwrap();

        assert_eq!(reparsed, parsed, "round trip failed for {input}");
        assert_eq!(format_unbound_expression(&reparsed), canonical);
        assert_eq!(
            format_unbound_expression(&parsed),
            format_unbound_expression(&parsed)
        );
    }
}

#[test]
fn diagnostics_report_stable_byte_positions_and_messages() {
    for (input, position, message) in [
        ("", 0, "expected expression"),
        ("1 +", 3, "expected expression"),
        ("[weapon.damage", 14, "expected ']' after reference"),
        ("[Weapon.damage]", 1, "invalid reference entity key"),
        (
            "[weapon.damage.extra]",
            14,
            "reference must contain exactly one '.'",
        ),
        ("min(1 2)", 6, "expected ',' between function arguments"),
        ("sqrt(1)", 0, "unknown function 'sqrt'; expected min or max"),
        ("1e+", 3, "expected exponent digits"),
        (
            "1e999",
            0,
            "numeric literal must convert to a finite Number",
        ),
        ("1 2", 2, "unexpected trailing content"),
        ("@", 0, "expected number, reference, function, or '('"),
    ] {
        assert_parse_error(input, position, message);
    }
}

#[test]
fn references_require_exact_valid_identifier_components_without_whitespace() {
    for input in [
        "[weapon]",
        "[.damage]",
        "[weapon.]",
        "[weapon .damage]",
        "[weapon.damage ]",
        "[weapon..damage]",
    ] {
        assert!(
            parse_expression(input).is_err(),
            "malformed reference should fail: {input}"
        );
    }
    for input in [
        "[0.1]",
        "[weapon-2.damage_per-hit]",
        "[weapon_2.damage-avg]",
    ] {
        assert!(
            parse_expression(input).is_ok(),
            "valid reference should parse: {input}"
        );
    }
}

#[test]
fn input_limit_is_exactly_4096_bytes() {
    let accepted = format!("1{}", " ".repeat(4_095));
    assert_eq!(accepted.len(), 4_096);
    assert_eq!(parse_expression(&accepted).unwrap(), number(1.0));

    let rejected = format!("1{}", " ".repeat(4_096));
    assert_eq!(rejected.len(), 4_097);
    assert_parse_error(&rejected, 4_096, "expression exceeds 4096-byte limit");
}

#[test]
fn nesting_limit_allows_64_constructs_and_rejects_the_65th() {
    let accepted = format!("{}1{}", "(".repeat(64), ")".repeat(64));
    assert_eq!(parse_expression(&accepted).unwrap(), number(1.0));

    let rejected = format!("{}1{}", "(".repeat(65), ")".repeat(65));
    assert_parse_error(&rejected, 64, "expression exceeds 64-nesting limit");
}

#[test]
fn balanced_node_limit_allows_255_nodes_and_rejects_257() {
    let accepted_input = balanced_sum(128);
    let accepted = parse_expression(&accepted_input).expect("255 balanced nodes should parse");
    assert_eq!(validate_unbound_expression_structure(&accepted), Ok(()));

    let rejected_ast = balanced_ast(129);
    assert_eq!(
        validate_unbound_expression_structure(&rejected_ast),
        Err(ExpressionComplexityError::NodeLimit)
    );
    let rejected = parse_expression(&balanced_sum(129)).unwrap_err();
    assert_eq!(rejected.message, "expression exceeds 256-node limit");
}

#[test]
fn post_desugaring_depth_allows_64_and_rejects_65_in_flat_chains() {
    let accepted_input = left_associative_sum(64);
    let accepted = parse_expression(&accepted_input).expect("depth 64 should parse");
    assert_eq!(validate_unbound_expression_structure(&accepted), Ok(()));
    let canonical = format_unbound_expression(&accepted);
    assert_eq!(parse_expression(&canonical).unwrap(), accepted);

    let rejected_input = left_associative_sum(65);
    assert_parse_error(
        &rejected_input,
        rejected_input.len(),
        "expression exceeds 64-depth limit",
    );
    assert_eq!(
        validate_unbound_expression_structure(&left_associative_ast(65, 1.0)),
        Err(ExpressionComplexityError::DepthLimit)
    );
}

#[test]
fn unary_desugaring_participates_in_post_construction_depth() {
    let accepted_input = format!("{}[a.b]", "-".repeat(63));
    let accepted = parse_expression(&accepted_input).expect("desugared depth 64 should parse");
    assert_eq!(validate_unbound_expression_structure(&accepted), Ok(()));

    let rejected_input = format!("{}[a.b]", "-".repeat(64));
    assert_parse_error(
        &rejected_input,
        rejected_input.len(),
        "expression exceeds 64-depth limit",
    );
}

#[test]
fn canonical_byte_limit_is_enforced_after_construction() {
    let exact_field = "x".repeat(4_092);
    let exact_input = format!("[a.{exact_field}]");
    assert_eq!(exact_input.len(), 4_096);
    let exact = parse_expression(&exact_input).expect("4096 canonical bytes should parse");
    assert_eq!(format_unbound_expression(&exact).len(), 4_096);
    assert_eq!(validate_unbound_expression_structure(&exact), Ok(()));
    assert_eq!(
        parse_expression(&format_unbound_expression(&exact)).unwrap(),
        exact
    );

    let field = "x".repeat(24);
    let leaf_input = format!("[a.{field}]");
    let rejected_input = balanced_sum_with_leaf(128, &leaf_input);
    assert!(rejected_input.len() < 4_096);
    assert_parse_error(
        &rejected_input,
        rejected_input.len(),
        "canonical expression exceeds 4096-byte limit",
    );
    let rejected_ast = balanced_ast_with_leaf(128, &reference("a", &field));
    assert!(format_unbound_expression(&rejected_ast).len() > 4_096);
    assert_eq!(
        validate_unbound_expression_structure(&rejected_ast),
        Err(ExpressionComplexityError::CanonicalLengthLimit)
    );
}
