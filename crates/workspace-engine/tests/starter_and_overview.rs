mod common;

use common::{empty_document, game_balance_document};
use tachiko_semantic_core::{DiagnosticCode, Document, validate_document};
use tachiko_workspace_engine::{FieldKind, WorkspaceError, overview};

type DuplicateKeyMutation = fn(&mut Document);

fn duplicate_schema_key(document: &mut Document) {
    let duplicate_key = document
        .schemas
        .values()
        .next()
        .expect("starter should contain a schema")
        .key
        .clone();
    document
        .schemas
        .values_mut()
        .nth(1)
        .expect("starter should contain a second schema")
        .key = duplicate_key;
}

fn duplicate_entity_key(document: &mut Document) {
    let duplicate_key = document
        .entities
        .values()
        .next()
        .expect("starter should contain an entity")
        .key
        .clone();
    document
        .entities
        .values_mut()
        .nth(1)
        .expect("starter should contain a second entity")
        .key = duplicate_key;
}

fn duplicate_field_key(document: &mut Document) {
    let schema = document
        .schemas
        .values_mut()
        .find(|schema| schema.fields.len() > 1)
        .expect("starter should contain a schema with multiple fields");
    let duplicate_key = schema
        .fields
        .values()
        .next()
        .expect("selected schema should contain a field")
        .key
        .clone();
    schema
        .fields
        .values_mut()
        .nth(1)
        .expect("selected schema should contain a second field")
        .key = duplicate_key;
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
fn overview_rejects_duplicate_human_keys_as_an_invalid_document() {
    let cases: [(&str, DuplicateKeyMutation); 3] = [
        ("schema keys", duplicate_schema_key),
        ("entity keys", duplicate_entity_key),
        ("field keys within one schema", duplicate_field_key),
    ];

    for (category, make_duplicate) in cases {
        let mut document = game_balance_document("game", "Game");
        make_duplicate(&mut document);

        let error = overview(&document)
            .expect_err("overview should reject directly constructed duplicate human keys");
        let WorkspaceError::InvalidDocument { diagnostics, .. } = error else {
            panic!("{category}: expected InvalidDocument, got {error:?}");
        };
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code)
                .collect::<Vec<_>>(),
            [DiagnosticCode::DuplicateKey],
            "{category}"
        );
    }
}
