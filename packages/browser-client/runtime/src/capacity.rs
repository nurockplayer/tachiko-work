//! Candidate-wide resource admission. Counts are derived data, never a second
//! document authority. The full scan can later be replaced by candidate deltas.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use tachiko_workspace_engine::{Document, EntityId, Expression, FieldId, SchemaId, Value};
use thiserror::Error;

use crate::DesignerError;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CapacityLimits {
    pub collections: usize,
    pub table_fields: usize,
    pub table_rows: usize,
    pub total_entities: usize,
    pub populated_cells: usize,
    pub formulas: usize,
    pub formula_ast_nodes: usize,
    pub dependency_edges: usize,
}

pub(crate) const DEFAULT_LIMITS: CapacityLimits = CapacityLimits {
    collections: 32,
    table_fields: 32,
    table_rows: 128,
    total_entities: 1_024,
    populated_cells: 100_000,
    formulas: 32,
    formula_ast_nodes: 100_000,
    dependency_edges: 200_000,
};

/// Preserve the independently admitted Text/Number rectangle. Its content and
/// encoding restrictions remain owned by the existing profile gates.
pub(crate) const fn limits_for_profile(text_capacity: bool) -> CapacityLimits {
    if text_capacity {
        CapacityLimits {
            table_rows: crate::interop_adapter::MAX_TEXT_CAPACITY_ROWS,
            total_entities: crate::interop_adapter::MAX_TEXT_CAPACITY_ROWS,
            ..DEFAULT_LIMITS
        }
    } else {
        DEFAULT_LIMITS
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapacityLimit {
    Collections,
    TableFields,
    TableRows,
    TotalEntities,
    PopulatedCells,
    Formulas,
    FormulaAstNodes,
    DependencyEdges,
}

#[derive(Clone, Debug, Deserialize, Eq, Error, PartialEq, Serialize)]
#[error("{limit:?} capacity exceeded: maximum {maximum}, attempted {attempted}")]
pub struct CapacityExceeded {
    pub limit: CapacityLimit,
    pub maximum: usize,
    pub attempted: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection: Option<String>,
}

/// Use the same typed refusal for inexpensive operation-specific preflights
/// and the complete candidate check.
pub(crate) fn check_limit(
    limit: CapacityLimit,
    maximum: usize,
    attempted: usize,
    collection: Option<&str>,
) -> Result<(), CapacityExceeded> {
    if attempted > maximum {
        return Err(CapacityExceeded {
            limit,
            maximum,
            attempted,
            collection: collection.map(str::to_owned),
        });
    }
    Ok(())
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct TableCapacityUsage {
    pub rows: usize,
    pub fields: usize,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CapacityUsage {
    pub collections: usize,
    pub tables: BTreeMap<String, TableCapacityUsage>,
    /// Includes entities with no stored fields.
    pub total_entities: usize,
    /// Stored entries, including explicit empty Text and formula cells. Missing
    /// fields are blank and consume no populated-cell budget.
    pub populated_cells: usize,
    pub formulas: usize,
    pub formula_ast_nodes: usize,
    /// Unique bound targets per formula, matching the calculation graph.
    pub dependency_edges: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CapacityProjection {
    pub revision: String,
    pub usage: CapacityUsage,
    pub limits: CapacityLimits,
}

impl CapacityUsage {
    pub(crate) fn measure(document: &Document) -> Self {
        let mut usage = Self {
            collections: document.schemas.len(),
            total_entities: document.entities.len(),
            ..Self::default()
        };
        let mut tables = document
            .schemas
            .iter()
            .map(|(id, schema)| {
                (
                    id,
                    (
                        schema.key.to_string(),
                        TableCapacityUsage {
                            fields: schema.fields.len(),
                            rows: 0,
                        },
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut rows_per_schema = BTreeMap::<&SchemaId, usize>::new();
        for entity in document.entities.values() {
            *rows_per_schema.entry(&entity.schema).or_default() += 1;
        }
        for entity in document.entities.values() {
            if let Some((_, table)) = tables.get_mut(&entity.schema) {
                table.rows += 1;
                // Invalid documents can carry undeclared stored fields. Bound
                // their resource use too, before semantic validation refuses.
                table.fields = table.fields.max(entity.fields.len());
            }
            usage.populated_cells += entity.fields.len();
            for value in entity.fields.values() {
                if let Value::Formula(expression) = value {
                    usage.formulas += 1;
                    let (nodes, edges) = formula_usage(&rows_per_schema, &entity.id, expression);
                    usage.formula_ast_nodes += nodes;
                    usage.dependency_edges += edges;
                }
            }
        }
        usage.tables = tables.into_values().collect();
        usage
    }

    pub(crate) fn check(&self, limits: CapacityLimits) -> Result<(), DesignerError> {
        check_limit(
            CapacityLimit::Collections,
            limits.collections,
            self.collections,
            None,
        )?;
        for (collection, table) in &self.tables {
            check_limit(
                CapacityLimit::TableFields,
                limits.table_fields,
                table.fields,
                Some(collection),
            )?;
            check_limit(
                CapacityLimit::TableRows,
                limits.table_rows,
                table.rows,
                Some(collection),
            )?;
        }
        for (limit, maximum, attempted) in [
            (
                CapacityLimit::TotalEntities,
                limits.total_entities,
                self.total_entities,
            ),
            (
                CapacityLimit::PopulatedCells,
                limits.populated_cells,
                self.populated_cells,
            ),
            (CapacityLimit::Formulas, limits.formulas, self.formulas),
            (
                CapacityLimit::FormulaAstNodes,
                limits.formula_ast_nodes,
                self.formula_ast_nodes,
            ),
            (
                CapacityLimit::DependencyEdges,
                limits.dependency_edges,
                self.dependency_edges,
            ),
        ] {
            check_limit(limit, maximum, attempted, None)?;
        }
        Ok(())
    }
}

fn formula_usage<'a>(
    rows_per_schema: &BTreeMap<&SchemaId, usize>,
    owner: &'a EntityId,
    expression: &'a Expression,
) -> (usize, usize) {
    let mut pending = vec![expression];
    let mut nodes = 0;
    // Borrow identities so counting never copies formula trees or ID strings.
    let mut dependencies = BTreeSet::<(&EntityId, &FieldId)>::new();
    let mut columns = BTreeSet::<(&SchemaId, &FieldId)>::new();
    while let Some(expression) = pending.pop() {
        nodes += 1;
        match expression {
            Expression::Reference(reference) => {
                dependencies.insert((&reference.entity, &reference.field));
            }
            Expression::SameRowReference(field) => {
                dependencies.insert((owner, field));
            }
            Expression::ColumnTotal(column) => {
                columns.insert((&column.schema, &column.field));
            }
            _ => pending.extend(expression.children()),
        }
    }
    // A column total depends on that column in every row of its table. Count
    // the rows instead of listing them, so accounting stays linear in the
    // document even with many totals.
    let column_edges = columns
        .iter()
        .map(|(schema, _)| rows_per_schema.get(schema).copied().unwrap_or(0))
        .sum::<usize>();
    (nodes, dependencies.len() + column_edges)
}

pub(crate) fn check_document(
    document: &Document,
    limits: CapacityLimits,
) -> Result<CapacityUsage, DesignerError> {
    let usage = CapacityUsage::measure(document);
    usage.check(limits)?;
    Ok(usage)
}
