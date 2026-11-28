use tachiko_semantic_core::Schema;

use super::{
    AnalysisFailure, AnalysisPredicate, AnalysisPredicateOperator, AnalysisValueKind,
    CalculationFailure, CalculationOutcome, Document, EntityId, FieldId, FieldRef, FieldType,
    NormalizedAnalysisDefinition, PredicateOperand, Value, effective_formula_number,
    reference_value_is_typed, value_kind,
};

#[derive(Clone, Copy)]
struct PredicateCalculation<'a> {
    outcome: Option<&'a CalculationOutcome>,
    requested_failure: Option<&'a (FieldRef, CalculationFailure)>,
}

pub(super) fn select_entities(
    document: &Document,
    definition: &NormalizedAnalysisDefinition,
    candidates: &[EntityId],
    calculation: Option<&CalculationOutcome>,
    requested_failure: Option<&(FieldRef, CalculationFailure)>,
) -> Result<Vec<EntityId>, AnalysisFailure> {
    let schema = &document.schemas[&definition.schema];
    let calculation = PredicateCalculation {
        outcome: calculation,
        requested_failure,
    };
    let mut selected = Vec::new();
    for entity_id in candidates {
        if entity_matches(
            document,
            schema,
            entity_id,
            &definition.predicates,
            calculation,
        )? {
            selected.push(entity_id.clone());
        }
    }
    Ok(selected)
}

fn entity_matches(
    document: &Document,
    schema: &Schema,
    entity_id: &EntityId,
    predicates: &[AnalysisPredicate],
    calculation: PredicateCalculation<'_>,
) -> Result<bool, AnalysisFailure> {
    let entity = &document.entities[entity_id];
    for predicate in predicates {
        let Some(value) = entity.fields.get(&predicate.field) else {
            return Ok(false);
        };
        if let (FieldType::Reference { schema }, Value::Reference(reference)) =
            (&schema.fields[&predicate.field].field_type, value)
        {
            if !reference_value_is_typed(document, schema, reference) {
                return Err(AnalysisFailure::InvalidPredicateValue {
                    entity: entity_id.clone(),
                    field: predicate.field.clone(),
                    actual: AnalysisValueKind::Reference,
                });
            }
        }
        let effective = effective_predicate_value(entity_id, &predicate.field, value, calculation)?;
        if !operand_kinds_match(&effective, &predicate.operand) {
            return Err(AnalysisFailure::InvalidPredicateValue {
                entity: entity_id.clone(),
                field: predicate.field.clone(),
                actual: value_kind(value),
            });
        }
        if !predicate_matches(&effective, predicate) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn effective_predicate_value(
    entity: &EntityId,
    field: &FieldId,
    value: &Value,
    calculation: PredicateCalculation<'_>,
) -> Result<PredicateOperand, AnalysisFailure> {
    match value {
        Value::Number(value) => Ok(PredicateOperand::Number(*value)),
        Value::Text(value) => Ok(PredicateOperand::Text(value.clone())),
        Value::Boolean(value) => Ok(PredicateOperand::Boolean(*value)),
        Value::Date(value) => Ok(PredicateOperand::Date(*value)),
        Value::Reference(value) => Ok(PredicateOperand::Reference(value.clone())),
        Value::Formula(_) => effective_formula_number(
            &FieldRef::new(entity.clone(), field.clone()),
            calculation.outcome,
            calculation.requested_failure,
        )
        .map(PredicateOperand::Number),
    }
}

const fn operand_kinds_match(left: &PredicateOperand, right: &PredicateOperand) -> bool {
    matches!(
        (left, right),
        (PredicateOperand::Number(_), PredicateOperand::Number(_))
            | (PredicateOperand::Text(_), PredicateOperand::Text(_))
            | (PredicateOperand::Boolean(_), PredicateOperand::Boolean(_))
            | (PredicateOperand::Date(_), PredicateOperand::Date(_))
            | (
                PredicateOperand::Reference(_),
                PredicateOperand::Reference(_)
            )
    )
}

fn predicate_matches(effective: &PredicateOperand, predicate: &AnalysisPredicate) -> bool {
    match predicate.operator {
        AnalysisPredicateOperator::Equal => effective == &predicate.operand,
        AnalysisPredicateOperator::NotEqual => effective != &predicate.operand,
        AnalysisPredicateOperator::LessThan => effective < &predicate.operand,
        AnalysisPredicateOperator::LessThanOrEqual => effective <= &predicate.operand,
        AnalysisPredicateOperator::GreaterThan => effective > &predicate.operand,
        AnalysisPredicateOperator::GreaterThanOrEqual => effective >= &predicate.operand,
    }
}
