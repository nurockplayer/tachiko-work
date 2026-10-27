//! Deterministic formula parsing, binding, projection, and evaluation.

mod evaluation;
mod parser;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use tachiko_semantic_core::{
    AddressIndex, AddressIndexError, ColumnAddress, ColumnRef, Document, Expression, FieldAddress,
    FieldId, FieldRef, FieldType, FormulaExpressionType, MAX_EXPRESSION_DEPTH, MAX_EXPRESSION_NODES,
    Number, Value,
};
use thiserror::Error;

pub use parser::{
    ExpressionComplexityError, FormulaParseError, UnboundExpression, format_unbound_expression,
    parse_expression, validate_unbound_expression_structure,
};

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FormulaBindError {
    #[error("formula type mismatch: {message}")]
    TypeMismatch { message: String },
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
    #[error("formula column address '{address}' cannot be resolved: {source}")]
    Column {
        address: ColumnAddress,
        #[source]
        source: Box<AddressIndexError>,
    },
    #[error("formula column address '{address}' does not resolve to a numeric field")]
    NonNumericColumn { address: ColumnAddress },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CanonicalAuthoringProjectionError {
    #[error("bound formula references cannot be projected through current human addresses")]
    UnresolvableBoundReferences { targets: BTreeSet<FieldRef> },
    #[error("bound formula columns cannot be projected through current human addresses")]
    UnresolvableBoundColumns { columns: BTreeSet<ColumnRef> },
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
    owner: &FieldRef,
    expression: &UnboundExpression,
) -> Result<Expression, FormulaBindError> {
    validate_unbound_expression_structure(expression)?;
    let index = AddressIndex::build(document).map_err(|source| FormulaBindError::Index {
        source: Box::new(source),
    })?;
    let bound = bind_node(document, &index, owner, expression)?;
    if bound.expression_type() != Some(FormulaExpressionType::Number) {
        return Err(FormulaBindError::TypeMismatch {
            message:
                "conditions are only valid in IF, AND, OR, or NOT; ROUND requires a numeric value"
                    .into(),
        });
    }
    Ok(bound)
}

/// Resolve one human address to a numeric stable target.
fn bind_numeric_reference(
    document: &Document,
    index: &AddressIndex,
    address: &FieldAddress,
) -> Result<FieldRef, FormulaBindError> {
    let reference =
        index
            .resolve_field(document, address)
            .map_err(|source| FormulaBindError::Address {
                address: address.clone(),
                source: Box::new(source),
            })?;
    let entity =
        document
            .entities
            .get(&reference.entity)
            .ok_or_else(|| FormulaBindError::Index {
                source: Box::new(AddressIndexError::MissingBoundEntity {
                    entity: reference.entity.clone(),
                }),
            })?;
    let schema = document
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
    Ok(reference)
}

/// Resolve one human column address to a stable schema/field identity and
/// require that the column is numeric.
fn bind_column_reference(
    document: &Document,
    index: &AddressIndex,
    address: &ColumnAddress,
) -> Result<ColumnRef, FormulaBindError> {
    let unresolved = |source: AddressIndexError| FormulaBindError::Column {
        address: address.clone(),
        source: Box::new(source),
    };
    let schema = index
        .schema_id(&address.schema)
        .map_err(unresolved)?
        .clone();
    let field = index
        .schema_field_id(&schema, &address.field)
        .map_err(unresolved)?
        .clone();
    let definition = document
        .schemas
        .get(&schema)
        .and_then(|schema| schema.fields.get(&field))
        .ok_or_else(|| {
            unresolved(AddressIndexError::MissingSchemaFieldKey {
                schema: schema.clone(),
                field: address.field.clone(),
            })
        })?;
    if definition.field_type != FieldType::Number {
        return Err(FormulaBindError::NonNumericColumn {
            address: address.clone(),
        });
    }
    Ok(ColumnRef::new(schema, field))
}

fn bind_node(
    document: &Document,
    index: &AddressIndex,
    owner: &FieldRef,
    expression: &UnboundExpression,
) -> Result<Expression, FormulaBindError> {
    Ok(match expression {
        UnboundExpression::Number(number) => Expression::Number(*number),
        UnboundExpression::Reference(address) => {
            Expression::Reference(bind_numeric_reference(document, index, address)?)
        }
        UnboundExpression::SameRowReference(field) => {
            let entity =
                document
                    .entities
                    .get(&owner.entity)
                    .ok_or_else(|| FormulaBindError::Index {
                        source: Box::new(AddressIndexError::MissingBoundEntity {
                            entity: owner.entity.clone(),
                        }),
                    })?;
            let address = FieldAddress::new(entity.key.clone(), field.clone());
            Expression::SameRowReference(bind_numeric_reference(document, index, &address)?.field)
        }
        UnboundExpression::ColumnTotal(address) => {
            Expression::ColumnTotal(bind_column_reference(document, index, address)?)
        }
        UnboundExpression::Add { left, right } => Expression::Add {
            left: Box::new(bind_node(document, index, owner, left)?),
            right: Box::new(bind_node(document, index, owner, right)?),
        },
        UnboundExpression::Subtract { left, right } => Expression::Subtract {
            left: Box::new(bind_node(document, index, owner, left)?),
            right: Box::new(bind_node(document, index, owner, right)?),
        },
        UnboundExpression::Multiply { left, right } => Expression::Multiply {
            left: Box::new(bind_node(document, index, owner, left)?),
            right: Box::new(bind_node(document, index, owner, right)?),
        },
        UnboundExpression::Divide { left, right } => Expression::Divide {
            left: Box::new(bind_node(document, index, owner, left)?),
            right: Box::new(bind_node(document, index, owner, right)?),
        },
        UnboundExpression::Minimum { left, right } => Expression::Minimum {
            left: Box::new(bind_node(document, index, owner, left)?),
            right: Box::new(bind_node(document, index, owner, right)?),
        },
        UnboundExpression::Maximum { left, right } => Expression::Maximum {
            left: Box::new(bind_node(document, index, owner, left)?),
            right: Box::new(bind_node(document, index, owner, right)?),
        },
        UnboundExpression::Compare {
            operator,
            left,
            right,
        } => Expression::Compare {
            operator: *operator,
            left: Box::new(bind_node(document, index, owner, left)?),
            right: Box::new(bind_node(document, index, owner, right)?),
        },
        UnboundExpression::If {
            condition,
            then_value,
            else_value,
        } => Expression::If {
            condition: Box::new(bind_node(document, index, owner, condition)?),
            then_value: Box::new(bind_node(document, index, owner, then_value)?),
            else_value: Box::new(bind_node(document, index, owner, else_value)?),
        },
        UnboundExpression::And(arguments) => {
            Expression::And(bind_arguments(document, index, owner, arguments)?)
        }
        UnboundExpression::Or(arguments) => {
            Expression::Or(bind_arguments(document, index, owner, arguments)?)
        }
        UnboundExpression::Not(value) => {
            Expression::Not(Box::new(bind_node(document, index, owner, value)?))
        }
        UnboundExpression::Round { value, digits } => Expression::Round {
            value: Box::new(bind_node(document, index, owner, value)?),
            digits: bind_round_digits(digits)?,
        },
        UnboundExpression::IsBlank(reference) => Expression::IsBlank(Box::new(
            bind_blank_reference(document, index, owner, reference)?,
        )),
    })
}

fn bind_round_digits(expression: &UnboundExpression) -> Result<u8, FormulaBindError> {
    let UnboundExpression::Number(digits) = expression else {
        return Err(round_digits_error());
    };
    let digits = digits.get();
    if !(0.0..=10.0).contains(&digits) || digits.fract() != 0.0 {
        return Err(round_digits_error());
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    // The literal was checked to be an integer in 0..=10.
    Ok(digits as u8)
}

fn round_digits_error() -> FormulaBindError {
    FormulaBindError::TypeMismatch {
        message: "ROUND digits must be an integer literal in 0..=10".into(),
    }
}

fn bind_arguments(
    document: &Document,
    index: &AddressIndex,
    owner: &FieldRef,
    arguments: &[UnboundExpression],
) -> Result<Vec<Expression>, FormulaBindError> {
    arguments
        .iter()
        .map(|argument| bind_node(document, index, owner, argument))
        .collect()
}

fn bind_blank_reference(
    document: &Document,
    index: &AddressIndex,
    owner: &FieldRef,
    expression: &UnboundExpression,
) -> Result<Expression, FormulaBindError> {
    let (address, same_row) =
        match expression {
            UnboundExpression::Reference(address) => (address.clone(), false),
            UnboundExpression::SameRowReference(field) => {
                let entity = document.entities.get(&owner.entity).ok_or_else(|| {
                    FormulaBindError::Index {
                        source: Box::new(AddressIndexError::MissingBoundEntity {
                            entity: owner.entity.clone(),
                        }),
                    }
                })?;
                (FieldAddress::new(entity.key.clone(), field.clone()), true)
            }
            _ => {
                return Err(FormulaBindError::TypeMismatch {
                    message: "ISBLANK requires a field reference".into(),
                });
            }
        };
    let reference =
        index
            .resolve_field(document, &address)
            .map_err(|source| FormulaBindError::Address {
                address,
                source: Box::new(source),
            })?;
    Ok(if same_row {
        Expression::SameRowReference(reference.field)
    } else {
        Expression::Reference(reference)
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
    owner: &FieldRef,
    expression: &Expression,
) -> Result<String, CanonicalAuthoringProjectionError> {
    validate_expression_structure(expression)?;
    // Column totals are projected through their own identity, so only the
    // direct references need a human-address round-trip proof here.
    let references = direct_references(owner, expression);
    let Ok(index) = AddressIndex::build(document) else {
        return Err(
            CanonicalAuthoringProjectionError::UnresolvableBoundReferences {
                targets: references,
            },
        );
    };
    let mut addresses = BTreeMap::new();
    let mut unresolved = BTreeSet::new();
    for target in references {
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

    let mut columns = BTreeMap::new();
    let mut unresolved_columns = BTreeSet::new();
    for column in extract_column_totals(expression) {
        match column_address(document, &column) {
            Some(address) => {
                columns.insert(column, address);
            }
            None => {
                unresolved_columns.insert(column);
            }
        }
    }
    if !unresolved_columns.is_empty() {
        return Err(
            CanonicalAuthoringProjectionError::UnresolvableBoundColumns {
                columns: unresolved_columns,
            },
        );
    }

    let projected = render_bound(expression, owner, &addresses, &columns);
    if projected.len() > parser::MAX_INPUT_BYTES {
        return Err(ExpressionComplexityError::CanonicalLengthLimit.into());
    }
    Ok(projected)
}

fn render_bound(
    expression: &Expression,
    owner: &FieldRef,
    addresses: &BTreeMap<FieldRef, FieldAddress>,
    columns: &BTreeMap<ColumnRef, ColumnAddress>,
) -> String {
    match expression {
        Expression::Number(number) => parser::format_number(*number),
        Expression::Reference(reference) => {
            let address = &addresses[reference];
            format!("[{}.{}]", address.entity, address.field)
        }
        Expression::SameRowReference(field) => {
            let reference = FieldRef::new(owner.entity.clone(), field.clone());
            format!("[.{}]", addresses[&reference].field)
        }
        Expression::ColumnTotal(column) => {
            let address = &columns[column];
            format!("[{}.{}:total]", address.schema, address.field)
        }
        Expression::Add { left, right } => {
            render_bound_binary(left, "+", right, owner, addresses, columns)
        }
        Expression::Subtract { left, right } => {
            render_bound_binary(left, "-", right, owner, addresses, columns)
        }
        Expression::Multiply { left, right } => {
            render_bound_binary(left, "*", right, owner, addresses, columns)
        }
        Expression::Divide { left, right } => {
            render_bound_binary(left, "/", right, owner, addresses, columns)
        }
        Expression::Minimum { left, right } => format!(
            "min({}, {})",
            render_bound(left, owner, addresses, columns),
            render_bound(right, owner, addresses, columns)
        ),
        Expression::Maximum { left, right } => format!(
            "max({}, {})",
            render_bound(left, owner, addresses, columns),
            render_bound(right, owner, addresses, columns)
        ),
        Expression::Compare {
            operator,
            left,
            right,
        } => render_bound_binary(left, operator.as_str(), right, owner, addresses, columns),
        Expression::If {
            condition,
            then_value,
            else_value,
        } => format!(
            "IF({}, {}, {})",
            render_bound(condition, owner, addresses, columns),
            render_bound(then_value, owner, addresses, columns),
            render_bound(else_value, owner, addresses, columns)
        ),
        Expression::And(arguments) => render_bound_function("AND", arguments, owner, addresses, columns),
        Expression::Or(arguments) => render_bound_function("OR", arguments, owner, addresses, columns),
        Expression::Not(value) => format!("NOT({})", render_bound(value, owner, addresses, columns)),
        Expression::Round { value, digits } => {
            format!("ROUND({}, {digits})", render_bound(value, owner, addresses, columns))
        }
        Expression::IsBlank(value) => format!("ISBLANK({})", render_bound(value, owner, addresses, columns)),
    }
}

fn render_bound_function(
    name: &str,
    arguments: &[Expression],
    owner: &FieldRef,
    addresses: &BTreeMap<FieldRef, FieldAddress>,
    columns: &BTreeMap<ColumnRef, ColumnAddress>,
) -> String {
    format!(
        "{name}({})",
        arguments
            .iter()
            .map(|argument| render_bound(argument, owner, addresses, columns))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn render_bound_binary(
    left: &Expression,
    operator: &str,
    right: &Expression,
    owner: &FieldRef,
    addresses: &BTreeMap<FieldRef, FieldAddress>,
    columns: &BTreeMap<ColumnRef, ColumnAddress>,
) -> String {
    format!(
        "({} {operator} {})",
        render_bound(left, owner, addresses, columns),
        render_bound(right, owner, addresses, columns)
    )
}

/// Resolve a column identity to the document's current human keys.
///
/// The field must still be a Number column, because canonical text only
/// round-trips if the projected address binds back to the same column total.
fn column_address(document: &Document, column: &ColumnRef) -> Option<ColumnAddress> {
    let schema = document.schemas.get(&column.schema)?;
    let field = schema.fields.get(&column.field)?;
    if field.field_type != FieldType::Number {
        return None;
    }
    Some(ColumnAddress::new(schema.key.clone(), field.key.clone()))
}

/// Collect every column identity named by a bound expression.
fn extract_column_totals(expression: &Expression) -> BTreeSet<ColumnRef> {
    expression.column_totals().into_iter().cloned().collect()
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
        stack.extend(
            node.children()
                .into_iter()
                .rev()
                .map(|child| (child, depth + 1)),
        );
    }
    Ok(())
}

/// Collect the value nodes named by direct references (`[e.f]` and `[.f]`).
fn direct_references(owner: &FieldRef, expression: &Expression) -> BTreeSet<FieldRef> {
    expression
        .reference_uses(owner)
        .into_iter()
        .map(|(reference, _)| reference)
        .collect()
}

/// Extract the deterministic static dependency set from a bound expression.
///
/// Same-row references resolve against `owner`, and a column total expands to
/// that column on every entity of the document's current schema, so the set is
/// exactly the stable value nodes the formula reads. Because a column total's
/// expansion follows current row membership, retained incremental state
/// re-extracts it whenever a row of that schema changes.
#[must_use]
pub fn extract_dependencies(
    document: &Document,
    owner: &FieldRef,
    expression: &Expression,
) -> BTreeSet<FieldRef> {
    let mut dependencies = expression
        .reference_uses(owner)
        .into_iter()
        .map(|(reference, _)| reference)
        .collect::<BTreeSet<_>>();
    for column in expression.column_totals() {
        for (entity_id, entity) in &document.entities {
            if entity.schema == column.schema {
                dependencies.insert(FieldRef::new(entity_id.clone(), column.field.clone()));
            }
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

struct FullCalculationParts {
    nodes: BTreeSet<FieldRef>,
    values: BTreeMap<FieldRef, Number>,
    failures: FailureMap,
    dependencies: DependencyMap,
}

impl RetainedCalculationState {
    /// Rebuild all retained formula state through the full ADR-0018 oracle
    /// phases.
    #[must_use]
    pub fn rebuild(document: &Document) -> (Self, IncrementalCalculationWork) {
        let FullCalculationParts {
            nodes,
            values,
            failures,
            dependencies,
        } = full_calculation_parts(document);
        let reverse_dependents = reverse_dependency_index(&dependencies);
        let work = IncrementalCalculationWork {
            full_rebuilds: 1,
            nodes_recomputed: nodes.len(),
            ..IncrementalCalculationWork::default()
        };
        (
            Self {
                nodes,
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
    /// proved, discard this state and call [`Self::rebuild`] instead. Column
    /// totals make row membership part of a formula's dependencies; the
    /// caller only lists changed rows, and this update re-extracts every
    /// column total those rows can affect.
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
        let dirty_roots = &self.with_column_total_roots(document, dirty_roots);
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

    /// A column total's dependencies are the schema's current rows, so a
    /// changed row can change the dependency set of a formula that is not
    /// itself dirty. Promote every column-total formula that read a root
    /// before (old graph) or totals a root's schema now, so its dependencies
    /// are re-extracted rather than left stale.
    fn with_column_total_roots(
        &self,
        document: &Document,
        dirty_roots: &BTreeSet<FieldRef>,
    ) -> BTreeSet<FieldRef> {
        let mut roots = dirty_roots.clone();
        let root_schemas = dirty_roots
            .iter()
            .filter_map(|root| document.entities.get(&root.entity))
            .map(|entity| &entity.schema)
            .collect::<BTreeSet<_>>();
        // A total also holds reverse edges from a root row's blank slots, which
        // are never roots themselves (only stored fields are). Walk every
        // reverse edge of each root's row, so deleting a row whose totalled
        // cell is blank still refreshes the total.
        let root_entities = dirty_roots
            .iter()
            .map(|root| &root.entity)
            .collect::<BTreeSet<_>>();
        for entity in root_entities {
            let first = FieldRef::new(entity.clone(), FieldId::from(String::new()));
            for (_, dependents) in self
                .reverse_dependents
                .range(first..)
                .take_while(|(slot, _)| &slot.entity == entity)
            {
                for dependent in dependents {
                    if matches!(
                        calculation_value(document, dependent),
                        Some(Value::Formula(expression))
                            if !extract_column_totals(expression).is_empty()
                    ) {
                        roots.insert(dependent.clone());
                    }
                }
            }
        }
        if root_schemas.is_empty() {
            return roots;
        }
        for (entity_id, entity) in &document.entities {
            for (field_id, value) in &entity.fields {
                if let Value::Formula(expression) = value {
                    if extract_column_totals(expression)
                        .iter()
                        .any(|column| root_schemas.contains(&column.schema))
                    {
                        roots.insert(FieldRef::new(entity_id.clone(), field_id.clone()));
                    }
                }
            }
        }
        roots
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
                Value::Number(_)
                | Value::Text(_)
                | Value::Boolean(_)
                | Value::Date(_)
                | Value::Reference(_) => None,
            })
            .collect::<FormulaNodes<'_>>();

        for field in dirty {
            self.failures.remove(field);
            self.values.remove(field);
        }
        self.failures
            .extend(pregraph_failures(document, &dirty_nodes, &dirty_formulas));
        assign_cycle_failures(&dirty_formulas, &self.dependencies, &mut self.failures);
        self.values
            .extend(initial_values(&dirty_nodes, &self.failures));
        evaluate_remaining_formulas(
            document,
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
                CalculationFailure::TypeMismatch => {
                    return CalculationError::InvalidExpression {
                        formula: field.clone(),
                        message: "formula type mismatch".into(),
                    };
                }
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
                CalculationFailure::InvalidColumn { columns } => {
                    let (column, column_failure) = columns
                        .first_key_value()
                        .expect("invalid-column failure must contain a column");
                    return CalculationError::InvalidColumn {
                        column: column.clone(),
                        message: match column_failure {
                            ColumnFailure::MissingSchema => "column schema does not exist",
                            ColumnFailure::MissingField => "column field does not exist",
                            ColumnFailure::NonNumeric => "column field is not numeric",
                        }
                        .to_owned(),
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
    TypeMismatch,
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
    InvalidColumn {
        columns: BTreeMap<ColumnRef, ColumnFailure>,
    },
}

/// Binding/type classification for one directly referenced stable value node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceFailure {
    Missing,
    NonNumeric,
}

/// Binding/type classification for one column total named by a formula.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColumnFailure {
    MissingSchema,
    MissingField,
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
    #[error("formula column '{column}' is invalid: {message}")]
    InvalidColumn { column: ColumnRef, message: String },
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
    let FullCalculationParts {
        values,
        failures,
        dependencies,
        ..
    } = full_calculation_parts(document);

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

fn full_calculation_parts(document: &Document) -> FullCalculationParts {
    let (nodes, formulas, dependencies) = collect_calculation_nodes(document);
    let mut failures = pregraph_failures(document, &nodes, &formulas);
    assign_cycle_failures(&formulas, &dependencies, &mut failures);
    let mut values = initial_values(&nodes, &failures);
    evaluate_remaining_formulas(
        document,
        &formulas,
        &dependencies,
        &mut failures,
        &mut values,
    );
    FullCalculationParts {
        nodes: nodes.into_keys().collect(),
        values,
        failures,
        dependencies,
    }
}

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
        Some(Value::Formula(expression)) => Some(extract_dependencies(document, field, expression)),
        Some(
            Value::Number(_)
            | Value::Text(_)
            | Value::Boolean(_)
            | Value::Date(_)
            | Value::Reference(_),
        )
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
) -> FailureMap {
    let mut failures = BTreeMap::new();
    // Phase 1a: structural failures take precedence over every later phase.
    for (formula, expression) in formulas {
        if let Err(error) = validate_expression_structure(expression) {
            failures.insert(
                formula.clone(),
                CalculationFailure::InvalidExpression { error },
            );
        } else if expression.expression_type() != Some(FormulaExpressionType::Number) {
            failures.insert(formula.clone(), CalculationFailure::TypeMismatch);
        }
    }

    // Phase 1b: a column identity that does not resolve to a numeric schema
    // field fails the naming formula before any row-level target check.
    for (formula, expression) in formulas {
        if failures.contains_key(formula) {
            continue;
        }
        let columns = column_failures(document, expression);
        if !columns.is_empty() {
            failures.insert(
                formula.clone(),
                CalculationFailure::InvalidColumn { columns },
            );
        }
    }

    // Phase 1c: capture every directly discovered missing/stale/type target.
    for (field, value) in nodes {
        if failures.contains_key(field) {
            continue;
        }
        let mut invalid_targets = BTreeMap::new();
        if let Some(failure) = reference_failure(document, field, true) {
            invalid_targets.insert(field.clone(), failure);
        }
        if let Value::Formula(expression) = value {
            let unconditional = expression.unconditional_references(field);
            let mut uses = expression.reference_uses(field);
            // A column total reads every non-blank cell of its column; a blank
            // cell is skipped by the sum, not a missing target. Cells read
            // outside IF branches are checked like direct references.
            let unconditional_columns = expression.unconditional_column_totals();
            let mut unconditional_cells = BTreeSet::new();
            for column in expression.column_totals() {
                for cell in column_cells(document, column) {
                    if unconditional_columns.contains(&column) {
                        unconditional_cells.insert(cell.clone());
                    }
                    uses.push((cell, true));
                }
            }
            for (dependency, numeric) in uses {
                // A value missing only inside an IF branch is decided at
                // evaluation, when the branch is actually selected; the
                // reference itself must still be well-formed.
                let failure = if unconditional.contains(&dependency)
                    || unconditional_cells.contains(&dependency)
                {
                    reference_failure(document, &dependency, numeric)
                } else {
                    structural_reference_failure(document, &dependency, numeric)
                };
                if let Some(failure) = failure {
                    invalid_targets.insert(dependency, failure);
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
    document: &Document,
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
        "SCC condensation must be acyclic"
    );
}

fn column_failures(
    document: &Document,
    expression: &Expression,
) -> BTreeMap<ColumnRef, ColumnFailure> {
    let mut failures = BTreeMap::new();
    for column in extract_column_totals(expression) {
        let failure = match document.schemas.get(&column.schema) {
            None => Some(ColumnFailure::MissingSchema),
            Some(schema) => match schema.fields.get(&column.field) {
                None => Some(ColumnFailure::MissingField),
                Some(definition) if definition.field_type != FieldType::Number => {
                    Some(ColumnFailure::NonNumeric)
                }
                Some(_) => None,
            },
        };
        if let Some(failure) = failure {
            failures.insert(column, failure);
        }
    }
    failures
}

/// The non-blank cells a column total sums, in entity order.
pub(crate) fn column_cells<'a>(
    document: &'a Document,
    column: &'a ColumnRef,
) -> impl Iterator<Item = FieldRef> + 'a {
    document
        .entities
        .iter()
        .filter(|(_, entity)| {
            entity.schema == column.schema && entity.fields.contains_key(&column.field)
        })
        .map(|(entity_id, _)| FieldRef::new(entity_id.clone(), column.field.clone()))
}

/// The reference names an existing entity and field of the right type,
/// whatever value it holds now.
fn structural_reference_failure(
    document: &Document,
    field: &FieldRef,
    numeric: bool,
) -> Option<ReferenceFailure> {
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
    (numeric && definition.field_type != FieldType::Number).then_some(ReferenceFailure::NonNumeric)
}

fn reference_failure(
    document: &Document,
    field: &FieldRef,
    numeric: bool,
) -> Option<ReferenceFailure> {
    if let Some(failure) = structural_reference_failure(document, field, numeric) {
        return Some(failure);
    }
    if !numeric {
        return None;
    }
    let entity = &document.entities[&field.entity];
    let definition = &document.schemas[&entity.schema].fields[&field.field];
    match entity.fields.get(&field.field) {
        Some(Value::Number(_) | Value::Formula(_)) => None,
        Some(Value::Text(_) | Value::Boolean(_) | Value::Date(_) | Value::Reference(_)) => {
            Some(ReferenceFailure::NonNumeric)
        }
        None if !definition.required => None,
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
