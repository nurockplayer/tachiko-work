use tachiko_semantic_core::validate_document;
use tachiko_storage::{load, to_canonical_string};
use tachiko_workflow::{FieldKind, StarterTemplate, create_document, overview};

#[test]
fn game_balance_starter_is_immediately_meaningful() {
    let document = create_document(
        StarterTemplate::GameBalance,
        "game-balance",
        "Moonfall: starter balance",
    );

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
        .find(|entity| entity.id.as_str() == "iron_sword")
        .expect("starter weapon should be present");
    assert_eq!(weapon.label, "Iron Sword");
    assert_eq!(weapon.schema.as_str(), "weapons");

    let dps = weapon
        .fields
        .iter()
        .find(|field| field.id.as_str() == "dps")
        .expect("weapon DPS should be present");
    assert_eq!(dps.kind, FieldKind::Formula);
    assert_eq!(dps.display_value, "40");

    let weapon_reference = view.entities[0]
        .fields
        .iter()
        .find(|field| field.id.as_str() == "weapon")
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
    let starter = create_document(StarterTemplate::GameBalance, "game", "Game");
    let view = overview(&starter).expect("starter should calculate");
    let entity_ids: Vec<_> = view
        .entities
        .iter()
        .map(|entity| entity.id.as_str())
        .collect();
    assert_eq!(
        entity_ids,
        ["alric", "iron_sword", "shop", "tempered_blade"]
    );

    let empty = create_document(StarterTemplate::Empty, "scratch", "Scratch");
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
fn built_in_starter_matches_the_checked_in_example() {
    let example_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/game-balance/game-balance.ro");
    let checked_in = load(&example_path).expect("checked-in example should load");
    let built_in = create_document(
        StarterTemplate::GameBalance,
        "game-balance",
        "Moonfall: starter balance",
    );

    assert_eq!(built_in, checked_in);
    assert_eq!(
        to_canonical_string(&built_in).unwrap(),
        std::fs::read_to_string(example_path).unwrap(),
        "the generated starter and checked-in example must remain byte-identical"
    );
}
