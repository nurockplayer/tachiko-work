//! Storage-owned `.roproj/v3` `presentation.jsonl` member codec (ADR-0049).

use std::collections::BTreeMap;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use tachiko_semantic_core::{
    BorderSide, CellAlignment, CellStyle, ConditionalRule, DateFormat, EntityId, FieldId,
    FooterAggregate, PresentationStore, RuleId, SchemaId, StyleId, TablePresentation,
};

use crate::{
    FormatError,
    number_format::{NumberFormatDto, write_compact},
    roproj::v3::{
        ExpressionV3, compact_member_prefix, deserialize_roproj, ensure_increasing,
        invalid_representation, map_frontend_error, require_id, write_expression,
        write_json_string,
    },
    strict_json::inspect_roproj,
};

const PRESENTATION_MAX_JSON_NESTING: usize = 140;
const PRESENTATION_PATH: &str = "presentation.jsonl";

/// The stable style ID: the first 32 lowercase hex digits of the SHA-256 of the
/// `style` object's canonical compact JSON bytes.
///
/// # Panics
///
/// Panics only if JSON encoding a style object into a `String` fails, which the
/// serializer cannot do when writing strings into an owned `String`.
#[must_use]
pub fn style_id(style: &CellStyle) -> StyleId {
    let mut object = String::new();
    write_style_object(&mut object, style)
        .expect("writing a style object into a String cannot fail");
    let digest = Sha256::digest(object.as_bytes());
    let mut id = String::with_capacity(32);
    for byte in digest.iter().take(16) {
        id.push(char::from(HEX_DIGITS[usize::from(byte >> 4)]));
        id.push(char::from(HEX_DIGITS[usize::from(byte & 0x0f)]));
    }
    StyleId::from(id)
}

const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

/// Encode a presentation store as the canonical member bytes.
///
/// # Errors
///
/// Returns [`FormatError::InvalidRoProjectRepresentation`] when the store
/// cannot be written canonically, or [`FormatError::Json`] when JSON string
/// encoding fails.
pub fn encode_presentation(store: &PresentationStore) -> Result<Option<Vec<u8>>, FormatError> {
    if store.is_empty() {
        return Ok(None);
    }
    let mut output = String::new();
    for (id, style) in &store.styles {
        write_style_record(&mut output, id, style)?;
        output.push('\n');
    }
    for (schema, table) in &store.tables {
        write_table_record(&mut output, schema, table)?;
        output.push('\n');
    }
    for (id, rule) in &store.rules {
        write_rule_record(&mut output, id, rule)?;
        output.push('\n');
    }
    for ((entity, field), style) in &store.cells {
        write_cell_record(&mut output, entity, field, style)?;
        output.push('\n');
    }
    Ok(Some(output.into_bytes()))
}

/// Decode the canonical member bytes strictly.
///
/// # Errors
///
/// Returns a `.roproj` format or representation error for malformed UTF-8,
/// duplicate members, an unknown member or kind, out-of-order records or
/// members, a style ID that does not match its content, a contentless table,
/// a missing final LF, a blank line, an empty member, or any input the writer
/// would not produce byte for byte.
#[allow(clippy::too_many_lines)] // One strict record loop keeps the refusal order visible.
pub fn decode_presentation(bytes: &[u8]) -> Result<PresentationStore, FormatError> {
    let source =
        std::str::from_utf8(bytes).map_err(|source| FormatError::InvalidRoProjectUtf8 {
            path: PRESENTATION_PATH.to_owned(),
            source,
        })?;
    if source.is_empty() {
        return invalid_representation("presentation.jsonl must not be empty".to_owned());
    }
    let body =
        source
            .strip_suffix('\n')
            .ok_or_else(|| FormatError::InvalidRoProjectRepresentation {
                message: "presentation.jsonl must end with one LF".to_owned(),
            })?;
    let mut store = PresentationStore::default();
    let mut previous_kind: Option<u8> = None;
    let mut previous_style: Option<String> = None;
    let mut previous_table: Option<String> = None;
    let mut previous_rule: Option<String> = None;
    let mut previous_cell: Option<(String, String)> = None;
    for (record_index, record) in body.split('\n').enumerate() {
        if record.is_empty() {
            return invalid_representation(
                "presentation.jsonl contains a blank JSONL record".to_owned(),
            );
        }
        let path = format!("{PRESENTATION_PATH}:{}", record_index + 1);
        inspect_roproj(record, PRESENTATION_MAX_JSON_NESTING)
            .map_err(|error| map_frontend_error(&path, error))?;
        let probe: KindProbe = deserialize_roproj(&path, record)?;
        let kind = record_kind(&probe.kind)?;
        if let Some(previous) = previous_kind {
            if kind < previous {
                return invalid_representation(
                    "presentation records are not in style, table, rule, cell order".to_owned(),
                );
            }
        }
        previous_kind = Some(kind);
        match kind {
            KIND_STYLE => {
                let dto: StyleRecordV3 = deserialize_roproj(&path, record)?;
                require_id("style id", &dto.id)?;
                ensure_increasing("style", previous_style.as_deref(), &dto.id)?;
                let style = style_into_semantic(dto.style)?;
                let computed = style_id(&style);
                if computed.as_str() != dto.id {
                    return invalid_representation(format!(
                        "style '{}' does not match its content",
                        dto.id
                    ));
                }
                previous_style = Some(dto.id.clone());
                store.styles.insert(StyleId::from(dto.id), style);
            }
            KIND_TABLE => {
                let dto: TableRecordV3 = deserialize_roproj(&path, record)?;
                require_id("table schema id", &dto.schema)?;
                ensure_increasing("table schema", previous_table.as_deref(), &dto.schema)?;
                previous_table = Some(dto.schema.clone());
                let (schema, table) = table_into_semantic(dto)?;
                store.tables.insert(SchemaId::from(schema), table);
            }
            KIND_RULE => {
                let dto: RuleRecordV3 = deserialize_roproj(&path, record)?;
                require_id("rule id", &dto.id)?;
                require_id("rule schema id", &dto.schema)?;
                require_id("rule style id", &dto.style)?;
                ensure_increasing("rule", previous_rule.as_deref(), &dto.id)?;
                previous_rule = Some(dto.id.clone());
                dto.condition.validate_ids_and_limits()?;
                let fields = rule_fields(dto.fields)?;
                let rule = ConditionalRule {
                    schema: SchemaId::from(dto.schema),
                    rank: dto.rank,
                    fields,
                    condition: dto.condition.into_semantic()?,
                    style: StyleId::from(dto.style),
                };
                store.rules.insert(RuleId::from(dto.id), rule);
            }
            KIND_CELL => {
                let dto: CellRecordV3 = deserialize_roproj(&path, record)?;
                require_id("cell entity id", &dto.entity)?;
                require_id("cell field id", &dto.field)?;
                require_id("cell style id", &dto.style)?;
                let key = (dto.entity.as_str(), dto.field.as_str());
                if let Some(previous) = &previous_cell {
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
                previous_cell = Some((dto.entity.clone(), dto.field.clone()));
                store.cells.insert(
                    (EntityId::from(dto.entity), FieldId::from(dto.field)),
                    StyleId::from(dto.style),
                );
            }
            _ => {
                return invalid_representation(format!(
                    "unknown presentation record kind rank {kind}"
                ));
            }
        }
    }
    let canonical = encode_presentation(&store)?.ok_or_else(|| {
        FormatError::InvalidRoProjectRepresentation {
            message: "presentation.jsonl decoded to an empty store".to_owned(),
        }
    })?;
    if canonical != bytes {
        return invalid_representation("presentation.jsonl is not canonical".to_owned());
    }
    Ok(store)
}

const KIND_STYLE: u8 = 0;
const KIND_TABLE: u8 = 1;
const KIND_RULE: u8 = 2;
const KIND_CELL: u8 = 3;

fn record_kind(kind: &str) -> Result<u8, FormatError> {
    match kind {
        "style" => Ok(KIND_STYLE),
        "table" => Ok(KIND_TABLE),
        "rule" => Ok(KIND_RULE),
        "cell" => Ok(KIND_CELL),
        other => invalid_representation(format!("unknown presentation record kind '{other}'")),
    }
}

#[derive(Deserialize)]
struct KindProbe {
    kind: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StyleRecordV3 {
    #[serde(rename = "kind")]
    _kind: String,
    id: String,
    style: StyleV3,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StyleV3 {
    #[serde(default)]
    bold: Option<bool>,
    #[serde(default)]
    italic: Option<bool>,
    #[serde(default)]
    fill: Option<String>,
    #[serde(default)]
    text_color: Option<String>,
    #[serde(default)]
    alignment: Option<String>,
    #[serde(default)]
    wrap: Option<bool>,
    #[serde(default)]
    borders: Option<Vec<String>>,
    #[serde(default, deserialize_with = "crate::number_format::optional")]
    number_format: Option<NumberFormatDto>,
    #[serde(default)]
    date_format: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TableRecordV3 {
    #[serde(rename = "kind")]
    _kind: String,
    schema: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    footer: Option<BTreeMap<String, String>>,
    #[serde(default)]
    column_styles: Option<BTreeMap<String, String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleRecordV3 {
    #[serde(rename = "kind")]
    _kind: String,
    id: String,
    schema: String,
    rank: String,
    fields: Vec<String>,
    condition: ExpressionV3,
    style: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CellRecordV3 {
    #[serde(rename = "kind")]
    _kind: String,
    entity: String,
    field: String,
    style: String,
}

fn style_into_semantic(style: StyleV3) -> Result<CellStyle, FormatError> {
    let fill = style
        .fill
        .map(|fill| require_color("fill", fill))
        .transpose()?;
    let text_color = style
        .text_color
        .map(|color| require_color("text_color", color))
        .transpose()?;
    let alignment = style
        .alignment
        .map(|alignment| cell_alignment(&alignment))
        .transpose()?;
    let borders = style.borders.map(border_sides).transpose()?;
    let date_format = style
        .date_format
        .map(|format| date_format(&format))
        .transpose()?;
    Ok(CellStyle {
        bold: style.bold,
        italic: style.italic,
        fill,
        text_color,
        alignment,
        wrap: style.wrap,
        borders,
        number_format: style.number_format.map(Into::into),
        date_format,
    })
}

fn border_sides(sides: Vec<String>) -> Result<Vec<BorderSide>, FormatError> {
    let mut mapped = Vec::with_capacity(sides.len());
    let mut previous: Option<u8> = None;
    for side in sides {
        let (side, rank) = border_side(&side)?;
        if let Some(previous) = previous {
            if rank <= previous {
                return invalid_representation(
                    "borders must list top, right, bottom, left without duplicates".to_owned(),
                );
            }
        }
        previous = Some(rank);
        mapped.push(side);
    }
    Ok(mapped)
}

fn table_footer(
    footer: BTreeMap<String, String>,
) -> Result<BTreeMap<FieldId, FooterAggregate>, FormatError> {
    if footer.is_empty() {
        return invalid_representation("table footer must be nonempty when present".to_owned());
    }
    footer
        .into_iter()
        .map(|(field, aggregate)| Ok((FieldId::from(field), footer_aggregate(&aggregate)?)))
        .collect()
}

fn table_column_styles(
    column_styles: BTreeMap<String, String>,
) -> Result<BTreeMap<FieldId, StyleId>, FormatError> {
    if column_styles.is_empty() {
        return invalid_representation(
            "table column_styles must be nonempty when present".to_owned(),
        );
    }
    column_styles
        .into_iter()
        .map(|(field, style)| {
            require_id("table column style id", &style)?;
            Ok((FieldId::from(field), StyleId::from(style)))
        })
        .collect()
}

fn table_into_semantic(record: TableRecordV3) -> Result<(String, TablePresentation), FormatError> {
    if record.title.is_none() && record.footer.is_none() && record.column_styles.is_none() {
        return invalid_representation(format!("table '{}' has no content", record.schema));
    }
    let footer = record
        .footer
        .map(table_footer)
        .transpose()?
        .unwrap_or_default();
    let column_styles = record
        .column_styles
        .map(table_column_styles)
        .transpose()?
        .unwrap_or_default();
    Ok((
        record.schema,
        TablePresentation {
            title: record.title,
            footer,
            column_styles,
        },
    ))
}

fn rule_fields(fields: Vec<String>) -> Result<Vec<FieldId>, FormatError> {
    if fields.is_empty() {
        return invalid_representation("rule fields must be nonempty".to_owned());
    }
    let mut previous: Option<&str> = None;
    let mut mapped = Vec::with_capacity(fields.len());
    for field in &fields {
        require_id("rule field id", field)?;
        ensure_increasing("rule field", previous, field)?;
        previous = Some(field);
        mapped.push(FieldId::from(field.clone()));
    }
    Ok(mapped)
}

fn cell_alignment(value: &str) -> Result<CellAlignment, FormatError> {
    match value {
        "left" => Ok(CellAlignment::Left),
        "center" => Ok(CellAlignment::Center),
        "right" => Ok(CellAlignment::Right),
        other => invalid_representation(format!("unknown cell alignment '{other}'")),
    }
}

fn border_side(value: &str) -> Result<(BorderSide, u8), FormatError> {
    match value {
        "top" => Ok((BorderSide::Top, border_rank(BorderSide::Top))),
        "right" => Ok((BorderSide::Right, border_rank(BorderSide::Right))),
        "bottom" => Ok((BorderSide::Bottom, border_rank(BorderSide::Bottom))),
        "left" => Ok((BorderSide::Left, border_rank(BorderSide::Left))),
        other => invalid_representation(format!("unknown border side '{other}'")),
    }
}

fn date_format(value: &str) -> Result<DateFormat, FormatError> {
    match value {
        "iso" => Ok(DateFormat::Iso),
        "mdy" => Ok(DateFormat::Mdy),
        "dmy" => Ok(DateFormat::Dmy),
        "ymd_slash" => Ok(DateFormat::YmdSlash),
        "ymd_cjk" => Ok(DateFormat::YmdCjk),
        "month_day_year" => Ok(DateFormat::MonthDayYear),
        "day_month_year" => Ok(DateFormat::DayMonthYear),
        other => invalid_representation(format!("unknown date format '{other}'")),
    }
}

fn footer_aggregate(value: &str) -> Result<FooterAggregate, FormatError> {
    match value {
        "sum" => Ok(FooterAggregate::Sum),
        "count" => Ok(FooterAggregate::Count),
        "average" => Ok(FooterAggregate::Average),
        "min" => Ok(FooterAggregate::Minimum),
        "max" => Ok(FooterAggregate::Maximum),
        other => invalid_representation(format!("unknown footer aggregate '{other}'")),
    }
}

const fn alignment_wire(alignment: CellAlignment) -> &'static str {
    match alignment {
        CellAlignment::Left => "left",
        CellAlignment::Center => "center",
        CellAlignment::Right => "right",
    }
}

const fn border_rank(side: BorderSide) -> u8 {
    match side {
        BorderSide::Top => 0,
        BorderSide::Right => 1,
        BorderSide::Bottom => 2,
        BorderSide::Left => 3,
    }
}

const fn border_wire(side: BorderSide) -> &'static str {
    match side {
        BorderSide::Top => "top",
        BorderSide::Right => "right",
        BorderSide::Bottom => "bottom",
        BorderSide::Left => "left",
    }
}

const fn date_format_wire(format: DateFormat) -> &'static str {
    match format {
        DateFormat::Iso => "iso",
        DateFormat::Mdy => "mdy",
        DateFormat::Dmy => "dmy",
        DateFormat::YmdSlash => "ymd_slash",
        DateFormat::YmdCjk => "ymd_cjk",
        DateFormat::MonthDayYear => "month_day_year",
        DateFormat::DayMonthYear => "day_month_year",
    }
}

const fn footer_aggregate_wire(aggregate: FooterAggregate) -> &'static str {
    match aggregate {
        FooterAggregate::Sum => "sum",
        FooterAggregate::Count => "count",
        FooterAggregate::Average => "average",
        FooterAggregate::Minimum => "min",
        FooterAggregate::Maximum => "max",
    }
}

fn is_canonical_color(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 7
        && bytes[0] == b'#'
        && bytes[1..]
            .iter()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'A'..=b'F'))
}

fn require_color(member: &str, value: String) -> Result<String, FormatError> {
    if !is_canonical_color(&value) {
        return invalid_representation(format!(
            "'{member}' must be '#' and six uppercase hexadecimal digits, found '{value}'"
        ));
    }
    Ok(value)
}

fn member_head(output: &mut String, first: &mut bool, name: &str) -> Result<(), FormatError> {
    if *first {
        *first = false;
    } else {
        output.push(',');
    }
    compact_member_prefix(output, name)
}

fn write_style_record(
    output: &mut String,
    id: &StyleId,
    style: &CellStyle,
) -> Result<(), FormatError> {
    output.push('{');
    let mut first = true;
    member_head(output, &mut first, "kind")?;
    write_json_string(output, "style")?;
    member_head(output, &mut first, "id")?;
    write_json_string(output, id.as_str())?;
    member_head(output, &mut first, "style")?;
    write_style_object(output, style)?;
    output.push('}');
    Ok(())
}

fn write_style_object(output: &mut String, style: &CellStyle) -> Result<(), FormatError> {
    output.push('{');
    let mut first = true;
    if let Some(bold) = style.bold {
        member_head(output, &mut first, "bold")?;
        output.push_str(if bold { "true" } else { "false" });
    }
    if let Some(italic) = style.italic {
        member_head(output, &mut first, "italic")?;
        output.push_str(if italic { "true" } else { "false" });
    }
    if let Some(fill) = &style.fill {
        member_head(output, &mut first, "fill")?;
        require_color("fill", fill.clone())?;
        write_json_string(output, fill)?;
    }
    if let Some(text_color) = &style.text_color {
        member_head(output, &mut first, "text_color")?;
        require_color("text_color", text_color.clone())?;
        write_json_string(output, text_color)?;
    }
    if let Some(alignment) = style.alignment {
        member_head(output, &mut first, "alignment")?;
        write_json_string(output, alignment_wire(alignment))?;
    }
    if let Some(wrap) = style.wrap {
        member_head(output, &mut first, "wrap")?;
        output.push_str(if wrap { "true" } else { "false" });
    }
    if let Some(borders) = &style.borders {
        let mut borders = borders.clone();
        borders.sort_by_key(|side| border_rank(*side));
        if borders.windows(2).any(|pair| pair[0] == pair[1]) {
            return invalid_representation("borders must not list the same side twice".to_owned());
        }
        member_head(output, &mut first, "borders")?;
        output.push('[');
        for (index, side) in borders.iter().enumerate() {
            if index > 0 {
                output.push(',');
            }
            write_json_string(output, border_wire(*side))?;
        }
        output.push(']');
    }
    if let Some(number_format) = &style.number_format {
        member_head(output, &mut first, "number_format")?;
        write_compact(output, &NumberFormatDto::from(number_format))?;
    }
    if let Some(date_format) = style.date_format {
        member_head(output, &mut first, "date_format")?;
        write_json_string(output, date_format_wire(date_format))?;
    }
    output.push('}');
    Ok(())
}

fn write_table_record(
    output: &mut String,
    schema: &SchemaId,
    table: &TablePresentation,
) -> Result<(), FormatError> {
    if table.title.is_none() && table.footer.is_empty() && table.column_styles.is_empty() {
        return invalid_representation(format!("table '{}' has no content", schema.as_str()));
    }
    output.push('{');
    let mut first = true;
    member_head(output, &mut first, "kind")?;
    write_json_string(output, "table")?;
    member_head(output, &mut first, "schema")?;
    write_json_string(output, schema.as_str())?;
    if let Some(title) = &table.title {
        member_head(output, &mut first, "title")?;
        write_json_string(output, title)?;
    }
    if !table.footer.is_empty() {
        member_head(output, &mut first, "footer")?;
        output.push('{');
        for (index, (field, aggregate)) in table.footer.iter().enumerate() {
            if index > 0 {
                output.push(',');
            }
            write_json_string(output, field.as_str())?;
            output.push(':');
            write_json_string(output, footer_aggregate_wire(*aggregate))?;
        }
        output.push('}');
    }
    if !table.column_styles.is_empty() {
        member_head(output, &mut first, "column_styles")?;
        output.push('{');
        for (index, (field, style)) in table.column_styles.iter().enumerate() {
            if index > 0 {
                output.push(',');
            }
            write_json_string(output, field.as_str())?;
            output.push(':');
            write_json_string(output, style.as_str())?;
        }
        output.push('}');
    }
    output.push('}');
    Ok(())
}

fn write_rule_record(
    output: &mut String,
    id: &RuleId,
    rule: &ConditionalRule,
) -> Result<(), FormatError> {
    if rule.fields.is_empty() {
        return invalid_representation(format!("rule '{}' paints no field", id.as_str()));
    }
    let mut fields = rule.fields.clone();
    fields.sort_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
    if fields.windows(2).any(|pair| pair[0] == pair[1]) {
        return invalid_representation(format!("rule '{}' lists a field twice", id.as_str()));
    }
    output.push('{');
    let mut first = true;
    member_head(output, &mut first, "kind")?;
    write_json_string(output, "rule")?;
    member_head(output, &mut first, "id")?;
    write_json_string(output, id.as_str())?;
    member_head(output, &mut first, "schema")?;
    write_json_string(output, rule.schema.as_str())?;
    member_head(output, &mut first, "rank")?;
    write_json_string(output, &rule.rank)?;
    member_head(output, &mut first, "fields")?;
    output.push('[');
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        write_json_string(output, field.as_str())?;
    }
    output.push(']');
    member_head(output, &mut first, "condition")?;
    write_expression(output, &ExpressionV3::from_semantic(&rule.condition))?;
    member_head(output, &mut first, "style")?;
    write_json_string(output, rule.style.as_str())?;
    output.push('}');
    Ok(())
}

fn write_cell_record(
    output: &mut String,
    entity: &EntityId,
    field: &FieldId,
    style: &StyleId,
) -> Result<(), FormatError> {
    output.push('{');
    let mut first = true;
    member_head(output, &mut first, "kind")?;
    write_json_string(output, "cell")?;
    member_head(output, &mut first, "entity")?;
    write_json_string(output, entity.as_str())?;
    member_head(output, &mut first, "field")?;
    write_json_string(output, field.as_str())?;
    member_head(output, &mut first, "style")?;
    write_json_string(output, style.as_str())?;
    output.push('}');
    Ok(())
}
