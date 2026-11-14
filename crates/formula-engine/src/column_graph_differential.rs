//! Differential oracle for the column-edge dependency graph.
//!
//! The reference half keeps the previous implementation: expanded per-cell
//! dependency sets, SCCs over those sets, and affected-by over the expanded
//! map. Deterministic generated documents must produce identical values,
//! failures, dependencies, and affected-by results through the graph.

use std::collections::{BTreeMap, BTreeSet};

use tachiko_semantic_core::{
    ColumnAggregateFunction, ColumnCriterion, ColumnRef, ComparisonOperator, ConditionalAggregate,
    Document, DocumentId, Entity, EntityId, EntityKey, Expression, FieldConstraint,
    FieldDefinition, FieldId, FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey,
    Value,
};

use super::{
    CalculationError, CalculationFailure, CalculationOutcome, ReferenceFailure,
    RetainedCalculationState, calculate_complete,
};

const ALPHA: &str = "alpha";
const BETA: &str = "beta";

/// Reference pipeline: everything below computes the previous behavior.
mod reference {
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Arc;

    use tachiko_semantic_core::{Document, FieldRef, Number, Value};

    use crate::{
        CalculationFailure, FailureMap, FormulaNodes, ValueNodes, evaluation, extract_dependencies,
        initial_values, pregraph_failures,
    };

    pub struct ReferenceCalculation {
        pub values: BTreeMap<FieldRef, Number>,
        pub failures: FailureMap,
        pub dependencies: BTreeMap<FieldRef, BTreeSet<FieldRef>>,
    }

    pub fn calculate(document: &Document) -> ReferenceCalculation {
        let (nodes, formulas, dependencies) = collect(document);
        let mut failures = pregraph_failures(document, &nodes, &formulas);
        assign_cycle_failures(&formulas, &dependencies, &mut failures);
        let mut values = initial_values(&nodes, &failures);
        evaluate(
            document,
            &formulas,
            &dependencies,
            &mut failures,
            &mut values,
        );
        ReferenceCalculation {
            values,
            failures,
            dependencies,
        }
    }

    fn collect(
        document: &Document,
    ) -> (
        ValueNodes<'_>,
        FormulaNodes<'_>,
        BTreeMap<FieldRef, BTreeSet<FieldRef>>,
    ) {
        let mut nodes = BTreeMap::new();
        let mut formulas = BTreeMap::new();
        let mut dependencies = BTreeMap::new();
        for (entity_id, entity) in &document.entities {
            for (field_id, value) in &entity.fields {
                if matches!(value, Value::Number(_) | Value::Formula(_)) {
                    let field = FieldRef::new(entity_id.clone(), field_id.clone());
                    nodes.insert(field.clone(), value);
                    if let Value::Formula(expression) = value {
                        dependencies.insert(
                            field.clone(),
                            extract_dependencies(document, &field, expression),
                        );
                        formulas.insert(field, expression);
                    }
                }
            }
        }
        (nodes, formulas, dependencies)
    }

    fn assign_cycle_failures(
        formulas: &FormulaNodes<'_>,
        dependencies: &BTreeMap<FieldRef, BTreeSet<FieldRef>>,
        failures: &mut FailureMap,
    ) {
        let eligible_formulas = formulas
            .keys()
            .filter(|formula| !failures.contains_key(*formula))
            .cloned()
            .collect::<BTreeSet<_>>();
        for members in cyclic_components(&eligible_formulas, dependencies) {
            let members = Arc::new(members);
            for member in members.iter() {
                failures.insert(
                    member.clone(),
                    CalculationFailure::Cycle {
                        members: Arc::clone(&members),
                    },
                );
            }
        }
    }

    fn evaluate(
        document: &Document,
        formulas: &FormulaNodes<'_>,
        dependencies: &BTreeMap<FieldRef, BTreeSet<FieldRef>>,
        failures: &mut FailureMap,
        values: &mut BTreeMap<FieldRef, Number>,
    ) {
        let remaining_formulas = formulas
            .keys()
            .filter(|formula| !failures.contains_key(*formula))
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut remaining_dependency_counts = remaining_formulas
            .iter()
            .map(|formula| {
                let count = dependencies
                    .get(formula)
                    .into_iter()
                    .flatten()
                    .filter(|dependency| remaining_formulas.contains(*dependency))
                    .count();
                (formula.clone(), count)
            })
            .collect::<BTreeMap<_, _>>();
        let mut dependents: BTreeMap<FieldRef, BTreeSet<FieldRef>> = BTreeMap::new();
        for formula in &remaining_formulas {
            for dependency in dependencies
                .get(formula)
                .into_iter()
                .flatten()
                .filter(|dependency| remaining_formulas.contains(*dependency))
            {
                dependents
                    .entry(dependency.clone())
                    .or_default()
                    .insert(formula.clone());
            }
        }
        let mut ready = remaining_dependency_counts
            .iter()
            .filter(|(_, count)| **count == 0)
            .map(|(formula, _)| formula.clone())
            .collect::<BTreeSet<_>>();

        let mut columns = evaluation::ColumnCache::default();
        while let Some(formula) = ready.pop_first() {
            remaining_dependency_counts.remove(&formula);
            let expression = formulas[&formula];
            for column in crate::extract_column_totals(expression) {
                columns.fill(document, values, failures, &column);
            }
            let mut evaluation = evaluation::Evaluation {
                document,
                owner: &formula,
                values,
                columns: &mut columns,
            };
            let failed_dependencies = evaluation
                .required_references(expression, failures)
                .iter()
                .filter(|dependency| failures.contains_key(*dependency))
                .cloned()
                .collect::<BTreeSet<_>>();
            if failed_dependencies.is_empty() {
                match evaluation.number(expression) {
                    Ok(value) => {
                        values.insert(formula.clone(), value);
                    }
                    Err(failure) => {
                        failures.insert(formula.clone(), failure);
                    }
                }
            } else {
                failures.insert(
                    formula.clone(),
                    CalculationFailure::FailedDependencies {
                        dependencies: failed_dependencies,
                    },
                );
            }

            for dependent in dependents.get(&formula).into_iter().flatten() {
                let Some(count) = remaining_dependency_counts.get_mut(dependent) else {
                    continue;
                };
                *count -= 1;
                if *count == 0 {
                    ready.insert(dependent.clone());
                }
            }
        }

        assert!(
            remaining_dependency_counts.is_empty(),
            "reference SCC condensation must be acyclic"
        );
    }

    fn cyclic_components(
        eligible: &BTreeSet<FieldRef>,
        dependencies: &BTreeMap<FieldRef, BTreeSet<FieldRef>>,
    ) -> Vec<BTreeSet<FieldRef>> {
        let mut visited = BTreeSet::new();
        let mut finish_order = Vec::new();
        for root in eligible {
            if !visited.insert(root.clone()) {
                continue;
            }
            let root_dependencies = dependencies
                .get(root)
                .into_iter()
                .flatten()
                .filter(|dependency| eligible.contains(*dependency))
                .cloned()
                .collect::<Vec<_>>();
            let mut stack = vec![(root.clone(), root_dependencies, 0_usize)];
            while !stack.is_empty() {
                let next_dependency = {
                    let (_, node_dependencies, next_index) =
                        stack.last_mut().expect("DFS stack is non-empty");
                    if *next_index < node_dependencies.len() {
                        let dependency = node_dependencies[*next_index].clone();
                        *next_index += 1;
                        Some(dependency)
                    } else {
                        None
                    }
                };
                if let Some(dependency) = next_dependency {
                    if !visited.insert(dependency.clone()) {
                        continue;
                    }
                    let child_dependencies = dependencies
                        .get(&dependency)
                        .into_iter()
                        .flatten()
                        .filter(|candidate| eligible.contains(*candidate))
                        .cloned()
                        .collect::<Vec<_>>();
                    stack.push((dependency, child_dependencies, 0));
                } else {
                    let (node, _, _) = stack.pop().expect("DFS stack is non-empty");
                    finish_order.push(node);
                }
            }
        }

        let mut reverse: BTreeMap<FieldRef, BTreeSet<FieldRef>> = eligible
            .iter()
            .cloned()
            .map(|node| (node, BTreeSet::new()))
            .collect();
        for source in eligible {
            for dependency in dependencies
                .get(source)
                .into_iter()
                .flatten()
                .filter(|dependency| eligible.contains(*dependency))
            {
                reverse
                    .get_mut(dependency)
                    .expect("eligible reverse node exists")
                    .insert(source.clone());
            }
        }

        let mut assigned = BTreeSet::new();
        let mut cycles = Vec::new();
        for root in finish_order.into_iter().rev() {
            if !assigned.insert(root.clone()) {
                continue;
            }
            let mut members = BTreeSet::new();
            let mut stack = vec![root];
            while let Some(node) = stack.pop() {
                members.insert(node.clone());
                for dependent in reverse[&node].iter().rev() {
                    if assigned.insert(dependent.clone()) {
                        stack.push(dependent.clone());
                    }
                }
            }
            let cyclic = members.len() > 1
                || members.first().is_some_and(|member| {
                    dependencies
                        .get(member)
                        .is_some_and(|targets| targets.contains(member))
                });
            if cyclic {
                cycles.push(members);
            }
        }
        cycles
    }

    pub fn affected_by(
        dependencies: &BTreeMap<FieldRef, BTreeSet<FieldRef>>,
        changed: &FieldRef,
    ) -> Vec<FieldRef> {
        let mut frontier = BTreeSet::from([changed.clone()]);
        let mut affected = BTreeSet::new();

        loop {
            let newly_affected: BTreeSet<_> = dependencies
                .iter()
                .filter(|(formula, inputs)| {
                    !affected.contains(*formula) && !inputs.is_disjoint(&frontier)
                })
                .map(|(formula, _)| formula.clone())
                .collect();
            if newly_affected.is_empty() {
                break;
            }
            frontier.clone_from(&newly_affected);
            affected.extend(newly_affected);
        }

        affected.into_iter().collect()
    }
}

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        let bound = u64::try_from(bound).expect("bounds fit u64");
        usize::try_from(self.next_u64() % bound).expect("remainder fits usize")
    }
}

fn number_field(id: &str) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(id),
        display_name: None,
        number_format: None,
        date_format: None,
        field_type: FieldType::Number,
        required: true,
        constraint: FieldConstraint::None,
    }
}

fn text_field(id: &str) -> FieldDefinition {
    FieldDefinition {
        field_type: FieldType::Text,
        ..number_field(id)
    }
}

fn generated_schema(key: &str) -> Schema {
    Schema {
        field_order: None,
        id: SchemaId::from(key),
        key: SchemaKey::from(key),
        display_name: None,
        fields: BTreeMap::from([
            (FieldId::from("name"), text_field("name")),
            (FieldId::from("c0"), number_field("c0")),
            (FieldId::from("c1"), number_field("c1")),
            (FieldId::from("c2"), number_field("c2")),
        ]),
    }
}

fn literal(value: f64) -> Expression {
    Expression::Number(Number::new(value).expect("generated literals are finite"))
}

fn same_row(field: &str) -> Expression {
    Expression::SameRowReference(FieldId::from(field))
}

fn column(schema: &str, field: &str) -> ColumnRef {
    ColumnRef::new(SchemaId::from(schema), FieldId::from(field))
}

fn total(schema: &str, field: &str) -> Expression {
    Expression::ColumnTotal(column(schema, field))
}

fn aggregate(function: ColumnAggregateFunction, schema: &str, field: &str) -> Expression {
    Expression::ColumnAggregate {
        function,
        column: column(schema, field),
    }
}

fn conditional_sum(
    schema: &str,
    field: &str,
    criterion_field: &str,
    value: Expression,
) -> Expression {
    Expression::ColumnConditional {
        aggregate: Box::new(ConditionalAggregate::Sum(column(schema, field))),
        criteria: vec![ColumnCriterion {
            column: column(schema, criterion_field),
            operator: ComparisonOperator::Equal,
            value: Some(Box::new(value)),
        }],
    }
}

fn add(left: Expression, right: Expression) -> Expression {
    Expression::Add {
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn divide(left: Expression, right: Expression) -> Expression {
    Expression::Divide {
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn subtract(left: Expression, right: Expression) -> Expression {
    Expression::Subtract {
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn generated_row(random: &mut SplitMix64, schema: &str, index: usize) -> Entity {
    let entity_id = format!("{schema}-{index:02}");
    let mut fields = BTreeMap::new();
    fields.insert(FieldId::from("name"), Value::Text(entity_id.clone()));
    match random.below(3) {
        0 => {
            let value = f64::from(u32::try_from(random.below(8) + 1).expect("small bound"));
            fields.insert(
                FieldId::from("c0"),
                Value::Number(Number::new(value).expect("finite number")),
            );
        }
        1 => {}
        _ => {
            fields.insert(FieldId::from("c0"), Value::Text("not a number".to_owned()));
        }
    }
    let c1 = match index % 7 {
        0 => divide(same_row("c0"), total(schema, "c0")),
        1 => conditional_sum(schema, "c0", "c0", same_row("c0")),
        2 => conditional_sum(schema, "c0", "c1", literal(2.0)),
        3 => Expression::If {
            condition: Box::new(Expression::IsBlank(Box::new(same_row("c0")))),
            then_value: Box::new(aggregate(ColumnAggregateFunction::Average, schema, "c1")),
            else_value: Box::new(add(
                aggregate(ColumnAggregateFunction::Maximum, schema, "c2"),
                aggregate(ColumnAggregateFunction::Minimum, schema, "c0"),
            )),
        },
        4 => Expression::Lookup {
            key: Box::new(same_row("c0")),
            key_column: column(schema, "c0"),
            result_column: column(schema, "c0"),
            if_missing: None,
        },
        5 => Expression::Lookup {
            key: Box::new(literal(99.0)),
            key_column: column(schema, "c0"),
            result_column: column(schema, "c0"),
            if_missing: Some(Box::new(divide(literal(1.0), literal(0.0)))),
        },
        _ => add(
            aggregate(ColumnAggregateFunction::Count, schema, "c0"),
            aggregate(ColumnAggregateFunction::Count, schema, "c2"),
        ),
    };
    let c1 = match index % 3 {
        0 => Expression::Abs(Box::new(c1)),
        1 => Expression::RoundUp {
            value: Box::new(c1),
            digits: 1,
        },
        _ => Expression::RoundDown {
            value: Box::new(c1),
            digits: 1,
        },
    };
    let c1 = Expression::IfError {
        value: Box::new(c1),
        fallback: Box::new(literal(-1.0)),
    };
    let c2 = if schema == BETA && index == 0 {
        total(schema, "c2")
    } else {
        match index % 3 {
            0 => total(schema, "c0"),
            1 => total(schema, "c1"),
            _ => add(
                Expression::Reference(FieldRef::new(entity_id.clone(), FieldId::from("c1"))),
                literal(1.0),
            ),
        }
    };
    fields.insert(FieldId::from("c1"), Value::Formula(c1));
    fields.insert(FieldId::from("c2"), Value::Formula(c2));
    Entity {
        id: EntityId::from(entity_id.as_str()),
        key: EntityKey::from(entity_id.as_str()),
        schema: SchemaId::from(schema),
        fields,
    }
}

fn generated_document(seed: u64) -> Document {
    let mut random = SplitMix64::new(seed);
    let mut entities = BTreeMap::new();
    for schema in [ALPHA, BETA] {
        let rows = 2 + random.below(4);
        for index in 0..rows {
            let row = generated_row(&mut random, schema, index);
            entities.insert(row.id.clone(), row);
        }
    }
    // Drop a row now and then, so column membership differs from a full grid.
    if random.below(3) == 0 {
        let victim = format!("{ALPHA}-00");
        entities.remove(&EntityId::from(victim.as_str()));
    }
    Document {
        id: DocumentId::from("column-graph-differential"),
        title: "Column graph differential".to_owned(),
        schemas: BTreeMap::from([
            (SchemaId::from(ALPHA), generated_schema(ALPHA)),
            (SchemaId::from(BETA), generated_schema(BETA)),
        ]),
        entities,
        keyed_grouped_sum_definitions: BTreeMap::new(),
    }
}

fn all_fields(document: &Document) -> BTreeSet<FieldRef> {
    let mut fields = BTreeSet::new();
    for (entity_id, entity) in &document.entities {
        for field_id in entity.fields.keys() {
            fields.insert(FieldRef::new(entity_id.clone(), field_id.clone()));
        }
        for field_id in document.schemas[&entity.schema].fields.keys() {
            fields.insert(FieldRef::new(entity_id.clone(), field_id.clone()));
        }
    }
    fields
}

fn assert_matches_reference(document: &Document, label: impl std::fmt::Display) {
    let expected = reference::calculate(document);
    let actual = super::full_calculation_parts(document);
    assert_eq!(&actual.values, &expected.values, "values of {label}");
    assert_eq!(&actual.failures, &expected.failures, "failures of {label}");
    assert_eq!(
        actual.graph.expanded_dependencies(),
        expected.dependencies,
        "expanded dependencies of {label}"
    );

    let outcome = calculate_complete(document);
    match (&outcome, expected.failures.is_empty()) {
        (CalculationOutcome::Complete(_), true) | (CalculationOutcome::Failed(_), false) => {}
        _ => panic!("outcome class disagrees with the reference for {label}"),
    }
    match &outcome {
        CalculationOutcome::Complete(calculation) => {
            assert_eq!(&calculation.dependencies(), &expected.dependencies);
        }
        CalculationOutcome::Failed(failures) => {
            assert_eq!(&failures.dependencies(), &expected.dependencies);
            assert_eq!(failures.failures(), &expected.failures);
        }
    }

    let mut fields = all_fields(document);
    fields.extend(expected.dependencies.keys().cloned());
    for dependencies in expected.dependencies.values() {
        fields.extend(dependencies.iter().cloned());
    }
    for field in fields {
        assert_eq!(
            actual.graph.affected_by(&field),
            reference::affected_by(&expected.dependencies, &field),
            "affected_by({field:?}) of {label}"
        );
    }
}

#[test]
fn generated_column_documents_match_the_expanded_reference() {
    for seed in 0..48_u64 {
        let document = generated_document(0x5eed_0957_1a2b_3c4d ^ seed);
        assert_matches_reference(&document, seed);
    }
}

fn projection_update_document() -> Document {
    fixed_document(
        vec![fixed_schema(
            "r",
            &[
                ("input", FieldType::Number),
                ("output", FieldType::Number),
                ("constant", FieldType::Number),
                ("added", FieldType::Number),
                ("stable", FieldType::Number),
            ],
        )],
        vec![fixed_row(
            "r",
            "r",
            &[
                ("input", fixed_number(2.0)),
                (
                    "output",
                    Value::Formula(divide(literal(8.0), same_row("input"))),
                ),
                ("constant", Value::Formula(literal(7.0))),
                ("added", fixed_number(1.0)),
                ("stable", Value::Formula(literal(9.0))),
            ],
        )],
    )
}

#[test]
fn retained_scalar_edit_reports_only_the_changed_formula_projection() {
    let mut document = projection_update_document();
    let (mut retained, _) = RetainedCalculationState::rebuild(&document);
    document
        .entities
        .get_mut("r")
        .unwrap()
        .fields
        .insert(FieldId::from("input"), fixed_number(4.0));
    let roots = BTreeSet::from([field_ref("r", "input")]);
    let transition = retained.update(&document, &roots);
    assert_eq!(
        transition.changed_calculation_projections,
        vec![field_ref("r", "output")]
    );
    assert_eq!(
        retained.value(&field_ref("r", "output")),
        Number::new(2.0).ok()
    );
    assert_eq!(
        retained
            .update(&document, &roots)
            .changed_calculation_projections,
        Vec::<FieldRef>::new()
    );
}

#[test]
fn retained_formula_root_removal_and_installation_report_changed_projections() {
    let mut document = projection_update_document();
    let (mut retained, _) = RetainedCalculationState::rebuild(&document);
    let roots = BTreeSet::from([field_ref("r", "output")]);
    for value in [
        fixed_number(6.0),
        Value::Formula(divide(literal(8.0), same_row("input"))),
    ] {
        document
            .entities
            .get_mut("r")
            .unwrap()
            .fields
            .insert(FieldId::from("output"), value);
        assert_eq!(
            retained
                .update(&document, &roots)
                .changed_calculation_projections,
            vec![field_ref("r", "output")]
        );
    }
}

#[test]
fn retained_constant_formula_root_changes_report_by_membership() {
    let mut document = projection_update_document();
    let (mut retained, _) = RetainedCalculationState::rebuild(&document);
    let roots = BTreeSet::from([field_ref("r", "constant")]);
    for value in [fixed_number(3.0), Value::Formula(literal(7.0))] {
        document
            .entities
            .get_mut("r")
            .unwrap()
            .fields
            .insert(FieldId::from("constant"), value);
        assert_eq!(
            retained
                .update(&document, &roots)
                .changed_calculation_projections,
            vec![field_ref("r", "constant")]
        );
    }
    for value in [fixed_number(7.0), Value::Formula(literal(7.0))] {
        document
            .entities
            .get_mut("r")
            .unwrap()
            .fields
            .insert(FieldId::from("constant"), value);
        assert_eq!(
            retained
                .update(&document, &roots)
                .changed_calculation_projections,
            Vec::<FieldRef>::new()
        );
    }
}

#[test]
fn retained_failure_flips_report_the_whole_before_and_after_formula_union() {
    let mut document = projection_update_document();
    let (mut retained, _) = RetainedCalculationState::rebuild(&document);
    let roots = BTreeSet::from([
        field_ref("r", "input"),
        field_ref("r", "constant"),
        field_ref("r", "added"),
    ]);
    for (input, constant, added, failed) in [
        (0.0, fixed_number(7.0), Value::Formula(literal(1.0)), true),
        (2.0, Value::Formula(literal(7.0)), fixed_number(1.0), false),
    ] {
        document.entities.get_mut("r").unwrap().fields.extend([
            (FieldId::from("input"), fixed_number(input)),
            (FieldId::from("constant"), constant),
            (FieldId::from("added"), added),
        ]);
        let transition = retained.update(&document, &roots);
        assert_eq!(retained.is_failed(), failed);
        assert_eq!(
            transition.changed_calculation_projections,
            vec![
                field_ref("r", "added"),
                field_ref("r", "constant"),
                field_ref("r", "output"),
                field_ref("r", "stable"),
            ]
        );
    }
}

#[test]
fn retained_promoted_column_total_root_reports_changes_and_removal() {
    let mut document = direct_and_column_document();
    let (mut retained, _) = RetainedCalculationState::rebuild(&document);
    let roots = change_direct_cell(&mut document);
    let total = field_ref("r-both", "x0");
    assert!(!roots.contains(&total));
    assert_eq!(
        retained
            .update(&document, &roots)
            .changed_calculation_projections,
        vec![field_ref("calc-1", "a"), total.clone()]
    );
    document
        .entities
        .get_mut("r-both")
        .unwrap()
        .fields
        .insert(FieldId::from("x0"), fixed_number(10.0));
    assert_eq!(
        retained
            .update(&document, &roots)
            .changed_calculation_projections,
        vec![total]
    );
}

#[test]
fn retained_update_matches_the_expanded_reference_after_row_edits() {
    for seed in 0..16_u64 {
        let mut document = generated_document(0x00c0_ffee_1234_5678 ^ seed);
        let (mut retained, _) = RetainedCalculationState::rebuild(&document);
        let rows = document.entities.keys().cloned().collect::<Vec<_>>();
        for (step, row) in rows.iter().enumerate() {
            let mut roots = BTreeSet::new();
            {
                let entity = document.entities.get_mut(row).expect("row exists");
                if step % 2 == 0 {
                    entity.fields.remove(&FieldId::from("c0"));
                } else {
                    let value = f64::from(u32::try_from(step + 1).expect("small step"));
                    entity.fields.insert(
                        FieldId::from("c0"),
                        Value::Number(Number::new(value).expect("finite")),
                    );
                }
                for field_id in entity.fields.keys() {
                    roots.insert(FieldRef::new(row.clone(), field_id.clone()));
                }
            }
            roots.insert(FieldRef::new(row.clone(), FieldId::from("c0")));
            let _ = retained.update(&document, &roots);
            let expected = reference::calculate(&document);
            assert_eq!(
                &retained.dependencies(),
                &expected.dependencies,
                "seed {seed} step {step} dependencies"
            );
            match (retained.outcome(), expected.failures.is_empty()) {
                (CalculationOutcome::Complete(calculation), true) => {
                    assert_eq!(calculation.values(), &expected.values);
                }
                (CalculationOutcome::Failed(failures), false) => {
                    assert_eq!(failures.failures(), &expected.failures);
                }
                _ => panic!("retained outcome class disagrees at seed {seed} step {step}"),
            }
        }
    }
}

/// Move every row between two schemas sharing a column, both directions, for a
/// row holding a number, a formula, or a blank in that column.
#[test]
fn retained_update_matches_the_expanded_reference_after_schema_moves() {
    for (from, to) in [("a", "b"), ("b", "a")] {
        for mover_value in [
            Some(Value::Number(Number::new(5.0).expect("finite"))),
            Some(Value::Formula(literal(5.0))),
            None,
        ] {
            let row = |id: &str, schema: &str, fields: BTreeMap<FieldId, Value>| Entity {
                id: EntityId::from(id),
                key: EntityKey::from(id),
                schema: SchemaId::from(schema),
                fields,
            };
            let mut mover_fields = BTreeMap::from([(
                FieldId::from("c1"),
                Value::Number(Number::new(1.0).expect("finite")),
            )]);
            if let Some(value) = mover_value {
                mover_fields.insert(FieldId::from("c0"), value);
            }
            let summary = |schema: &str| {
                BTreeMap::from([(FieldId::from("c1"), Value::Formula(total(schema, "c0")))])
            };
            let mut document = Document {
                schemas: BTreeMap::from([
                    (SchemaId::from("a"), generated_schema("a")),
                    (SchemaId::from("b"), generated_schema("b")),
                ]),
                entities: BTreeMap::from([
                    (EntityId::from("mover"), row("mover", from, mover_fields)),
                    (
                        EntityId::from("sum-a"),
                        row("sum-a", "summary", summary("a")),
                    ),
                    (
                        EntityId::from("sum-b"),
                        row("sum-b", "summary", summary("b")),
                    ),
                ]),
                ..generated_document(0)
            };
            document
                .schemas
                .insert(SchemaId::from("summary"), generated_schema("summary"));
            let (mut retained, _) = RetainedCalculationState::rebuild(&document);
            document
                .entities
                .get_mut(&EntityId::from("mover"))
                .expect("row exists")
                .schema = SchemaId::from(to);
            let roots = document.entities[&EntityId::from("mover")]
                .fields
                .keys()
                .map(|field| FieldRef::new(EntityId::from("mover"), field.clone()))
                .collect::<BTreeSet<_>>();
            let _ = retained.update(&document, &roots);
            let expected = reference::calculate(&document);
            assert_eq!(
                retained.dependencies(),
                expected.dependencies,
                "{from} to {to}"
            );
            match retained.outcome() {
                CalculationOutcome::Complete(calculation) => {
                    assert_eq!(calculation.values(), &expected.values, "{from} to {to}");
                }
                CalculationOutcome::Failed(failures) => {
                    assert_eq!(failures.failures(), &expected.failures, "{from} to {to}");
                }
            }
        }
    }
}

fn linear_document(rows: usize) -> Document {
    let mut entities = BTreeMap::new();
    for index in 0..rows {
        let entity_id = format!("row-{index:05}");
        let value = f64::from(u32::try_from(index % 97 + 1).expect("small remainder"));
        entities.insert(
            EntityId::from(entity_id.as_str()),
            Entity {
                id: EntityId::from(entity_id.as_str()),
                key: EntityKey::from(entity_id.as_str()),
                schema: SchemaId::from("row"),
                fields: BTreeMap::from([
                    (
                        FieldId::from("c0"),
                        Value::Number(Number::new(value).expect("finite")),
                    ),
                    (
                        FieldId::from("c1"),
                        Value::Formula(Expression::ColumnTotal(ColumnRef::new(
                            SchemaId::from("row"),
                            FieldId::from("c0"),
                        ))),
                    ),
                ]),
            },
        );
    }
    Document {
        id: DocumentId::from("linear-column-graph"),
        title: "Linear column graph".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("row"),
            Schema {
                field_order: None,
                id: SchemaId::from("row"),
                key: SchemaKey::from("row"),
                display_name: None,
                fields: BTreeMap::from([
                    (FieldId::from("c0"), number_field("c0")),
                    (FieldId::from("c1"), number_field("c1")),
                ]),
            },
        )]),
        entities,
        keyed_grouped_sum_definitions: BTreeMap::new(),
    }
}

#[test]
fn a_column_reading_formula_per_row_keeps_graph_memory_linear() {
    const ROWS: usize = 2_000;
    let document = linear_document(ROWS);
    let (retained, _) = RetainedCalculationState::rebuild(&document);
    let entries = retained.graph_entry_count();
    let formulas = ROWS;
    let slots = ROWS;
    assert!(
        entries <= 4 * (formulas + slots),
        "graph holds {entries} entries for {ROWS} rows"
    );
    assert!(entries > 0);
}

#[test]
fn a_column_reading_formula_per_row_walks_each_column_a_bounded_number_of_times() {
    const ROWS: usize = 2_000;
    let document = linear_document(ROWS);
    // The expanded pipeline walks the column once per formula in phase 1c and
    // again per formula in evaluation: 4,000 walks for these rows.
    crate::column_walk_counter::take();
    let outcome = calculate_complete(&document);
    let walks = crate::column_walk_counter::take();
    assert!(
        matches!(outcome, CalculationOutcome::Complete(_)),
        "the linear document must calculate"
    );
    assert!(walks > 0, "the counter must observe the column reads");
    assert!(
        walks <= 4,
        "{ROWS} rows walked column cells {walks} times in one calculation"
    );
}

#[test]
fn a_column_total_adds_in_entity_order_bit_for_bit() {
    let document = order_dependent_total_document();
    let values = fixed_values(&document);
    let total = values[&field_ref("row-sum", "total")].get();
    let entity_order = [1e16_f64, 1.0, -1e16]
        .into_iter()
        .fold(0.0_f64, |sum, value| sum + value);
    let regrouped = [1e16_f64, -1e16, 1.0]
        .into_iter()
        .fold(0.0_f64, |sum, value| sum + value);
    assert_eq!(
        entity_order.to_bits(),
        0.0_f64.to_bits(),
        "the chosen values lose the 1.0 in entity order"
    );
    assert_ne!(
        entity_order.to_bits(),
        regrouped.to_bits(),
        "the chosen values make the sum order-dependent"
    );
    assert_eq!(
        total.to_bits(),
        entity_order.to_bits(),
        "the total adds the cells in entity order"
    );
}

/// The aggregates one number column answers, folded here over the cells in
/// entity order with no engine code: the differential's independent arithmetic
/// reference for SUM, AVERAGE, COUNT, MIN, MAX and a conditional SUM (#716).
struct ReferenceAggregates {
    sum: f64,
    average: f64,
    count: f64,
    minimum: f64,
    maximum: f64,
    conditional_sum: f64,
}

/// Fold `value_column`'s present cells in entity order, counting the rows whose
/// `criterion_column` equals `criterion` exactly.
fn reference_aggregates(
    document: &Document,
    value_column: &ColumnRef,
    criterion_column: &ColumnRef,
    criterion: f64,
) -> ReferenceAggregates {
    let mut count = 0_usize;
    let mut sum = 0.0_f64;
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    let mut conditional_sum = 0.0_f64;
    for entity in document
        .entities
        .values()
        .filter(|entity| entity.schema == value_column.schema)
    {
        let Some(Value::Number(value)) = entity.fields.get(&value_column.field) else {
            continue;
        };
        let value = value.get();
        count += 1;
        sum += value;
        minimum = minimum.min(value);
        maximum = maximum.max(value);
        let matches = entity
            .fields
            .get(&criterion_column.field)
            .is_some_and(|cell| match cell {
                Value::Number(number) => number.get().to_bits() == criterion.to_bits(),
                _ => false,
            });
        if matches {
            conditional_sum += value;
        }
    }
    let count = f64::from(u32::try_from(count).expect("small reference column"));
    ReferenceAggregates {
        sum,
        average: sum / count,
        count,
        minimum,
        maximum,
        conditional_sum,
    }
}

/// Halves and small integers: each running sum is exact, so a plain fold and
/// the engine's exactly rounded sum must agree bit for bit.
fn reference_aggregate_document() -> Document {
    fixed_document(
        vec![
            fixed_schema(
                "calc",
                &[("v", FieldType::Number), ("k", FieldType::Number)],
            ),
            fixed_schema(
                "readers",
                &[
                    ("total", FieldType::Number),
                    ("average", FieldType::Number),
                    ("count", FieldType::Number),
                    ("minimum", FieldType::Number),
                    ("maximum", FieldType::Number),
                    ("conditional", FieldType::Number),
                ],
            ),
        ],
        vec![
            fixed_row(
                "calc-0",
                "calc",
                &[("v", fixed_number(0.5)), ("k", fixed_number(1.0))],
            ),
            fixed_row(
                "calc-1",
                "calc",
                &[("v", fixed_number(1.5)), ("k", fixed_number(2.0))],
            ),
            fixed_row(
                "calc-2",
                "calc",
                &[("v", fixed_number(2.0)), ("k", fixed_number(2.0))],
            ),
            fixed_row(
                "calc-3",
                "calc",
                &[("v", fixed_number(4.0)), ("k", fixed_number(3.0))],
            ),
            // A blank value in a row the criterion still matches.
            fixed_row("calc-4", "calc", &[("k", fixed_number(2.0))]),
            fixed_row(
                "reader",
                "readers",
                &[
                    ("total", Value::Formula(total("calc", "v"))),
                    (
                        "average",
                        Value::Formula(aggregate(ColumnAggregateFunction::Average, "calc", "v")),
                    ),
                    (
                        "count",
                        Value::Formula(aggregate(ColumnAggregateFunction::Count, "calc", "v")),
                    ),
                    (
                        "minimum",
                        Value::Formula(aggregate(ColumnAggregateFunction::Minimum, "calc", "v")),
                    ),
                    (
                        "maximum",
                        Value::Formula(aggregate(ColumnAggregateFunction::Maximum, "calc", "v")),
                    ),
                    (
                        "conditional",
                        Value::Formula(conditional_sum("calc", "v", "k", literal(2.0))),
                    ),
                ],
            ),
        ],
    )
}

#[test]
fn the_reference_fold_agrees_with_every_column_aggregate_bit_for_bit() {
    let document = reference_aggregate_document();
    let reference =
        reference_aggregates(&document, &column("calc", "v"), &column("calc", "k"), 2.0);
    let values = fixed_values(&document);
    let reader = |name: &str| values[&field_ref("reader", name)].get();
    assert_eq!(
        reader("total").to_bits(),
        reference.sum.to_bits(),
        "SUM folds the cells in entity order"
    );
    assert_eq!(
        reader("average").to_bits(),
        reference.average.to_bits(),
        "AVERAGE is the exact sum over the count"
    );
    assert_eq!(
        reader("count").to_bits(),
        reference.count.to_bits(),
        "COUNT counts the present cells"
    );
    assert_eq!(
        reader("minimum").to_bits(),
        reference.minimum.to_bits(),
        "MIN folds the present cells"
    );
    assert_eq!(
        reader("maximum").to_bits(),
        reference.maximum.to_bits(),
        "MAX folds the present cells"
    );
    assert_eq!(
        reader("conditional").to_bits(),
        reference.conditional_sum.to_bits(),
        "the conditional SUM adds the matching rows in entity order"
    );
    assert_eq!(reference.count.to_bits(), 4.0_f64.to_bits());
    assert_eq!(reference.conditional_sum.to_bits(), 3.5_f64.to_bits());
    assert_matches_reference(&document, "the reference aggregates");
}

fn field_ref(entity: &str, name: &str) -> FieldRef {
    FieldRef::new(EntityId::from(entity), FieldId::from(name))
}

fn fixed_number(value: f64) -> Value {
    Value::Number(Number::new(value).expect("fixed literals are finite"))
}

fn fixed_schema(key: &str, fields: &[(&str, FieldType)]) -> Schema {
    Schema {
        field_order: None,
        id: SchemaId::from(key),
        key: SchemaKey::from(key),
        display_name: None,
        fields: fields
            .iter()
            .map(|(id, field_type)| {
                let definition = number_field(id);
                (
                    FieldId::from(*id),
                    FieldDefinition {
                        field_type: field_type.clone(),
                        ..definition
                    },
                )
            })
            .collect(),
    }
}

fn fixed_row(id: &str, schema: &str, entries: &[(&str, Value)]) -> Entity {
    Entity {
        id: EntityId::from(id),
        key: EntityKey::from(id),
        schema: SchemaId::from(schema),
        fields: entries
            .iter()
            .map(|(name, value)| (FieldId::from(*name), value.clone()))
            .collect(),
    }
}

fn fixed_document(schemas: Vec<Schema>, entities: Vec<Entity>) -> Document {
    Document {
        id: DocumentId::from("fixed-column-cases"),
        title: "Fixed column cases".to_owned(),
        schemas: schemas
            .into_iter()
            .map(|schema| (schema.id.clone(), schema))
            .collect(),
        entities: entities
            .into_iter()
            .map(|entity| (entity.id.clone(), entity))
            .collect(),
        keyed_grouped_sum_definitions: BTreeMap::new(),
    }
}

fn fixed_failures(document: &Document) -> BTreeMap<FieldRef, CalculationFailure> {
    match calculate_complete(document) {
        CalculationOutcome::Complete(_) => BTreeMap::new(),
        CalculationOutcome::Failed(failures) => failures.failures().clone(),
    }
}

fn fixed_values(document: &Document) -> BTreeMap<FieldRef, Number> {
    match calculate_complete(document) {
        CalculationOutcome::Complete(calculation) => calculation.values().clone(),
        CalculationOutcome::Failed(failures) => {
            panic!(
                "expected a complete calculation, got {} failures",
                failures.failures().len()
            )
        }
    }
}

fn equal(left: Expression, right: Expression) -> Expression {
    Expression::Compare {
        operator: ComparisonOperator::Equal,
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn failed_expression() -> Value {
    Value::Formula(divide(literal(1.0), literal(0.0)))
}

/// A column holding a failed formula member, read by a total, every aggregate,
/// and a conditional aggregate as its value column and as a criterion column.
fn failed_member_document() -> Document {
    let mut rows = vec![
        fixed_row(
            "calc-1",
            "calc",
            &[("f", failed_expression()), ("v", fixed_number(2.0))],
        ),
        fixed_row(
            "calc-2",
            "calc",
            &[("f", fixed_number(2.0)), ("v", fixed_number(4.0))],
        ),
        fixed_row(
            "calc-3",
            "calc",
            &[
                ("f", Value::Formula(add(literal(1.0), literal(1.0)))),
                ("v", fixed_number(5.0)),
            ],
        ),
    ];
    let readers = [
        ("read-sum", total("calc", "f")),
        (
            "read-average",
            aggregate(ColumnAggregateFunction::Average, "calc", "f"),
        ),
        (
            "read-count",
            aggregate(ColumnAggregateFunction::Count, "calc", "f"),
        ),
        (
            "read-minimum",
            aggregate(ColumnAggregateFunction::Minimum, "calc", "f"),
        ),
        (
            "read-maximum",
            aggregate(ColumnAggregateFunction::Maximum, "calc", "f"),
        ),
        (
            "read-sumif-value",
            conditional_sum("calc", "f", "v", literal(2.0)),
        ),
        (
            "read-sumif-criterion",
            conditional_sum("calc", "v", "f", literal(2.0)),
        ),
    ];
    for (id, expression) in readers {
        rows.push(fixed_row(
            id,
            "readers",
            &[("x0", Value::Formula(expression))],
        ));
    }
    let document = fixed_document(
        vec![
            fixed_schema(
                "calc",
                &[("f", FieldType::Number), ("v", FieldType::Number)],
            ),
            fixed_schema("readers", &[("x0", FieldType::Number)]),
        ],
        rows,
    );
    let failures = fixed_failures(&document);
    assert_eq!(
        failures.get(&field_ref("calc-1", "f")),
        Some(&CalculationFailure::DivisionByZero),
        "case: a failed formula member of a read column"
    );
    for id in [
        "read-sum",
        "read-average",
        "read-count",
        "read-minimum",
        "read-maximum",
        "read-sumif-value",
        "read-sumif-criterion",
    ] {
        assert_eq!(
            failures.get(&field_ref(id, "x0")),
            Some(&CalculationFailure::FailedDependencies {
                dependencies: BTreeSet::from([field_ref("calc-1", "f")]),
            }),
            "case: {id} reads a column with a failed member"
        );
    }
    document
}

fn resolve_failed_member(document: &mut Document) -> BTreeSet<FieldRef> {
    document
        .entities
        .get_mut(&EntityId::from("calc-1"))
        .expect("row exists")
        .fields
        .insert(FieldId::from("f"), fixed_number(5.0));
    BTreeSet::from([field_ref("calc-1", "f")])
}

/// The same failing total behind an IF branch that is not taken, and beside a
/// reader whose condition selects it.
fn if_branch_document() -> Document {
    let skipped = Expression::If {
        condition: Box::new(equal(literal(1.0), literal(1.0))),
        then_value: Box::new(literal(0.0)),
        else_value: Box::new(total("calc", "f")),
    };
    let selected = Expression::If {
        condition: Box::new(equal(literal(1.0), literal(2.0))),
        then_value: Box::new(literal(0.0)),
        else_value: Box::new(total("calc", "f")),
    };
    let document = fixed_document(
        vec![
            fixed_schema("calc", &[("f", FieldType::Number)]),
            fixed_schema("readers", &[("x0", FieldType::Number)]),
        ],
        vec![
            fixed_row("calc-1", "calc", &[("f", failed_expression())]),
            fixed_row("calc-2", "calc", &[("f", fixed_number(3.0))]),
            fixed_row("r-untaken", "readers", &[("x0", Value::Formula(skipped))]),
            fixed_row("r-taken", "readers", &[("x0", Value::Formula(selected))]),
        ],
    );
    let failures = fixed_failures(&document);
    let values = fixed_values_ignoring_failures(&document);
    assert!(
        failures.contains_key(&field_ref("calc-1", "f")),
        "case: a failed member of a column read inside an untaken IF branch"
    );
    assert!(
        !failures.contains_key(&field_ref("r-untaken", "x0")),
        "case: the untaken branch does not read the failing column"
    );
    assert_eq!(
        values
            .get(&field_ref("r-untaken", "x0"))
            .map(|number| number.get().to_bits()),
        Some(0.0_f64.to_bits()),
        "case: the untaken branch yields its literal"
    );
    assert_eq!(
        failures.get(&field_ref("r-taken", "x0")),
        Some(&CalculationFailure::FailedDependencies {
            dependencies: BTreeSet::from([field_ref("calc-1", "f")]),
        }),
        "case: flipping the condition reads the failing column"
    );
    document
}

/// A partial view of the value map even when the document has failures.
fn fixed_values_ignoring_failures(document: &Document) -> BTreeMap<FieldRef, Number> {
    let parts = super::full_calculation_parts(document);
    parts.values
}

fn flip_if_branch(document: &mut Document) -> BTreeSet<FieldRef> {
    let selected = Expression::If {
        condition: Box::new(equal(literal(1.0), literal(2.0))),
        then_value: Box::new(literal(0.0)),
        else_value: Box::new(total("calc", "f")),
    };
    document
        .entities
        .get_mut(&EntityId::from("r-untaken"))
        .expect("row exists")
        .fields
        .insert(FieldId::from("x0"), Value::Formula(selected));
    BTreeSet::from([field_ref("r-untaken", "x0")])
}

/// Two formulas in a cycle through a column, plus a reader of a cycle member.
fn cycle_document() -> Document {
    let document = fixed_document(
        vec![
            fixed_schema(
                "calc",
                &[("a", FieldType::Number), ("b", FieldType::Number)],
            ),
            fixed_schema("readers", &[("x0", FieldType::Number)]),
        ],
        vec![
            fixed_row(
                "calc-1",
                "calc",
                &[
                    ("a", Value::Formula(total("calc", "b"))),
                    (
                        "b",
                        Value::Formula(Expression::Reference(field_ref("calc-1", "a"))),
                    ),
                ],
            ),
            fixed_row(
                "r-cycle-member",
                "readers",
                &[("x0", Value::Formula(total("calc", "b")))],
            ),
        ],
    );
    let failures = fixed_failures(&document);
    for member in ["a", "b"] {
        assert!(
            matches!(
                failures.get(&field_ref("calc-1", member)),
                Some(CalculationFailure::Cycle { .. })
            ),
            "case: a cycle through a column reaches calc-1.{member}"
        );
    }
    assert_eq!(
        failures.get(&field_ref("r-cycle-member", "x0")),
        Some(&CalculationFailure::FailedDependencies {
            dependencies: BTreeSet::from([field_ref("calc-1", "b")]),
        }),
        "case: a reader of a cycle member fails on that member"
    );
    document
}

fn break_cycle(document: &mut Document) -> BTreeSet<FieldRef> {
    document
        .entities
        .get_mut(&EntityId::from("calc-1"))
        .expect("row exists")
        .fields
        .insert(FieldId::from("b"), fixed_number(2.0));
    BTreeSet::from([field_ref("calc-1", "b")])
}

/// One reader naming a cell directly and through the column holding it.
fn direct_and_column_document() -> Document {
    let document = fixed_document(
        vec![
            fixed_schema("calc", &[("a", FieldType::Number)]),
            fixed_schema("readers", &[("x0", FieldType::Number)]),
        ],
        vec![
            fixed_row(
                "calc-1",
                "calc",
                &[("a", Value::Formula(add(literal(1.0), literal(1.0))))],
            ),
            fixed_row("calc-2", "calc", &[("a", fixed_number(3.0))]),
            fixed_row(
                "r-both",
                "readers",
                &[(
                    "x0",
                    Value::Formula(add(
                        Expression::Reference(field_ref("calc-1", "a")),
                        total("calc", "a"),
                    )),
                )],
            ),
        ],
    );
    let reader = &document.entities[&EntityId::from("r-both")].fields[&FieldId::from("x0")];
    assert!(
        matches!(
            reader,
            Value::Formula(Expression::Add { left, right })
                if matches!(left.as_ref(), Expression::Reference(_))
                    && matches!(right.as_ref(), Expression::ColumnTotal(_))
        ),
        "case: one formula names the same cell directly and through its column"
    );
    assert_eq!(
        fixed_values(&document)[&field_ref("r-both", "x0")]
            .get()
            .to_bits(),
        7.0_f64.to_bits(),
        "case: the direct path and the column path both count"
    );
    document
}

fn change_direct_cell(document: &mut Document) -> BTreeSet<FieldRef> {
    document
        .entities
        .get_mut(&EntityId::from("calc-1"))
        .expect("row exists")
        .fields
        .insert(FieldId::from("a"), fixed_number(4.0));
    BTreeSet::from([field_ref("calc-1", "a")])
}

/// Three chained columns of formulas: each totals the column before it, so a
/// formula reads its own row through another column, and a reader in another
/// schema reads the last column.
fn chained_columns_document() -> Document {
    let mut rows = Vec::new();
    for index in 1..=3 {
        let id = format!("calc-{index}");
        rows.push(fixed_row(
            &id,
            "calc",
            &[
                ("c0", fixed_number(f64::from(index))),
                ("c1", Value::Formula(total("calc", "c0"))),
                ("c2", Value::Formula(total("calc", "c1"))),
                ("c3", Value::Formula(total("calc", "c2"))),
            ],
        ));
    }
    rows.push(fixed_row(
        "r-chain",
        "readers",
        &[("x0", Value::Formula(total("calc", "c3")))],
    ));
    let document = fixed_document(
        vec![
            fixed_schema(
                "calc",
                &[
                    ("c0", FieldType::Number),
                    ("c1", FieldType::Number),
                    ("c2", FieldType::Number),
                    ("c3", FieldType::Number),
                ],
            ),
            fixed_schema("readers", &[("x0", FieldType::Number)]),
        ],
        rows,
    );
    let values = fixed_values(&document);
    for (field, expected) in [
        ("c1", 6.0_f64),
        ("c2", 18.0_f64),
        ("c3", 54.0_f64),
        ("x0", 162.0_f64),
    ] {
        let entity = if field == "x0" { "r-chain" } else { "calc-1" };
        assert_eq!(
            values[&field_ref(entity, field)].get().to_bits(),
            expected.to_bits(),
            "case: the chained columns carry {expected} into {field}"
        );
    }
    document
}

fn change_chain_root(document: &mut Document) -> BTreeSet<FieldRef> {
    document
        .entities
        .get_mut(&EntityId::from("calc-2"))
        .expect("row exists")
        .fields
        .insert(FieldId::from("c0"), fixed_number(10.0));
    BTreeSet::from([field_ref("calc-2", "c0")])
}

/// A Text cell in a column a total reads: the total fails on the target checks
/// before evaluation, and an aggregate (not walked in phase 1c) fails on
/// `first_missing`.
fn text_cell_document() -> Document {
    let document = fixed_document(
        vec![
            fixed_schema("calc", &[("f", FieldType::Number)]),
            fixed_schema(
                "readers",
                &[("x0", FieldType::Number), ("x1", FieldType::Number)],
            ),
        ],
        vec![
            fixed_row("calc-1", "calc", &[("f", fixed_number(1.0))]),
            fixed_row("calc-2", "calc", &[("f", Value::Text("n/a".to_owned()))]),
            fixed_row("calc-3", "calc", &[("f", Value::Formula(literal(2.0)))]),
            fixed_row(
                "r-sum",
                "readers",
                &[("x0", Value::Formula(total("calc", "f")))],
            ),
            fixed_row(
                "r-average",
                "readers",
                &[(
                    "x0",
                    Value::Formula(aggregate(ColumnAggregateFunction::Average, "calc", "f")),
                )],
            ),
        ],
    );
    let failures = fixed_failures(&document);
    assert_eq!(
        failures.get(&field_ref("r-sum", "x0")),
        Some(&CalculationFailure::InvalidReferences {
            targets: BTreeMap::from([(field_ref("calc-2", "f"), ReferenceFailure::NonNumeric,)]),
        }),
        "case: a Text cell in a summed column fails the total"
    );
    assert_eq!(
        failures.get(&field_ref("r-average", "x0")),
        Some(&CalculationFailure::FailedDependencies {
            dependencies: BTreeSet::from([field_ref("calc-2", "f")]),
        }),
        "case: the aggregate reports the first missing cell"
    );
    document
}

fn resolve_text_cell(document: &mut Document) -> BTreeSet<FieldRef> {
    document
        .entities
        .get_mut(&EntityId::from("calc-2"))
        .expect("row exists")
        .fields
        .insert(FieldId::from("f"), fixed_number(4.0));
    BTreeSet::from([field_ref("calc-2", "f")])
}

/// Two readers of one column, one of them nested in arithmetic.
fn two_readers_document() -> Document {
    let document = fixed_document(
        vec![
            fixed_schema("calc", &[("v", FieldType::Number)]),
            fixed_schema("readers", &[("x0", FieldType::Number)]),
        ],
        vec![
            fixed_row("calc-1", "calc", &[("v", fixed_number(1.0))]),
            fixed_row("calc-2", "calc", &[("v", fixed_number(2.0))]),
            fixed_row(
                "r-direct",
                "readers",
                &[("x0", Value::Formula(total("calc", "v")))],
            ),
            fixed_row(
                "r-nested",
                "readers",
                &[(
                    "x0",
                    Value::Formula(divide(add(total("calc", "v"), literal(1.0)), literal(2.0))),
                )],
            ),
        ],
    );
    let values = fixed_values(&document);
    assert_eq!(
        values[&field_ref("r-direct", "x0")].get().to_bits(),
        3.0_f64.to_bits(),
        "case: the direct reader sums the column"
    );
    assert_eq!(
        values[&field_ref("r-nested", "x0")].get().to_bits(),
        2.0_f64.to_bits(),
        "case: the nested reader sums the same column"
    );
    document
}

fn change_two_reader_root(document: &mut Document) -> BTreeSet<FieldRef> {
    document
        .entities
        .get_mut(&EntityId::from("calc-1"))
        .expect("row exists")
        .fields
        .insert(FieldId::from("v"), fixed_number(5.0));
    BTreeSet::from([field_ref("calc-1", "v")])
}

fn order_dependent_total_document() -> Document {
    fixed_document(
        vec![
            fixed_schema("calc", &[("v", FieldType::Number)]),
            fixed_schema("sums", &[("total", FieldType::Number)]),
        ],
        vec![
            fixed_row("row-a", "calc", &[("v", fixed_number(1e16))]),
            fixed_row("row-b", "calc", &[("v", fixed_number(1.0))]),
            fixed_row("row-c", "calc", &[("v", fixed_number(-1e16))]),
            fixed_row(
                "row-sum",
                "sums",
                &[("total", Value::Formula(total("calc", "v")))],
            ),
        ],
    )
}

fn iferror_document(denominator: f64) -> Document {
    fixed_document(
        vec![fixed_schema(
            "r",
            &[
                ("a", FieldType::Number),
                ("b", FieldType::Number),
                ("result", FieldType::Number),
            ],
        )],
        vec![fixed_row(
            "r",
            "r",
            &[
                ("a", fixed_number(8.0)),
                ("b", fixed_number(denominator)),
                (
                    "result",
                    Value::Formula(Expression::IfError {
                        value: Box::new(divide(same_row("a"), same_row("b"))),
                        fallback: Box::new(literal(0.0)),
                    }),
                ),
            ],
        )],
    )
}

fn flip_iferror_denominator(document: &mut Document) -> BTreeSet<FieldRef> {
    let entity = document.entities.get_mut("r").expect("row exists");
    let Value::Number(previous) = &entity.fields["b"] else {
        panic!("denominator is a Number");
    };
    let next = if previous.get() == 0.0 { 2.0 } else { 0.0 };
    entity.fields.insert(FieldId::from("b"), fixed_number(next));
    BTreeSet::from([FieldRef::new("r", "b")])
}

/// A named document, and the retained edit applied to it after the full run.
type FixedCase = (
    &'static str,
    Document,
    fn(&mut Document) -> BTreeSet<FieldRef>,
);

#[test]
fn fixed_column_documents_match_the_expanded_reference() {
    let cases: Vec<FixedCase> = vec![
        (
            "IFERROR switches to fallback",
            iferror_document(2.0),
            flip_iferror_denominator,
        ),
        (
            "IFERROR switches back to value",
            iferror_document(0.0),
            flip_iferror_denominator,
        ),
        (
            "failed member read by totals and conditionals",
            failed_member_document(),
            resolve_failed_member,
        ),
        (
            "failed member behind an untaken IF branch",
            if_branch_document(),
            flip_if_branch,
        ),
        (
            "cycle through a column with a member reader",
            cycle_document(),
            break_cycle,
        ),
        (
            "a cell named directly and through its column",
            direct_and_column_document(),
            change_direct_cell,
        ),
        (
            "a chain of three formula columns",
            chained_columns_document(),
            change_chain_root,
        ),
        (
            "a Text cell in a summed column",
            text_cell_document(),
            resolve_text_cell,
        ),
        (
            "two readers of one column, one nested",
            two_readers_document(),
            change_two_reader_root,
        ),
    ];
    for (name, mut document, edit) in cases {
        assert_matches_reference(&document, name);

        let (mut retained, _) = RetainedCalculationState::rebuild(&document);
        let roots = edit(&mut document);
        let _ = retained.update(&document, &roots);
        let expected = reference::calculate(&document);
        assert_eq!(
            retained.dependencies(),
            expected.dependencies,
            "{name}: dependencies after a retained edit"
        );
        match retained.outcome() {
            CalculationOutcome::Complete(calculation) => {
                assert_eq!(
                    calculation.values(),
                    &expected.values,
                    "{name}: values after a retained edit"
                );
            }
            CalculationOutcome::Failed(failures) => {
                assert_eq!(
                    failures.failures(),
                    &expected.failures,
                    "{name}: failures after a retained edit"
                );
            }
        }

        assert_matches_reference(&document, name);
    }
}

/// A column whose formula `c0 / (c1 - c1)` fails in every row, read by a total
/// in every row. Readers sort before the calculated rows, so the first failure
/// in the map is a reader and `compatibility_error` has to follow its list.
fn failing_column_document(rows: usize) -> Document {
    let mut entities = Vec::with_capacity(rows * 2);
    for index in 0..rows {
        entities.push(fixed_row(
            &format!("a-{index:05}"),
            "readers",
            &[("total", Value::Formula(total("calc", "f")))],
        ));
        entities.push(fixed_row(
            &format!("z-{index:05}"),
            "calc",
            &[
                ("c0", fixed_number(1.0)),
                ("c1", fixed_number(2.0)),
                (
                    "f",
                    Value::Formula(divide(
                        same_row("c0"),
                        subtract(same_row("c1"), same_row("c1")),
                    )),
                ),
            ],
        ));
    }
    fixed_document(
        vec![
            fixed_schema(
                "calc",
                &[
                    ("c0", FieldType::Number),
                    ("c1", FieldType::Number),
                    ("f", FieldType::Number),
                ],
            ),
            fixed_schema("readers", &[("total", FieldType::Number)]),
        ],
        entities,
    )
}

/// A Number column holding Text in every row, read by a total in every row.
fn text_column_document(rows: usize) -> Document {
    let mut entities = Vec::with_capacity(rows * 2);
    for index in 0..rows {
        entities.push(fixed_row(
            &format!("a-{index:05}"),
            "readers",
            &[("total", Value::Formula(total("calc", "f")))],
        ));
        entities.push(fixed_row(
            &format!("z-{index:05}"),
            "calc",
            &[("f", Value::Text("n/a".to_owned()))],
        ));
    }
    fixed_document(
        vec![
            fixed_schema("calc", &[("f", FieldType::Number)]),
            fixed_schema("readers", &[("total", FieldType::Number)]),
        ],
        entities,
    )
}

#[test]
fn a_failed_column_names_its_first_failed_cell_in_every_reader() {
    const ROWS: usize = 2_000;
    let document = failing_column_document(ROWS);
    let failures = fixed_failures(&document);
    let first = field_ref("z-00000", "f");
    let mut named = 0_usize;
    for index in 0..ROWS {
        let reader = field_ref(&format!("a-{index:05}"), "total");
        let Some(CalculationFailure::FailedDependencies { dependencies }) = failures.get(&reader)
        else {
            panic!("reader {reader} must fail on dependencies");
        };
        assert_eq!(
            dependencies,
            &BTreeSet::from([first.clone()]),
            "reader {reader} names the column's first failed cell"
        );
        named += dependencies.len();
    }
    assert_eq!(named, ROWS, "every reader names exactly one failed cell");
    match super::calculate(&document) {
        Err(error) => assert_eq!(
            error,
            CalculationError::DivisionByZero {
                formula: first.clone(),
            },
            "compatibility_error names the same cell the full list started with"
        ),
        Ok(_) => panic!("the document must fail"),
    }
}

#[test]
fn a_text_column_names_its_first_target_in_every_reader() {
    const ROWS: usize = 2_000;
    let document = text_column_document(ROWS);
    let failures = fixed_failures(&document);
    let first = field_ref("z-00000", "f");
    let mut named = 0_usize;
    for index in 0..ROWS {
        let reader = field_ref(&format!("a-{index:05}"), "total");
        let Some(CalculationFailure::InvalidReferences { targets }) = failures.get(&reader) else {
            panic!("reader {reader} must fail on invalid references");
        };
        assert_eq!(
            targets,
            &BTreeMap::from([(first.clone(), ReferenceFailure::NonNumeric)]),
            "reader {reader} names the column's first failed target"
        );
        named += targets.len();
    }
    assert_eq!(named, ROWS, "every reader names exactly one target");
    match super::calculate(&document) {
        Err(error) => assert_eq!(
            error,
            CalculationError::NonNumericReference {
                reference: first.clone(),
            },
            "compatibility_error names the same cell the full list started with"
        ),
        Ok(_) => panic!("the document must fail"),
    }
}

#[test]
fn the_expanded_reference_names_the_same_first_failed_cell() {
    assert_matches_reference(&failing_column_document(4), "a failed column read by rows");
    assert_matches_reference(&text_column_document(4), "a Text column read by rows");
}

/// One row's own failed cell beside a column that fails in every row.
fn own_and_column_failure_document() -> Document {
    let mut entities = Vec::new();
    for index in 0..3 {
        entities.push(fixed_row(
            &format!("calc-{index}"),
            "calc",
            &[
                ("c7", failed_expression()),
                ("c9", failed_expression()),
                (
                    "out",
                    Value::Formula(add(same_row("c7"), total("calc", "c9"))),
                ),
                (
                    "again",
                    Value::Formula(add(
                        Expression::Reference(field_ref("calc-0", "c9")),
                        total("calc", "c9"),
                    )),
                ),
            ],
        ));
    }
    fixed_document(
        vec![fixed_schema(
            "calc",
            &[
                ("c7", FieldType::Number),
                ("c9", FieldType::Number),
                ("out", FieldType::Number),
                ("again", FieldType::Number),
            ],
        )],
        entities,
    )
}

#[test]
fn a_formula_keeps_its_own_reference_beside_a_columns_first_failure() {
    let document = own_and_column_failure_document();
    let failures = fixed_failures(&document);
    let first_c9 = field_ref("calc-0", "c9");
    for index in 0..3 {
        let own = field_ref(&format!("calc-{index}"), "c7");
        assert_eq!(
            failures.get(&field_ref(&format!("calc-{index}"), "out")),
            Some(&CalculationFailure::FailedDependencies {
                dependencies: BTreeSet::from([own, first_c9.clone()]),
            }),
            "row {index} keeps its own failed cell beside the column's first"
        );
        assert_eq!(
            failures.get(&field_ref(&format!("calc-{index}"), "again")),
            Some(&CalculationFailure::FailedDependencies {
                dependencies: BTreeSet::from([first_c9.clone()]),
            }),
            "row {index} names the first c9 cell once, direct and through the column"
        );
    }
    assert_matches_reference(&document, "own reference beside a failed column");
}

fn resolve_first_failed_cell(document: &mut Document) -> BTreeSet<FieldRef> {
    document
        .entities
        .get_mut(&EntityId::from("calc-0"))
        .expect("row exists")
        .fields
        .insert(FieldId::from("c9"), fixed_number(4.0));
    BTreeSet::from([field_ref("calc-0", "c9")])
}

#[test]
fn a_retained_edit_unfailing_the_first_cell_names_the_next_one() {
    let mut document = own_and_column_failure_document();
    let (mut retained, _) = RetainedCalculationState::rebuild(&document);
    let roots = resolve_first_failed_cell(&mut document);
    let _ = retained.update(&document, &roots);
    let expected = reference::calculate(&document);
    assert_eq!(retained.dependencies(), expected.dependencies);
    let next = field_ref("calc-1", "c9");
    // Repairing one cell leaves every c7 failing, so the retained outcome must
    // stay `Failed`; accepting `Complete` would skip every assertion below.
    let CalculationOutcome::Failed(failures) = retained.outcome() else {
        panic!("the retained edit still has failing cells");
    };
    assert_eq!(failures.failures(), &expected.failures);
    for index in 0..3 {
        assert_eq!(
            failures
                .failures()
                .get(&field_ref(&format!("calc-{index}"), "out")),
            Some(&CalculationFailure::FailedDependencies {
                dependencies: BTreeSet::from([
                    field_ref(&format!("calc-{index}"), "c7"),
                    next.clone(),
                ]),
            }),
            "row {index} names the next failed cell after the edit"
        );
    }
    assert_matches_reference(&document, "the next failed cell after a retained edit");
}

#[test]
fn a_conditional_aggregate_names_the_first_failed_cell_of_each_column() {
    let document = fixed_document(
        vec![
            fixed_schema(
                "calc",
                &[("v", FieldType::Number), ("k", FieldType::Number)],
            ),
            fixed_schema("readers", &[("x0", FieldType::Number)]),
        ],
        vec![
            fixed_row(
                "calc-1",
                "calc",
                &[("v", failed_expression()), ("k", failed_expression())],
            ),
            fixed_row(
                "calc-2",
                "calc",
                &[("v", failed_expression()), ("k", failed_expression())],
            ),
            fixed_row(
                "r-sumif",
                "readers",
                &[(
                    "x0",
                    Value::Formula(conditional_sum("calc", "v", "k", literal(1.0))),
                )],
            ),
        ],
    );
    assert_eq!(
        fixed_failures(&document).get(&field_ref("r-sumif", "x0")),
        Some(&CalculationFailure::FailedDependencies {
            dependencies: BTreeSet::from([field_ref("calc-1", "k"), field_ref("calc-1", "v"),]),
        }),
        "a SUMIF names the first failed sum cell and the first failed criterion cell"
    );
    assert_matches_reference(&document, "a SUMIF over two failed columns");
}
