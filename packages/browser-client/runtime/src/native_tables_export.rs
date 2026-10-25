//! Outbound scalar projection for native tables. No import provenance or client
//! presentation participates: the immutable resident snapshot owns every value.
use std::collections::{BTreeMap, BTreeSet};

use tachiko_workspace_engine::{Document, FieldRef, FieldType, Value};

use crate::interop_adapter::{
    FidelityCategory, FidelityFinding, NativeTablesCheck, OutputProfile, SourceCell, SourceColumn,
    SourceSheet, SourceWorkbook, csv_formula_text, export_csv_for_profile, export_xlsx_for_profile,
    native_tables_caseless_key, native_tables_compatibility, native_tables_trim_worksheet_name,
    native_tables_xlsx_number_findings, validate_native_tables_csv, validate_native_tables_xlsx,
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
                    // Formula cells export their current computed Number; the
                    // definition itself stays in the Tachiko source.
                    Some(Value::Number(_) | Value::Formula(_)) => {
                        column.field_type == FieldType::Number
                    }
                    Some(Value::Boolean(_)) => column.field_type == FieldType::Boolean,
                    Some(Value::Date(_)) => column.field_type == FieldType::Date,
                    Some(Value::Reference(_)) => false,
                })
        })
}

/// Disclose every constrained field whose rule has no native spreadsheet
/// mapping. Values still export; nothing validates them after the transfer.
fn constraint_findings(
    document: &Document,
    spec: &CollectionSpec,
    ledger: &mut Vec<FidelityFinding>,
) {
    for field in document.schemas[spec.summary.id.as_str()].fields.values() {
        let requirement = match &field.constraint {
            tachiko_workspace_engine::FieldConstraint::None => continue,
            tachiko_workspace_engine::FieldConstraint::TextLiteralSet { values } => {
                format!("allowed values: {}", values.join(", "))
            }
            tachiko_workspace_engine::FieldConstraint::NumberInclusiveRange { min, max } => {
                format!("allowed range: {} to {}", min.get(), max.get())
            }
        };
        ledger.push(finding(
            FidelityCategory::LossyOnExport,
            "native_tables_field_constraint_excluded",
            &format!("collection {}/field {}", spec.summary.id, field.id),
            format!(
                "Field constraint ({requirement}) has no native spreadsheet mapping; its values export unchanged and the spreadsheet does not validate them."
            ),
        ));
    }
}

/// Workbook-wide exclusions that no native spreadsheet mapping carries.
fn exclusion_findings(ledger: &mut Vec<FidelityFinding>) {
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
}

/// Disclose derived state that the spreadsheet cannot carry: constrained
/// rules and formula definitions stay in the Tachiko source.
fn collection_findings(
    document: &Document,
    spec: &CollectionSpec,
    ledger: &mut Vec<FidelityFinding>,
) {
    constraint_findings(document, spec, ledger);
    if spec.entities.iter().any(|id| {
        spec.columns.iter().any(|column| {
            matches!(
                document.entities[id].fields.get(&column.id),
                Some(Value::Formula(_))
            )
        })
    }) {
        ledger.push(finding(FidelityCategory::LossyOnExport, "native_tables_formula_values_only", &spec.summary.id,
            "Formula cells export their current computed Number values; formula definitions are not written to the spreadsheet. Retain the Tachiko source to keep the formulas.".into()));
    }
}

/// Current calculated Number for every formula cell in the selected collection.
/// The resident snapshot is authoritative; no stored or cached result is read.
fn computed_formula_values(
    runtime: &DesignerRuntime,
    revision: &str,
    document: &Document,
    spec: &CollectionSpec,
) -> Result<BTreeMap<FieldRef, crate::CalculationProjection>, DesignerError> {
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
        let fields = chunk.iter().map(crate::field_target).collect::<Vec<_>>();
        let projection = runtime.query_fields(revision, &fields)?;
        for field in projection.fields {
            let target = FieldRef::new(field.target.entity, field.target.field);
            values.insert(
                target,
                field
                    .calculated
                    .unwrap_or(crate::CalculationProjection::Unavailable),
            );
        }
    }
    Ok(values)
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
        native_tables_trim_worksheet_name(&cleaned)
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
        if used.insert(native_tables_caseless_key(&candidate)) {
            return candidate;
        }
    }
    unreachable!("the finite export profile cannot exhaust name suffixes")
}

/// Project one stored scalar or one formula's current computed Number. A
/// formula without a current result refuses instead of writing a blank.
fn projected_cell(
    document: &Document,
    spec: &CollectionSpec,
    entity: &tachiko_workspace_engine::EntityId,
    column: &crate::ColumnSpec,
    computed: &BTreeMap<FieldRef, crate::CalculationProjection>,
) -> Result<SourceCell, DesignerError> {
    let stored = document.entities[entity].fields.get(&column.id);
    if !matches!(stored, Some(Value::Formula(_))) {
        return crate::interop_document::export_scalar(stored).map(|value| SourceCell {
            value,
            ..SourceCell::default()
        });
    }
    let target = FieldRef::new(entity.clone(), column.id.clone());
    let location = format!(
        "collection {}/field {}/row {entity}",
        spec.summary.id, column.id
    );
    let value = match computed.get(&target) {
        Some(crate::CalculationProjection::Value { value }) => *value,
        Some(crate::CalculationProjection::Failure { code, message }) => {
            return Err(refused(
                "native_tables_formula_value_unavailable",
                &location,
                format!(
                    "Formula cell has no current Number result ({code}: {message}); export refuses rather than writing a blank or a stale value."
                ),
            ));
        }
        Some(crate::CalculationProjection::Unavailable) | None => {
            return Err(refused(
                "native_tables_formula_value_unavailable",
                &location,
                "Formula cell has no current Number result; export refuses rather than writing a blank or a stale value.",
            ));
        }
    };
    Ok(SourceCell {
        value: crate::interop_adapter::SourceValue::Number { value },
        ..SourceCell::default()
    })
}

fn project_sheet(
    runtime: &DesignerRuntime,
    revision: &str,
    document: &Document,
    spec: &CollectionSpec,
    csv: bool,
    names: &mut BTreeSet<String>,
    ledger: &mut Vec<FidelityFinding>,
) -> Result<SourceSheet, DesignerError> {
    let name = if csv {
        mapped_name(&spec.summary.display_name, true, names)
    } else {
        native_tables_compatibility(
            NativeTablesCheck::Name {
                value: &spec.summary.display_name,
                worksheet: true,
                used: names,
                allocate: true,
            },
            &format!("collection {}/sheet-name", spec.summary.id),
        )
        .map_err(|finding| DesignerError::NativeTablesExport { finding })?
        .name
    };
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
            if csv {
                native_tables_compatibility(
                    NativeTablesCheck::CsvText(&column.display_name),
                    &format!("collection {}/header/field {}", spec.summary.id, column.id),
                )
                .map_err(|finding| DesignerError::NativeTablesExport { finding })?;
            }
            // Test the source too: sanitization must never bypass injection refusal.
            if csv && csv_formula_text(&column.display_name) {
                return Err(refused(
                    "native_tables_csv_formula_text",
                    column.id.as_str(),
                    "CSV header could be interpreted as a spreadsheet formula; use typed XLSX.",
                ));
            }
            let name = if csv {
                mapped_name(&column.display_name, false, &mut headers)
            } else {
                native_tables_compatibility(
                    NativeTablesCheck::Name {
                        value: &column.display_name,
                        worksheet: false,
                        used: &mut headers,
                        allocate: true,
                    },
                    &format!("collection {}/header/field {}", spec.summary.id, column.id),
                )
                .map_err(|finding| DesignerError::NativeTablesExport { finding })?
                .name
            };
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
    let computed = computed_formula_values(runtime, revision, document, spec)?;
    let rows = spec
        .entities
        .iter()
        .map(|id| {
            spec.columns
                .iter()
                .map(|column| projected_cell(document, spec, id, column, &computed))
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
    /// Refuses stale revisions, unknown active collections, mixed documents,
    /// formula cells without a current Number and output bounds. Reference
    /// collections have no native mapping.
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
            collection_findings(document, spec, &mut ledger);
            // Canonical specs are Rust-derived; the client supplies no names,
            // identity mapping, row order or values.
            sheets.push(project_sheet(
                self,
                expected_revision,
                document,
                spec,
                csv,
                &mut names,
                &mut ledger,
            )?);
        }
        exclusion_findings(&mut ledger);
        for id in document.keyed_grouped_sum_definitions.keys() {
            ledger.push(finding(FidelityCategory::LossyOnExport, "native_tables_saved_grouped_sum_excluded", id.as_str(),
                format!("Saved keyed-grouped-sum definition {id} and its calculated results are excluded.")));
        }
        if csv {
            ledger.push(finding(FidelityCategory::LossyOnExport, "native_tables_csv_values_only", collection,
                "CSV exports scalar lexical values only: typing and formatting are lost. Formula-leading Text or headers are refused; use typed XLSX.".into()));
            // Either state alone is ambiguous on readback. Populated nonempty
            // Text and empty tables have no value-level loss to disclose.
            if selected.columns.iter().any(|column| {
                !column.required
                    && column.field_type == FieldType::Text
                    && selected.entities.iter().any(|id| {
                        match document.entities[id].fields.get(&column.id) {
                            None => true,
                            Some(Value::Text(value)) => value.is_empty(),
                            _ => false,
                        }
                    })
            }) {
                ledger.push(finding(FidelityCategory::LossyOnExport, "native_tables_csv_blank_empty_text", &selected.summary.id,
                    "CSV cannot distinguish an absent optional Text value from present empty Text; both export as an empty field. Retain the Tachiko source to preserve presence.".into()));
            }
            ledger.push(finding(FidelityCategory::NativeEquivalent, "native_tables_csv_number_precision", collection,
                "CSV keeps full binary64 Number precision in shortest round-trip decimal text, including subnormal values. A spreadsheet application may infer types or round values when opening CSV.".into()));
        }
        let mut workbook = SourceWorkbook { sheets, ledger };
        if csv {
            validate_native_tables_csv(&workbook.sheets[0])
                .map_err(|finding| DesignerError::NativeTablesExport { finding })?;
        } else {
            validate_native_tables_xlsx(&workbook)
                .map_err(|finding| DesignerError::NativeTablesExport { finding })?;
            let precision_findings = native_tables_xlsx_number_findings(&workbook);
            workbook.ledger.extend(precision_findings);
        }
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
