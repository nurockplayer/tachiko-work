//! ADR-0049 stored presentation through the runtime open and save routes.
//!
//! Nothing changes the store during a session, so a delete never moves a
//! record; each save writes a pruned copy.

use std::collections::BTreeMap;

use tachiko_designer_runtime::{
    DesignerRequest, DesignerResponse, DesignerRuntime, OpenedProjection, close_project,
    inspect_project, open_portable_ro, open_project, verify_portable_ro,
};
use tachiko_storage::{
    CanonicalRoProjectV3, ROPROJ_V3_PRESENTATION_PATH, decode_portable_package_v1,
    encode_portable_package_v1, encode_presentation, encode_roproj_v3,
    encode_roproj_v3_with_presentation, style_id,
};
use tachiko_workspace_engine::{
    CellStyle, ConditionalRule, Date, Document, Entity, EntityId, EntityKey, Expression,
    FieldConstraint, FieldDefinition, FieldId, FieldKey, FieldType, FooterAggregate, Number,
    PresentationStore, RuleId, Schema, SchemaId, SchemaKey, StyleId, TablePresentation, Value,
};

const RESIDENT: &str = "00000000-0000-4000-8000-000000000901";
const SECOND: &str = "00000000-0000-4000-8000-000000000902";
const THIRD: &str = "00000000-0000-4000-8000-000000000903";
const FOURTH: &str = "00000000-0000-4000-8000-000000000904";

fn number(value: f64) -> Number {
    Number::new(value).unwrap()
}

fn field(id: &str, field_type: FieldType, required: bool) -> (FieldId, FieldDefinition) {
    (
        FieldId::from(id),
        FieldDefinition {
            id: FieldId::from(id),
            key: FieldKey::from(id),
            display_name: None,
            field_type,
            required,
            constraint: FieldConstraint::None,
            number_format: None,
        },
    )
}

fn entity(id: &str, schema: &str, fields: BTreeMap<FieldId, Value>) -> Entity {
    Entity {
        id: EntityId::from(id),
        key: EntityKey::from(id),
        schema: SchemaId::from(schema),
        fields,
    }
}

fn orders_document() -> Document {
    let schema = Schema {
        field_order: None,
        id: SchemaId::from("orders"),
        key: SchemaKey::from("orders"),
        display_name: None,
        fields: BTreeMap::from([
            field("amount", FieldType::Number, false),
            field("due", FieldType::Date, false),
            field("note", FieldType::Text, false),
        ]),
    };
    let mut document = Document::empty("doc-orders", "Orders");
    document.schemas.insert(schema.id.clone(), schema);
    for id in ["e1", "e2"] {
        document.entities.insert(
            EntityId::from(id),
            entity(
                id,
                "orders",
                BTreeMap::from([
                    (FieldId::from("amount"), Value::Number(number(10.0))),
                    (
                        FieldId::from("due"),
                        Value::Date(Date::parse("2026-01-10").unwrap()),
                    ),
                    (FieldId::from("note"), Value::Text(format!("row {id}"))),
                ]),
            ),
        );
    }
    document
}

/// The bounded stock tracker, which is the document `RemoveRows` accepts.
fn tracker_document() -> Document {
    let schema = Schema {
        field_order: None,
        id: SchemaId::from("tracker"),
        key: SchemaKey::from("tracker"),
        display_name: None,
        fields: BTreeMap::from([
            field("task", FieldType::Text, true),
            field("estimate", FieldType::Number, true),
            field("done", FieldType::Boolean, true),
        ]),
    };
    let mut document = Document::empty("doc-tracker", "Driver Tracker");
    document.schemas.insert(schema.id.clone(), schema);
    for (id, task) in [("e1", "Inspect"), ("e2", "Replace")] {
        document.entities.insert(
            EntityId::from(id),
            entity(
                id,
                "tracker",
                BTreeMap::from([
                    (FieldId::from("task"), Value::Text(task.to_owned())),
                    (FieldId::from("estimate"), Value::Number(number(2.0))),
                    (FieldId::from("done"), Value::Boolean(false)),
                ]),
            ),
        );
    }
    document
}

fn bold_style() -> CellStyle {
    CellStyle {
        bold: Some(true),
        ..CellStyle::default()
    }
}

fn fill_style() -> CellStyle {
    CellStyle {
        fill: Some("#FFEE00".into()),
        ..CellStyle::default()
    }
}

fn style_id_of(style: &CellStyle) -> StyleId {
    style_id(style).unwrap()
}

fn styled_store(
    cell_target: (&str, &str),
    rule_schema: &str,
    rule_field: &str,
) -> PresentationStore {
    let cell_style = style_id_of(&bold_style());
    let rule_style = style_id_of(&fill_style());
    let mut tables = BTreeMap::new();
    tables.insert(
        SchemaId::from(rule_schema),
        TablePresentation {
            title: Some("Q3 budget".into()),
            footer: BTreeMap::from([(FieldId::from(rule_field), FooterAggregate::Sum)]),
            column_styles: BTreeMap::from([(FieldId::from(rule_field), cell_style.clone())]),
        },
    );
    PresentationStore {
        styles: BTreeMap::from([
            (cell_style.clone(), bold_style()),
            (rule_style.clone(), fill_style()),
        ]),
        tables,
        rules: BTreeMap::from([(
            RuleId::from("rule-a1"),
            ConditionalRule {
                schema: SchemaId::from(rule_schema),
                rank: "a1".into(),
                fields: vec![FieldId::from(rule_field)],
                condition: Expression::IsBlank(Box::new(Expression::SameRowReference(
                    FieldId::from(rule_field),
                ))),
                style: rule_style,
            },
        )]),
        cells: BTreeMap::from([(
            (EntityId::from(cell_target.0), FieldId::from(cell_target.1)),
            cell_style,
        )]),
    }
}

fn tracker_store() -> PresentationStore {
    styled_store(("e1", "task"), "tracker", "estimate")
}

fn orders_store() -> PresentationStore {
    styled_store(("e1", "note"), "orders", "due")
}

fn v3_tree(document: &Document, store: &PresentationStore) -> CanonicalRoProjectV3 {
    encode_roproj_v3_with_presentation(document, store).unwrap()
}

fn transfer_files(files: Vec<(String, Vec<u8>)>) -> Vec<u8> {
    let mut bytes = b"TWDPROJ1".to_vec();
    bytes.extend_from_slice(&u32::try_from(files.len()).unwrap().to_le_bytes());
    for (path, body) in files {
        bytes.extend_from_slice(&u16::try_from(path.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(path.as_bytes());
        bytes.extend_from_slice(&body);
    }
    bytes
}

fn transfer(tree: &CanonicalRoProjectV3) -> Vec<u8> {
    transfer_files(
        tree.files()
            .iter()
            .map(|file| (file.path().to_owned(), file.bytes().to_vec()))
            .collect(),
    )
}

fn entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    assert!(bytes.starts_with(b"TWDPROJ1"));
    let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let mut offset = 12;
    let mut files = Vec::with_capacity(count);
    for _ in 0..count {
        let path_length = usize::from(u16::from_le_bytes(
            bytes[offset..offset + 2].try_into().unwrap(),
        ));
        offset += 2;
        let body_length =
            u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;
        let path = String::from_utf8(bytes[offset..offset + path_length].to_vec()).unwrap();
        offset += path_length;
        files.push((path, bytes[offset..offset + body_length].to_vec()));
        offset += body_length;
    }
    assert_eq!(offset, bytes.len());
    files
}

fn member(bytes: &[u8]) -> Option<Vec<u8>> {
    entries(bytes)
        .into_iter()
        .find(|(path, _)| path == ROPROJ_V3_PRESENTATION_PATH)
        .map(|(_, body)| body)
}

fn published(response: DesignerResponse) -> String {
    let DesignerResponse::Published(publication) = response else {
        panic!("expected one published revision");
    };
    publication.resulting_revision
}

fn open(bytes: &[u8], occurrence_id: &str) -> (Option<DesignerRuntime>, OpenedProjection) {
    let mut slot = None;
    let opened = open_project(&mut slot, bytes, occurrence_id).unwrap();
    (slot, opened)
}

#[test]
fn a_project_with_styles_saves_every_v3_route_byte_identically() {
    let document = orders_document();
    let input = v3_tree(&document, &orders_store());
    let input_member = input.file(ROPROJ_V3_PRESENTATION_PATH).unwrap().to_vec();
    let (mut slot, opened) = open(&transfer(&input), RESIDENT);
    assert_eq!(opened.bootstrap.revision, "resident/0");
    let runtime = slot.as_ref().unwrap();

    let v3 = runtime.export_project_v3("resident/0").unwrap();
    assert_eq!(member(&v3.bytes).as_deref(), Some(input_member.as_slice()));
    let canonical = runtime.export_canonical_tree("resident/0").unwrap();
    assert_eq!(
        member(&canonical.bytes).as_deref(),
        Some(input_member.as_slice())
    );
    let portable = runtime.export_portable_ro("resident/0").unwrap();
    let verified = decode_portable_package_v1(&portable.bytes).unwrap();
    assert_eq!(
        verified.tree().file(ROPROJ_V3_PRESENTATION_PATH),
        Some(input_member.as_slice())
    );

    let (mut reopened, _) = open(&v3.bytes, SECOND);
    assert_eq!(
        member(
            &reopened
                .as_ref()
                .unwrap()
                .export_project_v3("resident/0")
                .unwrap()
                .bytes,
        )
        .as_deref(),
        Some(input_member.as_slice())
    );
    let (mut canonical_slot, _) = open(&canonical.bytes, THIRD);
    assert_eq!(
        member(
            &canonical_slot
                .as_ref()
                .unwrap()
                .export_project_v3("resident/0")
                .unwrap()
                .bytes,
        )
        .as_deref(),
        Some(input_member.as_slice())
    );
    let mut portable_slot = None;
    open_portable_ro(&mut portable_slot, &portable.bytes, FOURTH).unwrap();
    assert_eq!(
        member(
            &portable_slot
                .as_ref()
                .unwrap()
                .export_project_v3("resident/0")
                .unwrap()
                .bytes,
        )
        .as_deref(),
        Some(input_member.as_slice())
    );

    close_project(&mut slot);
    close_project(&mut reopened);
    close_project(&mut canonical_slot);
    close_project(&mut portable_slot);
}

#[test]
fn a_project_without_styles_writes_the_nineteen_file_bytes_on_every_route() {
    let document = orders_document();
    let expected = encode_roproj_v3(&document).unwrap();
    let expected_bytes = transfer(&expected);
    let (mut slot, _) = open(&expected_bytes, RESIDENT);
    let runtime = slot.as_ref().unwrap();

    let v3 = runtime.export_project_v3("resident/0").unwrap();
    assert_eq!(v3.bytes, expected_bytes);
    assert_eq!(member(&v3.bytes), None);
    let canonical = runtime.export_canonical_tree("resident/0").unwrap();
    assert_eq!(canonical.bytes, expected_bytes);
    assert_eq!(member(&canonical.bytes), None);
    let portable = runtime.export_portable_ro("resident/0").unwrap();
    let verified = decode_portable_package_v1(&portable.bytes).unwrap();
    assert_eq!(verified.tree(), &expected);
    assert_eq!(verified.tree().file(ROPROJ_V3_PRESENTATION_PATH), None);
    close_project(&mut slot);
}

#[test]
fn a_deleted_row_loses_its_record_and_undo_restores_the_member() {
    let document = tracker_document();
    let input = v3_tree(&document, &tracker_store());
    let input_member = input.file(ROPROJ_V3_PRESENTATION_PATH).unwrap().to_vec();
    let (mut slot, _) = open(&transfer(&input), RESIDENT);
    let runtime = slot.as_mut().unwrap();

    let revision = published(
        runtime
            .handle(DesignerRequest::RemoveRows {
                expected_revision: "resident/0".into(),
                entities: vec!["e1".into()],
            })
            .unwrap(),
    );
    assert_eq!(revision, "resident/1");

    let mut pruned = tracker_store();
    pruned
        .cells
        .remove(&(EntityId::from("e1"), FieldId::from("task")));
    let pruned_member = encode_presentation(&pruned).unwrap().unwrap();
    let removed = runtime.export_project_v3("resident/1").unwrap();
    let removed_member = member(&removed.bytes).unwrap();
    assert_eq!(removed_member, pruned_member);
    assert!(
        !String::from_utf8(removed_member).unwrap().contains("e1"),
        "the deleted row keeps no record"
    );

    let (reopened, _) = open(&removed.bytes, SECOND);
    assert!(reopened.is_some());

    let revision = published(
        runtime
            .handle(DesignerRequest::Undo {
                expected_revision: "resident/1".into(),
            })
            .unwrap(),
    );
    assert_eq!(revision, "resident/2");
    let restored = runtime.export_project_v3("resident/2").unwrap();
    assert_eq!(
        member(&restored.bytes).as_deref(),
        Some(input_member.as_slice())
    );
    close_project(&mut slot);
}

#[test]
fn a_removed_column_drops_its_rule_and_undo_restores_the_member() {
    let document = orders_document();
    let input = v3_tree(&document, &orders_store());
    let input_member = input.file(ROPROJ_V3_PRESENTATION_PATH).unwrap().to_vec();
    let (mut slot, _) = open(&transfer(&input), RESIDENT);
    let runtime = slot.as_mut().unwrap();

    let revision = published(
        runtime
            .handle(DesignerRequest::RemoveColumn {
                expected_revision: "resident/0".into(),
                collection: "orders".into(),
                field: "due".into(),
            })
            .unwrap(),
    );
    assert_eq!(revision, "resident/1");

    let mut pruned = orders_store();
    pruned.rules.clear();
    pruned.styles.remove(&style_id_of(&fill_style()));
    let table = pruned.tables.get_mut(&SchemaId::from("orders")).unwrap();
    table.footer.remove(&FieldId::from("due"));
    table.column_styles.remove(&FieldId::from("due"));
    let pruned_member = encode_presentation(&pruned).unwrap().unwrap();
    let removed = runtime.export_project_v3("resident/1").unwrap();
    let removed_member = member(&removed.bytes).unwrap();
    assert_eq!(removed_member, pruned_member);
    let text = String::from_utf8(removed_member.clone()).unwrap();
    assert!(!text.contains("rule-a1"));
    assert!(!text.contains(style_id_of(&fill_style()).as_str()));

    let (reopened, _) = open(&removed.bytes, SECOND);
    assert!(reopened.is_some());

    let revision = published(
        runtime
            .handle(DesignerRequest::Undo {
                expected_revision: "resident/1".into(),
            })
            .unwrap(),
    );
    assert_eq!(revision, "resident/2");
    let restored = runtime.export_project_v3("resident/2").unwrap();
    assert_eq!(
        member(&restored.bytes).as_deref(),
        Some(input_member.as_slice())
    );
    close_project(&mut slot);
}

#[test]
fn a_store_that_prunes_to_nothing_writes_nineteen_files() {
    let document = orders_document();
    let cell_style = style_id_of(&bold_style());
    let store = PresentationStore {
        styles: BTreeMap::from([(cell_style.clone(), bold_style())]),
        cells: BTreeMap::from([((EntityId::from("e2"), FieldId::from("note")), cell_style)]),
        ..PresentationStore::default()
    };
    let input = v3_tree(&document, &store);
    let (mut slot, _) = open(&transfer(&input), RESIDENT);
    let runtime = slot.as_mut().unwrap();

    let revision = published(
        runtime
            .handle(DesignerRequest::RemoveTableRows {
                expected_revision: "resident/0".into(),
                collection: "orders".into(),
                entities: vec!["e2".into()],
            })
            .unwrap(),
    );
    assert_eq!(revision, "resident/1");
    let removed = runtime.export_project_v3("resident/1").unwrap();
    assert_eq!(entries(&removed.bytes).len(), 19);
    assert_eq!(member(&removed.bytes), None);
    let (reopened, _) = open(&removed.bytes, SECOND);
    assert!(reopened.is_some());
    close_project(&mut slot);
}

#[test]
fn export_project_refuses_a_store_and_leaves_the_occurrence_unchanged() {
    let document = orders_document();
    let input = v3_tree(&document, &orders_store());
    let (mut slot, _) = open(&transfer(&input), RESIDENT);
    let runtime = slot.as_ref().unwrap();
    let before = runtime.export_project_v3("resident/0").unwrap();
    let error = runtime.export_project("resident/0").unwrap_err();
    assert_eq!(
        error.failure_projection("resident/0").code,
        "legacy_export_unsupported"
    );
    let after = runtime.export_project_v3("resident/0").unwrap();
    assert_eq!(before.bytes, after.bytes);
    close_project(&mut slot);
}

#[test]
fn a_twenty_file_project_is_accepted_and_a_dangling_member_is_refused() {
    let document = orders_document();
    let input = v3_tree(&document, &orders_store());
    let bytes = transfer(&input);
    inspect_project(&bytes).unwrap();
    let portable = encode_portable_package_v1(&input).unwrap();
    verify_portable_ro(&portable).unwrap();

    // The writer refuses a dangling store, so append one to a canonical tree.
    let mut files = entries(&transfer(&encode_roproj_v3(&document).unwrap()));
    let cell_style = style_id_of(&bold_style());
    let dangling = PresentationStore {
        styles: BTreeMap::from([(cell_style.clone(), bold_style())]),
        cells: BTreeMap::from([(
            (EntityId::from("missing"), FieldId::from("note")),
            cell_style,
        )]),
        ..PresentationStore::default()
    };
    files.push((
        ROPROJ_V3_PRESENTATION_PATH.to_owned(),
        encode_presentation(&dangling).unwrap().unwrap(),
    ));
    let dangling_bytes = transfer_files(files);
    let mut slot = None;
    let error = open_project(&mut slot, &dangling_bytes, RESIDENT).unwrap_err();
    assert!(
        error.to_string().contains("missing entity"),
        "unexpected refusal: {error}"
    );
    assert!(slot.is_none());
}
