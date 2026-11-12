use super::{
    CellEdit, DesignerError, DesignerRequest, DesignerRuntime, FieldTarget, PresentationStore,
    ScalarEditInput, SessionProfile, totals_cache_counters, totals_tests, view_index_counters,
};
use tachiko_workspace_engine::{SchemaId, TablePresentation};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000678";

fn presentation() -> PresentationStore {
    let mut store = PresentationStore::default();
    let mut table = TablePresentation::default();
    table.column_widths.insert("f01_amount".into(), 120);
    store.tables.insert(SchemaId::from("rows"), table);
    store
}

#[test]
fn presentation_carries_caches_but_a_value_edit_does_not() {
    let mut runtime =
        DesignerRuntime::from_document(totals_tests::valid_document(), OCCURRENCE).unwrap();
    let before = runtime.session.export_snapshot();
    totals_tests::window_totals(&mut runtime);
    let builds = view_index_counters::builds();
    let fills = totals_cache_counters::fills();
    let hits = totals_cache_counters::hits();
    let old = runtime.current_revision().to_owned();
    let new = runtime.publish_presentation(&old, presentation()).unwrap();
    assert_ne!(old, new);
    assert_eq!(runtime.session.document(), before.document());
    totals_tests::window_totals(&mut runtime);
    assert_eq!(view_index_counters::builds(), builds);
    assert_eq!(totals_cache_counters::fills(), fills);
    assert_eq!(totals_cache_counters::hits(), hits + 1);
    runtime
        .handle(DesignerRequest::EditCells {
            expected_revision: new,
            edits: vec![CellEdit {
                target: FieldTarget {
                    entity: "e01".into(),
                    field: "f01_amount".into(),
                },
                input: ScalarEditInput::Number { input: "42".into() },
            }],
        })
        .unwrap();
    totals_tests::window_totals(&mut runtime);
    assert_eq!(view_index_counters::builds(), builds + 1);
    assert_eq!(totals_cache_counters::fills(), fills + 1);
}

#[test]
fn stale_presentation_is_refused() {
    let mut runtime =
        DesignerRuntime::from_document(totals_tests::valid_document(), OCCURRENCE).unwrap();
    let revision = runtime.current_revision().to_owned();
    let before = runtime.presentation.clone();
    assert!(matches!(
        runtime.publish_presentation("stale", presentation()),
        Err(DesignerError::StaleQuery { .. })
    ));
    assert_eq!(runtime.current_revision(), revision);
    assert_eq!(runtime.presentation, before);
}

#[test]
fn working_size_records_presentation_bytes() {
    let mut runtime = DesignerRuntime::from_document_profile(
        totals_tests::valid_document(),
        OCCURRENCE,
        SessionProfile::WorkingSize,
    )
    .unwrap();
    let store = presentation();
    let bytes = super::save_size::presentation_saved_bytes(&store).unwrap();
    let revision = runtime.current_revision().to_owned();
    runtime.publish_presentation(&revision, store).unwrap();
    assert!(bytes > 0);
    assert_eq!(runtime.presentation_saved_bytes, bytes);
}
