use tachiko_formula_engine::{FormulaParseError, format_expression, parse_expression};
use tachiko_semantic_core::{Expression, FieldRef};

fn number(value: f64) -> Expression {
    Expression::Number(value)
}

fn reference(entity: &str, field: &str) -> Expression {
    Expression::Reference(FieldRef::new(entity, field))
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
        Expression::Subtract {
            left: Box::new(Expression::Add {
                left: Box::new(number(1.0)),
                right: Box::new(Expression::Multiply {
                    left: Box::new(number(2.0)),
                    right: Box::new(number(3.0)),
                }),
            }),
            right: Box::new(Expression::Divide {
                left: Box::new(number(4.0)),
                right: Box::new(number(2.0)),
            }),
        }
    );
    assert_eq!(
        parse_expression("8 / 4 / 2").unwrap(),
        Expression::Divide {
            left: Box::new(Expression::Divide {
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
        Expression::Multiply {
            left: Box::new(Expression::Subtract {
                left: Box::new(number(0.0)),
                right: Box::new(reference("weapon", "damage")),
            }),
            right: Box::new(number(2.0)),
        }
    );
    assert_eq!(
        parse_expression("--[weapon.damage]").unwrap(),
        Expression::Subtract {
            left: Box::new(number(0.0)),
            right: Box::new(Expression::Subtract {
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
        Expression::Maximum {
            left: Box::new(Expression::Minimum {
                left: Box::new(number(1.0)),
                right: Box::new(number(2.0)),
            }),
            right: Box::new(Expression::Add {
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
fn canonical_formatter_covers_every_ast_shape() {
    let expression = Expression::Maximum {
        left: Box::new(Expression::Divide {
            left: Box::new(Expression::Add {
                left: Box::new(number(-2.0)),
                right: Box::new(reference("weapon", "damage")),
            }),
            right: Box::new(number(3.0)),
        }),
        right: Box::new(Expression::Minimum {
            left: Box::new(Expression::Subtract {
                left: Box::new(number(4.0)),
                right: Box::new(number(1.0)),
            }),
            right: Box::new(Expression::Multiply {
                left: Box::new(number(2.0)),
                right: Box::new(number(5.0)),
            }),
        }),
    };

    assert_eq!(
        format_expression(&expression),
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
        let canonical = format_expression(&parsed);
        let reparsed = parse_expression(&canonical).unwrap();

        assert_eq!(reparsed, parsed, "round trip failed for {input}");
        assert_eq!(format_expression(&reparsed), canonical);
        assert_eq!(format_expression(&parsed), format_expression(&parsed));
    }
}

#[test]
fn diagnostics_report_stable_byte_positions_and_messages() {
    for (input, position, message) in [
        ("", 0, "expected expression"),
        ("1 +", 3, "expected expression"),
        ("[weapon.damage", 14, "expected ']' after reference"),
        ("[Weapon.damage]", 1, "invalid reference entity identifier"),
        (
            "[weapon.damage.extra]",
            14,
            "reference must contain exactly one '.'",
        ),
        ("min(1 2)", 6, "expected ',' between function arguments"),
        ("sqrt(1)", 0, "unknown function 'sqrt'; expected min or max"),
        ("1e+", 3, "expected exponent digits"),
        ("1e999", 0, "numeric literal must be finite"),
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
fn node_limit_allows_255_nodes_and_rejects_the_257th_ast_node() {
    let accepted = std::iter::repeat_n("1", 128).collect::<Vec<_>>().join("+");
    assert!(parse_expression(&accepted).is_ok());

    let rejected = std::iter::repeat_n("1", 129).collect::<Vec<_>>().join("+");
    assert_parse_error(&rejected, 256, "expression exceeds 256-node limit");
}

#[test]
fn nesting_limit_allows_64_constructs_and_rejects_the_65th() {
    let accepted = format!("{}1{}", "(".repeat(64), ")".repeat(64));
    assert_eq!(parse_expression(&accepted).unwrap(), number(1.0));

    let rejected = format!("{}1{}", "(".repeat(65), ")".repeat(65));
    assert_parse_error(&rejected, 64, "expression exceeds 64-nesting limit");
}
