use std::collections::BTreeMap;

use tachiko_storage::{decode_portable_package_v1, encode_portable_package_v1, encode_roproj_v3};
use tachiko_workspace_engine::Document;

use crate::interop_adapter::{
    CellStyle, SourceCell, SourceColumn, SourceLayout, SourceSheet, SourceValue, SourceWorkbook,
    export_xlsx, import_xlsx,
};
use crate::{
    DesignerRequest, DesignerResponse, DesignerRuntime, FieldTarget, ImportFieldType,
    ImportSelection, InteropMetadata, NewTableColumnInput, ProjectInspectionProjection,
    import_workbook, import_workbook_working_size, open_imported_project, open_project,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000859";

pub(super) fn fixture(formula: bool) -> SourceWorkbook {
    SourceWorkbook {
        ledger: Vec::new(),
        sheets: ["Input", "Scratch"]
            .into_iter()
            .enumerate()
            .map(|(index, name)| SourceSheet {
                name: name.into(),
                has_header: true,
                layout: SourceLayout::default(),
                columns: vec![SourceColumn {
                    name: "Value".into(),
                    width: None,
                    number_format: None,
                }],
                rows: vec![vec![SourceCell {
                    value: SourceValue::Number {
                        value: if index == 0 { 23.0 } else { 46.0 },
                    },
                    formula: (formula && index == 1).then(|| "'Input'!A2*2".into()),
                    style: CellStyle::default(),
                }]],
            })
            .collect(),
    }
}

pub(super) fn selection() -> ImportSelection {
    ImportSelection {
        column_types: vec![vec![ImportFieldType::Number]; 2],
        extra_columns: vec![vec![]; 2],
    }
}

fn imported(formula: bool) -> (DesignerRuntime, InteropMetadata) {
    // Exercise the real XLSX codec and the real typed-import path.
    let source = import_xlsx(&export_xlsx(&fixture(formula)).unwrap()).unwrap();
    let (runtime, projection) = import_workbook(&source, &selection(), OCCURRENCE).unwrap();
    (runtime, projection.metadata)
}

fn inspect_current(runtime: &mut DesignerRuntime) -> ProjectInspectionProjection {
    let DesignerResponse::ProjectInspected(inspection) =
        runtime.handle(DesignerRequest::InspectProject).unwrap()
    else {
        panic!("expected resident project inspection");
    };
    inspection
}

#[test]
fn inspect_recovers_lost_rename_mapping_for_xlsx_export() {
    let (mut runtime, _) = imported(false);
    let published = runtime
        .rename_table("resident/0", "sheet_1", "Renamed Input")
        .unwrap();
    let inspection = inspect_current(&mut runtime);
    assert_eq!(inspection.revision, "resident/1");
    assert_eq!(inspection.import_metadata, published.import_metadata);
    let recovered = inspection.import_metadata.unwrap();
    let workbook = import_xlsx(&xlsx(&runtime, &recovered)).unwrap();
    assert_eq!(workbook.sheets.len(), 2);
    assert_eq!(workbook.sheets[0].name, "Renamed Input");
}

#[test]
fn inspect_recovers_post_undo_mapping_for_xlsx_export() {
    let (mut runtime, original) = imported(false);
    runtime
        .rename_table("resident/0", "sheet_1", "Renamed Input")
        .unwrap();
    let undone = runtime.history_edit("resident/1", false).unwrap();
    let inspection = inspect_current(&mut runtime);
    assert_eq!(inspection.revision, "resident/2");
    assert_eq!(inspection.import_metadata, undone.import_metadata);
    assert_eq!(inspection.import_metadata.as_ref(), Some(&original));
    let workbook = import_xlsx(&xlsx(&runtime, &inspection.import_metadata.unwrap())).unwrap();
    assert_eq!(workbook.sheets[0].name, "Input");
}

#[test]
fn inspect_omits_mapping_and_preserves_untouched_import_and_native_replies() {
    let (mut runtime, _) = imported(false);
    let before = snapshot(&runtime);
    let bootstrap = serde_json::to_vec(&runtime.bootstrap_projection()).unwrap();
    assert_eq!(
        serde_json::to_vec(&inspect_current(&mut runtime)).unwrap(),
        br#"{"revision":"resident/0"}"#,
    );
    assert_eq!(
        serde_json::to_vec(&runtime.bootstrap_projection()).unwrap(),
        bootstrap
    );
    assert_eq!(snapshot(&runtime), before);
    let mut native = DesignerRuntime::moonfall(OCCURRENCE).unwrap();
    assert!(inspect_current(&mut native).import_metadata.is_none());
}

fn columns() -> Vec<NewTableColumnInput> {
    vec![NewTableColumnInput {
        name: "Value".into(),
        field_type: "number".into(),
        required: false,
    }]
}

fn add(runtime: &mut DesignerRuntime) -> String {
    let revision = runtime.current_revision().to_owned();
    runtime
        .add_table(&revision, "A Added", &columns())
        .unwrap()
        .collection
        .key
}

fn names(runtime: &DesignerRuntime) -> Vec<String> {
    runtime
        .bootstrap_projection()
        .collections
        .into_iter()
        .map(|table| table.display_name)
        .collect()
}

fn xlsx(runtime: &DesignerRuntime, metadata: &InteropMetadata) -> Vec<u8> {
    export_xlsx(
        &runtime
            .export_workbook(runtime.current_revision(), metadata)
            .unwrap(),
    )
    .unwrap()
}

#[derive(Debug, PartialEq)]
struct Snapshot {
    document: Document,
    names: Vec<String>,
    mapping: serde_json::Value,
    ro: Vec<u8>,
    xlsx: Vec<u8>,
}

fn snapshot(runtime: &DesignerRuntime) -> Snapshot {
    let ro = runtime
        .export_portable_ro(runtime.current_revision())
        .unwrap()
        .bytes;
    let metadata = &runtime.imported.as_ref().unwrap().metadata;
    Snapshot {
        document: runtime.session.document().clone(),
        names: names(runtime),
        mapping: serde_json::to_value(metadata).unwrap(),
        ro,
        xlsx: xlsx(runtime, metadata),
    }
}

fn history_round_trip(runtime: &mut DesignerRuntime, before: &Snapshot) {
    let after = snapshot(runtime);
    assert_ne!(&after, before);
    let undo_len = runtime.undo.len();
    let revision = runtime.current_revision().to_owned();
    let serial = revision
        .strip_prefix("resident/")
        .unwrap()
        .parse::<usize>()
        .unwrap();
    let undo = runtime.history_edit(&revision, false).unwrap();
    assert_eq!(
        serde_json::to_value(undo.import_metadata.unwrap()).unwrap(),
        before.mapping
    );
    assert_eq!(
        runtime.current_revision(),
        format!("resident/{}", serial + 1)
    );
    assert_eq!(runtime.undo.len(), undo_len - 1);
    assert_eq!(&snapshot(runtime), before);
    let revision = runtime.current_revision().to_owned();
    let redo = runtime.history_edit(&revision, true).unwrap();
    assert_eq!(
        serde_json::to_value(redo.import_metadata.unwrap()).unwrap(),
        after.mapping
    );
    assert_eq!(
        runtime.current_revision(),
        format!("resident/{}", serial + 2)
    );
    assert_eq!(snapshot(runtime), after);
}

#[test]
fn untouched_import_keeps_main_names_client_metadata_and_history_replies() {
    let (mut runtime, mut metadata) = imported(false);
    assert_eq!(names(&runtime), ["sheet_1", "sheet_2"]);
    metadata.sheets[0].name = "Client Input".into();
    let workbook = runtime.export_workbook("resident/0", &metadata).unwrap();
    assert_eq!(workbook.sheets[0].name, "Client Input");
    let target = runtime.query_table("sheet_1").unwrap().rows[0].fields[0]
        .target
        .clone();
    let edited = runtime
        .edit_cells(
            "resident/0",
            &[crate::CellEdit {
                target,
                input: crate::ScalarEditInput::Number { input: "31".into() },
            }],
        )
        .unwrap();
    assert!(
        serde_json::to_value(edited)
            .unwrap()
            .get("import_metadata")
            .is_none()
    );
    let undone = runtime.history_edit("resident/1", false).unwrap();
    assert!(
        serde_json::to_value(undone)
            .unwrap()
            .get("import_metadata")
            .is_none()
    );
    assert_eq!(
        runtime
            .export_workbook("resident/2", &metadata)
            .unwrap()
            .sheets[0]
            .name,
        "Client Input"
    );
}

#[test]
fn lifecycle_export_compares_identity_names_and_undo_restores_client_authority() {
    let (mut runtime, metadata) = imported(false);
    let renamed = runtime
        .rename_table("resident/0", "sheet_1", "Renamed Input")
        .unwrap();
    let mut published = renamed.import_metadata.unwrap();
    // Presentation and wire-version differences do not make an identity mapping stale.
    published.sheets[0].columns[0].width = Some(99.0);
    published.version = 2;
    let projected = runtime.export_workbook("resident/1", &published).unwrap();
    assert_eq!(projected.sheets[0].name, "Renamed Input");
    assert_ne!(projected.sheets[0].columns[0].width, Some(99.0));
    let undone = runtime.history_edit("resident/1", false).unwrap();
    assert_eq!(undone.import_metadata.unwrap(), metadata);
    assert!(!runtime.imported.as_ref().unwrap().lifecycle_changed);
    let mut client_metadata = metadata;
    client_metadata.sheets[0].name = "Client Input".into();
    assert_eq!(
        runtime
            .export_workbook("resident/2", &client_metadata)
            .unwrap()
            .sheets[0]
            .name,
        "Client Input"
    );
    runtime.history_edit("resident/2", true).unwrap();
    assert!(matches!(
        runtime.export_workbook("resident/3", &client_metadata),
        Err(crate::DesignerError::ImportedSpreadsheetExport { .. })
    ));
}

#[test]
fn imported_duplicate_undo_restores_client_authority() {
    let (mut runtime, metadata) = imported(false);
    let DesignerResponse::Duplicated(_) = runtime
        .handle(DesignerRequest::DuplicateCollection {
            expected_revision: "resident/0".to_owned(),
            collection: "sheet_1".to_owned(),
            name: "Input Copy".to_owned(),
            column_names: BTreeMap::new(),
        })
        .unwrap()
    else {
        panic!("duplicate returns a duplicated projection");
    };
    assert!(runtime.imported.as_ref().unwrap().lifecycle_changed);
    let undone = runtime.history_edit("resident/1", false).unwrap();
    assert_eq!(undone.import_metadata.unwrap(), metadata);
    assert!(!runtime.imported.as_ref().unwrap().lifecycle_changed);
    let mut client_metadata = metadata;
    client_metadata.sheets[0].name = "Client Input".into();
    assert_eq!(
        runtime
            .export_workbook("resident/2", &client_metadata)
            .unwrap()
            .sheets[0]
            .name,
        "Client Input"
    );
}

#[test]
fn imported_rename_history_restores_authoritative_names_metadata_and_export() {
    let (mut runtime, metadata) = imported(true);
    let before = snapshot(&runtime);
    let publication = runtime
        .rename_table("resident/0", "sheet_1", "Renamed Input")
        .unwrap();
    let published = publication.import_metadata.unwrap();
    assert_eq!(published.sheets[0].name, "Renamed Input");
    let stale = runtime
        .export_workbook("resident/1", &metadata)
        .unwrap_err();
    assert!(matches!(
        stale,
        crate::DesignerError::ImportedSpreadsheetExport { .. }
    ));
    assert_eq!(runtime.current_revision(), "resident/1");
    assert_eq!(runtime.undo.len(), 1);
    let book = import_xlsx(&xlsx(&runtime, &published)).unwrap();
    assert_eq!(book.sheets[0].name, "Renamed Input");
    assert!(
        book.sheets[1].rows[0][0]
            .formula
            .as_ref()
            .unwrap()
            .contains("'Renamed Input'!$A$2")
    );
    history_round_trip(&mut runtime, &before);
}

#[test]
fn imported_add_history_restores_mapping_and_appends_after_source_sheets() {
    let (mut runtime, _) = imported(false);
    let before = snapshot(&runtime);
    add(&mut runtime);
    assert_eq!(runtime.current_revision(), "resident/1");
    assert_eq!(runtime.undo.len(), 1);
    assert_eq!(names(&runtime), ["sheet_1", "sheet_2", "A Added"]);
    history_round_trip(&mut runtime, &before);
}

#[test]
fn imported_delete_history_restores_source_position_and_presentation() {
    let mut source = fixture(false);
    source.sheets[0].rows[0][0].style.bold = true;
    source.sheets[0].rows[0][0].style.fill = Some("FFCCAA".into());
    let source = import_xlsx(&export_xlsx(&source).unwrap()).unwrap();
    let (mut runtime, projection) = import_workbook(&source, &selection(), OCCURRENCE).unwrap();
    let metadata = projection.metadata;
    let before = snapshot(&runtime);
    runtime.delete_table("resident/0", "sheet_1").unwrap();
    assert_eq!(runtime.current_revision(), "resident/1");
    assert_eq!(runtime.undo.len(), 1);
    assert_eq!(names(&runtime), ["sheet_2"]);
    history_round_trip(&mut runtime, &before);
    let undo = runtime.history_edit("resident/3", false).unwrap();
    assert_eq!(undo.import_metadata.unwrap(), metadata);
}

fn add_formula(runtime: &mut DesignerRuntime, collection: &str) -> FieldTarget {
    let revision = runtime.current_revision().to_owned();
    let field = runtime.query_table(collection).unwrap().columns[0]
        .id
        .clone();
    runtime
        .insert_row(
            None,
            &revision,
            collection,
            &[crate::RowInitializer {
                field,
                input: crate::ScalarEditInput::Number { input: "0".into() },
            }],
        )
        .unwrap();
    let table = runtime.query_table(collection).unwrap();
    let target = table.rows[0].fields[0].target.clone();
    let revision = runtime.current_revision().to_owned();
    runtime
        .update_formula(&revision, &target, "[sheet_1_row_1.column_1]*2")
        .unwrap();
    target
}

fn assert_recalculation(runtime: &mut DesignerRuntime, metadata: &InteropMetadata) {
    let table = runtime.query_table("sheet_1").unwrap();
    let revision = runtime.current_revision().to_owned();
    runtime
        .edit_cells(
            &revision,
            &[crate::CellEdit {
                target: table.rows[0].fields[0].target.clone(),
                input: crate::ScalarEditInput::Number { input: "31".into() },
            }],
        )
        .unwrap();
    let book = runtime
        .export_workbook(runtime.current_revision(), metadata)
        .unwrap();
    assert_eq!(
        book.sheets[1].rows[0][0].value,
        SourceValue::Number { value: 62.0 }
    );
}

#[test]
fn imported_lifecycle_export_companion_reopens_and_exports_bound_formulas() {
    let (mut runtime, _) = imported(false);
    let added = add(&mut runtime);
    add_formula(&mut runtime, &added);
    runtime
        .rename_table("resident/3", "sheet_1", "Renamed Input")
        .unwrap();
    let deleted = runtime.delete_table("resident/4", "sheet_2").unwrap();
    let metadata = deleted.import_metadata.unwrap();
    assert_eq!(names(&runtime), ["Renamed Input", "A Added"]);
    let exported = xlsx(&runtime, &metadata);
    let book = import_xlsx(&exported).unwrap();
    assert_eq!(
        book.sheets
            .iter()
            .map(|sheet| sheet.name.as_str())
            .collect::<Vec<_>>(),
        ["Renamed Input", "A Added"]
    );
    assert!(
        book.sheets[1].rows[0][0]
            .formula
            .as_ref()
            .unwrap()
            .contains("'Renamed Input'!$A$2")
    );
    let portable = runtime.export_portable_ro("resident/5").unwrap();
    let companion = portable.import_metadata.unwrap();
    assert_eq!(companion, metadata);
    let verified = decode_portable_package_v1(&portable.bytes).unwrap();
    let tree = verified.tree();
    let manifest: serde_json::Value =
        serde_json::from_slice(tree.file("manifest.json").unwrap()).unwrap();
    assert!(manifest.get("import_metadata").is_none());
    let saved = runtime.export_project_v3("resident/5").unwrap();
    assert_eq!(saved.import_metadata.as_ref(), Some(&companion));
    let mut reopened = None;
    open_imported_project(&mut reopened, &saved.bytes, &companion, OCCURRENCE).unwrap();
    assert_eq!(
        names(reopened.as_ref().unwrap()),
        ["Renamed Input", "A Added"]
    );
    let opened =
        open_imported_project(&mut reopened, &portable.bytes, &companion, OCCURRENCE).unwrap();
    assert_eq!(
        opened
            .bootstrap
            .collections
            .iter()
            .map(|table| table.display_name.as_str())
            .collect::<Vec<_>>(),
        ["Renamed Input", "A Added"]
    );
    let mut reopened = reopened.unwrap();
    assert_eq!(names(&reopened), ["Renamed Input", "A Added"]);
    assert_eq!(
        reopened
            .export_portable_ro("resident/0")
            .unwrap()
            .import_metadata,
        Some(companion)
    );
    let extra = reopened
        .add_table("resident/0", "Z Extra", &columns())
        .unwrap();
    reopened
        .delete_table("resident/1", &extra.collection.key)
        .unwrap();
    // Both the initial reply and later structural refreshes retain source order.
    assert_eq!(names(&reopened), ["Renamed Input", "A Added"]);
    assert_eq!(xlsx(&reopened, &metadata), exported);
    assert_recalculation(&mut reopened, &metadata);
    let (mut from_xlsx, projection) = import_workbook(&book, &selection(), OCCURRENCE).unwrap();
    assert_eq!(names(&from_xlsx), ["sheet_1", "sheet_2"]);
    assert_recalculation(&mut from_xlsx, &projection.metadata);
}

#[test]
fn untouched_import_and_old_portable_bytes_stay_exact() {
    let (runtime, metadata) = imported(false);
    let tree = encode_roproj_v3(runtime.session.document()).unwrap();
    let old_ro = encode_portable_package_v1(&tree).unwrap();
    let exported = runtime.export_portable_ro("resident/0").unwrap();
    assert_eq!(exported.bytes, old_ro);
    assert_eq!(exported.import_metadata.as_ref(), Some(&metadata));
    let mut reopened = None;
    crate::open_portable_ro(&mut reopened, &old_ro, OCCURRENCE).unwrap();
    let native_export = reopened.unwrap().export_portable_ro("resident/0").unwrap();
    assert_eq!(native_export.bytes, old_ro);
    assert!(native_export.import_metadata.is_none());
    let receipt = crate::ProjectExportProjection {
        revision: native_export.revision,
        byte_length: native_export.bytes.len(),
        import_metadata: native_export.import_metadata,
    };
    assert!(
        serde_json::to_value(receipt)
            .unwrap()
            .get("import_metadata")
            .is_none()
    );
}

fn assert_import_metadata_host_changes(
    export: fn(
        &DesignerRuntime,
        &str,
        Option<&InteropMetadata>,
    ) -> Result<crate::ProjectExport, crate::DesignerError>,
    legacy_export: bool,
) {
    let (mut runtime, mut metadata) = imported(false);
    metadata.sheets[0].name = "Host Renamed Input".into();
    metadata.sheets[0].columns[0].width = Some(24.0);

    // Host-only edits are accepted without a worksheet lifecycle publication.
    assert_eq!(runtime.metadata_for_export(&metadata).unwrap(), &metadata);
    let expected = runtime.export_workbook("resident/0", &metadata).unwrap();
    assert_eq!(expected.sheets[0].name, "Host Renamed Input");
    assert_eq!(expected.sheets[0].columns[0].width, Some(24.0));
    let expected_xlsx = xlsx(&runtime, &metadata);

    let saved = export(&runtime, "resident/0", Some(&metadata)).unwrap();
    let companion = saved.import_metadata.unwrap();
    assert_eq!(companion, metadata);
    let mut reopened = None;
    open_imported_project(&mut reopened, &saved.bytes, &companion, OCCURRENCE).unwrap();
    let reopened = reopened.unwrap();
    assert_eq!(
        reopened.export_workbook("resident/0", &companion).unwrap(),
        expected
    );
    assert_eq!(xlsx(&reopened, &companion), expected_xlsx);

    // Legacy exports cannot represent display names, so exercise deletion there.
    // V3 exports exercise rename, add and delete lifecycle authority as well.
    let deleted = if legacy_export {
        runtime.delete_table("resident/0", "sheet_2").unwrap()
    } else {
        let renamed = runtime
            .rename_table("resident/0", "sheet_1", "Runtime Name")
            .unwrap();
        assert_eq!(
            renamed.import_metadata.unwrap().sheets[0].name,
            "Runtime Name"
        );
        let added = runtime
            .add_table("resident/1", "Added", &columns())
            .unwrap();
        runtime
            .delete_table("resident/2", &added.collection.key)
            .unwrap()
    };
    let authoritative = deleted.import_metadata.unwrap();
    let revision = runtime.current_revision();
    let mut host_presentation = authoritative.clone();
    host_presentation.sheets[0].columns[0].width = Some(40.0);
    assert_eq!(
        export(&runtime, revision, Some(&host_presentation))
            .unwrap()
            .import_metadata,
        Some(authoritative.clone()),
    );
    assert_eq!(
        export(&runtime, revision, None).unwrap().import_metadata,
        Some(authoritative)
    );
    assert!(export(&runtime, revision, Some(&metadata)).is_err());
}

#[test]
fn import_metadata_host_changes_export_project() {
    assert_import_metadata_host_changes(DesignerRuntime::export_project_with_metadata, true);
}

#[test]
fn import_metadata_host_changes_export_project_v3() {
    assert_import_metadata_host_changes(DesignerRuntime::export_project_v3_with_metadata, false);
}

#[test]
fn import_metadata_host_changes_export_portable_ro() {
    assert_import_metadata_host_changes(DesignerRuntime::export_portable_ro_with_metadata, false);
}

fn assert_invalid_export_companion(
    runtime: &DesignerRuntime,
    metadata: &InteropMetadata,
    location: &str,
    message: &str,
) {
    for export in [
        DesignerRuntime::export_project_with_metadata,
        DesignerRuntime::export_project_v3_with_metadata,
        DesignerRuntime::export_portable_ro_with_metadata,
    ] {
        let crate::DesignerError::ImportedSpreadsheetExport { finding } =
            export(runtime, "resident/0", Some(metadata)).unwrap_err()
        else {
            panic!("expected typed import metadata export error");
        };
        assert_eq!(finding.code, "imported_spreadsheet_unrepresentable");
        assert_eq!(finding.location, location);
        assert_eq!(finding.message, message);
        assert!(finding.blocking);
    }
}

#[test]
fn project_exports_reject_zero_host_column_width() {
    let (runtime, mut metadata) = imported(false);
    metadata.sheets[0].columns[0].width = Some(0.0);
    let location = format!("collection {}", metadata.sheets[0].schema_id);
    assert_invalid_export_companion(
        &runtime,
        &metadata,
        &location,
        "Invalid source width.",
    );
}

#[test]
fn project_exports_reject_duplicate_host_worksheet_names() {
    let (runtime, mut metadata) = imported(false);
    metadata.sheets[1].name = metadata.sheets[0].name.clone();
    let location = format!("collection {}", metadata.sheets[1].schema_id);
    assert_invalid_export_companion(
        &runtime,
        &metadata,
        &location,
        "Duplicate worksheet or invalid source column mapping.",
    );
}

#[test]
fn working_size_companion_reopens_portable_with_source_order() {
    let source = import_xlsx(&export_xlsx(&fixture(false)).unwrap()).unwrap();
    let (mut runtime, _) = import_workbook_working_size(&source, &selection(), OCCURRENCE).unwrap();
    add(&mut runtime);
    runtime
        .rename_table("resident/1", "sheet_1", "Renamed Input")
        .unwrap();
    runtime.delete_table("resident/2", "sheet_2").unwrap();
    let exported = runtime.export_portable_ro("resident/3").unwrap();
    let companion = exported.import_metadata.unwrap();
    let (reopened, _) =
        crate::admit_imported_project_working_size(&exported.bytes, OCCURRENCE, &companion)
            .unwrap();
    assert_eq!(names(&reopened), ["Renamed Input", "A Added"]);
    assert_eq!(xlsx(&reopened, &companion), xlsx(&runtime, &companion));
}

#[test]
fn imported_delete_refusals_leave_revision_mapping_and_history_unchanged() {
    let (mut runtime, _) = imported(true);
    let before = snapshot(&runtime);
    let error = runtime.delete_table("resident/0", "sheet_1").unwrap_err();
    assert!(matches!(
        error,
        crate::DesignerError::TableReferenced { .. }
    ));
    assert_eq!(runtime.current_revision(), "resident/0");
    assert!(runtime.undo.is_empty());
    assert_eq!(snapshot(&runtime), before);
    runtime.delete_table("resident/0", "sheet_2").unwrap();
    let before = snapshot(&runtime);
    let error = runtime.delete_table("resident/1", "sheet_1").unwrap_err();
    assert!(matches!(error, crate::DesignerError::LastTable));
    assert_eq!(runtime.current_revision(), "resident/1");
    assert_eq!(runtime.undo.len(), 1);
    assert_eq!(snapshot(&runtime), before);
}

#[test]
fn legacy_main_metadata_and_ro_shapes_round_trip_without_migration_until_edit() {
    let (runtime, metadata) = imported(false);
    assert!(
        runtime
            .session
            .document()
            .schemas
            .values()
            .all(|schema| schema.display_name.is_none())
    );
    let tree = encode_roproj_v3(runtime.session.document()).unwrap();
    let project =
        crate::encode_project_bundle(tree.files().iter().map(|file| (file.path(), file.bytes())))
            .unwrap();
    // Frozen a65c3e4d shape: no lifecycle metadata fields or schema names.
    let golden = include_str!("../tests/fixtures/859-main-metadata-v1.json")
        .trim_end()
        .as_bytes();
    assert_eq!(serde_json::to_vec(&metadata).unwrap(), golden);
    let legacy: InteropMetadata = serde_json::from_slice(golden).unwrap();
    assert_eq!(serde_json::to_vec(&legacy).unwrap(), golden);
    let mut opened = None;
    open_imported_project(&mut opened, &project, &legacy, OCCURRENCE).unwrap();
    let mut opened = opened.unwrap();
    assert_eq!(
        opened.export_project_v3("resident/0").unwrap().bytes,
        project
    );
    assert_eq!(xlsx(&opened, &legacy), xlsx(&runtime, &metadata));
    let publication = opened
        .rename_table("resident/0", "sheet_1", "Renamed Input")
        .unwrap();
    let published = publication.import_metadata.unwrap();
    assert_eq!(
        import_xlsx(&xlsx(&opened, &published)).unwrap().sheets[0].name,
        "Renamed Input"
    );
    opened.history_edit("resident/1", false).unwrap();
    assert_eq!(
        opened.export_project_v3("resident/2").unwrap().bytes,
        project
    );
    let mut detached = None;
    open_project(&mut detached, &project, OCCURRENCE).unwrap();
    assert_eq!(
        detached
            .unwrap()
            .export_project_v3("resident/0")
            .unwrap()
            .bytes,
        project
    );
}

#[test]
fn working_size_import_lifecycle_publishes_mapping_with_same_order() {
    let source = import_xlsx(&export_xlsx(&fixture(false)).unwrap()).unwrap();
    let (mut runtime, _) = import_workbook_working_size(&source, &selection(), OCCURRENCE).unwrap();
    let before = snapshot(&runtime);
    let added = runtime
        .add_table("resident/0", "A Added", &columns())
        .unwrap();
    let metadata = added.publication.import_metadata.unwrap();
    assert_eq!(metadata.version, 2);
    assert_eq!(metadata.sheets.last().unwrap().name, "A Added");
    history_round_trip(&mut runtime, &before);
    let renamed = runtime
        .rename_table("resident/3", "sheet_1", "Renamed Input")
        .unwrap();
    assert_eq!(
        renamed.import_metadata.unwrap().sheets[0].name,
        "Renamed Input"
    );
    let deleted = runtime.delete_table("resident/4", "sheet_2").unwrap();
    let metadata = deleted.import_metadata.unwrap();
    let project = runtime.export_project_v3("resident/5").unwrap();
    let (reopened, opened) =
        crate::admit_imported_project_working_size(&project.bytes, OCCURRENCE, &metadata).unwrap();
    assert_eq!(names(&reopened), ["Renamed Input", "A Added"]);
    assert_eq!(
        opened
            .bootstrap
            .collections
            .iter()
            .map(|table| table.display_name.as_str())
            .collect::<Vec<_>>(),
        ["Renamed Input", "A Added"]
    );
    assert_eq!(xlsx(&reopened, &metadata), xlsx(&runtime, &metadata));
}

#[test]
fn imported_oversized_publication_refuses_before_mutation_and_history() {
    let mut source = fixture(false);
    for sheet in &mut source.sheets {
        sheet.rows.resize(8, sheet.rows[0].clone());
    }
    let (runtime, projection) = import_workbook(&source, &selection(), OCCURRENCE).unwrap();
    let mut metadata = projection.metadata;
    for sheet in &mut metadata.sheets {
        for row in &mut sheet.rows {
            row.styles[0].number_format = Some("0".repeat(crate::MAX_PROFILE_STRING_BYTES));
        }
    }
    let bytes = crate::serialized_projection_bytes(&metadata).unwrap();
    let excess = bytes - (crate::MAX_PROJECTION_BYTES - 16);
    let last = metadata
        .sheets
        .last_mut()
        .unwrap()
        .rows
        .last_mut()
        .unwrap()
        .styles[0]
        .number_format
        .as_mut()
        .unwrap();
    last.truncate(last.len() - excess);
    crate::interop_document::validate_import_metadata_for_profile(
        runtime.session.document(),
        &metadata,
        runtime.profile,
    )
    .unwrap();
    let project = runtime.export_project_v3("resident/0").unwrap().bytes;
    let mut resident = None;
    open_imported_project(&mut resident, &project, &metadata, OCCURRENCE).unwrap();
    let before = snapshot(resident.as_ref().unwrap());
    let request = serde_json::to_vec(&crate::DesignerRequest::RenameTable {
        expected_revision: "resident/0".into(),
        collection: "sheet_1".into(),
        name: "Renamed Input".into(),
    })
    .unwrap();
    let reply: crate::DesignerWireReply =
        serde_json::from_slice(&crate::process_wire_request(&mut resident, &request)).unwrap();
    assert!(matches!(reply, crate::DesignerWireReply::Error { error }
        if error.code == "query_too_large" && error.current_revision == "resident/0"));
    let runtime = resident.as_mut().unwrap();
    assert_eq!(snapshot(runtime), before);
    assert!(runtime.undo.is_empty());
    assert!(runtime.redo.is_empty());

    // Deleting a sheet fits, but Undo must refuse atomically when restoring its
    // complete source presentation would overflow the publication reply.
    runtime.delete_table("resident/0", "sheet_2").unwrap();
    let before = snapshot(runtime);
    assert!(matches!(
        runtime.history_edit("resident/1", false),
        Err(crate::DesignerError::ProjectionTooLarge { .. })
    ));
    assert_eq!(runtime.current_revision(), "resident/1");
    assert_eq!(snapshot(runtime), before);
    assert_eq!(runtime.undo.len(), 1);
    assert!(runtime.redo.is_empty());
}

#[test]
fn working_size_changed_cells_refuse_oversized_undo_before_mutation() {
    let mut source = fixture(false);
    source.sheets[1].rows[0][0].value = SourceValue::Text {
        value: "x".repeat(30_000),
    };
    let mut types = selection();
    types.column_types[1][0] = ImportFieldType::Text;
    let (runtime, projection) = import_workbook_working_size(&source, &types, OCCURRENCE).unwrap();
    let mut metadata = projection.metadata;
    metadata.version = 1;
    for sheet in &mut metadata.sheets {
        sheet.rows = runtime
            .session
            .document()
            .entities
            .values()
            .filter(|entity| entity.schema.as_str() == sheet.schema_id)
            .map(|entity| crate::interop_document::InteropRowMetadata {
                entity_id: entity.id.to_string(),
                styles: vec![CellStyle::default()],
            })
            .collect();
    }
    let target = crate::MAX_TEXT_CAPACITY_PROJECTION_BYTES - 15_000;
    let style = CellStyle {
        number_format: Some("0".repeat(crate::MAX_PROFILE_STRING_BYTES)),
        ..CellStyle::default()
    };
    let count = target / crate::MAX_PROFILE_STRING_BYTES;
    for _ in 0..count {
        let rows = &mut metadata.sheets[1].rows;
        rows.push(crate::interop_document::InteropRowMetadata {
            entity_id: format!("historical_{}", rows.len()),
            styles: vec![style.clone()],
        });
    }
    while crate::serialized_projection_bytes(&metadata).unwrap()
        > target + crate::MAX_PROFILE_STRING_BYTES
    {
        metadata.sheets[1].rows.pop();
    }
    let excess = crate::serialized_projection_bytes(&metadata).unwrap() - target;
    let format = metadata.sheets[1].rows.last_mut().unwrap().styles[0]
        .number_format
        .as_mut()
        .unwrap();
    format.truncate(format.len() - excess);
    let project = runtime.export_project_v3("resident/0").unwrap().bytes;
    let (mut runtime, _) =
        crate::admit_imported_project_working_size(&project, OCCURRENCE, &metadata).unwrap();
    runtime.delete_table("resident/0", "sheet_2").unwrap();
    let before = snapshot(&runtime);
    let entry = runtime.undo.last().unwrap().clone();
    let result = runtime.preflight_imported_reply(&metadata, |candidate| {
        let mut detached = entry;
        detached.imported = None;
        candidate.undo.push(detached);
        let mut publication = candidate.history_edit("resident/0", false).unwrap();
        publication.import_metadata = Some(metadata.clone());
        let response = DesignerResponse::Published(publication);
        assert!(
            crate::ensure_wire_reply_size(
                &crate::DesignerWireReply::Ok {
                    response: response.clone()
                },
                candidate.profile,
            )
            .is_ok()
        );
        assert!(
            crate::ensure_wire_reply_size(
                &crate::DesignerWireReply::Ok {
                    response: candidate.with_changed_cells(response.clone())
                },
                candidate.profile,
            )
            .is_err()
        );
        Ok(response)
    });
    assert!(matches!(
        result,
        Err(crate::DesignerError::ProjectionTooLarge { .. })
    ));
    let mut resident = Some(runtime);
    let request = serde_json::to_vec(&DesignerRequest::Undo {
        expected_revision: "resident/1".into(),
    })
    .unwrap();
    let reply: crate::DesignerWireReply =
        serde_json::from_slice(&crate::process_wire_request(&mut resident, &request)).unwrap();
    assert!(matches!(reply, crate::DesignerWireReply::Error { error }
        if error.code == "query_too_large" && error.current_revision == "resident/1"));
    let runtime = resident.as_ref().unwrap();
    assert_eq!(snapshot(runtime), before);
    assert_eq!(runtime.undo.len(), 1);
    assert!(runtime.redo.is_empty());
}

#[test]
fn legacy_working_metadata_v2_stays_byte_stable_and_supports_lifecycle() {
    let source = import_xlsx(&export_xlsx(&fixture(false)).unwrap()).unwrap();
    let (runtime, projection) =
        import_workbook_working_size(&source, &selection(), OCCURRENCE).unwrap();
    let golden = include_str!("../tests/fixtures/859-main-metadata-v2.json")
        .trim_end()
        .as_bytes();
    assert_eq!(serde_json::to_vec(&projection.metadata).unwrap(), golden);
    let metadata: InteropMetadata = serde_json::from_slice(golden).unwrap();
    assert_eq!(serde_json::to_vec(&metadata).unwrap(), golden);
    let tree = encode_roproj_v3(runtime.session.document()).unwrap();
    let project =
        crate::encode_project_bundle(tree.files().iter().map(|file| (file.path(), file.bytes())))
            .unwrap();
    let (mut reopened, _) =
        crate::admit_imported_project_working_size(&project, OCCURRENCE, &metadata).unwrap();
    assert_eq!(
        reopened.export_project_v3("resident/0").unwrap().bytes,
        project
    );
    let added = reopened
        .add_table("resident/0", "A Added", &columns())
        .unwrap();
    let published = added.publication.import_metadata.unwrap();
    assert_eq!(
        import_xlsx(&xlsx(&reopened, &published))
            .unwrap()
            .sheets
            .len(),
        3
    );
    reopened.history_edit("resident/1", false).unwrap();
    assert_eq!(
        reopened.export_project_v3("resident/2").unwrap().bytes,
        project
    );
}
