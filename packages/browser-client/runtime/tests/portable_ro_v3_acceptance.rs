//! Steward public-API acceptance for #519, with the same independent storage vector.
use tachiko_designer_runtime::{
    DesignerError, DesignerRequest, DesignerResponse, DesignerRuntime,
    KeyedGroupedSumGroupProjection, OpenedProjection, close_project, open_portable_ro,
    open_project, verify_portable_ro,
};
use tachiko_storage::{CanonicalRoProjectV3, encode_roproj_v3};
use tachiko_workspace_engine as semantic;
#[path = "../../../../acceptance/portable-package-v3/bytes.rs"]
mod bytes;
#[path = "../../../../acceptance/portable-package-v3/document.rs"]
mod fixture;
const FIXTURE_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../docs/research/fixtures/issue-519-portable-package-v1-v3"
);
const RESIDENT: &str = "00000000-0000-4000-8000-000000000519";
const FRESH: &str = "00000000-0000-4000-8000-000000000520";
const THIRD: &str = "00000000-0000-4000-8000-000000000521";
fn transfer(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut bytes = b"TWDPROJ1".to_vec();
    bytes.extend_from_slice(&u32::try_from(files.len()).unwrap().to_le_bytes());
    for (p, b) in files {
        bytes.extend_from_slice(&u16::try_from(p.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&u32::try_from(b.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(p.as_bytes());
        bytes.extend_from_slice(b);
    }
    bytes
}
fn prepared() -> (DesignerRuntime, OpenedProjection) {
    let mut slot = None;
    let opened = open_project(&mut slot, &transfer(&bytes::input_files()), RESIDENT)
        .expect("accepted v3 fixture must already reach runtime admission");
    (slot.unwrap(), opened)
}
fn prepared_control() -> (DesignerRuntime, OpenedProjection) {
    let mut slot = None;
    let opened = open_project(&mut slot, &transfer(&bytes::control_files()), RESIDENT)
        .expect("unconstrained Date/definition control must reach existing admission");
    (slot.unwrap(), opened)
}
fn summary(runtime: &mut DesignerRuntime) -> DesignerResponse {
    let result = runtime
        .handle(DesignerRequest::QueryKeyedGroupedSum {
            definition_id: fixture::DEFINITION_ID.into(),
        })
        .unwrap();
    let DesignerResponse::KeyedGroupedSum(ref projection) = result else {
        panic!("expected saved summary");
    };
    assert_eq!(projection.revision, "resident/0");
    assert!(projection.diagnostics.is_empty());
    assert_eq!(
        projection.groups,
        vec![
            KeyedGroupedSumGroupProjection {
                category: "Paper".into(),
                value: 1000.0
            },
            KeyedGroupedSumGroupProjection {
                category: "Stationery".into(),
                value: 800.0
            },
        ]
    );
    result
}
fn table(runtime: &mut DesignerRuntime) -> DesignerResponse {
    runtime
        .handle(DesignerRequest::QueryTable {
            collection: fixture::SCHEMA_ID.into(),
        })
        .unwrap()
}
fn assert_storage_class(error: DesignerError, expected: &str) {
    let DesignerError::Storage(error) = error else {
        panic!("expected storage class, found {error:?}");
    };
    assert!(
        error.to_string().starts_with(&format!("{expected}:")),
        "expected {expected}, actual {error}"
    );
}

#[test]
fn ac2_required_constrained_fixture_runtime_admission() {
    let tree = CanonicalRoProjectV3::try_from_files(bytes::input_files()).unwrap();
    assert_eq!(tree, encode_roproj_v3(&fixture::document()).unwrap());
    let (mut runtime, opened) = prepared();
    assert_eq!(opened.bootstrap.revision, "resident/0");
    assert_eq!(opened.table.revision, "resident/0");
    assert_eq!(
        opened.bootstrap.keyed_grouped_sum_definition_ids,
        [fixture::DEFINITION_ID]
    );
    summary(&mut runtime);
    assert_eq!(
        runtime.export_project_v3("resident/0").unwrap().bytes,
        transfer(&bytes::input_files())
    );
}

#[test]
fn ac2_export_matches_golden_and_never_mutates_resident_meaning_or_revision() {
    let (mut runtime, _) = prepared();
    let occurrence = runtime.observe_occurrence();
    let before_table = table(&mut runtime);
    let before_summary = summary(&mut runtime);
    let before_save = runtime.export_project_v3(&occurrence.revision).unwrap();
    let result = runtime.export_portable_ro(&occurrence.revision);
    assert_eq!(runtime.observe_occurrence(), occurrence);
    assert_eq!(table(&mut runtime), before_table);
    assert_eq!(summary(&mut runtime), before_summary);
    assert_eq!(
        runtime.export_project_v3(&occurrence.revision).unwrap(),
        before_save
    );
    let export = result.expect("Date + definitions + both constraints must export as .ro");
    assert_eq!(export.revision, occurrence.revision);
    assert_eq!(export.bytes, bytes::vector("golden"));
    assert_eq!(
        runtime.export_portable_ro(&occurrence.revision).unwrap(),
        export
    );
    verify_portable_ro(&export.bytes).unwrap();
    let mut fresh = None;
    open_portable_ro(&mut fresh, &export.bytes, FRESH).unwrap();
    assert_eq!(
        fresh
            .as_ref()
            .unwrap()
            .export_project_v3("resident/0")
            .unwrap()
            .bytes,
        before_save.bytes
    );
    assert_eq!(runtime.observe_occurrence(), occurrence);
}

#[test]
fn ac2_independent_golden_verifies_and_opens_in_a_fresh_runtime() {
    let golden = bytes::vector("golden");
    verify_portable_ro(&golden).expect("first-public golden must verify");
    let (mut reference, expected_opened) = prepared();
    let expected_table = table(&mut reference);
    let expected_summary = summary(&mut reference);
    let mut fresh = None;
    let opened =
        open_portable_ro(&mut fresh, &golden, FRESH).expect("fresh runtime must admit v3 package");
    assert_eq!(opened, expected_opened);
    let fresh_runtime = fresh.as_mut().unwrap();
    assert_eq!(fresh_runtime.observe_occurrence().revision, "resident/0");
    assert_ne!(
        fresh_runtime.observe_occurrence().scope,
        reference.observe_occurrence().scope
    );
    assert_eq!(table(fresh_runtime), expected_table);
    assert_eq!(summary(fresh_runtime), expected_summary);
    assert_eq!(
        fresh_runtime.export_project_v3("resident/0").unwrap().bytes,
        transfer(&bytes::input_files())
    );
    assert_eq!(
        fresh_runtime
            .export_portable_ro("resident/0")
            .unwrap()
            .bytes,
        golden
    );
    close_project(&mut fresh);
    assert!(fresh.is_none());
    let reopened = open_portable_ro(&mut fresh, &golden, THIRD).unwrap();
    assert_eq!(reopened, opened);
}

fn runtime_refusal(name: &str, expected: &str) {
    let (mut reference, _) = prepared_control();
    let occurrence = reference.observe_occurrence();
    let saved = reference.export_project_v3(&occurrence.revision).unwrap();
    let projected = table(&mut reference);
    let computed = summary(&mut reference);
    let candidate = bytes::vector(name);
    let mut slot = Some(reference);
    let result = open_portable_ro(&mut slot, &candidate, FRESH);
    let resident = slot
        .as_mut()
        .expect("refused candidate cannot clear the resident");
    assert_eq!(resident.observe_occurrence(), occurrence, "{name}");
    assert_eq!(
        resident.export_project_v3(&occurrence.revision).unwrap(),
        saved,
        "{name}"
    );
    assert_eq!(table(resident), projected, "{name}");
    assert_eq!(summary(resident), computed, "{name}");
    assert_storage_class(result.expect_err(name), expected);
    assert_storage_class(verify_portable_ro(&candidate).expect_err(name), expected);
}
macro_rules! refusal_case {
    ($test:ident, $name:literal, $class:literal) => {
        #[test]
        fn $test() {
            runtime_refusal($name, concat!("portable_package.", $class));
        }
    };
}
refusal_case!(
    ac3_developmental,
    "developmental",
    "unsupported_payload_representation"
);
refusal_case!(ac3_unknown_profile, "unknown-profile", "invalid_manifest");
refusal_case!(
    ac3_unknown_version,
    "unknown-version",
    "unsupported_version"
);
refusal_case!(
    ac3_unknown_payload_profile,
    "unknown-payload-profile",
    "unsupported_payload_representation"
);
refusal_case!(
    ac3_unknown_payload_version,
    "unknown-payload-version",
    "unsupported_payload_representation"
);
refusal_case!(
    ac3_old_payload_claims_before_body,
    "old-payload-claims-before-body",
    "unsupported_payload_representation"
);
refusal_case!(ac3_integrity, "integrity", "integrity_mismatch");
refusal_case!(ac3_missing_entry, "missing-entry", "entry_set_mismatch");
refusal_case!(ac3_extra_entry, "extra-entry", "entry_set_mismatch");
refusal_case!(ac3_duplicate_entry, "duplicate-entry", "entry_set_mismatch");
refusal_case!(ac3_alias_entry, "alias-entry", "entry_set_mismatch");
refusal_case!(ac3_entry_order, "entry-order", "noncanonical_container");
refusal_case!(ac3_crc, "crc", "crc_mismatch");
refusal_case!(
    ac3_inner_manifest,
    "inner-manifest",
    "payload_manifest_mismatch"
);
refusal_case!(
    ac3_noncanonical_payload,
    "noncanonical-payload",
    "noncanonical_payload"
);
refusal_case!(
    ac3_invalid_semantic_payload,
    "invalid-semantic-payload",
    "invalid_semantic_payload"
);
refusal_case!(ac3_compression, "compression", "noncanonical_container");
refusal_case!(ac3_encryption, "encryption", "noncanonical_container");
refusal_case!(
    ac3_data_descriptor,
    "data-descriptor",
    "noncanonical_container"
);
refusal_case!(ac3_timestamp, "timestamp", "noncanonical_container");
refusal_case!(ac3_truncated, "truncated", "invalid_container");
refusal_case!(ac3_trailing, "trailing", "invalid_container");
refusal_case!(ac3_prepended, "prepended", "invalid_container");
refusal_case!(ac3_split, "split", "invalid_container");
refusal_case!(ac3_zip64_sentinel, "zip64-sentinel", "invalid_container");
refusal_case!(
    ac3_manifest_depth_256,
    "manifest-depth-256",
    "unsupported_version"
);
refusal_case!(
    ac3_manifest_depth_257,
    "manifest-depth-257",
    "resource_limit"
);
refusal_case!(
    ac3_duplicate_manifest_member,
    "duplicate-manifest-member",
    "invalid_manifest"
);
refusal_case!(
    ac3_noncanonical_manifest,
    "noncanonical-manifest",
    "noncanonical_container"
);
refusal_case!(
    ac3_malformed_version,
    "malformed-version",
    "invalid_manifest"
);

#[test]
fn ac4_explicit_v3_export_reopen_and_stale_export_guards_remain() {
    let (runtime, expected) = prepared_control();
    let before = runtime.observe_occurrence();
    let explicit = runtime.export_project_v3(&before.revision).unwrap();
    assert_eq!(explicit.bytes, transfer(&bytes::control_files()));
    let mut fresh = None;
    assert_eq!(
        open_project(&mut fresh, &explicit.bytes, FRESH).unwrap(),
        expected
    );
    assert!(matches!(
        runtime.export_project(&before.revision),
        Err(DesignerError::UnsupportedProject { .. })
    ));
    for result in [
        runtime.export_project_v3("resident/1"),
        runtime.export_portable_ro("resident/1"),
    ] {
        assert!(matches!(result, Err(DesignerError::StaleQuery { .. })));
    }
    assert_eq!(runtime.observe_occurrence(), before);
}

#[test]
fn ac4_transfer_bound_is_identical_for_verify_and_open_without_mutation() {
    let (runtime, _) = prepared_control();
    let before = runtime.observe_occurrence();
    let saved = runtime.export_project_v3(&before.revision).unwrap();
    let mut slot = Some(runtime);
    let mut oversized = vec![0; 67_108_865];
    oversized[..4].copy_from_slice(b"PK\x03\x04");
    for result in [
        verify_portable_ro(&oversized),
        open_portable_ro(&mut slot, &oversized, FRESH).map(|_| ()),
    ] {
        assert!(matches!(
            result,
            Err(DesignerError::ProjectTransferTooLarge {
                actual: 67_108_865,
                maximum: 67_108_864
            })
        ));
    }
    oversized.pop();
    assert!(
        matches!(
            verify_portable_ro(&oversized),
            Err(DesignerError::Storage(_))
        ),
        "exactly-at-limit must reach storage framing"
    );
    assert_eq!(slot.as_ref().unwrap().observe_occurrence(), before);
    assert_eq!(
        slot.as_ref()
            .unwrap()
            .export_project_v3(&before.revision)
            .unwrap(),
        saved
    );
}

#[test]
fn unconstrained_control_reaches_the_existing_runtime_boundary() {
    let (mut runtime, opened) = prepared_control();
    assert_eq!(opened.bootstrap.revision, "resident/0");
    summary(&mut runtime);
    assert_eq!(
        runtime.export_project_v3("resident/0").unwrap().bytes,
        transfer(&bytes::control_files())
    );
}

#[test]
fn ac2_unconstrained_date_definition_control_exports_and_reopens_exactly() {
    let (mut runtime, expected_opened) = prepared_control();
    let before = runtime.observe_occurrence();
    let native = runtime.export_project_v3(&before.revision).unwrap();
    let projected = table(&mut runtime);
    let computed = summary(&mut runtime);
    let result = runtime.export_portable_ro(&before.revision);
    assert_eq!(runtime.observe_occurrence(), before);
    assert_eq!(runtime.export_project_v3(&before.revision).unwrap(), native);
    assert_eq!(table(&mut runtime), projected);
    assert_eq!(summary(&mut runtime), computed);
    let export = result.expect("unconstrained v3 Date/definition control must export");
    assert_eq!(export.bytes, bytes::vector("runtime-control"));
    assert_eq!(export.revision, before.revision);
    verify_portable_ro(&export.bytes).unwrap();
    let mut fresh = None;
    assert_eq!(
        open_portable_ro(&mut fresh, &export.bytes, FRESH).unwrap(),
        expected_opened
    );
    let fresh = fresh.as_mut().unwrap();
    assert_eq!(fresh.export_project_v3("resident/0").unwrap(), native);
    assert_eq!(fresh.export_portable_ro("resident/0").unwrap(), export);
    assert_eq!(table(fresh), projected);
    assert_eq!(summary(fresh), computed);
}

#[test]
fn ac4_csv_xlsx_lossy_on_export_inventory_is_unchanged_and_read_only() {
    use tachiko_designer_runtime::interop_adapter::{
        CellStyle, FidelityCategory, FidelityFinding, export_csv, export_xlsx,
    };
    for has_definition in [false, true] {
        let mut document = fixture::document();
        for schema in document.schemas.values_mut() {
            for field in schema.fields.values_mut() {
                field.constraint = semantic::FieldConstraint::None;
            }
        }
        if !has_definition {
            document.keyed_grouped_sum_definitions.clear();
        }
        let tree = encode_roproj_v3(&document).unwrap();
        let files = tree
            .files()
            .iter()
            .map(|f| (f.path().to_owned(), f.bytes().to_vec()))
            .collect::<Vec<_>>();
        let mut slot = None;
        open_project(&mut slot, &transfer(&files), RESIDENT).unwrap();
        let runtime = slot.as_mut().unwrap();
        let metadata: tachiko_designer_runtime::InteropMetadata = serde_json::from_value(serde_json::json!({
            "version": 1, "sheets": [{"schema_id": fixture::SCHEMA_ID, "name": "Sheet 1", "has_header": true,
                "columns": fixture::FIELD_IDS.iter().enumerate().map(|(i,id)| serde_json::json!({
                    "field_id": id, "name": format!("Column {}", i+1), "width": null
                })).collect::<Vec<_>>(),
                "rows": fixture::ROW_IDS.iter().map(|id| serde_json::json!({
                    "entity_id": id, "styles": vec![CellStyle::default();5]
                })).collect::<Vec<_>>()
            }]
        })).unwrap();
        let before = runtime.observe_occurrence();
        let native = runtime.export_project_v3(&before.revision).unwrap();
        let projected = table(runtime);
        let workbook = runtime
            .export_workbook(&before.revision, &metadata)
            .unwrap();
        let mut ledger = vec![FidelityFinding {
            category: FidelityCategory::Converted, code: "bound_formula_absolute_a1".into(), location: "workbook".into(),
            message: "Export is rebuilt from current stable identities and authoritative values. Formula references use current absolute worksheet coordinates; deleted rows are omitted.".into(), blocking: false,
        }];
        if has_definition {
            ledger.push(FidelityFinding { category: FidelityCategory::LossyOnExport,
                code: "saved_keyed_grouped_sum_definitions_not_exported".into(), location: "workbook".into(),
                message: "Saved grouped-summary definitions are not included in this spreadsheet export. Keep a Tachiko saved copy to preserve their bindings and recomputation.".into(), blocking: false,
            });
        }
        assert_eq!(workbook.ledger, ledger);
        let csv = export_csv(&workbook.sheets[0]).unwrap();
        let xlsx = export_xlsx(&workbook).unwrap();
        for _ in 0..2 {
            let repeated = runtime
                .export_workbook(&before.revision, &metadata)
                .unwrap();
            assert_eq!(repeated, workbook);
            assert_eq!(export_csv(&repeated.sheets[0]).unwrap(), csv);
            assert_eq!(export_xlsx(&repeated).unwrap(), xlsx);
        }
        assert_eq!(runtime.observe_occurrence(), before);
        assert_eq!(table(runtime), projected);
        assert_eq!(runtime.export_project_v3(&before.revision).unwrap(), native);
        if has_definition {
            summary(runtime);
        }
        let mut reopened = None;
        open_project(&mut reopened, &native.bytes, FRESH).unwrap();
        assert_eq!(
            reopened
                .as_ref()
                .unwrap()
                .export_workbook("resident/0", &metadata)
                .unwrap(),
            workbook
        );
    }
}

#[test]
fn ac2_changed_revision_export_preserves_history_and_fresh_revision_projection() {
    use tachiko_designer_runtime::{FieldTarget, ScalarEditInput};
    let (mut runtime, _) = prepared_control();
    let original = runtime.export_project_v3("resident/0").unwrap();
    runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/0".into(),
            target: FieldTarget {
                entity: fixture::ROW_IDS[0].into(),
                field: fixture::FIELD_IDS[2].into(),
            },
            input: ScalarEditInput::Number { input: "6".into() },
        })
        .unwrap();
    let occurrence = runtime.observe_occurrence();
    assert_eq!(occurrence.revision, "resident/1");
    let current = runtime.export_project_v3(&occurrence.revision).unwrap();
    let projected = table(&mut runtime);
    let computed = runtime
        .handle(DesignerRequest::QueryKeyedGroupedSum {
            definition_id: fixture::DEFINITION_ID.into(),
        })
        .unwrap();
    let result = runtime.export_portable_ro(&occurrence.revision);
    assert_eq!(runtime.observe_occurrence(), occurrence);
    assert_eq!(
        runtime.export_project_v3(&occurrence.revision).unwrap(),
        current
    );
    assert_eq!(table(&mut runtime), projected);
    assert_eq!(
        runtime
            .handle(DesignerRequest::QueryKeyedGroupedSum {
                definition_id: fixture::DEFINITION_ID.into()
            })
            .unwrap(),
        computed
    );
    let export = result.expect("current nonzero revision must export as v3 package");
    assert_eq!(export.revision, "resident/1");
    assert!(matches!(
        runtime.export_portable_ro("resident/0"),
        Err(DesignerError::StaleQuery { .. })
    ));
    let mut fresh = None;
    let opened = open_portable_ro(&mut fresh, &export.bytes, FRESH).unwrap();
    assert_eq!(opened.bootstrap.revision, "resident/0");
    let fresh_runtime = fresh.as_mut().unwrap();
    assert_eq!(
        fresh_runtime.export_project_v3("resident/0").unwrap().bytes,
        current.bytes
    );
    assert_changed_fresh_projections(fresh_runtime, projected, computed);
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: occurrence.revision,
        })
        .unwrap();
    assert_eq!(
        runtime
            .export_project_v3(&runtime.observe_occurrence().revision)
            .unwrap()
            .bytes,
        original.bytes
    );
    runtime
        .handle(DesignerRequest::Redo {
            expected_revision: runtime.observe_occurrence().revision,
        })
        .unwrap();
    assert_eq!(
        runtime
            .export_project_v3(&runtime.observe_occurrence().revision)
            .unwrap()
            .bytes,
        current.bytes
    );
}

fn assert_changed_fresh_projections(
    fresh_runtime: &mut DesignerRuntime,
    projected: DesignerResponse,
    computed: DesignerResponse,
) {
    let DesignerResponse::Table(mut expected_table) = projected else {
        panic!("table projection required");
    };
    expected_table.revision = "resident/0".into();
    assert_eq!(
        table(fresh_runtime),
        DesignerResponse::Table(expected_table)
    );
    let DesignerResponse::KeyedGroupedSum(mut expected_summary) = computed else {
        panic!("summary projection required");
    };
    assert_eq!(
        expected_summary.groups,
        vec![
            KeyedGroupedSumGroupProjection {
                category: "Paper".into(),
                value: 1000.0
            },
            KeyedGroupedSumGroupProjection {
                category: "Stationery".into(),
                value: 1200.0
            },
        ]
    );
    expected_summary.revision = "resident/0".into();
    assert_eq!(
        fresh_runtime
            .handle(DesignerRequest::QueryKeyedGroupedSum {
                definition_id: fixture::DEFINITION_ID.into()
            })
            .unwrap(),
        DesignerResponse::KeyedGroupedSum(expected_summary)
    );
}
