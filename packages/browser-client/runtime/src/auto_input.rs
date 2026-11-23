//! Classifier-backed Auto authoring; values and implied display publish together.
use std::collections::BTreeSet;

use tachiko_workspace_engine::{
    ClassifiedDateDisplay, ClassifiedInput, DateFormat, DateOrder, StyleId, classify_input,
};

use crate::{
    AutoDateOrder, CellStyle, ColumnSpec, DesignerError, DesignerRuntime, Document, EntityId,
    FieldId, FieldRef, FieldType, HistoryAction, HistoryEntry, NumberFormat, PresentationStore,
    PublicationProjection, ScalarEditInput, SemanticCommand, SessionProfile, prune_presentation,
    save_size, table_error, validate_presentation,
};

fn classified(input: &str, order: Option<AutoDateOrder>) -> ClassifiedInput {
    classify_input(
        input,
        order.map(|order| match order {
            AutoDateOrder::MonthFirst => DateOrder::MonthFirst,
            AutoDateOrder::DayFirst => DateOrder::DayFirst,
        }),
    )
}

pub(super) fn value(input: &ScalarEditInput) -> Result<Option<crate::Value>, DesignerError> {
    use crate::Value;
    Ok(match input {
        ScalarEditInput::Auto { input, date_order } => match classified(input, *date_order) {
            ClassifiedInput::Blank => None,
            ClassifiedInput::FormulaSource(_) => {
                return Err(table_error("Auto formulas are unsupported"));
            }
            ClassifiedInput::Number { value, .. } => Some(Value::Number(value)),
            ClassifiedInput::Text(value) => Some(Value::Text(value)),
            ClassifiedInput::Boolean(value) => Some(Value::Boolean(value)),
            ClassifiedInput::Date { value, .. } => Some(Value::Date(value)),
        },
        // Explicit tagged input keeps empty Text distinct from a clear.
        ScalarEditInput::Text { value } => Some(Value::Text(value.clone())),
        ScalarEditInput::Boolean { value } => Some(Value::Boolean(*value)),
        ScalarEditInput::Number { .. } => {
            Some(crate::parse_column_initializer(&FieldType::Number, input)?)
        }
        ScalarEditInput::Date { .. } => {
            Some(crate::parse_column_initializer(&FieldType::Date, input)?)
        }
        ScalarEditInput::Clear => None,
    })
}

#[derive(Clone, Debug)]
pub(super) struct CellDisplay {
    field: FieldRef,
    style: Option<CellStyle>,
}

pub(super) fn collect_display(
    field_type: &FieldType,
    field: &FieldRef,
    input: &ScalarEditInput,
    displays: &mut Vec<CellDisplay>,
) {
    if *field_type != FieldType::Auto {
        return;
    }
    let ScalarEditInput::Auto { input, date_order } = input else {
        return;
    };
    let style = match classified(input, *date_order) {
        ClassifiedInput::Number {
            display: Some(format),
            ..
        } => CellStyle {
            number_format: Some(format),
            ..CellStyle::default()
        },
        ClassifiedInput::Date {
            display: Some(display),
            ..
        } => CellStyle {
            date_format: Some(match display {
                ClassifiedDateDisplay::YmdSlash => DateFormat::YmdSlash,
                ClassifiedDateDisplay::YmdCjk => DateFormat::YmdCjk,
                ClassifiedDateDisplay::MonthFirst => DateFormat::Mdy,
                ClassifiedDateDisplay::DayFirst => DateFormat::Dmy,
            }),
            ..CellStyle::default()
        },
        _ => return,
    };
    displays.push(CellDisplay {
        field: field.clone(),
        style: Some(style),
    });
}

impl CellDisplay {
    pub(super) fn approx_bytes(&self) -> usize {
        self.field.entity.as_str().len() + self.field.field.as_str().len() + 256
    }
}

/// One cell's stored display record, if it has one.
pub(super) fn stored_style<'a>(
    store: &'a PresentationStore,
    entity: &str,
    field: &FieldId,
) -> Option<&'a CellStyle> {
    store
        .cells
        .get(&(EntityId::from(entity), field.clone()))
        .and_then(|id| store.styles.get(id))
}

/// The Number and Date formats a cell shows: its display record's, layered
/// attribute by attribute over the column's (ADR-0049 §2-§3).
pub(super) fn layered_formats<'a>(
    style: Option<&'a CellStyle>,
    number_format: Option<&'a NumberFormat>,
    date_format: Option<&'a DateFormat>,
) -> (Option<&'a NumberFormat>, Option<&'a DateFormat>) {
    (
        style
            .and_then(|style| style.number_format.as_ref())
            .or(number_format),
        style
            .and_then(|style| style.date_format.as_ref())
            .or(date_format),
    )
}

/// The formats one cell of `column` shows. An Auto cell's implied display
/// overrides its column's; a declared column shows only its own formats.
pub(super) fn cell_formats<'a>(
    store: &'a PresentationStore,
    column: &'a ColumnSpec,
    entity: &str,
) -> (Option<&'a NumberFormat>, Option<&'a DateFormat>) {
    let style = if column.field_type == FieldType::Auto {
        stored_style(store, entity, &column.id)
    } else {
        None
    };
    layered_formats(
        style,
        column.number_format.as_ref(),
        column.date_format.as_ref(),
    )
}

fn cell_style(store: &PresentationStore, field: &FieldRef) -> Option<CellStyle> {
    store
        .cells
        .get(&(field.entity.clone(), field.field.clone()))
        .and_then(|id| store.styles.get(id))
        .cloned()
}

fn with_displays(
    store: &PresentationStore,
    displays: &[CellDisplay],
) -> Result<PresentationStore, DesignerError> {
    let mut result = store.clone();
    for display in displays {
        let key = (display.field.entity.clone(), display.field.field.clone());
        result.cells.remove(&key);
        if let Some(style) = &display.style {
            let id = tachiko_storage::style_id(style)?;
            result.styles.insert(id.clone(), style.clone());
            result.cells.insert(key, id);
        }
    }
    let used: std::collections::BTreeSet<StyleId> = result
        .cells
        .values()
        .chain(
            result
                .tables
                .values()
                .flat_map(|table| table.column_styles.values()),
        )
        .chain(result.rules.values().map(|rule| &rule.style))
        .cloned()
        .collect();
    result.styles.retain(|id, _| used.contains(id));
    Ok(result)
}

pub(super) fn admit_presentation(
    store: Option<&PresentationStore>,
    document: &Document,
    profile: SessionProfile,
) -> Result<Option<u64>, DesignerError> {
    let Some(store) = store else {
        return Ok(None);
    };
    validate_presentation(&prune_presentation(store, document), document)
        .map_err(|error| table_error(&format!("invalid Auto input presentation: {error:?}")))?;
    Ok(Some(match profile {
        SessionProfile::WorkingSize => save_size::presentation_saved_bytes(store)?,
        SessionProfile::Ordinary | SessionProfile::TextCapacity => 0,
    }))
}

impl DesignerRuntime {
    pub(super) fn publish_input_commands(
        &mut self,
        expected: &str,
        commands: &[SemanticCommand],
        styles: &[CellDisplay],
    ) -> Result<PublicationProjection, DesignerError> {
        if styles.is_empty() {
            return self.publish_commands(expected, commands);
        }
        let presentation = with_displays(&self.presentation, styles)?;
        if commands.is_empty() {
            validate_presentation(
                &prune_presentation(&presentation, self.session.document()),
                self.session.document(),
            )
            .map_err(|error| table_error(&format!("invalid Auto input presentation: {error:?}")))?;
            let resulting_revision = self.publish_presentation(expected, presentation)?;
            return Ok(crate::bound_publication_targets(PublicationProjection {
                base_revision: expected.to_owned(),
                resulting_revision,
                schemas: Vec::new(),
                schema_fields: Vec::new(),
                entities: Vec::new(),
                fields: styles
                    .iter()
                    .map(|style| crate::field_target(&style.field))
                    .collect(),
                affected_calculations: Vec::new(),
                many: false,
                changed: Vec::new(),
                import_metadata: None,
                header_fallbacks: Vec::new(),
            }));
        }
        let mut publication = self.publish_commands_with_imported(
            expected,
            commands,
            None,
            None,
            Some(presentation),
        )?;
        if publication.many {
            return Ok(publication);
        }
        // A cell whose value stays while its implied display changes is in no
        // semantic invalidation; list it so its new display republishes.
        publication.fields = std::mem::take(&mut publication.fields)
            .into_iter()
            .chain(styles.iter().map(|style| crate::field_target(&style.field)))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok(match self.profile {
            SessionProfile::WorkingSize => crate::bound_publication_targets(publication),
            SessionProfile::Ordinary | SessionProfile::TextCapacity => publication,
        })
    }

    pub(super) fn record_input_edit(
        &mut self,
        expected: &str,
        forward: Vec<SemanticCommand>,
        inverse: Vec<SemanticCommand>,
        displays: Vec<CellDisplay>,
    ) -> Result<PublicationProjection, DesignerError> {
        let mut styles = Vec::new();
        let mut undo = Vec::new();
        for display in displays {
            let old = cell_style(&self.presentation, &display.field);
            let mut style = old.clone().unwrap_or_default();
            let implied = display.style.expect("classifier supplies a display");
            if implied.number_format.is_some() {
                style.number_format = implied.number_format;
            }
            if implied.date_format.is_some() {
                style.date_format = implied.date_format;
            }
            if Some(&style) != old.as_ref() {
                undo.push(CellDisplay {
                    field: display.field.clone(),
                    style: old,
                });
                styles.push(CellDisplay {
                    field: display.field,
                    style: Some(style),
                });
            }
        }
        if styles.is_empty() {
            return self.record_edit(expected, forward, inverse);
        }
        let publication = self.publish_input_commands(expected, &forward, &styles)?;
        self.record_history_entry(HistoryEntry::new(
            HistoryAction::InputCommands {
                commands: forward,
                styles,
            },
            HistoryAction::InputCommands {
                commands: inverse,
                styles: undo,
            },
        ));
        Ok(publication)
    }
}
