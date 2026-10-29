use std::collections::BTreeMap;

use tachiko_semantic_core::{
    Document, FieldConstraint, FieldDefinition, FieldId, FieldType, Schema, validate_document,
};

fn field(id: &str) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        display_name: None,
        field_type: FieldType::Number,
        required: false,
        constraint: FieldConstraint::None,
        number_format: None,
    }
}

fn schema(order: Option<Vec<&str>>) -> Schema {
    Schema {
        id: "s".into(),
        key: "s".into(),
        display_name: None,
        field_order: order.map(|ids| ids.into_iter().map(FieldId::from).collect()),
        fields: BTreeMap::from([
            ("a".into(), field("a")),
            ("b".into(), field("b")),
            ("c".into(), field("c")),
        ]),
    }
}

fn ids(schema: &Schema) -> Vec<String> {
    schema
        .ordered_fields()
        .map(|field| field.id.to_string())
        .collect()
}

fn invalid_order_count(schema: Schema) -> usize {
    let mut document = Document::empty("d", "Order");
    document.schemas.insert("s".into(), schema);
    validate_document(&document)
        .iter()
        .filter(|diagnostic| diagnostic.code.as_str() == "core.invalid_field_order")
        .count()
}

#[test]
fn ordered_fields_follow_the_order_and_none_is_id_order() {
    assert_eq!(ids(&schema(None)), ["a", "b", "c"]);
    assert_eq!(ids(&schema(Some(vec!["c", "a", "b"]))), ["c", "a", "b"]);
}

#[test]
fn normalize_drops_stale_ids_appends_missing_and_clears_identity() {
    let mut stale = schema(Some(vec!["c", "zz", "a"]));
    stale.fields.remove(&FieldId::from("b"));
    stale.fields.insert("d".into(), field("d"));
    stale.normalize_field_order();
    assert_eq!(
        stale.field_order,
        Some(vec!["c".into(), "a".into(), "d".into()])
    );

    let mut identity = schema(Some(vec!["a", "b", "c"]));
    identity.normalize_field_order();
    assert_eq!(identity.field_order, None);

    let mut none = schema(None);
    none.normalize_field_order();
    assert_eq!(none.field_order, None);
}

#[test]
fn validation_rejects_duplicates_missing_extra_and_redundant_orders() {
    assert_eq!(invalid_order_count(schema(Some(vec!["a", "a", "c"]))), 1);
    assert_eq!(invalid_order_count(schema(Some(vec!["c", "a"]))), 1);
    assert_eq!(invalid_order_count(schema(Some(vec!["c", "a", "x"]))), 1);
    assert_eq!(invalid_order_count(schema(Some(vec!["a", "b", "c"]))), 1);
    assert_eq!(invalid_order_count(schema(Some(vec!["c", "a", "b"]))), 0);
    assert_eq!(invalid_order_count(schema(None)), 0);
}
