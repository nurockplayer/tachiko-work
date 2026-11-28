use tachiko_semantic_core::ComparisonOperator;

const TOKENS: [(&str, ComparisonOperator); 6] = [
    ("<=", ComparisonOperator::LessOrEqual),
    (">=", ComparisonOperator::GreaterOrEqual),
    ("<>", ComparisonOperator::NotEqual),
    ("=", ComparisonOperator::Equal),
    ("<", ComparisonOperator::Less),
    (">", ComparisonOperator::Greater),
];

pub(super) fn parse_prefix(input: &str) -> Option<ComparisonOperator> {
    TOKENS
        .into_iter()
        .find(|&(token, _)| input.starts_with(token))
        .map(|(_, operator)| operator)
}
