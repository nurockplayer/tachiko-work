//! Candidate-wide resource admission. Counts are derived data, never a second
//! document authority. The full scan can later be replaced by candidate deltas.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use tachiko_workspace_engine::{
    ConditionalAggregate, Document, EntityId, Expression, FieldId, SchemaId, Value,
};
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

// Plan A-06b scratch measurement: the admission profile is lifted far beyond
// the 100k-cell fixtures so one runtime edit can be timed. Never merge.
pub(crate) const DEFAULT_LIMITS: CapacityLimits = CapacityLimits {
    collections: 32,
    table_fields: 32,
    table_rows: 1_000_000,
    total_entities: 1_000_000,
    populated_cells: 10_000_000,
    formulas: 1_000_000,
    formula_ast_nodes: 10_000_000,
    // Plan W1 scratch measurement: lifted from 20,000,000. A 10,000-row
    // column-total or column-conditional shape counts each formula's whole
    // column dependency (up to 200M edges at 10,000 rows). Never merge.
    dependency_edges: 1_000_000_000,
};

/// Sheet's native-table target: 10,000 rows, with 100,000 cells and formulas.
pub(crate) const WORKING_LIMITS: CapacityLimits = CapacityLimits {
    table_rows: 10_000,
    total_entities: 10_000,
    populated_cells: 100_000,
    formulas: 100_000,
    formula_ast_nodes: 1_000_000,
    dependency_edges: 1_000_000,
    ..DEFAULT_LIMITS
};

/// What a session was admitted under. Chosen when the occurrence is created and
/// kept through every edit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SessionProfile {
    Ordinary,
    TextCapacity,
    WorkingSize,
}

/// Largest v3 save a working-size edit may leave behind. One mebibyte of the
/// 64 MiB project-transfer limit stays free for `export_portable_ro`'s ZIP
/// framing around the same file bodies.
pub(crate) const MAX_WORKING_SAVED_BYTES: usize = crate::MAX_PROJECT_TRANSFER_BYTES - 1024 * 1024;

/// Refuse a working-size document whose v3 save could pass the transfer limit.
///
/// The admission gate runs before anything publishes, so an edit that cannot
/// be saved is refused instead of stranding the session at save time.
pub(crate) fn check_working_saved_bytes(
    document: &Document,
    presentation_saved_bytes: u64,
) -> Result<(), DesignerError> {
    check_working_saved_bytes_bound(
        crate::save_size::saved_bytes_upper_bound(document),
        presentation_saved_bytes,
    )
}

/// The same refusal from a document bound the caller already computed.
pub(crate) fn check_working_saved_bytes_bound(
    document_saved_bytes: u64,
    presentation_saved_bytes: u64,
) -> Result<(), DesignerError> {
    // Compare in `u64`: on wasm32 `usize` is 32 bits and a working-size
    // document can legitimately bound above 4 GiB.
    let attempted = document_saved_bytes.saturating_add(presentation_saved_bytes);
    let maximum = u64::try_from(MAX_WORKING_SAVED_BYTES).unwrap_or(u64::MAX);
    if attempted <= maximum {
        return Ok(());
    }
    Err(CapacityExceeded {
        limit: CapacityLimit::SavedBytes,
        maximum: MAX_WORKING_SAVED_BYTES,
        attempted: usize::try_from(attempted).unwrap_or(usize::MAX),
        collection: None,
    }
    .into())
}

/// Preserve the independently admitted Text/Number rectangle. Its content and
/// encoding restrictions remain owned by the existing profile gates.
pub(crate) const fn limits_for_profile(profile: SessionProfile) -> CapacityLimits {
    match profile {
        SessionProfile::Ordinary => DEFAULT_LIMITS,
        SessionProfile::WorkingSize => WORKING_LIMITS,
        SessionProfile::TextCapacity => CapacityLimits {
            table_rows: crate::interop_adapter::MAX_TEXT_CAPACITY_ROWS,
            total_entities: crate::interop_adapter::MAX_TEXT_CAPACITY_ROWS,
            ..DEFAULT_LIMITS
        },
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
    ProjectionBytes,
    SavedBytes,
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

impl CapacityExceeded {
    pub(crate) fn import_message(&self, at_least: bool) -> String {
        if self.limit == CapacityLimit::ProjectionBytes {
            return format!(
                "Workbook is too large to open: it needs {} bytes and the limit is {}. The workbook was not imported.",
                self.attempted, self.maximum
            );
        }
        let subject = self
            .collection
            .as_ref()
            .map_or_else(|| "Workbook".to_owned(), |name| format!("Sheet ‘{name}’"));
        let unit = match self.limit {
            CapacityLimit::Collections => "sheets",
            CapacityLimit::TableFields => "columns",
            CapacityLimit::TableRows | CapacityLimit::TotalEntities => "data rows",
            CapacityLimit::Formulas => "formulas",
            _ => "items",
        };
        let bound = if at_least { "at least " } else { "" };
        format!(
            "{subject} has {bound}{} {unit}; the maximum is {}. The workbook was not imported.",
            self.attempted, self.maximum
        )
    }
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
    #[cfg(test)]
    pub(crate) fn measure(document: &Document) -> Self {
        Self::measure_for(document, SessionProfile::Ordinary)
    }

    pub(crate) fn measure_for(document: &Document, profile: SessionProfile) -> Self {
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
                    let (nodes, edges) =
                        formula_usage(profile, &rows_per_schema, &entity.id, expression);
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
    profile: SessionProfile,
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
            Expression::ColumnTotal(column) | Expression::ColumnAggregate { column, .. } => {
                columns.insert((&column.schema, &column.field));
            }
            Expression::ColumnConditional {
                aggregate,
                criteria,
            } => {
                if let ConditionalAggregate::Sum(column) | ConditionalAggregate::Average(column) =
                    aggregate.as_ref()
                {
                    columns.insert((&column.schema, &column.field));
                }
                for criterion in criteria {
                    columns.insert((&criterion.column.schema, &criterion.column.field));
                    if let Some(expression) = &criterion.value {
                        pending.push(expression.as_ref());
                    }
                }
            }
            _ => pending.extend(expression.children()),
        }
    }
    // A column total depends on that column in every row of its table. Count
    // the rows instead of listing them, so accounting stays linear in the
    // document even with many totals. A working-size session reads each
    // column once, so it counts one edge per column instead.
    let column_edges = match profile {
        SessionProfile::WorkingSize => columns.len(),
        SessionProfile::Ordinary | SessionProfile::TextCapacity => columns
            .iter()
            .map(|(schema, _)| rows_per_schema.get(schema).copied().unwrap_or(0))
            .sum::<usize>(),
    };
    (nodes, dependencies.len() + column_edges)
}

#[cfg(test)]
pub(crate) fn check_document(
    document: &Document,
    limits: CapacityLimits,
) -> Result<CapacityUsage, DesignerError> {
    let usage = CapacityUsage::measure(document);
    usage.check(limits)?;
    Ok(usage)
}

pub(crate) fn check_document_for(
    document: &Document,
    profile: SessionProfile,
) -> Result<CapacityUsage, DesignerError> {
    let usage = CapacityUsage::measure_for(document, profile);
    usage.check(limits_for_profile(profile))?;
    Ok(usage)
}

/// Candidate totals for a batch that only replaces stored values on entities
/// the base document already holds.
///
/// Each touched cell leaves the base totals by its old contribution and joins
/// them by its new one. Row counts cannot change, so formulas reuse the base's
/// rows per schema instead of recounting every entity.
pub(crate) fn value_edit_usage(
    profile: SessionProfile,
    base: &CapacityUsage,
    base_document: &Document,
    candidate: &Document,
    targets: &BTreeSet<(EntityId, FieldId)>,
) -> CapacityUsage {
    let mut usage = base.clone();
    let rows_per_schema = rows_per_schema(base, base_document);
    let mut touched_entities = BTreeSet::new();
    for (entity_id, field_id) in targets {
        let Some(base_entity) = base_document.entities.get(entity_id) else {
            continue;
        };
        touched_entities.insert(entity_id);
        let old = base_entity.fields.get(field_id);
        let new = candidate
            .entities
            .get(entity_id)
            .and_then(|entity| entity.fields.get(field_id));
        usage.populated_cells = usage
            .populated_cells
            .saturating_add(usize::from(new.is_some()))
            .saturating_sub(usize::from(old.is_some()));
        let old_formula = stored_formula(old);
        let new_formula = stored_formula(new);
        // `formulas` can never exceed `populated_cells`: a formula is a stored
        // cell, and a value edit that adds one adds a populated cell in the
        // same step. `check` tests `PopulatedCells` first with the same bound,
        // so a value edit cannot cross the Formulas limit on its own.
        usage.formulas = usage
            .formulas
            .saturating_add(usize::from(new_formula.is_some()))
            .saturating_sub(usize::from(old_formula.is_some()));
        if let Some(expression) = old_formula {
            let (nodes, edges) =
                formula_usage(profile, &rows_per_schema, &base_entity.id, expression);
            usage.formula_ast_nodes = usage.formula_ast_nodes.saturating_sub(nodes);
            usage.dependency_edges = usage.dependency_edges.saturating_sub(edges);
        }
        if let Some(expression) = new_formula {
            let (nodes, edges) =
                formula_usage(profile, &rows_per_schema, &base_entity.id, expression);
            usage.formula_ast_nodes = usage.formula_ast_nodes.saturating_add(nodes);
            usage.dependency_edges = usage.dependency_edges.saturating_add(edges);
        }
    }
    // A stored entry can only add a field, never remove one, so the largest
    // stored-field count only grows.
    for entity_id in touched_entities {
        let Some(entity) = candidate.entities.get(entity_id) else {
            continue;
        };
        let Some(schema) = base_document.schemas.get(&entity.schema) else {
            continue;
        };
        if let Some(table) = usage.tables.get_mut(schema.key.as_str()) {
            table.fields = table.fields.max(entity.fields.len());
        }
    }
    usage
}

/// The formula an optional stored value carries, if any.
fn stored_formula(value: Option<&Value>) -> Option<&Expression> {
    match value {
        Some(Value::Formula(expression)) => Some(expression),
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

/// Rows per schema read back from the base totals, which already counted them.
fn rows_per_schema<'a>(
    usage: &CapacityUsage,
    document: &'a Document,
) -> BTreeMap<&'a SchemaId, usize> {
    document
        .schemas
        .iter()
        .map(|(id, schema)| {
            let rows = usage
                .tables
                .get(schema.key.as_str())
                .map_or(0, |table| table.rows);
            (id, rows)
        })
        .collect()
}
