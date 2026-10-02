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

/// Put the sole non-None constraint outside the default table and all saved
/// definition bindings. The empty extra schema is valid storage, not a resource
/// or missing-reference refusal vector.
pub fn nondefault_constrained_v3_document(field_index: usize) -> Document {
    let mut document = constrained_v3_document(field_index);
    let source = document
        .schemas
        .get_mut(&SchemaId::from(SCHEMA_ID))
        .unwrap()
        .fields
        .get_mut(&FieldId::from(FIELD_IDS[field_index]))
        .unwrap();
    let constraint = std::mem::replace(&mut source.constraint, FieldConstraint::None);
    let field_type = source.field_type.clone();
    let id = SchemaId::from("zz_acceptance_unbound_schema");
    let field_id = FieldId::from("zz_acceptance_unbound_field");
    document.schemas.insert(
        id.clone(),
        Schema {
            id,
            key: SchemaKey::from("zz_unused"),
            fields: BTreeMap::from([(
                field_id.clone(),
                FieldDefinition {
                    id: field_id,
                    key: FieldKey::from("unused"),
                    field_type,
                    required: false,
                    constraint,
                },
            )]),
        },
    );
    document
}

pub fn nondefault_constrained_v3_transfer(field_index: usize) -> Vec<u8> {
    frame_v3(&encode_roproj_v3(&nondefault_constrained_v3_document(field_index)).unwrap())
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

// These existing-API adapters are used only by the fixture validation target.
#[allow(dead_code)]
pub fn inspect_existing_api(bytes: &[u8]) -> Result<OpenedProjection, DesignerError> {
    inspect_project(bytes)
}

#[allow(dead_code)] // The selected target uses the public adapter directly.
pub fn open_existing_api(
    slot: &mut Option<DesignerRuntime>,
    bytes: &[u8],
    occurrence_id: &str,
) -> Result<OpenedProjection, DesignerError> {
    open_project(slot, bytes, occurrence_id)
}

/// Mutate exactly one canonical definition member per vector. These literal
/// anchors are shared with the existing-API fixture validation target.
pub fn malformed_definition_payloads(original: &str) -> Vec<(&'static str, Vec<u8>)> {
    let replace_once = |source: &str, from: &str, to: &str| {
        assert_eq!(
            source.matches(from).count(),
            1,
            "mutator must target one canonical member"
        );
        source.replacen(from, to, 1).into_bytes()
    };
    let mut cases = vec![
        (
            "empty definition id",
            replace_once(
                original,
                r#""id": "acceptance-orders-summary""#,
                r#""id": """#,
            ),
        ),
        (
            "omitted definition id",
            replace_once(original, "    \"id\": \"acceptance-orders-summary\",\n", ""),
        ),
        (
            "missing schema reference",
            replace_once(
                original,
                r#"    "orders": {
      "schema": "import_00000000-0000-4000-8000-000000000001_0002""#,
                r#"    "orders": {
      "schema": "missing-schema""#,
            ),
        ),
        (
            "missing field reference",
            replace_once(
                original,
                &format!("\"lookup_key_field\": \"{}\"", FIELD_IDS[0]),
                r#""lookup_key_field": "missing-field""#,
            ),
        ),
        (
            "wrong bound field type",
            replace_once(
                original,
                &format!("\"quantity_field\": \"{}\"", FIELD_IDS[2]),
                &format!("\"quantity_field\": \"{}\"", FIELD_IDS[4]),
            ),
        ),
    ];
    let lookup_with_comma = format!("      \"lookup_key_field\": \"{}\",\n", FIELD_IDS[0]);
    let lookup_without_comma = lookup_with_comma.replace(",\n", "\n");
    let quantity_line = format!("      \"quantity_field\": \"{}\"\n", FIELD_IDS[2]);
    let without_comma = String::from_utf8(replace_once(
        original,
        &lookup_with_comma,
        &lookup_without_comma,
    ))
    .unwrap();
    let missing_binding = replace_once(&without_comma, &quantity_line, "");
    cases.push(("missing binding member", missing_binding));
    let record = original
        .strip_prefix("[\n")
        .unwrap()
        .strip_suffix("]\n")
        .unwrap()
        .trim_end();
    cases.push((
        "duplicate definition record",
        format!("[\n{record},\n{record}\n]\n").into_bytes(),
    ));
    cases.push((
        "unknown catalogue item shape",
        b"[\n  \"not-a-definition-record\"\n]\n".to_vec(),
    ));
    cases
}

/// Inputs are explicit so context checks never mutate process environment.
#[derive(Clone, Copy, Default)]
pub struct CaptureInputs<'a> {
    pub required: Option<&'a str>,
    pub directory: Option<&'a str>,
    pub head: Option<&'a str>,
    pub run_id: Option<&'a str>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CaptureContext<'a> {
    pub directory: &'a str,
    pub head: &'a str,
    pub run_id: &'a str,
}

pub fn resolve_capture_context<'a>(
    input: CaptureInputs<'a>,
    compiled_head: Option<&str>,
    compiled_run_id: Option<&str>,
) -> Result<Option<CaptureContext<'a>>, &'static str> {
    if input.required.is_some_and(|value| value != "1") {
        return Err("CAPTURE_REQUIRED must be exactly 1 when supplied");
    }
    if input.required.is_none()
        && input.directory.is_none()
        && input.head.is_none()
        && input.run_id.is_none()
        && compiled_head.is_none()
        && compiled_run_id.is_none()
    {
        return Ok(None);
    }
    let (Some(directory), Some(head), Some(run_id)) = (input.directory, input.head, input.run_id)
    else {
        return Err("capture requires complete runtime directory/head/run context");
    };
    if directory.is_empty()
        || !std::path::Path::new(directory).is_absolute()
        || directory.contains('\0')
    {
        return Err("capture directory must be a nonempty absolute path");
    }
    if head.len() != 40
        || !head
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("capture head must be lowercase 40-hex");
    }
    if run_id.len() != 36
        || !run_id.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
    {
        return Err("capture run must be a canonical lowercase UUID");
    }
    match (compiled_head, compiled_run_id) {
        (Some(compiled_head), Some(compiled_run_id))
            if compiled_head == head && compiled_run_id == run_id => {}
        _ => return Err("capture runtime context must match both compiled identities"),
    }
    Ok(Some(CaptureContext {
        directory,
        head,
        run_id,
    }))
}
