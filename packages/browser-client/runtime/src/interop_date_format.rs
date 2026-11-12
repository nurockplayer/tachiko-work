use tachiko_workspace_engine::{
    CellAlignment, DateFormat, EntityId, FieldDefinition, FieldId, PresentationStore, SchemaId,
};

use crate::interop_adapter::{CellStyle, SourceValue};

pub(crate) fn to_excel(format: DateFormat) -> &'static str {
    match format {
        DateFormat::Iso => "yyyy-mm-dd",
        DateFormat::Mdy => "m/d/yyyy",
        DateFormat::Dmy => "d/m/yyyy",
        DateFormat::YmdSlash => "yyyy/m/d",
        DateFormat::YmdCjk => "yyyy\"年\"m\"月\"d\"日\"",
        DateFormat::MonthDayYear => "mmm d, yyyy",
        DateFormat::DayMonthYear => "d mmm yyyy",
    }
}

pub(crate) fn from_excel(pattern: &str) -> Option<DateFormat> {
    let pattern = if pattern.starts_with("[$-") {
        let end = pattern.find(']')?;
        if end == 3 {
            return None;
        }
        &pattern[end + 1..]
    } else {
        pattern
    };
    [
        DateFormat::Iso,
        DateFormat::Mdy,
        DateFormat::Dmy,
        DateFormat::YmdSlash,
        DateFormat::YmdCjk,
        DateFormat::MonthDayYear,
        DateFormat::DayMonthYear,
    ]
    .into_iter()
    .find(|format| pattern.eq_ignore_ascii_case(to_excel(*format)))
}

pub(crate) fn cell_style(
    store: &PresentationStore,
    schema: &SchemaId,
    field: &FieldDefinition,
    entity: &EntityId,
    value: &SourceValue,
    mut result: CellStyle,
) -> CellStyle {
    let column = store
        .tables
        .get(schema)
        .and_then(|table| table.column_styles.get(&field.id))
        .and_then(|id| store.styles.get(id));
    let cell = store
        .cells
        .get(&(entity.clone(), field.id.clone()))
        .and_then(|id| store.styles.get(id));
    // A cell's own source pattern (one the store can't represent) beats the
    // field default; store cell formats below beat both.
    if let (Some(format), SourceValue::Number { .. }, None) =
        (&field.number_format, value, &result.number_format)
    {
        result.number_format = Some(crate::interop_number_format::to_excel(format));
    }
    for style in [column, cell].into_iter().flatten() {
        if let Some(bold) = style.bold {
            result.bold = bold;
        }
        if let Some(fill) = &style.fill {
            result.fill = Some(fill[1..].to_owned());
        }
        // XLSX export represents any nonempty side list as a full box.
        if let Some(borders) = &style.borders {
            result.border = !borders.is_empty();
        }
        if let (Some(format), SourceValue::Number { .. } | SourceValue::Empty) =
            (&style.number_format, value)
        {
            result.number_format = Some(crate::interop_number_format::to_excel(format));
        }
        if let Some(alignment) = style.alignment {
            result.alignment = Some(
                match alignment {
                    CellAlignment::Left => "left",
                    CellAlignment::Center => "center",
                    CellAlignment::Right => "right",
                }
                .into(),
            );
        }
        if let Some(wrap) = style.wrap {
            result.wrap = wrap;
        }
    }
    let date_format = cell
        .and_then(|style| style.date_format)
        .or_else(|| column.and_then(|style| style.date_format))
        .or(field.date_format);
    if matches!(value, SourceValue::Date { .. })
        || (field.field_type == tachiko_workspace_engine::FieldType::Date
            && matches!(value, SourceValue::Empty))
    {
        if let Some(format) = date_format {
            result.number_format = Some(to_excel(format).into());
        } else if result.number_format.is_none() {
            result.number_format = Some(to_excel(DateFormat::Iso).into());
        }
    }
    result
}

pub(crate) fn column_width(
    store: &PresentationStore,
    schema: &SchemaId,
    field: &FieldId,
    incoming: Option<f64>,
) -> Option<f64> {
    store
        .tables
        .get(schema)
        .and_then(|table| table.column_widths.get(field))
        .map_or(incoming, |width| Some(f64::from(*width) / 256.0))
}

#[cfg(test)]
mod tests {
    use super::{DateFormat, from_excel, to_excel};
    use crate::interop_number_format::{NumberFormatKind, classify};

    #[test]
    fn formats_round_trip_and_are_classified_as_dates() {
        for format in [
            DateFormat::Iso,
            DateFormat::Mdy,
            DateFormat::Dmy,
            DateFormat::YmdSlash,
            DateFormat::YmdCjk,
            DateFormat::MonthDayYear,
            DateFormat::DayMonthYear,
        ] {
            assert_eq!(from_excel(to_excel(format)), Some(format));
            assert_eq!(classify(Some(to_excel(format))), Ok(NumberFormatKind::Date));
        }
        assert_eq!(from_excel("[$-409]m/d/yyyy"), Some(DateFormat::Mdy));
        assert_eq!(from_excel("mm/dd/yyyy"), None);
        assert_eq!(from_excel("yyyy-mm-dd hh:mm"), None);
    }
}
