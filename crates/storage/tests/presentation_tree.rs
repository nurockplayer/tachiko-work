//! ADR-0049 presentation member in the canonical `.roproj/v3` tree (#668).
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use tachiko_semantic_core::{
    BorderSide, CellAlignment, CellStyle, ComparisonOperator, ConditionalRule, DateFormat,
    Document, Entity, EntityId, Expression, FieldConstraint, FieldDefinition, FieldId, FieldType,
    FooterAggregate, Number, NumberFormat, NumberFormatKind, PresentationStore, RuleId, Schema,
    SchemaId, StyleId, TablePresentation,
};
use tachiko_storage::{
    CanonicalRoProjectV3, ROPROJ_V3_PRESENTATION_PATH, decode_roproj_v3_with_presentation,
    encode_presentation, encode_roproj_v3, encode_roproj_v3_with_presentation, publish_roproj_v3,
    read_canonical_roproj_v3,
};

const STYLE_FULL: &str = "4a19f7869a552af02c3dc68dc00487d4";
const STYLE_ITALIC: &str = "93c83af0f1468de5e223f716f7ad95f3";
const STYLE_FALSE: &str = "e6973aede98d95414bddea0f99614984";

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "presentation-tree-{}-{}",
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
            date_format: None,
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

fn files(tree: &CanonicalRoProjectV3) -> Vec<(String, Vec<u8>)> {
    tree.files()
        .iter()
        .map(|file| (file.path().to_owned(), file.bytes().to_vec()))
        .collect()
}

fn member() -> Vec<u8> {
    encode_presentation(&store()).unwrap().unwrap()
}

fn refused(files: Vec<(String, Vec<u8>)>) {
    assert!(
        CanonicalRoProjectV3::try_from_files(files).is_err(),
        "expected the tree to be refused"
    );
}

#[test]
fn an_empty_store_keeps_the_nineteen_file_tree() {
    let document = document();
    let plain = encode_roproj_v3(&document).unwrap();
    let with_presentation =
        encode_roproj_v3_with_presentation(&document, &PresentationStore::default()).unwrap();
    assert_eq!(files(&with_presentation), files(&plain));
    assert_eq!(with_presentation.files().len(), 19);
}

#[test]
fn a_store_round_trips_as_the_twentieth_member() {
    let document = document();
    let store = store();
    let tree = encode_roproj_v3_with_presentation(&document, &store).unwrap();
    assert_eq!(tree.files().len(), 20);
    let last = tree.files().last().unwrap();
    assert_eq!(last.path(), ROPROJ_V3_PRESENTATION_PATH);
    assert_eq!(last.bytes(), member());

    let admitted = CanonicalRoProjectV3::try_from_files(files(&tree)).unwrap();
    let (decoded_document, decoded_store) = decode_roproj_v3_with_presentation(&admitted).unwrap();
    assert_eq!(decoded_document, document);
    assert_eq!(decoded_store, store);
    let reencoded = encode_roproj_v3_with_presentation(&decoded_document, &decoded_store).unwrap();
    assert_eq!(files(&reencoded), files(&tree));
}

#[test]
fn admission_refuses_any_other_member_shape() {
    let base = files(&encode_roproj_v3(&document()).unwrap());

    let mut other_name = base.clone();
    other_name.push(("notes.jsonl".into(), member()));
    refused(other_name);

    let mut not_last = base.clone();
    not_last.insert(3, (ROPROJ_V3_PRESENTATION_PATH.into(), member()));
    refused(not_last);

    let mut empty = base.clone();
    empty.push((ROPROJ_V3_PRESENTATION_PATH.into(), Vec::new()));
    refused(empty);

    let mut noncanonical = base.clone();
    let mut bytes = member();
    bytes.push(b'\n');
    noncanonical.push((ROPROJ_V3_PRESENTATION_PATH.into(), bytes));
    refused(noncanonical);

    let mut twenty_one = base.clone();
    twenty_one.push((ROPROJ_V3_PRESENTATION_PATH.into(), member()));
    twenty_one.push((ROPROJ_V3_PRESENTATION_PATH.into(), member()));
    refused(twenty_one);
}

#[test]
fn admission_refuses_a_member_naming_a_missing_entity_field_or_schema() {
    let base = files(&encode_roproj_v3(&document()).unwrap());
    let expected = String::from_utf8(member()).unwrap();
    let variants = [
        expected.replace(
            "\"kind\":\"cell\",\"entity\":\"e1\"",
            "\"kind\":\"cell\",\"entity\":\"e2\"",
        ),
        expected.replace("\"footer\":{\"f1\"", "\"footer\":{\"f9\""),
        expected.replace(
            "\"kind\":\"table\",\"schema\":\"orders\"",
            "\"kind\":\"table\",\"schema\":\"ghost\"",
        ),
    ];
    for variant in variants {
        let mut files = base.clone();
        files.push((ROPROJ_V3_PRESENTATION_PATH.into(), variant.into_bytes()));
        refused(files);
    }
}

#[test]
fn host_reads_and_writes_the_optional_member() {
    let temp = Temp::new();
    let document = document();

    let with = temp.path("with-presentation");
    let tree = encode_roproj_v3_with_presentation(&document, &store()).unwrap();
    publish_roproj_v3(&with, &tree).unwrap();
    assert!(with.join(ROPROJ_V3_PRESENTATION_PATH).is_file());
    let read = read_canonical_roproj_v3(&with).unwrap();
    assert_eq!(files(&read), files(&tree));
    let (read_document, read_store) = decode_roproj_v3_with_presentation(&read).unwrap();
    assert_eq!(read_document, document);
    assert_eq!(read_store, store());

    let without = temp.path("without-presentation");
    let plain = encode_roproj_v3(&document).unwrap();
    publish_roproj_v3(&without, &plain).unwrap();
    let read = read_canonical_roproj_v3(&without).unwrap();
    assert_eq!(read.files().len(), 19);
    assert_eq!(
        decode_roproj_v3_with_presentation(&read).unwrap().1,
        PresentationStore::default()
    );
}

#[test]
fn publishing_without_the_member_after_a_project_had_it_leaves_none() {
    let temp = Temp::new();
    let document = document();
    let destination = temp.path("project");
    let with_presentation = encode_roproj_v3_with_presentation(&document, &store()).unwrap();
    publish_roproj_v3(&destination, &with_presentation).unwrap();

    // The storage publisher refuses an existing destination, so a replacement
    // removes the old tree first and publishes the new one in its place.
    fs::remove_dir_all(&destination).unwrap();
    publish_roproj_v3(&destination, &encode_roproj_v3(&document).unwrap()).unwrap();
    assert!(!destination.join(ROPROJ_V3_PRESENTATION_PATH).exists());
    assert_eq!(
        read_canonical_roproj_v3(&destination)
            .unwrap()
            .files()
            .len(),
        19
    );
}

#[test]
fn an_unknown_extra_root_file_is_refused() {
    let temp = Temp::new();
    let root = temp.path("project");
    let tree = encode_roproj_v3_with_presentation(&document(), &store()).unwrap();
    publish_roproj_v3(&root, &tree).unwrap();
    fs::write(root.join("notes.jsonl"), b"").unwrap();
    assert!(read_canonical_roproj_v3(&root).is_err());
}
