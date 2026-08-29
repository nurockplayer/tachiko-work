//! Deterministic formula parsing, binding, projection, and evaluation.

mod parser;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use tachiko_semantic_core::{
    AddressIndex, AddressIndexError, Document, Expression, FieldAddress, FieldRef, FieldType,
    MAX_EXPRESSION_DEPTH, MAX_EXPRESSION_NODES, Number, Value,
};
use thiserror::Error;

pub use parser::{
    ExpressionComplexityError, FormulaParseError, UnboundExpression, format_unbound_expression,
    parse_expression, validate_unbound_expression_structure,
};

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FormulaBindError {
    #[error(transparent)]
    Complexity(#[from] ExpressionComplexityError),
    #[error("formula address index is invalid: {source}")]
    Index {
        #[source]
        source: Box<AddressIndexError>,
    },
    #[error("formula address '{address}' cannot be resolved: {source}")]
    Address {
        address: FieldAddress,
        #[source]
        source: Box<AddressIndexError>,
    },
    #[error("formula address '{address}' resolves to non-numeric field '{reference}'")]
    NonNumericTarget {
        address: FieldAddress,
        reference: FieldRef,
    },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CanonicalAuthoringProjectionError {
    #[error("bound formula references cannot be projected through current human addresses")]
    UnresolvableBoundReferences { targets: BTreeSet<FieldRef> },
    #[error(transparent)]
    Complexity(#[from] ExpressionComplexityError),
}

/// Bind every parsed human address to stable IDs in one document snapshot.
///
/// # Errors
///
/// Returns a typed address or numeric-target error without producing a partial
/// bound expression.
pub fn bind_expression(
    document: &Document,
    expression: &UnboundExpression,
) -> Result<Expression, FormulaBindError> {
    validate_unbound_expression_structure(expression)?;
    let index = AddressIndex::build(document).map_err(|source| FormulaBindError::Index {
        source: Box::new(source),
    })?;
    bind_node(document, &index, expression)
}

fn bind_node(
    document: &Document,
    index: &AddressIndex,
    expression: &UnboundExpression,
) -> Result<Expression, FormulaBindError> {
    Ok(match expression {
        UnboundExpression::Number(number) => Expression::Number(*number),
        UnboundExpression::Reference(address) => {
            let reference = index.resolve_field(document, address).map_err(|source| {
                FormulaBindError::Address {
                    address: address.clone(),
                    source: Box::new(source),
                }
            })?;
            let entity = document.entities.get(&reference.entity).ok_or_else(|| {
                FormulaBindError::Index {
                    source: Box::new(AddressIndexError::MissingBoundEntity {
                        entity: reference.entity.clone(),
                    }),
                }
            })?;
            let schema =
                document
                    .schemas
                    .get(&entity.schema)
                    .ok_or_else(|| FormulaBindError::Index {
                        source: Box::new(AddressIndexError::MissingBoundSchema {
                            schema: entity.schema.clone(),
                        }),
                    })?;
            let definition =
                schema
                    .fields
                    .get(&reference.field)
                    .ok_or_else(|| FormulaBindError::Index {
                        source: Box::new(AddressIndexError::MissingBoundField {
                            entity: reference.entity.clone(),
                            field: reference.field.clone(),
                        }),
                    })?;
            if definition.field_type != FieldType::Number {
                return Err(FormulaBindError::NonNumericTarget {
                    address: address.clone(),
                    reference,
                });
            }
            Expression::Reference(reference)
        }
        UnboundExpression::Add { left, right } => Expression::Add {
            left: Box::new(bind_node(document, index, left)?),
            right: Box::new(bind_node(document, index, right)?),
        },
        UnboundExpression::Subtract { left, right } => Expression::Subtract {
            left: Box::new(bind_node(document, index, left)?),
            right: Box::new(bind_node(document, index, right)?),
        },
        UnboundExpression::Multiply { left, right } => Expression::Multiply {
            left: Box::new(bind_node(document, index, left)?),
            right: Box::new(bind_node(document, index, right)?),
        },
        UnboundExpression::Divide { left, right } => Expression::Divide {
            left: Box::new(bind_node(document, index, left)?),
            right: Box::new(bind_node(document, index, right)?),
        },
        UnboundExpression::Minimum { left, right } => Expression::Minimum {
            left: Box::new(bind_node(document, index, left)?),
            right: Box::new(bind_node(document, index, right)?),
        },
        UnboundExpression::Maximum { left, right } => Expression::Maximum {
            left: Box::new(bind_node(document, index, left)?),
            right: Box::new(bind_node(document, index, right)?),
        },
    })
}

/// Project a bound formula through the document's current human keys.
///
/// Every reference is round-trip checked against the same snapshot. Failure
/// returns all unresolved stable targets and no source text.
///
/// # Errors
///
/// Returns a typed stable-target set or structural/canonical-length failure.
pub fn project_expression(
    document: &Document,
    expression: &Expression,
) -> Result<String, CanonicalAuthoringProjectionError> {
    validate_expression_structure(expression)?;
    let dependencies = extract_dependencies(expression);
    let Ok(index) = AddressIndex::build(document) else {
        return Err(
            CanonicalAuthoringProjectionError::UnresolvableBoundReferences {
                targets: dependencies,
            },
        );
    };
    let mut addresses = BTreeMap::new();
    let mut unresolved = BTreeSet::new();
    for target in dependencies {
        match index.field_address(document, &target) {
            Ok(address) => {
                addresses.insert(target, address);
            }
            Err(_) => {
                unresolved.insert(target);
            }
        }
    }
    if !unresolved.is_empty() {
        return Err(
            CanonicalAuthoringProjectionError::UnresolvableBoundReferences {
                targets: unresolved,
            },
        );
    }

    let projected = render_bound(expression, &addresses);
    if projected.len() > parser::MAX_INPUT_BYTES {
        return Err(ExpressionComplexityError::CanonicalLengthLimit.into());
    }
    Ok(projected)
}

fn render_bound(expression: &Expression, addresses: &BTreeMap<FieldRef, FieldAddress>) -> String {
    match expression {
        Expression::Number(number) => parser::format_number(*number),
        Expression::Reference(reference) => {
            let address = &addresses[reference];
            format!("[{}.{}]", address.entity, address.field)
        }
        Expression::Add { left, right } => render_bound_binary(left, "+", right, addresses),
        Expression::Subtract { left, right } => render_bound_binary(left, "-", right, addresses),
        Expression::Multiply { left, right } => render_bound_binary(left, "*", right, addresses),
        Expression::Divide { left, right } => render_bound_binary(left, "/", right, addresses),
        Expression::Minimum { left, right } => format!(
            "min({}, {})",
            render_bound(left, addresses),
            render_bound(right, addresses)
        ),
        Expression::Maximum { left, right } => format!(
            "max({}, {})",
            render_bound(left, addresses),
            render_bound(right, addresses)
        ),
    }
}

fn render_bound_binary(
    left: &Expression,
    operator: &str,
    right: &Expression,
    addresses: &BTreeMap<FieldRef, FieldAddress>,
) -> String {
    format!(
        "({} {operator} {})",
        render_bound(left, addresses),
        render_bound(right, addresses)
    )
}

/// Validate node and depth limits for a directly supplied bound expression.
/// Canonical byte length is checked by [`project_expression`] because it
/// depends on the document's current human keys.
///
/// # Errors
///
/// Returns a typed node/depth limit error.
pub fn validate_expression_structure(
    expression: &Expression,
) -> Result<(), ExpressionComplexityError> {
    let mut nodes = 0_usize;
    let mut stack = vec![(expression, 1_usize)];
    while let Some((node, depth)) = stack.pop() {
        if depth > MAX_EXPRESSION_DEPTH {
            return Err(ExpressionComplexityError::DepthLimit);
        }
        nodes += 1;
        if nodes > MAX_EXPRESSION_NODES {
            return Err(ExpressionComplexityError::NodeLimit);
        }
        match node {
            Expression::Add { left, right }
            | Expression::Subtract { left, right }
            | Expression::Multiply { left, right }
            | Expression::Divide { left, right }
            | Expression::Minimum { left, right }
            | Expression::Maximum { left, right } => {
                stack.push((right, depth + 1));
                stack.push((left, depth + 1));
            }
            Expression::Number(_) | Expression::Reference(_) => {}
        }
    }
    Ok(())
}

/// Extract the deterministic static dependency set from a bound expression.
#[must_use]
pub fn extract_dependencies(expression: &Expression) -> BTreeSet<FieldRef> {
    let mut dependencies = BTreeSet::new();
    let mut stack = vec![expression];
    while let Some(node) = stack.pop() {
        match node {
            Expression::Reference(reference) => {
                dependencies.insert(reference.clone());
            }
            Expression::Add { left, right }
            | Expression::Subtract { left, right }
            | Expression::Multiply { left, right }
            | Expression::Divide { left, right }
            | Expression::Minimum { left, right }
            | Expression::Maximum { left, right } => {
                stack.push(right);
                stack.push(left);
            }
            Expression::Number(_) => {}
        }
    }
    dependencies
}

#[derive(Clone, Debug, PartialEq)]
pub struct Calculation {
    values: BTreeMap<FieldRef, Number>,
    dependencies: BTreeMap<FieldRef, BTreeSet<FieldRef>>,
}

/// Complete atomic result of a fresh full-document recomputation.
///
/// A failed outcome deliberately contains no successful values. Static
/// dependencies remain available because they are extracted before graph and
/// evaluation failure classification.
#[derive(Clone, Debug, PartialEq)]
pub enum CalculationOutcome {
    Complete(Calculation),
    Failed(CalculationFailures),
}

/// Runtime-only formula state retained by a resident semantic workspace.
///
/// Successful values may remain present internally while another independent
/// component is failed so a later local edit can reuse unaffected work. They
/// are never exposed through [`Self::outcome`] while any failure exists.
/// This state is derived entirely from one [`Document`] and is not canonical
/// semantic state, a cache key contract, or a serialization surface.
#[derive(Clone, Debug, PartialEq)]
pub struct RetainedCalculationState {
    nodes: BTreeSet<FieldRef>,
    values: BTreeMap<FieldRef, Number>,
    failures: FailureMap,
    dependencies: DependencyMap,
    reverse_dependents: DependencyMap,
}

/// Deterministic work evidence for one retained calculation transition.
///
/// These counters describe the current provisional algorithm. They are
/// runtime/benchmark evidence, not semantic identity or a performance SLA.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IncrementalCalculationWork {
    pub full_rebuilds: usize,
    pub incremental_updates: usize,
    pub nodes_recomputed: usize,
    pub nodes_reused: usize,
    pub reverse_edges_traversed: usize,
}

/// Runtime-only transition evidence consumed by resident projection
/// invalidation. Both field lists are deterministic and exclude dirty roots
/// from `affected_calculations` while retaining changed root projections in
/// `changed_calculation_projections` when applicable.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IncrementalCalculationTransition {
    pub work: IncrementalCalculationWork,
    pub affected_calculations: Vec<FieldRef>,
    pub changed_calculation_projections: Vec<FieldRef>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RetainedNodeProjection {
    Value(Number),
    Failure(CalculationFailure),
    Unavailable,
}

struct DirtyCalculationImpact {
    dirty: BTreeSet<FieldRef>,
    old_dependents: BTreeSet<FieldRef>,
    new_dependents: BTreeSet<FieldRef>,
    reverse_edges_traversed: usize,
}

impl RetainedCalculationState {
    /// Rebuild all retained formula state through the full ADR-0018 oracle
    /// phases.
    #[must_use]
    pub fn rebuild(document: &Document) -> (Self, IncrementalCalculationWork) {
        let (nodes, formulas, dependencies) = collect_calculation_nodes(document);
        let mut failures = pregraph_failures(document, &nodes, &formulas, &dependencies);
        assign_cycle_failures(&formulas, &dependencies, &mut failures);
        let mut values = initial_values(&nodes, &failures);
        evaluate_remaining_formulas(&formulas, &dependencies, &mut failures, &mut values);
        let node_ids = nodes.keys().cloned().collect::<BTreeSet<_>>();
        let reverse_dependents = reverse_dependency_index(&dependencies);
        let work = IncrementalCalculationWork {
            full_rebuilds: 1,
            nodes_recomputed: node_ids.len(),
            ..IncrementalCalculationWork::default()
        };
        (
            Self {
                nodes: node_ids,
                values,
                failures,
                dependencies,
                reverse_dependents,
            },
            work,
        )
    }

    /// Recompute dirty roots and their old/new-graph reverse dependent closure.
    ///
    /// The caller owns conservative impact classification. When that cannot be
    /// proved, discard this state and call [`Self::rebuild`] instead.
    #[must_use]
    pub fn update(
        &mut self,
        document: &Document,
        dirty_roots: &BTreeSet<FieldRef>,
    ) -> IncrementalCalculationTransition {
        if dirty_roots.is_empty() {
            return IncrementalCalculationTransition {
                work: IncrementalCalculationWork {
                    incremental_updates: 1,
                    nodes_reused: self.nodes.len(),
                    ..IncrementalCalculationWork::default()
                },
                ..IncrementalCalculationTransition::default()
            };
        }

        let before_failed = self.is_failed();
        let before_formulas = self.dependencies.keys().cloned().collect::<BTreeSet<_>>();
        let impact = self.update_dependency_impact(document, dirty_roots);
        let before_projections = impact
            .dirty
            .iter()
            .map(|field| (field.clone(), self.node_projection(field)))
            .collect::<BTreeMap<_, _>>();
        self.recompute_dirty(document, dirty_roots, &impact.dirty);

        let nodes_recomputed = impact
            .dirty
            .iter()
            .filter(|field| self.nodes.contains(*field))
            .count();
        let work = IncrementalCalculationWork {
            incremental_updates: 1,
            nodes_recomputed,
            nodes_reused: self.nodes.len().saturating_sub(nodes_recomputed),
            reverse_edges_traversed: impact.reverse_edges_traversed,
            ..IncrementalCalculationWork::default()
        };
        let after_failed = self.is_failed();
        let formula_fields = before_formulas
            .into_iter()
            .chain(self.dependencies.keys().cloned())
            .collect::<BTreeSet<_>>();
        let changed_calculation_projections = if before_failed == after_failed {
            formula_fields
                .intersection(&impact.dirty)
                .filter(|field| {
                    before_projections.get(*field) != Some(&self.node_projection(field))
                })
                .cloned()
                .collect()
        } else {
            formula_fields.into_iter().collect()
        };
        IncrementalCalculationTransition {
            work,
            affected_calculations: impact
                .old_dependents
                .into_iter()
                .chain(impact.new_dependents)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            changed_calculation_projections,
        }
    }

    fn update_dependency_impact(
        &mut self,
        document: &Document,
        dirty_roots: &BTreeSet<FieldRef>,
    ) -> DirtyCalculationImpact {
        let (old_dependents, old_edges) =
            reverse_dependent_closure(&self.reverse_dependents, dirty_roots);
        for root in dirty_roots {
            replace_root_dependencies(
                &mut self.dependencies,
                &mut self.reverse_dependents,
                root,
                formula_dependencies(document, root),
            );
        }
        let (new_dependents, new_edges) =
            reverse_dependent_closure(&self.reverse_dependents, dirty_roots);
        let dirty = dirty_roots
            .iter()
            .chain(old_dependents.iter())
            .chain(new_dependents.iter())
            .cloned()
            .collect();
        DirtyCalculationImpact {
            dirty,
            old_dependents,
            new_dependents,
            reverse_edges_traversed: old_edges.saturating_add(new_edges),
        }
    }

    fn recompute_dirty(
        &mut self,
        document: &Document,
        dirty_roots: &BTreeSet<FieldRef>,
        dirty: &BTreeSet<FieldRef>,
    ) {
        for root in dirty_roots {
            if calculation_value(document, root).is_some() {
                self.nodes.insert(root.clone());
            } else {
                self.nodes.remove(root);
            }
        }
        let dirty_nodes = dirty
            .iter()
            .filter_map(|field| {
                calculation_value(document, field).map(|value| (field.clone(), value))
            })
            .collect::<ValueNodes<'_>>();
        let dirty_formulas = dirty_nodes
            .iter()
            .filter_map(|(field, value)| match value {
                Value::Formula(expression) => Some((field.clone(), expression)),
                Value::Number(_) | Value::Text(_) | Value::Boolean(_) | Value::Reference(_) => None,
            })
            .collect::<FormulaNodes<'_>>();

        for field in dirty {
            self.failures.remove(field);
            self.values.remove(field);
        }
        self.failures.extend(pregraph_failures(
            document,
            &dirty_nodes,
            &dirty_formulas,
            &self.dependencies,
        ));
        assign_cycle_failures(&dirty_formulas, &self.dependencies, &mut self.failures);
        self.values
            .extend(initial_values(&dirty_nodes, &self.failures));
        evaluate_remaining_formulas(
            &dirty_formulas,
            &self.dependencies,
            &mut self.failures,
            &mut self.values,
        );
    }

    /// Project the Accepted atomic calculation outcome for this revision.
    #[must_use]
    pub fn outcome(&self) -> CalculationOutcome {
        if self.failures.is_empty() {
            CalculationOutcome::Complete(Calculation {
                values: self.values.clone(),
                dependencies: self.dependencies.clone(),
            })
        } else {
            CalculationOutcome::Failed(CalculationFailures {
                failures: self.failures.clone(),
                dependencies: self.dependencies.clone(),
            })
        }
    }

    /// Return whether the atomic calculation outcome is failed.
    #[must_use]
    pub fn is_failed(&self) -> bool {
        !self.failures.is_empty()
    }

    /// Return a published value only when the complete calculation is atomic.
    #[must_use]
    pub fn value(&self, field: &FieldRef) -> Option<Number> {
        self.failures
            .is_empty()
            .then(|| self.values.get(field).copied())
            .flatten()
    }

    /// Return the complete published value map only for an atomic successful
    /// outcome.
    #[must_use]
    pub fn complete_values(&self) -> Option<&BTreeMap<FieldRef, Number>> {
        self.failures.is_empty().then_some(&self.values)
    }

    /// Return one primary failure only while the atomic outcome is failed.
    #[must_use]
    pub fn failure(&self, field: &FieldRef) -> Option<&CalculationFailure> {
        self.failures.get(field)
    }

    /// Return all primary failures retained for the current revision.
    #[must_use]
    pub fn failures(&self) -> &BTreeMap<FieldRef, CalculationFailure> {
        &self.failures
    }

    /// Return every current static formula dependency set.
    #[must_use]
    pub fn dependencies(&self) -> &BTreeMap<FieldRef, BTreeSet<FieldRef>> {
        &self.dependencies
    }

    fn node_projection(&self, field: &FieldRef) -> RetainedNodeProjection {
        if self.is_failed() {
            self.failures
                .get(field)
                .map_or(RetainedNodeProjection::Unavailable, |failure| {
                    RetainedNodeProjection::Failure(failure.clone())
                })
        } else {
            self.values
                .get(field)
                .map_or(RetainedNodeProjection::Unavailable, |value| {
                    RetainedNodeProjection::Value(*value)
                })
        }
    }

    /// Return the retained reverse dependency index used for bounded impact
    /// traversal.
    #[must_use]
    pub fn reverse_dependents(&self) -> &BTreeMap<FieldRef, BTreeSet<FieldRef>> {
        &self.reverse_dependents
    }

    /// Return the deterministic reverse transitive closure, excluding the
    /// supplied dirty roots themselves.
    #[must_use]
    pub fn affected_by_all(&self, dirty_roots: &BTreeSet<FieldRef>) -> Vec<FieldRef> {
        reverse_dependent_closure(&self.reverse_dependents, dirty_roots)
            .0
            .into_iter()
            .collect()
    }
}

/// Every primary semantic failure from one failed full recomputation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CalculationFailures {
    failures: BTreeMap<FieldRef, CalculationFailure>,
    dependencies: BTreeMap<FieldRef, BTreeSet<FieldRef>>,
}

impl CalculationFailures {
    /// Return the stable value-node-keyed primary failure map.
    #[must_use]
    pub fn failures(&self) -> &BTreeMap<FieldRef, CalculationFailure> {
        &self.failures
    }

    /// Return every statically extracted formula dependency set, including
    /// edges incident to failed nodes.
    #[must_use]
    pub fn dependencies(&self) -> &BTreeMap<FieldRef, BTreeSet<FieldRef>> {
        &self.dependencies
    }

    fn compatibility_error(&self) -> CalculationError {
        let (mut field, mut failure) = self
            .failures
            .first_key_value()
            .expect("failed calculation must contain a primary failure");
        let mut visited = BTreeSet::new();
        loop {
            assert!(
                visited.insert(field.clone()),
                "failed-dependency outcomes form an acyclic graph"
            );
            match failure {
                CalculationFailure::InvalidExpression { error } => {
                    return CalculationError::InvalidExpression {
                        formula: field.clone(),
                        message: error.to_string(),
                    };
                }
                CalculationFailure::InvalidReferences { targets } => {
                    let (reference, reference_failure) = targets
                        .first_key_value()
                        .expect("invalid-reference failure must contain a target");
                    return match reference_failure {
                        ReferenceFailure::Missing => CalculationError::MissingReference {
                            reference: reference.clone(),
                        },
                        ReferenceFailure::NonNumeric => CalculationError::NonNumericReference {
                            reference: reference.clone(),
                        },
                    };
                }
                CalculationFailure::Cycle { members } => {
                    return CalculationError::Cycle {
                        path: self.compatibility_cycle_witness(field, members),
                    };
                }
                CalculationFailure::FailedDependencies { dependencies } => {
                    let Some((dependency, dependency_failure)) =
                        dependencies.iter().find_map(|dependency| {
                            self.failures
                                .get(dependency)
                                .map(|failure| (dependency, failure))
                        })
                    else {
                        return CalculationError::MissingReference {
                            reference: dependencies
                                .first()
                                .cloned()
                                .unwrap_or_else(|| field.clone()),
                        };
                    };
                    field = dependency;
                    failure = dependency_failure;
                }
                CalculationFailure::DivisionByZero => {
                    return CalculationError::DivisionByZero {
                        formula: field.clone(),
                    };
                }
                CalculationFailure::NonFiniteResult => {
                    return CalculationError::NonFiniteResult {
                        field: field.clone(),
                    };
                }
            }
        }
    }

    /// Derive a deterministic legacy witness from semantic SCC membership.
    /// The witness is presentation-only; the complete member set above is the
    /// graph-failure authority.
    fn compatibility_cycle_witness(
        &self,
        selected: &FieldRef,
        members: &BTreeSet<FieldRef>,
    ) -> Vec<FieldRef> {
        let start = if members.contains(selected) {
            selected
        } else {
            members
                .first()
                .expect("cycle failure must contain an SCC member")
        };
        let outgoing = self
            .dependencies
            .get(start)
            .into_iter()
            .flatten()
            .filter(|dependency| members.contains(*dependency));

        for next in outgoing {
            if next == start {
                return vec![start.clone(), start.clone()];
            }

            let mut queue = VecDeque::from([next.clone()]);
            let mut visited = BTreeSet::from([next.clone()]);
            let mut parents: BTreeMap<FieldRef, FieldRef> = BTreeMap::new();
            while let Some(node) = queue.pop_front() {
                for dependency in self
                    .dependencies
                    .get(&node)
                    .into_iter()
                    .flatten()
                    .filter(|dependency| members.contains(*dependency))
                {
                    if dependency == start {
                        let mut reverse_path = vec![node.clone()];
                        while reverse_path.last() != Some(next) {
                            let parent = parents
                                .get(reverse_path.last().expect("path is non-empty"))
                                .expect("visited SCC node has a parent")
                                .clone();
                            reverse_path.push(parent);
                        }
                        reverse_path.reverse();
                        let mut path = vec![start.clone()];
                        path.extend(reverse_path);
                        path.push(start.clone());
                        return path;
                    }
                    if visited.insert(dependency.clone()) {
                        parents.insert(dependency.clone(), node.clone());
                        queue.push_back(dependency.clone());
                    }
                }
            }
        }

        debug_assert!(false, "SCC member must have a cycle witness");
        let mut fallback = members.iter().cloned().collect::<Vec<_>>();
        fallback.push(start.clone());
        fallback
    }
}

/// One primary semantic failure for a stable value node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CalculationFailure {
    InvalidExpression {
        error: ExpressionComplexityError,
    },
    InvalidReferences {
        targets: BTreeMap<FieldRef, ReferenceFailure>,
    },
    Cycle {
        members: BTreeSet<FieldRef>,
    },
    FailedDependencies {
        dependencies: BTreeSet<FieldRef>,
    },
    DivisionByZero,
    NonFiniteResult,
}

/// Binding/type classification for one directly referenced stable value node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceFailure {
    Missing,
    NonNumeric,
}

impl Calculation {
    #[must_use]
    pub fn value(&self, field: &FieldRef) -> Option<Number> {
        self.values.get(field).copied()
    }

    #[must_use]
    pub fn values(&self) -> &BTreeMap<FieldRef, Number> {
        &self.values
    }

    #[must_use]
    pub fn dependencies(&self) -> &BTreeMap<FieldRef, BTreeSet<FieldRef>> {
        &self.dependencies
    }

    #[must_use]
    pub fn dependencies_of(&self, field: &FieldRef) -> Option<&BTreeSet<FieldRef>> {
        self.dependencies.get(field)
    }

    #[must_use]
    pub fn affected_by(&self, changed: &FieldRef) -> Vec<FieldRef> {
        let mut frontier = BTreeSet::from([changed.clone()]);
        let mut affected = BTreeSet::new();

        loop {
            let newly_affected: BTreeSet<_> = self
                .dependencies
                .iter()
                .filter(|(formula, dependencies)| {
                    !affected.contains(*formula) && !dependencies.is_disjoint(&frontier)
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

#[derive(Clone, Debug, Error, PartialEq)]
pub enum CalculationError {
    #[error("formula reference '{reference}' does not exist")]
    MissingReference { reference: FieldRef },
    #[error("formula reference '{reference}' is not numeric")]
    NonNumericReference { reference: FieldRef },
    #[error("formula '{formula}' divided by zero")]
    DivisionByZero { formula: FieldRef },
    #[error("formula dependency cycle: {path:?}")]
    Cycle { path: Vec<FieldRef> },
    #[error("calculation for '{field}' produced a non-finite result")]
    NonFiniteResult { field: FieldRef },
    #[error("formula '{formula}' violates the bound expression contract: {message}")]
    InvalidExpression { formula: FieldRef, message: String },
}

/// Calculate every numeric literal and formula in deterministic stable-ID order.
///
/// This compatibility API is a convenience projection of [`calculate_complete`].
/// A failed complete outcome projects the first stable-node-keyed primary
/// failure into the historical fail-first error family. In particular, a
/// projected cycle path is not semantic authority; [`CalculationFailure::Cycle`]
/// carries the authoritative complete SCC membership.
///
/// # Errors
///
/// Returns a [`CalculationError`] for invalid bound expressions, missing or
/// non-numeric references, cycles, division by zero, or non-finite results.
pub fn calculate(document: &Document) -> Result<Calculation, CalculationError> {
    match calculate_complete(document) {
        CalculationOutcome::Complete(calculation) => Ok(calculation),
        CalculationOutcome::Failed(failures) => Err(failures.compatibility_error()),
    }
}

/// Run the ADR-0018 complete atomic full-recompute correctness oracle.
///
/// Every calculation node receives one deterministic value-or-primary-failure
/// outcome. Independent failures accumulate, cycles use complete SCC member
/// sets, and dependency failures name only directly failed dependencies. A
/// failed outcome publishes no partial [`Calculation`].
#[must_use]
pub fn calculate_complete(document: &Document) -> CalculationOutcome {
    let (nodes, formulas, dependencies) = collect_calculation_nodes(document);
    let mut failures = pregraph_failures(document, &nodes, &formulas, &dependencies);
    assign_cycle_failures(&formulas, &dependencies, &mut failures);
    let mut values = initial_values(&nodes, &failures);
    evaluate_remaining_formulas(&formulas, &dependencies, &mut failures, &mut values);

    if failures.is_empty() {
        CalculationOutcome::Complete(Calculation {
            values,
            dependencies,
        })
    } else {
        CalculationOutcome::Failed(CalculationFailures {
            failures,
            dependencies,
        })
    }
}

type ValueNodes<'document> = BTreeMap<FieldRef, &'document Value>;
type FormulaNodes<'document> = BTreeMap<FieldRef, &'document Expression>;
type DependencyMap = BTreeMap<FieldRef, BTreeSet<FieldRef>>;
type FailureMap = BTreeMap<FieldRef, CalculationFailure>;

fn collect_calculation_nodes(
    document: &Document,
) -> (ValueNodes<'_>, FormulaNodes<'_>, DependencyMap) {
    let mut nodes = BTreeMap::new();
    let mut formulas = BTreeMap::new();
    let mut dependencies = BTreeMap::new();
    for (entity_id, entity) in &document.entities {
        for (field_id, value) in &entity.fields {
            if matches!(value, Value::Number(_) | Value::Formula(_)) {
                let field = FieldRef::new(entity_id.clone(), field_id.clone());
                nodes.insert(field.clone(), value);
                if let Value::Formula(expression) = value {
                    dependencies.insert(field.clone(), extract_dependencies(expression));
                    formulas.insert(field, expression);
                }
            }
        }
    }
    (nodes, formulas, dependencies)
}

fn calculation_value<'document>(
    document: &'document Document,
    field: &FieldRef,
) -> Option<&'document Value> {
    document
        .entities
        .get(&field.entity)
        .and_then(|entity| entity.fields.get(&field.field))
        .filter(|value| matches!(value, Value::Number(_) | Value::Formula(_)))
}

fn formula_dependencies(document: &Document, field: &FieldRef) -> Option<BTreeSet<FieldRef>> {
    match calculation_value(document, field) {
        Some(Value::Formula(expression)) => Some(extract_dependencies(expression)),
        Some(Value::Number(_) | Value::Text(_) | Value::Boolean(_) | Value::Reference(_))
        | None => None,
    }
}

fn reverse_dependency_index(dependencies: &DependencyMap) -> DependencyMap {
    let mut reverse = BTreeMap::<FieldRef, BTreeSet<FieldRef>>::new();
    for (formula, inputs) in dependencies {
        for input in inputs {
            reverse
                .entry(input.clone())
                .or_default()
                .insert(formula.clone());
        }
    }
    reverse
}

fn replace_root_dependencies(
    dependencies: &mut DependencyMap,
    reverse_dependents: &mut DependencyMap,
    root: &FieldRef,
    replacement: Option<BTreeSet<FieldRef>>,
) {
    if let Some(previous) = dependencies.remove(root) {
        for input in previous {
            let remove_entry = reverse_dependents
                .get_mut(&input)
                .is_some_and(|dependents| {
                    dependents.remove(root);
                    dependents.is_empty()
                });
            if remove_entry {
                reverse_dependents.remove(&input);
            }
        }
    }
    let Some(replacement) = replacement else {
        return;
    };
    for input in &replacement {
        reverse_dependents
            .entry(input.clone())
            .or_default()
            .insert(root.clone());
    }
    dependencies.insert(root.clone(), replacement);
}

fn reverse_dependent_closure(
    reverse_dependents: &DependencyMap,
    roots: &BTreeSet<FieldRef>,
) -> (BTreeSet<FieldRef>, usize) {
    let mut queue = roots.iter().cloned().collect::<VecDeque<_>>();
    let mut affected = BTreeSet::new();
    let mut traversed = 0_usize;
    while let Some(field) = queue.pop_front() {
        for dependent in reverse_dependents.get(&field).into_iter().flatten() {
            traversed = traversed.saturating_add(1);
            if roots.contains(dependent) || !affected.insert(dependent.clone()) {
                continue;
            }
            queue.push_back(dependent.clone());
        }
    }
    (affected, traversed)
}

fn pregraph_failures(
    document: &Document,
    nodes: &ValueNodes<'_>,
    formulas: &FormulaNodes<'_>,
    dependencies: &DependencyMap,
) -> FailureMap {
    let mut failures = BTreeMap::new();
    // Phase 1a: structural failures take precedence over every later phase.
    for (formula, expression) in formulas {
        if let Err(error) = validate_expression_structure(expression) {
            failures.insert(
                formula.clone(),
                CalculationFailure::InvalidExpression { error },
            );
        }
    }

    // Phase 1b: capture every directly discovered missing/stale/type target.
    for (field, value) in nodes {
        if failures.contains_key(field) {
            continue;
        }
        let mut invalid_targets = BTreeMap::new();
        if let Some(failure) = reference_failure(document, field) {
            invalid_targets.insert(field.clone(), failure);
        }
        if matches!(value, Value::Formula(_)) {
            for dependency in dependencies.get(field).into_iter().flatten() {
                if let Some(failure) = reference_failure(document, dependency) {
                    invalid_targets.insert(dependency.clone(), failure);
                }
            }
        }
        if !invalid_targets.is_empty() {
            failures.insert(
                field.clone(),
                CalculationFailure::InvalidReferences {
                    targets: invalid_targets,
                },
            );
        }
    }
    failures
}

fn assign_cycle_failures(
    formulas: &FormulaNodes<'_>,
    dependencies: &DependencyMap,
    failures: &mut FailureMap,
) {
    // Phase 2: semantic cycles are complete SCCs in the graph induced by
    // formulas without an earlier primary failure.
    let eligible_formulas = formulas
        .keys()
        .filter(|formula| !failures.contains_key(*formula))
        .cloned()
        .collect::<BTreeSet<_>>();
    for members in cyclic_components(&eligible_formulas, dependencies) {
        for member in &members {
            failures.insert(
                member.clone(),
                CalculationFailure::Cycle {
                    members: members.clone(),
                },
            );
        }
    }
}

fn initial_values(nodes: &ValueNodes<'_>, failures: &FailureMap) -> BTreeMap<FieldRef, Number> {
    // Valid stored numbers are the initial ready values. Failed stored nodes
    // remain primary failures but never become partial published state.
    nodes
        .iter()
        .filter_map(|(field, value)| match value {
            Value::Number(number) if !failures.contains_key(field) => {
                Some((field.clone(), *number))
            }
            _ => None,
        })
        .collect()
}

fn evaluate_remaining_formulas(
    formulas: &FormulaNodes<'_>,
    dependencies: &DependencyMap,
    failures: &mut FailureMap,
    values: &mut BTreeMap<FieldRef, Number>,
) {
    // Phases 3 and 4: process the remaining acyclic graph dependency-first.
    // Edges to already-failed nodes are ready and become direct dependency
    // subjects; edges to remaining formulas participate in the Kahn count.
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
        let failed_dependencies = dependencies
            .get(&formula)
            .into_iter()
            .flatten()
            .filter(|dependency| failures.contains_key(*dependency))
            .cloned()
            .collect::<BTreeSet<_>>();
        if failed_dependencies.is_empty() {
            let expression = formulas[&formula];
            match evaluate_bound_expression(expression, values) {
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
        "SCC condensation must be acyclic"
    );
}

#[derive(Clone, Copy, Debug)]
enum BinaryOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Minimum,
    Maximum,
}

enum EvaluationFrame<'expression> {
    Expression(&'expression Expression),
    Apply(BinaryOperation),
}

fn reference_failure(document: &Document, field: &FieldRef) -> Option<ReferenceFailure> {
    let Some(entity) = document.entities.get(&field.entity) else {
        return Some(ReferenceFailure::Missing);
    };
    let Some(definition) = document
        .schemas
        .get(&entity.schema)
        .and_then(|schema| schema.fields.get(&field.field))
    else {
        return Some(ReferenceFailure::Missing);
    };
    if definition.field_type != FieldType::Number {
        return Some(ReferenceFailure::NonNumeric);
    }
    match entity.fields.get(&field.field) {
        Some(Value::Number(_) | Value::Formula(_)) => None,
        Some(Value::Text(_) | Value::Boolean(_) | Value::Reference(_)) => {
            Some(ReferenceFailure::NonNumeric)
        }
        None => Some(ReferenceFailure::Missing),
    }
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

fn evaluate_bound_expression(
    expression: &Expression,
    values: &BTreeMap<FieldRef, Number>,
) -> Result<Number, CalculationFailure> {
    let mut frames = vec![EvaluationFrame::Expression(expression)];
    let mut results = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            EvaluationFrame::Expression(Expression::Number(number)) => results.push(*number),
            EvaluationFrame::Expression(Expression::Reference(reference)) => {
                let Some(value) = values.get(reference) else {
                    return Err(CalculationFailure::FailedDependencies {
                        dependencies: BTreeSet::from([reference.clone()]),
                    });
                };
                results.push(*value);
            }
            EvaluationFrame::Expression(Expression::Add { left, right }) => {
                push_binary(&mut frames, BinaryOperation::Add, left, right);
            }
            EvaluationFrame::Expression(Expression::Subtract { left, right }) => {
                push_binary(&mut frames, BinaryOperation::Subtract, left, right);
            }
            EvaluationFrame::Expression(Expression::Multiply { left, right }) => {
                push_binary(&mut frames, BinaryOperation::Multiply, left, right);
            }
            EvaluationFrame::Expression(Expression::Divide { left, right }) => {
                push_binary(&mut frames, BinaryOperation::Divide, left, right);
            }
            EvaluationFrame::Expression(Expression::Minimum { left, right }) => {
                push_binary(&mut frames, BinaryOperation::Minimum, left, right);
            }
            EvaluationFrame::Expression(Expression::Maximum { left, right }) => {
                push_binary(&mut frames, BinaryOperation::Maximum, left, right);
            }
            EvaluationFrame::Apply(operation) => {
                let right = pop_result(&mut results);
                let left = pop_result(&mut results);
                results.push(apply_operation(operation, left, right)?);
            }
        }
    }

    assert_eq!(
        results.len(),
        1,
        "validated expression produces exactly one result"
    );
    Ok(pop_result(&mut results))
}

fn push_binary<'expression>(
    frames: &mut Vec<EvaluationFrame<'expression>>,
    operation: BinaryOperation,
    left: &'expression Expression,
    right: &'expression Expression,
) {
    frames.push(EvaluationFrame::Apply(operation));
    frames.push(EvaluationFrame::Expression(right));
    frames.push(EvaluationFrame::Expression(left));
}

fn pop_result(results: &mut Vec<Number>) -> Number {
    results
        .pop()
        .expect("validated expression result stack is non-empty")
}

fn apply_operation(
    operation: BinaryOperation,
    left: Number,
    right: Number,
) -> Result<Number, CalculationFailure> {
    match operation {
        BinaryOperation::Add => number_result(left.get() + right.get()),
        BinaryOperation::Subtract => number_result(left.get() - right.get()),
        BinaryOperation::Multiply => number_result(left.get() * right.get()),
        BinaryOperation::Divide => {
            if right.get() == 0.0 {
                return Err(CalculationFailure::DivisionByZero);
            }
            number_result(left.get() / right.get())
        }
        BinaryOperation::Minimum => Ok(if left <= right { left } else { right }),
        BinaryOperation::Maximum => Ok(if left >= right { left } else { right }),
    }
}

fn number_result(value: f64) -> Result<Number, CalculationFailure> {
    Number::new(value).map_err(|_| CalculationFailure::NonFiniteResult)
}
