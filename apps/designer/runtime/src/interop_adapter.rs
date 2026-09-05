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
pub const MAX_FORMULAS: usize = 32;

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
        if row.len() > MAX_COLUMNS || rows.len() > MAX_DATA_ROWS + usize::from(options.header) {
            return fail("CSV exceeds row or column bounds");
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
    if width == 0 || width > MAX_COLUMNS || rows.len() > MAX_DATA_ROWS + usize::from(options.header)
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
    Ok(finish_source_admission(SourceWorkbook {
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
    }))
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
    let mut nodes = 0usize;
    loop {
        let event = reader
            .read_event()
            .map_err(|e| InteropError(format!("Invalid XML: {e}")))?;
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
                for a in e.attributes() {
                    attribute_count += 1;
                    if attribute_count > 64 {
                        return fail("XML attribute count exceeds profile");
                    }
                    let a = a.map_err(|e| InteropError(e.to_string()))?;
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
                        if value.len() > 512 {
                            return fail("XML namespace URI exceeds profile");
                        }
                        namespace_declarations.push(value.clone());
                        bindings.insert(String::new(), value);
                    } else if let Some(prefix) = key.strip_prefix("xmlns:") {
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
                    node.text.push_str(&value);
                } else if !value.trim().is_empty() {
                    return fail("Text outside XML root");
                }
            }
            Event::CData(e) => {
                let value = std::str::from_utf8(e.as_ref())
                    .map_err(|_| InteropError("Invalid CDATA".into()))?;
                if !xml_text_valid(value) {
                    return fail("Invalid CDATA XML character");
                }
                if let Some((node, _)) = stack.last_mut() {
                    node.text.push_str(value);
                } else {
                    return fail("CDATA outside XML root");
                }
            }
            Event::DocType(_) => return fail("DTD is forbidden"),
            Event::Eof => break,
            _ => {}
        }
    }
    if !stack.is_empty() {
        return fail("Unclosed XML element");
    }
    root.ok_or_else(|| InteropError("Missing XML root".into()))
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
        14..=17 => Some("yyyy-mm-dd".into()),
        18 => Some("h:mm AM/PM".into()),
        19 => Some("h:mm:ss AM/PM".into()),
        20 => Some("h:mm".into()),
        21 => Some("h:mm:ss".into()),
        22 => Some("yyyy-mm-dd hh:mm:ss".into()),
        37 => Some("#,##0 ;(#,##0)".into()),
        38 => Some("#,##0 ;[Red](#,##0)".into()),
        39 => Some("#,##0.00;(#,##0.00)".into()),
        40 => Some("#,##0.00;[Red](#,##0.00)".into()),
        49 => Some("@".into()),
        _ => Some(format!("unsupported_builtin_{id}")),
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
    ledger.push(finding(FidelityCategory::LossyOnExport,"style_profile","xl/styles.xml","Basic bold/RGB solid fill/wrap/border/alignment/number format retained; font family, theme colors and other layout details are source-only",false));
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
    if col == 0 || row == 0 || row > MAX_DATA_ROWS + 1 {
        return fail("Cell row exceeds profile");
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
    !node.text.trim().is_empty()
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
            if row_index == 0 || row_index > MAX_DATA_ROWS + 1 {
                return fail("Worksheet row exceeds profile");
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
        sheets.push(SourceSheet {
            name,
            has_header: true,
            columns,
            rows,
        });
    }
    root_inventory(&parts, &worksheet_paths, &mut ledger)?;
    ledger.push(finding(FidelityCategory::NativeEquivalent,"bounded_workbook","source","All bounded worksheets and scalar cells were inspected; formula sources require authoritative Rust binding",false));
    Ok(finish_source_admission(SourceWorkbook { sheets, ledger }))
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
    if sheet.columns.is_empty()
        || sheet.columns.len() > MAX_COLUMNS
        || sheet.rows.len() > MAX_DATA_ROWS
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
    if output.len() > MAX_SOURCE_BYTES {
        return fail("CSV output exceeds 2 MiB");
    }
    Ok(output.into_bytes())
}
fn csv_formula_text(value: &str) -> bool {
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
fn valid_worksheet_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= 31
        && !name.contains(['[', ']', ':', '*', '?', '/', '\\'])
        && xml_text_valid(name)
}
pub(crate) fn validate_output(workbook: &SourceWorkbook) -> Result<()> {
    if workbook.sheets.is_empty() || workbook.sheets.len() > MAX_SHEETS {
        return fail("Export requires 1..=4 sheets");
    }
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
                || s.columns
                    .iter()
                    .map(|c| &c.name)
                    .collect::<BTreeSet<_>>()
                    .len()
                    != s.columns.len())
        {
            return fail("XLSX export headers must be nonempty and unique");
        }
        if !valid_worksheet_name(&s.name) || !names.insert(s.name.to_lowercase()) {
            return fail("Invalid or duplicate XLSX sheet name");
        }
        if s.columns.is_empty() || s.columns.len() > MAX_COLUMNS || s.rows.len() > MAX_DATA_ROWS {
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
    validate_output(workbook)?;
    let mut style_keys = vec![
        serde_json::to_string(&CellStyle::default()).map_err(|e| InteropError(e.to_string()))?,
    ];
    let mut styles = vec![CellStyle::default()];
    for s in &workbook.sheets {
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
    let mut sheet_list = String::new();
    let mut sheet_rels = String::new();
    let mut types = String::new();
    for (index, s) in workbook.sheets.iter().enumerate() {
        let n = index + 1;
        sheet_list.push_str(&format!(
            "<sheet name=\"{}\" sheetId=\"{n}\" r:id=\"rId{n}\"/>",
            escape(&s.name)
        ));
        sheet_rels.push_str(&format!("<Relationship Id=\"rId{n}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet{n}.xml\"/>"));
        types.push_str(&format!("<Override PartName=\"/xl/worksheets/sheet{n}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"));
        let mut xml = format!("<worksheet xmlns=\"{MAIN}\"><cols>");
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
        xml.push_str("</cols><sheetData>");
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
                let key = serde_json::to_string(&output_style(c))
                    .map_err(|e| InteropError(e.to_string()))?;
                let sid = style_keys.iter().position(|x| x == &key).unwrap_or(0);
                let formula = c
                    .formula
                    .as_ref()
                    .map(|f| format!("<f>{}</f>", escape(f.trim_start_matches('='))))
                    .unwrap_or_default();
                let (kind, body) = match &c.value {
                    SourceValue::Empty => ("n", formula.clone()),
                    SourceValue::Text { value } => (
                        "inlineStr",
                        format!(
                            "{formula}<is><t xml:space=\"preserve\">{}</t></is>",
                            escape(value)
                        ),
                    ),
                    SourceValue::Number { value } => ("n", format!("{formula}<v>{value}</v>")),
                    SourceValue::Boolean { value } => {
                        ("b", format!("{formula}<v>{}</v>", u8::from(*value)))
                    }
                    SourceValue::Date { value } => {
                        ("d", format!("{formula}<v>{}</v>", escape(value)))
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
    parts.insert("xl/workbook.xml".into(),format!("<workbook xmlns=\"{MAIN}\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><workbookPr date1904=\"0\"/><sheets>{sheet_list}</sheets><calcPr calcMode=\"auto\" fullCalcOnLoad=\"1\"/></workbook>"));
    parts.insert("xl/_rels/workbook.xml.rels".into(),format!("<Relationships xmlns=\"{REL}\">{sheet_rels}<Relationship Id=\"styles\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/></Relationships>"));
    parts.insert("_rels/.rels".into(),format!("<Relationships xmlns=\"{REL}\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>"));
    parts.insert("[Content_Types].xml".into(),format!("<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>{types}</Types>"));
    let mut output = Cursor::new(Vec::new());
    {
        let mut writer = ZipWriter::new(&mut output);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, xml) in parts {
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
    source_bound(&bytes)?;
    Ok(bytes)
}
