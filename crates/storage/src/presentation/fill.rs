use std::collections::BTreeSet;

use serde::Deserialize;
use tachiko_semantic_core::{CellFill, PresentationStore};

use super::{invalid_representation, require_color, write_json_string};
use crate::FormatError;

pub(super) fn validate_clear_fill_bindings(store: &PresentationStore) -> Result<(), FormatError> {
    let mut clear_styles = store
        .styles
        .iter()
        .filter(|(_, style)| style.fill == Some(CellFill::Clear))
        .map(|(id, _)| id)
        .collect::<BTreeSet<_>>();
    if clear_styles.is_empty() {
        return Ok(());
    }
    let mut non_cell_styles = store
        .tables
        .values()
        .flat_map(|table| table.column_styles.values())
        .chain(store.rules.values().map(|rule| &rule.style));
    if let Some(style) = non_cell_styles.find(|style| clear_styles.contains(*style)) {
        return invalid_representation(format!(
            "style '{style}' uses fill:false outside a cell binding"
        ));
    }
    for style in store.cells.values() {
        clear_styles.remove(style);
    }
    if let Some(style) = clear_styles.first() {
        return invalid_representation(format!(
            "style '{style}' uses fill:false without a cell binding"
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum FillV3 {
    Solid(String),
    Clear(bool),
}

pub(super) fn deserialize_fill<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<FillV3>, D::Error> {
    FillV3::deserialize(deserializer).map(Some)
}

pub(super) fn write_fill(output: &mut String, fill: &CellFill) -> Result<(), FormatError> {
    match fill {
        CellFill::Solid(color) => {
            require_color("fill", color.clone())?;
            write_json_string(output, color)
        }
        CellFill::Clear => {
            output.push_str("false");
            Ok(())
        }
    }
}
