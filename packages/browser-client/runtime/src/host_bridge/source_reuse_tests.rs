use super::{DesignerHost, ImportOptions, SourceWorkbook, import_csv_profile};
use serde_json::json;

#[test]
fn inspected_source_lifecycle_and_fallback_replies() {
    let options = ImportOptions::default();
    let source = export_xlsx(&import_csv_profile(b"Name\nAda\n", &options, false).unwrap());
    let different = export_xlsx(&import_csv_profile(b"Name\nGrace\n", &options, false).unwrap());
    let inspect = json!({"type":"inspect", "format":"xlsx", "csv_options":options});
    let import = json!({"type":"import", "format":"xlsx", "csv_options":options,
        "selection":{"column_types":[["text"]], "extra_columns":[[]]},
        "occurrence_id":"00000000-0000-4000-8000-000000000678", "install":true});
    let mut host = DesignerHost::new();
    let inspect_bytes = serde_json::to_vec(&inspect).unwrap();
    for (bytes, changed_options) in [(&source, false), (&different, false), (&source, true)] {
        let preview = host.spreadsheet(&inspect_bytes, &source);
        assert!(host.retained_source.is_some());
        assert_eq!(
            preview.reply,
            DesignerHost::new()
                .spreadsheet(&inspect_bytes, &source)
                .reply
        );
        let mut request = import.clone();
        if changed_options {
            request["csv_options"]["keep_summary_rows"] = true.into();
        }
        let request = serde_json::to_vec(&request).unwrap();
        let actual = host.spreadsheet(&request, bytes);
        let expected = DesignerHost::new().spreadsheet(&request, bytes);
        assert_eq!(actual.reply, expected.reply);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&actual.reply).unwrap()["status"],
            "ok"
        );
        assert!(host.retained_source.is_none());
    }
    let _ = host.spreadsheet(&inspect_bytes, &source);
    host.close_project();
    assert!(host.retained_source.is_none());
    let _ = host.spreadsheet(&inspect_bytes, &source);
    let failed = host.spreadsheet(&inspect_bytes, b"invalid zip");
    assert!(host.retained_source.is_none());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&failed.reply).unwrap()["status"],
        "error"
    );
}

fn export_xlsx(workbook: &SourceWorkbook) -> Vec<u8> {
    crate::interop_adapter::export_xlsx(workbook).unwrap()
}
