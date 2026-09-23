//! Storage-owned `.roproj/v3` DTOs and canonical tree codec.

use std::{collections::BTreeMap, convert::Infallible};

use serde::{Deserialize, Deserializer, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, EntityKey, Expression, FieldConstraint,
    FieldDefinition, FieldId, FieldKey, FieldRef, FieldType, KeyedGroupedSumDefinition,
    KeyedGroupedSumDefinitionId, KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding,
    MAX_EXPRESSION_DEPTH, MAX_EXPRESSION_NODES, Number, Schema, SchemaId, SchemaKey, Value,
    validate_document, validate_keyed_grouped_sum_definitions,
};

use crate::{
    FormatError,
    strict_json::{FrontendError, VersionToken, inspect_roproj},
};

pub const ROPROJ_V3_FORMAT_VERSION: u32 = 3;
// A deepest valid entity is: entity -> fields -> value -> 64 expression
// objects, with one binary `args` object between each expression node and one
// reference `args` object at the leaf. No valid v3 DTO nests more deeply.
const ROPROJ_V3_MAX_JSON_NESTING: usize = 132;
pub const ROPROJ_V3_PATHS: [&str; 19] = [
    "manifest.json",
    "schemas.json",
    "definitions.json",
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
pub struct CanonicalRoProjectFileV3 {
    path: String,
    bytes: Vec<u8>,
}

impl CanonicalRoProjectFileV3 {
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
pub struct CanonicalRoProjectV3 {
    files: Vec<CanonicalRoProjectFileV3>,
}

#[derive(Debug)]
pub enum CanonicalRoProjectV3AdmissionError<E> {
    Format(FormatError),
    Profile(E),
}

impl CanonicalRoProjectV3 {
    #[must_use]
    pub fn files(&self) -> &[CanonicalRoProjectFileV3] {
        &self.files
    }

    #[must_use]
    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.files
            .iter()
            .find(|file| file.path == path)
            .map(CanonicalRoProjectFileV3::bytes)
    }

    /// Construct an exact canonical `.roproj/v3` tree from ordered path/byte pairs.
    ///
    /// # Errors
    ///
    /// Returns a `.roproj` format, version, JSON, semantic, or representation
    /// error unless the input is the exact canonical nineteen-file tree.
    pub fn try_from_files(files: Vec<(String, Vec<u8>)>) -> Result<Self, FormatError> {
        match Self::try_from_files_with_profile(files, |_| Ok::<(), Infallible>(())) {
            Ok((tree, _)) => Ok(tree),
            Err(CanonicalRoProjectV3AdmissionError::Format(error)) => Err(error),
            Err(CanonicalRoProjectV3AdmissionError::Profile(never)) => match never {},
        }
    }

    /// Admit an exact canonical tree while allowing a caller-owned resource
    /// profile to reject the decoded document before semantic validation.
    ///
    /// The callback must inspect resource shape only. Successful return still
    /// requires complete storage-owned semantic validation and byte-for-byte
    /// canonical re-encoding before the document is released.
    ///
    /// # Errors
    ///
    /// Returns a storage format error when the tree is invalid or noncanonical,
    /// or the caller's profile error when bounded admission rejects the decoded
    /// document before semantic validation.
    pub fn try_from_files_with_profile<E>(
        files: Vec<(String, Vec<u8>)>,
        profile: impl FnOnce(&Document) -> Result<(), E>,
    ) -> Result<(Self, Document), CanonicalRoProjectV3AdmissionError<E>> {
        if files.len() != ROPROJ_V3_PATHS.len() {
            return Err(CanonicalRoProjectV3AdmissionError::Format(
                FormatError::InvalidRoProjectRepresentation {
                    message: format!(
                        ".roproj/v3 requires exactly {} files",
                        ROPROJ_V3_PATHS.len()
                    ),
                },
            ));
        }
        let mut canonical_files = Vec::with_capacity(files.len());
        for (index, (path, bytes)) in files.into_iter().enumerate() {
            if path != ROPROJ_V3_PATHS[index] {
                return Err(CanonicalRoProjectV3AdmissionError::Format(
                    FormatError::InvalidRoProjectRepresentation {
                        message: format!(
                            "expected canonical path '{}' at position {index}, found '{path}'",
                            ROPROJ_V3_PATHS[index]
                        ),
                    },
                ));
            }
            canonical_files.push(CanonicalRoProjectFileV3 { path, bytes });
        }
        let tree = CanonicalRoProjectV3 {
            files: canonical_files,
        };
        let document =
            decode_unvalidated(&tree).map_err(CanonicalRoProjectV3AdmissionError::Format)?;
        profile(&document).map_err(CanonicalRoProjectV3AdmissionError::Profile)?;
        check_v3_document(&document).map_err(CanonicalRoProjectV3AdmissionError::Format)?;
        validate_semantic_expression_limits(&document)
            .map_err(CanonicalRoProjectV3AdmissionError::Format)?;
        validate_keyed_grouped_sum_definitions(&document).map_err(|error| {
            CanonicalRoProjectV3AdmissionError::Format(
                FormatError::InvalidRoProjectRepresentation {
                    message: format!("invalid keyed grouped-sum definition: {error}"),
                },
            )
        })?;
        let expected =
            encode_validated(&document).map_err(CanonicalRoProjectV3AdmissionError::Format)?;
        if expected != tree {
            return Err(CanonicalRoProjectV3AdmissionError::Format(
                FormatError::InvalidRoProjectRepresentation {
                    message: "tree bytes are not canonical .roproj/v3".to_owned(),
                },
            ));
        }
        Ok((tree, document))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestV3 {
    format: String,
    format_version: u32,
    document: DocumentIdentityV3,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentIdentityV3 {
    id: String,
    title: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaV3 {
    id: String,
    key: String,
    fields: Vec<FieldDefinitionV3>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FieldDefinitionV3 {
    id: String,
    key: String,
    field_type: FieldTypeV3,
    required: bool,
    constraint: ConstraintV3,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum ConstraintV3 {
    None {},
    TextLiteralSet { values: Vec<String> },
    NumberInclusiveRange { min: NumberV3, max: NumberV3 },
}

enum FieldTypeV3 {
    Number,
    Text,
    Boolean,
    Date,
    Reference { schema: String },
}

impl<'de> Deserialize<'de> for FieldTypeV3 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut object = BTreeMap::<String, serde_json::Value>::deserialize(deserializer)?;
        let field_type = object
            .remove("type")
            .ok_or_else(|| serde::de::Error::missing_field("type"))?;
        let serde_json::Value::String(field_type) = field_type else {
            return Err(serde::de::Error::custom("field `type` must be a string"));
        };

        match field_type.as_str() {
            "number" => {
                reject_unknown_field_type_members::<D>(&object, &["type"])?;
                Ok(Self::Number)
            }
            "text" => {
                reject_unknown_field_type_members::<D>(&object, &["type"])?;
                Ok(Self::Text)
            }
            "boolean" => {
                reject_unknown_field_type_members::<D>(&object, &["type"])?;
                Ok(Self::Boolean)
            }
            "date" => {
                reject_unknown_field_type_members::<D>(&object, &["type"])?;
                Ok(Self::Date)
            }
            "reference" => {
                let schema = object.get("schema");
                reject_unknown_field_type_members::<D>(&object, &["type", "schema"])?;
                let schema = schema.ok_or_else(|| serde::de::Error::missing_field("schema"))?;
                let serde_json::Value::String(schema) = schema else {
                    return Err(serde::de::Error::custom("field `schema` must be a string"));
                };
                Ok(Self::Reference {
                    schema: schema.clone(),
                })
            }
            other => Err(serde::de::Error::unknown_variant(
                other,
                &["number", "text", "boolean", "date", "reference"],
            )),
        }
    }
}

fn reject_unknown_field_type_members<'de, D>(
    object: &BTreeMap<String, serde_json::Value>,
    expected: &'static [&'static str],
) -> Result<(), D::Error>
where
    D: Deserializer<'de>,
{
    if let Some(member) = object
        .keys()
        .find(|member| !expected.contains(&member.as_str()))
    {
        return Err(serde::de::Error::unknown_field(member, expected));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntityV3 {
    id: String,
    key: String,
    schema: String,
    fields: BTreeMap<String, ValueV3>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(transparent)]
struct NumberV3(f64);

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum ValueV3 {
    Number(NumberV3),
    Text(String),
    Boolean(bool),
    Date(String),
    Reference(String),
    Formula(ExpressionV3),
}

#[derive(Deserialize)]
#[serde(
    tag = "op",
    content = "args",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum ExpressionV3 {
    Number(NumberV3),
    Reference(FieldRefV3),
    Add(BinaryArgumentsV3),
    Subtract(BinaryArgumentsV3),
    Multiply(BinaryArgumentsV3),
    Divide(BinaryArgumentsV3),
    Minimum(BinaryArgumentsV3),
    Maximum(BinaryArgumentsV3),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FieldRefV3 {
    entity: String,
    field: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BinaryArgumentsV3 {
    left: Box<ExpressionV3>,
    right: Box<ExpressionV3>,
}

/// `.roproj/v3` owns this closed-world definition DTO; it intentionally does
/// not serialize the semantic-core type directly.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DefinitionV3 {
    id: String,
    orders: OrdersBindingV3,
    products: ProductsBindingV3,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OrdersBindingV3 {
    schema: String,
    lookup_key_field: String,
    quantity_field: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductsBindingV3 {
    schema: String,
    key_field: String,
    category_field: String,
    price_field: String,
}

impl DefinitionV3 {
    fn into_semantic(self) -> KeyedGroupedSumDefinition {
        KeyedGroupedSumDefinition {
            id: self.id.into(),
            orders: KeyedGroupedSumOrdersBinding {
                schema: self.orders.schema.into(),
                lookup_key_field: self.orders.lookup_key_field.into(),
                quantity_field: self.orders.quantity_field.into(),
            },
            products: KeyedGroupedSumProductsBinding {
                schema: self.products.schema.into(),
                key_field: self.products.key_field.into(),
                category_field: self.products.category_field.into(),
                price_field: self.products.price_field.into(),
            },
        }
    }

    fn from_semantic(definition: &KeyedGroupedSumDefinition) -> Self {
        Self {
            id: definition.id.to_string(),
            orders: OrdersBindingV3 {
                schema: definition.orders.schema.to_string(),
                lookup_key_field: definition.orders.lookup_key_field.to_string(),
                quantity_field: definition.orders.quantity_field.to_string(),
            },
            products: ProductsBindingV3 {
                schema: definition.products.schema.to_string(),
                key_field: definition.products.key_field.to_string(),
                category_field: definition.products.category_field.to_string(),
                price_field: definition.products.price_field.to_string(),
            },
        }
    }
}

/// Encode a valid semantic document into the exact canonical `.roproj/v3` tree.
///
/// # Errors
///
/// Returns [`FormatError::InvalidDocument`] when semantic validation fails or
/// [`FormatError::Json`] when canonical JSON string encoding fails.
pub fn encode(document: &Document) -> Result<CanonicalRoProjectV3, FormatError> {
    let mut canonical = document.clone();
    for field in canonical
        .schemas
        .values_mut()
        .flat_map(|schema| schema.fields.values_mut())
    {
        if let FieldConstraint::TextLiteralSet { values } = &mut field.constraint {
            values.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
        }
    }
    check_v3_document(&canonical)?;
    validate_semantic_expression_limits(&canonical)?;
    validate_keyed_grouped_sum_definitions(&canonical).map_err(|error| {
        FormatError::InvalidRoProjectRepresentation {
            message: format!("invalid keyed grouped-sum definition: {error}"),
        }
    })?;
    encode_validated(&canonical)
}

fn encode_validated(document: &Document) -> Result<CanonicalRoProjectV3, FormatError> {
    let manifest = ManifestV3::from_semantic(document);
    let mut schemas = document
        .schemas
        .values()
        .map(SchemaV3::from_semantic)
        .collect::<Result<Vec<_>, _>>()?;
    schemas.sort_by(|left, right| left.id.as_bytes().cmp(right.id.as_bytes()));
    let mut entities = document
        .entities
        .values()
        .map(EntityV3::from_semantic)
        .collect::<Vec<_>>();
    entities.sort_by(|left, right| left.id.as_bytes().cmp(right.id.as_bytes()));

    let mut files = Vec::with_capacity(ROPROJ_V3_PATHS.len());
    files.push(CanonicalRoProjectFileV3 {
        path: ROPROJ_V3_PATHS[0].to_owned(),
        bytes: render_manifest(&manifest)?.into_bytes(),
    });
    files.push(CanonicalRoProjectFileV3 {
        path: ROPROJ_V3_PATHS[1].to_owned(),
        bytes: render_schemas(&schemas)?.into_bytes(),
    });
    files.push(CanonicalRoProjectFileV3 {
        path: ROPROJ_V3_PATHS[2].to_owned(),
        bytes: render_definitions(document)?.into_bytes(),
    });

    let mut shards = std::array::from_fn::<String, 16, _>(|_| String::new());
    for entity in entities {
        let shard = shard_index(&entity.id);
        write_entity(&mut shards[shard], &entity)?;
        shards[shard].push('\n');
    }
    for (index, path) in ROPROJ_V3_PATHS.iter().enumerate().skip(3).take(16) {
        files.push(CanonicalRoProjectFileV3 {
            path: (*path).to_owned(),
            bytes: std::mem::take(&mut shards[index - 3]).into_bytes(),
        });
    }

    Ok(CanonicalRoProjectV3 { files })
}

/// Decode an exact canonical `.roproj/v3` tree into the semantic document.
///
/// Manifest format/version dispatch completes before schema or entity bytes
/// receive DTO or semantic interpretation.
///
/// # Errors
///
/// Returns an explicit `.roproj` format/version error, a strict JSON or
/// representation error, or [`FormatError::InvalidDocument`] when semantic
/// validation fails.
pub fn decode(tree: &CanonicalRoProjectV3) -> Result<Document, FormatError> {
    let document = decode_unvalidated(tree)?;
    check_v3_document(&document)?;
    validate_semantic_expression_limits(&document)?;
    Ok(document)
}

/// Convert a validated v1 semantic document into the independently rendered
/// v3 representation. The v3 codec never delegates its v3 admission or DTO
/// interpretation to the v1 codec.
///
/// # Errors
///
/// Returns an error if the v1 tree cannot be decoded or the resulting semantic
/// document cannot be represented as canonical v3.
pub fn migrate_v1(
    tree: &super::v1::CanonicalRoProjectV1,
) -> Result<CanonicalRoProjectV3, FormatError> {
    let intermediate = super::v2::migrate_v1(tree)?;
    migrate_v2(&intermediate)
}

/// Convert a validated v2 tree by explicitly decoding v2 semantics, adding
/// `none` to each field constraint, and independently encoding canonical v3.
///
/// # Errors
///
/// Returns v2 admission or semantic errors, or a v3 representation error when
/// the decoded document cannot be represented by the v3 contract.
pub fn migrate_v2(
    tree: &super::v2::CanonicalRoProjectV2,
) -> Result<CanonicalRoProjectV3, FormatError> {
    let document = super::v2::decode(tree)?;
    encode(&document)
}

fn decode_unvalidated(tree: &CanonicalRoProjectV3) -> Result<Document, FormatError> {
    let manifest = decode_manifest(tree.file(ROPROJ_V3_PATHS[0]).ok_or_else(|| {
        FormatError::InvalidRoProjectRepresentation {
            message: "canonical tree is missing manifest.json".to_owned(),
        }
    })?)?;
    let schemas: Vec<SchemaV3> = decode_json_file(
        ROPROJ_V3_PATHS[1],
        tree.file(ROPROJ_V3_PATHS[1]).ok_or_else(|| {
            FormatError::InvalidRoProjectRepresentation {
                message: "canonical tree is missing schemas.json".to_owned(),
            }
        })?,
    )?;
    let schemas = schemas_into_semantic(schemas)?;
    let entities = decode_entities(tree)?;
    let definitions = decode_definitions(tree.file("definitions.json").ok_or_else(|| {
        FormatError::InvalidRoProjectRepresentation {
            message: "canonical tree is missing definitions.json".to_owned(),
        }
    })?)?;
    let document = Document {
        id: DocumentId::from(manifest.document.id),
        title: manifest.document.title,
        schemas,
        entities,
        keyed_grouped_sum_definitions: definitions,
    };
    Ok(document)
}

fn check_v3_document(document: &Document) -> Result<(), FormatError> {
    let diagnostics = validate_document(document);
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(FormatError::InvalidDocument { diagnostics })
    }
}

fn decode_manifest(bytes: &[u8]) -> Result<ManifestV3, FormatError> {
    let path = ROPROJ_V3_PATHS[0];
    let source = utf8(path, bytes)?;
    let inspection = inspect_roproj(source, ROPROJ_V3_MAX_JSON_NESTING)
        .map_err(|error| map_frontend_error(path, error))?;
    let value: serde_json::Value = deserialize_roproj(path, source)?;
    let object = value
        .as_object()
        .ok_or(FormatError::RoProjectFormatMalformed)?;
    match object.get("format") {
        None => return Err(FormatError::RoProjectFormatMissing),
        Some(serde_json::Value::String(format)) if format == "tachiko.roproj" => {}
        Some(_) => return Err(FormatError::RoProjectFormatMalformed),
    }
    let version = match inspection.version {
        None => return Err(FormatError::RoProjectVersionMissing),
        Some(VersionToken::Unsigned(version)) => version
            .parse::<u32>()
            .map_err(|_| FormatError::RoProjectVersionMalformed)?,
        Some(VersionToken::Other) => return Err(FormatError::RoProjectVersionMalformed),
    };
    if version == 0 {
        return Err(FormatError::RoProjectVersionMalformed);
    }
    if version != ROPROJ_V3_FORMAT_VERSION {
        return Err(FormatError::UnsupportedRoProjectVersion {
            found: version,
            supported: ROPROJ_V3_FORMAT_VERSION,
        });
    }
    let manifest: ManifestV3 = serde_json::from_value(value).map_err(|error| {
        FormatError::InvalidRoProjectRepresentation {
            message: format!("manifest.json does not match .roproj/v3: {error}"),
        }
    })?;
    if manifest.format != "tachiko.roproj" || manifest.format_version != ROPROJ_V3_FORMAT_VERSION {
        return Err(FormatError::RoProjectVersionMalformed);
    }
    require_id("document id", &manifest.document.id)?;
    Ok(manifest)
}

pub(crate) fn manifest_version(bytes: &[u8]) -> Result<u32, FormatError> {
    let path = ROPROJ_V3_PATHS[0];
    let source = utf8(path, bytes)?;
    let inspection = inspect_roproj(source, ROPROJ_V3_MAX_JSON_NESTING)
        .map_err(|error| map_frontend_error(path, error))?;
    let value: serde_json::Value = deserialize_roproj(path, source)?;
    let object = value
        .as_object()
        .ok_or(FormatError::RoProjectFormatMalformed)?;
    match object.get("format") {
        None => return Err(FormatError::RoProjectFormatMissing),
        Some(serde_json::Value::String(format)) if format == "tachiko.roproj" => {}
        Some(_) => return Err(FormatError::RoProjectFormatMalformed),
    }
    match inspection.version {
        None => Err(FormatError::RoProjectVersionMissing),
        Some(VersionToken::Unsigned(version)) => {
            let version = version
                .parse::<u32>()
                .map_err(|_| FormatError::RoProjectVersionMalformed)?;
            if version == 0 {
                Err(FormatError::RoProjectVersionMalformed)
            } else {
                Ok(version)
            }
        }
        Some(VersionToken::Other) => Err(FormatError::RoProjectVersionMalformed),
    }
}

fn decode_json_file<T>(path: &str, bytes: &[u8]) -> Result<T, FormatError>
where
    T: for<'de> Deserialize<'de>,
{
    let source = utf8(path, bytes)?;
    inspect_roproj(source, ROPROJ_V3_MAX_JSON_NESTING)
        .map_err(|error| map_frontend_error(path, error))?;
    deserialize_roproj(path, source)
}

fn decode_entities(tree: &CanonicalRoProjectV3) -> Result<BTreeMap<EntityId, Entity>, FormatError> {
    let mut entities = BTreeMap::new();
    for (shard, path) in ROPROJ_V3_PATHS.iter().enumerate().skip(3).take(16) {
        let bytes = tree
            .file(path)
            .ok_or_else(|| FormatError::InvalidRoProjectRepresentation {
                message: format!("canonical tree is missing '{path}'"),
            })?;
        if bytes.is_empty() {
            continue;
        }
        let source = utf8(path, bytes)?;
        let records = source.strip_suffix('\n').ok_or_else(|| {
            FormatError::InvalidRoProjectRepresentation {
                message: format!("nonempty entity shard '{path}' must end with one LF"),
            }
        })?;
        let mut previous_id: Option<String> = None;
        for (record_index, record) in records.split('\n').enumerate() {
            if record.is_empty() {
                return invalid_representation(format!(
                    "entity shard '{path}' contains a blank JSONL record"
                ));
            }
            let record_path = format!("{path}:{}", record_index + 1);
            inspect_roproj(record, ROPROJ_V3_MAX_JSON_NESTING)
                .map_err(|error| map_frontend_error(&record_path, error))?;
            let dto: EntityV3 = deserialize_roproj(&record_path, record)?;
            require_id("entity id", &dto.id)?;
            ensure_increasing("entity", previous_id.as_deref(), &dto.id)?;
            previous_id = Some(dto.id.clone());
            if shard_index(&dto.id) != shard - 3 {
                return invalid_representation(format!(
                    "entity '{}' is in wrong shard '{path}'",
                    dto.id
                ));
            }
            let id_text = dto.id.clone();
            let id = EntityId::from(id_text.clone());
            let entity = dto.into_semantic()?;
            if entities.insert(id, entity).is_some() {
                return invalid_representation(format!("duplicate entity id '{id_text}'"));
            }
        }
    }
    Ok(entities)
}

fn schemas_into_semantic(
    schemas: Vec<SchemaV3>,
) -> Result<BTreeMap<SchemaId, Schema>, FormatError> {
    let mut semantic = BTreeMap::new();
    let mut previous_id: Option<String> = None;
    for schema in schemas {
        require_id("schema id", &schema.id)?;
        ensure_increasing("schema", previous_id.as_deref(), &schema.id)?;
        previous_id = Some(schema.id.clone());
        let id = SchemaId::from(schema.id.clone());
        let id_text = schema.id.clone();
        let schema = schema.into_semantic()?;
        if semantic.insert(id, schema).is_some() {
            return invalid_representation(format!("duplicate schema id '{id_text}'"));
        }
    }
    Ok(semantic)
}

impl SchemaV3 {
    fn into_semantic(self) -> Result<Schema, FormatError> {
        let mut fields = BTreeMap::new();
        let mut previous_id: Option<String> = None;
        for field in self.fields {
            require_id("field id", &field.id)?;
            ensure_increasing("field", previous_id.as_deref(), &field.id)?;
            previous_id = Some(field.id.clone());
            let id = FieldId::from(field.id.clone());
            let id_text = field.id.clone();
            let definition = field.into_semantic()?;
            if fields.insert(id, definition).is_some() {
                return invalid_representation(format!("duplicate field id '{id_text}'"));
            }
        }
        Ok(Schema {
            id: SchemaId::from(self.id),
            key: SchemaKey::from(self.key),
            fields,
        })
    }
}

impl FieldDefinitionV3 {
    fn into_semantic(self) -> Result<FieldDefinition, FormatError> {
        Ok(FieldDefinition {
            id: FieldId::from(self.id),
            key: FieldKey::from(self.key),
            field_type: self.field_type.into_semantic()?,
            required: self.required,
            constraint: self.constraint.into_semantic()?,
        })
    }
}

impl ConstraintV3 {
    fn into_semantic(self) -> Result<FieldConstraint, FormatError> {
        match self {
            Self::None {} => Ok(FieldConstraint::None),
            Self::TextLiteralSet { values } => {
                validate_text_literals(&values, true)?;
                Ok(FieldConstraint::TextLiteralSet { values })
            }
            Self::NumberInclusiveRange { min, max } => {
                let min = min.into_semantic()?;
                let max = max.into_semantic()?;
                Ok(FieldConstraint::NumberInclusiveRange { min, max })
            }
        }
    }
}

fn validate_text_literals(values: &[String], require_order: bool) -> Result<(), FormatError> {
    if values.is_empty() || values.len() > 256 {
        return invalid_representation(
            "text_literal_set must contain 1 through 256 values".to_owned(),
        );
    }
    let mut total = 0_usize;
    let mut previous: Option<&str> = None;
    for value in values {
        if value.len() > 1024 {
            return invalid_representation(
                "text_literal_set value exceeds 1024 UTF-8 bytes".to_owned(),
            );
        }
        total = total.saturating_add(value.len());
        if let Some(previous) = previous {
            match previous.as_bytes().cmp(value.as_bytes()) {
                std::cmp::Ordering::Equal => {
                    return invalid_representation(
                        "text_literal_set contains a duplicate value".to_owned(),
                    );
                }
                std::cmp::Ordering::Greater if require_order => {
                    return invalid_representation(
                        "text_literal_set values are not in unsigned UTF-8 order".to_owned(),
                    );
                }
                std::cmp::Ordering::Less | std::cmp::Ordering::Greater => {}
            }
        }
        previous = Some(value);
    }
    if total > 65_536 {
        return invalid_representation(
            "text_literal_set exceeds 65536 aggregate UTF-8 bytes".to_owned(),
        );
    }
    Ok(())
}

impl FieldTypeV3 {
    fn into_semantic(self) -> Result<FieldType, FormatError> {
        Ok(match self {
            Self::Number => FieldType::Number,
            Self::Text => FieldType::Text,
            Self::Boolean => FieldType::Boolean,
            Self::Date => FieldType::Date,
            Self::Reference { schema } => {
                require_id("reference field target schema id", &schema)?;
                FieldType::Reference {
                    schema: SchemaId::from(schema),
                }
            }
        })
    }
}

impl EntityV3 {
    fn into_semantic(self) -> Result<Entity, FormatError> {
        require_id("entity schema id", &self.schema)?;
        let fields = self
            .fields
            .into_iter()
            .map(|(id, value)| {
                require_id("entity field id", &id)?;
                Ok((FieldId::from(id), value.into_semantic()?))
            })
            .collect::<Result<_, FormatError>>()?;
        Ok(Entity {
            id: EntityId::from(self.id),
            key: EntityKey::from(self.key),
            schema: SchemaId::from(self.schema),
            fields,
        })
    }
}

impl ValueV3 {
    fn into_semantic(self) -> Result<Value, FormatError> {
        Ok(match self {
            Self::Number(number) => Value::Number(number.into_semantic()?),
            Self::Text(text) => Value::Text(text),
            Self::Boolean(boolean) => Value::Boolean(boolean),
            Self::Date(date) => Value::Date(date.parse().map_err(|_| {
                FormatError::InvalidRoProjectRepresentation {
                    message: format!("invalid canonical date value '{date}'"),
                }
            })?),
            Self::Reference(entity) => {
                require_id("entity reference id", &entity)?;
                Value::Reference(EntityId::from(entity))
            }
            Self::Formula(expression) => {
                expression.validate_ids_and_limits()?;
                Value::Formula(expression.into_semantic()?)
            }
        })
    }
}

impl NumberV3 {
    fn into_semantic(self) -> Result<Number, FormatError> {
        Number::new(self.0).map_err(|_| FormatError::InvalidRoProjectRepresentation {
            message: "number must be finite".to_owned(),
        })
    }
}

impl ExpressionV3 {
    fn validate_ids_and_limits(&self) -> Result<(), FormatError> {
        let mut nodes = 0_usize;
        let mut stack = vec![(self, 1_usize)];
        while let Some((node, depth)) = stack.pop() {
            if depth > MAX_EXPRESSION_DEPTH {
                return invalid_representation(format!(
                    "formula expression exceeds {MAX_EXPRESSION_DEPTH}-depth limit"
                ));
            }
            nodes += 1;
            if nodes > MAX_EXPRESSION_NODES {
                return invalid_representation(format!(
                    "formula expression exceeds {MAX_EXPRESSION_NODES}-node limit"
                ));
            }
            match node {
                Self::Reference(reference) => {
                    require_id("formula entity id", &reference.entity)?;
                    require_id("formula field id", &reference.field)?;
                }
                Self::Add(arguments)
                | Self::Subtract(arguments)
                | Self::Multiply(arguments)
                | Self::Divide(arguments)
                | Self::Minimum(arguments)
                | Self::Maximum(arguments) => {
                    stack.push((&arguments.right, depth + 1));
                    stack.push((&arguments.left, depth + 1));
                }
                Self::Number(_) => {}
            }
        }
        Ok(())
    }

    fn into_semantic(self) -> Result<Expression, FormatError> {
        Ok(match self {
            Self::Number(number) => Expression::Number(number.into_semantic()?),
            Self::Reference(reference) => {
                Expression::Reference(FieldRef::new(reference.entity, reference.field))
            }
            Self::Add(arguments) => {
                arguments.into_semantic(|left, right| Expression::Add { left, right })?
            }
            Self::Subtract(arguments) => {
                arguments.into_semantic(|left, right| Expression::Subtract { left, right })?
            }
            Self::Multiply(arguments) => {
                arguments.into_semantic(|left, right| Expression::Multiply { left, right })?
            }
            Self::Divide(arguments) => {
                arguments.into_semantic(|left, right| Expression::Divide { left, right })?
            }
            Self::Minimum(arguments) => {
                arguments.into_semantic(|left, right| Expression::Minimum { left, right })?
            }
            Self::Maximum(arguments) => {
                arguments.into_semantic(|left, right| Expression::Maximum { left, right })?
            }
        })
    }
}

impl BinaryArgumentsV3 {
    fn into_semantic(
        self,
        constructor: impl FnOnce(Box<Expression>, Box<Expression>) -> Expression,
    ) -> Result<Expression, FormatError> {
        Ok(constructor(
            Box::new(self.left.into_semantic()?),
            Box::new(self.right.into_semantic()?),
        ))
    }
}

impl ManifestV3 {
    fn from_semantic(document: &Document) -> Self {
        Self {
            format: "tachiko.roproj".to_owned(),
            format_version: ROPROJ_V3_FORMAT_VERSION,
            document: DocumentIdentityV3 {
                id: document.id.to_string(),
                title: document.title.clone(),
            },
        }
    }
}

impl SchemaV3 {
    fn from_semantic(schema: &Schema) -> Result<Self, FormatError> {
        let mut fields = schema
            .fields
            .values()
            .map(FieldDefinitionV3::from_semantic)
            .collect::<Result<Vec<_>, _>>()?;
        fields.sort_by(|left, right| left.id.as_bytes().cmp(right.id.as_bytes()));
        Ok(Self {
            id: schema.id.to_string(),
            key: schema.key.to_string(),
            fields,
        })
    }
}

impl FieldDefinitionV3 {
    fn from_semantic(field: &FieldDefinition) -> Result<Self, FormatError> {
        Ok(Self {
            id: field.id.to_string(),
            key: field.key.to_string(),
            field_type: FieldTypeV3::from_semantic(&field.field_type),
            required: field.required,
            constraint: ConstraintV3::from_semantic(&field.constraint)?,
        })
    }
}

impl ConstraintV3 {
    fn from_semantic(constraint: &FieldConstraint) -> Result<Self, FormatError> {
        Ok(match constraint {
            FieldConstraint::None => Self::None {},
            FieldConstraint::TextLiteralSet { values } => {
                validate_text_literals(values, false)?;
                let mut values = values.clone();
                values.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
                Self::TextLiteralSet { values }
            }
            FieldConstraint::NumberInclusiveRange { min, max } => Self::NumberInclusiveRange {
                min: NumberV3(min.get()),
                max: NumberV3(max.get()),
            },
        })
    }
}

impl FieldTypeV3 {
    fn from_semantic(field_type: &FieldType) -> Self {
        match field_type {
            FieldType::Number => Self::Number,
            FieldType::Text => Self::Text,
            FieldType::Boolean => Self::Boolean,
            FieldType::Date => Self::Date,
            FieldType::Reference { schema } => Self::Reference {
                schema: schema.to_string(),
            },
        }
    }
}

impl EntityV3 {
    fn from_semantic(entity: &Entity) -> Self {
        let fields = entity
            .fields
            .iter()
            .map(|(id, value)| (id.to_string(), ValueV3::from_semantic(value)))
            .collect();
        Self {
            id: entity.id.to_string(),
            key: entity.key.to_string(),
            schema: entity.schema.to_string(),
            fields,
        }
    }
}

impl ValueV3 {
    fn from_semantic(value: &Value) -> Self {
        match value {
            Value::Number(number) => Self::Number(NumberV3(number.get())),
            Value::Text(text) => Self::Text(text.clone()),
            Value::Boolean(boolean) => Self::Boolean(*boolean),
            Value::Date(date) => Self::Date(date.to_canonical_string()),
            Value::Reference(entity) => Self::Reference(entity.to_string()),
            Value::Formula(expression) => Self::Formula(ExpressionV3::from_semantic(expression)),
        }
    }
}

impl ExpressionV3 {
    fn from_semantic(expression: &Expression) -> Self {
        match expression {
            Expression::Number(number) => Self::Number(NumberV3(number.get())),
            Expression::Reference(reference) => {
                Self::Reference(FieldRefV3::from_semantic(reference))
            }
            Expression::Add { left, right } => {
                Self::Add(BinaryArgumentsV3::from_semantic(left, right))
            }
            Expression::Subtract { left, right } => {
                Self::Subtract(BinaryArgumentsV3::from_semantic(left, right))
            }
            Expression::Multiply { left, right } => {
                Self::Multiply(BinaryArgumentsV3::from_semantic(left, right))
            }
            Expression::Divide { left, right } => {
                Self::Divide(BinaryArgumentsV3::from_semantic(left, right))
            }
            Expression::Minimum { left, right } => {
                Self::Minimum(BinaryArgumentsV3::from_semantic(left, right))
            }
            Expression::Maximum { left, right } => {
                Self::Maximum(BinaryArgumentsV3::from_semantic(left, right))
            }
        }
    }
}

impl FieldRefV3 {
    fn from_semantic(reference: &FieldRef) -> Self {
        Self {
            entity: reference.entity.to_string(),
            field: reference.field.to_string(),
        }
    }
}

impl BinaryArgumentsV3 {
    fn from_semantic(left: &Expression, right: &Expression) -> Self {
        Self {
            left: Box::new(ExpressionV3::from_semantic(left)),
            right: Box::new(ExpressionV3::from_semantic(right)),
        }
    }
}

fn render_manifest(manifest: &ManifestV3) -> Result<String, FormatError> {
    let mut output = String::new();
    output.push_str("{\n");
    pretty_member_string(&mut output, 1, "format", &manifest.format, true)?;
    pretty_member_literal(
        &mut output,
        1,
        "format_version",
        &manifest.format_version.to_string(),
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

fn render_schemas(schemas: &[SchemaV3]) -> Result<String, FormatError> {
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

fn decode_definitions(
    bytes: &[u8],
) -> Result<BTreeMap<KeyedGroupedSumDefinitionId, KeyedGroupedSumDefinition>, FormatError> {
    let records: Vec<DefinitionV3> = decode_json_file("definitions.json", bytes)?;
    let mut definitions = BTreeMap::new();
    for record in records {
        require_id("keyed grouped-sum definition id", &record.id)?;
        let id_text = record.id.clone();
        let definition = record.into_semantic();
        if definitions
            .insert(definition.id.clone(), definition)
            .is_some()
        {
            return invalid_representation(format!(
                "duplicate keyed grouped-sum definition id '{id_text}'"
            ));
        }
    }
    Ok(definitions)
}

fn render_definitions(document: &Document) -> Result<String, FormatError> {
    let mut definitions = document
        .keyed_grouped_sum_definitions
        .values()
        .map(DefinitionV3::from_semantic)
        .collect::<Vec<_>>();
    definitions.sort_by(|left, right| left.id.as_bytes().cmp(right.id.as_bytes()));
    let mut output = String::new();
    if definitions.is_empty() {
        output.push_str("[]\n");
        return Ok(output);
    }
    output.push_str("[\n");
    for (index, definition) in definitions.iter().enumerate() {
        write_definition(&mut output, definition, 1)?;
        if index + 1 != definitions.len() {
            output.push(',');
        }
        output.push('\n');
    }
    output.push_str("]\n");
    Ok(output)
}

fn write_definition(
    output: &mut String,
    definition: &DefinitionV3,
    indent: usize,
) -> Result<(), FormatError> {
    push_indent(output, indent);
    output.push_str("{\n");
    pretty_member_string(output, indent + 1, "id", &definition.id, true)?;
    pretty_member_prefix(output, indent + 1, "orders")?;
    output.push_str("{\n");
    pretty_member_string(
        output,
        indent + 2,
        "schema",
        &definition.orders.schema,
        true,
    )?;
    pretty_member_string(
        output,
        indent + 2,
        "lookup_key_field",
        &definition.orders.lookup_key_field,
        true,
    )?;
    pretty_member_string(
        output,
        indent + 2,
        "quantity_field",
        &definition.orders.quantity_field,
        false,
    )?;
    push_indent(output, indent + 1);
    output.push_str("},\n");
    pretty_member_prefix(output, indent + 1, "products")?;
    output.push_str("{\n");
    pretty_member_string(
        output,
        indent + 2,
        "schema",
        &definition.products.schema,
        true,
    )?;
    pretty_member_string(
        output,
        indent + 2,
        "key_field",
        &definition.products.key_field,
        true,
    )?;
    pretty_member_string(
        output,
        indent + 2,
        "category_field",
        &definition.products.category_field,
        true,
    )?;
    pretty_member_string(
        output,
        indent + 2,
        "price_field",
        &definition.products.price_field,
        false,
    )?;
    push_indent(output, indent + 1);
    output.push_str("}\n");
    push_indent(output, indent);
    output.push('}');
    Ok(())
}

fn write_schema(output: &mut String, schema: &SchemaV3, indent: usize) -> Result<(), FormatError> {
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
    field: &FieldDefinitionV3,
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
        true,
    )?;
    pretty_member_prefix(output, indent + 1, "constraint")?;
    write_constraint(output, &field.constraint, indent + 1)?;
    output.push('\n');
    push_indent(output, indent);
    output.push('}');
    Ok(())
}

fn write_field_type(
    output: &mut String,
    field_type: &FieldTypeV3,
    indent: usize,
) -> Result<(), FormatError> {
    output.push_str("{\n");
    match field_type {
        FieldTypeV3::Number => pretty_member_string(output, indent + 1, "type", "number", false)?,
        FieldTypeV3::Text => pretty_member_string(output, indent + 1, "type", "text", false)?,
        FieldTypeV3::Boolean => pretty_member_string(output, indent + 1, "type", "boolean", false)?,
        FieldTypeV3::Date => pretty_member_string(output, indent + 1, "type", "date", false)?,
        FieldTypeV3::Reference { schema } => {
            pretty_member_string(output, indent + 1, "type", "reference", true)?;
            pretty_member_string(output, indent + 1, "schema", schema, false)?;
        }
    }
    push_indent(output, indent);
    output.push('}');
    Ok(())
}

fn write_constraint(
    output: &mut String,
    constraint: &ConstraintV3,
    indent: usize,
) -> Result<(), FormatError> {
    output.push_str("{\n");
    match constraint {
        ConstraintV3::None {} => pretty_member_string(output, indent + 1, "type", "none", false)?,
        ConstraintV3::TextLiteralSet { values } => {
            pretty_member_string(output, indent + 1, "type", "text_literal_set", true)?;
            pretty_member_prefix(output, indent + 1, "values")?;
            if values.is_empty() {
                output.push_str("[]\n");
            } else {
                output.push_str("[\n");
                for (index, value) in values.iter().enumerate() {
                    push_indent(output, indent + 2);
                    write_json_string(output, value)?;
                    if index + 1 != values.len() {
                        output.push(',');
                    }
                    output.push('\n');
                }
                push_indent(output, indent + 1);
                output.push_str("]\n");
            }
        }
        ConstraintV3::NumberInclusiveRange { min, max } => {
            pretty_member_string(output, indent + 1, "type", "number_inclusive_range", true)?;
            pretty_member_number(output, indent + 1, "min", *min, true)?;
            pretty_member_number(output, indent + 1, "max", *max, false)?;
        }
    }
    push_indent(output, indent);
    output.push('}');
    Ok(())
}

fn pretty_member_number(
    output: &mut String,
    indent: usize,
    name: &str,
    value: NumberV3,
    comma: bool,
) -> Result<(), FormatError> {
    if !value.0.is_finite() {
        return invalid_representation("number must be finite".to_owned());
    }
    pretty_member_prefix(output, indent, name)?;
    let mut buffer = ryu_js::Buffer::new();
    output.push_str(buffer.format_finite(if value.0 == 0.0 { 0.0 } else { value.0 }));
    if comma {
        output.push(',');
    }
    output.push('\n');
    Ok(())
}

fn write_entity(output: &mut String, entity: &EntityV3) -> Result<(), FormatError> {
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

fn write_value(output: &mut String, value: &ValueV3) -> Result<(), FormatError> {
    output.push('{');
    match value {
        ValueV3::Number(number) => {
            compact_member_string(output, "kind", "number", true)?;
            compact_member_number(output, "value", *number, false)?;
        }
        ValueV3::Text(text) => {
            compact_member_string(output, "kind", "text", true)?;
            compact_member_string(output, "value", text, false)?;
        }
        ValueV3::Boolean(boolean) => {
            compact_member_string(output, "kind", "boolean", true)?;
            compact_member_literal(
                output,
                "value",
                if *boolean { "true" } else { "false" },
                false,
            )?;
        }
        ValueV3::Date(date) => {
            compact_member_string(output, "kind", "date", true)?;
            compact_member_string(output, "value", date, false)?;
        }
        ValueV3::Reference(entity) => {
            compact_member_string(output, "kind", "reference", true)?;
            compact_member_string(output, "value", entity, false)?;
        }
        ValueV3::Formula(expression) => {
            compact_member_string(output, "kind", "formula", true)?;
            compact_member_prefix(output, "value")?;
            write_expression(output, expression)?;
        }
    }
    output.push('}');
    Ok(())
}

fn write_expression(output: &mut String, expression: &ExpressionV3) -> Result<(), FormatError> {
    output.push('{');
    match expression {
        ExpressionV3::Number(number) => {
            compact_member_string(output, "op", "number", true)?;
            compact_member_number(output, "args", *number, false)?;
        }
        ExpressionV3::Reference(reference) => {
            compact_member_string(output, "op", "reference", true)?;
            compact_member_prefix(output, "args")?;
            output.push('{');
            compact_member_string(output, "entity", &reference.entity, true)?;
            compact_member_string(output, "field", &reference.field, false)?;
            output.push('}');
        }
        ExpressionV3::Add(args) => write_binary_expression(output, "add", args)?,
        ExpressionV3::Subtract(args) => write_binary_expression(output, "subtract", args)?,
        ExpressionV3::Multiply(args) => write_binary_expression(output, "multiply", args)?,
        ExpressionV3::Divide(args) => write_binary_expression(output, "divide", args)?,
        ExpressionV3::Minimum(args) => write_binary_expression(output, "minimum", args)?,
        ExpressionV3::Maximum(args) => write_binary_expression(output, "maximum", args)?,
    }
    output.push('}');
    Ok(())
}

fn write_binary_expression(
    output: &mut String,
    operator: &str,
    arguments: &BinaryArgumentsV3,
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
    value: NumberV3,
    comma: bool,
) -> Result<(), FormatError> {
    if !value.0.is_finite() {
        return invalid_representation("number must be finite".to_owned());
    }
    let value = if value.0 == 0.0 { 0.0 } else { value.0 };
    let mut buffer = ryu_js::Buffer::new();
    compact_member_prefix(output, name)?;
    output.push_str(buffer.format_finite(value));
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

fn validate_semantic_expression_limits(document: &Document) -> Result<(), FormatError> {
    for entity in document.entities.values() {
        for value in entity.fields.values() {
            let Value::Formula(expression) = value else {
                continue;
            };
            let mut nodes = 0_usize;
            let mut stack = vec![(expression, 1_usize)];
            while let Some((node, depth)) = stack.pop() {
                if depth > MAX_EXPRESSION_DEPTH {
                    return invalid_representation(format!(
                        "formula expression exceeds {MAX_EXPRESSION_DEPTH}-depth limit"
                    ));
                }
                nodes += 1;
                if nodes > MAX_EXPRESSION_NODES {
                    return invalid_representation(format!(
                        "formula expression exceeds {MAX_EXPRESSION_NODES}-node limit"
                    ));
                }
                match node {
                    Expression::Add { left, right }
                    | Expression::Subtract { left, right }
                    | Expression::Multiply { left, right }
                    | Expression::Divide { left, right }
                    | Expression::Minimum { left, right }
                    | Expression::Maximum { left, right } => {
                        stack.push((right, depth + 1));
                        stack.push((left, depth + 1));
                    }
                    Expression::Number(_) | Expression::Reference(_) => {}
                }
            }
        }
    }
    Ok(())
}

fn utf8<'a>(path: &str, bytes: &'a [u8]) -> Result<&'a str, FormatError> {
    std::str::from_utf8(bytes).map_err(|source| FormatError::InvalidRoProjectUtf8 {
        path: path.to_owned(),
        source,
    })
}

fn map_frontend_error(path: &str, error: FrontendError) -> FormatError {
    match error {
        FrontendError::InvalidJson(source) => FormatError::InvalidRoProjectJson {
            path: path.to_owned(),
            source,
        },
        FrontendError::DuplicateMember(member) => FormatError::DuplicateRoProjectMember {
            path: path.to_owned(),
            member,
        },
        FrontendError::NestingLimit { limit, .. } => FormatError::InvalidRoProjectRepresentation {
            message: format!("'{path}' exceeds .roproj/v3 JSON nesting limit {limit}"),
        },
    }
}

fn deserialize_roproj<T>(path: &str, source: &str) -> Result<T, FormatError>
where
    T: DeserializeOwned,
{
    let mut deserializer = serde_json::Deserializer::from_str(source);
    deserializer.disable_recursion_limit();
    let value = T::deserialize(&mut deserializer).map_err(|error| {
        FormatError::InvalidRoProjectRepresentation {
            message: format!("'{path}' does not match .roproj/v3: {error}"),
        }
    })?;
    deserializer
        .end()
        .map_err(|error| FormatError::InvalidRoProjectRepresentation {
            message: format!("'{path}' does not match .roproj/v3: {error}"),
        })?;
    Ok(value)
}

fn require_id(kind: &str, id: &str) -> Result<(), FormatError> {
    if id.is_empty() {
        invalid_representation(format!("{kind} must not be empty"))
    } else {
        Ok(())
    }
}

fn ensure_increasing(kind: &str, previous: Option<&str>, current: &str) -> Result<(), FormatError> {
    let Some(previous) = previous else {
        return Ok(());
    };
    match previous.as_bytes().cmp(current.as_bytes()) {
        std::cmp::Ordering::Less => Ok(()),
        std::cmp::Ordering::Equal => {
            invalid_representation(format!("duplicate {kind} id '{current}'"))
        }
        std::cmp::Ordering::Greater => invalid_representation(format!(
            "{kind} ids are not in unsigned UTF-8 order: '{current}' follows '{previous}'"
        )),
    }
}

fn invalid_representation<T>(message: String) -> Result<T, FormatError> {
    Err(FormatError::InvalidRoProjectRepresentation { message })
}
