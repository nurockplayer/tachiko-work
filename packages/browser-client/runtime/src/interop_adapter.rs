//! Private bounded file adapter. This module never evaluates formulas or executes source content.
use quick_xml::{Reader, events::Event};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read, Write};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

pub const MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_EXPANDED_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_ZIP_ENTRIES: usize = 256;
pub const MAX_SHEETS: usize = 4;
pub const MAX_COLUMNS: usize = 16;
pub const MAX_DATA_ROWS: usize = 64;
pub const MAX_NATIVE_TRACKER_DATA_ROWS: usize = 128;
// Outbound-only: matches native admission geometry without widening imports.
pub const MAX_NATIVE_TABLES_SHEETS: usize = 32;
pub const MAX_NATIVE_TABLES_COLUMNS: usize = 32;
pub const MAX_NATIVE_TABLES_DATA_ROWS: usize = 128;
pub const MAX_NATIVE_TABLES_TOTAL_ROWS: usize = 1_024;
pub const MAX_NATIVE_TABLES_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_NATIVE_TABLES_EXPANDED_BYTES: usize = 24 * 1024 * 1024;
pub const MAX_NATIVE_TABLES_XLSX_TEXT_UNITS: usize = 32_767;
const MAX_NATIVE_TABLES_PRECISION_LOCATIONS: usize = 20;
pub const MAX_FORMULAS: usize = 32;
pub(crate) const MAX_TEXT_CAPACITY_ROWS: usize = 8_406;
pub(crate) const MAX_TEXT_CAPACITY_CELLS: usize = 25_218;
pub(crate) const MAX_TEXT_CAPACITY_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_TEXT_CAPACITY_HEADER_BYTES: usize = 39;

/// Native outbound capacities are explicit and do not widen the shared
/// import/export profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OutputProfile {
    Shared,
    NativeTracker,
    NativeTables,
    TextCapacity,
}

impl OutputProfile {
    const fn max_sheets(self) -> usize {
        if matches!(self, Self::NativeTables) {
            MAX_NATIVE_TABLES_SHEETS
        } else {
            MAX_SHEETS
        }
    }

    const fn max_columns(self) -> usize {
        if matches!(self, Self::NativeTables) {
            MAX_NATIVE_TABLES_COLUMNS
        } else {
            MAX_COLUMNS
        }
    }

    const fn max_output_bytes(self) -> usize {
        if matches!(self, Self::NativeTables) {
            MAX_NATIVE_TABLES_OUTPUT_BYTES
        } else {
            MAX_SOURCE_BYTES
        }
    }
    const fn max_data_rows(self) -> usize {
        match self {
            Self::Shared => MAX_DATA_ROWS,
            Self::NativeTracker => MAX_NATIVE_TRACKER_DATA_ROWS,
            Self::NativeTables => MAX_NATIVE_TABLES_DATA_ROWS,
            Self::TextCapacity => MAX_TEXT_CAPACITY_ROWS,
        }
    }
}

/// Bound the work while reaching a stable key for multi-step folds such as
/// capital sharp S: ẞ -> ß -> ss. Native allocation and preflight share this key.
pub(crate) fn native_tables_caseless_key(name: &str) -> String {
    let mut key = name.to_owned();
    for _ in 0..4 {
        let next = key.to_uppercase().to_lowercase();
        if next == key {
            break;
        }
        key = next;
    }
    key
}

/// A private resource exception selected from complete source content.
pub(crate) fn text_capacity_shape(workbook: &SourceWorkbook) -> bool {
    workbook.sheets.len() == 1
        && text_capacity_sheet_shape(&workbook.sheets[0])
        && !workbook.ledger.iter().any(|finding| {
            finding.code == "bounded_workbook"
                || matches!(
                    finding.category,
                    FidelityCategory::LossyOnExport | FidelityCategory::UnsupportedSafeDisabled
                )
        })
}

fn text_capacity_sheet_shape(sheet: &SourceSheet) -> bool {
    sheet.has_header
        && !sheet.columns.is_empty()
        && sheet.columns.len() <= MAX_COLUMNS
        && sheet.rows.len() <= MAX_TEXT_CAPACITY_ROWS
        && sheet.rows.len().saturating_mul(sheet.columns.len()) <= MAX_TEXT_CAPACITY_CELLS
        && sheet.columns.iter().all(|column| column.width.is_none())
        && sheet.rows.iter().all(|row| {
            row.len() == sheet.columns.len()
                && row.iter().all(|cell| {
                    cell.formula.is_none()
                        && matches!(
                            cell.value,
                            SourceValue::Empty
                                | SourceValue::Text { .. }
                                | SourceValue::Number { .. }
                        )
                        && cell.style == CellStyle::default()
                })
        })
}

pub(crate) fn capacity_header_limit(columns: usize) -> usize {
    MAX_TEXT_CAPACITY_HEADER_BYTES.max(13 * columns)
}

pub(crate) fn capacity_auxiliary_xml_reserve(columns: usize) -> usize {
    capacity_header_limit(columns)
        .saturating_mul(6)
        .saturating_add(31 * 6)
}

pub(crate) fn output_profile(workbook: &SourceWorkbook) -> OutputProfile {
    if workbook
        .sheets
        .iter()
        .any(|sheet| sheet.rows.len() > MAX_DATA_ROWS)
        && text_capacity_shape(workbook)
    {
        OutputProfile::TextCapacity
    } else {
        OutputProfile::Shared
    }
}

/// Exact RFC CSV width, including quote doubling and empty-field quotes.
pub(crate) fn csv_text_bytes(value: &str) -> usize {
    value.len()
        + if value.is_empty() || value.contains([',', '"', '\n', '\r']) {
            2 + value.bytes().filter(|byte| *byte == b'"').count()
        } else {
            0
        }
}

pub(crate) fn capacity_header_bytes(sheet: &SourceSheet) -> usize {
    sheet
        .columns
        .iter()
        .map(|column| csv_text_bytes(&column.name))
        .sum::<usize>()
        + sheet.columns.len()
        + 1
}

/// Prove both encodings before admitting this both-formats resource profile.
/// The CSV body uses an invariant header reservation, independent of host metadata.
pub(crate) fn validate_text_capacity(workbook: &SourceWorkbook) -> Result<()> {
    if !text_capacity_shape(workbook) {
        return fail(
            "Text capacity requires one headered, default-style Text/Number table within 8,406 rows, 16 columns, and 25,218 cells",
        );
    }
    let sheet = &workbook.sheets[0];
    if st_xstring(&sheet.name) || sheet.columns.iter().any(|column| st_xstring(&column.name)) {
        return fail("Text capacity refuses SpreadsheetML escape patterns");
    }
    let header_limit = capacity_header_limit(sheet.columns.len());
    if capacity_header_bytes(sheet) > header_limit {
        return fail("Text capacity CSV header exceeds its reserved encoded bytes");
    }
    let mut text_bytes = 0usize;
    let mut csv_bytes = header_limit;
    for row in &sheet.rows {
        csv_bytes += sheet.columns.len() + 1; // Delimiters and CRLF.
        for cell in row {
            let (value, is_text) = match &cell.value {
                SourceValue::Text { value } => (value.as_str(), true),
                SourceValue::Empty => ("", true),
                SourceValue::Number { value } if value.is_finite() => ("", false),
                SourceValue::Number { .. } => {
                    return fail("Text capacity refuses nonfinite Number");
                }
                SourceValue::Boolean { .. } | SourceValue::Date { .. } => {
                    return fail("Text capacity contains a value outside Text/Number");
                }
            };
            if is_text {
                if value.len() > 4096 {
                    return fail("Text capacity value exceeds 4096 UTF-8 bytes");
                }
                if st_xstring(value) {
                    return fail("Text capacity refuses SpreadsheetML escape patterns");
                }
                if csv_formula_text(value) || !xml_text_valid(value) {
                    return fail("Text capacity requires safe CSV Text and valid XML characters");
                }
                text_bytes += value.len();
                csv_bytes += csv_text_bytes(value);
            } else if let SourceValue::Number { value } = &cell.value {
                let canonical = value.to_string();
                if !xml_text_valid(&canonical) {
                    return fail("Text capacity Number is invalid XML text");
                }
                csv_bytes += csv_text_bytes(&canonical);
            }
        }
    }
    if text_bytes > MAX_TEXT_CAPACITY_BYTES {
        return fail("Text capacity aggregate Text exceeds 1048576 bytes");
    }
    if csv_bytes > MAX_SOURCE_BYTES {
        return fail("Text capacity encoded CSV exceeds 2 MiB");
    }
    let mut expanded = CapacityXmlCount(16 * 1024);
    capacity_worksheet(sheet, &mut expanded);
    if expanded.0 > MAX_EXPANDED_BYTES {
        return fail("Text capacity expanded XLSX exceeds 8 MiB");
    }
    // Worksheet nodes: row + (c,is,t) for populated Text cells. Empty and
    // Number cells use fewer nodes. Other parts are independent XML documents.
    if 2 + (sheet.rows.len() + 1) * (1 + 3 * sheet.columns.len()) > 100_000 {
        return fail("Text capacity XML exceeds 100000 nodes");
    }
    for column in &sheet.columns {
        if csv_formula_text(&column.name) || !xml_text_valid(&column.name) {
            return fail("Text capacity requires safe CSV headers and valid XML characters");
        }
    }
    // Prove the compressed source bound with the actual deterministic writer.
    // This runs at import/export, not on each scalar edit.
    Ok(())
}

// Shared native/WASM inspection eligibility; structural validity alone is insufficient.
pub(crate) fn validate_text_capacity_inspection(workbook: &SourceWorkbook) -> Result<()> {
    if !text_capacity_shape(workbook) {
        return fail("Source exceeds the bounded Text/Number capacity rectangle");
    }
    let sheet = &workbook.sheets[0];
    let legacy_small_text = sheet.columns.len() == 3
        && sheet
            .rows
            .iter()
            .flatten()
            .all(|cell| matches!(cell.value, SourceValue::Empty | SourceValue::Text { .. }));
    if sheet.rows.len() <= MAX_DATA_ROWS {
        if !legacy_small_text {
            return fail("Only large or legacy Text sources use the private capacity inspection");
        }
        // Small sources must qualify as original safe legacy Text, without
        // prospective Number conversion granting them an enlarged reply.
        validate_text_capacity(workbook)?;
    }
    validate_capacity_source_inspection(workbook)
}

fn validate_capacity_source_inspection(workbook: &SourceWorkbook) -> Result<()> {
    if !text_capacity_shape(workbook) {
        return fail("Source exceeds the bounded Text/Number capacity rectangle");
    }
    let mut prospective = workbook.clone();
    for column in 0..prospective.sheets[0].columns.len() {
        let needs_number = prospective.sheets[0].rows.iter().any(|row| {
            matches!(&row[column].value, SourceValue::Text { value } if value.len() > 4096 || st_xstring(value) || csv_formula_text(value) || !xml_text_valid(value))
        });
        if !needs_number {
            continue;
        }
        for row in &mut prospective.sheets[0].rows {
            if let SourceValue::Text { value } = &row[column].value {
                let Some(number) = value
                    .parse::<f64>()
                    .ok()
                    .and_then(|value| tachiko_workspace_engine::Number::new(value).ok())
                else {
                    return fail("Unsafe Text column is not entirely finite decimal Numbers");
                };
                row[column].value = SourceValue::Number {
                    value: number.get(),
                };
            }
        }
    }
    validate_text_capacity(&prospective)?;
    export_csv_for_profile(&prospective.sheets[0], OutputProfile::TextCapacity)?;
    export_xlsx_for_profile(&prospective, OutputProfile::TextCapacity)?;
    Ok(())
}

fn st_xstring(value: &str) -> bool {
    value.as_bytes().windows(7).any(|token| {
        token.starts_with(b"_x")
            && token[2..6].iter().all(u8::is_ascii_hexdigit)
            && token[6] == b'_'
    })
}

/// Removing one boundary can expose the other; repeat until both are gone.
/// Used for native worksheet allocation and CSV's internal worksheet label.
pub(crate) fn native_tables_trim_worksheet_name(mut name: &str) -> &str {
    loop {
        let trimmed = name.trim().trim_matches('\'');
        if trimmed.len() == name.len() {
            return trimmed;
        }
        name = trimmed;
    }
}

/// The native-only compatibility inventory. Allocation and writer preflight
/// use the same policy; CSV retains its existing lexical/name behavior.
pub(crate) enum NativeTablesCheck<'a> {
    Name {
        value: &'a str,
        worksheet: bool,
        used: &'a mut BTreeSet<String>,
        allocate: bool,
    },
    Cell(&'a SourceValue),
    CsvText(&'a str),
    Grid {
        sheets: usize,
        columns: usize,
        rows: usize,
        total_rows: usize,
    },
}

#[derive(Debug, Default)]
pub(crate) struct NativeTablesCompatibility {
    pub name: String,
    pub precision_loss: bool,
}

/// Return allocated names and precision disclosure decisions from one policy.
/// This is the single `NativeTables` Excel compatibility policy. Source names
/// and final candidates both pass its Text checks, before any bytes are emitted.
#[allow(clippy::too_many_lines)] // One auditable constraint inventory, matching the specification table.
pub(crate) fn native_tables_compatibility(
    check: NativeTablesCheck<'_>,
    location: &str,
) -> std::result::Result<NativeTablesCompatibility, FidelityFinding> {
    let refuse = |code: &str, message: &str| FidelityFinding {
        category: FidelityCategory::UnsupportedSafeDisabled,
        code: code.into(),
        location: location.into(),
        message: message.into(),
        blocking: true,
    };
    let text = |value: &str, cell_text: bool| {
        if !xml_text_valid(value) {
            Err(refuse(
                "native_tables_xlsx_xml_character",
                "XML-illegal characters cannot be retained in native XLSX; the source is refused rather than changed.",
            ))
        } else if st_xstring(value) {
            Err(refuse(
                "native_tables_xlsx_escape_sequence",
                "Literal SpreadsheetML _xHHHH_ escape sequences are refused because Excel decodes them and would change the source Text.",
            ))
        } else if cell_text && value.encode_utf16().count() > MAX_NATIVE_TABLES_XLSX_TEXT_UNITS {
            Err(refuse(
                "native_tables_xlsx_text_limit",
                "Text exceeds Excel's limit of 32,767 UTF-16 code units; native XLSX refuses rather than truncates it.",
            ))
        } else if cell_text && value.bytes().filter(|c| *c == b'\n').count() > 253 {
            Err(refuse(
                "native_tables_xlsx_line_feeds",
                "Text exceeds Excel's limit of 253 line feeds per cell; native XLSX refuses rather than changing it.",
            ))
        } else {
            Ok(())
        }
    };
    match check {
        NativeTablesCheck::Name {
            value,
            worksheet,
            used,
            allocate,
        } => {
            text(value, !worksheet)?;
            if allocate {
                let cleaned: String = value
                    .chars()
                    .map(|c| {
                        if worksheet && (c < ' ' || "[]:*?/\\".contains(c)) {
                            '_'
                        } else {
                            c
                        }
                    })
                    .collect();
                let cleaned = if worksheet {
                    native_tables_trim_worksheet_name(&cleaned)
                } else {
                    &cleaned
                };
                let base = if cleaned.trim().is_empty() {
                    if worksheet { "Table" } else { "Column" }
                } else {
                    cleaned
                };
                for serial in 1.. {
                    let suffix = if serial == 1 {
                        String::new()
                    } else {
                        format!(" ({serial})")
                    };
                    let prefix = if worksheet {
                        let mut units = 0;
                        let prefix = base
                            .chars()
                            .take_while(|c| {
                                units += c.len_utf16();
                                units <= 31 - suffix.encode_utf16().count()
                            })
                            .collect::<String>();
                        native_tables_trim_worksheet_name(&prefix).to_owned()
                    } else {
                        base.to_owned()
                    };
                    let candidate = format!("{prefix}{suffix}");
                    text(&candidate, !worksheet)?;
                    let key = native_tables_caseless_key(&candidate);
                    if worksheet && key == "history" {
                        continue;
                    }
                    if used.insert(key) {
                        return Ok(NativeTablesCompatibility {
                            name: candidate,
                            ..NativeTablesCompatibility::default()
                        });
                    }
                }
                unreachable!("bounded native allocation cannot exhaust suffixes");
            }
            if value.trim().is_empty()
                || (worksheet
                    && (value.encode_utf16().count() > 31
                        || value.contains(['[', ']', ':', '*', '?', '/', '\\'])
                        || value.starts_with('\'')
                        || value.ends_with('\'')
                        || value.chars().any(|c| c < ' ')
                        || native_tables_caseless_key(value) == "history"))
                || !used.insert(native_tables_caseless_key(value))
            {
                return Err(refuse(
                    "native_tables_xlsx_name",
                    "Invalid, reserved or caseless-duplicate worksheet/header name; allocate names through the native compatibility policy.",
                ));
            }
            return Ok(NativeTablesCompatibility {
                name: value.into(),
                ..NativeTablesCompatibility::default()
            });
        }
        NativeTablesCheck::CsvText(value) => {
            if !xml_text_valid(value) {
                return Err(refuse(
                    "native_tables_csv_xml_character",
                    "XML-illegal characters are refused in native CSV Text and headers rather than emitted or changed.",
                ));
            }
        }
        NativeTablesCheck::Cell(value) => match value {
            SourceValue::Text { value } => text(value, true)?,
            SourceValue::Number { value } => {
                if !value.is_finite() {
                    return Err(refuse(
                        "native_tables_xlsx_nonfinite_number",
                        "Excel Number must be finite.",
                    ));
                }
                if *value != 0.0 && value.abs() < f64::MIN_POSITIVE {
                    return Err(refuse(
                        "native_tables_xlsx_subnormal_number",
                        "Excel cannot retain non-zero subnormal Numbers; native XLSX refuses rather than changing the value to zero.",
                    ));
                }
                return Ok(NativeTablesCompatibility {
                    precision_loss: native_tables_significant_digits(*value) > 15,
                    ..NativeTablesCompatibility::default()
                });
            }
            SourceValue::Date { value } => {
                if tachiko_workspace_engine::Date::parse(value).is_err()
                    || !("1900-01-01"..="9999-12-31").contains(&value.as_str())
                {
                    return Err(refuse(
                        "native_tables_xlsx_date_range",
                        "Excel's 1900 date system supports Gregorian Dates from 1900-01-01 through 9999-12-31; out-of-range or invalid dates are refused.",
                    ));
                }
            }
            SourceValue::Boolean { .. } | SourceValue::Empty => {}
        },
        NativeTablesCheck::Grid {
            sheets,
            columns,
            rows,
            total_rows,
        } => {
            if !(1..=MAX_NATIVE_TABLES_SHEETS).contains(&sheets)
                || !(1..=MAX_NATIVE_TABLES_COLUMNS).contains(&columns)
                || rows > MAX_NATIVE_TABLES_DATA_ROWS
                || total_rows > MAX_NATIVE_TABLES_TOTAL_ROWS
            {
                return Err(refuse(
                    "native_tables_xlsx_grid",
                    "Native tables export requires 1..=32 sheets, 1..=32 columns, at most 128 data rows per sheet and 1,024 total data rows; these bounds are below Excel's grid limits.",
                ));
            }
        }
    }
    Ok(NativeTablesCompatibility::default())
}

pub(crate) fn validate_native_tables_xlsx(
    workbook: &SourceWorkbook,
) -> std::result::Result<(), FidelityFinding> {
    let mut names = BTreeSet::new();
    let total_rows = workbook.sheets.iter().map(|s| s.rows.len()).sum();
    // Check an empty workbook as well as every located sheet.
    if workbook.sheets.is_empty() {
        native_tables_compatibility(
            NativeTablesCheck::Grid {
                sheets: 0,
                columns: 0,
                rows: 0,
                total_rows,
            },
            "workbook",
        )?;
    }
    for (sheet_index, sheet) in workbook.sheets.iter().enumerate() {
        let location = format!("sheet {} {:?}", sheet_index + 1, sheet.name);
        native_tables_compatibility(
            NativeTablesCheck::Grid {
                sheets: workbook.sheets.len(),
                columns: sheet.columns.len(),
                rows: sheet.rows.len(),
                total_rows,
            },
            &location,
        )?;
        native_tables_compatibility(
            NativeTablesCheck::Name {
                value: &sheet.name,
                worksheet: true,
                used: &mut names,
                allocate: false,
            },
            &format!("{location}/sheet-name"),
        )?;
        let mut headers = BTreeSet::new();
        if sheet.has_header {
            for (column, header) in sheet.columns.iter().enumerate() {
                native_tables_compatibility(
                    NativeTablesCheck::Name {
                        value: &header.name,
                        worksheet: false,
                        used: &mut headers,
                        allocate: false,
                    },
                    &format!("{location}/header/column {}", column + 1),
                )?;
            }
        }
        for (row, cells) in sheet.rows.iter().enumerate() {
            for (column, cell) in cells.iter().enumerate() {
                native_tables_compatibility(
                    NativeTablesCheck::Cell(&cell.value),
                    &native_tables_cell_location(sheet, sheet_index, row, column),
                )?;
            }
        }
    }
    Ok(())
}

/// CSV applies only the shared XML-character refusal to the selected sheet;
/// XLSX-specific string lengths, escapes and numeric rules remain separate.
pub(crate) fn validate_native_tables_csv(
    sheet: &SourceSheet,
) -> std::result::Result<(), FidelityFinding> {
    if sheet.has_header {
        for (column, header) in sheet.columns.iter().enumerate() {
            native_tables_compatibility(
                NativeTablesCheck::CsvText(&header.name),
                &format!("sheet 1 {:?}/header/column {}", sheet.name, column + 1),
            )?;
        }
    }
    for (row, cells) in sheet.rows.iter().enumerate() {
        for (column, cell) in cells.iter().enumerate() {
            if let SourceValue::Text { value } = &cell.value {
                native_tables_compatibility(
                    NativeTablesCheck::CsvText(value),
                    &native_tables_cell_location(sheet, 0, row, column),
                )?;
            }
        }
    }
    Ok(())
}

pub(crate) fn native_tables_cell_location(
    sheet: &SourceSheet,
    index: usize,
    row: usize,
    column: usize,
) -> String {
    format!(
        "sheet {} {:?}/row {}/column {}",
        index + 1,
        sheet.name,
        row + 1 + usize::from(sheet.has_header),
        column + 1
    )
}

/// Display uses the shortest round-trip decimal. Ignore the sign, decimal
/// point, exponent and insignificant leading/trailing zeros when counting.
fn native_tables_significant_digits(value: f64) -> usize {
    let decimal = value.to_string();
    let digits: String = decimal
        .split(['e', 'E'])
        .next()
        .unwrap_or_default()
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    digits.trim_matches('0').len()
}

pub(crate) fn native_tables_xlsx_number_findings(
    workbook: &SourceWorkbook,
) -> Vec<FidelityFinding> {
    let mut findings = Vec::new();
    let mut count = 0usize;
    for (sheet_index, sheet) in workbook.sheets.iter().enumerate() {
        for (row, cells) in sheet.rows.iter().enumerate() {
            for (column, cell) in cells.iter().enumerate() {
                if let SourceValue::Number { value } = cell.value {
                    if native_tables_compatibility(
                        NativeTablesCheck::Cell(&cell.value),
                        &native_tables_cell_location(sheet, sheet_index, row, column),
                    )
                    .is_ok_and(|checked| checked.precision_loss)
                    {
                        count += 1;
                        if findings.len() < MAX_NATIVE_TABLES_PRECISION_LOCATIONS {
                            findings.push(FidelityFinding {
                                category: FidelityCategory::LossyOnExport,
                                code: "native_tables_xlsx_precision_loss".into(),
                                location: native_tables_cell_location(sheet, sheet_index, row, column),
                                message: format!("Number {value} needs more than 15 significant decimal digits; Excel may round it to 15 significant decimal digits. The exported value is not rounded; CSV keeps full precision."),
                                blocking: false,
                            });
                        }
                    }
                }
            }
        }
    }
    if count > 0 {
        let shown = findings.len();
        findings.push(FidelityFinding {
            category: FidelityCategory::LossyOnExport,
            code: "native_tables_xlsx_precision_count".into(),
            location: "workbook".into(),
            message: format!("{count} Number cells need more than 15 significant decimal digits; showing the first {shown} locations; {} additional locations omitted. Excel may round these values; CSV keeps full precision.", count - shown),
            blocking: false,
        });
    }
    findings
}

trait CapacityXmlSink: std::fmt::Write {
    fn text(&mut self, value: &str);
}
impl CapacityXmlSink for String {
    fn text(&mut self, value: &str) {
        if value.contains(['&', '<', '>', '"', '\'', '\r']) {
            self.push_str(&escape(value));
        } else {
            self.push_str(value);
        }
    }
}
struct CapacityXmlCount(usize);
impl std::fmt::Write for CapacityXmlCount {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.0 += value.len();
        Ok(())
    }
}
impl CapacityXmlSink for CapacityXmlCount {
    fn text(&mut self, value: &str) {
        self.0 += escaped_xml_bytes(value);
    }
}
// The writer and admission counter share every worksheet token. Other permitted
// XML members, including escaped headers/name, fit the existing 16 KiB reserve.
fn capacity_worksheet(sheet: &SourceSheet, output: &mut impl CapacityXmlSink) {
    write!(
        output,
        "<worksheet xmlns=\"{MAIN}\"><sheetData><row r=\"1\">"
    )
    .expect("capacity XML sinks are infallible");
    for (column, _) in sheet.columns.iter().enumerate() {
        write!(
            output,
            "<c r=\"{}1\" s=\"0\" t=\"s\"><v>{column}</v></c>",
            column_name(column)
        )
        .expect("capacity XML sinks are infallible");
    }
    write!(output, "</row>").expect("capacity XML sinks are infallible");
    for (index, row) in sheet.rows.iter().enumerate() {
        let number = index + 2;
        write!(output, "<row r=\"{number}\">").expect("capacity XML sinks are infallible");
        for (index, cell) in row.iter().enumerate() {
            let column = column_name(index);
            match &cell.value {
                SourceValue::Text { value } => {
                    write!(output,
                        "<c r=\"{column}{number}\" s=\"0\" t=\"inlineStr\"><is><t xml:space=\"preserve\">"
                    ).expect("capacity XML sinks are infallible");
                    output.text(value);
                    write!(output, "</t></is></c>").expect("capacity XML sinks are infallible");
                }
                SourceValue::Number { value } => write!(
                    output,
                    "<c r=\"{column}{number}\" s=\"0\" t=\"n\"><v>{value}</v></c>"
                )
                .expect("capacity XML sinks are infallible"),
                SourceValue::Empty | SourceValue::Boolean { .. } | SourceValue::Date { .. } => {
                    write!(output, "<c r=\"{column}{number}\" s=\"0\" t=\"n\"></c>")
                        .expect("capacity XML sinks are infallible");
                }
            }
        }
        write!(output, "</row>").expect("capacity XML sinks are infallible");
    }
    write!(output, "</sheetData></worksheet>").expect("capacity XML sinks are infallible");
}

fn escaped_xml_bytes(value: &str) -> usize {
    value
        .bytes()
        .map(|byte| match byte {
            b'&' | b'\r' => 5,
            b'<' | b'>' => 4,
            b'"' | b'\'' => 6,
            _ => 1,
        })
        .sum()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceWorkbook {
    pub sheets: Vec<SourceSheet>,
    pub ledger: Vec<FidelityFinding>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceSheet {
    pub name: String,
    pub has_header: bool,
    pub columns: Vec<SourceColumn>,
    /// Data rows exclude the header and retain every intervening empty source row/column.
    pub rows: Vec<Vec<SourceCell>>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceColumn {
    pub name: String,
    pub width: Option<f64>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SourceCell {
    /// When `formula` is present this is source cached evidence only, never canonical truth.
    pub value: SourceValue,
    /// Raw A1 authoring source; the runtime must translate, bind and calculate it in Rust.
    pub formula: Option<String>,
    pub style: CellStyle,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceValue {
    #[default]
    Empty,
    Text {
        value: String,
    },
    Number {
        value: f64,
    },
    Boolean {
        value: bool,
    },
    Date {
        value: String,
    },
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CellStyle {
    pub number_format: Option<String>,
    pub bold: bool,
    pub fill: Option<String>,
    pub wrap: bool,
    pub border: bool,
    pub alignment: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FidelityCategory {
    NativeEquivalent,
    PreservedReadable,
    Converted,
    UnsupportedSafeDisabled,
    LossyOnExport,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FidelityFinding {
    pub category: FidelityCategory,
    pub code: String,
    pub location: String,
    pub message: String,
    /// Sole admission decision: any blocking finding prevents semantic import/export.
    pub blocking: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImportOptions {
    pub delimiter: char,
    pub header: bool,
}
impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            delimiter: ',',
            header: true,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct InteropError(pub String);
type Result<T> = std::result::Result<T, InteropError>;
fn fail<T>(message: impl Into<String>) -> Result<T> {
    Err(InteropError(message.into()))
}
fn finding(
    category: FidelityCategory,
    code: &str,
    location: &str,
    message: &str,
    blocking: bool,
) -> FidelityFinding {
    FidelityFinding {
        category,
        code: code.into(),
        location: location.into(),
        message: message.into(),
        blocking,
    }
}
fn source_bound(bytes: &[u8]) -> Result<()> {
    if bytes.is_empty() || bytes.len() > MAX_SOURCE_BYTES {
        fail("Source must contain 1..=2097152 bytes")
    } else {
        Ok(())
    }
}

/// Parse CSV without accepting inferred types.
/// # Errors
/// Rejects invalid UTF-8, quoting, headers or profile bounds.
#[allow(clippy::too_many_lines)] // One bounded delimiter state machine.
pub fn import_csv(bytes: &[u8], options: &ImportOptions) -> Result<SourceWorkbook> {
    source_bound(bytes)?;
    if ![',', ';', '\t'].contains(&options.delimiter) {
        return fail("Unsupported CSV delimiter");
    }
    let text = std::str::from_utf8(bytes).map_err(|_| InteropError("CSV must be UTF-8".into()))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut closed = false;
    let mut touched = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        touched = true;
        if quoted {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                    closed = true;
                }
            } else {
                field.push(ch);
            }
        } else if ch == options.delimiter {
            row.push(std::mem::take(&mut field));
            closed = false;
        } else if ch == '\r' || ch == '\n' {
            if ch == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut field));
            rows.push(std::mem::take(&mut row));
            closed = false;
            touched = false;
        } else if closed {
            return fail("CSV contains characters after a closing quote");
        } else if ch == '"' {
            if !field.is_empty() {
                return fail("CSV quote inside unquoted field");
            }
            quoted = true;
        } else {
            field.push(ch);
        }
        if row.len() > MAX_COLUMNS
            || rows.len() > MAX_TEXT_CAPACITY_ROWS + usize::from(options.header)
        {
            return fail("CSV exceeds 8,406 row or column bounds");
        }
    }
    if quoted {
        return fail("CSV contains an unclosed quoted field");
    }
    if touched || !row.is_empty() || !field.is_empty() {
        row.push(field);
        rows.push(row);
    }
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    if width == 0
        || width > MAX_COLUMNS
        || rows.len() > MAX_TEXT_CAPACITY_ROWS + usize::from(options.header)
    {
        return fail("CSV exceeds profile bounds");
    }
    let headers = if options.header {
        rows.remove(0)
    } else {
        (1..=width).map(|n| format!("Column {n}")).collect()
    };
    if headers.len() != width
        || headers.iter().any(String::is_empty)
        || headers.iter().collect::<BTreeSet<_>>().len() != width
    {
        return fail("CSV headers must be nonempty and unique");
    }
    let columns = headers
        .into_iter()
        .map(|name| SourceColumn { name, width: None })
        .collect();
    let rows = rows
        .into_iter()
        .map(|mut row| {
            row.resize(width, String::new());
            row.into_iter()
                .map(|value| SourceCell {
                    value: if value.is_empty() {
                        SourceValue::Empty
                    } else {
                        SourceValue::Text { value }
                    },
                    ..SourceCell::default()
                })
                .collect()
        })
        .collect();
    let workbook = SourceWorkbook {
        sheets: vec![SourceSheet {
            name: "Imported table".into(),
            has_header: options.header,
            columns,
            rows,
        }],
        ledger: vec![
            finding(
                FidelityCategory::NativeEquivalent,
                "csv_utf8",
                "source",
                "UTF-8 CSV retained quoted values, empty positions and explicit delimiter/header choice",
                false,
            ),
            finding(
                FidelityCategory::PreservedReadable,
                "csv_inference_advisory",
                "source",
                "CSV values remain Text or missing until explicit type acceptance; no locale-sensitive inference is authoritative",
                false,
            ),
        ],
    };
    if workbook.sheets[0].rows.len() > MAX_DATA_ROWS {
        if !text_capacity_shape(&workbook) {
            return fail("CSV exceeds the bounded Text/Number capacity rectangle");
        }
        // Validate an isolated typed feasibility copy, retaining the lexical
        // source values for explicit user selection.
        validate_capacity_source_inspection(&workbook)?;
        return Ok(workbook);
    }
    Ok(finish_source_admission(workbook))
}

// Namespace-aware bounded XML tree. No DTD or entity expansion is accepted.
#[derive(Clone, Debug)]
struct Xml {
    name: String,
    ns: String,
    attrs: BTreeMap<String, String>,
    namespace_declarations: Vec<String>,
    children: Vec<Xml>,
    text: String,
    plain_attribute_namespaces: bool,
    plain_lexical_values: bool,
}
impl Xml {
    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.get(key).map(String::as_str)
    }
    fn child(&self, name: &str) -> Option<&Xml> {
        self.children
            .iter()
            .find(|x| x.name == name && x.ns == self.ns)
    }
    fn kids<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Xml> {
        self.children
            .iter()
            .filter(move |x| x.name == name && x.ns == self.ns)
    }
    fn all<'a>(&'a self, name: &str, out: &mut Vec<&'a Xml>) {
        if self.name == name {
            out.push(self);
        }
        for c in &self.children {
            c.all(name, out);
        }
    }
    fn content(&self) -> String {
        if self.name == "t" {
            return self.text.clone();
        }
        let mut s = String::new();
        for c in self
            .children
            .iter()
            .filter(|c| c.ns == self.ns && matches!(c.name.as_str(), "t" | "r"))
        {
            s.push_str(&c.content());
        }
        s
    }
}
const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

fn xml_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
}
fn plain_qname(name: &[u8]) -> bool {
    let parts = name.split(|byte| *byte == b':').collect::<Vec<_>>();
    parts.len() <= 2
        && parts.iter().all(|part| {
            part.first()
                .is_some_and(|first| first.is_ascii_alphabetic() || *first == b'_')
                && part
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        })
}
fn plain_tag(raw: &[u8], attributes: bool) -> bool {
    let mut position = raw
        .iter()
        .position(|byte| xml_space(*byte))
        .unwrap_or(raw.len());
    if !plain_qname(&raw[..position]) {
        return false;
    }
    while position < raw.len() {
        let separator = position;
        while position < raw.len() && xml_space(raw[position]) {
            position += 1;
        }
        if position == raw.len() {
            return true;
        }
        if !attributes || position == separator {
            return false;
        }
        let start = position;
        while position < raw.len() && !xml_space(raw[position]) && raw[position] != b'=' {
            position += 1;
        }
        if !plain_qname(&raw[start..position]) {
            return false;
        }
        while position < raw.len() && xml_space(raw[position]) {
            position += 1;
        }
        if raw.get(position) != Some(&b'=') {
            return false;
        }
        position += 1;
        while position < raw.len() && xml_space(raw[position]) {
            position += 1;
        }
        let Some(&quote @ (b'\'' | b'"')) = raw.get(position) else {
            return false;
        };
        position += 1;
        while position < raw.len() && raw[position] != quote {
            if raw[position] == b'<' {
                return false;
            }
            position += 1;
        }
        if position == raw.len() {
            return false;
        }
        position += 1;
    }
    true
}
fn plain_token(raw: &[u8], event: &Event<'_>) -> bool {
    match event {
        Event::Start(_) => raw
            .strip_prefix(b"<")
            .and_then(|v| v.strip_suffix(b">"))
            .is_some_and(|v| plain_tag(v, true)),
        Event::Empty(_) => raw
            .strip_prefix(b"<")
            .and_then(|v| v.strip_suffix(b"/>"))
            .is_some_and(|v| plain_tag(v, true)),
        Event::End(_) => raw
            .strip_prefix(b"</")
            .and_then(|v| v.strip_suffix(b">"))
            .is_some_and(|v| plain_tag(v, false)),
        Event::Decl(_) => raw
            .strip_prefix(b"<?")
            .and_then(|v| v.strip_suffix(b"?>"))
            .is_some_and(|v| plain_tag(v, true)),
        Event::Text(_) => !raw.windows(3).any(|token| token == b"]]>"),
        Event::CData(_) | Event::Eof => true,
        _ => false,
    }
}

// Only a complete UTF-8 XML 1.0 declaration has a plain-source proof.
fn capacity_plain_declaration(decl: &quick_xml::events::BytesDecl<'_>) -> bool {
    let Ok(text) = std::str::from_utf8(decl.as_ref()) else {
        return false;
    };
    let declaration = quick_xml::events::BytesStart::from_content(text, 3);
    let mut position = 0;
    for attribute in declaration.attributes() {
        let Ok(attribute) = attribute else {
            return false;
        };
        position = match (attribute.key.as_ref(), attribute.value.as_ref(), position) {
            (b"version", b"1.0", 0) => 1,
            (b"encoding", value, 1) if value.eq_ignore_ascii_case(b"UTF-8") => 2,
            (b"standalone", b"yes" | b"no", 1 | 2) => 3,
            _ => return false,
        };
    }
    position > 0
}

#[allow(clippy::too_many_lines)] // One XML event state machine and namespace scope.
fn parse_xml(bytes: &[u8]) -> Result<Xml> {
    let input = std::str::from_utf8(bytes).map_err(|_| InteropError("XML must be UTF-8".into()))?;
    if input.chars().any(|c| {
        (c < ' ' && !matches!(c, '\t' | '\n' | '\r')) || matches!(c, '\u{fffe}' | '\u{ffff}')
    }) {
        return fail("Invalid XML character");
    }
    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<(Xml, BTreeMap<String, String>)> = Vec::new();
    let mut root = None;
    // Reader positions exclude the optional UTF-8 BOM and preserve raw closing tags.
    let tokens = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    let mut nodes = 0usize;
    let mut plain_document = true;
    let mut document_event = false;
    loop {
        let start = usize::try_from(reader.buffer_position())
            .map_err(|_| InteropError("XML offset overflow".into()))?;
        let event = reader
            .read_event()
            .map_err(|e| InteropError(format!("Invalid XML: {e}")))?;
        let end = usize::try_from(reader.buffer_position())
            .map_err(|_| InteropError("XML offset overflow".into()))?;
        plain_document &= tokens
            .get(start..end)
            .is_some_and(|raw| plain_token(raw, &event));
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(e) | Event::Empty(e) => {
                nodes += 1;
                if nodes > 100_000 || stack.len() >= 64 {
                    return fail("XML structure exceeds bounded profile");
                }
                let mut bindings = stack
                    .last()
                    .map_or_else(BTreeMap::new, |(_, ns)| ns.clone());
                bindings
                    .entry("xml".into())
                    .or_insert_with(|| "http://www.w3.org/XML/1998/namespace".into());
                let mut attrs = BTreeMap::new();
                let mut namespace_declarations = Vec::new();
                let mut attribute_count = 0;
                let mut plain_lexical_values = true;
                for a in e.attributes() {
                    attribute_count += 1;
                    if attribute_count > 64 {
                        return fail("XML attribute count exceeds profile");
                    }
                    let a = a.map_err(|e| InteropError(e.to_string()))?;
                    plain_lexical_values &=
                        !a.value.iter().any(|b| matches!(b, b'\t' | b'\n' | b'\r'));
                    let key = std::str::from_utf8(a.key.as_ref())
                        .map_err(|_| InteropError("Invalid XML attribute".into()))?
                        .to_owned();
                    let value = a
                        .unescape_value()
                        .map_err(|e| InteropError(e.to_string()))?
                        .into_owned();
                    if !xml_text_valid(&value) {
                        return fail("Invalid escaped XML attribute character");
                    }
                    if key.len() > 256 || value.len() > 65_536 {
                        return fail("XML attribute exceeds profile");
                    }
                    if key == "xmlns" {
                        plain_lexical_values &= !matches!(
                            value.as_str(),
                            "http://www.w3.org/XML/1998/namespace"
                                | "http://www.w3.org/2000/xmlns/"
                        );
                        if value.len() > 512 {
                            return fail("XML namespace URI exceeds profile");
                        }
                        namespace_declarations.push(value.clone());
                        bindings.insert(String::new(), value);
                    } else if let Some(prefix) = key.strip_prefix("xmlns:") {
                        plain_lexical_values &= prefix != "xmlns"
                            && (prefix == "xml")
                                == (value == "http://www.w3.org/XML/1998/namespace")
                            && value != "http://www.w3.org/2000/xmlns/";
                        if value.len() > 512 {
                            return fail("XML namespace URI exceeds profile");
                        }
                        namespace_declarations.push(value.clone());
                        bindings.insert(prefix.into(), value);
                    } else {
                        attrs.insert(key, value);
                    }
                }
                if bindings.len() > 64 {
                    return fail("XML namespace scope exceeds profile");
                }
                let mut expanded_attributes = BTreeSet::new();
                let mut plain_attribute_namespaces = true;
                for key in attrs.keys() {
                    let (prefix, local) = key.split_once(':').unwrap_or(("", key));
                    let uri = if prefix.is_empty() {
                        ""
                    } else {
                        bindings
                            .get(prefix)
                            .map(String::as_str)
                            .ok_or_else(|| InteropError("Unbound XML attribute namespace".into()))?
                    };
                    if !expanded_attributes.insert((uri, local)) {
                        return fail("Duplicate expanded XML attribute");
                    }
                    plain_attribute_namespaces &= uri.is_empty()
                        || (uri
                            == "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                            && local == "id")
                        || (uri == "http://www.w3.org/XML/1998/namespace" && local == "space");
                }
                let qname = std::str::from_utf8(e.name().as_ref())
                    .map_err(|_| InteropError("Invalid XML element".into()))?
                    .to_owned();
                let (prefix, name) = qname.split_once(':').unwrap_or(("", &qname));
                let ns = bindings.get(prefix).cloned().unwrap_or_default();
                if !prefix.is_empty() && ns.is_empty() {
                    return fail("Unbound XML namespace prefix");
                }
                // Normalize relationship id attributes by URI, not prefix spelling.
                for (key, value) in attrs.clone() {
                    if let Some((prefix, local)) = key.split_once(':') {
                        if bindings.get(prefix).is_some_and(|ns|ns=="http://schemas.openxmlformats.org/officeDocument/2006/relationships") { attrs.insert(format!("rel:{local}"),value); }
                    }
                }
                let node = Xml {
                    name: name.into(),
                    ns,
                    attrs,
                    namespace_declarations,
                    children: Vec::new(),
                    text: String::new(),
                    plain_attribute_namespaces,
                    plain_lexical_values,
                };
                if empty {
                    attach_xml(node, &mut stack, &mut root)?;
                } else {
                    stack.push((node, bindings));
                }
            }
            Event::End(_) => {
                let (node, _) = stack
                    .pop()
                    .ok_or_else(|| InteropError("Unexpected XML end".into()))?;
                attach_xml(node, &mut stack, &mut root)?;
            }
            Event::Text(e) => {
                let value = e.unescape().map_err(|e| InteropError(e.to_string()))?;
                if !xml_text_valid(&value) {
                    return fail("Invalid escaped XML character");
                }
                if let Some((node, _)) = stack.last_mut() {
                    node.plain_lexical_values &=
                        !matches!(node.name.as_str(), "t" | "v") || !e.as_ref().contains(&b'\r');
                    node.text.push_str(&value);
                } else {
                    plain_document &= e
                        .as_ref()
                        .iter()
                        .all(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'));
                    if !value.trim().is_empty() {
                        return fail("Text outside XML root");
                    }
                }
            }
            Event::CData(e) => {
                let value = std::str::from_utf8(e.as_ref())
                    .map_err(|_| InteropError("Invalid CDATA".into()))?;
                if !xml_text_valid(value) {
                    return fail("Invalid CDATA XML character");
                }
                if let Some((node, _)) = stack.last_mut() {
                    node.plain_lexical_values &=
                        !matches!(node.name.as_str(), "t" | "v") || !e.as_ref().contains(&b'\r');
                    node.text.push_str(value);
                } else {
                    return fail("CDATA outside XML root");
                }
            }
            Event::Decl(decl) => {
                plain_document &= !document_event && capacity_plain_declaration(&decl);
            }
            Event::DocType(_) => return fail("DTD is forbidden"),
            Event::Eof => break,
            // Discarded comments and processing instructions have no plain proof.
            _ => plain_document = false,
        }
        document_event = true;
    }
    if !stack.is_empty() {
        return fail("Unclosed XML element");
    }
    let mut root = root.ok_or_else(|| InteropError("Missing XML root".into()))?;
    root.plain_lexical_values &= plain_document;
    Ok(root)
}
fn attach_xml(
    node: Xml,
    stack: &mut [(Xml, BTreeMap<String, String>)],
    root: &mut Option<Xml>,
) -> Result<()> {
    if let Some((parent, _)) = stack.last_mut() {
        parent.children.push(node);
    } else if root.replace(node).is_some() {
        return fail("Multiple XML roots");
    }
    Ok(())
}

fn archive(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    source_bound(bytes)?;
    validate_zip_directory(bytes)?;
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(|e| InteropError(e.to_string()))?;
    if zip.len() > MAX_ZIP_ENTRIES {
        return fail("ZIP entry count exceeds 256");
    }
    let mut result = BTreeMap::new();
    let mut total = 0usize;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| InteropError(e.to_string()))?;
        let name = file.name().to_owned();
        if name.is_empty()
            || name.starts_with('/')
            || name.contains(['\\', ':', '\0'])
            || name.split('/').any(|s| s == ".." || s == ".")
        {
            return fail("Unsafe ZIP entry path");
        }
        if result.contains_key(&name) {
            return fail("Duplicate ZIP entry path");
        }
        let size =
            usize::try_from(file.size()).map_err(|_| InteropError("ZIP size overflow".into()))?;
        total = total
            .checked_add(size)
            .ok_or_else(|| InteropError("ZIP size overflow".into()))?;
        if total > MAX_EXPANDED_BYTES {
            return fail("Expanded ZIP exceeds 8 MiB");
        }
        let mut contents = Vec::new();
        (&mut file)
            .take((size + 1) as u64)
            .read_to_end(&mut contents)
            .map_err(|e| InteropError(e.to_string()))?;
        if contents.len() != size {
            return fail("ZIP expanded size mismatch");
        }
        result.insert(name, contents);
    }
    Ok(result)
}
fn zip_u16(bytes: &[u8], offset: usize) -> Result<usize> {
    let raw = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| InteropError("Truncated ZIP directory".into()))?;
    Ok(usize::from(u16::from_le_bytes([raw[0], raw[1]])))
}
fn zip_u32(bytes: &[u8], offset: usize) -> Result<usize> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| InteropError("Truncated ZIP directory".into()))?;
    usize::try_from(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
        .map_err(|_| InteropError("ZIP directory offset overflow".into()))
}
fn validate_zip_directory(bytes: &[u8]) -> Result<()> {
    // zip indexes entries by name, so duplicate names must be rejected before its index collapses them.
    let start = bytes.len().saturating_sub(65_557);
    let end = (start..bytes.len().saturating_sub(21))
        .rev()
        .find(|&p| {
            bytes.get(p..p + 4) == Some(b"PK\x05\x06")
                && zip_u16(bytes, p + 20).is_ok_and(|n| p + 22 + n == bytes.len())
        })
        .ok_or_else(|| InteropError("Missing ZIP end directory".into()))?;
    if zip_u16(bytes, end + 4)? != 0 || zip_u16(bytes, end + 6)? != 0 {
        return fail("Multi-disk ZIP is outside profile");
    }
    let count = zip_u16(bytes, end + 10)?;
    if count > MAX_ZIP_ENTRIES || zip_u16(bytes, end + 8)? != count {
        return fail("ZIP entry count exceeds profile");
    }
    let mut pos = zip_u32(bytes, end + 16)?;
    let size = zip_u32(bytes, end + 12)?;
    if pos.checked_add(size) != Some(end) {
        return fail("Invalid ZIP central directory bounds");
    }
    let mut names = BTreeSet::new();
    for _ in 0..count {
        if bytes.get(pos..pos + 4) != Some(b"PK\x01\x02") {
            return fail("Invalid ZIP central entry");
        }
        let name_len = zip_u16(bytes, pos + 28)?;
        let extra = zip_u16(bytes, pos + 30)?;
        let comment = zip_u16(bytes, pos + 32)?;
        let name = bytes
            .get(pos + 46..pos + 46 + name_len)
            .ok_or_else(|| InteropError("Truncated ZIP name".into()))?;
        if !names.insert(name) {
            return fail("Duplicate ZIP entry path");
        }
        pos = pos
            .checked_add(46 + name_len + extra + comment)
            .ok_or_else(|| InteropError("ZIP directory overflow".into()))?;
        if pos > end {
            return fail("ZIP directory outside archive");
        }
    }
    if pos != end {
        return fail("Unexpected ZIP directory remainder");
    }
    Ok(())
}
fn part<'a>(parts: &'a BTreeMap<String, Vec<u8>>, name: &str) -> Result<&'a [u8]> {
    parts
        .get(name)
        .map(Vec::as_slice)
        .ok_or_else(|| InteropError(format!("Missing XLSX part {name}")))
}
fn xml_part(parts: &BTreeMap<String, Vec<u8>>, name: &str, root: &str, ns: &str) -> Result<Xml> {
    let xml = parse_xml(part(parts, name)?)?;
    if xml.name != root || xml.ns != ns {
        return fail(format!("Invalid namespace/root for {name}"));
    }
    Ok(xml)
}
fn rel_target(base: &str, target: &str) -> Result<String> {
    if target.contains(['\\', ':', '\0', '?', '#']) {
        return fail("Unsupported relationship target");
    }
    let mut result = if target.starts_with('/') {
        Vec::new()
    } else {
        base.split('/')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
    };
    for piece in target.trim_start_matches('/').split('/') {
        match piece {
            ".." => {
                if result.pop().is_none() {
                    return fail("Relationship escapes ZIP root");
                }
            }
            "." => {}
            "" => return fail("Empty relationship path segment"),
            _ => result.push(piece),
        }
    }
    Ok(result.join("/"))
}
const FEATURE_BAG_NS: &str =
    "http://schemas.microsoft.com/office/spreadsheetml/2022/featurepropertybag";

/// Recognize only the complete source-only checkbox mapper represented by the
/// ordinary workbook fixture. Bag indices are positional references, so the
/// accepted order and links are explicit rather than inferred from type names.
fn checkbox_property_bags(root: &Xml) -> bool {
    fn shape(node: &Xml, name: &str, attrs: &[(&str, &str)], text: &str, count: usize) -> bool {
        node.name == name
            && node.ns == FEATURE_BAG_NS
            && node
                .namespace_declarations
                .iter()
                .all(|ns| ns == FEATURE_BAG_NS)
            && node.attrs.len() == attrs.len()
            && attrs
                .iter()
                .all(|(key, value)| node.attr(key) == Some(*value))
            && node.text.trim_matches([' ', '\t', '\n', '\r']) == text
            && node.children.len() == count
    }
    if !shape(root, "FeaturePropertyBags", &[], "", 4) {
        return false;
    }
    let bags = &root.children;
    shape(&bags[0], "bag", &[("type", "Checkbox")], "", 0)
        && shape(&bags[1], "bag", &[("type", "XFControls")], "", 1)
        && shape(
            &bags[1].children[0],
            "bagId",
            &[("k", "CellControl")],
            "0",
            0,
        )
        && shape(&bags[2], "bag", &[("type", "XFComplement")], "", 1)
        && shape(
            &bags[2].children[0],
            "bagId",
            &[("k", "XFControls")],
            "1",
            0,
        )
        && shape(
            &bags[3],
            "bag",
            &[
                ("type", "XFComplements"),
                ("extRef", "XFComplementsMapperExtRef"),
            ],
            "",
            1,
        )
        && shape(
            &bags[3].children[0],
            "a",
            &[("k", "MappedFeaturePropertyBags")],
            "",
            1,
        )
        && shape(&bags[3].children[0].children[0], "bagId", &[], "2", 0)
}

#[allow(clippy::case_sensitive_file_extension_comparisons)] // Compared path is already ASCII-lowercase.
fn root_inventory(
    parts: &BTreeMap<String, Vec<u8>>,
    worksheets: &BTreeSet<String>,
    ledger: &mut Vec<FidelityFinding>,
) -> Result<()> {
    let worksheet_relationships = worksheets
        .iter()
        .map(|path| worksheet_relationship_path(path))
        .collect::<BTreeSet<_>>();
    for (path, bytes) in parts {
        let lower = path.to_ascii_lowercase();
        let mut inventoried = false;
        if lower == "xl/featurepropertybag/featurepropertybag.xml" {
            let xml = parse_xml(bytes)?;
            if checkbox_property_bags(&xml) {
                inventoried = true;
                ledger.push(finding(FidelityCategory::LossyOnExport,"checkbox_presentation",path,"Checkbox control presentation is source-only; Boolean cell values are preserved",false));
            }
        }
        for (needle, code) in [
            ("vbaproject", "macro_disabled"),
            ("macrosheets/", "xlm_disabled"),
            ("activex/", "activex_disabled"),
            ("embeddings/", "ole_disabled"),
            ("externallinks/", "external_link_disabled"),
            ("connections", "connection_disabled"),
            ("pivot", "pivot_unsupported"),
            ("charts/", "chart_unsupported"),
            ("tables/", "table_rules_unsupported"),
        ] {
            if lower.contains(needle) {
                inventoried = true;
                ledger.push(finding(FidelityCategory::UnsupportedSafeDisabled,code,path,"Source-only construct retained in original bytes; never executed or emitted by bounded export",false));
            }
        }
        let structural = matches!(
            path.as_str(),
            "[Content_Types].xml"
                | "_rels/.rels"
                | "xl/workbook.xml"
                | "xl/_rels/workbook.xml.rels"
                | "xl/styles.xml"
                | "xl/sharedStrings.xml"
        ) || worksheets.contains(path);
        if worksheet_relationships.contains(path) {
            ledger.push(worksheet_relationship_finding(path, bytes)?);
            inventoried = true;
        }
        let presentation = lower.starts_with("docprops/")
            || lower.starts_with("xl/theme/")
            || lower == "xl/calcchain.xml"
            || lower.starts_with("xl/printersettings/");
        if !structural && !inventoried && !path.ends_with('/') {
            ledger.push(finding(
                if presentation {
                    FidelityCategory::LossyOnExport
                } else {
                    FidelityCategory::UnsupportedSafeDisabled
                },
                if presentation {
                    "source_metadata_not_exported"
                } else {
                    "unknown_package_part"
                },
                path,
                if presentation {
                    "Source metadata/layout is not emitted by this profile"
                } else {
                    "Unknown package part has no declared semantic mapping"
                },
                !presentation,
            ));
        }
        if lower.ends_with(".xml") || lower.ends_with(".rels") {
            let xml = parse_xml(bytes)?;
            let mut relationships = Vec::new();
            xml.all("Relationship", &mut relationships);
            for node in relationships {
                if node.ns == REL && node.attr("TargetMode") == Some("External") {
                    ledger.push(finding(
                        FidelityCategory::UnsupportedSafeDisabled,
                        "external_relationship",
                        path,
                        "External relationship will not be followed",
                        false,
                    ));
                }
            }
        }
    }
    Ok(())
}
fn worksheet_relationship_path(path: &str) -> String {
    path.rsplit_once('/').map_or_else(
        || format!("_rels/{path}.rels"),
        |(parent, name)| format!("{parent}/_rels/{name}.rels"),
    )
}
fn worksheet_relationship_finding(path: &str, bytes: &[u8]) -> Result<FidelityFinding> {
    let xml = parse_xml(bytes)?;
    if xml.name != "Relationships"
        || xml.ns != REL
        || xml
            .children
            .iter()
            .any(|node| node.name != "Relationship" || node.ns != REL || !node.children.is_empty())
    {
        return fail("Invalid worksheet relationships part");
    }
    Ok(finding(
        FidelityCategory::LossyOnExport,
        "worksheet_relationships_not_exported",
        path,
        "Worksheet relationships remain source-only and are not emitted",
        false,
    ))
}
fn builtin_format(id: u32) -> Option<String> {
    match id {
        0 => None,
        1 => Some("0".into()),
        2 => Some("0.00".into()),
        3 => Some("#,##0".into()),
        4 => Some("#,##0.00".into()),
        9 => Some("0%".into()),
        10 => Some("0.00%".into()),
        11 => Some("0.00E+00".into()),
        12 => Some("# ?/?".into()),
        13 => Some("# ??/??".into()),
        14 => Some("mm-dd-yy".into()),
        15 => Some("d-mmm-yy".into()),
        16 => Some("d-mmm".into()),
        17 => Some("mmm-yy".into()),
        18 => Some("h:mm AM/PM".into()),
        19 => Some("h:mm:ss AM/PM".into()),
        20 => Some("h:mm".into()),
        21 => Some("h:mm:ss".into()),
        22 => Some("m/d/yy h:mm".into()),
        37 => Some("#,##0 ;(#,##0)".into()),
        38 => Some("#,##0 ;[Red](#,##0)".into()),
        39 => Some("#,##0.00;(#,##0.00)".into()),
        40 => Some("#,##0.00;[Red](#,##0.00)".into()),
        45 => Some("mm:ss".into()),
        46 => Some("[h]:mm:ss".into()),
        47 => Some("mmss.0".into()),
        48 => Some("##0.0E+0".into()),
        49 => Some("@".into()),
        _ => Some(format!("unsupported_builtin_{id}")),
    }
}
fn capacity_styles_xml() -> String {
    format!(
        "<styleSheet xmlns=\"{MAIN}\"><fonts count=\"1\"><font><sz val=\"11\"/><name val=\"Arial\"/></font></fonts><fills count=\"2\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill></fills><borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs><cellXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/></cellXfs></styleSheet>"
    )
}

fn same_plain_tree(actual: &Xml, expected: &Xml) -> bool {
    actual.plain_attribute_namespaces
        && actual.plain_lexical_values
        && actual.namespace_declarations.iter().all(|uri| uri == MAIN)
        && actual.name == expected.name
        && actual.ns == expected.ns
        && actual.attrs == expected.attrs
        && actual.text.trim_matches([' ', '\t', '\n', '\r'])
            == expected.text.trim_matches([' ', '\t', '\n', '\r'])
        && actual.children.len() == expected.children.len()
        && actual
            .children
            .iter()
            .zip(&expected.children)
            .all(|(a, b)| same_plain_tree(a, b))
}

/// Closed raw-source proof for the minimal fixture and private plain writer.
/// Unknown presentation remains admissible only under the existing generic ledger.
fn capacity_plain_node(node: &Xml) -> bool {
    const TYPES: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
    const OFFICE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    let (namespace, attrs, children): (&str, &[&str], &[&str]) = match node.name.as_str() {
        "Types" => (TYPES, &[], &["Default", "Override"]),
        "Default" => (TYPES, &["Extension", "ContentType"], &[]),
        "Override" => (TYPES, &["PartName", "ContentType"], &[]),
        "Relationships" => (REL, &[], &["Relationship"]),
        "Relationship" => (REL, &["Id", "Type", "Target"], &[]),
        "workbook" => (MAIN, &[], &["workbookPr", "sheets"]),
        "workbookPr" => (MAIN, &["date1904"], &[]),
        "sheets" => (MAIN, &[], &["sheet"]),
        "sheet" => (MAIN, &["name", "sheetId", "rel:id"], &[]),
        "worksheet" => (MAIN, &[], &["sheetData"]),
        "sheetData" => (MAIN, &[], &["row"]),
        "row" => (MAIN, &["r"], &["c"]),
        "c" => (MAIN, &["r", "s", "t"], &["v", "is"]),
        "sst" => (MAIN, &["count", "uniqueCount"], &["si"]),
        "si" | "is" => (MAIN, &[], &["t"]),
        "t" => (MAIN, &["xml:space"], &[]),
        "v" => (MAIN, &[], &[]),
        _ => return false,
    };
    if node.ns != namespace
        || (matches!(node.name.as_str(), "t" | "v") && st_xstring(&node.text))
        || !node.plain_attribute_namespaces
        || !node.plain_lexical_values
        || !node.namespace_declarations.iter().all(|uri| {
            [
                MAIN,
                REL,
                TYPES,
                OFFICE,
                "http://www.w3.org/XML/1998/namespace",
            ]
            .contains(&uri.as_str())
        })
        || !node.attrs.keys().all(|key| {
            attrs.contains(&match key.rsplit_once(':') {
                Some((_, "id")) => "rel:id",
                Some((_, "space")) => "xml:space",
                _ => key.as_str(),
            })
        })
        || !node
            .children
            .iter()
            .all(|child| children.contains(&child.name.as_str()) && capacity_plain_node(child))
        || (!matches!(node.name.as_str(), "t" | "v")
            && !node.text.trim_matches([' ', '\t', '\n', '\r']).is_empty())
    {
        return false;
    }
    capacity_plain_values(node)
}

fn capacity_plain_values(node: &Xml) -> bool {
    const OFFICE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    match node.name.as_str() {
        "workbook" => node.kids("sheets").count() == 1 && node.kids("workbookPr").count() <= 1,
        "sheets" | "worksheet" | "si" | "is" => node.children.len() == 1,
        "workbookPr" => node.attr("date1904") == Some("0"),
        "sheet" => {
            node.attr("name").is_some()
                && node.attr("sheetId") == Some("1")
                && node.attr("rel:id").is_some()
        }
        "c" => {
            matches!(node.attr("s"), None | Some("0"))
                && match node.attr("t") {
                    Some("inlineStr") => node.children.len() == 1 && node.children[0].name == "is",
                    Some("str" | "s") => node.children.len() == 1 && node.children[0].name == "v",
                    None | Some("n") => {
                        node.children.is_empty()
                            || (node.children.len() == 1 && node.children[0].name == "v")
                    }
                    _ => false,
                }
        }
        "t" => node.attrs.values().all(|value| value == "preserve"),
        "sst" => node
            .attrs
            .values()
            .all(|value| value.parse::<usize>().ok() == Some(node.children.len())),
        "Relationship" => {
            let role = match node.attr("Target") {
                Some("xl/workbook.xml") => "officeDocument",
                Some("worksheets/sheet1.xml") => "worksheet",
                Some("styles.xml") => "styles",
                Some("sharedStrings.xml") => "sharedStrings",
                _ => return false,
            };
            node.attr("Type") == Some(format!("{OFFICE}/{role}").as_str()) && node.attrs.len() == 3
        }
        "Default" => {
            matches!(
                (node.attr("Extension"), node.attr("ContentType")),
                (
                    Some("rels"),
                    Some("application/vnd.openxmlformats-package.relationships+xml")
                ) | (Some("xml"), Some("application/xml"))
            ) && node.attrs.len() == 2
        }
        "Override" => {
            node.attr("PartName")
                .and_then(|path| path.strip_prefix('/'))
                .and_then(capacity_part_kind)
                .is_some_and(|(root, _, kind)| {
                    ["workbook", "worksheet", "styleSheet", "sst"].contains(&root)
                        && node.attr("ContentType") == Some(kind)
                })
                && node.attrs.len() == 2
        }
        _ => true,
    }
}

fn capacity_part_kind(path: &str) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match path {
        "[Content_Types].xml" => (
            "Types",
            "http://schemas.openxmlformats.org/package/2006/content-types",
            "application/xml",
        ),
        "_rels/.rels" | "xl/_rels/workbook.xml.rels" => (
            "Relationships",
            REL,
            "application/vnd.openxmlformats-package.relationships+xml",
        ),
        "xl/workbook.xml" => (
            "workbook",
            MAIN,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
        ),
        "xl/worksheets/sheet1.xml" => (
            "worksheet",
            MAIN,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml",
        ),
        "xl/styles.xml" => (
            "styleSheet",
            MAIN,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml",
        ),
        "xl/sharedStrings.xml" => (
            "sst",
            MAIN,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml",
        ),
        _ => return None,
    })
}

fn capacity_plain_parts(parts: &BTreeMap<String, Vec<u8>>) -> Result<bool> {
    if [
        "[Content_Types].xml",
        "_rels/.rels",
        "xl/workbook.xml",
        "xl/_rels/workbook.xml.rels",
        "xl/worksheets/sheet1.xml",
    ]
    .iter()
    .any(|path| !parts.contains_key(*path))
    {
        return Ok(false);
    }
    for (path, bytes) in parts {
        let Some((root, namespace, _)) = capacity_part_kind(path) else {
            return Ok(false);
        };
        let Ok(xml) = parse_xml(bytes) else {
            return Ok(false);
        };
        if xml.name != root || xml.ns != namespace {
            return Ok(false);
        }
        if path == "xl/styles.xml" {
            if !same_plain_tree(&xml, &parse_xml(capacity_styles_xml().as_bytes())?) {
                return Ok(false);
            }
        } else if !capacity_plain_node(&xml) || !capacity_plain_bindings(path, &xml, parts) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn capacity_plain_bindings(path: &str, xml: &Xml, parts: &BTreeMap<String, Vec<u8>>) -> bool {
    let main_parts = parts.keys().filter(|path| {
        capacity_part_kind(path).is_some_and(|(root, _, _)| {
            ["workbook", "worksheet", "styleSheet", "sst"].contains(&root)
        })
    });
    match path {
        "[Content_Types].xml" => {
            let expected = main_parts
                .map(|path| format!("/{path}"))
                .collect::<BTreeSet<_>>();
            let declared = xml
                .kids("Override")
                .filter_map(|node| node.attr("PartName"))
                .collect::<BTreeSet<_>>();
            expected
                .iter()
                .map(String::as_str)
                .eq(declared.iter().copied())
                && declared.len() == xml.kids("Override").count()
                && xml
                    .kids("Default")
                    .filter_map(|node| node.attr("Extension"))
                    .collect::<BTreeSet<_>>()
                    == ["rels", "xml"].into_iter().collect()
                && xml.kids("Default").count() == 2
        }
        "_rels/.rels" => {
            xml.children.len() == 1 && xml.children[0].attr("Target") == Some("xl/workbook.xml")
        }
        "xl/_rels/workbook.xml.rels" => {
            let mut expected = BTreeSet::from(["worksheets/sheet1.xml"]);
            for (part, target) in [
                ("xl/styles.xml", "styles.xml"),
                ("xl/sharedStrings.xml", "sharedStrings.xml"),
            ] {
                if parts.contains_key(part) {
                    expected.insert(target);
                }
            }
            let declared = xml
                .children
                .iter()
                .filter_map(|node| node.attr("Target"))
                .collect::<BTreeSet<_>>();
            declared == expected && declared.len() == xml.children.len()
        }
        _ => true,
    }
}

fn parse_styles(
    parts: &BTreeMap<String, Vec<u8>>,
    ledger: &mut Vec<FidelityFinding>,
) -> Result<Vec<CellStyle>> {
    if !parts.contains_key("xl/styles.xml") {
        return Ok(vec![CellStyle::default()]);
    }
    let xml = xml_part(parts, "xl/styles.xml", "styleSheet", MAIN)?;
    let formats = xml
        .child("numFmts")
        .map(|n| {
            n.kids("numFmt")
                .filter_map(|n| {
                    Some((
                        n.attr("numFmtId")?.parse::<u32>().ok()?,
                        n.attr("formatCode")?.to_owned(),
                    ))
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let fonts = xml
        .child("fonts")
        .map(|n| n.kids("font").collect::<Vec<_>>())
        .unwrap_or_default();
    let fills = xml
        .child("fills")
        .map(|n| n.kids("fill").collect::<Vec<_>>())
        .unwrap_or_default();
    let borders = xml
        .child("borders")
        .map(|n| n.kids("border").collect::<Vec<_>>())
        .unwrap_or_default();
    let mut styles = Vec::new();
    for xf in xml
        .child("cellXfs")
        .map(|n| n.kids("xf").collect::<Vec<_>>())
        .unwrap_or_default()
    {
        let index = |key: &str| {
            xf.attr(key)
                .unwrap_or("0")
                .parse::<usize>()
                .map_err(|_| InteropError("Invalid style index".into()))
        };
        let fmt_id = u32::try_from(index("numFmtId")?)
            .map_err(|_| InteropError("Invalid number format".into()))?;
        let number_format = formats
            .get(&fmt_id)
            .cloned()
            .or_else(|| builtin_format(fmt_id));
        let font = fonts.get(index("fontId")?).copied();
        let fill = fills.get(index("fillId")?).copied();
        let border = borders.get(index("borderId")?).copied();
        let bold = font
            .and_then(|n| n.child("b"))
            .is_some_and(|b| !matches!(b.attr("val"), Some("0" | "false")));
        let fill = fill.and_then(|f| f.child("patternFill")).and_then(|p| {
            if p.attr("patternType") == Some("solid") {
                p.child("fgColor")
                    .and_then(|c| c.attr("rgb"))
                    .map(str::to_owned)
            } else {
                None
            }
        });
        let has_border =
            border.is_some_and(|b| b.children.iter().any(|x| x.attr("style").is_some()));
        let alignment = xf.child("alignment");
        let horizontal = alignment
            .and_then(|a| a.attr("horizontal"))
            .filter(|a| ["left", "center", "right"].contains(a))
            .map(str::to_owned);
        let wrap = alignment.is_some_and(|a| matches!(a.attr("wrapText"), Some("1" | "true")));
        styles.push(CellStyle {
            number_format,
            bold,
            fill,
            wrap,
            border: has_border,
            alignment: horizontal,
        });
    }
    if same_plain_tree(&xml, &parse_xml(capacity_styles_xml().as_bytes())?) {
        ledger.push(finding(
            FidelityCategory::NativeEquivalent,
            "plain_styles",
            "xl/styles.xml",
            "The complete private plain style tree is represented by default cell styles",
            false,
        ));
    } else {
        ledger.push(finding(FidelityCategory::LossyOnExport,"style_profile","xl/styles.xml","Basic bold/RGB solid fill/wrap/border/alignment/number format retained; font family, theme colors and other layout details are source-only",false));
    }
    if styles.is_empty() {
        styles.push(CellStyle::default());
    }
    Ok(styles)
}
fn coordinate(address: &str) -> Result<(usize, usize)> {
    let split = address
        .bytes()
        .position(|b| !b.is_ascii_alphabetic())
        .ok_or_else(|| InteropError("Missing cell row".into()))?;
    let mut col = 0usize;
    for b in address[..split].bytes() {
        col = col
            .checked_mul(26)
            .and_then(|n| n.checked_add(usize::from(b.to_ascii_uppercase() - b'A' + 1)))
            .ok_or_else(|| InteropError("Cell column overflow".into()))?;
        if col > MAX_COLUMNS {
            return fail("Cell column exceeds profile");
        }
    }
    let row = address[split..]
        .parse::<usize>()
        .map_err(|_| InteropError("Invalid cell row".into()))?;
    if col == 0 || row == 0 || row > MAX_TEXT_CAPACITY_ROWS + 1 {
        return fail("Cell row exceeds 8,406 profile");
    }
    Ok((row - 1, col - 1))
}
use crate::interop_number_format::NumberFormatKind;

fn checked_number_format(style: &CellStyle) -> Result<NumberFormatKind> {
    crate::interop_number_format::classify(style.number_format.as_deref()).map_err(InteropError)
}
#[allow(clippy::float_cmp, clippy::cast_possible_truncation)] // Serial is first proven finite, integral, and within the i64 range; serial 60 is exact.
fn date_from_serial(value: f64, date1904: bool) -> Result<String> {
    if !value.is_finite() || value.fract() != 0.0 {
        return fail("Date serial includes time-of-day or non-finite value");
    }
    if !date1904 && value == 60.0 {
        return fail("Excel 1900 serial 60 is not a Gregorian date");
    }
    if !(-700_000.0..=2_900_000.0).contains(&value) {
        return fail("Date serial is outside Gregorian profile");
    }
    let serial = value as i64;
    let days = if date1904 {
        serial - 24107
    } else {
        serial - 25569 + i64::from(serial < 60)
    };
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    y += i64::from(m <= 2);
    let text = format!("{y:04}-{m:02}-{d:02}");
    tachiko_workspace_engine::Date::parse(&text).map_err(|e| InteropError(e.to_string()))?;
    Ok(text)
}
fn cell_value(
    cell: &Xml,
    shared: &[String],
    style: &CellStyle,
    date1904: bool,
) -> Result<SourceValue> {
    if c_unknown(cell) {
        return fail("Unknown semantic cell XML construct");
    }
    let format = checked_number_format(style)?;
    if cell.child("f").is_some() && format != NumberFormatKind::Number {
        return fail("Formula requires a uniform numeric number format");
    }
    let raw = cell.child("v").map_or("", |v| v.text.as_str());
    match cell.attr("t").unwrap_or("n") {
        "s" => shared
            .get(
                raw.parse::<usize>()
                    .map_err(|_| InteropError("Invalid shared string index".into()))?,
            )
            .cloned()
            .map(|value| SourceValue::Text { value })
            .ok_or_else(|| InteropError("Shared string index out of bounds".into())),
        "inlineStr" => Ok(SourceValue::Text {
            value: cell.child("is").map(Xml::content).unwrap_or_default(),
        }),
        "str" => Ok(SourceValue::Text { value: raw.into() }),
        "b" => match raw {
            "1" => Ok(SourceValue::Boolean { value: true }),
            "0" => Ok(SourceValue::Boolean { value: false }),
            _ => fail("Invalid XLSX Boolean"),
        },
        "d" => {
            tachiko_workspace_engine::Date::parse(raw).map_err(|e| InteropError(e.to_string()))?;
            Ok(SourceValue::Date { value: raw.into() })
        }
        "n" => {
            if raw.is_empty() {
                return Ok(SourceValue::Empty);
            }
            let value = raw
                .parse::<f64>()
                .map_err(|_| InteropError("Invalid XLSX Number".into()))?;
            if !value.is_finite() {
                return fail("Nonfinite XLSX Number");
            }
            match format {
                NumberFormatKind::Date => Ok(SourceValue::Date {
                    value: date_from_serial(value, date1904)?,
                }),
                NumberFormatKind::Time => fail("Time-only display has no canonical Date mapping"),
                NumberFormatKind::Number => Ok(SourceValue::Number { value }),
            }
        }
        _ => fail("Unsupported XLSX cell type"),
    }
}
fn c_unknown(node: &Xml) -> bool {
    if node.ns != MAIN {
        return true;
    }
    if matches!(node.name.as_str(), "rPr" | "rPh" | "phoneticPr") {
        return false;
    }
    let allowed: &[&str] = match node.name.as_str() {
        "v" | "f" | "t" => return !node.children.is_empty(),
        "c" => {
            if ["f", "v", "is"]
                .iter()
                .any(|name| node.kids(name).count() > 1)
                || (node.child("v").is_some() && node.child("is").is_some())
                || (node.child("is").is_some() && node.attr("t") != Some("inlineStr"))
                || (node.child("v").is_some() && node.attr("t") == Some("inlineStr"))
            {
                return true;
            }
            &["f", "v", "is"]
        }
        "is" | "si" => {
            if node.kids("t").count() > 1
                || node.kids("phoneticPr").count() > 1
                || (node.child("t").is_some() && node.child("r").is_some())
            {
                return true;
            }
            &["t", "r", "rPh", "phoneticPr"]
        }
        "r" => {
            if node.kids("t").count() != 1 || node.kids("rPr").count() > 1 {
                return true;
            }
            &["rPr", "t"]
        }
        _ => return true,
    };
    !node.text.trim_matches([' ', '\t', '\n', '\r']).is_empty()
        || node
            .children
            .iter()
            .any(|child| !allowed.contains(&child.name.as_str()) || c_unknown(child))
}

fn ignorable_formula_cache(cell: &Xml, style: &CellStyle) -> bool {
    !c_unknown(cell)
        && matches!(
            cell.attr("t").unwrap_or("n"),
            "n" | "e" | "str" | "b" | "s" | "inlineStr" | "d"
        )
        && matches!(checked_number_format(style), Ok(NumberFormatKind::Number))
}
fn inventory_worksheet_children(xml: &Xml, sheet: &str, ledger: &mut Vec<FidelityFinding>) {
    for child in &xml.children {
        let (code, blocking) = if child.ns == MAIN {
            match child.name.as_str() {
                // Cell coordinates derive dimension; columns and data are mapped below.
                "dimension" | "cols" | "sheetData" => continue,
                "dataValidations" => ("validation_rules", false),
                "autoFilter" => ("filter_rules", false),
                "tableParts" => ("table_rules", false),
                "mergeCells" => ("merged_cells", false),
                "conditionalFormatting" => ("conditional_formats", false),
                "drawing" => ("drawing", false),
                "extLst" => ("worksheet_extensions", false),
                "sheetProtection" | "protectedRanges" => ("worksheet_protection", false),
                "hyperlinks" => ("worksheet_hyperlinks", false),
                "sheetPr" | "sheetViews" | "sheetFormatPr" | "printOptions" | "pageMargins"
                | "pageSetup" | "headerFooter" | "rowBreaks" | "colBreaks" | "customSheetViews"
                | "sortState" | "sheetCalcPr" | "ignoredErrors" | "phoneticPr" => {
                    ("worksheet_layout_rules", false)
                }
                _ => ("unknown_worksheet_child", true),
            }
        } else {
            ("unknown_worksheet_child", true)
        };
        ledger.push(finding(
            FidelityCategory::UnsupportedSafeDisabled,
            code,
            &format!("{sheet}:{{{}}}{}", child.ns, child.name),
            "Source worksheet construct is not represented or emitted; original source retains it",
            blocking,
        ));
    }
}

fn inventory_workbook_children(xml: &Xml, ledger: &mut Vec<FidelityFinding>) -> Result<()> {
    for singleton in ["sheets", "workbookPr", "calcPr"] {
        if xml.kids(singleton).count() > 1 {
            return fail("Duplicate singleton workbook construct");
        }
    }
    for child in &xml.children {
        let (code, blocking) = if child.ns == MAIN {
            match child.name.as_str() {
                "sheets" => {
                    if child
                        .children
                        .iter()
                        .any(|node| node.ns != MAIN || node.name != "sheet")
                    {
                        return fail("Unknown workbook sheet construct");
                    }
                    continue;
                }
                "workbookPr"
                    if child.children.is_empty()
                        && child.attrs.keys().all(|key| key == "date1904") =>
                {
                    continue;
                }
                "workbookPr" => ("workbook_properties", false),
                "definedNames" => ("defined_names", false),
                "workbookProtection" => ("workbook_protection", false),
                "fileVersion"
                | "fileSharing"
                | "bookViews"
                | "calcPr"
                | "customWorkbookViews"
                | "extLst" => ("workbook_layout_rules", false),
                _ => ("unknown_workbook_child", true),
            }
        } else {
            ("unknown_workbook_child", true)
        };
        ledger.push(finding(
            FidelityCategory::UnsupportedSafeDisabled,
            code,
            &format!("xl/workbook.xml:{{{}}}{}", child.ns, child.name),
            "Source workbook construct is not represented or emitted; original source retains it",
            blocking,
        ));
    }
    Ok(())
}

fn inventory_grid_attributes(node: &Xml, sheet: &str, ledger: &mut Vec<FidelityFinding>) {
    let (mapped, layout): (&[&str], &[&str]) = if node.name == "row" {
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
        .keys()
        .filter(|key| !mapped.contains(&key.as_str()))
    {
        ledger.push(finding(
            FidelityCategory::UnsupportedSafeDisabled,
            "unmapped_grid_attribute",
            &format!("{sheet}:{}@{key}", node.name),
            "Source row/column attribute is not represented or emitted; original source retains it",
            !layout.contains(&key.as_str()),
        ));
    }
}

fn visibility_finding(code: &str, location: &str) -> FidelityFinding {
    finding(
        FidelityCategory::UnsupportedSafeDisabled,
        code,
        location,
        "Hidden source content cannot be admitted or exported because this profile does not preserve visibility; original source bytes remain unchanged",
        true,
    )
}

fn inventory_hidden(
    value: Option<&str>,
    code: &str,
    location: &str,
    ledger: &mut Vec<FidelityFinding>,
) -> Result<()> {
    match value {
        None | Some("0" | "false") => Ok(()),
        Some("1" | "true") => {
            ledger.push(visibility_finding(code, location));
            Ok(())
        }
        Some(_) => fail("Invalid hidden visibility boolean"),
    }
}

/// Inspect all bounded worksheets and produce the sole fidelity admission ledger.
/// # Errors
/// Rejects malformed archives/XML, unresolved relationships and resource limits.
#[allow(clippy::too_many_lines)] // Ordered admission of workbook, worksheets, cells and headers.
pub fn import_xlsx(bytes: &[u8]) -> Result<SourceWorkbook> {
    let parts = archive(bytes)?;
    let mut ledger = Vec::new();
    let workbook = xml_part(&parts, "xl/workbook.xml", "workbook", MAIN)?;
    inventory_workbook_children(&workbook, &mut ledger)?;
    let relationships = xml_part(&parts, "xl/_rels/workbook.xml.rels", "Relationships", REL)?;
    let mut rels = BTreeMap::new();
    for relationship in relationships.kids("Relationship") {
        let id = relationship
            .attr("Id")
            .ok_or_else(|| InteropError("Missing relationship identity".into()))?;
        if rels.insert(id.to_owned(), relationship).is_some() {
            return fail("Duplicate workbook relationship identity");
        }
    }
    let date1904 = match workbook
        .child("workbookPr")
        .and_then(|node| node.attr("date1904"))
    {
        None | Some("0" | "false") => false,
        Some("1" | "true") => true,
        Some(_) => return fail("Invalid workbook date1904 boolean"),
    };
    let styles = parse_styles(&parts, &mut ledger)?;
    let shared = if parts.contains_key("xl/sharedStrings.xml") {
        let strings = xml_part(&parts, "xl/sharedStrings.xml", "sst", MAIN)?;
        if strings
            .children
            .iter()
            .any(|node| node.name != "si" || c_unknown(node))
        {
            return fail("Unknown shared string XML construct");
        }
        strings.kids("si").map(Xml::content).collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let sheet_nodes = workbook
        .child("sheets")
        .ok_or_else(|| InteropError("Missing worksheets".into()))?
        .kids("sheet")
        .collect::<Vec<_>>();
    if sheet_nodes.is_empty() || sheet_nodes.len() > MAX_SHEETS {
        return fail("Workbook must have 1..=4 sheets");
    }
    let mut sheets = Vec::new();
    let mut worksheet_paths = BTreeSet::new();
    let mut formula_count = 0;
    let mut names = BTreeSet::new();
    for sheet in sheet_nodes {
        let name = sheet
            .attr("name")
            .ok_or_else(|| InteropError("Missing worksheet name".into()))?
            .to_owned();
        if !valid_worksheet_name(&name) || !names.insert(name.to_lowercase()) {
            return fail("Invalid or duplicate worksheet name");
        }
        match sheet.attr("state") {
            None | Some("visible") => {}
            Some("hidden" | "veryHidden") => ledger.push(visibility_finding("hidden_sheet", &name)),
            Some(_) => return fail("Unknown worksheet visibility state"),
        }
        let relationship = rels
            .get(
                sheet
                    .attr("rel:id")
                    .ok_or_else(|| InteropError("Missing sheet relationship".into()))?,
            )
            .ok_or_else(|| InteropError("Unresolved worksheet relationship".into()))?;
        if relationship.attr("TargetMode") == Some("External") {
            return fail("External worksheet is not admitted");
        }
        if relationship.attr("Type")
            != Some("http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet")
        {
            return fail("Workbook sheet relationship is not a worksheet");
        }
        let path = rel_target(
            "xl",
            relationship
                .attr("Target")
                .ok_or_else(|| InteropError("Missing sheet target".into()))?,
        )?;
        let xml = xml_part(&parts, &path, "worksheet", MAIN)?;
        if !worksheet_paths.insert(path.clone()) {
            return fail("Duplicate worksheet part mapping");
        }
        for singleton in ["sheetData", "dimension", "sheetFormatPr"] {
            if xml.kids(singleton).count() > 1 {
                return fail("Duplicate singleton worksheet construct");
            }
        }
        if let Some(format) = xml.child("sheetFormatPr") {
            inventory_hidden(
                format.attr("zeroHeight"),
                "hidden_default_rows",
                &name,
                &mut ledger,
            )?;
        }
        inventory_worksheet_children(&xml, &name, &mut ledger);
        let mut rows: Vec<Vec<SourceCell>> = Vec::new();
        let mut seen = BTreeSet::new();
        let mut width = 0;
        let data = xml
            .child("sheetData")
            .ok_or_else(|| InteropError("Missing sheetData".into()))?;
        if data
            .children
            .iter()
            .any(|n| n.name != "row" || n.ns != MAIN)
        {
            return fail("Unknown sheetData construct");
        }
        for row in data.kids("row") {
            inventory_grid_attributes(row, &name, &mut ledger);
            if row.attr("s").is_some_and(|style| style != "0") {
                ledger.push(finding(FidelityCategory::UnsupportedSafeDisabled, "inherited_row_style", &name, "Inherited row style is not applied by this profile; explicit cell styles are required", true));
            }
            inventory_hidden(
                row.attr("hidden"),
                "hidden_row",
                &format!("{name}:row {}", row.attr("r").unwrap_or("?")),
                &mut ledger,
            )?;
            if row.children.iter().any(|n| n.name != "c" || n.ns != MAIN) {
                return fail("Unknown worksheet row construct");
            }
            let row_index = row
                .attr("r")
                .ok_or_else(|| InteropError("Missing worksheet row index".into()))?
                .parse::<usize>()
                .map_err(|_| InteropError("Invalid row index".into()))?;
            if row_index == 0 || row_index > MAX_TEXT_CAPACITY_ROWS + 1 {
                return fail("Worksheet row exceeds 8,406 profile");
            }
            rows.resize_with(rows.len().max(row_index), Vec::new);
            for c in row.kids("c") {
                let address = c
                    .attr("r")
                    .ok_or_else(|| InteropError("Missing cell coordinate".into()))?;
                let (r, col) = coordinate(address)?;
                if r + 1 != row_index || !seen.insert((r, col)) {
                    return fail("Duplicate or inconsistent cell coordinate");
                }
                width = width.max(col + 1);
                let new_width = rows[r].len().max(col + 1);
                rows[r].resize(new_width, SourceCell::default());
                let style = styles
                    .get(
                        c.attr("s")
                            .unwrap_or("0")
                            .parse::<usize>()
                            .map_err(|_| InteropError("Invalid cell style".into()))?,
                    )
                    .ok_or_else(|| InteropError("Cell style index out of bounds".into()))?
                    .clone();
                let location = format!("{name}!{address}");
                let formula = c.child("f");
                let mut value = match cell_value(c, &shared, &style, date1904) {
                    Ok(value) => value,
                    Err(error) if formula.is_some() && ignorable_formula_cache(c, &style) => {
                        ledger.push(finding(FidelityCategory::Converted, "formula_cache_ignored", &location, &format!("Cached formula result is unusable and ignored: {}. Rust must bind and recalculate the source formula", error.0), false));
                        SourceValue::Empty
                    }
                    Err(error) => {
                        ledger.push(finding(
                            FidelityCategory::UnsupportedSafeDisabled,
                            "scalar_mapping_rejected",
                            &location,
                            &error.0,
                            true,
                        ));
                        SourceValue::Empty
                    }
                };
                let mut source = formula.map(|f| f.text.clone());
                if let Some(f) = formula {
                    formula_count += 1;
                    if formula_count > MAX_FORMULAS {
                        return fail("Workbook exceeds 32 formulas");
                    }
                    if f.attr("t").is_some_and(|t| t != "normal") || f.text.is_empty() {
                        ledger.push(finding(FidelityCategory::UnsupportedSafeDisabled,"shared_array_dynamic_formula",&location,"Shared, empty, array or dynamic formula is safe-disabled; cached value is not semantic truth",true));
                        value = SourceValue::Empty;
                    } else if matches!(f.text.to_ascii_uppercase().as_str(), "TRUE()" | "FALSE()") {
                        value = SourceValue::Boolean {
                            value: f.text.eq_ignore_ascii_case("TRUE()"),
                        };
                        source = None;
                        ledger.push(finding(
                            FidelityCategory::Converted,
                            "boolean_constant_formula",
                            &location,
                            "TRUE()/FALSE() converted to Boolean",
                            false,
                        ));
                    } else if f.text.contains('[') || f.text.to_ascii_uppercase().contains("DDE(") {
                        ledger.push(finding(
                            FidelityCategory::UnsupportedSafeDisabled,
                            "external_or_dde_formula",
                            &location,
                            "External/DDE formula is not executed",
                            true,
                        ));
                        value = SourceValue::Empty;
                    } else if unsupported_formula_function(&f.text) {
                        ledger.push(finding(FidelityCategory::UnsupportedSafeDisabled,"unsupported_formula_function",&location,"Only numeric arithmetic and binary MIN/MAX are admitted; Rust still validates binding",true));
                        value = SourceValue::Empty;
                    }
                }
                if matches!(value, SourceValue::Date { .. }) {
                    ledger.push(finding(FidelityCategory::Converted,"gregorian_date",&location,"Date mapped to Gregorian YYYY-MM-DD; source serial identity is adapter-only",false));
                }
                if style
                    .number_format
                    .as_ref()
                    .is_some_and(|s| s.contains(['%', '$']))
                {
                    ledger.push(finding(
                        FidelityCategory::Converted,
                        "number_presentation",
                        &location,
                        "Percentage/currency is Number plus presentation",
                        false,
                    ));
                }
                rows[r][col] = SourceCell {
                    value,
                    formula: source,
                    style,
                };
            }
        }
        if width == 0 {
            return fail("Worksheet has no cells");
        }
        for row in &mut rows {
            row.resize(width, SourceCell::default());
        }
        let header = rows.remove(0);
        let mut capacity_plain = header.iter().all(|cell| cell.style == CellStyle::default());
        let mut headers = BTreeSet::new();
        let mut columns = Vec::new();
        for (i, c) in header.into_iter().enumerate() {
            if c.style != CellStyle::default() {
                ledger.push(finding(FidelityCategory::LossyOnExport,"header_style_not_preserved",&format!("{name}!{}1",column_name(i)),"Header text is retained; source header styling remains in original source bytes",false));
            }
            let title = match c.value {
                SourceValue::Text { value } if !value.is_empty() && c.formula.is_none() => value,
                _ => {
                    ledger.push(finding(
                        FidelityCategory::UnsupportedSafeDisabled,
                        "invalid_header",
                        &format!("{name}!{}1", column_name(i)),
                        "XLSX profile requires a nonempty Text, nonformula header",
                        true,
                    ));
                    format!("Column {}", i + 1)
                }
            };
            if !headers.insert(title.clone()) {
                ledger.push(finding(
                    FidelityCategory::UnsupportedSafeDisabled,
                    "duplicate_header",
                    &name,
                    "Duplicate worksheet header",
                    true,
                ));
            }
            columns.push(SourceColumn {
                name: title,
                width: None,
            });
        }
        for cols in xml.kids("cols") {
            if cols
                .children
                .iter()
                .any(|node| node.name != "col" || node.ns != MAIN)
            {
                return fail("Unknown worksheet column construct");
            }
            for c in cols.kids("col") {
                inventory_grid_attributes(c, &name, &mut ledger);
                if c.attr("style").is_some_and(|style| style != "0") {
                    ledger.push(finding(FidelityCategory::UnsupportedSafeDisabled, "inherited_column_style", &name, "Inherited column style is not applied by this profile; explicit cell styles are required", true));
                }
                inventory_hidden(
                    c.attr("hidden"),
                    "hidden_column",
                    &format!(
                        "{name}:columns {}..{}",
                        c.attr("min").unwrap_or("1"),
                        c.attr("max").unwrap_or("1")
                    ),
                    &mut ledger,
                )?;
                let min = c
                    .attr("min")
                    .unwrap_or("1")
                    .parse::<usize>()
                    .map_err(|_| InteropError("Invalid column width range".into()))?;
                let max = c
                    .attr("max")
                    .unwrap_or("1")
                    .parse::<usize>()
                    .map_err(|_| InteropError("Invalid column width range".into()))?;
                let w = c
                    .attr("width")
                    .map(str::parse::<f64>)
                    .transpose()
                    .map_err(|_| InteropError("Invalid column width".into()))?;
                capacity_plain &= w.is_none();
                if min == 0
                    || max < min
                    || max > MAX_COLUMNS
                    || w.is_some_and(|w| !w.is_finite() || w <= 0.0 || w > 255.0)
                {
                    return fail("Invalid column width");
                }
                if max > columns.len() && w.is_some() {
                    ledger.push(finding(FidelityCategory::LossyOnExport, "column_width_outside_grid", &format!("{name}:columns {min}..{max}"), "Column width outside the represented cell grid remains source-only and is omitted on export", false));
                }
                for c in columns.iter_mut().take(max).skip(min - 1) {
                    c.width = w;
                }
            }
        }
        if rows.len() > MAX_DATA_ROWS && !capacity_plain {
            return fail(
                "8,406-row capacity requires default source headers and no column widths, including outside the grid",
            );
        }
        sheets.push(SourceSheet {
            name,
            has_header: true,
            columns,
            rows,
        });
    }
    root_inventory(&parts, &worksheet_paths, &mut ledger)?;
    let plain = capacity_plain_parts(&parts)?;
    // Parser-derived classification replaces the existing record with a shorter
    // code. Nonplain generic source ledgers and their wire width stay unchanged.
    ledger.push(finding(FidelityCategory::NativeEquivalent,if plain {"plain_workbook"} else {"bounded_workbook"},"source","All bounded worksheets and scalar cells were inspected; formula sources require authoritative Rust binding",false));
    let workbook = SourceWorkbook { sheets, ledger };
    if workbook
        .sheets
        .iter()
        .any(|sheet| sheet.rows.len() > MAX_DATA_ROWS)
    {
        if !text_capacity_shape(&workbook) {
            return fail("XLSX exceeds the bounded Text/Number capacity rectangle");
        }
        // Validate isolated Number-feasibility hypotheses without changing
        // the typed values the user sees and explicitly selects.
        validate_capacity_source_inspection(&workbook)?;
        return Ok(workbook);
    }
    Ok(finish_source_admission(workbook))
}

/// Every parsed source uses the same representation predicate as output. Existing
/// blockers retain their original inventory and are never replaced by a generic error.
fn finish_source_admission(mut workbook: SourceWorkbook) -> SourceWorkbook {
    if !workbook.ledger.iter().any(|finding| finding.blocking) {
        if let Err(error) = validate_output(&workbook) {
            workbook.ledger.push(finding(
                FidelityCategory::UnsupportedSafeDisabled,
                "output_profile_rejected",
                "workbook",
                &format!(
                    "Source cannot be represented by the shared output profile: {}",
                    error.0
                ),
                true,
            ));
        }
    }
    workbook
}
/// Emit the selected sheet values. Caller discloses formula/format/sheet losses.
/// # Errors
/// Rejects an oversized or invalid scalar output profile.
pub fn export_csv(sheet: &SourceSheet) -> Result<Vec<u8>> {
    let profile = if sheet.rows.len() > MAX_DATA_ROWS && text_capacity_sheet_shape(sheet) {
        OutputProfile::TextCapacity
    } else {
        OutputProfile::Shared
    };
    export_csv_for_profile(sheet, profile)
}

/// Emit the selected sheet values using one explicit bounded output profile.
/// # Errors
/// Rejects an oversized or invalid scalar output profile.
pub(crate) fn export_csv_for_profile(
    sheet: &SourceSheet,
    profile: OutputProfile,
) -> Result<Vec<u8>> {
    if profile == OutputProfile::NativeTables {
        validate_native_tables_csv(sheet).map_err(|finding| {
            InteropError(format!(
                "{} at {}: {}",
                finding.code, finding.location, finding.message
            ))
        })?;
    }
    if profile == OutputProfile::TextCapacity {
        validate_text_capacity(&SourceWorkbook {
            sheets: vec![sheet.clone()],
            ledger: Vec::new(),
        })?;
    }
    if sheet.columns.is_empty()
        || sheet.columns.len() > profile.max_columns()
        || sheet.rows.len() > profile.max_data_rows()
        || sheet.rows.iter().any(|r| r.len() != sheet.columns.len())
    {
        return fail("CSV output exceeds rectangular profile");
    }
    if sheet
        .rows
        .iter()
        .flatten()
        .any(|c| matches!(c.value,SourceValue::Number{value} if !value.is_finite()))
    {
        return fail("CSV cannot export a nonfinite Number");
    }
    // RFC quoting does not prevent spreadsheet formula activation. Preserve
    // canonical Text by refusing ambiguous CSV output; typed XLSX remains safe.
    if (sheet.has_header && sheet.columns.iter().any(|c| csv_formula_text(&c.name)))
        || sheet
            .rows
            .iter()
            .flatten()
            .any(|c| matches!(&c.value, SourceValue::Text { value } if csv_formula_text(value)))
    {
        return fail(
            "CSV text/header could be interpreted as a spreadsheet formula; export typed XLSX to preserve literal Text",
        );
    }
    let mut output = String::new();
    let mut rows = Vec::new();
    if sheet.has_header {
        rows.push(
            sheet
                .columns
                .iter()
                .map(|c| c.name.clone())
                .collect::<Vec<_>>(),
        );
    }
    for row in &sheet.rows {
        rows.push(row.iter().map(|c| value_text(&c.value)).collect());
    }
    for row in rows {
        output.push_str(
            &row.iter()
                .map(|s| {
                    if s.contains([',', '"', '\n', '\r']) || s.is_empty() {
                        format!("\"{}\"", s.replace('"', "\"\""))
                    } else {
                        s.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(","),
        );
        output.push_str("\r\n");
    }
    if output.len() > profile.max_output_bytes() {
        return fail(if profile == OutputProfile::NativeTables {
            "Native tables CSV output exceeds 8 MiB"
        } else {
            "CSV output exceeds 2 MiB"
        });
    }
    Ok(output.into_bytes())
}
pub(crate) fn csv_formula_text(value: &str) -> bool {
    value
        .trim_start_matches(|c: char| c.is_whitespace() || c.is_control())
        .starts_with(['=', '+', '-', '@'])
}
fn value_text(value: &SourceValue) -> String {
    match value {
        SourceValue::Empty => String::new(),
        SourceValue::Text { value } | SourceValue::Date { value } => value.clone(),
        SourceValue::Number { value } => value.to_string(),
        SourceValue::Boolean { value } => value.to_string(),
    }
}
fn unsupported_formula_function(source: &str) -> bool {
    // Inventory function names only, without parsing or evaluating expression semantics.
    let mut token = String::new();
    let mut quoted = false;
    for c in source.chars() {
        if c == '\'' {
            quoted = !quoted;
            token.clear();
            continue;
        }
        if quoted {
            continue;
        }
        if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
            token.push(c);
        } else if c == '(' {
            if !token.is_empty()
                && !matches!(
                    token.to_ascii_uppercase().as_str(),
                    "MIN" | "MAX" | "TRUE" | "FALSE"
                )
            {
                return true;
            }
            token.clear();
        } else if !c.is_whitespace() {
            token.clear();
        }
    }
    false
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
        .replace('\r', "&#13;")
}

fn capacity_attribute(value: &str) -> String {
    escape(value).replace('\n', "&#10;").replace('\t', "&#9;")
}
fn column_name(index: usize) -> String {
    let mut n = index + 1;
    let mut s = String::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        s.insert(
            0,
            char::from(b'A' + u8::try_from(rem).expect("bounded column")),
        );
        n = (n - 1) / 26;
    }
    s
}
pub(crate) fn valid_worksheet_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= 31
        && !name.contains(['[', ']', ':', '*', '?', '/', '\\'])
        && xml_text_valid(name)
}
pub(crate) fn validate_output(workbook: &SourceWorkbook) -> Result<()> {
    validate_output_for_profile(workbook, output_profile(workbook))
}

fn validate_output_collection_bounds(
    workbook: &SourceWorkbook,
    profile: OutputProfile,
) -> Result<()> {
    if workbook.sheets.is_empty() || workbook.sheets.len() > profile.max_sheets() {
        return fail(if profile == OutputProfile::NativeTables {
            "Native tables export requires 1..=32 sheets"
        } else {
            "Export requires 1..=4 sheets"
        });
    }
    if profile == OutputProfile::NativeTables
        && workbook
            .sheets
            .iter()
            .map(|sheet| sheet.rows.len())
            .sum::<usize>()
            > MAX_NATIVE_TABLES_TOTAL_ROWS
    {
        return fail("Native tables export exceeds 1,024 total rows");
    }
    Ok(())
}

fn output_headers_unique(columns: &[SourceColumn], profile: OutputProfile) -> bool {
    if profile == OutputProfile::NativeTables {
        columns
            .iter()
            .map(|c| native_tables_caseless_key(&c.name))
            .collect::<BTreeSet<_>>()
            .len()
            == columns.len()
    } else {
        columns
            .iter()
            .map(|c| &c.name)
            .collect::<BTreeSet<_>>()
            .len()
            == columns.len()
    }
}

pub(crate) fn validate_output_for_profile(
    workbook: &SourceWorkbook,
    profile: OutputProfile,
) -> Result<()> {
    if profile == OutputProfile::TextCapacity {
        validate_text_capacity(workbook)?;
    }
    validate_output_collection_bounds(workbook, profile)?;
    if workbook.ledger.iter().any(|f| f.blocking) {
        return fail("Blocking fidelity findings prevent export");
    }
    let mut names = BTreeSet::new();
    let mut formulas = 0;
    for s in &workbook.sheets {
        if !xml_text_valid(&s.name) || s.columns.iter().any(|c| !xml_text_valid(&c.name)) {
            return fail("Invalid XML character in sheet or header");
        }
        if s.has_header
            && (s.columns.iter().any(|c| c.name.is_empty())
                || !output_headers_unique(&s.columns, profile))
        {
            return fail("XLSX export headers must be nonempty and unique");
        }
        let name_key = if profile == OutputProfile::NativeTables {
            native_tables_caseless_key(&s.name)
        } else {
            s.name.to_lowercase()
        };
        if !valid_worksheet_name(&s.name) || !names.insert(name_key) {
            return fail("Invalid or duplicate XLSX sheet name");
        }
        if s.columns.is_empty()
            || s.columns.len() > profile.max_columns()
            || s.rows.len() > profile.max_data_rows()
        {
            return fail("Export exceeds sheet bounds");
        }
        for row in &s.rows {
            if row.len() != s.columns.len() {
                return fail("Export rows must be rectangular");
            }
            for c in row {
                let format = checked_number_format(&c.style)?;
                if c.formula.is_some() && format != NumberFormatKind::Number {
                    return fail("Formula requires a uniform numeric number format");
                }
                if matches!(c.value, SourceValue::Number { .. })
                    && format != NumberFormatKind::Number
                {
                    return fail("Number requires a uniform numeric number format");
                }
                if !xml_text_valid(&value_text(&c.value))
                    || c.formula.as_ref().is_some_and(|s| !xml_text_valid(s))
                    || c.style
                        .number_format
                        .as_ref()
                        .is_some_and(|s| !xml_text_valid(s))
                {
                    return fail("Invalid XML character in cell");
                }
                if c.style
                    .alignment
                    .as_ref()
                    .is_some_and(|s| !["left", "center", "right", "general"].contains(&s.as_str()))
                {
                    return fail("Unsupported cell alignment");
                }
                if matches!(c.value,SourceValue::Number{value} if !value.is_finite()) {
                    return fail("Nonfinite export Number");
                }
                if let SourceValue::Date { value } = &c.value {
                    tachiko_workspace_engine::Date::parse(value)
                        .map_err(|e| InteropError(e.to_string()))?;
                }
                if let Some(formula) = &c.formula {
                    formulas += 1;
                    if formula.contains('[')
                        || formula.to_ascii_uppercase().contains("DDE(")
                        || unsupported_formula_function(formula)
                    {
                        return fail("Unsafe or unsupported export formula");
                    }
                }
                if let Some(fill) = &c.style.fill {
                    if ![6, 8].contains(&fill.len()) || !fill.bytes().all(|b| b.is_ascii_hexdigit())
                    {
                        return fail("Invalid RGB fill");
                    }
                }
            }
        }
    }
    if formulas > MAX_FORMULAS {
        return fail("Export exceeds formula bound");
    }
    Ok(())
}
fn xml_text_valid(text: &str) -> bool {
    !text.chars().any(|c| {
        (c < ' ' && !matches!(c, '\t' | '\n' | '\r')) || matches!(c, '\u{fffe}' | '\u{ffff}')
    })
}
fn output_style(cell: &SourceCell) -> CellStyle {
    let mut style = cell.style.clone();
    if matches!(cell.value, SourceValue::Date { .. }) && style.number_format.is_none() {
        style.number_format = Some("yyyy-mm-dd".into());
    }
    style
}
/// Emit typed worksheets using caller-projected A1 formulas without evaluation.
/// # Errors
/// Rejects blocking findings, invalid output values/styles and bounded size limits.
#[allow(clippy::too_many_lines, clippy::format_push_string)] // Bounded deterministic XML assembly; temporary strings stay within the tiny workbook profile.
pub fn export_xlsx(workbook: &SourceWorkbook) -> Result<Vec<u8>> {
    export_xlsx_for_profile(
        workbook,
        if use_text_capacity_writer(workbook) {
            OutputProfile::TextCapacity
        } else {
            OutputProfile::Shared
        },
    )
}

fn use_text_capacity_writer(workbook: &SourceWorkbook) -> bool {
    if validate_text_capacity(workbook).is_err() {
        return false;
    }
    let sheet = &workbook.sheets[0];
    sheet.rows.len() > MAX_DATA_ROWS
        || (sheet.columns.len() == 3
            && sheet
                .rows
                .iter()
                .flatten()
                .all(|cell| matches!(cell.value, SourceValue::Empty | SourceValue::Text { .. })))
}

/// Emit typed worksheets using the selected bounded output profile.
/// # Errors
/// Rejects blocking findings, invalid output values/styles and bounded size limits.
#[allow(clippy::too_many_lines, clippy::format_push_string)] // Bounded deterministic XML assembly; temporary strings stay within the tiny workbook profile.
pub(crate) fn export_xlsx_for_profile(
    workbook: &SourceWorkbook,
    profile: OutputProfile,
) -> Result<Vec<u8>> {
    if profile == OutputProfile::NativeTables {
        validate_native_tables_xlsx(workbook).map_err(|finding| {
            InteropError(format!(
                "{} at {}: {}",
                finding.code, finding.location, finding.message
            ))
        })?;
    }
    validate_output_for_profile(workbook, profile)?;
    let mut style_keys = vec![
        serde_json::to_string(&CellStyle::default()).map_err(|e| InteropError(e.to_string()))?,
    ];
    let mut styles = vec![CellStyle::default()];
    for s in workbook
        .sheets
        .iter()
        .filter(|_| profile != OutputProfile::TextCapacity)
    {
        for row in &s.rows {
            for c in row {
                let style = output_style(c);
                let key = serde_json::to_string(&style).map_err(|e| InteropError(e.to_string()))?;
                if !style_keys.contains(&key) {
                    style_keys.push(key);
                    styles.push(style);
                }
            }
        }
    }
    let mut fonts = String::new();
    let mut fills = String::from(
        "<fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill>",
    );
    let mut borders = String::new();
    let mut fmts = String::new();
    let mut xfs = String::new();
    for (i, style) in styles.iter().enumerate() {
        fonts.push_str(&format!(
            "<font>{}<sz val=\"11\"/><name val=\"Arial\"/></font>",
            if style.bold { "<b/>" } else { "" }
        ));
        fills.push_str(&style.fill.as_ref().map_or_else(||"<fill><patternFill patternType=\"none\"/></fill>".into(),|rgb|format!("<fill><patternFill patternType=\"solid\"><fgColor rgb=\"{}\"/><bgColor indexed=\"64\"/></patternFill></fill>",if rgb.len()==6{format!("FF{rgb}")}else{rgb.clone()})));
        borders.push_str(if style.border{"<border><left style=\"thin\"/><right style=\"thin\"/><top style=\"thin\"/><bottom style=\"thin\"/><diagonal/></border>"}else{"<border><left/><right/><top/><bottom/><diagonal/></border>"});
        let fmt = style.number_format.as_deref().unwrap_or("General");
        fmts.push_str(&format!(
            "<numFmt numFmtId=\"{}\" formatCode=\"{}\"/>",
            164 + i,
            escape(fmt)
        ));
        xfs.push_str(&format!("<xf numFmtId=\"{}\" fontId=\"{i}\" fillId=\"{}\" borderId=\"{i}\" xfId=\"0\" applyNumberFormat=\"1\" applyFont=\"1\" applyFill=\"1\" applyBorder=\"1\" applyAlignment=\"1\"><alignment horizontal=\"{}\" wrapText=\"{}\"/></xf>",164+i,i+2,escape(style.alignment.as_deref().unwrap_or("general")),u8::from(style.wrap)));
    }
    let style_xml = format!(
        "<styleSheet xmlns=\"{MAIN}\"><numFmts count=\"{}\">{fmts}</numFmts><fonts count=\"{}\">{fonts}</fonts><fills count=\"{}\">{fills}</fills><borders count=\"{}\">{borders}</borders><cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs><cellXfs count=\"{}\">{xfs}</cellXfs></styleSheet>",
        styles.len(),
        styles.len(),
        styles.len() + 2,
        styles.len(),
        styles.len()
    );
    let mut parts = BTreeMap::new();
    parts.insert("xl/styles.xml".into(), style_xml);
    if profile == OutputProfile::TextCapacity {
        parts.insert("xl/styles.xml".into(), capacity_styles_xml());
        let mut headers = String::new();
        for column in &workbook.sheets[0].columns {
            headers.push_str("<si><t xml:space=\"preserve\">");
            headers.push_str(&escape(&column.name));
            headers.push_str("</t></si>");
        }
        parts.insert(
            "xl/sharedStrings.xml".into(),
            format!(
                "<sst xmlns=\"{MAIN}\" count=\"{}\" uniqueCount=\"{}\">{headers}</sst>",
                workbook.sheets[0].columns.len(),
                workbook.sheets[0].columns.len()
            ),
        );
    }
    let mut sheet_list = String::new();
    let mut sheet_rels = String::new();
    let mut types = String::new();
    for (index, s) in workbook.sheets.iter().enumerate() {
        let n = index + 1;
        sheet_list.push_str(&format!(
            "<sheet name=\"{}\" sheetId=\"{n}\" r:id=\"rId{n}\"/>",
            if profile == OutputProfile::TextCapacity {
                capacity_attribute(&s.name)
            } else {
                escape(&s.name)
            }
        ));
        sheet_rels.push_str(&format!("<Relationship Id=\"rId{n}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet{n}.xml\"/>"));
        types.push_str(&format!("<Override PartName=\"/xl/worksheets/sheet{n}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"));
        if profile == OutputProfile::TextCapacity {
            let mut xml = String::new();
            capacity_worksheet(s, &mut xml);
            parts.insert(format!("xl/worksheets/sheet{n}.xml"), xml);
            continue;
        }
        let mut xml = format!("<worksheet xmlns=\"{MAIN}\">");
        // CT_Cols contains one or more CT_Col entries. An empty <cols>
        // container is rejected by Excel's package reader, even though the
        // surrounding ZIP and XML remain structurally readable.
        if s.columns.iter().any(|column| column.width.is_some()) {
            xml.push_str("<cols>");
            for (col, c) in s.columns.iter().enumerate() {
                if let Some(width) = c.width {
                    if !width.is_finite() || width <= 0.0 || width > 255.0 {
                        return fail("Invalid export column width");
                    }
                    xml.push_str(&format!(
                        "<col min=\"{}\" max=\"{}\" width=\"{width}\" customWidth=\"1\"/>",
                        col + 1,
                        col + 1
                    ));
                }
            }
            xml.push_str("</cols>");
        }
        xml.push_str("<sheetData>");
        let header = s
            .columns
            .iter()
            .map(|c| SourceCell {
                value: SourceValue::Text {
                    value: c.name.clone(),
                },
                ..SourceCell::default()
            })
            .collect::<Vec<_>>();
        let rows = if s.has_header {
            std::iter::once(&header)
                .chain(s.rows.iter())
                .collect::<Vec<_>>()
        } else {
            s.rows.iter().collect::<Vec<_>>()
        };
        for (r, row) in rows.iter().enumerate() {
            xml.push_str(&format!("<row r=\"{}\">", r + 1));
            for (col, c) in row.iter().enumerate() {
                let address = format!("{}{}", column_name(col), r + 1);
                let sid = if profile == OutputProfile::TextCapacity {
                    0
                } else {
                    let key = serde_json::to_string(&output_style(c))
                        .map_err(|e| InteropError(e.to_string()))?;
                    style_keys.iter().position(|x| x == &key).unwrap_or(0)
                };
                let formula = c
                    .formula
                    .as_ref()
                    .map(|f| format!("<f>{}</f>", escape(f.trim_start_matches('='))))
                    .unwrap_or_default();
                let (kind, body) = if profile == OutputProfile::TextCapacity && r == 0 {
                    ("s", format!("<v>{col}</v>"))
                } else {
                    match &c.value {
                        SourceValue::Empty => ("n", formula.clone()),
                        SourceValue::Text { value } => (
                            "inlineStr",
                            format!(
                                "{formula}<is><t xml:space=\"preserve\">{}</t></is>",
                                escape(value)
                            ),
                        ),
                        SourceValue::Number { value }
                            if profile == OutputProfile::NativeTables && *value == 0.0 =>
                        {
                            ("n", format!("{formula}<v>0</v>"))
                        }
                        SourceValue::Number { value } => ("n", format!("{formula}<v>{value}</v>")),
                        SourceValue::Boolean { value } => {
                            ("b", format!("{formula}<v>{}</v>", u8::from(*value)))
                        }
                        SourceValue::Date { value } => {
                            ("d", format!("{formula}<v>{}</v>", escape(value)))
                        }
                    }
                };
                xml.push_str(&format!(
                    "<c r=\"{address}\" s=\"{sid}\" t=\"{kind}\">{body}</c>"
                ));
            }
            xml.push_str("</row>");
        }
        xml.push_str("</sheetData></worksheet>");
        parts.insert(format!("xl/worksheets/sheet{n}.xml"), xml);
    }
    let calculation = if profile == OutputProfile::TextCapacity {
        ""
    } else {
        "<calcPr calcMode=\"auto\" fullCalcOnLoad=\"1\"/>"
    };
    parts.insert("xl/workbook.xml".into(),format!("<workbook xmlns=\"{MAIN}\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><workbookPr date1904=\"0\"/><sheets>{sheet_list}</sheets>{calculation}</workbook>"));
    let strings_relationship = if profile == OutputProfile::TextCapacity {
        "<Relationship Id=\"strings\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings\" Target=\"sharedStrings.xml\"/>"
    } else {
        ""
    };
    parts.insert("xl/_rels/workbook.xml.rels".into(),format!("<Relationships xmlns=\"{REL}\">{sheet_rels}{strings_relationship}<Relationship Id=\"styles\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/></Relationships>"));
    parts.insert("_rels/.rels".into(),format!("<Relationships xmlns=\"{REL}\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>"));
    let strings_type = if profile == OutputProfile::TextCapacity {
        "<Override PartName=\"/xl/sharedStrings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml\"/>"
    } else {
        ""
    };
    parts.insert("[Content_Types].xml".into(),format!("<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>{strings_type}{types}</Types>"));
    let expanded_limit = if profile == OutputProfile::NativeTables {
        MAX_NATIVE_TABLES_EXPANDED_BYTES
    } else {
        MAX_EXPANDED_BYTES
    };
    if matches!(
        profile,
        OutputProfile::TextCapacity | OutputProfile::NativeTables
    ) && parts.values().map(String::len).sum::<usize>() > expanded_limit
    {
        return fail(if profile == OutputProfile::NativeTables {
            "Native tables expanded XLSX exceeds 24 MiB"
        } else {
            "Text capacity expanded XLSX exceeds 8 MiB"
        });
    }
    let mut output = Cursor::new(Vec::new());
    {
        let mut writer = ZipWriter::new(&mut output);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, xml) in parts {
            let options = if profile == OutputProfile::TextCapacity
                && matches!(name.as_str(), "xl/workbook.xml" | "xl/sharedStrings.xml")
            {
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored)
            } else {
                options
            };
            writer
                .start_file(name, options)
                .map_err(|e| InteropError(e.to_string()))?;
            writer
                .write_all(xml.as_bytes())
                .map_err(|e| InteropError(e.to_string()))?;
        }
        writer.finish().map_err(|e| InteropError(e.to_string()))?;
    }
    let bytes = output.into_inner();
    if profile == OutputProfile::NativeTables {
        if bytes.len() > profile.max_output_bytes() {
            return fail("Native tables XLSX output exceeds 8 MiB");
        }
    } else {
        source_bound(&bytes)?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod scalar_carrier_tests {
    use super::{CellStyle, MAIN, Xml, c_unknown, cell_value, ignorable_formula_cache, parse_xml};

    fn cell(kind: &str, carrier: &str, formula: &str) -> Xml {
        let xml = format!("<c xmlns=\"{MAIN}\"{kind}>{formula}{carrier}</c>");
        parse_xml(xml.as_bytes()).unwrap()
    }

    #[test]
    fn mismatched_carriers_cannot_reach_defaults_or_formula_cache_recovery() {
        for (kind, carrier) in [
            ("", "<is><t>value</t></is>"),
            (" t=\"n\"", "<is><t>value</t></is>"),
            (" t=\"str\"", "<is><t>value</t></is>"),
            (" t=\"b\"", "<is><t>1</t></is>"),
            (" t=\"s\"", "<is><t>0</t></is>"),
            (" t=\"e\"", "<is><t>#VALUE!</t></is>"),
            (" t=\"d\"", "<is><t>2026-10-10</t></is>"),
            (" t=\"inlineStr\"", "<v>123</v>"),
        ] {
            for formula in ["", "<f>1+2</f>"] {
                let cell = cell(kind, carrier, formula);
                assert!(c_unknown(&cell), "{kind} {carrier} {formula}");
                assert!(cell_value(&cell, &[], &CellStyle::default(), false).is_err());
                assert!(!ignorable_formula_cache(&cell, &CellStyle::default()));
            }
        }
    }

    #[test]
    fn compatible_and_absent_carriers_keep_formula_cache_eligibility() {
        for (kind, carrier) in [
            ("", ""),
            (" t=\"n\"", "<v/>"),
            (" t=\"n\"", "<v>3</v>"),
            (" t=\"inlineStr\"", ""),
            (" t=\"inlineStr\"", "<is><t>3</t></is>"),
            (" t=\"e\"", "<v>#VALUE!</v>"),
            (" t=\"str\"", "<v>cache</v>"),
        ] {
            let cell = cell(kind, carrier, "<f>1+2</f>");
            assert!(!c_unknown(&cell), "{kind} {carrier}");
            assert!(ignorable_formula_cache(&cell, &CellStyle::default()));
        }
    }
}

#[cfg(test)]
mod capacity_proof_tests {
    use std::fmt::Write;

    use super::{
        CapacityXmlCount, ImportOptions, SourceColumn, SourceValue, capacity_auxiliary_xml_reserve,
        capacity_header_bytes, capacity_header_limit, capacity_worksheet, export_xlsx, import_csv,
        import_xlsx, plain_tag, st_xstring, validate_text_capacity,
    };

    #[test]
    fn raw_csv_inspection_defers_capacity_output_until_explicit_number_selection() {
        use crate::{ImportFieldType, ImportSelection, StoredValueProjection, import_workbook};

        assert_eq!(capacity_header_limit(2), 39);
        assert_eq!(capacity_header_limit(16), 208);
        assert_eq!(capacity_auxiliary_xml_reserve(16), 1_434);
        let wide_headers = super::SourceSheet {
            name: "Capacity".into(),
            has_header: true,
            columns: (0..16)
                .map(|index| SourceColumn {
                    name: format!("h{index}"),
                    width: None,
                })
                .collect(),
            rows: Vec::new(),
        };
        assert_eq!(capacity_header_bytes(&wide_headers), 55);
        let mut csv = String::from("label,amount\n");
        for row in 0..65 {
            writeln!(csv, "row{row},{}", if row == 0 { "-13.25" } else { "42.5" }).unwrap();
        }
        let source = import_csv(csv.as_bytes(), &ImportOptions::default()).unwrap();
        assert!(super::text_capacity_shape(&source));
        assert_eq!(
            source.sheets[0].rows[0][1].value,
            SourceValue::Text {
                value: "-13.25".into()
            }
        );
        assert!(validate_text_capacity(&source).is_err());

        let selection = ImportSelection {
            column_types: vec![vec![ImportFieldType::Text, ImportFieldType::Number]],
            extra_columns: vec![Vec::new()],
        };
        let (runtime, imported) =
            import_workbook(&source, &selection, "00000000-0000-4000-8000-000000000507").unwrap();
        assert_eq!(imported.opened.table.columns[1].field_type, "number");
        assert_eq!(
            imported.opened.table.rows[0].fields[1].stored,
            Some(StoredValueProjection::Number { value: -13.25 })
        );
        let workbook = runtime
            .export_workbook(&runtime.observe_occurrence().revision, &imported.metadata)
            .unwrap();
        let reopened = import_xlsx(&export_xlsx(&workbook).unwrap()).unwrap();
        assert_eq!(
            reopened.sheets[0].rows[0][1].value,
            SourceValue::Number { value: -13.25 }
        );

        let text_selection = ImportSelection {
            column_types: vec![vec![ImportFieldType::Text; 2]],
            extra_columns: vec![Vec::new()],
        };
        assert!(
            import_workbook(
                &source,
                &text_selection,
                "00000000-0000-4000-8000-000000000508"
            )
            .is_err()
        );
    }

    #[test]
    fn unsafe_capacity_text_requires_a_finite_number_for_the_whole_column() {
        for invalid in ["-1+2", "inf", "not-a-number"] {
            let mut csv = String::from("label,amount\nrow0,-1\n");
            writeln!(csv, "row1,{invalid}\nrow2,2").unwrap();
            for _ in 0..62 {
                writeln!(csv, "row,2").unwrap();
            }
            assert!(import_csv(csv.as_bytes(), &ImportOptions::default()).is_err());
        }
    }

    #[test]
    fn raw_token_grammar_and_shared_writer_count_agree() {
        for (raw, expected) in [
            ("a p=\"x\"q=\"y\"", false),
            ("a p=\"x<y\"", false),
            ("1:a", false),
            ("a:b:c", false),
            ("a p=\"x&amp;y\"", true),
            ("a\tp = 'x' q = \"y\" ", true),
        ] {
            assert_eq!(plain_tag(raw.as_bytes(), true), expected, "{raw}");
        }
        for (text, expected) in [
            ("_x000D_", true),
            ("_x00af_", true),
            ("_X000D_", false),
            ("_x000G_", false),
        ] {
            assert_eq!(st_xstring(text), expected);
        }
        let mut workbook =
            import_csv(b"a,b,c\none,two,three\n", &ImportOptions::default()).unwrap();
        workbook.sheets[0].rows[0][0].value = SourceValue::Text {
            value: "&<>\"'\r\n雪".into(),
        };
        workbook.sheets[0].rows[0][1].value = SourceValue::Empty;
        for value in ["plain雪\t\n", "&", "<", ">", "\"", "'", "\r"] {
            workbook.sheets[0].rows[0][0].value = SourceValue::Text {
                value: value.into(),
            };
            let reopened = import_xlsx(&export_xlsx(&workbook).unwrap()).unwrap();
            assert_eq!(
                reopened.sheets[0].rows[0][0].value,
                workbook.sheets[0].rows[0][0].value
            );
        }
        workbook.sheets[0].rows[0][0].value = SourceValue::Text {
            value: "&<>\"'\r\n雪".into(),
        };
        let row = workbook.sheets[0].rows[0].clone();
        for rows in [0, 8, 9, 98, 99, 998, 999, 8406] {
            workbook.sheets[0].rows = vec![row.clone(); rows];
            let mut output = String::new();
            let mut count = CapacityXmlCount(0);
            capacity_worksheet(&workbook.sheets[0], &mut output);
            capacity_worksheet(&workbook.sheets[0], &mut count);
            assert_eq!(count.0, output.len(), "{rows}");
        }
    }
}

#[cfg(test)]
mod native_tables_output_tests {
    use super::{
        FidelityCategory, MAX_NATIVE_TABLES_EXPANDED_BYTES, MAX_NATIVE_TABLES_OUTPUT_BYTES,
        NativeTablesCheck, OutputProfile, SourceCell, SourceColumn, SourceSheet, SourceValue,
        SourceWorkbook, date_from_serial, export_csv_for_profile, export_xlsx_for_profile,
        import_xlsx, native_tables_caseless_key, native_tables_compatibility,
        native_tables_trim_worksheet_name, validate_native_tables_csv, validate_native_tables_xlsx,
        validate_output_for_profile,
    };
    use std::collections::BTreeSet;
    use std::io::{Cursor, Read, Write};
    use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

    fn sheet(columns: usize, rows: usize) -> SourceSheet {
        SourceSheet {
            name: "Table".into(),
            has_header: true,
            columns: (0..columns)
                .map(|index| SourceColumn {
                    name: format!("c{index}"),
                    width: None,
                })
                .collect(),
            rows: vec![vec![SourceCell::default(); columns]; rows],
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)] // Table/fixture enumerates the complete native compatibility inventory.
    fn native_tables_compatibility_name_inventory_boundaries() {
        // Source labels are allocated; the very same policy validates emitted names.
        let cases = [
            ("minimum", "A".into(), "A".into()),
            ("blank", String::new(), "Table".into()),
            ("whitespace", "  ".into(), "Table".into()),
            ("31 ASCII units", "a".repeat(31), "a".repeat(31)),
            ("32 ASCII units", "a".repeat(32), "a".repeat(31)),
            ("31 BMP units", "漢".repeat(31), "漢".repeat(31)),
            ("32 BMP units", "漢".repeat(32), "漢".repeat(31)),
            (
                "31 astral units",
                format!("{}a", "😀".repeat(15)),
                format!("{}a", "😀".repeat(15)),
            ),
            ("32 astral units", "😀".repeat(16), "😀".repeat(15)),
            ("forbidden", "[]:*?/\\".into(), "_______".into()),
            ("apostrophes", "'a'".into(), "a".into()),
            ("exposed apostrophes", "'  'a'".into(), "a".into()),
            ("interior apostrophe", "a'b".into(), "a'b".into()),
            ("reserved", "History".into(), "History (2)".into()),
            ("reserved caseless", "hIsToRy".into(), "hIsToRy (2)".into()),
        ];
        for (case, source, expected) in cases {
            let mut used = BTreeSet::new();
            let actual = native_tables_compatibility(
                NativeTablesCheck::Name {
                    value: &source,
                    worksheet: true,
                    used: &mut used,
                    allocate: true,
                },
                case,
            )
            .unwrap()
            .name;
            assert_eq!(actual, expected, "{case}");
            assert!(actual.encode_utf16().count() <= 31);
            let mut emitted = BTreeSet::new();
            native_tables_compatibility(
                NativeTablesCheck::Name {
                    value: &actual,
                    worksheet: true,
                    used: &mut emitted,
                    allocate: false,
                },
                case,
            )
            .unwrap();
            if source != actual {
                assert!(
                    native_tables_compatibility(
                        NativeTablesCheck::Name {
                            value: &source,
                            worksheet: true,
                            used: &mut BTreeSet::new(),
                            allocate: false,
                        },
                        case
                    )
                    .is_err(),
                    "{case}"
                );
            }
        }
        for worksheet in [false, true] {
            for group in [
                vec!["σ", "ς", "Σ"],
                vec!["ß", "SS", "ss"],
                vec!["İ", "I", "ı", "i"],
                vec!["a", "A"],
            ] {
                let mut used = BTreeSet::new();
                let mut emitted = BTreeSet::new();
                for source in group {
                    let name = native_tables_compatibility(
                        NativeTablesCheck::Name {
                            value: source,
                            worksheet,
                            used: &mut used,
                            allocate: true,
                        },
                        source,
                    )
                    .unwrap()
                    .name;
                    native_tables_compatibility(
                        NativeTablesCheck::Name {
                            value: &name,
                            worksheet,
                            used: &mut emitted,
                            allocate: false,
                        },
                        source,
                    )
                    .unwrap();
                }
            }
        }
        // Reserved/collision chains, including an already allocated suffix.
        let mut used = BTreeSet::new();
        for (source, expected) in [
            ("History (2)", "History (2)"),
            ("History", "History (3)"),
            ("HISTORY", "HISTORY (4)"),
        ] {
            assert_eq!(
                native_tables_compatibility(
                    NativeTablesCheck::Name {
                        value: source,
                        worksheet: true,
                        used: &mut used,
                        allocate: true,
                    },
                    source
                )
                .unwrap()
                .name,
                expected
            );
        }
        // Reserve suffix units without splitting an astral character.
        let mut used = BTreeSet::new();
        for expected in ["😀".repeat(15), format!("{} (2)", "😀".repeat(13))] {
            assert_eq!(
                native_tables_compatibility(
                    NativeTablesCheck::Name {
                        value: &"😀".repeat(16),
                        worksheet: true,
                        used: &mut used,
                        allocate: true,
                    },
                    "astral collision"
                )
                .unwrap()
                .name,
                expected
            );
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)] // Table/fixture enumerates the complete native compatibility inventory.
    fn native_tables_compatibility_cell_inventory_boundaries() {
        let text = |s: &str| SourceValue::Text { value: s.into() };
        let date = |s: &str| SourceValue::Date { value: s.into() };
        let number = |value| SourceValue::Number { value };
        let below = f64::from_bits(f64::MIN_POSITIVE.to_bits() - 1);
        let cases = [
            ("empty Text", text(""), None),
            ("whitespace", text(" \t\r\n x \t"), None),
            ("XML lowest legal", text("\u{20}"), None),
            (
                "XML below lowest",
                text("\u{1f}"),
                Some("native_tables_xlsx_xml_character"),
            ),
            (
                "XML NUL",
                text("\0"),
                Some("native_tables_xlsx_xml_character"),
            ),
            ("XML highest BMP", text("\u{fffd}"), None),
            (
                "XML above BMP legal",
                text("\u{fffe}"),
                Some("native_tables_xlsx_xml_character"),
            ),
            (
                "XML FFFF",
                text("\u{ffff}"),
                Some("native_tables_xlsx_xml_character"),
            ),
            ("XML maximum scalar", text("\u{10ffff}"), None),
            (
                "escape",
                text("_x000D_"),
                Some("native_tables_xlsx_escape_sequence"),
            ),
            (
                "mixed hex",
                text("_xAbCd_"),
                Some("native_tables_xlsx_escape_sequence"),
            ),
            ("near escape length", text("_x00D_"), None),
            ("near escape digit", text("_x000G_"), None),
            ("normal positive", number(f64::MIN_POSITIVE), None),
            (
                "subnormal positive",
                number(below),
                Some("native_tables_xlsx_subnormal_number"),
            ),
            ("normal negative", number(-f64::MIN_POSITIVE), None),
            (
                "subnormal negative",
                number(-below),
                Some("native_tables_xlsx_subnormal_number"),
            ),
            ("maximum finite", number(f64::MAX), None),
            (
                "positive infinity",
                number(f64::INFINITY),
                Some("native_tables_xlsx_nonfinite_number"),
            ),
            (
                "negative infinity",
                number(f64::NEG_INFINITY),
                Some("native_tables_xlsx_nonfinite_number"),
            ),
            (
                "NaN",
                number(f64::NAN),
                Some("native_tables_xlsx_nonfinite_number"),
            ),
            ("zero", number(0.0), None),
            ("negative zero", number(-0.0), None),
            ("earliest date", date("1900-01-01"), None),
            (
                "date before minimum",
                date("1899-12-31"),
                Some("native_tables_xlsx_date_range"),
            ),
            ("latest date", date("9999-12-31"), None),
            (
                "date after maximum",
                date("10000-01-01"),
                Some("native_tables_xlsx_date_range"),
            ),
            ("before Excel leap bug", date("1900-02-28"), None),
            ("after Excel leap bug", date("1900-03-01"), None),
            (
                "fictitious leap day",
                date("1900-02-29"),
                Some("native_tables_xlsx_date_range"),
            ),
            ("true", SourceValue::Boolean { value: true }, None),
            ("false", SourceValue::Boolean { value: false }, None),
            ("absent optional", SourceValue::Empty, None),
        ];
        for (case, value, code) in cases {
            let result = native_tables_compatibility(
                NativeTablesCheck::Cell(&value),
                "sheet 1/row 2/column 1",
            );
            if let Some(code) = code {
                let finding = result.unwrap_err();
                assert_eq!(finding.code, code, "{case}");
                assert!(finding.blocking);
                assert_eq!(finding.location, "sheet 1/row 2/column 1");
            } else {
                result.unwrap();
            }
        }
        // Every string rule applies to both source worksheet labels and headers.
        for worksheet in [false, true] {
            for (value, code) in [
                ("a\0b", "native_tables_xlsx_xml_character"),
                ("_xAbCd_", "native_tables_xlsx_escape_sequence"),
            ] {
                let finding = native_tables_compatibility(
                    NativeTablesCheck::Name {
                        value,
                        worksheet,
                        used: &mut BTreeSet::new(),
                        allocate: true,
                    },
                    "sheet 1/header/column 1",
                )
                .unwrap_err();
                assert_eq!(finding.code, code);
                assert!(finding.blocking);
            }
        }
    }

    #[test]
    fn native_tables_line_feed_and_header_suffix_boundaries() {
        for count in [253, 254] {
            let value = "\n".repeat(count);
            for header in [false, true] {
                let result = if header {
                    // Keep a nonblank header, while retaining every line feed.
                    native_tables_compatibility(
                        NativeTablesCheck::Name {
                            value: &format!("header{value}"),
                            worksheet: false,
                            used: &mut BTreeSet::new(),
                            allocate: true,
                        },
                        "header/column 1",
                    )
                } else {
                    native_tables_compatibility(
                        NativeTablesCheck::Cell(&SourceValue::Text {
                            value: value.clone(),
                        }),
                        "row 2/column 1",
                    )
                };
                if count == 253 {
                    result.unwrap();
                } else {
                    let finding = result.unwrap_err();
                    assert_eq!(finding.code, "native_tables_xlsx_line_feeds");
                    assert!(finding.blocking);
                }
            }
        }
        for length in [32_763, 32_764] {
            let name = "h".repeat(length);
            let mut used = BTreeSet::new();
            native_tables_compatibility(
                NativeTablesCheck::Name {
                    value: &name,
                    worksheet: false,
                    used: &mut used,
                    allocate: true,
                },
                "header/column 1",
            )
            .unwrap();
            let result = native_tables_compatibility(
                NativeTablesCheck::Name {
                    value: &name,
                    worksheet: false,
                    used: &mut used,
                    allocate: true,
                },
                "header/column 2",
            );
            if length == 32_763 {
                assert_eq!(result.unwrap().name.encode_utf16().count(), 32_767);
            } else {
                assert_eq!(result.unwrap_err().code, "native_tables_xlsx_text_limit");
            }
        }
    }

    #[test]
    fn native_tables_1900_serials_read_back_across_leap_bug() {
        for (date, serial) in [
            ("1900-01-01", 1),
            ("1900-02-28", 59),
            ("1900-03-01", 61),
            ("1900-03-02", 62),
            ("2024-02-29", 45_351),
            ("9999-12-31", 2_958_465),
        ] {
            let mut workbook = SourceWorkbook {
                sheets: vec![sheet(1, 1)],
                ledger: vec![],
            };
            workbook.sheets[0].rows[0][0].value = SourceValue::Date { value: date.into() };
            // Keep the existing typed ISO Date encoding: unlike raw serials,
            // LibreOffice also interprets pre-March-1900 dates correctly here.
            let bytes = export_xlsx_for_profile(&workbook, OutputProfile::NativeTables).unwrap();
            let imported = import_xlsx(&bytes).unwrap();
            assert!(
                !imported.ledger.iter().any(|f| f.blocking),
                "{:?}",
                imported.ledger
            );
            assert_eq!(
                imported.sheets[0].rows[0][0].value,
                workbook.sheets[0].rows[0][0].value
            );
            if date != "9999-12-31" {
                assert_eq!(date_from_serial(f64::from(serial), false).unwrap(), date);
                // Read an equivalent numeric-serial XLSX fixture as well. This
                // proves the 59 -> Feb 28 / 61 -> Mar 1 leap-bug mapping through
                // the crate's actual file importer, not only scalar arithmetic.
                let mut archive = ZipArchive::new(Cursor::new(&bytes)).unwrap();
                let mut numeric = ZipWriter::new(Cursor::new(Vec::new()));
                for i in 0..archive.len() {
                    let mut part = archive.by_index(i).unwrap();
                    let name = part.name().to_owned();
                    let mut data = String::new();
                    part.read_to_string(&mut data).unwrap();
                    if name == "xl/worksheets/sheet1.xml" {
                        assert!(data.contains(&format!("t=\"d\"><v>{date}</v>")));
                        data = data.replace(
                            &format!("t=\"d\"><v>{date}</v>"),
                            &format!("t=\"n\"><v>{serial}</v>"),
                        );
                    }
                    numeric
                        .start_file(name, SimpleFileOptions::default())
                        .unwrap();
                    numeric.write_all(data.as_bytes()).unwrap();
                }
                let imported = import_xlsx(&numeric.finish().unwrap().into_inner()).unwrap();
                assert!(
                    !imported.ledger.iter().any(|f| f.blocking),
                    "{:?}",
                    imported.ledger
                );
                assert_eq!(
                    imported.sheets[0].rows[0][0].value,
                    workbook.sheets[0].rows[0][0].value
                );
            }
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)] // Table/fixture enumerates the complete native compatibility inventory.
    fn native_tables_torture_workbook_for_independent_reader() {
        // One accepted-edge workbook, independent of the smaller resident/import
        // admission limits. Native output still enforces its own full policy.
        let values = vec![
            SourceValue::Text {
                value: "a".repeat(32_767),
            },
            SourceValue::Text {
                value: "漢".repeat(32_767),
            },
            SourceValue::Text {
                value: format!("{}a", "😀".repeat(16_383)),
            },
            SourceValue::Text {
                value: " \t leading\ntrailing \t ".into(),
            },
            SourceValue::Text {
                value: "_x00D_ _x000G_ _X000D_ & < > \" ' 000123".into(),
            },
            SourceValue::Text {
                value: "\u{fffd}\u{10000}\u{10ffff}".into(),
            },
            SourceValue::Number {
                value: f64::MIN_POSITIVE,
            },
            SourceValue::Number {
                value: -f64::MIN_POSITIVE,
            },
            SourceValue::Number { value: -0.0 },
            SourceValue::Number { value: 0.1 },
            SourceValue::Number {
                value: 123_456_789_012_345.0,
            },
            SourceValue::Number {
                value: 1_234_567_890_123_456.0,
            },
            SourceValue::Number {
                value: 0.300_000_000_000_000_04,
            },
            SourceValue::Boolean { value: false },
            SourceValue::Boolean { value: true },
            SourceValue::Date {
                value: "1900-01-01".into(),
            },
            SourceValue::Date {
                value: "1900-02-28".into(),
            },
            SourceValue::Date {
                value: "1900-03-01".into(),
            },
            SourceValue::Date {
                value: "9999-12-31".into(),
            },
            SourceValue::Text {
                value: String::new(),
            },
            SourceValue::Empty,
            SourceValue::Text {
                value: "line\n".repeat(253),
            },
            SourceValue::Number { value: 0.0 },
            SourceValue::Number { value: 1e20 },
            SourceValue::Number { value: 1e-20 },
            SourceValue::Number { value: f64::MAX },
        ];
        let mut workbook = SourceWorkbook {
            sheets: vec![sheet(1, 0)],
            ledger: vec![],
        };
        workbook.sheets[0].name = native_tables_compatibility(
            NativeTablesCheck::Name {
                value: "History",
                worksheet: true,
                used: &mut BTreeSet::new(),
                allocate: true,
            },
            "sheet-name",
        )
        .unwrap()
        .name;
        workbook.sheets[0].columns = sheet(32, 0).columns;
        workbook.sheets[0].columns[0].name = " \tHeader 漢😀 ".into();
        for (index, character) in ['a', '漢', '😀'].into_iter().enumerate() {
            workbook.sheets[0].columns[index + 1].name =
                native_tables_text_with_units(character, 32_767);
        }
        workbook.sheets[0].rows = values
            .into_iter()
            .map(|value| {
                let mut row = vec![SourceCell::default(); 32];
                row[0].value = value;
                row
            })
            .collect();
        let mut names = BTreeSet::new();
        names.insert("history (2)".into());
        for name in [
            "a".repeat(31),
            "漢".repeat(31),
            format!("{}a", "😀".repeat(15)),
            "😀".repeat(16),
            "σ".into(),
            "ς".into(),
            "Σ".into(),
            "ß".into(),
            "SS".into(),
            "ss".into(),
            "İ".into(),
            "I".into(),
            "ı".into(),
            "i".into(),
            "a'b".into(),
            "[]:*?/\\".into(),
            String::new(),
        ] {
            let mut extra = sheet(1, 0);
            extra.name = native_tables_compatibility(
                NativeTablesCheck::Name {
                    value: &name,
                    worksheet: true,
                    used: &mut names,
                    allocate: true,
                },
                "sheet-name",
            )
            .unwrap()
            .name;
            workbook.sheets.push(extra);
        }
        while workbook.sheets.len() < 32 {
            let mut extra = sheet(1, 0);
            extra.name = format!("edge-{}", workbook.sheets.len());
            workbook.sheets.push(extra);
        }
        for sheet in workbook.sheets.iter_mut().take(8) {
            sheet
                .rows
                .resize(128, vec![SourceCell::default(); sheet.columns.len()]);
        }
        let number_findings = super::native_tables_xlsx_number_findings(&workbook);
        workbook.ledger.extend(number_findings);
        let bytes = export_xlsx_for_profile(&workbook, OutputProfile::NativeTables).unwrap();
        let mut archive = ZipArchive::new(Cursor::new(&bytes)).unwrap();
        let mut xml = String::new();
        archive
            .by_name("xl/worksheets/sheet1.xml")
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        assert!(xml.contains("xml:space=\"preserve\""));
        if let Some(path) = std::env::var_os("TACHIKO_NATIVE_TABLES_TORTURE_EVIDENCE") {
            let path = std::path::PathBuf::from(path);
            std::fs::write(&path, bytes).unwrap();
            std::fs::write(
                path.with_extension("json"),
                serde_json::to_vec_pretty(&workbook).unwrap(),
            )
            .unwrap();
        }
    }

    #[test]
    fn native_tables_connector_boundary_trim_loops_until_stable() {
        let mut used = BTreeSet::new();
        for (source, expected) in [
            ("Foo", "Foo"),
            ("' Foo '", "Foo (2)"),
            ("'' Foo ''", "Foo (3)"),
            (" ' Foo ' ", "Foo (4)"),
            (" ' ' '' ", "Table"),
            ("Table", "Table (2)"),
        ] {
            let trimmed = native_tables_trim_worksheet_name(source);
            assert_eq!(native_tables_trim_worksheet_name(trimmed), trimmed);
            let allocated = native_tables_compatibility(
                NativeTablesCheck::Name {
                    value: source,
                    worksheet: true,
                    used: &mut used,
                    allocate: true,
                },
                "sheet-name",
            )
            .unwrap()
            .name;
            assert_eq!(allocated, expected);
            assert_eq!(native_tables_trim_worksheet_name(&allocated), allocated);
        }
        // UTF-16 truncation can expose a space/apostrophe boundary too.
        let source = format!("{} ' suffix", "x".repeat(29));
        assert_eq!(
            native_tables_compatibility(
                NativeTablesCheck::Name {
                    value: &source,
                    worksheet: true,
                    used: &mut BTreeSet::new(),
                    allocate: true,
                },
                "sheet-name"
            )
            .unwrap()
            .name,
            "x".repeat(29)
        );
    }

    #[test]
    fn native_tables_connector_csv_xml_character_findings_cover_cells_and_headers() {
        for character in ['\0', '\u{1}', '\u{1f}', '\u{fffe}', '\u{ffff}'] {
            for header in [false, true] {
                let mut selected = sheet(2, 2);
                let text = format!("bad{character}text");
                let suffix = if header {
                    selected.columns[1].name = text;
                    "/header/column 2"
                } else {
                    selected.rows[1][1].value = SourceValue::Text { value: text };
                    "/row 3/column 2"
                };
                let finding = validate_native_tables_csv(&selected).unwrap_err();
                assert_eq!(finding.code, "native_tables_csv_xml_character");
                assert!(finding.blocking);
                assert_eq!(finding.category, FidelityCategory::UnsupportedSafeDisabled);
                assert_eq!(finding.location, format!("sheet 1 \"Table\"{suffix}"));
                let error =
                    export_csv_for_profile(&selected, OutputProfile::NativeTables).unwrap_err();
                assert!(error.0.contains(&finding.location));
                assert!(export_csv_for_profile(&selected, OutputProfile::Shared).is_ok());
            }
        }
        // XML-legal Text stays lexical in CSV; no XLSX escape/length/LF limits.
        for text in [
            "safe\tline\nnext\rfinal".into(),
            "_x000D_".into(),
            "x".repeat(32_768),
            "line\n".repeat(254),
        ] {
            for header in [false, true] {
                let mut selected = sheet(1, 1);
                if header {
                    selected.columns[0].name = text.clone();
                } else {
                    selected.rows[0][0].value = SourceValue::Text {
                        value: text.clone(),
                    };
                }
                validate_native_tables_csv(&selected).unwrap();
                let bytes = export_csv_for_profile(&selected, OutputProfile::NativeTables).unwrap();
                assert!(String::from_utf8(bytes).unwrap().contains(&text));
            }
        }
    }

    #[test]
    fn native_tables_caseless_key_is_a_bounded_fixed_point_and_idempotent() {
        for (source, expected) in [
            ("ß", "ss"),
            ("ẞ", "ss"),
            ("SS", "ss"),
            ("ss", "ss"),
            ("Straẞe", "strasse"),
            ("σ", "σ"),
            ("ς", "σ"),
            ("Σ", "σ"),
            ("İ", "i\u{307}"),
            ("I", "i"),
            ("ı", "i"),
            ("i", "i"),
            ("Sales", "sales"),
            ("ΟΣ", "ος"),
            ("ΣΟΣ", "σος"),
            ("ﬀ", "ff"),
        ] {
            let key = native_tables_caseless_key(source);
            assert_eq!(key, expected, "{source}");
            assert_eq!(native_tables_caseless_key(&key), key, "{source}");
            assert_eq!(key.to_uppercase().to_lowercase(), key, "{source}");
        }
        // Validate the four-step cap against every scalar in the toolchain's
        // Unicode tables, plus contextual multi-character Greek cases above.
        for scalar in (0..=0x10_ffff).filter_map(char::from_u32) {
            let key = native_tables_caseless_key(&scalar.to_string());
            assert_eq!(native_tables_caseless_key(&key), key, "{scalar:?}");
            assert_eq!(key.to_uppercase().to_lowercase(), key, "{scalar:?}");
        }
    }

    #[test]
    fn native_tables_writer_enforces_caseless_names_without_changing_shared_profile() {
        for (first, second, shared_sheets_accepted) in
            [("σ", "ς", true), ("ß", "ẞ", false), ("ẞ", "ß", false)]
        {
            let mut workbook = SourceWorkbook {
                sheets: vec![sheet(2, 0)],
                ledger: vec![],
            };
            workbook.sheets[0].columns[0].name = first.into();
            workbook.sheets[0].columns[1].name = second.into();
            let finding = validate_native_tables_xlsx(&workbook).unwrap_err();
            assert_eq!(finding.code, "native_tables_xlsx_name");
            assert!(finding.blocking);
            assert!(finding.location.ends_with("/header/column 2"));
            assert!(validate_output_for_profile(&workbook, OutputProfile::NativeTables).is_err());
            assert!(export_xlsx_for_profile(&workbook, OutputProfile::NativeTables).is_err());
            // Other profiles retain their prior header comparison.
            assert!(export_xlsx_for_profile(&workbook, OutputProfile::Shared).is_ok());
            workbook.sheets = vec![sheet(1, 0), sheet(1, 0)];
            workbook.sheets[0].name = first.into();
            workbook.sheets[1].name = second.into();
            let finding = validate_native_tables_xlsx(&workbook).unwrap_err();
            assert_eq!(finding.code, "native_tables_xlsx_name");
            assert!(finding.blocking);
            assert!(finding.location.starts_with("sheet 2 "));
            assert!(finding.location.ends_with("/sheet-name"));
            assert!(validate_output_for_profile(&workbook, OutputProfile::NativeTables).is_err());
            assert!(export_xlsx_for_profile(&workbook, OutputProfile::NativeTables).is_err());
            // Shared sheet names still compare only with lowercase.
            assert_eq!(
                export_xlsx_for_profile(&workbook, OutputProfile::Shared).is_ok(),
                shared_sheets_accepted
            );
        }
    }

    #[test]
    fn native_tables_writer_refuses_subnormal_numbers_and_normalizes_signed_zero_only() {
        let below = f64::from_bits(f64::MIN_POSITIVE.to_bits() - 1);
        for value in [
            f64::MIN_POSITIVE,
            below,
            -f64::MIN_POSITIVE,
            -below,
            0.0,
            -0.0,
        ] {
            let mut workbook = SourceWorkbook {
                sheets: vec![sheet(1, 1)],
                ledger: vec![],
            };
            workbook.sheets[0].rows[0][0].value = SourceValue::Number { value };
            let native = export_xlsx_for_profile(&workbook, OutputProfile::NativeTables);
            if value != 0.0 && value.abs() < f64::MIN_POSITIVE {
                assert!(native.is_err());
                let finding = validate_native_tables_xlsx(&workbook).unwrap_err();
                assert!(finding.blocking);
                assert_eq!(finding.code, "native_tables_xlsx_subnormal_number");
            } else {
                let bytes = native.unwrap();
                if value == 0.0 {
                    let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
                    let mut xml = String::new();
                    archive
                        .by_name("xl/worksheets/sheet1.xml")
                        .unwrap()
                        .read_to_string(&mut xml)
                        .unwrap();
                    assert!(xml.contains("<v>0</v>"));
                    assert!(!xml.contains("<v>-0</v>"));
                }
            }
            assert!(export_xlsx_for_profile(&workbook, OutputProfile::Shared).is_ok());
            let csv =
                export_csv_for_profile(&workbook.sheets[0], OutputProfile::NativeTables).unwrap();
            assert!(
                String::from_utf8(csv)
                    .unwrap()
                    .ends_with(&format!("{value}\r\n"))
            );
        }
    }

    #[test]
    fn native_tables_writer_refuses_geometry_plus_one_without_widening_shared_profile() {
        let profile = OutputProfile::NativeTables;
        let mut workbook = SourceWorkbook {
            sheets: vec![sheet(32, 128)],
            ledger: vec![],
        };
        assert!(export_csv_for_profile(&workbook.sheets[0], profile).is_ok());
        assert!(export_xlsx_for_profile(&workbook, profile).is_ok());
        assert!(export_xlsx_for_profile(&workbook, OutputProfile::Shared).is_err());
        for invalid in [sheet(33, 1), sheet(1, 129)] {
            assert!(export_csv_for_profile(&invalid, profile).is_err());
            workbook.sheets = vec![invalid];
            assert!(export_xlsx_for_profile(&workbook, profile).is_err());
        }
        workbook.sheets = (0..32)
            .map(|index| {
                let mut sheet = sheet(1, 0);
                sheet.name = format!("s{index}");
                sheet
            })
            .collect();
        assert!(export_xlsx_for_profile(&workbook, profile).is_ok());
        workbook.sheets.push(sheet(1, 0));
        assert!(export_xlsx_for_profile(&workbook, profile).is_err());
        workbook.sheets = (0..8)
            .map(|index| {
                let mut sheet = sheet(1, 128);
                sheet.name = format!("s{index}");
                sheet
            })
            .collect();
        assert!(export_xlsx_for_profile(&workbook, profile).is_ok());
        workbook.sheets.push(sheet(1, 1));
        assert!(export_xlsx_for_profile(&workbook, profile).is_err());
    }

    #[test]
    fn native_tables_csv_exact_encoded_byte_limit_and_plus_one() {
        let mut sheet = sheet(1, 1);
        // c0 + CRLF + data + CRLF = data + 6 bytes.
        sheet.rows[0][0].value = SourceValue::Text {
            value: "x".repeat(MAX_NATIVE_TABLES_OUTPUT_BYTES - 6),
        };
        assert_eq!(
            export_csv_for_profile(&sheet, OutputProfile::NativeTables)
                .unwrap()
                .len(),
            MAX_NATIVE_TABLES_OUTPUT_BYTES
        );
        if let SourceValue::Text { value } = &mut sheet.rows[0][0].value {
            value.push('x');
        }
        assert!(export_csv_for_profile(&sheet, OutputProfile::NativeTables).is_err());
    }

    fn native_tables_text_with_units(character: char, units: usize) -> String {
        let mut value = character.to_string().repeat(units / character.len_utf16());
        if units % character.len_utf16() != 0 {
            value.push('a');
        }
        assert_eq!(value.encode_utf16().count(), units);
        value
    }

    #[test]
    fn native_tables_xlsx_utf16_boundary_for_values_and_headers() {
        for character in ['x', '漢', '😀'] {
            for header in [false, true] {
                for units in [32_767, 32_768] {
                    let mut workbook = SourceWorkbook {
                        sheets: vec![sheet(1, 1)],
                        ledger: vec![],
                    };
                    let value = native_tables_text_with_units(character, units);
                    if header {
                        workbook.sheets[0].columns[0].name = value;
                    } else {
                        workbook.sheets[0].rows[0][0].value = SourceValue::Text { value };
                    }
                    let result = export_xlsx_for_profile(&workbook, OutputProfile::NativeTables);
                    if units == 32_767 {
                        assert!(result.is_ok(), "{character} header={header}: {result:?}");
                    } else {
                        let error = result.unwrap_err();
                        assert!(error.0.contains("32,767 UTF-16"), "{error:?}");
                        assert!(
                            error.0.contains(if header { "header" } else { "row 2" }),
                            "{error:?}"
                        );
                        let finding = validate_native_tables_xlsx(&workbook).unwrap_err();
                        assert!(finding.blocking);
                        assert_eq!(finding.category, FidelityCategory::UnsupportedSafeDisabled);
                        assert_eq!(finding.code, "native_tables_xlsx_text_limit");
                    }
                    assert!(
                        export_csv_for_profile(&workbook.sheets[0], OutputProfile::NativeTables)
                            .is_ok()
                    );
                    // The repair must not change any other XLSX output profile.
                    assert!(export_xlsx_for_profile(&workbook, OutputProfile::Shared).is_ok());
                }
            }
        }
    }

    #[test]
    fn native_tables_xlsx_exact_expanded_byte_limit_and_plus_one() {
        let profile = OutputProfile::NativeTables;
        let mut workbook = SourceWorkbook {
            sheets: vec![sheet(32, 32)],
            ledger: vec![],
        };
        for cell in workbook.sheets[0].rows.iter_mut().flatten() {
            cell.value = SourceValue::Text {
                value: String::new(),
            };
        }
        let bytes = export_xlsx_for_profile(&workbook, profile).unwrap();
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let overhead: u64 = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().size())
            .sum();
        let mut remaining = MAX_NATIVE_TABLES_EXPANDED_BYTES - usize::try_from(overhead).unwrap();
        for cell in workbook.sheets[0].rows.iter_mut().flatten() {
            let units = remaining.min(32_767);
            cell.value = SourceValue::Text {
                value: "x".repeat(units),
            };
            remaining -= units;
        }
        assert_eq!(remaining, 0);
        let bytes = export_xlsx_for_profile(&workbook, profile).unwrap();
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let expanded: u64 = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().size())
            .sum();
        assert_eq!(
            expanded,
            u64::try_from(MAX_NATIVE_TABLES_EXPANDED_BYTES).unwrap()
        );
        let last = workbook.sheets[0]
            .rows
            .last_mut()
            .unwrap()
            .last_mut()
            .unwrap();
        if let SourceValue::Text { value } = &mut last.value {
            value.push('x');
            assert!(value.encode_utf16().count() <= 32_767);
        }
        let error = export_xlsx_for_profile(&workbook, profile).unwrap_err();
        assert!(error.0.contains("24 MiB"), "{error:?}");
    }
}
