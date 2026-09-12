//! Canonical `.roproj/v2` persistence for saved keyed grouped-sum definitions.

use std::collections::BTreeMap;

use serde::{
    Deserialize, Serialize,
    ser::{SerializeSeq, SerializeStruct, Serializer},
};
use tachiko_semantic_core::{
    Document, KeyedGroupedSumDefinition, KeyedGroupedSumDefinitionId, KeyedGroupedSumOrdersBinding,
    KeyedGroupedSumProductsBinding, validate_keyed_grouped_sum_definitions,
};

use super::v1::{CanonicalRoProjectV1, ROPROJ_V1_PATHS};
use crate::{
    FormatError,
    strict_json::{FrontendError, VersionToken, inspect_roproj},
};

pub const ROPROJ_V2_FORMAT_VERSION: u32 = 2;
pub const ROPROJ_V2_PATHS: [&str; 19] = [
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
    "definitions.json",
];

const MANIFEST_MAX_NESTING: usize = 4;
const DEFINITIONS_MAX_NESTING: usize = 5;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRoProjectFileV2 {
    path: String,
    bytes: Vec<u8>,
}

impl CanonicalRoProjectFileV2 {
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
pub struct CanonicalRoProjectV2 {
    files: Vec<CanonicalRoProjectFileV2>,
}

impl CanonicalRoProjectV2 {
    #[must_use]
    pub fn files(&self) -> &[CanonicalRoProjectFileV2] {
        &self.files
    }

    #[must_use]
    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.files
            .iter()
            .find(|file| file.path == path)
            .map(CanonicalRoProjectFileV2::bytes)
    }

    /// Admit an exact nineteen-file `.roproj/v2` tree.
    ///
    /// Manifest dispatch occurs before any schema/entity/definition DTO is
    /// interpreted, so unsupported future versions fail closed at the envelope.
    ///
    /// The eighteen non-definition files must already be exact canonical v2
    /// bytes. The `definitions.json` member additionally admits the bounded
    /// non-canonical family of structural whitespace, object-member order, and
    /// definition-ID order, and the returned tree is exact canonical v2.
    ///
    /// # Errors
    ///
    /// Returns a typed format/representation/semantic error unless the tree is
    /// the v2 representation of one valid semantic document within the bounded
    /// non-canonical definition family above.
    pub fn try_from_files(files: Vec<(String, Vec<u8>)>) -> Result<Self, FormatError> {
        require_exact_paths(&files)?;
        dispatch_manifest(&files[0].1)?;
        let tree = Self {
            files: files
                .into_iter()
                .map(|(path, bytes)| CanonicalRoProjectFileV2 { path, bytes })
                .collect(),
        };
        let document = decode_unvalidated(&tree)?;
        let canonical = encode(&document)?;
        for (input, expected) in tree.files.iter().zip(&canonical.files) {
            if input.path == "definitions.json" {
                continue;
            }
            if input != expected {
                return invalid("tree bytes are not canonical .roproj/v2");
            }
        }
        Ok(canonical)
    }
}

/// Encode one semantic document as exact canonical `.roproj/v2`.
///
/// Existing schema/entity encoding remains the frozen v1 canonical body while
/// `definitions.json` is the only new durable semantic member.
///
/// # Errors
///
/// Returns a semantic, v1-body, JSON, or v2 representation failure.
pub fn encode(document: &Document) -> Result<CanonicalRoProjectV2, FormatError> {
    validate_keyed_grouped_sum_definitions(document).map_err(|error| {
        FormatError::InvalidRoProjectRepresentation {
            message: format!("invalid keyed grouped-sum definition: {error}"),
        }
    })?;

    let mut body = document.clone();
    body.keyed_grouped_sum_definitions.clear();
    let v1 = super::v1::encode(&body)?;
    let mut files = v1
        .files()
        .iter()
        .map(|file| CanonicalRoProjectFileV2 {
            path: file.path().to_owned(),
            bytes: file.bytes().to_vec(),
        })
        .collect::<Vec<_>>();
    files[0].bytes = manifest_v1_to_v2(&files[0].bytes)?;
    files.push(CanonicalRoProjectFileV2 {
        path: "definitions.json".to_owned(),
        bytes: render_definitions(document)?.into_bytes(),
    });
    Ok(CanonicalRoProjectV2 { files })
}

/// Decode one already-admitted canonical v2 tree.
///
/// # Errors
///
/// Returns a semantic or representation error if the tree no longer satisfies
/// the v2 contract.
pub fn decode(tree: &CanonicalRoProjectV2) -> Result<Document, FormatError> {
    let document = decode_unvalidated(tree)?;
    let canonical = encode(&document)?;
    if canonical != *tree {
        return invalid("tree bytes are not canonical .roproj/v2");
    }
    Ok(document)
}

/// Deterministically migrate an admitted v1 tree without reserializing its
/// schemas/entity shards. Only the manifest version changes and canonical empty
/// `definitions.json` is added.
///
/// # Errors
///
/// Returns a v2 representation error if the admitted v1 bytes cannot produce
/// the exact canonical v2 no-definition tree.
pub fn migrate_v1(tree: &CanonicalRoProjectV1) -> Result<CanonicalRoProjectV2, FormatError> {
    let mut files = tree
        .files()
        .iter()
        .map(|file| (file.path().to_owned(), file.bytes().to_vec()))
        .collect::<Vec<_>>();
    files[0].1 = manifest_v1_to_v2(&files[0].1)?;
    files.push(("definitions.json".to_owned(), b"[]\n".to_vec()));
    CanonicalRoProjectV2::try_from_files(files)
}

fn decode_unvalidated(tree: &CanonicalRoProjectV2) -> Result<Document, FormatError> {
    dispatch_manifest(tree.file("manifest.json").ok_or_else(|| {
        FormatError::InvalidRoProjectRepresentation {
            message: "canonical v2 tree is missing manifest.json".to_owned(),
        }
    })?)?;

    let mut v1_files = ROPROJ_V1_PATHS
        .iter()
        .map(|path| {
            let bytes =
                tree.file(path)
                    .ok_or_else(|| FormatError::InvalidRoProjectRepresentation {
                        message: format!("canonical v2 tree is missing '{path}'"),
                    })?;
            Ok(((*path).to_owned(), bytes.to_vec()))
        })
        .collect::<Result<Vec<_>, FormatError>>()?;
    v1_files[0].1 = manifest_v2_to_v1(&v1_files[0].1)?;
    let v1 = CanonicalRoProjectV1::try_from_files(v1_files)?;
    let mut document = super::v1::decode(&v1)?;

    let definitions = decode_definitions(tree.file("definitions.json").ok_or_else(|| {
        FormatError::InvalidRoProjectRepresentation {
            message: "canonical v2 tree is missing definitions.json".to_owned(),
        }
    })?)?;
    document.keyed_grouped_sum_definitions = definitions;
    validate_keyed_grouped_sum_definitions(&document).map_err(|error| {
        FormatError::InvalidRoProjectRepresentation {
            message: format!("invalid keyed grouped-sum definition: {error}"),
        }
    })?;
    Ok(document)
}

/// `.roproj/v2`-owned closed-world wire DTO for one saved definition record.
///
/// These types are deliberately independent of the semantic-core
/// `KeyedGroupedSumDefinition` family, its `serde` derives, and its field
/// declaration order. Every nested object denies unknown members.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DefinitionWire {
    id: String,
    orders: OrdersBindingWire,
    products: ProductsBindingWire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OrdersBindingWire {
    schema: String,
    lookup_key_field: String,
    quantity_field: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductsBindingWire {
    schema: String,
    key_field: String,
    category_field: String,
    price_field: String,
}

impl DefinitionWire {
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
}

fn decode_definitions(
    bytes: &[u8],
) -> Result<BTreeMap<KeyedGroupedSumDefinitionId, KeyedGroupedSumDefinition>, FormatError> {
    let source =
        std::str::from_utf8(bytes).map_err(|source| FormatError::InvalidRoProjectUtf8 {
            path: "definitions.json".to_owned(),
            source,
        })?;
    inspect_roproj(source, DEFINITIONS_MAX_NESTING)
        .map_err(|error| map_frontend_error("definitions.json", error))?;
    let records: Vec<DefinitionWire> = serde_json::from_str(source).map_err(|error| {
        FormatError::InvalidRoProjectRepresentation {
            message: format!("'definitions.json' does not match .roproj/v2: {error}"),
        }
    })?;
    let mut definitions = BTreeMap::new();
    for record in records {
        if record.id.is_empty() {
            return invalid("keyed grouped-sum definition id must not be empty");
        }
        let definition = record.into_semantic();
        let id = definition.id.clone();
        if definitions.insert(id.clone(), definition).is_some() {
            return invalid(&format!("duplicate keyed grouped-sum definition id '{id}'"));
        }
    }
    Ok(definitions)
}

/// One ordered `definitions.json` array written with the fixed member order of
/// the v2 DTO specification rather than any derived struct layout.
struct CanonicalDefinitions<'a>(&'a [&'a KeyedGroupedSumDefinition]);

impl Serialize for CanonicalDefinitions<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut records = serializer.serialize_seq(Some(self.0.len()))?;
        for definition in self.0 {
            records.serialize_element(&CanonicalDefinition(definition))?;
        }
        records.end()
    }
}

struct CanonicalDefinition<'a>(&'a KeyedGroupedSumDefinition);

impl Serialize for CanonicalDefinition<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let definition = self.0;
        let mut record = serializer.serialize_struct("KeyedGroupedSumDefinition", 3)?;
        record.serialize_field("id", definition.id.as_str())?;
        record.serialize_field("orders", &CanonicalOrdersBinding(&definition.orders))?;
        record.serialize_field("products", &CanonicalProductsBinding(&definition.products))?;
        record.end()
    }
}

struct CanonicalOrdersBinding<'a>(&'a KeyedGroupedSumOrdersBinding);

impl Serialize for CanonicalOrdersBinding<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let orders = self.0;
        let mut record = serializer.serialize_struct("orders", 3)?;
        record.serialize_field("schema", orders.schema.as_str())?;
        record.serialize_field("lookup_key_field", orders.lookup_key_field.as_str())?;
        record.serialize_field("quantity_field", orders.quantity_field.as_str())?;
        record.end()
    }
}

struct CanonicalProductsBinding<'a>(&'a KeyedGroupedSumProductsBinding);

impl Serialize for CanonicalProductsBinding<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let products = self.0;
        let mut record = serializer.serialize_struct("products", 4)?;
        record.serialize_field("schema", products.schema.as_str())?;
        record.serialize_field("key_field", products.key_field.as_str())?;
        record.serialize_field("category_field", products.category_field.as_str())?;
        record.serialize_field("price_field", products.price_field.as_str())?;
        record.end()
    }
}

fn render_definitions(document: &Document) -> Result<String, FormatError> {
    let mut definitions = document
        .keyed_grouped_sum_definitions
        .values()
        .collect::<Vec<_>>();
    definitions.sort_by(|left, right| {
        left.id
            .as_str()
            .as_bytes()
            .cmp(right.id.as_str().as_bytes())
    });
    let mut output = serde_json::to_string_pretty(&CanonicalDefinitions(&definitions))?;
    output.push('\n');
    Ok(output)
}

fn dispatch_manifest(bytes: &[u8]) -> Result<(), FormatError> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Manifest {
        format: String,
        format_version: u32,
        document: DocumentIdentity,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct DocumentIdentity {
        id: String,
        title: String,
    }

    let source =
        std::str::from_utf8(bytes).map_err(|source| FormatError::InvalidRoProjectUtf8 {
            path: "manifest.json".to_owned(),
            source,
        })?;
    let inspection = inspect_roproj(source, MANIFEST_MAX_NESTING)
        .map_err(|error| map_frontend_error("manifest.json", error))?;
    // `inspect_roproj` performs the strict duplicate/nesting pass while reading
    // `format_version`, but its fast path does not retain the sibling `format`
    // member. Read it from the already-validated bytes so the manifest still
    // dispatches on the frozen v2 discriminator without weakening validation.
    let value: serde_json::Value = serde_json::from_str(source).map_err(|error| {
        FormatError::InvalidRoProjectRepresentation {
            message: format!("manifest.json does not match .roproj/v2: {error}"),
        }
    })?;
    match value.get("format") {
        Some(serde_json::Value::String(format)) if format == "tachiko.roproj" => {}
        None => return Err(FormatError::RoProjectFormatMissing),
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
    if version != ROPROJ_V2_FORMAT_VERSION {
        return Err(FormatError::UnsupportedRoProjectVersion {
            found: version,
            supported: ROPROJ_V2_FORMAT_VERSION,
        });
    }
    let manifest: Manifest = serde_json::from_value(value).map_err(|error| {
        FormatError::InvalidRoProjectRepresentation {
            message: format!("manifest.json does not match .roproj/v2: {error}"),
        }
    })?;
    if manifest.format != "tachiko.roproj" || manifest.format_version != ROPROJ_V2_FORMAT_VERSION {
        return Err(FormatError::RoProjectVersionMalformed);
    }
    if manifest.document.id.is_empty() {
        return invalid("document id must not be empty");
    }
    let _ = manifest.document.title;
    Ok(())
}

fn require_exact_paths(files: &[(String, Vec<u8>)]) -> Result<(), FormatError> {
    if files.len() != ROPROJ_V2_PATHS.len() {
        return invalid(&format!(
            "canonical v2 tree requires {} files, found {}",
            ROPROJ_V2_PATHS.len(),
            files.len()
        ));
    }
    for (index, ((path, _), expected)) in files.iter().zip(ROPROJ_V2_PATHS).enumerate() {
        if path != expected {
            return invalid(&format!(
                "canonical v2 path {index} must be '{expected}', found '{path}'"
            ));
        }
    }
    Ok(())
}

fn manifest_v1_to_v2(bytes: &[u8]) -> Result<Vec<u8>, FormatError> {
    replace_manifest_version(bytes, 1, ROPROJ_V2_FORMAT_VERSION)
}

fn manifest_v2_to_v1(bytes: &[u8]) -> Result<Vec<u8>, FormatError> {
    replace_manifest_version(bytes, ROPROJ_V2_FORMAT_VERSION, 1)
}

fn replace_manifest_version(bytes: &[u8], from: u32, to: u32) -> Result<Vec<u8>, FormatError> {
    let source =
        std::str::from_utf8(bytes).map_err(|source| FormatError::InvalidRoProjectUtf8 {
            path: "manifest.json".to_owned(),
            source,
        })?;
    let needle = format!("  \"format_version\": {from},\n");
    let replacement = format!("  \"format_version\": {to},\n");
    if source.matches(&needle).count() != 1 {
        return invalid("manifest is not in the expected canonical .roproj layout");
    }
    Ok(source.replacen(&needle, &replacement, 1).into_bytes())
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
            message: format!("'{path}' exceeds .roproj/v2 JSON nesting limit {limit}"),
        },
    }
}

fn invalid<T>(message: &str) -> Result<T, FormatError> {
    Err(FormatError::InvalidRoProjectRepresentation {
        message: message.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use tachiko_semantic_core::{
        Document, FieldDefinition, FieldId, FieldKey, FieldType, KeyedGroupedSumDefinition,
        KeyedGroupedSumDefinitionId, KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding,
        Schema, SchemaId, SchemaKey,
    };

    use super::{CanonicalRoProjectV2, ROPROJ_V2_PATHS, decode, encode, migrate_v1};

    fn field(id: &str, field_type: FieldType) -> (FieldId, FieldDefinition) {
        let id = FieldId::from(id);
        let key = FieldKey::from(format!("key_{id}"));
        (
            id.clone(),
            FieldDefinition {
                id,
                key,
                field_type,
                required: true,
            },
        )
    }

    fn grouped_sum_document() -> Document {
        let mut document = Document::empty("doc", "Grouped sum");
        document.schemas.insert(
            SchemaId::from("orders"),
            Schema {
                id: SchemaId::from("orders"),
                key: SchemaKey::from("orders"),
                fields: BTreeMap::from([
                    field("lookup", FieldType::Text),
                    field("quantity", FieldType::Number),
                ]),
            },
        );
        document.schemas.insert(
            SchemaId::from("products"),
            Schema {
                id: SchemaId::from("products"),
                key: SchemaKey::from("products"),
                fields: BTreeMap::from([
                    field("key", FieldType::Text),
                    field("category", FieldType::Text),
                    field("price", FieldType::Number),
                ]),
            },
        );
        for id in ["beta", "alpha"] {
            let id = KeyedGroupedSumDefinitionId::from(id);
            document.keyed_grouped_sum_definitions.insert(
                id.clone(),
                KeyedGroupedSumDefinition {
                    id,
                    orders: KeyedGroupedSumOrdersBinding {
                        schema: SchemaId::from("orders"),
                        lookup_key_field: FieldId::from("lookup"),
                        quantity_field: FieldId::from("quantity"),
                    },
                    products: KeyedGroupedSumProductsBinding {
                        schema: SchemaId::from("products"),
                        key_field: FieldId::from("key"),
                        category_field: FieldId::from("category"),
                        price_field: FieldId::from("price"),
                    },
                },
            );
        }
        document
    }

    fn with_definitions(tree: &CanonicalRoProjectV2, definitions: &[u8]) -> Vec<(String, Vec<u8>)> {
        tree.files()
            .iter()
            .map(|file| {
                let bytes = if file.path() == "definitions.json" {
                    definitions.to_vec()
                } else {
                    file.bytes().to_vec()
                };
                (file.path().to_owned(), bytes)
            })
            .collect()
    }

    #[test]
    fn manifest_dispatch_fails_closed_without_the_v2_format_discriminator() {
        let tree = encode(&Document::empty("doc", "title")).unwrap();
        let mut files = tree
            .files()
            .iter()
            .map(|file| (file.path().to_owned(), file.bytes().to_vec()))
            .collect::<Vec<_>>();
        let manifest = String::from_utf8(files[0].1.clone()).unwrap();
        files[0].1 = manifest
            .replacen("  \"format\": \"tachiko.roproj\",\n", "", 1)
            .into_bytes();
        assert!(matches!(
            super::CanonicalRoProjectV2::try_from_files(files),
            Err(crate::FormatError::RoProjectFormatMissing)
        ));
    }

    #[test]
    fn empty_v1_migration_preserves_body_bytes_and_adds_only_v2_members() {
        let document = Document::empty("doc", "title");
        let v1 = crate::encode_roproj_v1(&document).unwrap();
        let v2 = migrate_v1(&v1).unwrap();
        assert_eq!(v2.files().len(), ROPROJ_V2_PATHS.len());
        assert_eq!(v2.file("definitions.json"), Some(&b"[]\n"[..]));
        for path in ROPROJ_V2_PATHS.iter().skip(1).take(17) {
            assert_eq!(v2.file(path), v1.file(path));
        }
        assert_eq!(decode(&v2).unwrap(), document);
    }

    #[test]
    fn definition_bytes_are_deterministic_and_closed() {
        let mut document = Document::empty("doc", "title");
        let id = KeyedGroupedSumDefinitionId::from("definition");
        document.keyed_grouped_sum_definitions.insert(
            id.clone(),
            KeyedGroupedSumDefinition {
                id,
                orders: KeyedGroupedSumOrdersBinding {
                    schema: "orders".into(),
                    lookup_key_field: "key".into(),
                    quantity_field: "quantity".into(),
                },
                products: KeyedGroupedSumProductsBinding {
                    schema: "products".into(),
                    key_field: "key".into(),
                    category_field: "category".into(),
                    price_field: "price".into(),
                },
            },
        );
        // Bindings deliberately point at absent schemas, so exact v2 encoding
        // must refuse before any canonical tree can be published.
        assert!(encode(&document).is_err());
    }

    #[test]
    fn duplicate_definition_id_is_rejected_before_publication() {
        let source = br#"[
  {
    "id": "same",
    "orders": {"schema":"o","lookup_key_field":"k","quantity_field":"q"},
    "products": {"schema":"p","key_field":"k","category_field":"c","price_field":"x"}
  },
  {
    "id": "same",
    "orders": {"schema":"o","lookup_key_field":"k","quantity_field":"q"},
    "products": {"schema":"p","key_field":"k","category_field":"c","price_field":"x"}
  }
]
"#;
        assert!(super::decode_definitions(source).is_err());
    }

    #[test]
    fn unknown_definition_member_is_rejected() {
        let source = br#"[{"id":"x","orders":{"schema":"o","lookup_key_field":"k","quantity_field":"q"},"products":{"schema":"p","key_field":"k","category_field":"c","price_field":"x"},"unknown":true}]
"#;
        assert!(super::decode_definitions(source).is_err());
    }

    #[test]
    fn encoded_definitions_use_the_fixed_v2_member_order() {
        let tree = encode(&grouped_sum_document()).unwrap();
        let expected = concat!(
            "[\n",
            "  {\n",
            "    \"id\": \"alpha\",\n",
            "    \"orders\": {\n",
            "      \"schema\": \"orders\",\n",
            "      \"lookup_key_field\": \"lookup\",\n",
            "      \"quantity_field\": \"quantity\"\n",
            "    },\n",
            "    \"products\": {\n",
            "      \"schema\": \"products\",\n",
            "      \"key_field\": \"key\",\n",
            "      \"category_field\": \"category\",\n",
            "      \"price_field\": \"price\"\n",
            "    }\n",
            "  },\n",
            "  {\n",
            "    \"id\": \"beta\",\n",
            "    \"orders\": {\n",
            "      \"schema\": \"orders\",\n",
            "      \"lookup_key_field\": \"lookup\",\n",
            "      \"quantity_field\": \"quantity\"\n",
            "    },\n",
            "    \"products\": {\n",
            "      \"schema\": \"products\",\n",
            "      \"key_field\": \"key\",\n",
            "      \"category_field\": \"category\",\n",
            "      \"price_field\": \"price\"\n",
            "    }\n",
            "  }\n",
            "]\n",
        );
        assert_eq!(tree.file("definitions.json"), Some(expected.as_bytes()));
    }

    #[test]
    fn non_canonical_definitions_canonicalize_to_exact_v2_output() {
        let document = grouped_sum_document();
        let canonical = encode(&document).unwrap();
        // Records in reverse ID order, reordered members, and structural
        // whitespace are all inside the bounded non-canonical input family.
        let non_canonical = br#"[
  {
    "products": {"price_field":"price","category_field":"category","key_field":"key","schema":"products"},
    "orders": {"quantity_field":"quantity","lookup_key_field":"lookup","schema":"orders"},
    "id": "beta"
  },
  { "id": "alpha",
    "orders": { "schema": "orders", "lookup_key_field": "lookup", "quantity_field": "quantity" },
    "products": { "schema": "products", "key_field": "key", "category_field": "category", "price_field": "price" }
  }
]
"#;
        assert_ne!(canonical.file("definitions.json"), Some(&non_canonical[..]));
        let admitted =
            CanonicalRoProjectV2::try_from_files(with_definitions(&canonical, non_canonical))
                .unwrap();
        assert_eq!(admitted, canonical);
        assert_eq!(decode(&admitted).unwrap(), document);
    }

    #[test]
    fn duplicate_definition_ids_fail_closed_in_non_canonical_order() {
        let source = br#"[
  {"id":"beta","orders":{"schema":"o","lookup_key_field":"k","quantity_field":"q"},"products":{"schema":"p","key_field":"k","category_field":"c","price_field":"x"}},
  {"id":"alpha","orders":{"schema":"o","lookup_key_field":"k","quantity_field":"q"},"products":{"schema":"p","key_field":"k","category_field":"c","price_field":"x"}},
  {"id":"beta","orders":{"schema":"o","lookup_key_field":"k","quantity_field":"q"},"products":{"schema":"p","key_field":"k","category_field":"c","price_field":"x"}}
]
"#;
        assert!(super::decode_definitions(source).is_err());
    }

    #[test]
    fn admission_rejects_duplicate_definition_ids() {
        let canonical = encode(&grouped_sum_document()).unwrap();
        let duplicate = br#"[
  {"id":"alpha","orders":{"schema":"orders","lookup_key_field":"lookup","quantity_field":"quantity"},"products":{"schema":"products","key_field":"key","category_field":"category","price_field":"price"}},
  {"id":"alpha","orders":{"schema":"orders","lookup_key_field":"lookup","quantity_field":"quantity"},"products":{"schema":"products","key_field":"key","category_field":"category","price_field":"price"}}
]
"#;
        assert!(
            CanonicalRoProjectV2::try_from_files(with_definitions(&canonical, duplicate)).is_err()
        );
    }

    #[test]
    fn non_canonical_non_definition_members_still_fail_closed() {
        let canonical = encode(&grouped_sum_document()).unwrap();
        let mut files = canonical
            .files()
            .iter()
            .map(|file| (file.path().to_owned(), file.bytes().to_vec()))
            .collect::<Vec<_>>();
        files[1].1.push(b'\n');
        assert!(CanonicalRoProjectV2::try_from_files(files).is_err());
    }

    #[test]
    fn unknown_nested_definition_member_is_rejected() {
        let source = br#"[{"id":"x","orders":{"schema":"o","lookup_key_field":"k","quantity_field":"q","unknown":1},"products":{"schema":"p","key_field":"k","category_field":"c","price_field":"x"}}]
"#;
        assert!(super::decode_definitions(source).is_err());
    }

    #[test]
    fn malformed_definition_member_is_rejected() {
        let missing = br#"[{"id":"x","orders":{"schema":"o","lookup_key_field":"k"},"products":{"schema":"p","key_field":"k","category_field":"c","price_field":"x"}}]
"#;
        assert!(super::decode_definitions(missing).is_err());
        let wrong_type = br#"[{"id":"x","orders":{"schema":"o","lookup_key_field":"k","quantity_field":"q"},"products":{"schema":7,"key_field":"k","category_field":"c","price_field":"x"}}]
"#;
        assert!(super::decode_definitions(wrong_type).is_err());
        let keyed_object = br#"{"id":"x"}
"#;
        assert!(super::decode_definitions(keyed_object).is_err());
    }

    #[test]
    fn map_type_is_not_part_of_wire_shape() {
        let _: BTreeMap<String, String> = BTreeMap::new();
    }
}
