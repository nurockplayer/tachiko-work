//! ADR-0049 presentation entry in portable package v1 (#668).
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use sha2::{Digest, Sha256};
use tachiko_semantic_core::{
    BorderSide, CellAlignment, CellStyle, ComparisonOperator, ConditionalRule, DateFormat,
    Document, Entity, EntityId, Expression, FieldConstraint, FieldDefinition, FieldId, FieldType,
    FooterAggregate, Number, NumberFormat, NumberFormatKind, PresentationStore, RuleId, Schema,
    SchemaId, StyleId, TablePresentation,
};
use tachiko_storage::{
    CanonicalRoProjectV3, FormatError, PortablePackageError, ROPROJ_V3_PRESENTATION_PATH,
    decode_portable_package_v1, encode_portable_package_v1, encode_presentation, encode_roproj_v3,
    encode_roproj_v3_with_presentation, pack_roproj, portable_package_payload_root,
    publish_roproj_v3, read_canonical_roproj_v3, unpack_roproj,
};

const STYLE_FULL: &str = "4a19f7869a552af02c3dc68dc00487d4";
const STYLE_ITALIC: &str = "93c83af0f1468de5e223f716f7ad95f3";
const STYLE_FALSE: &str = "e6973aede98d95414bddea0f99614984";
const PRESENTATION_ENTRY: &str = "payload/presentation.jsonl";
const PAYLOAD_DOMAIN: &[u8] = b"tachiko.portable-package/v1\0tachiko.roproj/v3\0";

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "presentation-package-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn field(id: &str) -> (FieldId, FieldDefinition) {
    (
        FieldId::from(id),
        FieldDefinition {
            id: FieldId::from(id),
            key: id.into(),
            display_name: None,
            number_format: None,
            field_type: FieldType::Number,
            required: false,
            constraint: FieldConstraint::None,
        },
    )
}

fn document() -> Document {
    let schema = Schema {
        field_order: None,
        id: "orders".into(),
        key: "orders".into(),
        display_name: None,
        fields: BTreeMap::from([field("f1"), field("f2")]),
    };
    let mut document = Document::empty("d", "Doc");
    document.schemas.insert(schema.id.clone(), schema);
    document.entities.insert(
        EntityId::from("e1"),
        Entity {
            id: "e1".into(),
            key: "e1".into(),
            schema: "orders".into(),
            fields: BTreeMap::new(),
        },
    );
    document
}

fn full_style() -> CellStyle {
    CellStyle {
        bold: Some(true),
        italic: Some(false),
        fill: Some("#FFEE00".into()),
        text_color: Some("#112233".into()),
        alignment: Some(CellAlignment::Center),
        wrap: Some(true),
        borders: Some(vec![BorderSide::Top, BorderSide::Bottom]),
        number_format: Some(NumberFormat {
            decimals: 2,
            kind: NumberFormatKind::Plain,
            thousands_separator: true,
        }),
        date_format: Some(DateFormat::YmdCjk),
    }
}

fn store() -> PresentationStore {
    PresentationStore {
        styles: BTreeMap::from([
            (StyleId::from(STYLE_FULL), full_style()),
            (
                StyleId::from(STYLE_ITALIC),
                CellStyle {
                    italic: Some(true),
                    ..CellStyle::default()
                },
            ),
            (
                StyleId::from(STYLE_FALSE),
                CellStyle {
                    bold: Some(false),
                    borders: Some(Vec::new()),
                    ..CellStyle::default()
                },
            ),
        ]),
        tables: BTreeMap::from([(
            SchemaId::from("orders"),
            TablePresentation {
                title: Some("Q3 budget".into()),
                footer: BTreeMap::from([(FieldId::from("f1"), FooterAggregate::Sum)]),
                column_styles: BTreeMap::from([(FieldId::from("f2"), StyleId::from(STYLE_FULL))]),
            },
        )]),
        rules: BTreeMap::from([(
            RuleId::from("r1"),
            ConditionalRule {
                schema: SchemaId::from("orders"),
                rank: "a1".into(),
                fields: vec![FieldId::from("f1")],
                condition: Expression::Compare {
                    operator: ComparisonOperator::Less,
                    left: Box::new(Expression::SameRowReference(FieldId::from("f1"))),
                    right: Box::new(Expression::Number(Number::new(0.0).unwrap())),
                },
                style: StyleId::from(STYLE_ITALIC),
            },
        )]),
        cells: BTreeMap::from([(
            (EntityId::from("e1"), FieldId::from("f1")),
            StyleId::from(STYLE_FALSE),
        )]),
    }
}

fn minimal_store() -> PresentationStore {
    PresentationStore {
        styles: BTreeMap::from([(
            StyleId::from(STYLE_ITALIC),
            CellStyle {
                italic: Some(true),
                ..CellStyle::default()
            },
        )]),
        cells: BTreeMap::from([(
            (EntityId::from("e1"), FieldId::from("f1")),
            StyleId::from(STYLE_ITALIC),
        )]),
        ..PresentationStore::default()
    }
}

fn files(tree: &CanonicalRoProjectV3) -> Vec<(String, Vec<u8>)> {
    tree.files()
        .iter()
        .map(|file| (file.path().to_owned(), file.bytes().to_vec()))
        .collect()
}

fn payload_entries(tree: &CanonicalRoProjectV3) -> Vec<(String, Vec<u8>)> {
    tree.files()
        .iter()
        .map(|file| (format!("payload/{}", file.path()), file.bytes().to_vec()))
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn tree_root(files: &[(String, Vec<u8>)]) -> [u8; 32] {
    let mut root = Sha256::new();
    root.update(PAYLOAD_DOMAIN);
    for (path, body) in files {
        let mut leaf = Sha256::new();
        leaf.update(path.as_bytes());
        leaf.update([0]);
        leaf.update(body);
        root.update(leaf.finalize());
    }
    root.finalize().into()
}

fn manifest_body(root: &[u8; 32]) -> Vec<u8> {
    format!(
        "{{\n  \"format\": \"tachiko.portable-package\",\n  \"format_version\": 1,\n  \"payload_format\": \"tachiko.roproj\",\n  \"payload_format_version\": 3,\n  \"payload_root_sha256\": \"{}\"\n}}\n",
        hex(root)
    )
    .into_bytes()
}

fn crc32(source: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for &byte in source {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320_u32 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn build_package(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut archive = Vec::new();
    let mut offsets = Vec::with_capacity(entries.len());
    for (name, body) in entries {
        offsets.push(u32::try_from(archive.len()).unwrap());
        let size = u32::try_from(body.len()).unwrap();
        let crc = crc32(body);
        push_u32(&mut archive, 0x0403_4b50);
        push_u16(&mut archive, 10);
        push_u16(&mut archive, 0x0800);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0x0021);
        push_u32(&mut archive, crc);
        push_u32(&mut archive, size);
        push_u32(&mut archive, size);
        push_u16(&mut archive, u16::try_from(name.len()).unwrap());
        push_u16(&mut archive, 0);
        archive.extend_from_slice(name.as_bytes());
        archive.extend_from_slice(body);
    }
    let central_offset = u32::try_from(archive.len()).unwrap();
    for ((name, body), offset) in entries.iter().zip(offsets) {
        let size = u32::try_from(body.len()).unwrap();
        let crc = crc32(body);
        push_u32(&mut archive, 0x0201_4b50);
        push_u16(&mut archive, 20);
        push_u16(&mut archive, 10);
        push_u16(&mut archive, 0x0800);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0x0021);
        push_u32(&mut archive, crc);
        push_u32(&mut archive, size);
        push_u32(&mut archive, size);
        push_u16(&mut archive, u16::try_from(name.len()).unwrap());
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0);
        push_u32(&mut archive, 0);
        push_u32(&mut archive, offset);
        archive.extend_from_slice(name.as_bytes());
    }
    let central_size = u32::try_from(archive.len()).unwrap() - central_offset;
    let count = u16::try_from(entries.len()).unwrap();
    push_u32(&mut archive, 0x0605_4b50);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, count);
    push_u16(&mut archive, count);
    push_u32(&mut archive, central_size);
    push_u32(&mut archive, central_offset);
    push_u16(&mut archive, 0);
    archive
}

/// Payload entries plus the package.json record, as the canonical package
/// writer orders them. `payload` is `manifest.json`, `schemas.json`, ... .
fn package_entries(payload: &[(String, Vec<u8>)]) -> Vec<(String, Vec<u8>)> {
    let root = tree_root(&payload_tree_files(payload));
    let mut entries = vec![("package.json".to_owned(), manifest_body(&root))];
    entries.extend(payload.iter().cloned());
    entries
}

/// The payload root is keyed by tree paths, not by `payload/` entry names.
fn payload_tree_files(payload: &[(String, Vec<u8>)]) -> Vec<(String, Vec<u8>)> {
    payload
        .iter()
        .map(|(path, body)| (path.trim_start_matches("payload/").to_owned(), body.clone()))
        .collect()
}

fn assert_entry_set_refused(bytes: &[u8]) {
    match decode_portable_package_v1(bytes) {
        Err(FormatError::PortablePackage(PortablePackageError::EntrySetMismatch { .. })) => {}
        other => panic!("expected an entry-set refusal, got {other:?}"),
    }
}

#[test]
fn a_twenty_one_entry_package_round_trips_and_unpacks() {
    let tree = encode_roproj_v3_with_presentation(&document(), &store()).unwrap();
    let bytes = encode_portable_package_v1(&tree).unwrap();
    let end = &bytes[bytes.len() - 22..];
    assert_eq!(u16::from_le_bytes([end[8], end[9]]), 21);
    assert_eq!(u16::from_le_bytes([end[10], end[11]]), 21);
    let verified = decode_portable_package_v1(&bytes).unwrap();
    assert_eq!(files(verified.tree()), files(&tree));

    let temp = Temp::new();
    let source = temp.path("source");
    publish_roproj_v3(&source, &tree).unwrap();
    let package = temp.path("project.package");
    pack_roproj(&source, &package).unwrap();
    let unpacked = temp.path("unpacked");
    unpack_roproj(&package, &unpacked).unwrap();
    assert!(unpacked.join(ROPROJ_V3_PRESENTATION_PATH).is_file());
    assert_eq!(fs::read_dir(&unpacked).unwrap().count(), 5);
    assert_eq!(fs::read_dir(unpacked.join("entities")).unwrap().count(), 16);
    assert_eq!(
        files(&read_canonical_roproj_v3(&unpacked).unwrap()),
        files(&tree)
    );
}

#[test]
fn the_payload_root_changes_when_the_member_bytes_change() {
    let document = document();
    let full = encode_roproj_v3_with_presentation(&document, &store()).unwrap();
    let minimal = encode_roproj_v3_with_presentation(&document, &minimal_store()).unwrap();
    assert_ne!(files(&full), files(&minimal));
    assert_ne!(
        portable_package_payload_root(&full),
        portable_package_payload_root(&minimal)
    );
    // A package without the member keeps the nineteen-leaf root.
    let plain = encode_roproj_v3(&document).unwrap();
    assert_ne!(
        portable_package_payload_root(&plain),
        portable_package_payload_root(&full)
    );
    assert_eq!(
        portable_package_payload_root(&plain),
        tree_root(&files(&plain))
    );
}

#[test]
fn a_package_refuses_a_renamed_member_or_a_member_not_last() {
    let tree = encode_roproj_v3_with_presentation(&document(), &store()).unwrap();
    let payload = payload_entries(&tree);

    let mut renamed = payload.clone();
    renamed.last_mut().unwrap().0 = "payload/presentatioX.jsonl".to_owned();
    assert_entry_set_refused(&build_package(&package_entries(&renamed)));

    let mut moved = payload.clone();
    let last = moved.pop().unwrap();
    let before = moved.pop().unwrap();
    moved.push(last);
    moved.push(before);
    match decode_portable_package_v1(&build_package(&package_entries(&moved))) {
        Err(FormatError::PortablePackage(PortablePackageError::NonCanonicalContainer {
            ..
        })) => {}
        other => panic!("expected a noncanonical-container refusal, got {other:?}"),
    }
}

#[test]
fn a_package_refuses_an_empty_member() {
    let tree = encode_roproj_v3_with_presentation(&document(), &store()).unwrap();
    let mut payload = payload_entries(&tree);
    assert_eq!(payload.last().unwrap().0, PRESENTATION_ENTRY);
    payload.last_mut().unwrap().1 = Vec::new();
    let bytes = build_package(&package_entries(&payload));
    match decode_portable_package_v1(&bytes) {
        Err(FormatError::PortablePackage(PortablePackageError::NonCanonicalPayload { .. })) => {}
        other => panic!("expected a noncanonical-payload refusal, got {other:?}"),
    }
}

#[test]
fn the_member_bytes_reach_the_package_entries() {
    let tree = encode_roproj_v3_with_presentation(&document(), &store()).unwrap();
    let entries = package_entries(&payload_entries(&tree));
    assert_eq!(entries.len(), 21);
    assert_eq!(entries[0].0, "package.json");
    assert_eq!(entries[20].0, PRESENTATION_ENTRY);
    assert_eq!(
        entries[20].1,
        encode_presentation(&store()).unwrap().unwrap()
    );
}
