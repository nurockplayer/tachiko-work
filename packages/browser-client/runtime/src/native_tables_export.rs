//! Outbound scalar projection for native tables. No import provenance or client
//! presentation participates: the immutable resident snapshot owns every value.
use std::collections::BTreeSet;

use tachiko_workspace_engine::{Document, FieldType, Value};

use crate::interop_adapter::{
    FidelityCategory, FidelityFinding, OutputProfile, SourceCell, SourceColumn, SourceSheet,
    SourceWorkbook, csv_formula_text, export_csv_for_profile, export_xlsx_for_profile,
    validate_output_for_profile,
};
use crate::{
    CollectionSpec, DesignerError, DesignerResponse, DesignerRuntime, DesignerWireReply,
    SpreadsheetExportProjection,
};

fn finding(
    category: FidelityCategory,
    code: &str,
    location: &str,
    message: String,
) -> FidelityFinding {
    FidelityFinding {
        category,
        code: code.into(),
        location: location.into(),
        message,
        blocking: false,
    }
}

fn refused(code: &str, location: &str, message: impl Into<String>) -> DesignerError {
    let mut finding = finding(
        FidelityCategory::UnsupportedSafeDisabled,
        code,
        location,
        message.into(),
    );
    finding.blocking = true;
    DesignerError::NativeTablesExport { finding }
}

fn scalar_mapping(document: &Document, spec: &CollectionSpec) -> bool {
    !spec.columns.is_empty()
        && spec.columns.iter().all(|column| {
            matches!(
                column.field_type,
                FieldType::Text | FieldType::Number | FieldType::Boolean | FieldType::Date
            )
        })
        && spec.entities.iter().all(|id| {
            let entity = &document.entities[id];
            spec.columns
                .iter()
                .all(|column| match entity.fields.get(&column.id) {
                    None => !column.required,
                    Some(Value::Text(_)) => column.field_type == FieldType::Text,
                    Some(Value::Number(_)) => column.field_type == FieldType::Number,
                    Some(Value::Boolean(_)) => column.field_type == FieldType::Boolean,
                    Some(Value::Date(_)) => column.field_type == FieldType::Date,
                    Some(Value::Reference(_) | Value::Formula(_)) => false,
                })
        })
}

fn safe_character(c: char) -> bool {
    c >= ' ' && !matches!(c, '\u{fffe}' | '\u{ffff}')
}

/// Reserve suffix space before truncating, and always test the complete name.
/// `BTreeSet` membership is only a lookup; allocation follows canonical order.
fn mapped_name(source: &str, worksheet: bool, used: &mut BTreeSet<String>) -> String {
    let cleaned: String = source
        .chars()
        .map(|c| {
            if !safe_character(c) || (worksheet && "[]:*?/\\".contains(c)) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = if worksheet {
        cleaned.trim().trim_matches('\'')
    } else {
        cleaned.as_str()
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
        let prefix: String = if worksheet {
            base.chars()
                .take(31 - suffix.chars().count())
                .collect::<String>()
                .trim_end_matches('\'')
                .to_owned()
        } else {
            base.to_owned()
        };
        let candidate = format!("{prefix}{suffix}");
        if used.insert(candidate.to_lowercase()) {
            return candidate;
        }
    }
    unreachable!("the finite export profile cannot exhaust name suffixes")
}

fn project_sheet(
    document: &Document,
    spec: &CollectionSpec,
    csv: bool,
    names: &mut BTreeSet<String>,
    ledger: &mut Vec<FidelityFinding>,
) -> Result<SourceSheet, DesignerError> {
    let name = mapped_name(&spec.summary.display_name, true, names);
    // CSV has no worksheet name, but keeping this deterministic internal name
    // lets the same rectangular validator check both projections.
    if !csv {
        ledger.push(finding(
            FidelityCategory::Converted,
            "native_tables_worksheet_name",
            &spec.summary.id,
            format!(
                "Worksheet name {:?} mapped to {name:?}.",
                spec.summary.display_name
            ),
        ));
    }
    let mut headers = BTreeSet::new();
    let columns = spec
        .columns
        .iter()
        .map(|column| {
            // Test the source too: sanitization must never bypass injection refusal.
            if csv && csv_formula_text(&column.display_name) {
                return Err(refused(
                    "native_tables_csv_formula_text",
                    column.id.as_str(),
                    "CSV header could be interpreted as a spreadsheet formula; use typed XLSX.",
                ));
            }
            let name = mapped_name(&column.display_name, false, &mut headers);
            if name != column.display_name {
                ledger.push(finding(
                    FidelityCategory::Converted,
                    "native_tables_header_name",
                    &format!("{}/{}", spec.summary.id, column.id),
                    format!(
                        "Header {:?} mapped to {name:?}; headers are never truncated.",
                        column.display_name
                    ),
                ));
            }
            Ok(SourceColumn { name, width: None })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let rows = spec
        .entities
        .iter()
        .map(|id| {
            spec.columns
                .iter()
                .map(|column| {
                    crate::interop_document::export_scalar(
                        document.entities[id].fields.get(&column.id),
                    )
                    .map(|value| SourceCell {
                        value,
                        ..SourceCell::default()
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SourceSheet {
        name,
        has_header: true,
        columns,
        rows,
    })
}

impl DesignerRuntime {
    /// Export every native scalar collection in canonical collection-key order.
    /// Fields and rows use the runtime's canonical stable-identity order.
    ///
    /// # Errors
    /// Refuses stale revisions, unknown active collections, mixed documents and
    /// output bounds. Formula/reference collections have no native mapping.
    pub fn export_native_tables_workbook(
        &self,
        expected_revision: &str,
        collection: &str,
    ) -> Result<SourceWorkbook, DesignerError> {
        self.native_tables_workbook(expected_revision, collection, false)
    }

    /// Export only the selected collection, including absent optional scalars.
    ///
    /// # Errors
    /// Refuses stale revisions, unknown/non-scalar collections and output bounds.
    /// The CSV encoder additionally refuses formula-leading Text and headers.
    pub fn export_native_tables_csv_workbook(
        &self,
        expected_revision: &str,
        collection: &str,
    ) -> Result<SourceWorkbook, DesignerError> {
        self.native_tables_workbook(expected_revision, collection, true)
    }

    fn native_tables_workbook(
        &self,
        expected_revision: &str,
        collection: &str,
        csv: bool,
    ) -> Result<SourceWorkbook, DesignerError> {
        self.check_revision(expected_revision)?;
        let selected = self.collection_spec(collection).map_err(|_| {
            refused(
                "native_tables_unknown_collection",
                collection,
                "Requested collection is unavailable.",
            )
        })?;
        let snapshot = self.session.export_snapshot();
        let document = snapshot.document();
        let mut ledger = vec![finding(FidelityCategory::NativeEquivalent, "native_tables_outbound_profile", "workbook",
            "Native scalar output: at most 32 collections, 32 columns, 128 rows per collection, 1,024 total rows, 8 MiB encoded bytes and 24 MiB expanded XLSX. The general incoming profile remains separate (4 sheets, 16 columns, 64 rows); its restricted Text-capacity exception is unchanged. Larger output is not promised reimportable.".into())];
        let mut sheets = Vec::new();
        let mut names = BTreeSet::new();
        for (key, spec) in &self.collection_specs {
            if csv && spec.summary.id != selected.summary.id {
                ledger.push(finding(FidelityCategory::LossyOnExport, "native_tables_csv_collection_excluded", &spec.summary.id,
                    format!("CSV excludes collection {:?} ({key}); only the requested collection is exported.", spec.summary.display_name)));
                continue;
            }
            if !scalar_mapping(document, spec) {
                return Err(refused(
                    "native_tables_unmapped_collection",
                    &spec.summary.id,
                    format!(
                        "Collection {key:?} has no native scalar mapping; XLSX refuses mixed documents, and CSV requires an eligible selected collection."
                    ),
                ));
            }
            // Canonical specs are Rust-derived; the client supplies no names,
            // identity mapping, row order or values.
            sheets.push(project_sheet(document, spec, csv, &mut names, &mut ledger)?);
        }
        for (code, message) in [
            (
                "native_tables_grouped_sum_definitions_excluded",
                "Keyed-grouped-sum definitions are excluded; retain the Tachiko source to preserve bindings.",
            ),
            (
                "native_tables_grouped_sum_results_excluded",
                "Keyed-grouped-sum calculated results are excluded; no calculated snapshots are emitted.",
            ),
            (
                "native_tables_reports_charts_excluded",
                "Reports and charts are excluded; this export contains native source tables only.",
            ),
            (
                "native_tables_session_history_excluded",
                "Session Undo/Redo history is excluded.",
            ),
            (
                "native_tables_sheet_presentation_excluded",
                "Sheet presentation, view sorting/filtering and per-cell formatting are excluded; Dates use yyyy-mm-dd.",
            ),
        ] {
            ledger.push(finding(
                FidelityCategory::LossyOnExport,
                code,
                "workbook",
                message.into(),
            ));
        }
        for id in document.keyed_grouped_sum_definitions.keys() {
            ledger.push(finding(FidelityCategory::LossyOnExport, "native_tables_saved_grouped_sum_excluded", id.as_str(),
                format!("Saved keyed-grouped-sum definition {id} and its calculated results are excluded.")));
        }
        if csv {
            ledger.push(finding(FidelityCategory::LossyOnExport, "native_tables_csv_values_only", collection,
                "CSV exports scalar lexical values only: typing and formatting are lost, and missing values cannot be distinguished from empty Text. Formula-leading Text or headers are refused; use typed XLSX.".into()));
        }
        let workbook = SourceWorkbook { sheets, ledger };
        validate_output_for_profile(&workbook, OutputProfile::NativeTables)
            .map_err(|error| refused("native_tables_output_refused", "workbook", error.0))?;
        Ok(workbook)
    }
}

/// Both transports call the same encoder and return its exact receipt/ledger.
pub(crate) fn encode_native_tables_spreadsheet(
    runtime: &DesignerRuntime,
    revision: &str,
    collection: &str,
    csv: bool,
) -> Result<(DesignerResponse, Vec<u8>), DesignerError> {
    let workbook = runtime.native_tables_workbook(revision, collection, csv)?;
    let bytes = if csv {
        export_csv_for_profile(&workbook.sheets[0], OutputProfile::NativeTables)
    } else {
        export_xlsx_for_profile(&workbook, OutputProfile::NativeTables)
    }
    .map_err(|error| refused("native_tables_output_refused", collection, error.0))?;
    crate::enforce_project_transfer_limit(bytes.len())?;
    let reply = DesignerWireReply::Ok {
        response: DesignerResponse::SpreadsheetExported(SpreadsheetExportProjection {
            revision: revision.into(),
            byte_length: bytes.len(),
            ledger: workbook.ledger,
        }),
    };
    // Keep the existing complete receipt envelope bound. Refuse with a native
    // finding before either transport can return a generic size failure.
    crate::ensure_wire_reply_size(&reply, runtime.text_capacity).map_err(|error| {
        refused(
            "native_tables_receipt_bounds",
            "workbook",
            error.to_string(),
        )
    })?;
    let DesignerWireReply::Ok { response } = reply else {
        unreachable!("constructed success receipt")
    };
    Ok((response, bytes))
}
