//! Storage-owned `.roproj/v1` DTOs and canonical tree writer.

use sha2::{Digest, Sha256};
use tachiko_semantic_core::{
    Document, Entity, Expression, FieldDefinition, FieldRef, FieldType, Number, Schema, Value,
};

use crate::FormatError;

pub const ROPROJ_V1_FORMAT_VERSION: u32 = 1;
pub const ROPROJ_V1_PATHS: [&str; 18] = [
    "manifest.json",
    "schemas.json",
    "entities/0.jsonl",
    "entities/1.jsonl",
    "entities/2.jsonl",
    "entities/3.jsonl",
    "entities/4.jsonl",
    "entities/5.jsonl",
    "entities/6.jsonl",
    "entities/7.jsonl",
    "entities/8.jsonl",
    "entities/9.jsonl",
    "entities/a.jsonl",
    "entities/b.jsonl",
    "entities/c.jsonl",
    "entities/d.jsonl",
    "entities/e.jsonl",
    "entities/f.jsonl",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRoProjectFile {
    path: String,
    bytes: Vec<u8>,
}

impl CanonicalRoProjectFile {
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRoProjectV1 {
    files: Vec<CanonicalRoProjectFile>,
}

impl CanonicalRoProjectV1 {
    #[must_use]
    pub fn files(&self) -> &[CanonicalRoProjectFile] {
        &self.files
    }

    #[must_use]
    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.files
            .iter()
            .find(|file| file.path == path)
            .map(CanonicalRoProjectFile::bytes)
    }
}

struct ManifestV1 {
    document: DocumentIdentityV1,
}

struct DocumentIdentityV1 {
    id: String,
    title: String,
}

struct SchemaV1 {
    id: String,
    key: String,
    fields: Vec<FieldDefinitionV1>,
}

struct FieldDefinitionV1 {
    id: String,
    key: String,
    field_type: FieldTypeV1,
    required: bool,
}

enum FieldTypeV1 {
    Number,
    Text,
    Boolean,
    Reference { schema: String },
}

struct EntityV1 {
    id: String,
    key: String,
    schema: String,
    fields: Vec<(String, ValueV1)>,
}

enum ValueV1 {
    Number(Number),
    Text(String),
    Boolean(bool),
    Reference(String),
    Formula(ExpressionV1),
}

enum ExpressionV1 {
    Number(Number),
    Reference(FieldRefV1),
    Add(BinaryArgumentsV1),
    Subtract(BinaryArgumentsV1),
    Multiply(BinaryArgumentsV1),
    Divide(BinaryArgumentsV1),
    Minimum(BinaryArgumentsV1),
    Maximum(BinaryArgumentsV1),
}

struct FieldRefV1 {
    entity: String,
    field: String,
}

struct BinaryArgumentsV1 {
    left: Box<ExpressionV1>,
    right: Box<ExpressionV1>,
}

/// Encode a valid semantic document into the exact canonical `.roproj/v1` tree.
///
/// # Errors
///
/// Returns [`FormatError::InvalidDocument`] when semantic validation fails or
/// [`FormatError::Json`] when canonical JSON string encoding fails.
pub fn encode(document: &Document) -> Result<CanonicalRoProjectV1, FormatError> {
    super::super::check_document(document)?;

    let manifest = ManifestV1::from_semantic(document);
    let mut schemas = document
        .schemas
        .values()
        .map(SchemaV1::from_semantic)
        .collect::<Vec<_>>();
    schemas.sort_by(|left, right| left.id.as_bytes().cmp(right.id.as_bytes()));
    let mut entities = document
        .entities
        .values()
        .map(EntityV1::from_semantic)
        .collect::<Vec<_>>();
    entities.sort_by(|left, right| left.id.as_bytes().cmp(right.id.as_bytes()));

    let mut files = Vec::with_capacity(ROPROJ_V1_PATHS.len());
    files.push(CanonicalRoProjectFile {
        path: ROPROJ_V1_PATHS[0].to_owned(),
        bytes: render_manifest(&manifest)?.into_bytes(),
    });
    files.push(CanonicalRoProjectFile {
        path: ROPROJ_V1_PATHS[1].to_owned(),
        bytes: render_schemas(&schemas)?.into_bytes(),
    });

    let mut shards = std::array::from_fn::<String, 16, _>(|_| String::new());
    for entity in entities {
        let shard = shard_index(&entity.id);
        write_entity(&mut shards[shard], &entity)?;
        shards[shard].push('\n');
    }
    for (index, path) in ROPROJ_V1_PATHS.iter().enumerate().skip(2) {
        files.push(CanonicalRoProjectFile {
            path: (*path).to_owned(),
            bytes: std::mem::take(&mut shards[index - 2]).into_bytes(),
        });
    }

    Ok(CanonicalRoProjectV1 { files })
}

impl ManifestV1 {
    fn from_semantic(document: &Document) -> Self {
        Self {
            document: DocumentIdentityV1 {
                id: document.id.to_string(),
                title: document.title.clone(),
            },
        }
    }
}

impl SchemaV1 {
    fn from_semantic(schema: &Schema) -> Self {
        let mut fields = schema
            .fields
            .values()
            .map(FieldDefinitionV1::from_semantic)
            .collect::<Vec<_>>();
        fields.sort_by(|left, right| left.id.as_bytes().cmp(right.id.as_bytes()));
        Self {
            id: schema.id.to_string(),
            key: schema.key.to_string(),
            fields,
        }
    }
}

impl FieldDefinitionV1 {
    fn from_semantic(field: &FieldDefinition) -> Self {
        Self {
            id: field.id.to_string(),
            key: field.key.to_string(),
            field_type: FieldTypeV1::from_semantic(&field.field_type),
            required: field.required,
        }
    }
}

impl FieldTypeV1 {
    fn from_semantic(field_type: &FieldType) -> Self {
        match field_type {
            FieldType::Number => Self::Number,
            FieldType::Text => Self::Text,
            FieldType::Boolean => Self::Boolean,
            FieldType::Reference { schema } => Self::Reference {
                schema: schema.to_string(),
            },
        }
    }
}

impl EntityV1 {
    fn from_semantic(entity: &Entity) -> Self {
        let mut fields = entity
            .fields
            .iter()
            .map(|(id, value)| (id.to_string(), ValueV1::from_semantic(value)))
            .collect::<Vec<_>>();
        fields.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
        Self {
            id: entity.id.to_string(),
            key: entity.key.to_string(),
            schema: entity.schema.to_string(),
            fields,
        }
    }
}

impl ValueV1 {
    fn from_semantic(value: &Value) -> Self {
        match value {
            Value::Number(number) => Self::Number(*number),
            Value::Text(text) => Self::Text(text.clone()),
            Value::Boolean(boolean) => Self::Boolean(*boolean),
            Value::Reference(entity) => Self::Reference(entity.to_string()),
            Value::Formula(expression) => Self::Formula(ExpressionV1::from_semantic(expression)),
        }
    }
}

impl ExpressionV1 {
    fn from_semantic(expression: &Expression) -> Self {
        match expression {
            Expression::Number(number) => Self::Number(*number),
            Expression::Reference(reference) => {
                Self::Reference(FieldRefV1::from_semantic(reference))
            }
            Expression::Add { left, right } => {
                Self::Add(BinaryArgumentsV1::from_semantic(left, right))
            }
            Expression::Subtract { left, right } => {
                Self::Subtract(BinaryArgumentsV1::from_semantic(left, right))
            }
            Expression::Multiply { left, right } => {
                Self::Multiply(BinaryArgumentsV1::from_semantic(left, right))
            }
            Expression::Divide { left, right } => {
                Self::Divide(BinaryArgumentsV1::from_semantic(left, right))
            }
            Expression::Minimum { left, right } => {
                Self::Minimum(BinaryArgumentsV1::from_semantic(left, right))
            }
            Expression::Maximum { left, right } => {
                Self::Maximum(BinaryArgumentsV1::from_semantic(left, right))
            }
        }
    }
}

impl FieldRefV1 {
    fn from_semantic(reference: &FieldRef) -> Self {
        Self {
            entity: reference.entity.to_string(),
            field: reference.field.to_string(),
        }
    }
}

impl BinaryArgumentsV1 {
    fn from_semantic(left: &Expression, right: &Expression) -> Self {
        Self {
            left: Box::new(ExpressionV1::from_semantic(left)),
            right: Box::new(ExpressionV1::from_semantic(right)),
        }
    }
}

fn render_manifest(manifest: &ManifestV1) -> Result<String, FormatError> {
    let mut output = String::new();
    output.push_str("{\n");
    pretty_member_string(&mut output, 1, "format", "tachiko.roproj", true)?;
    pretty_member_literal(
        &mut output,
        1,
        "format_version",
        &ROPROJ_V1_FORMAT_VERSION.to_string(),
        true,
    )?;
    pretty_member_prefix(&mut output, 1, "document")?;
    output.push_str("{\n");
    pretty_member_string(&mut output, 2, "id", &manifest.document.id, true)?;
    pretty_member_string(&mut output, 2, "title", &manifest.document.title, false)?;
    push_indent(&mut output, 1);
    output.push_str("}\n}\n");
    Ok(output)
}

fn render_schemas(schemas: &[SchemaV1]) -> Result<String, FormatError> {
    let mut output = String::new();
    if schemas.is_empty() {
        output.push_str("[]\n");
        return Ok(output);
    }
    output.push_str("[\n");
    for (index, schema) in schemas.iter().enumerate() {
        write_schema(&mut output, schema, 1)?;
        if index + 1 != schemas.len() {
            output.push(',');
        }
        output.push('\n');
    }
    output.push_str("]\n");
    Ok(output)
}

fn write_schema(output: &mut String, schema: &SchemaV1, indent: usize) -> Result<(), FormatError> {
    push_indent(output, indent);
    output.push_str("{\n");
    pretty_member_string(output, indent + 1, "id", &schema.id, true)?;
    pretty_member_string(output, indent + 1, "key", &schema.key, true)?;
    pretty_member_prefix(output, indent + 1, "fields")?;
    if schema.fields.is_empty() {
        output.push_str("[]\n");
    } else {
        output.push_str("[\n");
        for (index, field) in schema.fields.iter().enumerate() {
            write_field_definition(output, field, indent + 2)?;
            if index + 1 != schema.fields.len() {
                output.push(',');
            }
            output.push('\n');
        }
        push_indent(output, indent + 1);
        output.push_str("]\n");
    }
    push_indent(output, indent);
    output.push('}');
    Ok(())
}

fn write_field_definition(
    output: &mut String,
    field: &FieldDefinitionV1,
    indent: usize,
) -> Result<(), FormatError> {
    push_indent(output, indent);
    output.push_str("{\n");
    pretty_member_string(output, indent + 1, "id", &field.id, true)?;
    pretty_member_string(output, indent + 1, "key", &field.key, true)?;
    pretty_member_prefix(output, indent + 1, "field_type")?;
    write_field_type(output, &field.field_type, indent + 1)?;
    output.push_str(",\n");
    pretty_member_literal(
        output,
        indent + 1,
        "required",
        if field.required { "true" } else { "false" },
        false,
    )?;
    push_indent(output, indent);
    output.push('}');
    Ok(())
}

fn write_field_type(
    output: &mut String,
    field_type: &FieldTypeV1,
    indent: usize,
) -> Result<(), FormatError> {
    output.push_str("{\n");
    match field_type {
        FieldTypeV1::Number => pretty_member_string(output, indent + 1, "type", "number", false)?,
        FieldTypeV1::Text => pretty_member_string(output, indent + 1, "type", "text", false)?,
        FieldTypeV1::Boolean => pretty_member_string(output, indent + 1, "type", "boolean", false)?,
        FieldTypeV1::Reference { schema } => {
            pretty_member_string(output, indent + 1, "type", "reference", true)?;
            pretty_member_string(output, indent + 1, "schema", schema, false)?;
        }
    }
    push_indent(output, indent);
    output.push('}');
    Ok(())
}

fn write_entity(output: &mut String, entity: &EntityV1) -> Result<(), FormatError> {
    output.push('{');
    compact_member_string(output, "id", &entity.id, true)?;
    compact_member_string(output, "key", &entity.key, true)?;
    compact_member_string(output, "schema", &entity.schema, true)?;
    compact_member_prefix(output, "fields")?;
    output.push('{');
    for (index, (id, value)) in entity.fields.iter().enumerate() {
        write_json_string(output, id)?;
        output.push(':');
        write_value(output, value)?;
        if index + 1 != entity.fields.len() {
            output.push(',');
        }
    }
    output.push_str("}}");
    Ok(())
}

fn write_value(output: &mut String, value: &ValueV1) -> Result<(), FormatError> {
    output.push('{');
    match value {
        ValueV1::Number(number) => {
            compact_member_string(output, "kind", "number", true)?;
            compact_member_number(output, "value", *number, false)?;
        }
        ValueV1::Text(text) => {
            compact_member_string(output, "kind", "text", true)?;
            compact_member_string(output, "value", text, false)?;
        }
        ValueV1::Boolean(boolean) => {
            compact_member_string(output, "kind", "boolean", true)?;
            compact_member_literal(
                output,
                "value",
                if *boolean { "true" } else { "false" },
                false,
            )?;
        }
        ValueV1::Reference(entity) => {
            compact_member_string(output, "kind", "reference", true)?;
            compact_member_string(output, "value", entity, false)?;
        }
        ValueV1::Formula(expression) => {
            compact_member_string(output, "kind", "formula", true)?;
            compact_member_prefix(output, "value")?;
            write_expression(output, expression)?;
        }
    }
    output.push('}');
    Ok(())
}

fn write_expression(output: &mut String, expression: &ExpressionV1) -> Result<(), FormatError> {
    output.push('{');
    match expression {
        ExpressionV1::Number(number) => {
            compact_member_string(output, "op", "number", true)?;
            compact_member_number(output, "args", *number, false)?;
        }
        ExpressionV1::Reference(reference) => {
            compact_member_string(output, "op", "reference", true)?;
            compact_member_prefix(output, "args")?;
            output.push('{');
            compact_member_string(output, "entity", &reference.entity, true)?;
            compact_member_string(output, "field", &reference.field, false)?;
            output.push('}');
        }
        ExpressionV1::Add(args) => write_binary_expression(output, "add", args)?,
        ExpressionV1::Subtract(args) => write_binary_expression(output, "subtract", args)?,
        ExpressionV1::Multiply(args) => write_binary_expression(output, "multiply", args)?,
        ExpressionV1::Divide(args) => write_binary_expression(output, "divide", args)?,
        ExpressionV1::Minimum(args) => write_binary_expression(output, "minimum", args)?,
        ExpressionV1::Maximum(args) => write_binary_expression(output, "maximum", args)?,
    }
    output.push('}');
    Ok(())
}

fn write_binary_expression(
    output: &mut String,
    operator: &str,
    arguments: &BinaryArgumentsV1,
) -> Result<(), FormatError> {
    compact_member_string(output, "op", operator, true)?;
    compact_member_prefix(output, "args")?;
    output.push('{');
    compact_member_prefix(output, "left")?;
    write_expression(output, &arguments.left)?;
    output.push(',');
    compact_member_prefix(output, "right")?;
    write_expression(output, &arguments.right)?;
    output.push('}');
    Ok(())
}

fn pretty_member_prefix(output: &mut String, indent: usize, name: &str) -> Result<(), FormatError> {
    push_indent(output, indent);
    write_json_string(output, name)?;
    output.push_str(": ");
    Ok(())
}

fn pretty_member_string(
    output: &mut String,
    indent: usize,
    name: &str,
    value: &str,
    comma: bool,
) -> Result<(), FormatError> {
    pretty_member_prefix(output, indent, name)?;
    write_json_string(output, value)?;
    if comma {
        output.push(',');
    }
    output.push('\n');
    Ok(())
}

fn pretty_member_literal(
    output: &mut String,
    indent: usize,
    name: &str,
    value: &str,
    comma: bool,
) -> Result<(), FormatError> {
    pretty_member_prefix(output, indent, name)?;
    output.push_str(value);
    if comma {
        output.push(',');
    }
    output.push('\n');
    Ok(())
}

fn compact_member_prefix(output: &mut String, name: &str) -> Result<(), FormatError> {
    write_json_string(output, name)?;
    output.push(':');
    Ok(())
}

fn compact_member_string(
    output: &mut String,
    name: &str,
    value: &str,
    comma: bool,
) -> Result<(), FormatError> {
    compact_member_prefix(output, name)?;
    write_json_string(output, value)?;
    if comma {
        output.push(',');
    }
    Ok(())
}

fn compact_member_number(
    output: &mut String,
    name: &str,
    value: Number,
    comma: bool,
) -> Result<(), FormatError> {
    let mut buffer = ryu_js::Buffer::new();
    compact_member_prefix(output, name)?;
    output.push_str(buffer.format_finite(value.get()));
    if comma {
        output.push(',');
    }
    Ok(())
}

fn compact_member_literal(
    output: &mut String,
    name: &str,
    value: &str,
    comma: bool,
) -> Result<(), FormatError> {
    compact_member_prefix(output, name)?;
    output.push_str(value);
    if comma {
        output.push(',');
    }
    Ok(())
}

fn write_json_string(output: &mut String, value: &str) -> Result<(), FormatError> {
    output.push_str(&serde_json::to_string(value)?);
    Ok(())
}

fn push_indent(output: &mut String, indent: usize) {
    for _ in 0..indent {
        output.push_str("  ");
    }
}

fn shard_index(entity_id: &str) -> usize {
    usize::from(Sha256::digest(entity_id.as_bytes())[0] >> 4)
}
