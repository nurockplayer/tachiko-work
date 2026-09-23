use std::collections::BTreeMap;

use tachiko_semantic_core::{
    FieldConstraint, FieldDefinition, FieldType, KeyedGroupedSumDefinition,
    KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding,
};

use super::*;

fn field(id: &str, field_type: FieldType) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        field_type,
        required: false,
        constraint: FieldConstraint::None,
    }
}

fn document_with_keyed_sum() -> Document {
    let mut document = Document::empty("doc", "title");
    document.schemas.insert(
        "orders".into(),
        Schema {
            id: "orders".into(),
            key: "orders".into(),
            fields: BTreeMap::from([
                ("sku".into(), field("sku", FieldType::Text)),
                ("quantity".into(), field("quantity", FieldType::Number)),
            ]),
        },
    );
    document.schemas.insert(
        "products".into(),
        Schema {
            id: "products".into(),
            key: "products".into(),
            fields: BTreeMap::from([
                ("sku".into(), field("sku", FieldType::Text)),
                ("category".into(), field("category", FieldType::Text)),
                ("price".into(), field("price", FieldType::Number)),
            ]),
        },
    );
    document.keyed_grouped_sum_definitions.insert(
        "summary".into(),
        KeyedGroupedSumDefinition {
            id: "summary".into(),
            orders: KeyedGroupedSumOrdersBinding {
                schema: "orders".into(),
                lookup_key_field: "sku".into(),
                quantity_field: "quantity".into(),
            },
            products: KeyedGroupedSumProductsBinding {
                schema: "products".into(),
                key_field: "sku".into(),
                category_field: "category".into(),
                price_field: "price".into(),
            },
        },
    );
    document
}

#[test]
fn equal_intrinsically_invalid_definition_maps_fail_with_typed_base_cause() {
    let mut invalid = document_with_keyed_sum();
    invalid
        .keyed_grouped_sum_definitions
        .get_mut(&"summary".into())
        .unwrap()
        .orders
        .schema = "missing".into();

    assert!(matches!(
        merge_documents_v2(SEMANTIC_CONFLICT_V2, &invalid, &invalid, &invalid),
        Err(WorkspaceError::InvalidMergeKeyedGroupedSumDefinitions {
            role: ValidationRole::MergeBase,
            source: KeyedGroupedSumDefinitionError::MissingSchema {
                role: "orders",
                schema,
            },
        }) if schema == "missing".into()
    ));
}

#[test]
fn unchanged_definition_map_is_revalidated_after_each_side_schema_edit() {
    let valid = document_with_keyed_sum();
    let mut invalid = valid.clone();
    invalid
        .schemas
        .get_mut("orders")
        .unwrap()
        .fields
        .get_mut("sku")
        .unwrap()
        .field_type = FieldType::Number;

    for (base, ours, theirs, expected_role) in [
        (&invalid, &valid, &valid, ValidationRole::MergeBase),
        (&valid, &invalid, &valid, ValidationRole::MergeOurs),
        (&valid, &valid, &invalid, ValidationRole::MergeTheirs),
    ] {
        assert!(matches!(
            merge_documents_v2(SEMANTIC_CONFLICT_V2, base, ours, theirs),
            Err(WorkspaceError::InvalidMergeKeyedGroupedSumDefinitions {
                role,
                source: KeyedGroupedSumDefinitionError::WrongFieldType {
                    role: tachiko_semantic_core::KeyedGroupedSumBindingRole::OrdersLookupKey,
                    schema,
                    field,
                    expected: FieldType::Text,
                    actual: FieldType::Number,
                },
            }) if role == expected_role && schema == "orders".into() && field == "sku".into()
        ));
    }
}
