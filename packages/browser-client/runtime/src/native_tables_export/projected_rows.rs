use std::collections::BTreeMap;

use tachiko_formula_engine::CellFailureCode;
use tachiko_workspace_engine::formula_operations::FormulaCalculationOutcome;
use tachiko_workspace_engine::{Document, EntityId, FieldRef, Value};

use crate::interop_adapter::xlsx_cells::CachedErrors;
use crate::interop_adapter::{CellStyle, SourceCell, SourceValue};
use crate::{CollectionSpec, ColumnSpec, DesignerError, DesignerRuntime};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ProjectionFormat {
    Csv,
    Workbook,
    Xlsx,
}

pub(super) fn project_rows(
    runtime: &DesignerRuntime,
    document: &Document,
    spec: &CollectionSpec,
    format: ProjectionFormat,
    has_header: bool,
) -> Result<(Vec<Vec<SourceCell>>, CachedErrors), DesignerError> {
    let computed = computed_formula_values(runtime, document, spec)?;
    let mut errors = CachedErrors::new();
    let mut rows = Vec::with_capacity(spec.entities.len());
    for (row, entity) in spec.entities.iter().enumerate() {
        let mut cells = Vec::with_capacity(spec.columns.len());
        for (col, column) in spec.columns.iter().enumerate() {
            let stored = document.entities[entity].fields.get(&column.id);
            let (value, error) = match stored {
                Some(Value::Formula(_)) => {
                    let target = FieldRef::new(entity.clone(), column.id.clone());
                    formula_value(&computed[&target], spec, &target, format)?
                }
                other => (crate::interop_document::export_scalar(other)?, None),
            };
            if let Some(error) = error {
                errors.insert((row + usize::from(has_header), col), error);
            }
            cells.push(styled_cell(runtime, spec, (entity, column), value, format));
        }
        rows.push(cells);
    }
    Ok((rows, errors))
}

/// Read the resident's calculation outcomes by stable field identity.
fn computed_formula_values(
    runtime: &DesignerRuntime,
    document: &Document,
    spec: &CollectionSpec,
) -> Result<BTreeMap<FieldRef, FormulaCalculationOutcome>, DesignerError> {
    let targets = spec
        .entities
        .iter()
        .flat_map(|entity| {
            spec.columns.iter().filter_map(|column| {
                match document.entities[entity].fields.get(&column.id) {
                    Some(Value::Formula(_)) => {
                        Some(FieldRef::new(entity.clone(), column.id.clone()))
                    }
                    _ => None,
                }
            })
        })
        .collect::<Vec<_>>();
    let mut values = BTreeMap::new();
    for chunk in targets.chunks(crate::MAX_FIELD_QUERY_TARGETS) {
        for field in runtime.session.query_fields(chunk)?.into_value() {
            values.insert(
                field.field,
                field
                    .calculated_value
                    .expect("a stored formula has a calculation outcome"),
            );
        }
    }
    Ok(values)
}

fn formula_value(
    outcome: &FormulaCalculationOutcome,
    spec: &CollectionSpec,
    target: &FieldRef,
    format: ProjectionFormat,
) -> Result<(SourceValue, Option<CellFailureCode>), DesignerError> {
    let value = match outcome {
        FormulaCalculationOutcome::Value(number) => SourceValue::Number {
            value: number.get(),
        },
        FormulaCalculationOutcome::Date(date) => SourceValue::Date {
            value: date.to_string(),
        },
        FormulaCalculationOutcome::Text(text) => SourceValue::Text {
            value: text.clone(),
        },
        FormulaCalculationOutcome::Boolean(value) => SourceValue::Boolean { value: *value },
        FormulaCalculationOutcome::CellFailure(failure) if format == ProjectionFormat::Xlsx => {
            return Ok((SourceValue::Empty, Some(failure.code)));
        }
        other => return Err(unavailable(other, spec, target)),
    };
    Ok((value, None))
}

fn unavailable(
    outcome: &FormulaCalculationOutcome,
    spec: &CollectionSpec,
    target: &FieldRef,
) -> DesignerError {
    let detail = match crate::calculation_projection(outcome) {
        crate::CalculationProjection::Failure { code, message } => format!(" ({code}: {message})"),
        _ => String::new(),
    };
    super::refused(
        "native_tables_formula_value_unavailable",
        &format!(
            "collection {}/field {}/row {}",
            spec.summary.id, target.field, target.entity
        ),
        format!(
            "Formula cell has no current result{detail}; export refuses rather than writing a blank or a stale value."
        ),
    )
}

fn styled_cell(
    runtime: &DesignerRuntime,
    spec: &CollectionSpec,
    (entity, column): (&EntityId, &ColumnSpec),
    value: SourceValue,
    format: ProjectionFormat,
) -> SourceCell {
    let style = if format == ProjectionFormat::Csv {
        CellStyle::default()
    } else {
        let schema = &runtime.session.document().schemas[spec.summary.id.as_str()];
        crate::interop_date_format::cell_style(
            &runtime.presentation,
            &schema.id,
            &schema.fields[&column.id],
            entity,
            &value,
            CellStyle::default(),
        )
    };
    SourceCell {
        value,
        style,
        ..SourceCell::default()
    }
}
