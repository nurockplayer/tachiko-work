//! Collection table replies shared by full and windowed readers.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{CollectionSummary, ColumnProjection, RowProjection, deserialize_column_totals};

/// Collection eligibility for the cell-style commands at the projected revision.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", content = "reason", rename_all = "snake_case")]
pub enum CellStylingCapability {
    Supported,
    Unsupported(CellStylingUnsupportedReason),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CellStylingUnsupportedReason {
    ReadOnlyTable,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TableProjection {
    pub revision: String,
    pub cell_styling: CellStylingCapability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracker_profile: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_table_profile: Option<bool>,
    pub collection: CollectionSummary,
    pub columns: Vec<ColumnProjection>,
    pub rows: Vec<RowProjection>,
    /// One total per Number column, keyed by column id and observed at the same
    /// revision as `rows`. A null total means a contributing value is
    /// unavailable or not a number; blank cells are skipped, as in a formula
    /// column total.
    #[serde(default, deserialize_with = "deserialize_column_totals")]
    pub column_totals: BTreeMap<String, Option<f64>>,
}

/// One bounded window of a collection table at one revision.
///
/// `rows` is exactly the `RowProjection` shape of the complete table query and
/// `columns` the same column projection. `total_rows` counts the rows the view
/// admits (including kept entities); `unfiltered_rows` counts every row in the
/// collection. `start` is clamped to `total_rows`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TableWindowProjection {
    pub revision: String,
    pub cell_styling: CellStylingCapability,
    /// The same profile flags `queryTable` carries, so a client reading only
    /// windows knows whether native row and column commands apply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracker_profile: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_table_profile: Option<bool>,
    pub collection: CollectionSummary,
    pub columns: Vec<ColumnProjection>,
    /// Deterministic digest of the column definitions. It changes only when a
    /// column definition changes, never for an ordinary row edit.
    pub columns_revision: String,
    pub total_rows: usize,
    pub unfiltered_rows: usize,
    pub start: usize,
    pub rows: Vec<RowProjection>,
    /// Set when the reply's bytes ended this window before its count. Continue
    /// at `start + rows.len()`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub byte_limited: bool,
    /// Whole-collection totals, one per Number column, identical to
    /// `TableProjection::column_totals` at the same revision. View filters do
    /// not apply, and blank cells are skipped.
    #[serde(default, deserialize_with = "deserialize_column_totals")]
    pub column_totals: BTreeMap<String, Option<f64>>,
}
