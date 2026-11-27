use std::{collections::BTreeSet, fmt::Write};

use super::SummaryRow;
use serde::{Deserialize, Serialize};

use super::{
    FidelityCategory, FidelityFinding, InteropError, MAIN, MAX_COLUMNS, Result, SourceColumn,
    SourceSheet, Xml, check_import_limit, fail, finding,
};

pub(super) fn inventory_grid_attributes(
    node: &Xml,
    sheet: &str,
    ledger: &mut Vec<FidelityFinding>,
) {
    let (mapped, layout): (&[&str], &[&str]) = if node.name.as_str() == "row" {
        (
            &["r", "hidden", "s", "customFormat"],
            &[
                "spans",
                "ht",
                "customHeight",
                "outlineLevel",
                "collapsed",
                "thickTop",
                "thickBot",
                "ph",
            ],
        )
    } else {
        (
            &["min", "max", "width", "customWidth", "hidden", "style"],
            &["bestFit", "outlineLevel", "collapsed", "phonetic"],
        )
    };
    for key in node
        .attrs
        .iter()
        .map(|(key, _)| key)
        .filter(|key| !mapped.contains(&key.as_ref()))
    {
        ledger.push(finding(
            FidelityCategory::UnsupportedSafeDisabled,
            "unmapped_grid_attribute",
            &format!("{sheet}:{}@{key}", node.name),
            "Source row/column attribute is not represented or emitted; original source retains it",
            !layout.contains(&key.as_ref()),
        ));
    }
}

pub(super) fn visibility_finding(code: &str, location: &str) -> FidelityFinding {
    finding(
        FidelityCategory::UnsupportedSafeDisabled,
        code,
        location,
        "Hidden source content cannot be admitted or exported because this profile does not preserve visibility; original source bytes remain unchanged",
        true,
    )
}

pub(super) fn inventory_hidden(
    value: Option<&str>,
    code: &str,
    location: &str,
    ledger: &mut Vec<FidelityFinding>,
) -> Result<()> {
    if hidden_attribute(value)? {
        ledger.push(visibility_finding(code, location));
    }
    Ok(())
}

pub(super) fn hidden_attribute(value: Option<&str>) -> Result<bool> {
    match value {
        None | Some("0" | "false") => Ok(false),
        Some("1" | "true") => Ok(true),
        Some(_) => fail("Invalid hidden visibility boolean"),
    }
}

pub(super) fn data_row_visibility(
    hidden: BTreeSet<usize>,
    header: usize,
    rows: usize,
    sheet: &str,
    ledger: &mut Vec<FidelityFinding>,
) -> BTreeSet<usize> {
    hidden
        .into_iter()
        .filter_map(|row| {
            if row > header && row <= header + rows {
                Some(row - header - 1)
            } else {
                ledger.push(visibility_finding(
                    "hidden_row",
                    &format!("{sheet}:row {row}"),
                ));
                None
            }
        })
        .collect()
}

pub(super) fn column_grid(
    worksheet: &Xml,
    sheet: &str,
    columns: &mut [SourceColumn],
    working: bool,
    ledger: &mut Vec<FidelityFinding>,
) -> Result<(BTreeSet<usize>, bool)> {
    let mut hidden_columns = BTreeSet::new();
    let mut has_widths = false;
    let limit = if working {
        crate::capacity::WORKING_LIMITS.table_fields
    } else {
        MAX_COLUMNS
    };
    for group in worksheet.kids("cols") {
        if group
            .children
            .iter()
            .any(|node| node.name.as_str() != "col" || node.ns.as_str() != MAIN)
        {
            return fail("Unknown worksheet column construct");
        }
        for column in group.kids("col") {
            has_widths |=
                apply_column(column, sheet, (columns, &mut hidden_columns), limit, ledger)?;
        }
    }
    Ok((hidden_columns, has_widths))
}

fn apply_column(
    column: &Xml,
    sheet: &str,
    (columns, hidden_columns): (&mut [SourceColumn], &mut BTreeSet<usize>),
    limit: usize,
    ledger: &mut Vec<FidelityFinding>,
) -> Result<bool> {
    inventory_grid_attributes(column, sheet, ledger);
    if column.attr("style").is_some_and(|style| style != "0") {
        ledger.push(finding(FidelityCategory::UnsupportedSafeDisabled, "inherited_column_style", sheet, "Inherited column style is not applied by this profile; explicit cell styles are required", true));
    }
    let index = |key| {
        column
            .attr(key)
            .unwrap_or("1")
            .parse::<usize>()
            .map_err(|_| InteropError::Invalid("Invalid column width range".into()))
    };
    let (min, max) = (index("min")?, index("max")?);
    let width = column
        .attr("width")
        .map(str::parse::<f64>)
        .transpose()
        .map_err(|_| InteropError::Invalid("Invalid column width".into()))?;
    if min == 0 || max < min || max > 16_384 || width.is_some_and(|w| !valid_width(w)) {
        return fail("Invalid column width");
    }
    let hidden = hidden_attribute(column.attr("hidden"))?;
    check_import_limit(
        crate::CapacityLimit::TableFields,
        limit,
        max,
        Some(sheet),
        true,
    )?;
    inventory_outside_grid(sheet, (min, max), columns.len(), (hidden, width), ledger);
    for (index, column) in columns.iter_mut().enumerate().take(max).skip(min - 1) {
        column.width = width;
        if hidden {
            hidden_columns.insert(index);
        } else {
            hidden_columns.remove(&index);
        }
    }
    Ok(width.is_some())
}

fn inventory_outside_grid(
    sheet: &str,
    (min, max): (usize, usize),
    columns: usize,
    (hidden, width): (bool, Option<f64>),
    ledger: &mut Vec<FidelityFinding>,
) {
    if max > columns {
        let location = format!("{sheet}:columns {min}..{max}");
        if hidden {
            ledger.push(visibility_finding("hidden_column", &location));
        }
        if width.is_some() {
            ledger.push(finding(FidelityCategory::LossyOnExport, "column_width_outside_grid", &location, "Column width outside the represented cell grid remains source-only and is omitted on export", false));
        }
    }
}

fn valid_width(width: f64) -> bool {
    width.is_finite() && width > 0.0 && width <= 255.0
}

pub(super) fn hidden_xml(hidden: bool) -> &'static str {
    if hidden { " hidden=\"1\"" } else { "" }
}

pub(super) fn write_grid_columns(sheet: &SourceSheet, xml: &mut impl Write) -> Result<()> {
    if sheet.layout.hidden_columns.is_empty()
        && sheet.columns.iter().all(|column| column.width.is_none())
    {
        return Ok(());
    }
    xml.write_str("<cols>")
        .expect("worksheet XML sinks are infallible");
    for (index, column) in sheet.columns.iter().enumerate() {
        let hidden = sheet.layout.hidden_columns.contains(&index);
        if column.width.is_none() && !hidden {
            continue;
        }
        let position = index + 1;
        write!(xml, "<col min=\"{position}\" max=\"{position}\"")
            .expect("worksheet XML sinks are infallible");
        if let Some(width) = column.width {
            if !valid_width(width) {
                return fail("Invalid export column width");
            }
            write!(xml, " width=\"{width}\" customWidth=\"1\"")
                .expect("worksheet XML sinks are infallible");
        }
        write!(xml, "{}/>", hidden_xml(hidden)).expect("worksheet XML sinks are infallible");
    }
    xml.write_str("</cols>")
        .expect("worksheet XML sinks are infallible");
    Ok(())
}

pub(super) fn validate_grid_visibility(sheet: &SourceSheet) -> Result<()> {
    if sheet
        .layout
        .hidden_rows
        .last()
        .is_some_and(|&r| r >= sheet.rows.len())
        || sheet
            .layout
            .hidden_columns
            .last()
            .is_some_and(|&c| c >= sheet.columns.len())
    {
        return fail("Hidden row or column is outside the represented grid");
    }
    Ok(())
}

pub(super) fn write_capacity_header(sheet: &SourceSheet, output: &mut impl super::CapacityXmlSink) {
    write!(output, "<worksheet xmlns=\"{MAIN}\">").expect("capacity XML sinks are infallible");
    write_grid_columns(sheet, output).expect("admitted capacity columns have no custom widths");
    write!(output, "<sheetData><row r=\"1\">").expect("capacity XML sinks are infallible");
    for column in 0..sheet.columns.len() {
        write!(
            output,
            "<c r=\"{}1\" s=\"0\" t=\"s\"><v>{column}</v></c>",
            super::column_name(column)
        )
        .expect("capacity XML sinks are infallible");
    }
    write!(output, "</row>").expect("capacity XML sinks are infallible");
}

pub(super) fn capacity_grid_children(node: &Xml) -> bool {
    node.children.len() == 1 + usize::from(node.child("cols").is_some())
}

pub(super) fn capacity_grid_tag(
    tag: &str,
) -> (
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
) {
    match tag {
        "worksheet" => (MAIN, &[], &["cols", "sheetData"]),
        "sheetData" => (MAIN, &[], &["row"]),
        "row" => (MAIN, &["r", "hidden"], &["c"]),
        "cols" => (MAIN, &[], &["col"]),
        "col" => (MAIN, &["min", "max", "hidden"], &[]),
        _ => unreachable!("capacity grid tag"),
    }
}

/// Where the imported grid sat in the source worksheet.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLayout {
    /// Source rows above the header that were not imported (title or blank rows).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rows_above_header: u32,
    /// A trailing summary row recognised and left out of the data rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_row: Option<SummaryRow>,
    /// Zero-based positions in the represented data rows and columns.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub hidden_rows: BTreeSet<usize>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub hidden_columns: BTreeSet<usize>,
}
impl SourceLayout {
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}
#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "Serde passes the field by reference."
)]
fn is_zero(value: &u32) -> bool {
    *value == 0
}
