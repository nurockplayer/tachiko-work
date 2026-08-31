//! Deterministic Product Gap document shared by Designer development evidence.
//!
//! This module is included directly by the fixture test and generator example.
//! It is deliberately not part of the Designer runtime library or semantic core.

use std::collections::BTreeMap;

use tachiko_workspace_engine::{
    Document, DocumentId, Entity, EntityId, EntityKey, Expression, FieldDefinition, FieldId,
    FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

const DOCUMENT_ID: &str = "ba30fc0a-5b11-4dbf-9ef3-76f904315a4d";
const SCHEMA_ID: &str = "d8b3db6e-a2ca-48f1-82f5-4e44630418dc";

const TITLE_FIELD_ID: &str = "197ae0df-85bd-4c9a-8a9d-0ea27a599881";
const AREA_FIELD_ID: &str = "3f49496c-1764-438b-a106-5563246ad8d3";
const IMPACT_FIELD_ID: &str = "52a0f7e9-4d6b-4628-ad7d-6b649f8c8a77";
const FRICTION_FIELD_ID: &str = "6b529d6d-f003-42ff-aa22-5e317a3e5f37";
const PRIORITY_FIELD_ID: &str = "75f3d634-452c-4838-925b-8c52e54fc972";
const CONFIRMED_FIELD_ID: &str = "8e1647d7-1a4c-48f7-bbf0-14527730619f";
const GITHUB_ISSUE_FIELD_ID: &str = "9c3c9df1-ac56-4211-9d9d-846ce8c4afbd";

const DESIGNER_PROFILE_BOUND_ID: &str = "1d37df46-01f6-4b05-8fd9-064718dc91ea";
const SCHEMA_AUTHORING_MISSING_ID: &str = "2a80fa52-df19-4e75-992c-ed92b7194970";
const BROWSER_SAVE_AS_ONLY_ID: &str = "3c7d2a33-66b4-4f02-b590-8c6f2d9ecdf1";

const ISSUE_URL: &str = "https://github.com/nurockplayer/tachiko-work/issues/219";

/// Build the fixed semantic document behind the checked-in Product Gap project.
#[must_use]
pub fn document() -> Document {
    let schema_id = SchemaId::from(SCHEMA_ID);
    Document {
        id: DocumentId::from(DOCUMENT_ID),
        title: "Tachiko Work Product Gaps".to_owned(),
        schemas: BTreeMap::from([(
            schema_id.clone(),
            Schema {
                id: schema_id.clone(),
                key: SchemaKey::from("product_gaps"),
                fields: fields(),
            },
        )]),
        entities: BTreeMap::from([
            gap(
                DESIGNER_PROFILE_BOUND_ID,
                "designer_profile_bound",
                &schema_id,
                "Designer admission was bound to Moonfall",
                "Designer",
                5.0,
                5.0,
            ),
            gap(
                SCHEMA_AUTHORING_MISSING_ID,
                "schema_authoring_missing",
                &schema_id,
                "Schema and field authoring is not exposed",
                "Authoring",
                5.0,
                4.0,
            ),
            gap(
                BROWSER_SAVE_AS_ONLY_ID,
                "browser_save_as_only",
                &schema_id,
                "Browser persistence is create-only Save As",
                "Persistence",
                4.0,
                4.0,
            ),
        ]),
    }
}

fn fields() -> BTreeMap<FieldId, FieldDefinition> {
    [
        field(TITLE_FIELD_ID, "title", FieldType::Text),
        field(AREA_FIELD_ID, "area", FieldType::Text),
        field(IMPACT_FIELD_ID, "impact", FieldType::Number),
        field(FRICTION_FIELD_ID, "friction", FieldType::Number),
        field(PRIORITY_FIELD_ID, "priority", FieldType::Number),
        field(CONFIRMED_FIELD_ID, "confirmed", FieldType::Boolean),
        field(GITHUB_ISSUE_FIELD_ID, "github_issue", FieldType::Text),
    ]
    .into_iter()
    .map(|definition| (definition.id.clone(), definition))
    .collect()
}

fn field(id: &str, key: &str, field_type: FieldType) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(key),
        field_type,
        required: true,
    }
}

#[allow(clippy::too_many_arguments)]
fn gap(
    id: &str,
    key: &str,
    schema: &SchemaId,
    title: &str,
    area: &str,
    impact: f64,
    friction: f64,
) -> (EntityId, Entity) {
    let entity_id = EntityId::from(id);
    (
        entity_id.clone(),
        Entity {
            id: entity_id.clone(),
            key: EntityKey::from(key),
            schema: schema.clone(),
            fields: BTreeMap::from([
                (FieldId::from(TITLE_FIELD_ID), Value::Text(title.to_owned())),
                (FieldId::from(AREA_FIELD_ID), Value::Text(area.to_owned())),
                (
                    FieldId::from(IMPACT_FIELD_ID),
                    Value::Number(Number::new(impact).expect("fixture score is finite")),
                ),
                (
                    FieldId::from(FRICTION_FIELD_ID),
                    Value::Number(Number::new(friction).expect("fixture score is finite")),
                ),
                (
                    FieldId::from(PRIORITY_FIELD_ID),
                    Value::Formula(Expression::Add {
                        left: Box::new(Expression::Reference(FieldRef::new(
                            entity_id.clone(),
                            IMPACT_FIELD_ID,
                        ))),
                        right: Box::new(Expression::Reference(FieldRef::new(
                            entity_id,
                            FRICTION_FIELD_ID,
                        ))),
                    }),
                ),
                (FieldId::from(CONFIRMED_FIELD_ID), Value::Boolean(true)),
                (
                    FieldId::from(GITHUB_ISSUE_FIELD_ID),
                    Value::Text(ISSUE_URL.to_owned()),
                ),
            ]),
        },
    )
}
