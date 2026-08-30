//! Storage-owned `.roproj/v1` DTOs and canonical tree codec.

use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;

use serde::{Deserialize, Deserializer, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, EntityKey, Expression, FieldDefinition, FieldId,
    FieldKey, FieldRef, FieldType, MAX_EXPRESSION_DEPTH, MAX_EXPRESSION_NODES, Number, Schema,
    SchemaId, SchemaKey, Value,
};

use crate::{
    FormatError,
    strict_json::{FrontendError, VersionToken, inspect_roproj},
};

pub const ROPROJ_V1_FORMAT_VERSION: u32 = 1;
// A deepest valid entity is: entity -> fields -> value -> 64 expression
// objects, with one binary `args` object between each expression node and one
// reference `args` object at the leaf. No valid v1 DTO nests more deeply.
const ROPROJ_V1_MAX_JSON_NESTING: usize = 132;
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

#[derive(Debug)]
pub enum CanonicalRoProjectAdmissionError<E> {
    Format(FormatError),
    Profile(E),
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

    /// Construct an exact canonical `.roproj/v1` tree from ordered path/byte pairs.
    ///
    /// # Errors
    ///
    /// Returns a `.roproj` format, version, JSON, semantic, or representation
    /// error unless the input is the exact canonical eighteen-file tree.
    pub fn try_from_files(files: Vec<(String, Vec<u8>)>) -> Result<Self, FormatError> {
        match Self::try_from_files_with_profile(files, |_| Ok::<(), Infallible>(())) {
            Ok((tree, _)) => Ok(tree),
            Err(CanonicalRoProjectAdmissionError::Format(error)) => Err(error),
            Err(CanonicalRoProjectAdmissionError::Profile(never)) => match never {},
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
    ) -> Result<(Self, Document), CanonicalRoProjectAdmissionError<E>> {
        if files.len() != ROPROJ_V1_PATHS.len() {
            return Err(CanonicalRoProjectAdmissionError::Format(
                FormatError::InvalidRoProjectRepresentation {
                    message: format!(
                        "canonical tree requires {} files, found {}",
                        ROPROJ_V1_PATHS.len(),
                        files.len()
                    ),
                },
            ));
        }
        for (index, ((path, _), expected)) in files.iter().zip(ROPROJ_V1_PATHS).enumerate() {
            if path != expected {
                return Err(CanonicalRoProjectAdmissionError::Format(
                    FormatError::InvalidRoProjectRepresentation {
                        message: format!(
                            "canonical path {index} must be '{expected}', found '{path}'"
                        ),
                    },
                ));
            }
        }
        let tree = Self {
            files: files
                .into_iter()
                .map(|(path, bytes)| CanonicalRoProjectFile { path, bytes })
                .collect(),
        };
        let document =
            decode_unvalidated(&tree).map_err(CanonicalRoProjectAdmissionError::Format)?;
        profile(&document).map_err(CanonicalRoProjectAdmissionError::Profile)?;
        super::super::check_document(&document)
            .map_err(CanonicalRoProjectAdmissionError::Format)?;
        validate_semantic_expression_limits(&document)
            .map_err(CanonicalRoProjectAdmissionError::Format)?;
        let canonical =
            encode_validated(&document).map_err(CanonicalRoProjectAdmissionError::Format)?;
        if tree != canonical {
            return Err(CanonicalRoProjectAdmissionError::Format(
                FormatError::InvalidRoProjectRepresentation {
                    message: "tree bytes are not canonical .roproj/v1".to_owned(),
                },
            ));
        }
        Ok((tree, document))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestV1 {
    format: String,
    format_version: u32,
    document: DocumentIdentityV1,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentIdentityV1 {
    id: String,
    title: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaV1 {
    id: String,
    key: String,
    fields: Vec<FieldDefinitionV1>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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

impl<'de> Deserialize<'de> for FieldTypeV1 {
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
                &["number", "text", "boolean", "reference"],
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
struct EntityV1 {
    id: String,
    key: String,
    schema: String,
    fields: BTreeMap<String, ValueV1>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(transparent)]
struct NumberV1(f64);

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum ValueV1 {
    Number(NumberV1),
    Text(String),
    Boolean(bool),
    Reference(String),
    Formula(ExpressionV1),
}

#[derive(Deserialize)]
#[serde(
    tag = "op",
    content = "args",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum ExpressionV1 {
    Number(NumberV1),
    Reference(FieldRefV1),
    Add(BinaryArgumentsV1),
    Subtract(BinaryArgumentsV1),
    Multiply(BinaryArgumentsV1),
    Divide(BinaryArgumentsV1),
    Minimum(BinaryArgumentsV1),
    Maximum(BinaryArgumentsV1),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FieldRefV1 {
    entity: String,
    field: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BinaryArgumentsV1 {
    left: Box<ExpressionV1>,
    right: Box<ExpressionV1>,
}

struct UnorderedRoProjectV1 {
    manifest: ManifestV1,
    schemas: Vec<SchemaV1>,
    entities: Vec<EntityV1>,
}

/// Encode a valid semantic document into the exact canonical `.roproj/v1` tree.
///
/// # Errors
///
/// Returns [`FormatError::InvalidDocument`] when semantic validation fails or
/// [`FormatError::Json`] when canonical JSON string encoding fails.
pub fn encode(document: &Document) -> Result<CanonicalRoProjectV1, FormatError> {
    super::super::check_document(document)?;
    validate_semantic_expression_limits(document)?;
    encode_validated(document)
}

fn encode_validated(document: &Document) -> Result<CanonicalRoProjectV1, FormatError> {
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

/// Decode an exact canonical `.roproj/v1` tree into the semantic document.
///
/// Manifest format/version dispatch completes before schema or entity bytes
/// receive DTO or semantic interpretation.
///
/// # Errors
///
/// Returns an explicit `.roproj` format/version error, a strict JSON or
/// representation error, or [`FormatError::InvalidDocument`] when semantic
/// validation fails.
pub fn decode(tree: &CanonicalRoProjectV1) -> Result<Document, FormatError> {
    let document = decode_unvalidated(tree)?;
    super::super::check_document(&document)?;
    validate_semantic_expression_limits(&document)?;
    Ok(document)
}

fn decode_unvalidated(tree: &CanonicalRoProjectV1) -> Result<Document, FormatError> {
    let manifest = decode_manifest(tree.file(ROPROJ_V1_PATHS[0]).ok_or_else(|| {
        FormatError::InvalidRoProjectRepresentation {
            message: "canonical tree is missing manifest.json".to_owned(),
        }
    })?)?;
    let schemas: Vec<SchemaV1> = decode_json_file(
        ROPROJ_V1_PATHS[1],
        tree.file(ROPROJ_V1_PATHS[1]).ok_or_else(|| {
            FormatError::InvalidRoProjectRepresentation {
                message: "canonical tree is missing schemas.json".to_owned(),
            }
        })?,
    )?;
    let schemas = schemas_into_semantic(schemas)?;
    let entities = decode_entities(tree)?;
    let document = Document {
        id: DocumentId::from(manifest.document.id),
        title: manifest.document.title,
        schemas,
        entities,
    };
    Ok(document)
}

fn decode_manifest(bytes: &[u8]) -> Result<ManifestV1, FormatError> {
    let path = ROPROJ_V1_PATHS[0];
    let source = utf8(path, bytes)?;
    let inspection = inspect_roproj(source, ROPROJ_V1_MAX_JSON_NESTING)
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
    if version != ROPROJ_V1_FORMAT_VERSION {
        return Err(FormatError::UnsupportedRoProjectVersion {
            found: version,
            supported: ROPROJ_V1_FORMAT_VERSION,
        });
    }
    let manifest: ManifestV1 = serde_json::from_value(value).map_err(|error| {
        FormatError::InvalidRoProjectRepresentation {
            message: format!("manifest.json does not match .roproj/v1: {error}"),
        }
    })?;
    if manifest.format != "tachiko.roproj" || manifest.format_version != ROPROJ_V1_FORMAT_VERSION {
        return Err(FormatError::RoProjectVersionMalformed);
    }
    require_id("document id", &manifest.document.id)?;
    Ok(manifest)
}

pub(crate) fn dispatch_manifest(bytes: &[u8]) -> Result<(), FormatError> {
    decode_manifest(bytes).map(|_| ())
}

pub(crate) fn canonicalize_unordered(
    manifest_bytes: &[u8],
    schemas_bytes: &[u8],
    entity_records: Vec<(String, Vec<u8>)>,
) -> Result<CanonicalRoProjectV1, FormatError> {
    let aggregate = UnorderedRoProjectV1::decode(manifest_bytes, schemas_bytes, entity_records)?;
    aggregate.prove_scoped_uniqueness()?;
    let document = aggregate.into_semantic()?;
    super::super::check_document(&document)?;
    validate_semantic_expression_limits(&document)?;
    encode(&document)
}

fn decode_json_file<T>(path: &str, bytes: &[u8]) -> Result<T, FormatError>
where
    T: for<'de> Deserialize<'de>,
{
    let source = utf8(path, bytes)?;
    inspect_roproj(source, ROPROJ_V1_MAX_JSON_NESTING)
        .map_err(|error| map_frontend_error(path, error))?;
    deserialize_roproj(path, source)
}

fn decode_json_file_unordered<T>(path: &str, bytes: &[u8]) -> Result<T, FormatError>
where
    T: DeserializeOwned,
{
    let source = utf8(path, bytes)?;
    inspect_roproj(source, ROPROJ_V1_MAX_JSON_NESTING)
        .map_err(|error| map_frontend_error(path, error))?;
    let value: serde_json::Value = deserialize_roproj(path, source)?;
    serde_json::from_value(value).map_err(|error| FormatError::InvalidRoProjectRepresentation {
        message: format!("'{path}' does not match .roproj/v1: {error}"),
    })
}

fn decode_entities(tree: &CanonicalRoProjectV1) -> Result<BTreeMap<EntityId, Entity>, FormatError> {
    let mut entities = BTreeMap::new();
    for (shard, path) in ROPROJ_V1_PATHS.iter().enumerate().skip(2) {
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
            inspect_roproj(record, ROPROJ_V1_MAX_JSON_NESTING)
                .map_err(|error| map_frontend_error(&record_path, error))?;
            let dto: EntityV1 = deserialize_roproj(&record_path, record)?;
            require_id("entity id", &dto.id)?;
            ensure_increasing("entity", previous_id.as_deref(), &dto.id)?;
            previous_id = Some(dto.id.clone());
            if shard_index(&dto.id) != shard - 2 {
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
    schemas: Vec<SchemaV1>,
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

impl UnorderedRoProjectV1 {
    fn decode(
        manifest_bytes: &[u8],
        schemas_bytes: &[u8],
        entity_records: Vec<(String, Vec<u8>)>,
    ) -> Result<Self, FormatError> {
        let manifest = decode_manifest(manifest_bytes)?;
        let schemas = decode_json_file_unordered(ROPROJ_V1_PATHS[1], schemas_bytes)?;
        let entities = entity_records
            .into_iter()
            .map(|(record_path, bytes)| decode_json_file_unordered(&record_path, &bytes))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            manifest,
            schemas,
            entities,
        })
    }

    fn prove_scoped_uniqueness(&self) -> Result<(), FormatError> {
        let mut schema_ids = BTreeSet::new();
        for schema in &self.schemas {
            require_id("schema id", &schema.id)?;
            if !schema_ids.insert(schema.id.as_str()) {
                return invalid_representation(format!("duplicate schema id '{}'", schema.id));
            }
        }

        for schema in &self.schemas {
            let mut field_ids = BTreeSet::new();
            for field in &schema.fields {
                require_id("field id", &field.id)?;
                if !field_ids.insert(field.id.as_str()) {
                    return invalid_representation(format!("duplicate field id '{}'", field.id));
                }
            }
        }

        let mut entity_ids = BTreeSet::new();
        for entity in &self.entities {
            require_id("entity id", &entity.id)?;
            if !entity_ids.insert(entity.id.as_str()) {
                return invalid_representation(format!("duplicate entity id '{}'", entity.id));
            }
        }
        Ok(())
    }

    fn into_semantic(self) -> Result<Document, FormatError> {
        let schemas = self
            .schemas
            .into_iter()
            .map(|schema| {
                let id = SchemaId::from(schema.id.clone());
                Ok((id, schema.into_semantic_unordered()?))
            })
            .collect::<Result<_, FormatError>>()?;
        let entities = self
            .entities
            .into_iter()
            .map(|entity| {
                let id = EntityId::from(entity.id.clone());
                Ok((id, entity.into_semantic()?))
            })
            .collect::<Result<_, FormatError>>()?;
        Ok(Document {
            id: DocumentId::from(self.manifest.document.id),
            title: self.manifest.document.title,
            schemas,
            entities,
        })
    }
}

impl SchemaV1 {
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

    fn into_semantic_unordered(self) -> Result<Schema, FormatError> {
        let fields = self
            .fields
            .into_iter()
            .map(|field| {
                let id = FieldId::from(field.id.clone());
                Ok((id, field.into_semantic()?))
            })
            .collect::<Result<_, FormatError>>()?;
        Ok(Schema {
            id: SchemaId::from(self.id),
            key: SchemaKey::from(self.key),
            fields,
        })
    }
}

impl FieldDefinitionV1 {
    fn into_semantic(self) -> Result<FieldDefinition, FormatError> {
        Ok(FieldDefinition {
            id: FieldId::from(self.id),
            key: FieldKey::from(self.key),
            field_type: self.field_type.into_semantic()?,
            required: self.required,
        })
    }
}

impl FieldTypeV1 {
    fn into_semantic(self) -> Result<FieldType, FormatError> {
        Ok(match self {
            Self::Number => FieldType::Number,
            Self::Text => FieldType::Text,
            Self::Boolean => FieldType::Boolean,
            Self::Reference { schema } => {
                require_id("reference field target schema id", &schema)?;
                FieldType::Reference {
                    schema: SchemaId::from(schema),
                }
            }
        })
    }
}

impl EntityV1 {
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

impl ValueV1 {
    fn into_semantic(self) -> Result<Value, FormatError> {
        Ok(match self {
            Self::Number(number) => Value::Number(number.into_semantic()?),
            Self::Text(text) => Value::Text(text),
            Self::Boolean(boolean) => Value::Boolean(boolean),
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

impl NumberV1 {
    fn into_semantic(self) -> Result<Number, FormatError> {
        Number::new(self.0).map_err(|_| FormatError::InvalidRoProjectRepresentation {
            message: "number must be finite".to_owned(),
        })
    }
}

impl ExpressionV1 {
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

impl BinaryArgumentsV1 {
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

impl ManifestV1 {
    fn from_semantic(document: &Document) -> Self {
        Self {
            format: "tachiko.roproj".to_owned(),
            format_version: ROPROJ_V1_FORMAT_VERSION,
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
        let fields = entity
            .fields
            .iter()
            .map(|(id, value)| (id.to_string(), ValueV1::from_semantic(value)))
            .collect();
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
            Value::Number(number) => Self::Number(NumberV1(number.get())),
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
            Expression::Number(number) => Self::Number(NumberV1(number.get())),
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
    value: NumberV1,
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
            message: format!("'{path}' exceeds .roproj/v1 JSON nesting limit {limit}"),
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
            message: format!("'{path}' does not match .roproj/v1: {error}"),
        }
    })?;
    deserializer
        .end()
        .map_err(|error| FormatError::InvalidRoProjectRepresentation {
            message: format!("'{path}' does not match .roproj/v1: {error}"),
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

#[cfg(any(test, feature = "issue-175-research"))]
mod issue_175_oracle_bridge;

#[cfg(test)]
mod issue_175_research;

#[cfg(feature = "issue-175-research")]
pub use issue_175_oracle_bridge::issue_175_admit_a0_a1;

#[cfg(test)]
mod tests {
    use super::NumberV1;

    #[test]
    fn storage_number_conversion_normalizes_zero_and_rejects_nonfinite() {
        assert_eq!(NumberV1(-0.0).into_semantic().unwrap().to_bits(), 0);
        assert!(NumberV1(f64::INFINITY).into_semantic().is_err());
    }
}
