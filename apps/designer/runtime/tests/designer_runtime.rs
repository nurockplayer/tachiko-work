use std::collections::BTreeMap;

use tachiko_designer_runtime::{
    CalculationProjection, DesignerRequest, DesignerResponse, DesignerRuntime, DesignerWireReply,
    ScalarEditInput, ScalarKind, StoredValueProjection, close_project, open_project,
    process_wire_request,
};
use tachiko_workspace_engine::{
    Date, Document, DocumentId, Entity, EntityId, EntityKey, Expression, FieldAddress,
    FieldDefinition, FieldId, FieldKey, FieldRef, FieldType, IdGenerator, Number, Schema, SchemaId,
    SchemaKey, SemanticIdKind, StarterTemplate, Value, create_document,
};

const OCCURRENCE_ZERO: &str = "00000000-0000-4000-8000-000000000000";
const OCCURRENCE_ONE: &str = "00000000-0000-4000-8000-000000000001";
const OCCURRENCE_TWO: &str = "00000000-0000-4000-8000-000000000002";

fn moonfall() -> DesignerRuntime {
    DesignerRuntime::moonfall(OCCURRENCE_ZERO).expect("fixture should be valid")
}

#[derive(Default)]
struct TestIds(u64);

impl IdGenerator for TestIds {
    fn generate(&mut self, _kind: SemanticIdKind) -> String {
        self.0 += 1;
        format!("test_id_{:03}", self.0)
    }
}

fn product_gap_document_without_formulas() -> Document {
    Document {
        id: DocumentId::from("tachiko_product_gaps"),
        title: "Tachiko Product Gaps".to_owned(),
        schemas: product_gap_schemas_without_formulas(),
        entities: product_gap_entities_without_formulas(),
    }
}

fn product_gap_schemas_without_formulas() -> BTreeMap<SchemaId, Schema> {
    let areas = SchemaId::from("schema_areas");
    let product_gaps = SchemaId::from("schema_product_gaps");
    let roadmap_notes = SchemaId::from("schema_roadmap_notes");
    let label = FieldId::from("field_label");
    let title = FieldId::from("field_title");
    let confirmed = FieldId::from("field_confirmed");

    BTreeMap::from([
        (
            areas.clone(),
            Schema {
                id: areas.clone(),
                key: SchemaKey::from("areas"),
                fields: BTreeMap::from([(
                    label.clone(),
                    FieldDefinition {
                        id: label.clone(),
                        key: FieldKey::from("label"),
                        field_type: FieldType::Text,
                        required: true,
                    },
                )]),
            },
        ),
        (
            product_gaps.clone(),
            Schema {
                id: product_gaps.clone(),
                key: SchemaKey::from("product_gaps"),
                fields: BTreeMap::from([
                    (
                        title.clone(),
                        FieldDefinition {
                            id: title.clone(),
                            key: FieldKey::from("title"),
                            field_type: FieldType::Text,
                            required: true,
                        },
                    ),
                    (
                        confirmed.clone(),
                        FieldDefinition {
                            id: confirmed.clone(),
                            key: FieldKey::from("confirmed"),
                            field_type: FieldType::Boolean,
                            required: true,
                        },
                    ),
                ]),
            },
        ),
        (
            roadmap_notes.clone(),
            Schema {
                id: roadmap_notes.clone(),
                key: SchemaKey::from("roadmap_notes"),
                fields: BTreeMap::from([(
                    title.clone(),
                    FieldDefinition {
                        id: title.clone(),
                        key: FieldKey::from("title"),
                        field_type: FieldType::Text,
                        required: true,
                    },
                )]),
            },
        ),
    ])
}

fn product_gap_entities_without_formulas() -> BTreeMap<EntityId, Entity> {
    let areas = SchemaId::from("schema_areas");
    let product_gaps = SchemaId::from("schema_product_gaps");
    let roadmap_notes = SchemaId::from("schema_roadmap_notes");
    let label = FieldId::from("field_label");
    let title = FieldId::from("field_title");
    let confirmed = FieldId::from("field_confirmed");

    BTreeMap::from([
        (
            EntityId::from("area_designer"),
            Entity {
                id: EntityId::from("area_designer"),
                key: EntityKey::from("designer"),
                schema: areas,
                fields: BTreeMap::from([(label, Value::Text("Designer".to_owned()))]),
            },
        ),
        (
            EntityId::from("gap_authoring"),
            Entity {
                id: EntityId::from("gap_authoring"),
                key: EntityKey::from("schema_authoring"),
                schema: product_gaps.clone(),
                fields: BTreeMap::from([
                    (
                        title.clone(),
                        Value::Text("Schema authoring is not exposed".to_owned()),
                    ),
                    (confirmed.clone(), Value::Boolean(true)),
                ]),
            },
        ),
        (
            EntityId::from("gap_persistence"),
            Entity {
                id: EntityId::from("gap_persistence"),
                key: EntityKey::from("create_only_persistence"),
                schema: product_gaps,
                fields: BTreeMap::from([
                    (
                        title.clone(),
                        Value::Text("Browser persistence is create-only".to_owned()),
                    ),
                    (confirmed, Value::Boolean(false)),
                ]),
            },
        ),
        (
            EntityId::from("note_beta"),
            Entity {
                id: EntityId::from("note_beta"),
                key: EntityKey::from("team_workspace_beta"),
                schema: roadmap_notes.clone(),
                fields: BTreeMap::from([(
                    title.clone(),
                    Value::Text("Team Workspace Beta".to_owned()),
                )]),
            },
        ),
        (
            EntityId::from("note_designer"),
            Entity {
                id: EntityId::from("note_designer"),
                key: EntityKey::from("designer_mvp"),
                schema: roadmap_notes,
                fields: BTreeMap::from([(title, Value::Text("Designer MVP".to_owned()))]),
            },
        ),
    ])
}

fn date_document() -> Document {
    Document {
        id: DocumentId::from("date_project"),
        title: "Date Project".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("events"),
            Schema {
                id: SchemaId::from("events"),
                key: SchemaKey::from("events"),
                fields: BTreeMap::from([(
                    FieldId::from("published"),
                    FieldDefinition {
                        id: FieldId::from("published"),
                        key: FieldKey::from("published"),
                        field_type: FieldType::Date,
                        required: true,
                    },
                )]),
            },
        )]),
        entities: BTreeMap::from([(
            EntityId::from("launch"),
            Entity {
                id: EntityId::from("launch"),
                key: EntityKey::from("launch"),
                schema: SchemaId::from("events"),
                fields: BTreeMap::from([(
                    FieldId::from("published"),
                    Value::Date(Date::parse("2024-02-29").unwrap()),
                )]),
            },
        )]),
    }
}

#[test]
fn bootstrap_exposes_fixture_collections_without_a_document_snapshot() {
    let mut runtime = moonfall();

    let DesignerResponse::Bootstrap(bootstrap) = runtime
        .handle(DesignerRequest::Bootstrap {
            occurrence_id: OCCURRENCE_ZERO.to_owned(),
        })
        .expect("bootstrap should succeed")
    else {
        panic!("expected bootstrap response");
    };

    assert_eq!(bootstrap.title, "Moonfall Balance");
    assert_eq!(bootstrap.revision, "resident/0");
    assert_eq!(bootstrap.default_collection, "weapons");
    assert_eq!(
        bootstrap
            .collections
            .iter()
            .map(|collection| collection.key.as_str())
            .collect::<Vec<_>>(),
        ["characters", "economy", "items", "weapons"]
    );
}

#[test]
fn admission_selects_a_bounded_non_moonfall_collection_without_formulas() {
    let mut runtime =
        DesignerRuntime::from_document(product_gap_document_without_formulas(), OCCURRENCE_ONE)
            .expect("an ordinary bounded project should be admitted");

    let DesignerResponse::Bootstrap(bootstrap) = runtime
        .handle(DesignerRequest::Bootstrap {
            occurrence_id: OCCURRENCE_ONE.to_owned(),
        })
        .expect("bootstrap should succeed")
    else {
        panic!("expected bootstrap response");
    };
    assert_eq!(bootstrap.title, "Tachiko Product Gaps");
    assert_eq!(bootstrap.default_collection, "product_gaps");

    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: bootstrap.default_collection,
        })
        .expect("the deterministic default collection should be queryable")
    else {
        panic!("expected table response");
    };
    assert_eq!(table.collection.entity_count, 2);
    assert_eq!(table.rows.len(), 2);
    assert!(
        table
            .rows
            .iter()
            .flat_map(|row| &row.fields)
            .all(|field| field.formula.is_none())
    );
}

#[test]
fn default_collection_breaks_equal_size_ties_by_human_key() {
    let mut document = product_gap_document_without_formulas();
    let product_gaps_schema = document
        .schemas
        .values()
        .find(|schema| schema.key.as_str() == "product_gaps")
        .expect("product gap schema should exist")
        .clone();
    let mut later_schema = product_gaps_schema.clone();
    later_schema.id = SchemaId::from("schema_z_product_gaps");
    later_schema.key = SchemaKey::from("z_product_gaps");
    let later_schema_id = later_schema.id.clone();
    document
        .schemas
        .insert(later_schema_id.clone(), later_schema);

    let product_gap_entities = document
        .entities
        .values()
        .filter(|entity| entity.schema == product_gaps_schema.id)
        .cloned()
        .collect::<Vec<_>>();
    for (index, mut entity) in product_gap_entities.into_iter().enumerate() {
        entity.id = EntityId::from(format!("z_product_gap_{index}"));
        entity.key = EntityKey::from(format!("z_product_gap_{index}"));
        entity.schema = later_schema_id.clone();
        document.entities.insert(entity.id.clone(), entity);
    }

    let mut runtime = DesignerRuntime::from_document(document, OCCURRENCE_ONE)
        .expect("equal-sized bounded collections should be admitted");
    let DesignerResponse::Bootstrap(bootstrap) = runtime
        .handle(DesignerRequest::Bootstrap {
            occurrence_id: OCCURRENCE_ONE.to_owned(),
        })
        .expect("bootstrap should succeed")
    else {
        panic!("expected bootstrap response");
    };
    assert_eq!(bootstrap.default_collection, "product_gaps");
}

#[test]
fn admission_rejects_any_collection_that_cannot_be_rendered() {
    let mut document = create_document(
        StarterTemplate::GameBalance,
        "Oversized collection",
        &mut TestIds::default(),
    )
    .expect("fixture should be valid");
    let items = document
        .schemas
        .values_mut()
        .find(|schema| schema.key.as_str() == "items")
        .expect("items schema should exist");
    while items.fields.len() <= 32 {
        let key = format!("overflow_{:02}", items.fields.len());
        items.fields.insert(
            FieldId::from(key.as_str()),
            FieldDefinition {
                id: FieldId::from(key.as_str()),
                key: FieldKey::from(key.as_str()),
                field_type: FieldType::Number,
                required: false,
            },
        );
    }

    let Err(error) = DesignerRuntime::from_document(document, OCCURRENCE_ONE) else {
        panic!("every advertised collection must fit the bounded table profile");
    };
    assert_eq!(
        error.failure_projection("unavailable").code,
        "unsupported_project"
    );
}

#[test]
fn admission_rejects_an_unbounded_collection_catalog() {
    let mut document = create_document(
        StarterTemplate::GameBalance,
        "Too many collections",
        &mut TestIds::default(),
    )
    .expect("fixture should be valid");
    for index in 0..64 {
        let id = format!("extra_schema_{index:02}");
        let key = format!("extra_collection_{index:02}");
        document.schemas.insert(
            SchemaId::from(id.as_str()),
            Schema {
                id: SchemaId::from(id.as_str()),
                key: SchemaKey::from(key.as_str()),
                fields: BTreeMap::default(),
            },
        );
    }

    let Err(error) = DesignerRuntime::from_document(document, OCCURRENCE_ONE) else {
        panic!("the advertised collection catalog must remain bounded");
    };
    let failure = error.failure_projection("unavailable");
    assert_eq!(failure.code, "unsupported_project");
    assert!(failure.message.contains("bounded maximum is 32"));
}

#[test]
fn admission_bounds_document_identity_before_authority_construction() {
    let mut document = create_document(
        StarterTemplate::GameBalance,
        "Oversized identity",
        &mut TestIds::default(),
    )
    .expect("fixture should be valid");
    document.id = DocumentId::from("d".repeat(4_097));

    let Err(error) = DesignerRuntime::from_document(document, OCCURRENCE_ONE) else {
        panic!("authority scope construction must receive a bounded document identity");
    };
    let failure = error.failure_projection("unavailable");
    assert_eq!(failure.code, "unsupported_project");
    assert!(failure.message.contains("4096-byte maximum"));
}

#[test]
fn admission_bounds_document_title_before_validation_and_projection() {
    let mut document = create_document(
        StarterTemplate::GameBalance,
        "Bounded title",
        &mut TestIds::default(),
    )
    .expect("fixture should be valid");
    document.title = "t".repeat(4_097);

    let Err(error) = DesignerRuntime::from_document(document, OCCURRENCE_ONE) else {
        panic!("projection construction must receive a bounded document title");
    };
    let failure = error.failure_projection("unavailable");
    assert_eq!(failure.code, "unsupported_project");
    assert!(failure.message.contains("document title"));
    assert!(failure.message.contains("4096-byte maximum"));
}

#[test]
fn admission_bounds_aggregate_profile_strings_before_projection_construction() {
    let mut document = create_document(
        StarterTemplate::GameBalance,
        "Oversized profile strings",
        &mut TestIds::default(),
    )
    .expect("fixture should be valid");
    let schema = document
        .schemas
        .values_mut()
        .next()
        .expect("fixture should contain a schema");
    for index in 0..16 {
        let field_id = FieldId::from(format!("aggregate_profile_{index}_{}", "i".repeat(2_500)));
        schema.fields.insert(
            field_id.clone(),
            FieldDefinition {
                id: field_id,
                key: FieldKey::from(format!("aggregate_profile_{index}")),
                field_type: FieldType::Number,
                required: false,
            },
        );
    }

    let Err(error) = DesignerRuntime::from_document(document, OCCURRENCE_ONE) else {
        panic!("projection construction must receive bounded aggregate profile strings");
    };
    let failure = error.failure_projection("unavailable");
    assert_eq!(failure.code, "unsupported_project");
    assert!(
        failure
            .message
            .contains("aggregate project profile strings")
    );
    assert!(failure.message.contains("65536-byte projection maximum"));
}

#[test]
fn admission_bounds_stored_text_before_validation_and_projection() {
    let mut document = create_document(
        StarterTemplate::GameBalance,
        "Oversized stored text",
        &mut TestIds::default(),
    )
    .expect("fixture should be valid");
    let value = document
        .entities
        .values_mut()
        .flat_map(|entity| entity.fields.values_mut())
        .next()
        .expect("fixture should contain a stored value");
    *value = Value::Text("t".repeat(65_537));

    let Err(error) = DesignerRuntime::from_document(document, OCCURRENCE_ONE) else {
        panic!("projection construction must receive bounded stored text");
    };
    let failure = error.failure_projection("unavailable");
    assert_eq!(failure.code, "unsupported_project");
    assert!(failure.message.contains("stored text"));
    assert!(failure.message.contains("65536-byte projection maximum"));
}

#[test]
fn admission_bounds_aggregate_collection_text_before_validation_and_projection() {
    let mut document = create_document(
        StarterTemplate::GameBalance,
        "Oversized collection text",
        &mut TestIds::default(),
    )
    .expect("fixture should be valid");
    let schema_id = document
        .schemas
        .values()
        .next()
        .expect("fixture should contain a schema")
        .id
        .clone();
    let entity_id = document
        .entities
        .values()
        .find(|entity| entity.schema == schema_id)
        .expect("fixture schema should contain an entity")
        .id
        .clone();
    for index in 0..3 {
        let field_id = FieldId::from(format!("aggregate_text_{index}"));
        document
            .schemas
            .get_mut(&schema_id)
            .expect("fixture schema should exist")
            .fields
            .insert(
                field_id.clone(),
                FieldDefinition {
                    id: field_id.clone(),
                    key: FieldKey::from(format!("aggregate_text_{index}")),
                    field_type: FieldType::Text,
                    required: false,
                },
            );
        document
            .entities
            .get_mut(&entity_id)
            .expect("fixture entity should exist")
            .fields
            .insert(field_id, Value::Text("t".repeat(22_000)));
    }

    let Err(error) = DesignerRuntime::from_document(document, OCCURRENCE_ONE) else {
        panic!("table construction must receive bounded aggregate stored text");
    };
    let failure = error.failure_projection("unavailable");
    assert_eq!(failure.code, "unsupported_project");
    assert!(failure.message.contains("a collection contains more than"));
    assert!(failure.message.contains("65536-byte stored-text"));
}

#[test]
fn admission_bounds_the_complete_cross_collection_post_edit_refresh() {
    let mut document = create_document(
        StarterTemplate::GameBalance,
        "Oversized post-edit refresh",
        &mut TestIds::default(),
    )
    .expect("fixture should be valid");
    let schema_ids = document.schemas.keys().cloned().collect::<Vec<_>>();
    for (collection_index, schema_id) in schema_ids.into_iter().enumerate() {
        let entity_id = document
            .entities
            .values()
            .find(|entity| entity.schema == schema_id)
            .expect("fixture schema should contain an entity")
            .id
            .clone();
        let source_id = FieldId::from(format!("refresh_source_{collection_index}"));
        let source_key = FieldKey::from(format!(
            "refresh_source_{}{}",
            collection_index,
            "s".repeat(3_400)
        ));
        let schema = document
            .schemas
            .get_mut(&schema_id)
            .expect("fixture schema should exist");
        schema.fields.insert(
            source_id.clone(),
            FieldDefinition {
                id: source_id.clone(),
                key: source_key,
                field_type: FieldType::Number,
                required: false,
            },
        );
        let entity = document
            .entities
            .get_mut(&entity_id)
            .expect("fixture entity should exist");
        entity.fields.insert(
            source_id.clone(),
            Value::Number(Number::new(1.0).expect("finite fixture number")),
        );
        for formula_index in 0..6 {
            let formula_id = FieldId::from(format!(
                "refresh_formula_{collection_index}_{formula_index}"
            ));
            schema.fields.insert(
                formula_id.clone(),
                FieldDefinition {
                    id: formula_id.clone(),
                    key: FieldKey::from(format!(
                        "refresh_formula_{collection_index}_{formula_index}"
                    )),
                    field_type: FieldType::Number,
                    required: false,
                },
            );
            entity.fields.insert(
                formula_id,
                Value::Formula(Expression::Reference(FieldRef::new(
                    entity_id.clone(),
                    source_id.clone(),
                ))),
            );
        }
    }

    let Err(error) = DesignerRuntime::from_document(document, OCCURRENCE_ONE) else {
        panic!("candidate admission must bound the complete post-edit refresh");
    };
    let failure = error.failure_projection("unavailable");
    assert_eq!(failure.code, "unsupported_project");
    assert!(
        failure.message.contains("worst-case post-edit refresh"),
        "{}",
        failure.message
    );
    assert!(failure.message.contains("bounded maximum is 65536"));
}

#[test]
fn admission_bounds_formula_reference_identity_before_validation() {
    let mut document = create_document(
        StarterTemplate::GameBalance,
        "Oversized formula reference",
        &mut TestIds::default(),
    )
    .expect("fixture should be valid");
    let formula = document
        .entities
        .values_mut()
        .flat_map(|entity| entity.fields.values_mut())
        .find(|value| matches!(value, Value::Formula(_)))
        .expect("fixture should contain a formula");
    *formula = Value::Formula(Expression::Reference(FieldRef::new(
        "e".repeat(4_097),
        "bounded_field",
    )));

    let Err(error) = DesignerRuntime::from_document(document, OCCURRENCE_ONE) else {
        panic!("formula validation must receive bounded reference identities");
    };
    let failure = error.failure_projection("unavailable");
    assert_eq!(failure.code, "unsupported_project");
    assert!(
        failure
            .message
            .contains("formula reference entity identity")
    );
    assert!(failure.message.contains("4096-byte maximum"));
}

#[test]
fn admission_bounds_formula_analysis_before_per_formula_projection() {
    let mut document = create_document(
        StarterTemplate::GameBalance,
        "Too many formulas",
        &mut TestIds::default(),
    )
    .expect("fixture should be valid");
    let formula_target = document
        .resolve_field(&FieldAddress::new("shop", "upgrade_cost"))
        .expect("fixture formula should resolve");
    let formula = document.entities[&formula_target.entity].fields[&formula_target.field].clone();
    assert!(matches!(formula, Value::Formula(_)));

    for (template_key, field_key, prefix) in [
        ("iron_sword", "damage", "weapon_formula"),
        ("alric", "level", "character_formula"),
    ] {
        let target = document
            .resolve_field(&FieldAddress::new(template_key, field_key))
            .expect("fixture Number field should resolve");
        let template = document.entities[&target.entity].clone();
        for index in 0..17 {
            let id = format!("{prefix}_entity_{index:02}");
            let key = format!("{prefix}_{index:02}");
            let mut entity = template.clone();
            entity.id = EntityId::from(id.as_str());
            entity.key = EntityKey::from(key.as_str());
            entity.fields.insert(target.field.clone(), formula.clone());
            document.entities.insert(entity.id.clone(), entity);
        }
    }
    let formula_count = document
        .entities
        .values()
        .flat_map(|entity| entity.fields.values())
        .filter(|value| matches!(value, Value::Formula(_)))
        .count();
    assert!(formula_count > 32);
    document
        .schemas
        .values_mut()
        .find(|schema| schema.key.as_str() == "weapons")
        .expect("weapons schema should exist")
        .id = SchemaId::from("deliberate_map_key_mismatch");

    let Err(error) = DesignerRuntime::from_document(document, OCCURRENCE_ONE) else {
        panic!("formula analysis must have a cheap aggregate admission bound");
    };
    let failure = error.failure_projection("unavailable");
    assert_eq!(failure.code, "unsupported_project");
    assert!(
        failure
            .message
            .contains(&format!("contains {formula_count} formulas"))
    );
    assert!(failure.message.contains("bounded maximum is 32"));
}

#[test]
fn admission_bounds_total_entities_before_workspace_validation() {
    let mut document = create_document(
        StarterTemplate::GameBalance,
        "Too many malformed entities",
        &mut TestIds::default(),
    )
    .expect("fixture should be valid");
    let template = document
        .entities
        .values()
        .next()
        .expect("fixture entity should exist")
        .clone();
    document.entities.clear();
    for index in 0..1_025 {
        let id = format!("malformed_entity_{index:04}");
        let mut entity = template.clone();
        entity.id = EntityId::from(id.as_str());
        entity.key = EntityKey::from(id.as_str());
        entity.schema = SchemaId::from(format!("missing_schema_{index:04}"));
        entity.fields.clear();
        document.entities.insert(entity.id.clone(), entity);
    }

    let Err(error) = DesignerRuntime::from_document(document, OCCURRENCE_ONE) else {
        panic!("total entities must be bounded before workspace validation");
    };
    let failure = error.failure_projection("unavailable");
    assert_eq!(failure.code, "unsupported_project");
    assert!(failure.message.contains("contains 1025 entities"));
    assert!(failure.message.contains("bounded maximum is 1024"));
}

#[test]
fn admission_bounds_entity_fields_before_workspace_validation() {
    let mut document = create_document(
        StarterTemplate::GameBalance,
        "Too many undeclared fields",
        &mut TestIds::default(),
    )
    .expect("fixture should be valid");
    let entity = document
        .entities
        .values_mut()
        .find(|entity| entity.key.as_str() == "iron_sword")
        .expect("fixture entity should exist");
    while entity.fields.len() <= 32 {
        let field = FieldId::from(format!("undeclared_field_{:02}", entity.fields.len()));
        entity.fields.insert(
            field,
            Value::Number(Number::new(1.0).expect("finite fixture number")),
        );
    }

    let Err(error) = DesignerRuntime::from_document(document, OCCURRENCE_ONE) else {
        panic!("stored entity fields must be bounded before workspace validation");
    };
    let failure = error.failure_projection("unavailable");
    assert_eq!(failure.code, "unsupported_project");
    assert!(failure.message.contains("contains 33 stored fields"));
    assert!(failure.message.contains("bounded maximum is 32"));
}

#[test]
fn admission_rejects_an_oversized_serialized_table_projection() {
    let mut document = create_document(
        StarterTemplate::GameBalance,
        "Oversized projection",
        &mut TestIds::default(),
    )
    .expect("fixture should be valid");
    let name = document
        .resolve_field(&FieldAddress::new("iron_sword", "name"))
        .expect("fixture name field should resolve");
    document
        .entities
        .get_mut(&name.entity)
        .expect("fixture entity should exist")
        .fields
        .insert(name.field, Value::Text("x".repeat(65_500)));

    let Err(error) = DesignerRuntime::from_document(document, OCCURRENCE_ONE) else {
        panic!("serialized table projections must remain bounded");
    };
    let failure = error.failure_projection("unavailable");
    assert_eq!(failure.code, "unsupported_project");
    assert!(failure.message.contains("bounded maximum is 65536"));
}

#[test]
fn admission_rejects_an_unbounded_worst_case_publication_projection() {
    let mut document = create_document(
        StarterTemplate::GameBalance,
        "Oversized publication",
        &mut TestIds::default(),
    )
    .expect("fixture should be valid");
    let items_schema = document
        .schemas
        .values_mut()
        .find(|schema| schema.key.as_str() == "items")
        .expect("items schema should exist");
    let schema_id = items_schema.id.clone();
    while items_schema.fields.len() < 32 {
        let index = items_schema.fields.len();
        let id = format!("publication_fanout_field_identifier_{index:02}");
        let key = format!("fanout_field_{index:02}");
        items_schema.fields.insert(
            FieldId::from(id.as_str()),
            FieldDefinition {
                id: FieldId::from(id.as_str()),
                key: FieldKey::from(key.as_str()),
                field_type: FieldType::Number,
                required: false,
            },
        );
    }
    let number_fields = items_schema
        .fields
        .values()
        .filter(|field| matches!(field.field_type, FieldType::Number))
        .map(|field| field.id.clone())
        .collect::<Vec<_>>();
    let template = document
        .entities
        .values()
        .find(|entity| entity.schema == schema_id)
        .expect("items fixture entity should exist")
        .clone();
    while document
        .entities
        .values()
        .filter(|entity| entity.schema == schema_id)
        .count()
        < 32
    {
        let index = document.entities.len();
        let id = format!("publication_fanout_entity_identifier_{index:02}");
        let key = format!("fanout_entity_{index:02}");
        let mut entity = template.clone();
        entity.id = EntityId::from(id.as_str());
        entity.key = EntityKey::from(key.as_str());
        for field in &number_fields {
            entity.fields.insert(
                field.clone(),
                Value::Number(Number::new(1.0).expect("finite fixture number")),
            );
        }
        document.entities.insert(entity.id.clone(), entity);
    }

    let Err(error) = DesignerRuntime::from_document(document, OCCURRENCE_ONE) else {
        panic!("the worst-case publication projection must remain bounded");
    };
    let failure = error.failure_projection("unavailable");
    assert_eq!(failure.code, "unsupported_project");
    assert!(
        failure
            .message
            .contains("worst-case publication projection")
    );
    assert!(failure.message.contains("bounded maximum is 65536"));
}

#[test]
fn table_query_keeps_stored_formula_and_calculated_values_distinct() {
    let mut runtime = moonfall();

    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: "weapons".to_owned(),
        })
        .expect("bounded table query should succeed")
    else {
        panic!("expected table response");
    };

    assert_eq!(table.revision, "resident/0");
    assert_eq!(table.collection.key, "weapons");
    assert_eq!(table.rows.len(), 1);
    assert_eq!(table.rows[0].key, "iron_sword");

    let damage = table.rows[0]
        .fields
        .iter()
        .find(|field| field.target.field == "damage")
        .expect("damage projection");
    assert_eq!(
        damage
            .stored
            .as_ref()
            .and_then(StoredValueProjection::number),
        Some(36.0)
    );
    assert!(damage.formula.is_none());
    assert!(damage.calculated.is_none());
    assert_eq!(damage.editable_scalar, Some(ScalarKind::Number));

    let dps = table.rows[0]
        .fields
        .iter()
        .find(|field| field.target.field == "dps")
        .expect("DPS projection");
    assert!(dps.stored.is_none());
    assert_eq!(
        dps.formula.as_ref().map(|formula| formula.source.as_str()),
        Some("([iron_sword.damage] / [iron_sword.attack_interval])")
    );
    assert_eq!(
        dps.calculated
            .as_ref()
            .and_then(CalculationProjection::number),
        Some(40.0)
    );
    assert_eq!(dps.editable_scalar, None);
}

#[test]
fn date_projection_and_edit_use_the_rust_semantic_authority() {
    let mut runtime = DesignerRuntime::from_document(date_document(), OCCURRENCE_ONE)
        .expect("date project should fit the bounded Designer profile");

    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: "events".to_owned(),
        })
        .expect("date table should be queryable")
    else {
        panic!("expected table response");
    };
    assert_eq!(table.columns[0].field_type, "date");
    assert!(matches!(
        table.rows[0].fields[0].stored,
        Some(StoredValueProjection::Date { value }) if value == Date::parse("2024-02-29").unwrap()
    ));
    assert_eq!(
        table.rows[0].fields[0].editable_scalar,
        Some(ScalarKind::Date)
    );

    let DesignerResponse::Published(publication) = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/0".to_owned(),
            target: "launch.published".into(),
            input: ScalarEditInput::Date {
                value: "2025-01-01".to_owned(),
            },
        })
        .expect("valid date edit should publish")
    else {
        panic!("expected publication response");
    };
    assert_eq!(publication.resulting_revision, "resident/1");

    let DesignerResponse::Fields(fields) = runtime
        .handle(DesignerRequest::QueryFields {
            expected_revision: "resident/1".to_owned(),
            fields: vec!["launch.published".into()],
        })
        .expect("edited date should be queryable")
    else {
        panic!("expected fields response");
    };
    assert!(matches!(
        fields.fields[0].stored,
        Some(StoredValueProjection::Date { value }) if value == Date::parse("2025-01-01").unwrap()
    ));

    let invalid = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/1".to_owned(),
            target: "launch.published".into(),
            input: ScalarEditInput::Date {
                value: "1900-02-29".to_owned(),
            },
        })
        .expect_err("fake Gregorian dates must not publish");
    assert_eq!(
        invalid.failure_projection("resident/1").code,
        "invalid_date"
    );

    let export_error = runtime
        .export_project("resident/1")
        .expect_err("frozen .roproj/v1 must not be widened for Date");
    assert_eq!(
        export_error.failure_projection("resident/1").code,
        "invalid_project"
    );
}

#[test]
fn number_edit_publishes_once_and_refreshes_only_direct_and_dependent_fields() {
    let mut runtime = moonfall();

    let DesignerResponse::Published(publication) = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/0".to_owned(),
            target: "iron_sword.damage".into(),
            input: ScalarEditInput::Number {
                input: "45".to_owned(),
            },
        })
        .expect("valid Number edit should execute")
    else {
        panic!("expected publication response");
    };

    assert_eq!(publication.base_revision, "resident/0");
    assert_eq!(publication.resulting_revision, "resident/1");
    assert_eq!(publication.fields, ["iron_sword.damage".into()]);
    assert_eq!(publication.affected_calculations, ["iron_sword.dps".into()]);
    assert!(!publication.fields.contains(&"shop.upgrade_cost".into()));
    assert!(
        !publication
            .affected_calculations
            .contains(&"shop.upgrade_cost".into())
    );

    let DesignerResponse::Fields(refreshed) = runtime
        .handle(DesignerRequest::QueryFields {
            expected_revision: publication.resulting_revision.clone(),
            fields: vec!["iron_sword.damage".into(), "iron_sword.dps".into()],
        })
        .expect("selective refresh should succeed")
    else {
        panic!("expected fields response");
    };
    assert_eq!(refreshed.revision, "resident/1");
    assert_eq!(refreshed.fields.len(), 2);
    assert_eq!(
        refreshed.fields[0]
            .stored
            .as_ref()
            .and_then(StoredValueProjection::number),
        Some(45.0)
    );
    assert_eq!(
        refreshed.fields[1]
            .calculated
            .as_ref()
            .and_then(CalculationProjection::number),
        Some(50.0)
    );
}

#[test]
fn text_and_boolean_edits_publish_against_the_resident_revision() {
    let mut runtime = moonfall();

    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: "weapons".to_owned(),
        })
        .expect("table query should succeed")
    else {
        panic!("expected table response");
    };
    let fields = &table.rows[0].fields;
    assert_eq!(
        fields
            .iter()
            .find(|field| field.target.field == "name")
            .map(|field| field.editable_scalar),
        Some(Some(ScalarKind::Text))
    );
    assert_eq!(
        fields
            .iter()
            .find(|field| field.target.field == "enabled")
            .map(|field| field.editable_scalar),
        Some(Some(ScalarKind::Boolean))
    );
    assert_eq!(
        fields
            .iter()
            .find(|field| field.target.field == "dps")
            .map(|field| field.editable_scalar),
        Some(None)
    );

    let DesignerResponse::Published(text) = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/0".to_owned(),
            target: "iron_sword.name".into(),
            input: ScalarEditInput::Text {
                value: "Longsword".to_owned(),
            },
        })
        .expect("text edit should publish")
    else {
        panic!("expected publication response");
    };
    assert_eq!(text.base_revision, "resident/0");
    assert_eq!(text.resulting_revision, "resident/1");
    assert_eq!(text.fields, ["iron_sword.name".into()]);
    assert!(text.affected_calculations.is_empty());

    let DesignerResponse::Published(boolean) = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: text.resulting_revision,
            target: "iron_sword.enabled".into(),
            input: ScalarEditInput::Boolean { value: false },
        })
        .expect("boolean edit should publish")
    else {
        panic!("expected publication response");
    };
    assert_eq!(boolean.base_revision, "resident/1");
    assert_eq!(boolean.resulting_revision, "resident/2");
    assert_eq!(boolean.fields, ["iron_sword.enabled".into()]);
    assert!(boolean.affected_calculations.is_empty());

    let DesignerResponse::Fields(fields) = runtime
        .handle(DesignerRequest::QueryFields {
            expected_revision: "resident/2".to_owned(),
            fields: vec!["iron_sword.name".into(), "iron_sword.enabled".into()],
        })
        .expect("edited scalar fields should be current")
    else {
        panic!("expected fields response");
    };
    assert!(matches!(
        fields
            .fields
            .iter()
            .find(|field| field.target.field == "name")
            .and_then(|field| field.stored.as_ref()),
        Some(StoredValueProjection::Text { value }) if value == "Longsword"
    ));
    assert!(matches!(
        fields
            .fields
            .iter()
            .find(|field| field.target.field == "enabled")
            .and_then(|field| field.stored.as_ref()),
        Some(StoredValueProjection::Boolean { value: false })
    ));
}

#[test]
fn rejected_scalar_kind_does_not_publish_a_canonical_change() {
    let mut runtime = moonfall();

    let error = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/0".to_owned(),
            target: "iron_sword.name".into(),
            input: ScalarEditInput::Boolean { value: false },
        })
        .expect_err("the Rust authority must reject a mismatched scalar kind");
    assert_eq!(
        error.failure_projection("resident/0").code,
        "unsupported_edit"
    );

    let DesignerResponse::Fields(fields) = runtime
        .handle(DesignerRequest::QueryFields {
            expected_revision: "resident/0".to_owned(),
            fields: vec!["iron_sword.name".into()],
        })
        .expect("rejected edit must leave the current projection queryable")
    else {
        panic!("expected fields response");
    };
    assert!(matches!(
        fields.fields[0].stored,
        Some(StoredValueProjection::Text { ref value }) if value == "Iron Sword"
    ));
}

#[test]
fn oversized_text_edit_is_rejected_before_publication() {
    let mut runtime = moonfall();

    let error = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/0".to_owned(),
            target: "iron_sword.name".into(),
            input: ScalarEditInput::Text {
                value: "x".repeat(65_500),
            },
        })
        .expect_err("a candidate that cannot refresh its bounded projections must not publish");
    assert_eq!(
        error.failure_projection("resident/0").code,
        "unsupported_project"
    );

    let DesignerResponse::Fields(fields) = runtime
        .handle(DesignerRequest::QueryFields {
            expected_revision: "resident/0".to_owned(),
            fields: vec!["iron_sword.name".into()],
        })
        .expect("rejected candidate must leave the resident projection unchanged")
    else {
        panic!("expected fields response");
    };
    assert!(matches!(
        fields.fields[0].stored,
        Some(StoredValueProjection::Text { ref value }) if value == "Iron Sword"
    ));
}

#[test]
fn stale_scalar_edits_are_rejected_before_candidate_preflight() {
    let mut runtime = moonfall();
    runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/0".to_owned(),
            target: "iron_sword.damage".into(),
            input: ScalarEditInput::Number {
                input: "45".to_owned(),
            },
        })
        .expect("first edit should advance the resident revision");

    let error = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/0".to_owned(),
            target: "iron_sword.name".into(),
            input: ScalarEditInput::Text {
                value: "x".repeat(65_500),
            },
        })
        .expect_err("stale edits must not construct a candidate");
    assert_eq!(
        error.failure_projection("resident/1").code,
        "stale_revision"
    );
}

#[test]
fn stale_and_calculation_failing_edits_leave_the_published_projection_unchanged() {
    let mut runtime = moonfall();
    runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/0".to_owned(),
            target: "iron_sword.damage".into(),
            input: ScalarEditInput::Number {
                input: "45".to_owned(),
            },
        })
        .expect("first edit should publish");

    let stale = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/0".to_owned(),
            target: "iron_sword.damage".into(),
            input: ScalarEditInput::Number {
                input: "50".to_owned(),
            },
        })
        .expect_err("stale expected revision must fail");
    let stale = stale.failure_projection("resident/1");
    assert_eq!(stale.code, "stale_revision");
    assert_eq!(stale.current_revision, "resident/1");

    let invalid = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/1".to_owned(),
            target: "iron_sword.attack_interval".into(),
            input: ScalarEditInput::Number {
                input: "0".to_owned(),
            },
        })
        .expect_err("division-by-zero candidate must fail validation");
    let invalid = invalid.failure_projection("resident/1");
    assert_eq!(invalid.code, "validation_failed");
    assert!(
        invalid
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "formula.division_by_zero")
    );

    let DesignerResponse::Fields(current) = runtime
        .handle(DesignerRequest::QueryFields {
            expected_revision: "resident/1".to_owned(),
            fields: vec![
                "iron_sword.attack_interval".into(),
                "iron_sword.damage".into(),
                "iron_sword.dps".into(),
            ],
        })
        .expect("current fields should remain queryable")
    else {
        panic!("expected fields response");
    };
    assert_eq!(current.revision, "resident/1");
    assert_eq!(
        current.fields[0]
            .stored
            .as_ref()
            .and_then(StoredValueProjection::number),
        Some(0.9)
    );
    assert_eq!(
        current.fields[1]
            .stored
            .as_ref()
            .and_then(StoredValueProjection::number),
        Some(45.0)
    );
    assert_eq!(
        current.fields[2]
            .calculated
            .as_ref()
            .and_then(CalculationProjection::number),
        Some(50.0)
    );
}

#[test]
fn wire_bridge_returns_bounded_results_and_structured_failures() {
    let mut runtime = None;

    let invalid_occurrence = process_wire_request(
        &mut runtime,
        br#"{"type":"bootstrap","occurrence_id":"reused-or-untrusted"}"#,
    );
    let DesignerWireReply::Error { error } =
        serde_json::from_slice(&invalid_occurrence).expect("failure should decode")
    else {
        panic!("invalid host occurrence must fail closed");
    };
    assert_eq!(error.code, "invalid_occurrence");
    assert!(runtime.is_none());

    let bootstrap = process_wire_request(
        &mut runtime,
        br#"{"type":"bootstrap","occurrence_id":"00000000-0000-4000-8000-000000000000"}"#,
    );
    let bootstrap_text = String::from_utf8(bootstrap.clone()).expect("JSON should be UTF-8");
    assert!(!bootstrap_text.contains("\"schemas\""));
    assert!(!bootstrap_text.contains("\"entities\""));
    let DesignerWireReply::Ok { response } =
        serde_json::from_slice(&bootstrap).expect("reply should decode")
    else {
        panic!("expected successful bootstrap reply");
    };
    assert!(matches!(response, DesignerResponse::Bootstrap(_)));

    let invalid = process_wire_request(
        &mut runtime,
        br#"{"type":"edit_scalar","expected_revision":"resident/0","target":{"entity":"iron_sword","field":"damage"},"input":{"kind":"number","input":"not-a-number"}}"#,
    );
    let DesignerWireReply::Error { error } =
        serde_json::from_slice(&invalid).expect("failure should decode")
    else {
        panic!("expected structured failure reply");
    };
    assert_eq!(error.code, "invalid_number");
    assert_eq!(error.current_revision, "resident/0");

    let oversized = process_wire_request(&mut runtime, &vec![b' '; 65_537]);
    let DesignerWireReply::Error { error } =
        serde_json::from_slice(&oversized).expect("oversized failure should decode")
    else {
        panic!("expected structured oversized-request failure reply");
    };
    assert_eq!(error.code, "request_too_large");
    assert_eq!(error.current_revision, "resident/0");

    let mut bounded_runtime = moonfall();
    let too_many_fields = bounded_runtime
        .handle(DesignerRequest::QueryFields {
            expected_revision: "resident/0".to_owned(),
            fields: vec!["iron_sword.damage".into(); 1_025],
        })
        .expect_err("oversized selective field query must fail");
    assert_eq!(
        too_many_fields.failure_projection("resident/0").code,
        "query_too_large"
    );
}

#[test]
fn canonical_open_is_candidate_first_and_starts_a_fresh_revision_domain() {
    let initial = moonfall();
    let original_scope = initial.occurrence_scope().to_owned();
    let bundle = initial
        .export_project("resident/0")
        .expect("initial snapshot should encode")
        .bytes;
    let mut runtime = Some(initial);
    runtime
        .as_mut()
        .unwrap()
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/0".to_owned(),
            target: "iron_sword.damage".into(),
            input: ScalarEditInput::Number {
                input: "45".to_owned(),
            },
        })
        .expect("current occurrence should advance");

    let mut corrupt = bundle.clone();
    *corrupt.last_mut().expect("bundle is not empty") ^= 1;
    let mut unsupported = bundle.clone();
    replace_ascii(
        &mut unsupported,
        b"\"format_version\": 1",
        b"\"format_version\": 2",
    );
    let mut noncanonical = bundle.clone();
    replace_ascii(
        &mut noncanonical,
        b"\"format_version\": 1",
        b"\"format_version\":1 ",
    );
    for (label, rejected) in [
        ("corrupt transfer", corrupt),
        ("unsupported version", unsupported),
        ("noncanonical bytes", noncanonical),
    ] {
        let Err(failure) = open_project(&mut runtime, &rejected, OCCURRENCE_ONE) else {
            panic!("{label} open must fail");
        };
        assert_eq!(
            failure.failure_projection("resident/1").code,
            "invalid_project",
            "{label}"
        );
        assert_eq!(runtime.as_ref().unwrap().occurrence_scope(), original_scope);
    }
    let DesignerResponse::Fields(current) = runtime
        .as_mut()
        .unwrap()
        .handle(DesignerRequest::QueryFields {
            expected_revision: "resident/1".to_owned(),
            fields: vec!["iron_sword.damage".into()],
        })
        .expect("failed open must preserve the current occurrence")
    else {
        panic!("expected fields response");
    };
    assert_eq!(
        current.fields[0]
            .stored
            .as_ref()
            .and_then(StoredValueProjection::number),
        Some(45.0)
    );

    let reopened =
        open_project(&mut runtime, &bundle, OCCURRENCE_ONE).expect("canonical open should succeed");
    assert_eq!(reopened.bootstrap.revision, "resident/0");
    assert_eq!(reopened.table.revision, "resident/0");
    assert_ne!(runtime.as_ref().unwrap().occurrence_scope(), original_scope);
    let DesignerResponse::Fields(reopened_fields) = runtime
        .as_mut()
        .unwrap()
        .handle(DesignerRequest::QueryFields {
            expected_revision: "resident/0".to_owned(),
            fields: vec!["iron_sword.damage".into(), "iron_sword.dps".into()],
        })
        .expect("fresh occurrence should be queryable")
    else {
        panic!("expected fields response");
    };
    assert_eq!(
        reopened_fields.fields[0]
            .stored
            .as_ref()
            .and_then(StoredValueProjection::number),
        Some(36.0)
    );
    assert_eq!(
        reopened_fields.fields[1]
            .calculated
            .as_ref()
            .and_then(CalculationProjection::number),
        Some(40.0)
    );
}

fn replace_ascii(bytes: &mut [u8], from: &[u8], to: &[u8]) {
    assert_eq!(from.len(), to.len());
    let offset = bytes
        .windows(from.len())
        .position(|candidate| candidate == from)
        .expect("expected canonical ASCII fragment");
    bytes[offset..offset + from.len()].copy_from_slice(to);
}

#[test]
fn exact_revision_export_does_not_capture_or_mark_a_later_edit() {
    let mut runtime = moonfall();
    let revision_zero = runtime
        .export_project("resident/0")
        .expect("revision zero should export");
    runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/0".to_owned(),
            target: "iron_sword.damage".into(),
            input: ScalarEditInput::Number {
                input: "45".to_owned(),
            },
        })
        .expect("later edit should publish");

    let stale = runtime
        .export_project("resident/0")
        .expect_err("a later snapshot must not be mislabeled as revision zero");
    assert_eq!(
        stale.failure_projection("resident/1").code,
        "stale_revision"
    );

    let mut reopened = None;
    let opened = open_project(&mut reopened, &revision_zero.bytes, OCCURRENCE_ONE)
        .expect("the already-exported revision remains a complete candidate");
    assert_eq!(opened.bootstrap.revision, "resident/0");
    let DesignerResponse::Fields(fields) = reopened
        .as_mut()
        .unwrap()
        .handle(DesignerRequest::QueryFields {
            expected_revision: "resident/0".to_owned(),
            fields: vec!["iron_sword.damage".into(), "iron_sword.dps".into()],
        })
        .expect("saved candidate should preserve its meaning")
    else {
        panic!("expected fields response");
    };
    assert_eq!(
        fields.fields[0]
            .stored
            .as_ref()
            .and_then(StoredValueProjection::number),
        Some(36.0)
    );
    assert_eq!(
        fields.fields[1]
            .calculated
            .as_ref()
            .and_then(CalculationProjection::number),
        Some(40.0)
    );
}

#[test]
fn close_destroys_the_occurrence_and_saved_output_reopens_without_git() {
    let runtime = moonfall();
    let original_scope = runtime.occurrence_scope().to_owned();
    let saved = runtime
        .export_project("resident/0")
        .expect("fixture should export");
    let mut occurrence = Some(runtime);

    close_project(&mut occurrence);
    assert!(occurrence.is_none());
    let closed_query = process_wire_request(
        &mut occurrence,
        br#"{"type":"query_table","collection":"weapons"}"#,
    );
    let DesignerWireReply::Error { error } =
        serde_json::from_slice(&closed_query).expect("closed failure should decode")
    else {
        panic!("closed occurrence must not silently recreate the demo");
    };
    assert_eq!(error.code, "no_project_open");
    assert!(occurrence.is_none());
    let reopened = open_project(&mut occurrence, &saved.bytes, OCCURRENCE_TWO)
        .expect("opaque durable bytes should open in a fresh occurrence");
    assert_eq!(reopened.bootstrap.revision, "resident/0");
    assert_ne!(
        occurrence.as_ref().unwrap().occurrence_scope(),
        original_scope
    );
}

#[test]
fn no_change_lifecycle_result_projects_without_publication_evidence() {
    let error = tachiko_designer_runtime::DesignerError::Lifecycle(
        tachiko_workspace_engine::patch_lifecycle::PatchLifecycleError::NoChange,
    );
    let failure = error.failure_projection("resident/0");
    assert_eq!(failure.code, "no_change");
    assert_eq!(failure.current_revision, "resident/0");
    assert!(failure.diagnostics.is_empty());
}
