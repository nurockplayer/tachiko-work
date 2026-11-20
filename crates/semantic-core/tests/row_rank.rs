use std::collections::BTreeMap;

use tachiko_semantic_core::{Document, Entity, RowRank, compare_row_order};

fn entity(id: &str, rank: Option<&str>) -> Entity {
    Entity {
        id: id.into(),
        key: id.into(),
        schema: "s".into(),
        fields: BTreeMap::new(),
        rank: rank.map(|value| RowRank::new(value).unwrap()),
    }
}

#[test]
fn rank_construction_and_deserialization_validate_the_same_boundary() {
    for invalid in ["", "0", "A0", "a_b", "a-b", "é", " A", &"A".repeat(65)] {
        assert!(RowRank::new(invalid).is_err());
        let json = serde_json::to_string(invalid).unwrap();
        assert!(serde_json::from_str::<RowRank>(&json).is_err());
    }
    for valid in ["1", "0A", "A", "a", &"A".repeat(64)] {
        let rank = RowRank::new(valid).unwrap();
        let json = serde_json::to_string(&rank).unwrap();
        assert_eq!(serde_json::from_str::<RowRank>(&json).unwrap(), rank);
    }
    assert!(serde_json::from_str::<RowRank>("null").is_err());
}

#[test]
fn rank_order_is_present_then_unsigned_bytes_then_entity_id() {
    let mut rows = [
        entity("b", None),
        entity("z", Some("a")),
        entity("a", None),
        entity("c", Some("A")),
        entity("b", Some("A")),
        entity("x", Some("1")),
        entity("d", Some("AA")),
    ];
    rows.sort_by(compare_row_order);
    assert_eq!(
        rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
        ["x", "b", "c", "d", "z", "a", "b"]
    );
    let mut document = Document::empty("doc", "Ranks");
    assert!(!document.has_row_ranks());
    document
        .entities
        .insert("row".into(), entity("row", Some("A")));
    assert!(document.has_row_ranks());
    assert_eq!(
        document.clone().entities["row"].rank,
        document.entities["row"].rank
    );
}
