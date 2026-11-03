//! Exercise the public host and browser arenas through the same complete lifecycle.
use super::{
    PROJECT, REQUEST, RESPONSE, tachiko_designer_project_close, tachiko_designer_project_export_v3,
    tachiko_designer_spreadsheet_run,
};
use crate::host_bridge::DesignerHost;
use crate::interop_adapter::{ImportOptions, SourceValue, export_xlsx, import_csv, import_xlsx};
use serde_json::Value;

fn spreadsheet(host: &mut DesignerHost, request: &Value, source: &[u8]) -> (Value, Vec<u8>) {
    let request = serde_json::to_vec(request).unwrap();
    REQUEST.with(|slot| slot.borrow_mut().clone_from(&request));
    PROJECT.with(|slot| *slot.borrow_mut() = source.to_vec());
    tachiko_designer_spreadsheet_run();
    let native = host.spreadsheet(&request, source);
    assert_eq!(RESPONSE.with(|slot| slot.borrow().clone()), native.reply);
    let bytes = native.bytes.unwrap_or_default();
    // Read-only/import operations may retain source bytes in the browser arena.
    if request.windows(8).any(|part| part == b"\"export\"") {
        assert_eq!(PROJECT.with(|slot| slot.borrow().clone()), bytes);
    }
    (serde_json::from_slice(&native.reply).unwrap(), bytes)
}

fn import_request(types: &[&str], format: &str) -> Value {
    let mut request: Value = serde_json::from_str(
        r#"{"type":"import","format":"csv","csv_options":{"delimiter":",","header":true},"selection":{"column_types":[],"extra_columns":[[]]},"occurrence_id":"00000000-0000-4000-8000-000000000635","install":true}"#,
    ).unwrap();
    request["format"] = format.into();
    request["selection"]["column_types"] = serde_json::to_value(vec![types]).unwrap();
    request
}

fn export_request(metadata: &Value, format: &str) -> Value {
    let mut request: Value = serde_json::from_str(
        r#"{"type":"export","format":"xlsx","expected_revision":"resident/0","metadata":null,"collection":"sheet_1"}"#,
    ).unwrap();
    request["format"] = format.into();
    request["metadata"] = metadata.clone();
    request["collection"] = metadata["sheets"][0]["schema_id"].clone();
    request
}

fn assert_ok(reply: &Value) {
    assert_eq!(reply["status"], "ok", "{reply}");
}

fn lifecycle(source: &[u8], source_format: &str, types: &[&str], output: &str) {
    tachiko_designer_project_close();
    let mut host = DesignerHost::new();
    let (imported, _) = spreadsheet(&mut host, &import_request(types, source_format), source);
    assert_ok(&imported);
    let request = export_request(&imported["response"]["payload"]["metadata"], output);
    let (exported, bytes) = spreadsheet(&mut host, &request, &[]);
    assert_ok(&exported);
    let parsed = if output == "xlsx" {
        import_xlsx(&bytes).unwrap()
    } else {
        import_csv(&bytes, &ImportOptions::default()).unwrap()
    };
    assert!(!parsed.ledger.iter().any(|finding| finding.blocking));
    if types.len() == 16 && types[0] == "date" {
        assert_eq!(parsed.sheets[0].rows.len(), 128);
        assert!(
            !parsed.ledger.iter().any(|finding| matches!(
                finding.code.as_str(),
                "style_profile" | "bounded_workbook"
            ))
        );
        assert!(parsed.sheets[0].rows.iter().flatten().all(|cell| {
            if output == "xlsx" {
                cell.value
                    == SourceValue::Date {
                        value: "2026-11-04".into(),
                    }
            } else {
                cell.value
                    == SourceValue::Text {
                        value: "2026-11-04".into(),
                    }
            }
        }));
    }
    // The downloaded bytes must support the same explicit field selection.
    let (reimported, _) = spreadsheet(&mut host, &import_request(types, output), &bytes);
    assert_ok(&reimported);
    let metadata = &reimported["response"]["payload"]["metadata"];
    REQUEST.with(|slot| *slot.borrow_mut() = b"resident/0".to_vec());
    tachiko_designer_project_export_v3();
    let saved = host.export_project_v3(b"resident/0");
    assert_eq!(RESPONSE.with(|slot| slot.borrow().clone()), saved.reply);
    assert_ok(&serde_json::from_slice(&saved.reply).unwrap());
    let saved = saved.bytes.unwrap();
    assert_eq!(PROJECT.with(|slot| slot.borrow().clone()), saved);
    let mut reopen: Value = serde_json::from_str(
        r#"{"type":"open_project","occurrence_id":"00000000-0000-4000-8000-000000000636","metadata":null}"#,
    ).unwrap();
    reopen["metadata"] = metadata.clone();
    let (opened, _) = spreadsheet(&mut host, &reopen, &saved);
    assert_ok(&opened);
    let (after, after_bytes) = spreadsheet(&mut host, &export_request(metadata, output), &[]);
    assert_ok(&after);
    // Re-import is a fresh admission: a shorter XLSX ledger may fit Shared
    // even when the original CSV projection required TextCapacity. Preserve
    // complete decoded sheets, not the previous occurrence's ZIP layout.
    let after_book = if output == "xlsx" {
        import_xlsx(&after_bytes).unwrap()
    } else {
        import_csv(&after_bytes, &ImportOptions::default()).unwrap()
    };
    assert_eq!(after_book.sheets, parsed.sheets);
    tachiko_designer_project_close();
}

#[test]
fn admitted_profiles_survive_host_and_wasm_export_reimport_save_reopen() {
    let headers = (0..16)
        .map(|index| format!("C{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let dates = format!("{}\n", ["2026-11-04"; 16].join(",")).repeat(128);
    let csv = format!("{headers}\n{dates}");
    for output in ["xlsx", "csv"] {
        lifecycle(csv.as_bytes(), "csv", &["date"; 16], output);
    }
    for rows in [65, 128] {
        let csv = format!("Name\n{}", "value\n".repeat(rows));
        for output in ["xlsx", "csv"] {
            lifecycle(csv.as_bytes(), "csv", &["text"], output);
        }
    }
    let csv = format!("Name\n{}", "_x0041_\n".repeat(64));
    lifecycle(csv.as_bytes(), "csv", &["text"], "csv");
    let csv = format!("Amount\n{}", "1.25\n".repeat(100));
    let mut styled = import_csv(csv.as_bytes(), &ImportOptions::default()).unwrap();
    for cell in styled.sheets[0].rows.iter_mut().flatten() {
        cell.value = SourceValue::Number { value: 1.25 };
        cell.style.number_format = Some("0.00".into());
    }
    lifecycle(&export_xlsx(&styled).unwrap(), "xlsx", &["number"], "xlsx");
}

#[test]
fn large_csv_escapes_are_refused_at_host_and_wasm_admission() {
    tachiko_designer_project_close();
    let mut host = DesignerHost::new();
    for (header, value) in [("Name", "_x0041_"), ("_x0041_", "value")] {
        let csv = format!("{header}\n{}", format!("{value}\n").repeat(65));
        let (reply, bytes) =
            spreadsheet(&mut host, &import_request(&["text"], "csv"), csv.as_bytes());
        assert_eq!(reply["status"], "error", "{reply}");
        assert!(reply.to_string().contains("#637"));
        assert_eq!(bytes, Vec::<u8>::new());
    }
    tachiko_designer_project_close();
}
