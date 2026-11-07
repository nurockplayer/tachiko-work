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

use super::{CalculationOutcome, RetainedCalculationState, calculate_complete};

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

        while let Some(formula) = ready.pop_first() {
            remaining_dependency_counts.remove(&formula);
            let evaluation = evaluation::Evaluation {
                document,
                owner: &formula,
                values,
            };
            let failed_dependencies = evaluation
                .required_references(formulas[&formula])
                .iter()
                .filter(|dependency| failures.contains_key(*dependency))
                .cloned()
                .collect::<BTreeSet<_>>();
            if failed_dependencies.is_empty() {
                let expression = formulas[&formula];
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
    let c1 = match index % 5 {
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
        _ => add(
            aggregate(ColumnAggregateFunction::Count, schema, "c0"),
            aggregate(ColumnAggregateFunction::Count, schema, "c2"),
        ),
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

fn assert_matches_reference(document: &Document, seed: u64) {
    let expected = reference::calculate(document);
    let actual = super::full_calculation_parts(document);
    assert_eq!(&actual.values, &expected.values, "values of seed {seed}");
    assert_eq!(
        &actual.failures, &expected.failures,
        "failures of seed {seed}"
    );
    assert_eq!(
        actual.graph.expanded_dependencies(),
        expected.dependencies,
        "expanded dependencies of seed {seed}"
    );

    let outcome = calculate_complete(document);
    match (&outcome, expected.failures.is_empty()) {
        (CalculationOutcome::Complete(_), true) | (CalculationOutcome::Failed(_), false) => {}
        _ => panic!("outcome class disagrees with the reference for seed {seed}"),
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
            "affected_by({field:?}) of seed {seed}"
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
