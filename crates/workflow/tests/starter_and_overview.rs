mod common;

use common::{empty_document, game_balance_document};
use tachiko_semantic_core::validate_document;
use tachiko_storage::load;
use tachiko_workflow::{DocumentOverview, FieldKind, overview};

type AuthoringField = (String, String, FieldKind);
type AuthoringEntity = (String, String, String, Vec<AuthoringField>);

fn authoring_projection(view: DocumentOverview) -> Vec<AuthoringEntity> {
    view.entities
        .into_iter()
        .map(|entity| {
            (
                entity.key.to_string(),
                entity.label,
                entity.schema.to_string(),
                entity
                    .fields
                    .into_iter()
                    .map(|field| (field.key.to_string(), field.display_value, field.kind))
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn game_balance_starter_is_immediately_meaningful() {
    let document = game_balance_document("game-balance", "Moonfall: starter balance");

    assert!(validate_document(&document).is_empty());
    assert_eq!(document.schemas.len(), 4);
    assert_eq!(document.entities.len(), 4);

    let view = overview(&document).expect("starter should calculate");
    assert_eq!(view.schema_count, 4);
    assert_eq!(view.entity_count, 4);
    assert_eq!(view.formula_count, 3);

    let weapon = view
        .entities
        .iter()
        .find(|entity| entity.key.as_str() == "iron_sword")
        .expect("starter weapon should be present");
    assert_eq!(weapon.label, "Iron Sword");
    assert_eq!(weapon.schema.as_str(), "weapons");

    let dps = weapon
        .fields
        .iter()
        .find(|field| field.key.as_str() == "dps")
        .expect("weapon DPS should be present");
    assert_eq!(dps.kind, FieldKind::Formula);
    assert_eq!(dps.display_value, "40");

    let weapon_reference = view.entities[0]
        .fields
        .iter()
        .find(|field| field.key.as_str() == "weapon")
        .expect("character weapon reference should be present");
    assert_eq!(
        weapon_reference.kind,
        FieldKind::Reference {
            target_schema: "weapons".into()
        }
    );
}

#[test]
fn overview_order_is_stable_and_empty_template_remains_available() {
    let starter = game_balance_document("game", "Game");
    let view = overview(&starter).expect("starter should calculate");
    let entity_keys: Vec<_> = view
        .entities
        .iter()
        .map(|entity| entity.key.as_str())
        .collect();
    assert_eq!(
        entity_keys,
        ["alric", "iron_sword", "shop", "tempered_blade"]
    );

    let empty = empty_document("scratch", "Scratch");
    assert_eq!(empty.id.as_str(), "scratch");
    assert_eq!(empty.title, "Scratch");
    assert!(empty.schemas.is_empty());
    assert!(empty.entities.is_empty());

    let empty_view = overview(&empty).expect("empty document should calculate");
    assert_eq!(empty_view.schema_count, 0);
    assert_eq!(empty_view.entity_count, 0);
    assert_eq!(empty_view.formula_count, 0);
    assert!(empty_view.entities.is_empty());
}

#[test]
fn built_in_starter_matches_the_legacy_example_at_the_authoring_boundary() {
    let example_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/game-balance/game-balance.ro");
    let checked_in = load(&example_path).expect("checked-in example should load");
    let built_in = game_balance_document("game-balance", "Moonfall: starter balance");

    assert_eq!(
        authoring_projection(overview(&built_in).unwrap()),
        authoring_projection(overview(&checked_in).unwrap())
    );
    assert_ne!(
        built_in.id, checked_in.id,
        "migration must establish new identity"
    );
}
