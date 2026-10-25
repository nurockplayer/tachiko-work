//! Writer regressions for alternate package ingress and portable export preflight.
use tachiko_designer_runtime::{
    DesignerError, DesignerRequest, DesignerRuntime, FieldTarget, ImportFieldType, ImportSelection,
    ScalarEditInput, import_workbook, interop_adapter, open_local_document, open_project,
};

#[path = "support/portable_package_v3/bytes.rs"]
mod bytes;

const FIXTURE_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../docs/research/fixtures/issue-519-portable-package-v1-v3"
);
const RESIDENT: &str = "00000000-0000-4000-8000-000000000519";
const FRESH: &str = "00000000-0000-4000-8000-000000000520";

fn resident() -> DesignerRuntime {
    DesignerRuntime::moonfall(RESIDENT).unwrap()
}

#[test]
fn local_constrained_package_admits_with_its_declarations() {
    // Confirm both the constrained payload and package independently verify.
    tachiko_storage::CanonicalRoProjectV3::try_from_files(bytes::input_files()).unwrap();
    let mut slot = Some(resident());
    let before = slot.as_ref().unwrap().observe_occurrence();
    let opened = open_local_document(&mut slot, &bytes::vector("golden"), FRESH).unwrap();
    let mut runtime = slot.unwrap();
    assert_ne!(runtime.observe_occurrence(), before);
    let columns = serde_json::to_value(&opened.table.columns).unwrap();
    assert_eq!(
        columns[1]["constraint"],
        serde_json::json!({"kind": "text_literal_set", "values": ["Paper", "Stationery"]})
    );
    assert_eq!(
        columns[2]["constraint"],
        serde_json::json!({"kind": "number_inclusive_range", "min": 0.0, "max": 10.0})
    );
    // A local constrained document keeps its v3 origin: legacy export refuses
    // and the portable re-export is byte-identical.
    assert!(matches!(
        runtime.export_project("resident/0"),
        Err(DesignerError::UnsupportedProject { .. })
    ));
    assert_eq!(
        runtime.export_portable_ro("resident/0").unwrap().bytes,
        bytes::vector("golden")
    );
    // The declarations keep enforcing the admitted stored values.
    let error = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/0".into(),
            target: FieldTarget {
                entity: opened.table.rows[0].id.clone(),
                field: opened.table.columns[1].id.clone(),
            },
            input: ScalarEditInput::Text {
                value: "Other".into(),
            },
        })
        .unwrap_err();
    assert!(matches!(
        error,
        DesignerError::FieldConstraintViolation { .. }
    ));
}

#[test]
fn local_unconstrained_package_preserves_v3_origin_and_exact_payload() {
    let mut slot = Some(resident());
    open_local_document(&mut slot, &bytes::vector("runtime-control"), FRESH).unwrap();
    let runtime = slot.unwrap();
    assert!(matches!(
        runtime.export_project("resident/0"),
        Err(DesignerError::UnsupportedProject { .. })
    ));
    assert_eq!(
        runtime.export_portable_ro("resident/0").unwrap().bytes,
        bytes::vector("runtime-control")
    );
    let tree =
        tachiko_storage::CanonicalRoProjectV3::try_from_files(bytes::control_files()).unwrap();
    assert_eq!(
        tachiko_storage::from_bytes(&bytes::vector("runtime-control")).unwrap(),
        tachiko_storage::decode_roproj_v3(&tree).unwrap()
    );
}

#[test]
fn direct_json_envelope_refuses_constrained_and_unconstrained_packages_without_mutation() {
    for vector in ["golden", "runtime-control"] {
        let runtime = resident();
        let occurrence = runtime.observe_occurrence();
        let saved = runtime.export_project(&occurrence.revision).unwrap();
        let mut slot = Some(runtime);
        let mut envelope = b"TWDPROJ2".to_vec();
        envelope.extend_from_slice(&bytes::vector(vector));
        let error = open_project(&mut slot, &envelope, FRESH).unwrap_err();
        assert!(
            matches!(error, DesignerError::InvalidProjectTransfer { ref message } if message.contains("direct-ro/v2 JSON"))
        );
        let runtime = slot.unwrap();
        assert_eq!(runtime.observe_occurrence(), occurrence);
        assert_eq!(runtime.export_project(&occurrence.revision).unwrap(), saved);
    }
}

#[test]
fn capacity_profile_portable_export_refuses_without_mutating_import_or_history() {
    let csv = format!("a,b,c\n{}", "x,y,z\n".repeat(320));
    let workbook =
        interop_adapter::import_csv(csv.as_bytes(), &interop_adapter::ImportOptions::default())
            .unwrap();
    let selection = ImportSelection {
        column_types: vec![vec![ImportFieldType::Text; 3]],
        extra_columns: vec![vec![]],
    };
    let (mut runtime, imported) = import_workbook(&workbook, &selection, RESIDENT).unwrap();
    assert_eq!(imported.opened.table.rows.len(), 320);
    let occurrence = runtime.observe_occurrence();
    let saved = runtime.export_project(&occurrence.revision).unwrap();
    let table_request = DesignerRequest::QueryTable {
        collection: imported.opened.table.collection.id.clone(),
    };
    let before = runtime.handle(table_request.clone()).unwrap();
    let error = runtime
        .export_portable_ro(&occurrence.revision)
        .unwrap_err();
    assert!(matches!(error, DesignerError::UnsupportedProject { .. }));
    assert_eq!(runtime.observe_occurrence(), occurrence);
    assert_eq!(runtime.export_project(&occurrence.revision).unwrap(), saved);
    assert_eq!(runtime.handle(table_request).unwrap(), before);
    let undone = runtime
        .handle(DesignerRequest::Undo {
            expected_revision: occurrence.revision,
        })
        .unwrap_err();
    assert_eq!(
        undone.failure_projection("resident/0").code,
        "invalid_tracker_operation"
    );
    assert!(
        undone
            .to_string()
            .contains("no operation is available in this history direction")
    );
}
