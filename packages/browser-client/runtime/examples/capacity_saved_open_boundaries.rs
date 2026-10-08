//! Acceptance-only public-native-API boundary fixtures for Work #489 / Sheet #150.
//! UNCOMPILED/UNRUN proposal: retain exact original fixture calculations and
//! outcomes, exercise the new metadata-aware saved-open route for projections.
//! Run only group "projections"; other source groups are retained, not new credit.
use std::{collections::BTreeMap, env};

use serde_json::{Value as Json, json};
use tachiko_designer_runtime::{
    DesignerError, DesignerRequest, DesignerResponse, DesignerRuntime, FieldBatchProjection,
    InteropMetadata, OpenedProjection, ScalarEditInput, TableProjection, inspect_imported_project,
    interop_adapter::{export_csv, export_xlsx},
    open_imported_project, process_wire_request, validate_import_metadata,
};
use tachiko_storage::to_canonical_string;
use tachiko_workspace_engine::{
    Document, Entity, EntityId, EntityKey, Expression, FieldConstraint, FieldDefinition, FieldId,
    FieldKey, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000151";
const MIB: usize = 1024 * 1024;

fn document(rows: usize) -> Document {
    let mut document = Document::empty("d", "capacity");
    let schema = Schema {
        id: SchemaId::from("s"),
        key: SchemaKey::from("grid"),
        fields: ["a", "b", "c"]
            .into_iter()
            .map(|key| {
                let id = FieldId::from(key);
                (
                    id.clone(),
                    FieldDefinition {
                        id,
                        key: FieldKey::from(key),
                        field_type: FieldType::Text,
                        required: true,
                        constraint: FieldConstraint::None,
                    },
                )
            })
            .collect(),
    };
    document.schemas.insert(schema.id.clone(), schema);
    for row in 0..rows {
        let id = EntityId::from(format!("r{row:05}"));
        document.entities.insert(
            id.clone(),
            Entity {
                id,
                key: EntityKey::from(format!("k{row:05}")),
                schema: SchemaId::from("s"),
                fields: ["a", "b", "c"]
                    .into_iter()
                    .map(|key| (FieldId::from(key), Value::Text("x".into())))
                    .collect(),
            },
        );
    }
    document
}

// Independent fixture calculation: repeated identity occurrences count separately.
// D.id + title + each (schema map ID + schema ID + key + each field map ID/ID/key)
// + each (entity map ID + entity ID + key + schema ID + every stored field ID).
fn profile_bytes(document: &Document) -> usize {
    let mut size = document.id.as_str().len() + document.title.len();
    for (id, schema) in &document.schemas {
        size += id.as_str().len() + schema.id.as_str().len() + schema.key.as_str().len();
        for (id, field) in &schema.fields {
            size += id.as_str().len() + field.id.as_str().len() + field.key.as_str().len();
        }
    }
    for (id, entity) in &document.entities {
        size += id.as_str().len()
            + entity.id.as_str().len()
            + entity.key.as_str().len()
            + entity.schema.as_str().len();
        size += entity
            .fields
            .keys()
            .map(|id| id.as_str().len())
            .sum::<usize>();
    }
    size
}

fn first_entity(document: &mut Document) -> &mut Entity {
    document.entities.values_mut().next().unwrap()
}

fn schema(document: &mut Document) -> &mut Schema {
    document.schemas.values_mut().next().unwrap()
}

fn set_text(document: &mut Document, entity: &str, field: &str, value: String) {
    document
        .entities
        .get_mut(&EntityId::from(entity))
        .unwrap()
        .fields
        .insert(FieldId::from(field), Value::Text(value));
}

fn runtime(document: Document) -> DesignerRuntime {
    DesignerRuntime::from_document(document, OCCURRENCE).expect("candidate must be admitted")
}

fn rejected(document: Document, label: &str) {
    tachiko_workspace_engine::validate(&document).expect("negative fixture is semantically valid");
    let error = DesignerRuntime::from_document(document, OCCURRENCE)
        .err()
        .expect("over-limit/ineligible candidate must refuse");
    println!("REFUSAL {label}: {error}");
}

fn table(runtime: &mut DesignerRuntime) -> TableProjection {
    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: "grid".into(),
        })
        .unwrap()
    else {
        panic!("table response expected")
    };
    table
}

fn metadata(document: &Document) -> InteropMetadata {
    serde_json::from_value(json!({"version":1,"sheets":[{
        "schema_id":"s","name":"grid","has_header":true,
        "columns":[{"field_id":"a","name":"a","width":null},
            {"field_id":"b","name":"b","width":null},
            {"field_id":"c","name":"c","width":null}],
        "rows":document.entities.keys().map(|id| json!({"entity_id":id,
            "styles":vec![tachiko_designer_runtime::interop_adapter::CellStyle::default();3]}))
            .collect::<Vec<_>>() }]}))
    .unwrap()
}

fn identities() {
    let mut at = document(129);
    at.id = "d".repeat(256).into();
    runtime(at.clone());
    at.id = "d".repeat(257).into();
    rejected(at, "identity257");
    let mut at = document(8_406);
    for entity in at.entities.values_mut() {
        entity.key = format!("{}{}", entity.key, "k".repeat(250)).into();
    }
    // A field ID appears twice in the schema and once in every stored row.
    // Keep field keys short; IDs never inflate the repeated display addresses.
    let padding = (4 * MIB - profile_bytes(&at)) / (8_406 + 2);
    assert!(padding <= 255);
    let mut field = schema(&mut at).fields.remove(&FieldId::from("a")).unwrap();
    field.id = format!("a{}", "i".repeat(padding)).into();
    let field_id = field.id.clone();
    schema(&mut at).fields.insert(field_id.clone(), field);
    for entity in at.entities.values_mut() {
        let value = entity.fields.remove(&FieldId::from("a")).unwrap();
        entity.fields.insert(field_id.clone(), value);
    }
    // Each extra entity ID byte appears twice; a title byte appears once.
    let mut remaining = 4 * MIB - profile_bytes(&at);
    let mut entities = BTreeMap::new();
    for (_, mut entity) in at.entities {
        let extra = (remaining / 2).min(250);
        entity.id = format!("{}{}", entity.id, "i".repeat(extra)).into();
        remaining -= 2 * extra;
        entities.insert(entity.id.clone(), entity);
    }
    at.entities = entities;
    assert!(remaining < 2);
    at.title.push_str(&"x".repeat(remaining));
    assert_eq!(profile_bytes(&at), 4 * MIB);
    runtime(at.clone());
    at.title.push('x');
    assert_eq!(profile_bytes(&at), 4 * MIB + 1);
    rejected(at, "profile4194305");
    println!("PASS identity256/257 and profile4194304/4194305");
}

fn metadata_limit() {
    for extra in [0, 1] {
        let mut at = document(8_406);
        let initial = serde_json::to_vec(&metadata(&at)).unwrap().len();
        let mut remaining = 3 * MIB + extra - initial;
        at.entities = at
            .entities
            .into_values()
            .map(|mut entity| {
                let padding = remaining.min(250);
                entity.id = format!("{}{}", entity.id, "i".repeat(padding)).into();
                remaining -= padding;
                (entity.id.clone(), entity)
            })
            .collect();
        assert_eq!(remaining, 0);
        assert!(profile_bytes(&at) < 4 * MIB);
        let metadata = metadata(&at);
        assert_eq!(
            serde_json::to_vec(&metadata).unwrap().len(),
            3 * MIB + extra
        );
        let runtime = runtime(at.clone());
        let result = validate_import_metadata(&at, &metadata);
        let exported = runtime.export_workbook(&runtime.observe_occurrence().revision, &metadata);
        if extra == 0 {
            result.unwrap();
            exported.unwrap();
        } else {
            assert!(result.is_err());
            assert!(exported.is_err());
        }
    }
    println!("PASS metadata3145728/3145729");
}

fn opened(document: &Document) -> Result<OpenedProjection, DesignerError> {
    let mut input = b"TWDPROJ2".to_vec();
    input.extend(to_canonical_string(document).unwrap().as_bytes());
    open_imported_project(&mut None, &input, &metadata(document), OCCURRENCE)
}

fn wire_size(opened: &OpenedProjection) -> usize {
    let mut reserved = opened.clone();
    // Reserve both revision strings, so later revisions never overrun admission.
    reserved.bootstrap.revision = "resident/18446744073709551615".into();
    reserved
        .table
        .revision
        .clone_from(&reserved.bootstrap.revision);
    serde_json::to_vec(&json!({"status":"ok","response":{"type":"opened","payload":reserved}}))
        .unwrap()
        .len()
}

fn projection_document(target: usize) -> (Document, OpenedProjection) {
    let mut at = document(8_406);
    for field in schema(&mut at).fields.values_mut() {
        field.key = format!("{}{}", field.key, "f".repeat(255)).into();
    }
    let mut expected = opened(&at).unwrap();
    let mut remaining = target
        .checked_sub(wire_size(&expected))
        .expect("projection headroom");
    // Row key appears once in the row and once in each of its three addresses.
    for (entity, row) in at.entities.values_mut().zip(&mut expected.table.rows) {
        let extra = (remaining / 4).min(250);
        let suffix = "k".repeat(extra);
        entity.key = format!("{}{suffix}", entity.key).into();
        row.key.push_str(&suffix);
        for field in &mut row.fields {
            let (key, column) = field.address.split_once('.').unwrap();
            field.address = format!("{key}{suffix}.{column}");
        }
        remaining -= extra * 4;
    }
    assert!(remaining < 4);
    let first = first_entity(&mut at);
    first
        .fields
        .insert(FieldId::from("a"), Value::Text("x".repeat(1 + remaining)));
    expected.table.rows[0].fields[0].stored =
        Some(tachiko_designer_runtime::StoredValueProjection::Text {
            value: "x".repeat(1 + remaining),
        });
    assert_eq!(wire_size(&expected), target);
    assert!(profile_bytes(&at) < 4 * MIB);
    (at, expected)
}

fn publish(runtime: &mut DesignerRuntime, request: DesignerRequest) {
    let before = runtime.observe_occurrence();
    let DesignerResponse::Published(publication) = runtime.handle(request).unwrap() else {
        panic!("publication expected")
    };
    let after = runtime.observe_occurrence();
    assert_eq!(publication.base_revision, before.revision);
    assert_eq!(publication.resulting_revision, after.revision);
    assert_ne!(after.revision, before.revision);
    assert_eq!(after.scope, before.scope);
}

fn history(runtime: &mut DesignerRuntime, redo: bool) {
    let expected_revision = runtime.observe_occurrence().revision;
    publish(
        runtime,
        if redo {
            DesignerRequest::Redo { expected_revision }
        } else {
            DesignerRequest::Undo { expected_revision }
        },
    );
}

fn export_bytes(runtime: &DesignerRuntime, metadata: &InteropMetadata) -> [Vec<u8>; 3] {
    let revision = runtime.observe_occurrence().revision;
    let workbook = runtime.export_workbook(&revision, metadata).unwrap();
    [
        runtime.export_project(&revision).unwrap().bytes,
        export_csv(&workbook.sheets[0]).unwrap(),
        export_xlsx(&workbook).unwrap(),
    ]
}

fn saved_carriers(document: &Document) -> [Vec<u8>; 2] {
    let mut direct = b"TWDPROJ2".to_vec();
    direct.extend(to_canonical_string(document).unwrap().as_bytes());
    let tree = tachiko_storage::encode_roproj_v1(document).unwrap();
    let mut opaque = b"TWDPROJ1".to_vec();
    opaque.extend(u32::try_from(tree.files().len()).unwrap().to_le_bytes());
    for file in tree.files() {
        opaque.extend(u16::try_from(file.path().len()).unwrap().to_le_bytes());
        opaque.extend(u32::try_from(file.bytes().len()).unwrap().to_le_bytes());
        opaque.extend(file.path().as_bytes());
        opaque.extend(file.bytes());
    }
    [direct, opaque]
}
fn refused_open_preserves(candidate: &Document) {
    for input in saved_carriers(candidate) {
        refused_saved_carrier_preserves(candidate, &input);
    }
}
fn refused_saved_carrier_preserves(candidate: &Document, input: &[u8]) {
    const NEW_OCCURRENCE: &str = "00000000-0000-4000-8000-000000000152";
    let candidate_metadata = metadata(candidate);
    let mut empty = None;
    assert!(
        open_imported_project(&mut empty, &input, &candidate_metadata, NEW_OCCURRENCE).is_err()
    );
    assert!(
        empty.is_none(),
        "refusal must not install into an empty resident"
    );

    let seed = document(2);
    let metadata = metadata(&seed);
    let mut resident = Some(runtime(seed));
    let current = resident.as_mut().unwrap();
    let original_rows = table(current).rows;
    let target = original_rows[0].fields[0].target.clone();
    let mut states = Vec::new();
    for value in ["history-one", "history-two"] {
        publish(
            current,
            DesignerRequest::EditScalar {
                expected_revision: current.observe_occurrence().revision,
                target: target.clone(),
                input: ScalarEditInput::Text {
                    value: value.into(),
                },
            },
        );
        states.push(table(current).rows);
    }
    history(current, false); // One Undo entry and one Redo entry must both remain.
    assert_eq!(table(current).rows, states[0]);
    let before_occurrence = current.observe_occurrence();
    let before_table = table(current);
    let before_exports = export_bytes(current, &metadata);

    assert!(
        open_imported_project(&mut resident, &input, &candidate_metadata, NEW_OCCURRENCE).is_err()
    );
    let current = resident.as_mut().expect("refusal must retain the resident");
    assert_eq!(current.observe_occurrence(), before_occurrence);
    assert_eq!(table(current), before_table); // Includes every cell, identity and revision.
    assert_eq!(export_bytes(current, &metadata), before_exports);
    // First consume the pre-existing Redo, then both Undo entries. Rebuild both
    // states afterward to prove complete history content, not merely stack size.
    for (redo, expected) in [
        (true, &states[1]),
        (false, &states[0]),
        (false, &original_rows),
        (true, &states[0]),
        (true, &states[1]),
    ] {
        history(current, redo);
        assert_eq!(&table(current).rows, expected);
    }
}

fn atomic_control() {
    refused_open_preserves(&document(8_407));
    println!(
        "PASS row-limit refusal preserves None, complete resident, exports and both history stacks"
    );
}

fn projections() {
    let (at, expected) = projection_document(16 * MIB);
    let actual = opened(&at).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(wire_size(&actual), 16 * MIB);
    for input in saved_carriers(&at) {
        let inspected = inspect_imported_project(&input, &metadata(&at))
            .expect("exact-limit metadata-aware inspection must succeed");
        assert_eq!(inspected, expected);
        assert_eq!(wire_size(&inspected), 16 * MIB);
        let opened = open_imported_project(&mut None, &input, &metadata(&at), OCCURRENCE).unwrap();
        assert_eq!(opened, expected);
        assert_eq!(wire_size(&opened), 16 * MIB);
    }
    let native_opened = serde_json::to_vec(&actual).unwrap().len();
    let native_table = serde_json::to_vec(&actual.table).unwrap().len();
    println!(
        "PASS native Opened={native_opened}, Table={native_table}, reserved full Opened reply={}",
        16 * MIB
    );
    let (over, expected_over) = projection_document(16 * MIB + 1);
    assert_eq!(wire_size(&expected_over), 16 * MIB + 1);
    for input in saved_carriers(&over) {
        let error = inspect_imported_project(&input, &metadata(&over))
            .expect_err("one-byte-over metadata-aware inspection must refuse");
        match error {
            DesignerError::ProjectionTooLarge { actual, maximum } => {
                assert_eq!(actual, maximum + 1);
            }
            other => panic!("expected projection-boundary refusal, got {other:?}"),
        }
    }
    // Exercise this exact oversized encoded-open candidate with both empty and
    // populated destinations; an install-then-error implementation must fail.
    refused_open_preserves(&over);
    println!(
        "PASS full Opened reply16777216/16777217; component-at-cap is superseded by wrapper cap"
    );
}

fn field_queries() {
    let mut at = document(129);
    let mut initial = runtime(at.clone());
    let fields = table(&mut initial)
        .rows
        .into_iter()
        .flat_map(|row| row.fields)
        .take(128)
        .collect::<Vec<_>>();
    let revision = initial.observe_occurrence().revision;
    let mut batch = FieldBatchProjection {
        revision: revision.clone(),
        fields,
    };
    let reply = |batch: &FieldBatchProjection| json!({"status":"ok","response":{"type":"fields","payload":batch}});
    let initial_size = serde_json::to_vec(&reply(&batch)).unwrap().len();
    let padding = 65_536 - initial_size;
    let each = padding / batch.fields.len();
    let remainder = padding % batch.fields.len();
    for (index, field) in batch.fields.iter_mut().enumerate() {
        let value = "x".repeat(1 + each + usize::from(index < remainder));
        set_text(
            &mut at,
            &field.target.entity,
            &field.target.field,
            value.clone(),
        );
        field.stored = Some(tachiko_designer_runtime::StoredValueProjection::Text { value });
    }
    let targets = batch
        .fields
        .iter()
        .map(|field| field.target.clone())
        .collect();
    let request = serde_json::to_vec(&DesignerRequest::QueryFields {
        expected_revision: revision,
        fields: targets,
    })
    .unwrap();
    let expected = reply(&batch);
    assert_eq!(serde_json::to_vec(&expected).unwrap().len(), 65_536);
    let mut resident = Some(runtime(at.clone()));
    let bytes = process_wire_request(&mut resident, &request);
    assert_eq!(bytes.len(), 65_536);
    assert_eq!(serde_json::from_slice::<Json>(&bytes).unwrap(), expected);
    let Value::Text(value) = first_entity(&mut at)
        .fields
        .get_mut(&FieldId::from("a"))
        .unwrap()
    else {
        unreachable!()
    };
    value.push('x');
    let mut resident = Some(runtime(at));
    let before = resident.as_ref().unwrap().observe_occurrence();
    let reply: Json =
        serde_json::from_slice(&process_wire_request(&mut resident, &request)).unwrap();
    assert_eq!(reply["status"], "error");
    assert_eq!(resident.as_ref().unwrap().observe_occurrence(), before);
    println!("PASS unchanged ordinary queryFields complete reply65536/65537");
}

fn ineligible() {
    let original = document(129);
    let mut constrained = original.clone();
    schema(&mut constrained)
        .fields
        .values_mut()
        .next()
        .unwrap()
        .constraint = FieldConstraint::TextLiteralSet {
        values: vec!["x".into()],
    };
    rejected(constrained, "constraint");
    let mut multiple = original.clone();
    multiple.schemas.insert(
        SchemaId::from("other"),
        Schema {
            id: SchemaId::from("other"),
            key: SchemaKey::from("other"),
            fields: BTreeMap::new(),
        },
    );
    rejected(multiple, "multiple schemas");
    for formula in [false, true] {
        let mut changed = original.clone();
        schema(&mut changed)
            .fields
            .get_mut(&FieldId::from("a"))
            .unwrap()
            .field_type = FieldType::Number;
        for entity in changed.entities.values_mut() {
            entity
                .fields
                .insert(FieldId::from("a"), Value::Number(Number::new(1.0).unwrap()));
        }
        if formula {
            first_entity(&mut changed).fields.insert(
                FieldId::from("a"),
                Value::Formula(Expression::Number(Number::new(1.0).unwrap())),
            );
        }
        if !formula {
            let mut saved = changed.clone();
            let definition = serde_json::from_value(json!({"id":"calc",
                "orders":{"schema":"s","lookup_key_field":"b","quantity_field":"a"},
                "products":{"schema":"s","key_field":"b","category_field":"c","price_field":"a"}
            }))
            .unwrap();
            saved
                .keyed_grouped_sum_definitions
                .insert("calc".into(), definition);
            rejected(saved, "saved definition plus required non-Text binding");
        }
        rejected(changed, if formula { "formula" } else { "non-Text" });
    }
    println!(
        "PASS large constraint/multiple-schema/non-Text/formula/saved-definition disqualification"
    );
    println!(
        "UNREACHABLE isolated valid saved-definition case: its required Number bindings already disqualify the three-Text shape"
    );
}

fn string_escape_profile() {
    for value in ["left_x0041_right", "_x00aF_", "_x005F_x0041_"] {
        let mut candidate = document(129);
        set_text(&mut candidate, "r00000", "a", value.into());
        refused_open_preserves(&candidate);
        let mut generic = document(2);
        set_text(&mut generic, "r00000", "a", value.into());
        assert_eq!(
            opened(&generic).unwrap().table.rows[0].fields[0].stored,
            Some(tachiko_designer_runtime::StoredValueProjection::Text {
                value: value.into()
            })
        );
    }
    for value in ["_X0041_", "_x041_", "_x00G1_"] {
        let mut candidate = document(129);
        set_text(&mut candidate, "r00000", "a", value.into());
        assert_eq!(
            opened(&candidate).unwrap().table.rows[0].fields[0].stored,
            Some(tachiko_designer_runtime::StoredValueProjection::Text {
                value: value.into()
            })
        );
    }
    println!("PASS ST_Xstring capacity reopen refusal, near-matches and generic preservation");
}

fn main() {
    let group = env::args().nth(1).unwrap_or_else(|| "all".into());
    for (name, check) in [
        ("identities", identities as fn()),
        ("metadata", metadata_limit),
        ("projections", projections),
        ("atomic", atomic_control),
        ("fields", field_queries),
        ("ineligible", ineligible),
        ("strings", string_escape_profile),
    ] {
        if group == "all" || group == name {
            check();
        }
    }
}
