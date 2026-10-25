//! Work #488 item (e): one closure test that saves a table carrying every
//! supported v3 feature, reopens it in a fresh runtime and proves the reopened
//! meaning is equal — Dates, explicit display names, required and optional
//! fields, blank cells, formulas, a duplicated table and a saved
//! keyed-grouped-sum definition — then does the same through a portable `.ro`.

use tachiko_designer_runtime::{
    BootstrapProjection, DesignerRequest, DesignerResponse, DesignerRuntime,
    KeyedGroupedSumProjection, ScalarEditInput, StoredValueProjection, TableProjection,
    open_portable_ro, open_project, verify_portable_ro,
};
use tachiko_workspace_engine::{
    Date, Document, Entity, EntityId, EntityKey, Expression, FieldConstraint, FieldDefinition,
    FieldId, FieldKey, FieldRef, FieldType, KeyedGroupedSumDefinition, KeyedGroupedSumDefinitionId,
    KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding, Number, Schema, SchemaId,
    SchemaKey, Value,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000700";
const REOPENED: &str = "00000000-0000-4000-8000-000000000701";
const PORTABLE: &str = "00000000-0000-4000-8000-000000000702";
const DEFINITION: &str = "revenue-by-category";

fn field(id: &str, name: &str, field_type: FieldType, required: bool) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(id),
        display_name: Some(name.to_owned()),
        field_type,
        required,
        constraint: FieldConstraint::None,
    }
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).expect("fixture numbers are finite"))
}

fn product_total(order: &str) -> Value {
    Value::Formula(Expression::Multiply {
        left: Box::new(Expression::Reference(FieldRef::new(order, "quantity"))),
        right: Box::new(Expression::Reference(FieldRef::new(order, "unit_price"))),
    })
}

fn date(value: &str) -> Value {
    Value::Date(Date::parse(value).expect("fixture dates are valid"))
}

fn entity(
    document: &mut Document,
    schema: &SchemaId,
    id: &str,
    fields: Vec<(&str, Value)>,
) -> EntityId {
    let entity_id = EntityId::from(id);
    document.entities.insert(
        entity_id.clone(),
        Entity {
            id: entity_id.clone(),
            key: EntityKey::from(id),
            schema: schema.clone(),
            fields: fields
                .into_iter()
                .map(|(field, value)| (FieldId::from(field), value))
                .collect(),
        },
    );
    entity_id
}

fn schema(document: &mut Document, id: &str, name: &str, fields: Vec<FieldDefinition>) -> SchemaId {
    let schema_id = SchemaId::from(id);
    document.schemas.insert(
        schema_id.clone(),
        Schema {
            id: schema_id.clone(),
            key: SchemaKey::from(id),
            display_name: Some(name.to_owned()),
            fields: fields
                .into_iter()
                .map(|field| (field.id.clone(), field))
                .collect(),
        },
    );
    schema_id
}

/// The feature matrix of item (e) in one document: unicode display names,
/// required/optional columns, a present-empty Text versus an absent one, Date,
/// a per-row formula, a duplicated schema pair for the grouped sum, and one
/// saved definition.
fn kitchen_sink_document() -> Document {
    let mut document = Document::empty("kitchen_sink", "Kitchen sink 全部");
    let orders = schema(
        &mut document,
        "orders",
        "Orders 注文",
        vec![
            field("item", "Item", FieldType::Text, true),
            field("category", "分類 Category", FieldType::Text, true),
            field("quantity", "Quantity", FieldType::Number, true),
            field("unit_price", "Unit price", FieldType::Number, true),
            field("total", "Total", FieldType::Number, false),
            field("ordered_on", "Ordered on", FieldType::Date, true),
            field("note", "Note", FieldType::Text, false),
            field("shipped", "Shipped", FieldType::Boolean, false),
        ],
    );
    for (id, item, quantity, price, day, note, shipped) in [
        (
            "order-pen",
            "PEN",
            4.0,
            3.0,
            "2026-09-28",
            Some(Value::Text("urgent".to_owned())),
            Some(Value::Boolean(true)),
        ),
        ("order-note", "NOTE", 5.0, 4.0, "2026-09-27", None, None),
        (
            "order-spare",
            "PEN",
            2.0,
            3.0,
            "2026-09-26",
            Some(Value::Text(String::new())),
            None,
        ),
    ] {
        let mut fields = vec![
            ("item", Value::Text(item.to_owned())),
            (
                "category",
                Value::Text(if item == "PEN" { "Stationery" } else { "Paper" }.to_owned()),
            ),
            ("quantity", number(quantity)),
            ("unit_price", number(price)),
            ("total", product_total(id)),
            ("ordered_on", date(day)),
        ];
        fields.extend(note.map(|note| ("note", note)));
        fields.extend(shipped.map(|shipped| ("shipped", shipped)));
        entity(&mut document, &orders, id, fields);
    }

    let products = schema(
        &mut document,
        "products",
        "Products 商品",
        vec![
            field("product_key", "Product key", FieldType::Text, true),
            field(
                "product_category",
                "Product category",
                FieldType::Text,
                true,
            ),
            field("product_price", "Product price", FieldType::Number, true),
        ],
    );
    for (id, key, category, price) in [
        ("product-pen", "PEN", "Stationery", 3.0),
        ("product-note", "NOTE", "Paper", 4.0),
    ] {
        entity(
            &mut document,
            &products,
            id,
            vec![
                ("product_key", Value::Text(key.to_owned())),
                ("product_category", Value::Text(category.to_owned())),
                ("product_price", number(price)),
            ],
        );
    }

    document.keyed_grouped_sum_definitions.insert(
        KeyedGroupedSumDefinitionId::from(DEFINITION),
        KeyedGroupedSumDefinition {
            id: KeyedGroupedSumDefinitionId::from(DEFINITION),
            orders: KeyedGroupedSumOrdersBinding {
                schema: orders,
                lookup_key_field: FieldId::from("item"),
                quantity_field: FieldId::from("quantity"),
            },
            products: KeyedGroupedSumProductsBinding {
                schema: products,
                key_field: FieldId::from("product_key"),
                category_field: FieldId::from("product_category"),
                price_field: FieldId::from("product_price"),
            },
        },
    );
    document
}

fn handle(runtime: &mut DesignerRuntime, request: DesignerRequest) -> DesignerResponse {
    runtime
        .handle(request)
        .expect("the fixture request is accepted")
}

fn table(runtime: &mut DesignerRuntime, collection: &str) -> TableProjection {
    let DesignerResponse::Table(table) = handle(
        runtime,
        DesignerRequest::QueryTable {
            collection: collection.to_owned(),
        },
    ) else {
        panic!("expected a table projection");
    };
    table
}

/// Revision spellings are per-occurrence history, not persisted meaning.
fn normalized(mut table: TableProjection) -> TableProjection {
    table.revision = "normalized".to_owned();
    table
}

fn bootstrap(runtime: &mut DesignerRuntime, occurrence: &str) -> BootstrapProjection {
    let DesignerResponse::Bootstrap(mut bootstrap) = handle(
        runtime,
        DesignerRequest::Bootstrap {
            occurrence_id: occurrence.to_owned(),
        },
    ) else {
        panic!("expected a bootstrap projection");
    };
    bootstrap.revision = "normalized".to_owned();
    bootstrap
}

fn grouped_sum(runtime: &mut DesignerRuntime) -> KeyedGroupedSumProjection {
    let DesignerResponse::KeyedGroupedSum(mut sum) = handle(
        runtime,
        DesignerRequest::QueryKeyedGroupedSum {
            definition_id: DEFINITION.to_owned(),
        },
    ) else {
        panic!("expected a keyed grouped-sum projection");
    };
    sum.revision = "normalized".to_owned();
    sum
}

fn cell<'a>(
    table: &'a TableProjection,
    entity: &str,
    field: &str,
) -> &'a tachiko_designer_runtime::FieldProjection {
    let row = table
        .rows
        .iter()
        .find(|row| row.id == entity)
        .unwrap_or_else(|| panic!("no row '{entity}'"));
    row.fields
        .iter()
        .find(|candidate| candidate.target.field == field)
        .unwrap_or_else(|| panic!("row '{entity}' has no '{field}' value"))
}

fn calculated(field: &tachiko_designer_runtime::FieldProjection) -> Option<f64> {
    field
        .calculated
        .as_ref()
        .and_then(tachiko_designer_runtime::CalculationProjection::number)
}

fn duplicate(runtime: &mut DesignerRuntime, expected: &str) -> String {
    let DesignerResponse::Duplicated(duplicated) = handle(
        runtime,
        DesignerRequest::DuplicateCollection {
            expected_revision: expected.to_owned(),
            collection: "orders".to_owned(),
            name: "Orders copy".to_owned(),
        },
    ) else {
        panic!("expected a duplicate projection");
    };
    assert_eq!(duplicated.publication.resulting_revision, "resident/1");
    assert_ne!(duplicated.collection.key, "orders");
    duplicated.collection.key
}

fn open_saved(bytes: &[u8], occurrence: &str) -> DesignerRuntime {
    let mut reopened = None;
    open_project(&mut reopened, bytes, occurrence).expect("the saved v3 project reopens");
    reopened.expect("a successful open installs the occurrence")
}

#[test]
fn v3_save_reopen_preserves_every_supported_feature_and_reexports_identically() {
    let source = DesignerRuntime::from_document(kitchen_sink_document(), OCCURRENCE)
        .expect("the kitchen-sink document is admissible");
    let mut source = source;
    let copy_key = duplicate(&mut source, "resident/0");
    let saved = source
        .export_project_v3("resident/1")
        .expect("the kitchen-sink document exports as v3");
    assert_eq!(saved.revision, "resident/1");

    let reopened = open_saved(&saved.bytes, REOPENED);
    let mut reopened = reopened;
    assert_eq!(
        bootstrap(&mut reopened, REOPENED),
        bootstrap(&mut source, OCCURRENCE),
        "bootstrap collections, title and saved definitions survive the round trip"
    );
    assert_eq!(
        bootstrap(&mut reopened, REOPENED).keyed_grouped_sum_definition_ids,
        vec![DEFINITION.to_owned()]
    );

    // Semantic equality per collection: stable IDs, keys, display names,
    // requiredness, presence/absence, Dates, formula sources and calculated
    // values are all carried by the projection itself.
    for collection in ["orders", "products", copy_key.as_str()] {
        assert_eq!(
            normalized(table(&mut reopened, collection)),
            normalized(table(&mut source, collection)),
            "collection '{collection}' reopens with identical meaning"
        );
    }
    assert_eq!(
        grouped_sum(&mut reopened),
        grouped_sum(&mut source),
        "the saved grouped-sum definition and its results survive"
    );

    // Spot-check the features that a scalar-only assertion could hide.
    let orders = table(&mut reopened, "orders");
    let ordered = cell(&orders, "order-note", "ordered_on");
    assert_eq!(
        ordered.stored,
        Some(StoredValueProjection::Date {
            value: Date::parse("2026-09-27").unwrap()
        })
    );
    let blank_row = orders
        .rows
        .iter()
        .find(|row| row.id == "order-note")
        .expect("the blank row reopens");
    assert_eq!(
        blank_row.blank_fields,
        vec!["note".to_owned(), "shipped".to_owned()],
        "optional fields with no value stay absent, not placeholdered"
    );
    assert!(
        blank_row
            .fields
            .iter()
            .all(|field| !matches!(field.target.field.as_str(), "note" | "shipped"))
    );
    let empty_note = cell(&orders, "order-spare", "note");
    assert_eq!(
        empty_note.stored,
        Some(StoredValueProjection::Text {
            value: String::new()
        }),
        "a present empty Text stays distinguishable from an absent one"
    );
    assert_eq!(calculated(cell(&orders, "order-pen", "total")), Some(12.0));
    assert_eq!(
        grouped_sum(&mut reopened).groups,
        vec![
            tachiko_designer_runtime::KeyedGroupedSumGroupProjection {
                category: "Paper".to_owned(),
                value: 20.0,
            },
            tachiko_designer_runtime::KeyedGroupedSumGroupProjection {
                category: "Stationery".to_owned(),
                value: 18.0,
            },
        ]
    );

    // Re-saving the reopened occurrence is byte-identical: the round trip
    // loses nothing that the canonical v3 encoding can express.
    assert_eq!(
        reopened
            .export_project_v3("resident/0")
            .expect("the reopened project exports again")
            .bytes,
        saved.bytes
    );

    // No stored result is trusted on reopen: an edited input recomputes.
    let quantity = cell(&orders, "order-pen", "quantity").target.clone();
    let DesignerResponse::Published(publication) = handle(
        &mut reopened,
        DesignerRequest::EditScalar {
            expected_revision: "resident/0".to_owned(),
            target: quantity,
            input: ScalarEditInput::Number {
                input: "10".to_owned(),
            },
        },
    ) else {
        panic!("expected a publication");
    };
    assert_eq!(publication.resulting_revision, "resident/1");
    let recomputed = table(&mut reopened, "orders");
    assert_eq!(
        calculated(cell(&recomputed, "order-pen", "total")),
        Some(30.0)
    );
    assert_eq!(
        calculated(cell(&recomputed, "order-note", "total")),
        Some(20.0)
    );
}

#[test]
fn v3_saved_project_round_trips_through_a_portable_ro() {
    let source = DesignerRuntime::from_document(kitchen_sink_document(), OCCURRENCE)
        .expect("the kitchen-sink document is admissible");
    let mut source = source;
    let copy_key = duplicate(&mut source, "resident/0");
    let saved = source
        .export_project_v3("resident/1")
        .expect("the kitchen-sink document exports as v3");
    let reopened = open_saved(&saved.bytes, REOPENED);
    let mut reopened = reopened;

    let portable = reopened
        .export_portable_ro("resident/0")
        .expect("Date, formulas and saved definitions export as portable .ro");
    verify_portable_ro(&portable.bytes).expect("the portable artifact verifies");

    let mut portable_runtime = None;
    open_portable_ro(&mut portable_runtime, &portable.bytes, PORTABLE)
        .expect("the portable artifact opens");
    let mut portable_runtime = portable_runtime.expect("a successful open installs the occurrence");

    assert_eq!(
        bootstrap(&mut portable_runtime, PORTABLE),
        bootstrap(&mut reopened, REOPENED)
    );
    for collection in ["orders", "products", copy_key.as_str()] {
        assert_eq!(
            normalized(table(&mut portable_runtime, collection)),
            normalized(table(&mut reopened, collection)),
            "collection '{collection}' survives the portable artifact"
        );
    }
    assert_eq!(
        grouped_sum(&mut portable_runtime),
        grouped_sum(&mut reopened)
    );
    let portable_saved = portable_runtime
        .export_project_v3("resident/0")
        .expect("the portable-opened project exports as v3");
    assert_eq!(
        portable_saved.bytes, saved.bytes,
        "the portable artifact and the v3 project carry the same document"
    );
}
