use std::collections::BTreeMap;

use tachiko_formula_engine::CellFailureCode;

use super::{
    OutputProfile, SourceCell, SourceValue, SourceWorkbook, escape, escape_xstring,
    future_ooxml_formula,
};

/// Cached XLSX errors by zero-based row and column, including the header.
pub(crate) type CachedErrors = BTreeMap<(usize, usize), CellFailureCode>;

pub(crate) struct XlsxProjection<'a> {
    pub(super) workbook: &'a SourceWorkbook,
    pub(super) errors: &'a [CachedErrors],
}

impl<'a> From<&'a SourceWorkbook> for XlsxProjection<'a> {
    fn from(workbook: &'a SourceWorkbook) -> Self {
        Self {
            workbook,
            errors: &[],
        }
    }
}

impl<'a> From<(&'a SourceWorkbook, &'a [CachedErrors])> for XlsxProjection<'a> {
    fn from((workbook, errors): (&'a SourceWorkbook, &'a [CachedErrors])) -> Self {
        Self { workbook, errors }
    }
}

pub(super) fn content(
    cell: &SourceCell,
    profile: OutputProfile,
    header: Option<usize>,
    error: Option<CellFailureCode>,
) -> (&'static str, String) {
    let formula = cell
        .formula
        .as_ref()
        .map(|text| {
            format!(
                "<f>{}</f>",
                escape(&future_ooxml_formula(text.trim_start_matches('='), true))
            )
        })
        .unwrap_or_default();
    if let Some(column) = header {
        return ("s", format!("<v>{column}</v>"));
    }
    if let Some(error) = error {
        return ("e", format!("{formula}<v>{}</v>", error.display()));
    }
    scalar(cell, profile, formula)
}

fn scalar(cell: &SourceCell, profile: OutputProfile, formula: String) -> (&'static str, String) {
    match &cell.value {
        SourceValue::Empty => ("n", formula),
        SourceValue::Text { value } if cell.formula.is_some() => {
            ("str", format!("{formula}<v>{}</v>", escape_xstring(value)))
        }
        SourceValue::Text { value } => (
            "inlineStr",
            format!(
                "{formula}<is><t xml:space=\"preserve\">{}</t></is>",
                escape_xstring(value)
            ),
        ),
        SourceValue::Number { value }
            if profile == OutputProfile::NativeTables && *value == 0.0 =>
        {
            ("n", format!("{formula}<v>0</v>"))
        }
        SourceValue::Number { value } => ("n", format!("{formula}<v>{value}</v>")),
        SourceValue::Boolean { value } => ("b", format!("{formula}<v>{}</v>", u8::from(*value))),
        SourceValue::Date { value } => ("d", format!("{formula}<v>{}</v>", escape(value))),
    }
}
