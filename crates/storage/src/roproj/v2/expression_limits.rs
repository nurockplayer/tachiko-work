use tachiko_semantic_core::{Expression, MAX_EXPRESSION_DEPTH, MAX_EXPRESSION_NODES};

use super::invalid_representation;
use crate::FormatError;

pub(super) fn validate_expression_limits(expression: &Expression) -> Result<(), FormatError> {
    let mut nodes = 0_usize;
    let mut stack = vec![(expression, 1_usize)];
    while let Some((node, depth)) = stack.pop() {
        if depth > MAX_EXPRESSION_DEPTH {
            return invalid_representation(format!(
                "formula expression exceeds {MAX_EXPRESSION_DEPTH}-depth limit"
            ));
        }
        nodes += 1;
        if nodes > MAX_EXPRESSION_NODES {
            return invalid_representation(format!(
                "formula expression exceeds {MAX_EXPRESSION_NODES}-node limit"
            ));
        }
        visit_node(node, depth, &mut stack)?;
    }
    Ok(())
}

fn visit_node<'a>(
    node: &'a Expression,
    depth: usize,
    stack: &mut Vec<(&'a Expression, usize)>,
) -> Result<(), FormatError> {
    match node {
        Expression::Add { left, right }
        | Expression::Subtract { left, right }
        | Expression::Multiply { left, right }
        | Expression::Divide { left, right }
        | Expression::Minimum { left, right }
        | Expression::Maximum { left, right } => {
            stack.extend([right, left].map(|child| (child.as_ref(), depth + 1)));
        }
        Expression::Compare { .. }
        | Expression::If { .. }
        | Expression::And(_)
        | Expression::Or(_)
        | Expression::Not(_)
        | Expression::Round { .. }
        | Expression::IfError { .. }
        | Expression::Abs(_)
        | Expression::RoundUp { .. }
        | Expression::RoundDown { .. }
        | Expression::DatePart { .. }
        | Expression::Weekday { .. }
        | Expression::Days { .. }
        | Expression::IsBlank(_) => {
            return invalid_representation("conditional and rounding formula expressions are not representable in frozen .roproj/v2".to_owned());
        }
        Expression::Number(_)
        | Expression::DeletedReference {}
        | Expression::Reference(_)
        | Expression::SameRowReference(_)
        | Expression::AnchoredReference(_)
        | Expression::AnchoredSameRowReference(_)
        | Expression::ColumnTotal(_)
        | Expression::ColumnAggregate { .. } => {}
        Expression::Lookup { .. } | Expression::ColumnConditional { .. } => {
            return invalid_representation("lookups and conditional aggregates are not representable in frozen .roproj/v2; use .roproj/v3".to_owned());
        }
    }
    Ok(())
}
