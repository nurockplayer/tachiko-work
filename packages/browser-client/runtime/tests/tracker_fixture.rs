use tachiko_designer_runtime::{DesignerRequest, DesignerResponse, DesignerRuntime, open_project};
use tachiko_storage::{
    CanonicalRoProjectV1, decode_roproj_v1, encode_roproj_v1, read_canonical_roproj,
};
use tachiko_workspace_engine::{FieldId, Value, validation_report};

const OCCURRENCE: &str = "4d9475a3-9ba3-4a61-a7f5-852a84e82257";
const ROWS: &str = include_str!("../../e2e/fixtures/operations-tracker.tsv");

#[test]
fn operational_tracker_fixture_is_canonical_admitted_and_matches_driver_paste() {
    let checked_in = read_canonical_roproj(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../dogfood/operations-tracker.roproj"),
    )
    .unwrap();
    let document = decode_roproj_v1(&checked_in).unwrap();
    assert_eq!(document.id.as_str(), OCCURRENCE);
    assert_eq!(document.entities.len(), 40);
    assert!(validation_report(&document).is_valid());
    assert_eq!(encode_roproj_v1(&document).unwrap(), checked_in);
    let source = ROWS
        .lines()
        .map(|line| line.split('\t').map(str::to_owned).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    for (entity, row) in document.entities.values().zip(&source) {
        assert_eq!(
            entity.fields[&FieldId::from("task")],
            Value::Text(row[0].clone())
        );
        assert_eq!(
            entity.fields[&FieldId::from("estimate")],
            Value::Number(row[1].parse::<f64>().unwrap().try_into().unwrap())
        );
        assert_eq!(
            entity.fields[&FieldId::from("done")],
            Value::Boolean(row[2].parse().unwrap())
        );
    }
    let mut runtime = DesignerRuntime::tracker(OCCURRENCE).unwrap();
    let DesignerResponse::Published(publication) = runtime
        .handle(DesignerRequest::PasteCells {
            expected_revision: "resident/0".to_owned(),
            collection: "tracker".to_owned(),
            start_entity: None,
            start_field: "task".to_owned(),
            rows: source,
        })
        .unwrap()
    else {
        panic!("paste must publish")
    };
    let export = runtime
        .export_project(&publication.resulting_revision)
        .unwrap();
    assert_eq!(export.bytes, private_bundle(&checked_in));
    let mut slot = None;
    let opened = open_project(
        &mut slot,
        &export.bytes,
        "00000000-0000-4000-8000-000000000257",
    )
    .unwrap();
    assert_eq!(opened.table.rows.len(), 40);
    assert_eq!(
        slot.as_ref()
            .unwrap()
            .export_project(&opened.table.revision)
            .unwrap()
            .bytes,
        export.bytes
    );
}

fn private_bundle(tree: &CanonicalRoProjectV1) -> Vec<u8> {
    let mut bytes = b"TWDPROJ1".to_vec();
    bytes.extend_from_slice(&u32::try_from(tree.files().len()).unwrap().to_le_bytes());
    for file in tree.files() {
        bytes.extend_from_slice(&u16::try_from(file.path().len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&u32::try_from(file.bytes().len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(file.path().as_bytes());
        bytes.extend_from_slice(file.bytes());
    }
    bytes
}
