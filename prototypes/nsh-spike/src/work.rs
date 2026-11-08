//! The Work side of the spike: one Work runtime occurrence behind the private
//! wire protocol, with no NSH types in this module.
//!
//! `host_bridge::DesignerHost` is compiled only for non-WASM targets, so this
//! module calls the public runtime functions it wraps — `open_local_document`,
//! `open_project`, `process_wire_request` and `DesignerRuntime::export_project`
//! — which compile for the web target too.

use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::{Value, json};
use tachiko_designer_runtime::{
    CalculationProjection, DesignerRuntime, FieldProjection, OpenedProjection, ProjectExport,
    ScalarKind, StoredValueProjection, TableProjection, open_local_document, open_project,
    process_wire_request,
};

/// The checked-in Moonfall balance fixture, embedded so the app does not
/// depend on the process working directory.
pub const FIXTURE: &[u8] = include_bytes!("../../../examples/game-balance/game-balance.ro");

/// What the first cell of the fixture's default table says.
pub const FIXTURE_TITLE: &str = "Moonfall: starter balance";

/// A fresh trusted occurrence identity per open, as the runtime requires.
fn fresh_occurrence() -> String {
    static NEXT: AtomicU32 = AtomicU32::new(0x648);
    let serial = NEXT.fetch_add(1, Ordering::Relaxed);
    format!("00000000-0000-4000-8000-{serial:012x}")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collection {
    pub id: String,
    pub display_name: String,
    pub entity_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    pub id: String,
    pub key: String,
    pub display_name: String,
    pub field_type: String,
}

/// One directly stored scalar the runtime accepts for a cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKind {
    Text,
    Number,
    Boolean,
    Date,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub entity: String,
    pub field: String,
    pub text: String,
    pub editable: Option<EditKind>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub entity: String,
    pub key: String,
    pub cells: Vec<Cell>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub collection: Collection,
    pub revision: String,
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Opened {
    pub title: String,
    pub default_collection: String,
    pub collections: Vec<Collection>,
    pub table: Table,
}

/// The resident Work occurrence, opened through the same private wire
/// protocol the browser and native hosts use.
pub struct WorkClient {
    runtime: Option<DesignerRuntime>,
}

impl Default for WorkClient {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkClient {
    #[must_use]
    pub fn new() -> Self {
        Self { runtime: None }
    }

    /// Open the embedded Moonfall balance fixture.
    pub fn open_fixture(&mut self) -> Result<Opened, String> {
        self.open_local(FIXTURE)
    }

    /// Admit a raw local `.ro` document.
    pub fn open_local(&mut self, bytes: &[u8]) -> Result<Opened, String> {
        let opened = open_local_document(&mut self.runtime, bytes, &fresh_occurrence())
            .map_err(|error| error.to_string())?;
        Ok(Self::opened(opened))
    }

    /// Admit the bytes a successful [`Self::export`] produced.
    pub fn open_project(&mut self, bytes: &[u8]) -> Result<Opened, String> {
        let opened = open_project(&mut self.runtime, bytes, &fresh_occurrence())
            .map_err(|error| error.to_string())?;
        Ok(Self::opened(opened))
    }

    /// Read one whole collection table at its current revision.
    pub fn query_table(&mut self, collection: &str) -> Result<Table, String> {
        let request = serde_json::to_vec(&json!({
            "type": "query_table",
            "collection": collection,
        }))
        .map_err(|error| error.to_string())?;
        let response = ok_response(&process_wire_request(&mut self.runtime, &request))?;
        let projection: TableProjection = serde_json::from_value(response)
            .map_err(|error| format!("unexpected table reply: {error}"))?;
        Ok(table_of(projection))
    }

    /// Commit one text edit through `edit_scalar`, then return the table at
    /// the resulting revision.
    pub fn edit_cell(
        &mut self,
        table: &Table,
        row: usize,
        column: usize,
        text: &str,
    ) -> Result<Table, String> {
        let cell = table
            .rows
            .get(row)
            .and_then(|row| row.cells.get(column))
            .ok_or_else(|| format!("no cell at row {row} column {column}"))?;
        let kind = cell
            .editable
            .ok_or_else(|| "the cell holds no directly stored scalar".to_owned())?;
        let input = match kind {
            EditKind::Text => json!({ "kind": "text", "value": text }),
            EditKind::Number => json!({ "kind": "number", "input": text }),
            EditKind::Boolean => json!({ "kind": "boolean", "value": text.eq_ignore_ascii_case("true") }),
            EditKind::Date => json!({ "kind": "date", "value": text }),
        };
        let request = serde_json::to_vec(&json!({
            "type": "edit_scalar",
            "expected_revision": table.revision,
            "target": { "entity": cell.entity, "field": cell.field },
            "input": input,
        }))
        .map_err(|error| error.to_string())?;
        ok_response(&process_wire_request(&mut self.runtime, &request))?;
        self.query_table(&table.collection.id)
    }

    /// Export the exact resident revision as an opaque project bundle.
    pub fn export(&self, table: &Table) -> Result<Vec<u8>, String> {
        let export: ProjectExport = self
            .runtime
            .as_ref()
            .ok_or_else(|| "no project is open".to_owned())?
            .export_project(&table.revision)
            .map_err(|error| error.to_string())?;
        Ok(export.bytes)
    }

    fn opened(opened: OpenedProjection) -> Opened {
        Opened {
            title: opened.bootstrap.title,
            default_collection: opened.bootstrap.default_collection,
            collections: opened
                .bootstrap
                .collections
                .into_iter()
                .map(|collection| Collection {
                    id: collection.id,
                    display_name: collection.display_name,
                    entity_count: collection.entity_count,
                })
                .collect(),
            table: table_of(opened.table),
        }
    }
}

fn ok_response(reply: &[u8]) -> Result<Value, String> {
    let reply: Value =
        serde_json::from_slice(reply).map_err(|error| format!("reply is not JSON: {error}"))?;
    if reply["status"] != "ok" {
        let code = reply["error"]["code"].as_str().unwrap_or("unknown");
        let message = reply["error"]["message"].as_str().unwrap_or("no message");
        return Err(format!("{code}: {message}"));
    }
    Ok(reply["response"]["payload"].clone())
}

fn table_of(projection: TableProjection) -> Table {
    let collection = Collection {
        id: projection.collection.id,
        display_name: projection.collection.display_name,
        entity_count: projection.collection.entity_count,
    };
    let columns: Vec<Column> = projection
        .columns
        .into_iter()
        .map(|column| Column {
            id: column.id,
            key: column.key,
            display_name: column.display_name,
            field_type: column.field_type,
        })
        .collect();
    let rows = projection
        .rows
        .into_iter()
        .map(|row| Row {
            cells: columns
                .iter()
                .map(|column| {
                    row.fields
                        .iter()
                        .find(|field| field.target.field == column.id)
                        .map_or_else(
                            || Cell {
                                entity: row.id.clone(),
                                field: column.id.clone(),
                                text: String::new(),
                                editable: None,
                            },
                            cell_of,
                        )
                })
                .collect(),
            entity: row.id,
            key: row.key,
        })
        .collect();
    Table {
        collection,
        revision: projection.revision,
        columns,
        rows,
    }
}

fn cell_of(field: &FieldProjection) -> Cell {
    Cell {
        entity: field.target.entity.clone(),
        field: field.target.field.clone(),
        text: field_text(field),
        editable: field.editable_scalar.map(|kind| match kind {
            ScalarKind::Text => EditKind::Text,
            ScalarKind::Number => EditKind::Number,
            ScalarKind::Boolean => EditKind::Boolean,
            ScalarKind::Date => EditKind::Date,
        }),
    }
}

fn field_text(field: &FieldProjection) -> String {
    if let Some(stored) = &field.stored {
        return stored_text(stored);
    }
    match &field.calculated {
        Some(CalculationProjection::Value { value }) => number_text(*value),
        Some(CalculationProjection::Failure { code, .. }) => format!("ERR({code})"),
        Some(CalculationProjection::Unavailable) | None => String::new(),
    }
}

fn stored_text(stored: &StoredValueProjection) -> String {
    match stored {
        StoredValueProjection::Number { value } => number_text(*value),
        StoredValueProjection::Text { value } => value.clone(),
        StoredValueProjection::Boolean { value } => value.to_string(),
        StoredValueProjection::Date { value } => value.to_string(),
        StoredValueProjection::Reference { entity } => entity.clone(),
    }
}

/// The display form the runtime's own projections use for whole numbers.
fn number_text(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_text_cell(table: &Table) -> (usize, usize) {
        for (row_index, row) in table.rows.iter().enumerate() {
            for (column_index, cell) in row.cells.iter().enumerate() {
                if cell.editable == Some(EditKind::Text) {
                    return (row_index, column_index);
                }
            }
        }
        panic!("the fixture exposes an editable text cell");
    }

    #[test]
    fn fixture_lists_its_tables_and_cells() {
        let mut client = WorkClient::new();
        let opened = client.open_fixture().expect("the fixture opens");
        assert_eq!(opened.title, FIXTURE_TITLE);
        assert_eq!(opened.collections.len(), 4, "{:?}", opened.collections);
        assert!(!opened.table.rows.is_empty());
        assert!(!opened.table.columns.is_empty());
        let (row, column) = first_text_cell(&opened.table);
        assert_eq!(
            opened.table.rows[row].cells[column].text,
            "Iron Sword",
            "the first editable text cell of the default table"
        );
    }

    #[test]
    fn edit_survives_export_and_reopen() {
        let mut client = WorkClient::new();
        let opened = client.open_fixture().expect("the fixture opens");
        let table = client
            .query_table(&opened.default_collection)
            .expect("the default table queries");
        let (row, column) = first_text_cell(&table);
        let edited = client
            .edit_cell(&table, row, column, "Alric spike")
            .expect("the text edit commits");
        assert_eq!(edited.rows[row].cells[column].text, "Alric spike");

        let exported = client.export(&edited).expect("the edit exports");
        let mut reopened = WorkClient::new();
        let reopened_opened = reopened.open_project(&exported).expect("the export opens");
        let reopened_table = reopened
            .query_table(&reopened_opened.default_collection)
            .expect("the reopened table queries");
        assert_eq!(reopened_table.rows[row].cells[column].text, "Alric spike");
    }

    #[test]
    fn a_stale_revision_is_refused() {
        let mut client = WorkClient::new();
        let opened = client.open_fixture().expect("the fixture opens");
        let table = client
            .query_table(&opened.default_collection)
            .expect("the default table queries");
        let (row, column) = first_text_cell(&table);
        let after = client
            .edit_cell(&table, row, column, "Alric spike")
            .expect("the first edit commits");
        let error = client
            .edit_cell(&table, row, column, "Alric again")
            .expect_err("the stale table revision is refused");
        assert!(error.contains("stale"), "{error}");
        assert_eq!(after.rows[row].cells[column].text, "Alric spike");
    }
}
