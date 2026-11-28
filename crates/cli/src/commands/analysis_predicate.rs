use tachiko_workspace_engine::analysis_operations::{AnalysisPredicateOperator, PredicateOperand};
use tachiko_workspace_engine::{Date, EntityId, Number};

use super::CommandError;

pub(super) fn parse_operator(
    value: &str,
    operator: &str,
) -> Result<AnalysisPredicateOperator, CommandError> {
    Ok(match operator {
        "=" | "==" | "eq" => AnalysisPredicateOperator::Equal,
        "!=" | "ne" => AnalysisPredicateOperator::NotEqual,
        "<" | "lt" => AnalysisPredicateOperator::LessThan,
        "<=" | "lte" => AnalysisPredicateOperator::LessThanOrEqual,
        ">" | "gt" => AnalysisPredicateOperator::GreaterThan,
        ">=" | "gte" => AnalysisPredicateOperator::GreaterThanOrEqual,
        _ => {
            return Err(CommandError::InvalidAnalysisSyntax {
                kind: "predicate",
                value: value.to_owned(),
                expected: "field:operator:type:value (operator: =, !=, <, <=, >, >=)",
            });
        }
    })
}

pub(super) fn parse_operand(
    value: &str,
    operand_type: &str,
    operand: &str,
) -> Result<PredicateOperand, CommandError> {
    Ok(match operand_type {
        "number" => operand
            .parse::<f64>()
            .ok()
            .and_then(|n| Number::new(n).ok())
            .map(PredicateOperand::Number)
            .ok_or_else(|| CommandError::InvalidAnalysisSyntax {
                kind: "predicate",
                value: value.to_owned(),
                expected: "a finite number operand",
            })?,
        "text" => PredicateOperand::Text(operand.to_owned()),
        "boolean" => match operand {
            "true" => PredicateOperand::Boolean(true),
            "false" => PredicateOperand::Boolean(false),
            _ => {
                return Err(CommandError::InvalidAnalysisSyntax {
                    kind: "predicate",
                    value: value.to_owned(),
                    expected: "boolean value true or false",
                });
            }
        },
        "date" => operand
            .parse::<Date>()
            .map(PredicateOperand::Date)
            .map_err(|_| CommandError::InvalidAnalysisSyntax {
                kind: "predicate",
                value: value.to_owned(),
                expected: "a canonical Gregorian date operand (YYYY-MM-DD)",
            })?,
        "reference" => PredicateOperand::Reference(EntityId::from(operand)),
        _ => {
            return Err(CommandError::InvalidAnalysisSyntax {
                kind: "predicate",
                value: value.to_owned(),
                expected: "type number, text, boolean, date, or reference",
            });
        }
    })
}
