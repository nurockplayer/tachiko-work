//! Fixed #519 input; shared by storage and runtime acceptance, never production.
use super::semantic::{
    Date, Document, Entity, EntityId, EntityKey, FieldConstraint, FieldDefinition, FieldId,
    FieldKey, FieldType, KeyedGroupedSumDefinition, KeyedGroupedSumDefinitionId,
    KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding, Number, Schema, SchemaId,
    SchemaKey, Value,
};
use std::collections::BTreeMap;

pub const DOCUMENT_ID: &str = "import_00000000-0000-4000-8000-000000000001_0001";
pub const SCHEMA_ID: &str = "import_00000000-0000-4000-8000-000000000001_0002";
pub const FIELD_IDS: [&str; 5] = [
    "import_00000000-0000-4000-8000-000000000001_0003",
    "import_00000000-0000-4000-8000-000000000001_0004",
    "import_00000000-0000-4000-8000-000000000001_0005",
    "import_00000000-0000-4000-8000-000000000001_0006",
    "import_00000000-0000-4000-8000-000000000001_0007",
];
pub const ROW_IDS: [&str; 2] = [
    "import_00000000-0000-4000-8000-000000000001_0008",
    "import_00000000-0000-4000-8000-000000000001_0009",
];
pub const DEFINITION_ID: &str = "acceptance-orders-summary";

/// Construct a semantically valid mixed Date/definition project with one
/// individually varied supported v3 field constraint.
pub fn document() -> Document {
    let schema_id = SchemaId::from(SCHEMA_ID);
    let definitions = [
        FieldType::Text,
        FieldType::Text,
        FieldType::Number,
        FieldType::Number,
        FieldType::Date,
    ];
    let schema = Schema {
        id: schema_id.clone(),
        key: SchemaKey::from("sheet_1"),
        display_name: None,
        fields: FIELD_IDS
            .iter()
            .zip(definitions)
            .enumerate()
            .map(|(index, (raw_id, field_type))| {
                let id = FieldId::from(*raw_id);
                let constraint = match index {
                    1 => FieldConstraint::TextLiteralSet {
                        values: vec!["Paper".to_owned(), "Stationery".to_owned()],
                    },
                    2 => FieldConstraint::NumberInclusiveRange {
                        min: Number::new(0.0).unwrap(),
                        max: Number::new(10.0).unwrap(),
                    },
                    _ => FieldConstraint::None,
                };
                (
                    id.clone(),
                    FieldDefinition {
                        id,
                        key: FieldKey::from(format!("column_{}", index + 1)),
                        display_name: None,
                        field_type,
                        required: false,
                        constraint,
                    },
                )
            })
            .collect(),
    };
    let mut document = Document::empty(DOCUMENT_ID, "Imported workbook");
    document.schemas.insert(schema_id.clone(), schema);
    for (index, (code, category, quantity, due)) in [
        ("PEN", "Stationery", 4.0, "2026-09-28"),
        ("NOTE", "Paper", 5.0, "2026-09-27"),
    ]
    .into_iter()
    .enumerate()
    {
        let id = EntityId::from(ROW_IDS[index]);
        let values = [
            Value::Text(code.to_owned()),
            Value::Text(category.to_owned()),
            Value::Number(Number::new(quantity).unwrap()),
            Value::Number(Number::new(200.0).unwrap()),
            Value::Date(Date::parse(due).unwrap()),
        ];
        document.entities.insert(
            id.clone(),
            Entity {
                id,
                key: EntityKey::from(format!("sheet_1_row_{}", index + 1)),
                schema: schema_id.clone(),
                fields: FIELD_IDS
                    .iter()
                    .zip(values)
                    .map(|(field, value)| (FieldId::from(*field), value))
                    .collect::<BTreeMap<_, _>>(),
            },
        );
    }
    document.keyed_grouped_sum_definitions.insert(
        KeyedGroupedSumDefinitionId::from(DEFINITION_ID),
        KeyedGroupedSumDefinition {
            id: DEFINITION_ID.into(),
            orders: KeyedGroupedSumOrdersBinding {
                schema: schema_id.clone(),
                lookup_key_field: FIELD_IDS[0].into(),
                quantity_field: FIELD_IDS[2].into(),
            },
            products: KeyedGroupedSumProductsBinding {
                schema: schema_id,
                key_field: FIELD_IDS[0].into(),
                category_field: FIELD_IDS[1].into(),
                price_field: FIELD_IDS[3].into(),
            },
        },
    );
    document
}
