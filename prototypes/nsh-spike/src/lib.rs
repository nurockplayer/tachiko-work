//! The spike's Rust UI: a Work table screen and a synthetic grid screen.
//!
//! Both screens live in one root (`App`) with two tabs. Every signal lives in
//! the root, so switching tabs keeps each screen's state (the lead and the
//! report treat that as a spike choice, not a design).

mod work;

use std::collections::HashMap;

use nsh_base::data_table::{
    DataTable, DataTableApi, DataTableCell, DataTableColumn, DataTableRange, DataTableRows,
    DataTableSelection, DataTableTarget, data_table,
};
use nsh_base::theme;
use nsh_core::prelude::*;

/// The app's UI set (`nsh.json` `"rust"`): what a TSX entry may import from
/// `tachiko-nsh-spike/native`.
#[native_module("tachiko-nsh-spike/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
}

/// Where **Save** writes the exported project by default.
const DEFAULT_RO_PATH: &str = "/tmp/nsh-spike-roundtrip.ro";

/// Work's own per-table limit; the spike shows it instead of hiding it.
const WORK_ROW_CAPACITY: usize = 128;

const GRID_ROWS: u32 = 100_000;
const GRID_COLUMNS: u32 = 50;

/// The application's root component.
#[component]
pub fn App() -> impl IntoElement {
    let tab = signal(0_u8);
    let work_state = WorkState::new();
    let grid_state = GridState::new();
    view()
        .flex_1()
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        .child(
            view()
                .flex_row()
                .gap_2()
                .items_center()
                .p_4()
                .child(button("tab-work", "Work table", move || tab.set(0)))
                .child(button("tab-grid", "Grid stress", move || tab.set(1))),
        )
        .child(
            when(
                move || tab.get() == 0,
                move || work_screen(work_state),
            )
            .otherwise(move || grid_screen(grid_state)),
        )
}

fn button(
    id: &'static str,
    label: &'static str,
    on_press: impl Fn() + 'static,
) -> impl IntoElement {
    view()
        .test_id(id)
        .px_3()
        .py_2()
        .rounded(px(8.0))
        .bg(move || theme().secondary)
        .active(|s| s.opacity(0.6))
        .on_press(on_press)
        .child(
            text(label)
                .text_size(px(14.0))
                .text_color(move || theme().secondary_foreground),
        )
}

// ---------------------------------------------------------------------------
// Screen A: the opened Work table.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct WorkState {
    client: Stored<work::WorkClient>,
    opened: Signal<Option<work::Opened>>,
    status: Signal<String>,
    path: Signal<String>,
    selected: Signal<Option<(usize, usize)>>,
    editor: Signal<String>,
}

impl WorkState {
    fn new() -> Self {
        Self {
            client: stored(work::WorkClient::new()),
            opened: signal(None),
            status: signal(String::from("no project open")),
            path: signal(DEFAULT_RO_PATH.to_owned()),
            selected: signal(None),
            editor: signal(String::new()),
        }
    }

    fn open_fixture(&self) {
        match self.client.with(work::WorkClient::open_fixture) {
            Ok(opened) => self.show(opened, "opened"),
            Err(error) => self.status.set(format!("open failed: {error}")),
        }
    }

    fn show(&self, opened: work::Opened, verb: &str) {
        let first = first_editable_cell(&opened.table);
        self.selected.set(first);
        self.editor
            .set(first.map_or_else(String::new, |cell| cell_text(&opened.table, cell).0));
        self.status.set(format!(
            "{verb} {} ({} collections, {} x {})",
            opened.title,
            opened.collections.len(),
            opened.table.rows.len(),
            opened.table.columns.len(),
        ));
        self.opened.set(Some(opened));
    }

    /// The commit of the editor: one `edit_scalar` through `work.rs`.
    fn commit(&self) {
        let Some(opened) = self.opened.get() else {
            self.status.set(String::from("no project open"));
            return;
        };
        let Some((row, column)) = self.selected.get() else {
            self.status.set(String::from("no cell selected"));
            return;
        };
        let (_, label) = cell_text(&opened.table, (row, column));
        let text = self.editor.get();
        match self
            .client
            .with(|client| client.edit_cell(&opened.table, row, column, &text))
        {
            Ok(table) => {
                let mut next = opened;
                next.table = table;
                self.opened.set(Some(next));
                self.status.set(format!("edited {label} = {text}"));
            }
            Err(error) => self.status.set(format!("edit failed: {error}")),
        }
    }

    /// Export the resident revision and write it where the path input says.
    fn save(&self) {
        let Some(opened) = self.opened.get() else {
            self.status.set(String::from("no project open"));
            return;
        };
        let path = self.path.get();
        match self.client.with(|client| client.export(&opened.table)) {
            Ok(bytes) => match write_file(&path, &bytes) {
                Ok(()) => self
                    .status
                    .set(format!("saved {} bytes to {path}", bytes.len())),
                Err(error) => self.status.set(format!("save failed: {error}")),
            },
            Err(error) => self.status.set(format!("export failed: {error}")),
        }
    }

    /// Read the file back as an exported project and open it.
    fn reopen(&self) {
        let path = self.path.get();
        let bytes = match read_file(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.status.set(format!("reopen failed: {error}"));
                return;
            }
        };
        match self.client.with(|client| client.open_project(&bytes)) {
            Ok(opened) => {
                let text = first_editable_cell(&opened.table)
                    .map_or_else(String::new, |cell| cell_text(&opened.table, cell).0);
                self.show(opened, "reopened");
                self.status
                    .set(format!("reopened: {}", if text.is_empty() { "?" } else { &text }));
            }
            Err(error) => self.status.set(format!("reopen failed: {error}")),
        }
    }

    /// A selection the user made (press or keyboard) fills the editor.
    fn on_selection(&self, selection: &Option<DataTableSelection>) {
        let Some(selection) = *selection else { return };
        let (Some(row), Some(column)) = (selection.row, selection.column) else {
            return;
        };
        let (row, column) = (row as usize, column as usize);
        self.selected.set(Some((row, column)));
        if let Some(opened) = self.opened.get() {
            self.editor.set(cell_text(&opened.table, (row, column)).0);
        }
    }
}

fn work_screen(state: WorkState) -> impl IntoElement {
    let client_rows = move || match state.opened.get() {
        Some(opened) => work_data(&opened.table),
        None => DataTableRows::default(),
    };
    let columns = move || match state.opened.get() {
        Some(opened) => work_columns(&opened.table),
        None => Vec::new(),
    };
    let selection = move || state.selected.get().map(selection_of);
    view()
        .flex_1()
        .gap_2()
        .p_4()
        .child(
            view()
                .flex_row()
                .gap_2()
                .items_center()
                .child(button("open-fixture", "Open fixture", move || {
                    state.open_fixture();
                }))
                .child(button("save", "Save", move || state.save()))
                .child(button("reopen", "Reopen", move || state.reopen()))
                .child(
                    input()
                        .test_id("path")
                        .w(px(260.0))
                        .value(move || state.path.get())
                        .on_change(move |text: &str| state.path.set(text.to_owned())),
                ),
        )
        .child(
            text(move || format!(
                "Work capacity: {WORK_ROW_CAPACITY} rows per table (Work limit, not NSH)"
            ))
            .text_size(px(12.0)),
        )
        .child(view().test_id("status").child(
            text(move || state.status.get()).text_size(px(13.0)),
        ))
        .child(
            view()
                .flex_row()
                .gap_2()
                .items_center()
                .child(
                    input()
                        .test_id("cell-editor")
                        .w(px(260.0))
                        .value(move || state.editor.get())
                        .on_change(move |text: &str| state.editor.set(text.to_owned())),
                )
                .child(button("commit", "Commit cell", move || state.commit())),
        )
        .child(
            data_table()
                .test_id("work-table")
                .flex_1()
                .columns(columns)
                .data(client_rows)
                .cell_selectable(true)
                .row_header(true)
                .selection(selection)
                .on_selection_change(move |selection: &Option<DataTableSelection>| {
                    state.on_selection(selection);
                }),
        )
}

fn first_editable_cell(table: &work::Table) -> Option<(usize, usize)> {
    for (row_index, row) in table.rows.iter().enumerate() {
        for (column_index, cell) in row.cells.iter().enumerate() {
            if cell.editable == Some(work::EditKind::Text) {
                return Some((row_index, column_index));
            }
        }
    }
    None
}

/// The cell's text and a `entity.field` label for status lines.
fn cell_text(table: &work::Table, (row, column): (usize, usize)) -> (String, String) {
    table
        .rows
        .get(row)
        .and_then(|row| row.cells.get(column))
        .map_or_else(
            || (String::new(), format!("row {row} column {column}")),
            |cell| (cell.text.clone(), format!("{}.{}", cell.entity, cell.field)),
        )
}

fn selection_of((row, column): (usize, usize)) -> DataTableSelection {
    let mut selection = DataTableSelection::default();
    selection.row = Some(row as u32);
    selection.column = Some(column as u32);
    selection
}

fn work_columns(table: &work::Table) -> Vec<DataTableColumn> {
    table
        .columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let mut result = DataTableColumn::default();
            result.key = format!("c{index}").into();
            result.title = Some(column.display_name.clone().into());
            result.width = Some(px(if index == 0 { 180.0 } else { 140.0 }));
            result.fixed = index == 0;
            result
        })
        .collect()
}

fn work_data(table: &work::Table) -> DataTableRows {
    let mut rows = DataTableRows::new(table.rows.len());
    for (index, _) in table.columns.iter().enumerate() {
        let key = format!("c{index}");
        rows = rows.texts(
            &key,
            table.rows.iter().map(|row| {
                row.cells
                    .get(index)
                    .map_or_else(String::new, |cell| cell.text.clone())
            }),
        );
    }
    rows
}

// ---------------------------------------------------------------------------
// Screen B: 100,000 x 50 synthetic cells, computed on demand.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
struct GridWindow {
    start: u32,
    end: u32,
}

#[derive(Clone, Copy)]
struct GridState {
    window: Signal<GridWindow>,
    edits: Signal<HashMap<(u32, u32), String>>,
    editing: Signal<Option<(u32, u32)>>,
    editor: Signal<String>,
    status: Signal<String>,
    selection: Signal<Option<(u32, u32)>>,
    marker: ElementRef,
    table: ComponentRef<DataTable>,
}

impl GridState {
    fn new() -> Self {
        Self {
            window: signal(GridWindow { start: 0, end: 40 }),
            edits: signal(HashMap::new()),
            editing: signal(None),
            editor: signal(String::new()),
            status: signal(String::from("grid: 100000 x 50, cells computed on demand")),
            selection: signal(None),
            marker: element_ref(),
            table: component_ref::<DataTable>(),
        }
    }

    fn begin_edit(&self, row: u32, column: u32) {
        self.editing.set(Some((row, column)));
        self.editor.set(grid_text(row, column, &self.edits.get()));
        self.status.set(format!("editing R{row}C{column}"));
    }

    fn commit(&self) {
        let Some((row, column)) = self.editing.get() else {
            self.status.set(String::from("no cell is being edited"));
            return;
        };
        let text = self.editor.get();
        self.edits
            .update(|edits| {
                edits.insert((row, column), text.clone());
            });
        self.editing.set(None);
        self.status.set(format!("edited R{row}C{column} = {text}"));
    }

    /// Scroll to the last cell and start editing it: the deep end of both
    /// axes in one press.
    fn jump_last(&self) {
        let row = GRID_ROWS - 1;
        let column = GRID_COLUMNS - 1;
        self.window.set(GridWindow {
            start: row.saturating_sub(12),
            end: GRID_ROWS,
        });
        let mut selection = DataTableSelection::default();
        selection.row = Some(row);
        selection.column = Some(column);
        self.selection.set(Some((row, column)));
        self.begin_edit(row, column);
        if let Some(api) = self.table.api::<DataTableApi>() {
            api.select(Some(selection));
            api.scroll_to_row(row, Some(false));
        }
        self.status.set(format!("jumped to R{row}C{column}"));
    }

    fn on_visible_rows(&self, range: &DataTableRange) {
        self.window.set(GridWindow {
            start: range.start.saturating_sub(2),
            end: (range.end + 2).min(GRID_ROWS),
        });
    }

    fn on_selection(&self, selection: &Option<DataTableSelection>) {
        let Some(selection) = *selection else { return };
        let (Some(row), Some(column)) = (selection.row, selection.column) else {
            return;
        };
        self.selection.set(Some((row, column)));
        self.editor.set(grid_text(row, column, &self.edits.get()));
    }
}

fn grid_screen(state: GridState) -> impl IntoElement {
    let rows = move || grid_rows(state.window.get(), &state.edits.get());
    view()
        .flex_1()
        .gap_2()
        .p_4()
        .child(
            view()
                .flex_row()
                .gap_2()
                .items_center()
                .child(button("jump-last", "Jump to R99999C49", move || {
                    state.jump_last();
                }))
                .child(
                    input()
                        .test_id("grid-editor")
                        .w(px(200.0))
                        .value(move || state.editor.get())
                        .on_change(move |text: &str| state.editor.set(text.to_owned())),
                )
                .child(button("grid-commit", "Commit grid cell", move || {
                    state.commit();
                }))
                .child(
                    view()
                        .test_id("grid-status")
                        .child(text(move || state.status.get()).text_size(px(13.0))),
                ),
        )
        .child(
            data_table()
                .test_id("grid-table")
                .component_ref(&state.table)
                .flex_1()
                .columns(grid_columns())
                .row_count(GRID_ROWS)
                .data_start(move || state.window.get().start)
                .data(rows)
                .row_height(px(24.0))
                .cell_selectable(true)
                .row_header(true)
                .loop_selection(false)
                .render_cell(move |cell: &DataTableCell| {
                    let (row, column) = (cell.row, cell.column);
                    let is_editing = move || state.editing.get() == Some((row, column));
                    view()
                        .flex_1()
                        .child(text(cell.text.clone()))
                        .child(when(is_editing, move || {
                            view()
                                .w(px(1.0))
                                .h(px(1.0))
                                .element_ref(&state.marker)
                        }))
                })
                .on_visible_rows_change(move |range: &DataTableRange| {
                    state.on_visible_rows(range);
                })
                .on_selection_change(move |selection: &Option<DataTableSelection>| {
                    state.on_selection(selection);
                })
                .on_double_press(move |target: &DataTableTarget| {
                    if let (Some(row), Some(column)) = (target.row, target.column) {
                        state.begin_edit(row, column);
                    }
                }),
        )
        .child(
            when(move || state.editing.get().is_some(), move || {
                overlay()
                    .anchor(&state.marker)
                    .placement(Placement::Bottom)
                    .offset(px(4.0))
                    .child(
                        view()
                            .flex_row()
                            .gap_2()
                            .items_center()
                            .p_2()
                            .bg(move || theme().popover)
                            .border_1()
                            .border_color(move || theme().border)
                            .rounded(px(8.0))
                            .child(
                                input()
                                    .test_id("cell-editor")
                                    .w(px(200.0))
                                    .auto_focus(true)
                                    .value(move || state.editor.get())
                                    .on_change(move |text: &str| state.editor.set(text.to_owned()))
                                    .on_submit(move |_text: &str| state.commit()),
                            )
                            .child(button("overlay-commit", "Commit", move || state.commit())),
                    )
            }),
        )
}

fn grid_text(row: u32, column: u32, edits: &HashMap<(u32, u32), String>) -> String {
    edits
        .get(&(row, column))
        .cloned()
        .unwrap_or_else(|| format!("R{row}C{column}"))
}

fn grid_rows(window: GridWindow, edits: &HashMap<(u32, u32), String>) -> DataTableRows {
    let len = window.end.saturating_sub(window.start) as usize;
    let mut rows = DataTableRows::new(len);
    for column in 0..GRID_COLUMNS {
        let key = format!("c{column}");
        rows = rows.texts(
            &key,
            (window.start..window.end).map(|row| grid_text(row, column, edits)),
        );
    }
    rows
}

fn grid_columns() -> Vec<DataTableColumn> {
    (0..GRID_COLUMNS)
        .map(|column| {
            let mut result = DataTableColumn::default();
            result.key = format!("c{column}").into();
            result.title = Some(format!("C{column}").into());
            result.width = Some(px(90.0));
            result.fixed = column == 0;
            result
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Files: the native targets have a filesystem, the web target does not.
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
fn write_file(path: &str, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
fn write_file(_path: &str, _bytes: &[u8]) -> Result<(), String> {
    Err(String::from("the web target has no file access"))
}

#[cfg(not(target_arch = "wasm32"))]
fn read_file(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
fn read_file(_path: &str) -> Result<Vec<u8>, String> {
    Err(String::from("the web target has no file access"))
}
