use tachiko_designer_runtime::{
    DesignerRequest, DesignerResponse, DesignerRuntime, open_local_document,
};

const GAME_BALANCE_RO: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../examples/game-balance/game-balance.ro"
));
const FIRST_OCCURRENCE: &str = "00000000-0000-4000-8000-000000000344";
const SECOND_OCCURRENCE: &str = "00000000-0000-4000-8000-000000000345";

#[test]
fn local_ro_ingress_admits_exact_raw_bytes_through_storage() {
    let mut slot = None;

    let opened = open_local_document(&mut slot, GAME_BALANCE_RO, FIRST_OCCURRENCE)
        .expect("the checked-in local .ro must be admitted through storage");

    assert_eq!(opened.bootstrap.title, "Moonfall: starter balance");
    assert_eq!(opened.bootstrap.revision, "resident/0");
    assert!(slot.unwrap().occurrence_scope().contains(FIRST_OCCURRENCE));
}

#[test]
fn invalid_local_ro_ingress_preserves_the_current_occurrence() {
    let mut slot = Some(DesignerRuntime::tracker(FIRST_OCCURRENCE).unwrap());
    let before = slot
        .as_mut()
        .unwrap()
        .export_project("resident/0")
        .unwrap()
        .bytes;

    let error = open_local_document(&mut slot, b"not a Tachiko document", SECOND_OCCURRENCE)
        .expect_err("invalid raw local bytes must not replace the resident");

    assert_eq!(
        error.failure_projection("resident/0").code,
        "invalid_project"
    );
    let DesignerResponse::Bootstrap(bootstrap) = slot
        .as_mut()
        .unwrap()
        .handle(DesignerRequest::Bootstrap {
            occurrence_id: FIRST_OCCURRENCE.to_owned(),
        })
        .unwrap()
    else {
        panic!("the current occurrence must remain available");
    };
    assert_eq!(bootstrap.title, "Driver Tracker");
    assert_eq!(
        slot.as_mut()
            .unwrap()
            .export_project("resident/0")
            .unwrap()
            .bytes,
        before
    );
}
