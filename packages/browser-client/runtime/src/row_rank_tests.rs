use super::*;
use crate::interop_adapter::{
    CellStyle, SourceCell, SourceColumn, SourceLayout, SourceSheet, SourceValue, SourceWorkbook,
};
use tachiko_semantic_core::RowRank;

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000871";
const NEXT_OCCURRENCE: &str = "00000000-0000-4000-8000-000000000872";

fn imported_fixture() -> (DesignerRuntime, InteropMetadata) {
    let source = SourceWorkbook {
        ledger: Vec::new(),
        sheets: vec![SourceSheet {
            name: "Input".into(),
            has_header: true,
            layout: SourceLayout::default(),
            columns: vec![SourceColumn {
                name: "Value".into(),
                width: None,
                number_format: None,
            }],
            rows: vec![vec![SourceCell {
                value: SourceValue::Number { value: 23.0 },
                formula: None,
                style: CellStyle::default(),
            }]],
        }],
    };
    let selection = ImportSelection {
        column_types: vec![vec![ImportFieldType::Number]],
        extra_columns: vec![vec![]],
    };
    let (runtime, projection) = import_workbook(&source, &selection, OCCURRENCE).unwrap();
    (runtime, projection.metadata)
}

fn assert_rank_refusal<T>(result: Result<T, DesignerError>) {
    let Err(error) = result else {
        panic!("ranked input was admitted")
    };
    assert!(error.to_string().contains("Row rank"), "{error}");
}

#[test]
fn ranked_v3_runtime_admission_remains_refused_until_order_consumers_land() {
    let (runtime, metadata) = imported_fixture();
    let original = runtime.session.export_snapshot().document().clone();
    let mut document = original.clone();
    document.entities.values_mut().next().unwrap().rank = Some(RowRank::new("A").unwrap());
    let tree = tachiko_storage::encode_roproj_v3(&document).unwrap();
    let bundle = encode_project_bundle(tree.files().iter().map(|f| (f.path(), f.bytes()))).unwrap();
    let portable = encode_portable_package_v1(&tree).unwrap();
    let mut resident = Some(runtime);
    let revision = resident.as_ref().unwrap().current_revision().to_owned();
    let scope = resident.as_ref().unwrap().occurrence_scope().to_owned();
    // Every replacing route must fail before publishing a new occurrence.
    macro_rules! refused_without_replacement {
        ($result:expr) => {
            assert_rank_refusal($result);
            let current = resident.as_ref().unwrap();
            assert_eq!(current.current_revision(), revision);
            assert_eq!(current.occurrence_scope(), scope);
            assert_eq!(current.session.export_snapshot().document(), &original);
        };
    }
    refused_without_replacement!(open_project(&mut resident, &bundle, NEXT_OCCURRENCE));
    refused_without_replacement!(open_project_working_size(
        &mut resident,
        &bundle,
        NEXT_OCCURRENCE
    ));
    refused_without_replacement!(open_portable_ro(&mut resident, &portable, NEXT_OCCURRENCE));
    refused_without_replacement!(open_portable_ro_working_size(
        &mut resident,
        &portable,
        NEXT_OCCURRENCE
    ));
    refused_without_replacement!(open_local_document(
        &mut resident,
        &portable,
        NEXT_OCCURRENCE
    ));
    refused_without_replacement!(open_local_document_working_size(
        &mut resident,
        &portable,
        NEXT_OCCURRENCE
    ));
    refused_without_replacement!(open_imported_project(
        &mut resident,
        &bundle,
        &metadata,
        NEXT_OCCURRENCE
    ));
    assert_rank_refusal(admit_imported_project_working_size(
        &bundle,
        NEXT_OCCURRENCE,
        &metadata,
    ));
    assert_rank_refusal(inspect_project(&bundle));
    assert_rank_refusal(inspect_project_working_size(&bundle));
    assert_rank_refusal(DesignerRuntime::from_document(
        document.clone(),
        NEXT_OCCURRENCE,
    ));
    assert_rank_refusal(DesignerRuntime::from_document_working_size(
        document.clone(),
        NEXT_OCCURRENCE,
    ));
    assert_rank_refusal(DesignerRuntime::from_document_profile(
        document.clone(),
        NEXT_OCCURRENCE,
        SessionProfile::TextCapacity,
    ));
    let extra_bytes = u64::try_from("\"rank\":\"A\",".len()).unwrap();
    assert_eq!(
        save_size::saved_bytes_upper_bound(&document),
        save_size::saved_bytes_upper_bound(&original) + extra_bytes
    );
}
