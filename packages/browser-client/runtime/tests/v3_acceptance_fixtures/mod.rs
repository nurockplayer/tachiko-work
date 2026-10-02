//! Existing-API acceptance fixtures shared with the bounded API compile check.
use std::collections::BTreeMap;

use tachiko_designer_runtime::{
    DesignerError, DesignerRuntime, OpenedProjection, inspect_project, open_project,
};
use tachiko_storage::{CanonicalRoProjectV3, encode_roproj_v3};
use tachiko_workspace_engine::{
    Date, Document, Entity, EntityId, EntityKey, FieldConstraint, FieldDefinition, FieldId,
    FieldKey, FieldType, KeyedGroupedSumDefinition, KeyedGroupedSumDefinitionId,
    KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding, Number, Schema, SchemaId,
    SchemaKey, Value,
};

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
pub fn constrained_v3_document(field_index: usize) -> Document {
    assert!(field_index == 1 || field_index == 2);
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
        fields: FIELD_IDS
            .iter()
            .zip(definitions)
            .enumerate()
            .map(|(index, (raw_id, field_type))| {
                let id = FieldId::from(*raw_id);
                let constraint = match index {
                    1 if field_index == 1 => FieldConstraint::TextLiteralSet {
                        values: vec!["Paper".to_owned(), "Stationery".to_owned()],
                    },
                    2 if field_index == 2 => FieldConstraint::NumberInclusiveRange {
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

pub fn constrained_v3_transfer(field_index: usize) -> Vec<u8> {
    frame_v3(&encode_roproj_v3(&constrained_v3_document(field_index)).unwrap())
}

pub fn frame_v3(tree: &CanonicalRoProjectV3) -> Vec<u8> {
    let mut bytes = b"TWDPROJ1".to_vec();
    bytes.extend_from_slice(&u32::try_from(tree.files().len()).unwrap().to_le_bytes());
    for file in tree.files() {
        let path = file.path().as_bytes();
        bytes.extend_from_slice(&u16::try_from(path.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&u32::try_from(file.bytes().len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(path);
        bytes.extend_from_slice(file.bytes());
    }
    bytes
}

pub fn inspect_existing_api(bytes: &[u8]) -> Result<OpenedProjection, DesignerError> {
    inspect_project(bytes)
}

pub fn open_existing_api(
    slot: &mut Option<DesignerRuntime>,
    bytes: &[u8],
    occurrence_id: &str,
) -> Result<OpenedProjection, DesignerError> {
    open_project(slot, bytes, occurrence_id)
}
