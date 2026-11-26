use super::{
    A1Geometry, DesignerError, Document, FormulaReferences, lookup_call, lookup_key,
    translate_a1_inner,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum A1ColumnMode {
    Scalar,
    Numeric,
    LookupResult,
}

impl A1ColumnMode {
    pub(super) fn conditional(numeric: bool) -> Self {
        if numeric { Self::Numeric } else { Self::Scalar }
    }

    pub(crate) fn needs_number(self) -> bool {
        self != Self::Scalar
    }
}

pub(super) fn translate_lookup_call(
    (name, argument): (&str, &str),
    current_sheet: usize,
    geometry: &mut impl A1Geometry,
    document: &Document,
    references: &mut FormulaReferences,
) -> Result<String, DesignerError> {
    let call = lookup_call(name, argument).ok_or_else(|| geometry.unknown_function())?;
    geometry.begin_conditional();
    let keys = geometry.column(
        &call.keys,
        current_sheet,
        A1ColumnMode::Scalar,
        document,
        references,
    )?;
    let results = geometry.column(
        &call.results,
        current_sheet,
        A1ColumnMode::LookupResult,
        document,
        references,
    )?;
    if let Some(end) = &call.range_end {
        geometry.column(
            end,
            current_sheet,
            A1ColumnMode::Scalar,
            document,
            &mut FormulaReferences::default(),
        )?;
    }
    let (key, nested) = translate_a1_inner(
        lookup_key(call.key).0,
        geometry,
        current_sheet,
        document,
        false,
    )?;
    references.extend(nested);
    let mut arguments = vec![key, keys, results];
    if let Some(fallback) = call.if_missing {
        let (fallback, nested) =
            translate_a1_inner(fallback, geometry, current_sheet, document, false)?;
        references.extend(nested);
        arguments.push(fallback);
    }
    Ok(format!("LOOKUP({})", arguments.join(",")))
}
