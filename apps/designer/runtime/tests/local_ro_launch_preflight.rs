use tachiko_designer_runtime::{
    CalculationProjection, DesignerRequest, DesignerResponse, DesignerRuntime,
};
use tachiko_storage::{encode_portable_package_v1, encode_roproj_v1, from_bytes};

const GAME_BALANCE_RO: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../examples/game-balance/game-balance.ro"
));
const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000344";

fn assert_designer_admits(source: &[u8]) {
    let document = from_bytes(source).expect("existing storage authority must admit the .ro source");
    let mut runtime = DesignerRuntime::from_document(document, OCCURRENCE)
        .expect("existing Designer profile must admit the semantic document");

    let DesignerResponse::Bootstrap(bootstrap) = runtime
        .handle(DesignerRequest::Bootstrap {
            occurrence_id: OCCURRENCE.to_owned(),
        })
        .expect("bootstrap must succeed")
    else {
        panic!("expected bootstrap projection");
    };
    assert_eq!(bootstrap.title, "Moonfall: starter balance");
    assert_eq!(bootstrap.revision, "resident/0");

    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: "weapons".to_owned(),
        })
        .expect("weapons projection must succeed")
    else {
        panic!("expected table projection");
    };
    let sword = table
        .rows
        .iter()
        .find(|row| row.key == "iron_sword")
        .expect("Iron Sword row must remain addressable by its human key");
    let dps = sword
        .fields
        .iter()
        .find(|field| field.address == "iron_sword.dps")
        .expect("DPS field must be projected");
    assert_eq!(
        dps.calculated
            .as_ref()
            .and_then(CalculationProjection::number),
        Some(40.0)
    );
}

#[test]
fn current_direct_ro_is_already_admissible_by_storage_and_designer() {
    assert_designer_admits(GAME_BALANCE_RO);
}

#[test]
fn current_portable_package_v1_is_already_admissible_by_the_same_byte_reader() {
    let document = from_bytes(GAME_BALANCE_RO).expect("direct .ro fixture must decode");
    let tree = encode_roproj_v1(&document).expect("fixture must materialize as canonical .roproj/v1");
    let package = encode_portable_package_v1(&tree).expect("fixture must package as portable-package/v1");

    assert!(package.starts_with(b"PK\x03\x04"));
    assert_designer_admits(&package);
}
