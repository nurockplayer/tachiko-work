use tachiko_semantic_core::{
    ConditionalRule, EntityId, FieldId, PresentationStore, RuleId, SchemaId, StyleId,
};

use super::{
    CellRecordV3, RuleRecordV3, StyleRecordV3, TableRecordV3, rule_fields, style_id,
    style_into_semantic, table_into_semantic,
};
use crate::{
    FormatError,
    roproj::v3::{deserialize_roproj, ensure_increasing, invalid_representation, require_id},
};

pub(super) fn decode_style(
    path: &str,
    record: &str,
    previous_style: &mut Option<String>,
    store: &mut PresentationStore,
) -> Result<(), FormatError> {
    let dto: StyleRecordV3 = deserialize_roproj(path, record)?;
    require_id("style id", &dto.id)?;
    ensure_increasing("style", previous_style.as_deref(), &dto.id)?;
    let style = style_into_semantic(dto.style)?;
    let computed = style_id(&style)?;
    if computed.as_str() != dto.id {
        return invalid_representation(format!("style '{}' does not match its content", dto.id));
    }
    *previous_style = Some(dto.id.clone());
    store.styles.insert(StyleId::from(dto.id), style);
    Ok(())
}

pub(super) fn decode_table(
    path: &str,
    record: &str,
    previous_table: &mut Option<String>,
    store: &mut PresentationStore,
) -> Result<(), FormatError> {
    let dto: TableRecordV3 = deserialize_roproj(path, record)?;
    require_id("table schema id", &dto.schema)?;
    ensure_increasing("table schema", previous_table.as_deref(), &dto.schema)?;
    *previous_table = Some(dto.schema.clone());
    let (schema, table) = table_into_semantic(dto)?;
    store.tables.insert(SchemaId::from(schema), table);
    Ok(())
}

pub(super) fn decode_rule(
    path: &str,
    record: &str,
    previous_rule: &mut Option<String>,
    store: &mut PresentationStore,
) -> Result<(), FormatError> {
    let dto: RuleRecordV3 = deserialize_roproj(path, record)?;
    require_id("rule id", &dto.id)?;
    require_id("rule schema id", &dto.schema)?;
    require_id("rule style id", &dto.style)?;
    ensure_increasing("rule", previous_rule.as_deref(), &dto.id)?;
    *previous_rule = Some(dto.id.clone());
    dto.condition.validate_ids_and_limits()?;
    let fields = rule_fields(&dto.fields)?;
    let rule = ConditionalRule {
        schema: SchemaId::from(dto.schema),
        rank: dto.rank,
        fields,
        condition: dto.condition.into_semantic()?,
        style: StyleId::from(dto.style),
    };
    store.rules.insert(RuleId::from(dto.id), rule);
    Ok(())
}

pub(super) fn decode_cell(
    path: &str,
    record: &str,
    previous_cell: &mut Option<(String, String)>,
    store: &mut PresentationStore,
) -> Result<(), FormatError> {
    let dto: CellRecordV3 = deserialize_roproj(path, record)?;
    require_id("cell entity id", &dto.entity)?;
    require_id("cell field id", &dto.field)?;
    require_id("cell style id", &dto.style)?;
    let key = (dto.entity.as_str(), dto.field.as_str());
    if let Some(previous) = previous_cell.as_ref() {
        let previous_key = (previous.0.as_str(), previous.1.as_str());
        match previous_key.cmp(&key) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => {
                return invalid_representation(format!(
                    "duplicate cell record ({}, {})",
                    dto.entity, dto.field
                ));
            }
            std::cmp::Ordering::Greater => {
                return invalid_representation(format!(
                    "cell records are not in unsigned UTF-8 order: ({}, {}) follows ({}, {})",
                    dto.entity, dto.field, previous.0, previous.1
                ));
            }
        }
    }
    *previous_cell = Some((dto.entity.clone(), dto.field.clone()));
    store.cells.insert(
        (EntityId::from(dto.entity), FieldId::from(dto.field)),
        StyleId::from(dto.style),
    );
    Ok(())
}
