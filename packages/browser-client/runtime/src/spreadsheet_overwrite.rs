//! Occurrence-local proof for replacing an XLSX opened by this runtime.
use std::{collections::BTreeMap, io::Cursor};

use quick_xml::{Reader, events::Event};
use zip::ZipArchive;

use crate::capacity::SessionProfile;
use crate::interop_adapter::{FidelityCategory, FidelityFinding, archive, export_xlsx_for_profile};
use crate::interop_document::{ImportedOutput, imported_export_error};
use crate::{
    DesignerError, DesignerResponse, DesignerRuntime, DiagnosticProjection, OccurrenceProjection,
    SpreadsheetExportProjection,
};

mod xml_part;

pub(super) type OverwriteProof = Result<Vec<String>, Vec<FidelityFinding>>;

pub(super) fn remember(
    candidate: Option<&mut DesignerRuntime>,
    original: Option<&[u8]>,
    response: &DesignerResponse,
) {
    if let (Some(candidate), Some(original)) = (candidate, original) {
        let ledger = match response {
            DesignerResponse::Imported(imported) => &imported.ledger,
            DesignerResponse::ImportedWindow(imported) => &imported.ledger,
            _ => unreachable!("an installed XLSX has an import reply"),
        };
        candidate.xlsx_overwrite = Some(prove(candidate, original, ledger));
    }
}

fn prove(runtime: &DesignerRuntime, original: &[u8], ledger: &[FidelityFinding]) -> OverwriteProof {
    let mut findings: Vec<_> = ledger
        .iter()
        .filter(|finding| finding.code == "rows_above_header_skipped")
        .cloned()
        .collect();
    // Compare the untouched semantic export. An import ledger cannot account
    // for content the reader never represented. ZIP compression and order
    // are immaterial. XML may differ in serialization without losing content.
    let result = encode(runtime, runtime.current_revision())
        .and_then(|(bytes, _)| compare_packages(original, &bytes, runtime.profile));
    match result {
        Ok((parts, differences)) => {
            findings.extend(differences);
            if findings.is_empty() {
                Ok(parts)
            } else {
                Err(findings)
            }
        }
        Err(error) => {
            findings.push(finding(
                "xlsx_source_unverified",
                "workbook",
                error.to_string(),
            ));
            Err(findings)
        }
    }
}

fn compare_packages(
    original: &[u8],
    regenerated: &[u8],
    profile: SessionProfile,
) -> Result<(Vec<String>, Vec<FidelityFinding>), DesignerError> {
    let working = profile == SessionProfile::WorkingSize;
    let parts = archive(original, working).map_err(package_error)?;
    let regenerated = archive(regenerated, working).map_err(package_error)?;
    let findings = parts
        .iter()
        .filter_map(|(path, bytes)| difference(path, bytes, &regenerated))
        .collect();
    Ok((parts.into_keys().collect(), findings))
}

fn difference(
    path: &str,
    bytes: &[u8],
    regenerated: &BTreeMap<String, Vec<u8>>,
) -> Option<FidelityFinding> {
    let copy = regenerated.get(path);
    if copy.is_some_and(|copy| xml_part::equivalent(path, bytes, copy)) {
        return None;
    }
    let missing = copy.is_none();
    if has_rich_text(bytes) {
        let message = if missing {
            "This part is omitted. Its rich-text runs are flattened to plain text."
        } else {
            "Rich-text runs in this part are flattened to plain text."
        };
        return Some(finding("xlsx_rich_text_flattened", path, message));
    }
    Some(if missing {
        finding(
            "xlsx_source_part_omitted",
            path,
            "This original XLSX part is omitted on export.",
        )
    } else {
        finding(
            "xlsx_source_part_changed",
            path,
            "Work cannot verify preservation of this original XLSX part.",
        )
    })
}

fn has_rich_text(bytes: &[u8]) -> bool {
    let mut reader = Reader::from_reader(bytes);
    let mut parents = Vec::new();
    while let Ok(event) = reader.read_event() {
        match event {
            Event::Start(node) | Event::Empty(node)
                if node.local_name().as_ref() == b"r" && parents.last() == Some(&true) =>
            {
                return true;
            }
            Event::Start(node) => parents.push(matches!(node.local_name().as_ref(), b"is" | b"si")),
            Event::End(_) => {
                parents.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    false
}

fn encode(
    runtime: &DesignerRuntime,
    revision: &str,
) -> Result<(Vec<u8>, Vec<FidelityFinding>), DesignerError> {
    let metadata = &runtime
        .imported
        .as_ref()
        .expect("XLSX has resident metadata")
        .metadata;
    let workbook = runtime.export_imported_workbook(revision, metadata, ImportedOutput::Xlsx)?;
    if let Some(sheet) = workbook.sheets.iter().find(|sheet| !sheet.has_header) {
        return Err(imported_export_error(
            &sheet.name,
            "This worksheet has no XLSX header.",
        ));
    }
    let bytes = export_xlsx_for_profile(&workbook, runtime.imported_output_profile())
        .map_err(package_error)?;
    Ok((bytes, workbook.ledger))
}

fn package_error(error: impl std::fmt::Display) -> DesignerError {
    imported_export_error("workbook", &error.to_string())
}

pub(super) fn reopen_working_size(
    runtime: Option<&DesignerRuntime>,
    expected: &OccurrenceProjection,
    occurrence_id: &str,
) -> Result<(DesignerRuntime, DesignerResponse), DesignerError> {
    let runtime = runtime.ok_or_else(source_unavailable)?;
    check_expected(runtime, expected)?;
    let proof = runtime
        .xlsx_overwrite
        .as_ref()
        .ok_or_else(source_unavailable)?;
    if expected.revision != "resident/0" {
        return Err(DesignerError::InvalidProjectTransfer {
            message: "Reopen the XLSX at working size before editing.".into(),
        });
    }
    if occurrence_id == runtime.row_namespace {
        return Err(DesignerError::InvalidOccurrenceIdentity);
    }

    // Both the snapshot and its mapping come from the proof's live occurrence.
    let snapshot = runtime.export_project_v3(&expected.revision)?;
    let metadata = snapshot
        .import_metadata
        .expect("XLSX has resident metadata");
    let (mut candidate, opened) =
        crate::admit_imported_project_working_size(&snapshot.bytes, occurrence_id, &metadata)?;
    candidate.xlsx_overwrite = Some(match proof {
        Ok(_) => {
            // The untouched export already matched the source. Verify the
            // reopened representation against it before carrying the proof.
            let (verified, _) = encode(runtime, &expected.revision)?;
            prove(&candidate, &verified, &[])
        }
        Err(_) => proof.clone(),
    });
    Ok((candidate, DesignerResponse::OpenedWindow(Box::new(opened))))
}

fn source_unavailable() -> DesignerError {
    refusal(vec![finding(
        "xlsx_source_unavailable",
        "workbook",
        "Open the original XLSX in this session to verify overwrite fidelity.",
    )])
}

fn check_expected(
    runtime: &DesignerRuntime,
    expected: &OccurrenceProjection,
) -> Result<(), DesignerError> {
    runtime.check_revision(&expected.revision)?;
    if expected.scope != runtime.occurrence_scope() {
        return Err(refusal(vec![finding(
            "xlsx_source_occurrence_changed",
            "workbook",
            "The opened workbook has changed. Overwrite is refused for the previous workbook.",
        )]));
    }
    Ok(())
}

pub(super) fn export(
    runtime: &DesignerRuntime,
    expected: &OccurrenceProjection,
) -> Result<(SpreadsheetExportProjection, Vec<u8>), DesignerError> {
    check_expected(runtime, expected)?;
    let revision = &expected.revision;
    let proof = runtime
        .xlsx_overwrite
        .as_ref()
        .ok_or_else(source_unavailable)?;
    let original = proof
        .as_ref()
        .map_err(|findings| refusal(findings.clone()))?;
    let (bytes, ledger) = encode(runtime, revision)?;
    let mut findings: Vec<_> = ledger
        .iter()
        .filter(|finding| {
            finding.blocking
                || !matches!(finding.category, FidelityCategory::NativeEquivalent)
                    && !matches!(
                        finding.code.as_str(),
                        "imported_current_structure" | "bound_formula_absolute_a1"
                    )
        })
        .cloned()
        .collect();
    let archive = ZipArchive::new(Cursor::new(&bytes)).map_err(package_error)?;
    findings.extend(
        original
            .iter()
            .filter(|path| archive.index_for_name(path).is_none())
            .map(|path| {
                finding(
                    "xlsx_source_part_omitted",
                    path,
                    "This original XLSX part is omitted on export.",
                )
            }),
    );
    if !findings.is_empty() {
        return Err(refusal(findings));
    }
    crate::enforce_project_transfer_limit(bytes.len())?;
    let receipt = SpreadsheetExportProjection {
        revision: revision.to_owned(),
        byte_length: bytes.len(),
        ledger,
    };
    Ok((receipt, bytes))
}

fn finding(code: &str, path: &str, message: impl Into<String>) -> FidelityFinding {
    FidelityFinding {
        category: FidelityCategory::LossyOnExport,
        code: code.into(),
        location: path.into(),
        message: message.into(),
        blocking: true,
    }
}

fn refusal(findings: Vec<FidelityFinding>) -> DesignerError {
    DesignerError::SpreadsheetOverwrite { findings }
}

pub(super) fn export_failure(error: &DesignerError) -> (&'static str, Vec<DiagnosticProjection>) {
    let (code, findings) = match error {
        DesignerError::NativeTablesExport { finding } => (
            "native_tables_export_refused",
            std::slice::from_ref(finding),
        ),
        DesignerError::ImportedSpreadsheetExport { finding } => (
            "imported_spreadsheet_export_refused",
            std::slice::from_ref(finding),
        ),
        DesignerError::SpreadsheetOverwrite { findings } => {
            ("xlsx_overwrite_refused", findings.as_slice())
        }
        _ => unreachable!("spreadsheet export failure has fidelity findings"),
    };
    let diagnostics = findings
        .iter()
        .map(|finding| DiagnosticProjection {
            code: finding.code.clone(),
            message: finding.message.clone(),
            path: finding.location.clone(),
            subjects: Vec::new(),
            related_subjects: Vec::new(),
            facts: Vec::new(),
        })
        .collect();
    (code, diagnostics)
}
