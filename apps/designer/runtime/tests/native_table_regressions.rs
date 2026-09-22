use std::collections::BTreeMap;

use tachiko_designer_runtime::{
    DesignerRequest, DesignerRuntime, NewTableColumnInput, open_project,
};
use tachiko_workspace_engine::{
    Date, Document, DocumentId, Entity, EntityId, EntityKey, FieldDefinition, FieldId, FieldKey,
    FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000317";
const REOPENED_OCCURRENCE: &str = "00000000-0000-4000-8000-000000000318";

fn scalar_document(ids_resemble_legacy_native_table: bool, all_fields_required: bool) -> Document {
    let (document_id, schema_id, entity_id, field_ids) = if ids_resemble_legacy_native_table {
        (
            "native_table_document_0001_spoof",
            "native_table_schema_0002_spoof",
            "native_table_row_0007_spoof",
            [
                "native_table_field_0003_spoof",
                "native_table_field_0004_spoof",
                "native_table_field_0005_spoof",
                "native_table_field_0006_spoof",
            ],
        )
    } else {
        (
            "document-respelled-without-provenance",
            "schema-respelled-without-provenance",
            "entity-respelled-without-provenance",
            [
                "field-item-respelled",
                "field-quantity-respelled",
                "field-active-respelled",
                "field-received-respelled",
            ],
        )
    };
    let schema_id = SchemaId::from(schema_id);
    let field_ids = field_ids.map(FieldId::from);
    let fields = [
        ("item", FieldType::Text),
        ("quantity", FieldType::Number),
        ("active", FieldType::Boolean),
        ("received", FieldType::Date),
    ]
    .into_iter()
    .zip(field_ids.iter())
    .map(|((key, field_type), id)| {
        (
            id.clone(),
            FieldDefinition {
                id: id.clone(),
                key: FieldKey::from(key),
                field_type,
                required: all_fields_required,
                constraint: tachiko_semantic_core::FieldConstraint::None,
            },
        )
    })
    .collect();
    let entity_id = EntityId::from(entity_id);
    Document {
        id: DocumentId::from(document_id),
        title: "Inventory".to_owned(),
        schemas: BTreeMap::from([(
            schema_id.clone(),
            Schema {
                id: schema_id.clone(),
                key: SchemaKey::from("inventory"),
                fields,
            },
        )]),
        entities: BTreeMap::from([(
            entity_id.clone(),
            Entity {
                id: entity_id,
                key: EntityKey::from("existing_inventory_row"),
                schema: schema_id,
                fields: BTreeMap::from([
                    (field_ids[0].clone(), Value::Text("0012".to_owned())),
                    (
                        field_ids[1].clone(),
                        Value::Number(Number::new(3.0).expect("fixed finite value")),
                    ),
                    (field_ids[2].clone(), Value::Boolean(true)),
                    (
                        field_ids[3].clone(),
                        Value::Date(Date::parse("2024-02-29").expect("fixed Gregorian value")),
                    ),
                ]),
            },
        )]),
        keyed_grouped_sum_definitions: BTreeMap::new(),
    }
}

fn inventory() -> DesignerRuntime {
    DesignerRuntime::new_table(
        OCCURRENCE,
        "Inventory",
        &[
            NewTableColumnInput {
                name: "item".to_owned(),
                field_type: "text".to_owned(),
            },
            NewTableColumnInput {
                name: "quantity".to_owned(),
                field_type: "number".to_owned(),
            },
            NewTableColumnInput {
                name: "active".to_owned(),
                field_type: "boolean".to_owned(),
            },
            NewTableColumnInput {
                name: "received".to_owned(),
                field_type: "date".to_owned(),
            },
        ],
    )
    .expect("the bounded Inventory fixture must admit")
}

fn table(
    runtime: &mut DesignerRuntime,
    collection: &str,
) -> tachiko_designer_runtime::TableProjection {
    let tachiko_designer_runtime::DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: collection.to_owned(),
        })
        .expect("the fixture table must be queryable")
    else {
        panic!("query_table must return a table projection");
    };
    table
}

#[test]
fn native_table_profile_requires_complete_typed_rows_without_publication() {
    let mut runtime = inventory();
    let initial = table(&mut runtime, "inventory");
    assert_eq!(initial.native_table_profile, Some(true));
    let before = runtime
        .export_project(&initial.revision)
        .expect("the initial Inventory must export")
        .bytes;

    let result = runtime.handle(DesignerRequest::PasteCells {
        expected_revision: initial.revision.clone(),
        collection: initial.collection.id,
        start_entity: None,
        start_field: initial.columns[0].id.clone(),
        rows: vec![vec!["only the item".to_owned()]],
    });

    assert!(
        result.is_err(),
        "partial new row must reject before publication"
    );
    assert_eq!(
        runtime
            .export_project(&initial.revision)
            .expect("rejected paste must keep the initial revision exportable")
            .bytes,
        before,
    );
    assert!(table(&mut runtime, "inventory").rows.is_empty());
}

#[test]
fn row_paste_rejects_non_native_collections_before_row_construction() {
    let mut runtime = DesignerRuntime::moonfall(OCCURRENCE).expect("Moonfall fixture must admit");
    let initial = table(&mut runtime, "weapons");
    assert_ne!(initial.native_table_profile, Some(true));
    let before = runtime
        .export_project(&initial.revision)
        .expect("Moonfall must export before the rejected request")
        .bytes;

    let result = runtime.handle(DesignerRequest::PasteCells {
        expected_revision: initial.revision.clone(),
        collection: initial.collection.id,
        start_entity: None,
        start_field: initial.columns[0].id.clone(),
        rows: vec![vec!["must not append".to_owned()]],
    });

    assert!(result.is_err(), "non-native row paste must be refused");
    assert_eq!(
        runtime
            .export_project(&initial.revision)
            .expect("rejected paste must preserve the Moonfall export")
            .bytes,
        before,
    );
}

#[test]
fn scalar_row_authoring_survives_respelled_opaque_ids_and_reopen() {
    let mut runtime = DesignerRuntime::from_document(scalar_document(false, true), OCCURRENCE)
        .expect("a scalar-shaped document with respelled IDs must admit");
    let initial = table(&mut runtime, "inventory");
    assert_eq!(initial.native_table_profile, Some(true));
    let initial_row = initial.rows[0].id.clone();
    let bytes = runtime
        .export_project(&initial.revision)
        .expect("the respelled document must export")
        .bytes;

    let mut reopened = None;
    open_project(&mut reopened, &bytes, REOPENED_OCCURRENCE)
        .expect("the respelled document must reopen");
    let runtime = reopened
        .as_mut()
        .expect("successful reopen installs a fresh occurrence");
    let reopened_table = table(runtime, "inventory");
    assert_eq!(reopened_table.native_table_profile, Some(true));
    let publication = runtime
        .handle(DesignerRequest::PasteCells {
            expected_revision: reopened_table.revision,
            collection: reopened_table.collection.id,
            start_entity: None,
            start_field: reopened_table.columns[0].id.clone(),
            rows: vec![
                reopened_table
                    .columns
                    .iter()
                    .map(|column| match column.field_type.as_str() {
                        "text" => "continued".to_owned(),
                        "number" => "4".to_owned(),
                        "boolean" => "false".to_owned(),
                        "date" => "2026-02-01".to_owned(),
                        other => panic!("scalar-shaped fixture has unsupported {other} column"),
                    })
                    .collect(),
            ],
        })
        .expect("reopened scalar-shaped table must allocate a row without parsing persisted IDs");
    let tachiko_designer_runtime::DesignerResponse::Published(publication) = publication else {
        panic!("typed scalar paste must publish");
    };
    let after_append = table(runtime, "inventory");
    assert_eq!(after_append.revision, publication.resulting_revision);
    assert_eq!(after_append.native_table_profile, Some(true));
    assert_eq!(after_append.rows.len(), 2);
    assert_ne!(after_append.rows[1].id, initial_row);
}

#[test]
fn legacy_looking_ids_do_not_grant_scalar_row_authoring_without_the_shape() {
    let mut runtime = DesignerRuntime::from_document(scalar_document(true, false), OCCURRENCE)
        .expect("a document with optional scalar fields must still admit normally");
    let initial = table(&mut runtime, "inventory");
    assert_ne!(initial.native_table_profile, Some(true));

    let result = runtime.handle(DesignerRequest::PasteCells {
        expected_revision: initial.revision,
        collection: initial.collection.id,
        start_entity: None,
        start_field: initial.columns[0].id.clone(),
        rows: vec![vec![
            "must not append".to_owned(),
            "1".to_owned(),
            "true".to_owned(),
            "2026-02-01".to_owned(),
        ]],
    });
    assert!(
        result.is_err(),
        "legacy-looking IDs must not grant capability"
    );
}
