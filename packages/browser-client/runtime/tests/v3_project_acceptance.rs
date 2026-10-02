//! Steward acceptance preparation for Work #374; no production admission.
#[path = "v3_acceptance_fixtures/mod.rs"]
mod acceptance_fixtures;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::OpenOptions,
    io::{Cursor, Read, Write},
};

use tachiko_designer_runtime::interop_adapter::{ImportOptions, import_csv};
use tachiko_designer_runtime::{
    BootstrapProjection, CollectionSummary, ColumnProjection, DesignerRequest, DesignerResponse,
    DesignerRuntime, FieldProjection, FieldTarget, ImportFieldType, ImportSelection,
    KeyedGroupedSumDefinitionInput, OpenedProjection, RowProjection, ScalarEditInput, ScalarKind,
    StoredValueProjection, TableProjection, close_project, import_workbook, inspect_project,
    open_project,
};
use tachiko_storage::{
    CanonicalRoProjectV1, CanonicalRoProjectV2, CanonicalRoProjectV3, decode_roproj_v1,
    decode_roproj_v2, decode_roproj_v3, encode_roproj_v1, encode_roproj_v2, encode_roproj_v3,
    from_bytes, to_canonical_string,
};
use tachiko_workspace_engine::{
    Date, Document, DocumentId, Entity, EntityId, EntityKey, FieldConstraint, FieldDefinition,
    FieldId, FieldKey, FieldType, KeyedGroupedSumDefinition, KeyedGroupedSumDefinitionId,
    KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding, Number, Schema, SchemaId,
    SchemaKey, Value,
};

const CSV: &[u8] =
    include_bytes!("../../../../acceptance/date-definition-save-v3/fixtures/mixed.csv");
const ORIGINAL: &str = "00000000-0000-4000-8000-000000000001";
const FRESH: &str = "00000000-0000-4000-8000-000000000002";
const SECOND_FRESH: &str = "00000000-0000-4000-8000-000000000003";
const DEFINITION: &str = "acceptance-orders-summary";
const DOCUMENT_ID: &str = "import_00000000-0000-4000-8000-000000000001_0001";
const SCHEMA_ID: &str = "import_00000000-0000-4000-8000-000000000001_0002";
const FIELD_IDS: [&str; 5] = [
    "import_00000000-0000-4000-8000-000000000001_0003",
    "import_00000000-0000-4000-8000-000000000001_0004",
    "import_00000000-0000-4000-8000-000000000001_0005",
    "import_00000000-0000-4000-8000-000000000001_0006",
    "import_00000000-0000-4000-8000-000000000001_0007",
];
const ROW_IDS: [&str; 2] = [
    "import_00000000-0000-4000-8000-000000000001_0008",
    "import_00000000-0000-4000-8000-000000000001_0009",
];

// Decode only the existing private transport framing, then delegate all semantic
// interpretation to version-owned storage readers. This does not select a save
// representation. A future framing change needs an explicit acceptance amendment.
fn saved_document(bytes: &[u8]) -> Document {
    if let Some(payload) = bytes.strip_prefix(b"TWDPROJ2") {
        return from_bytes(payload).expect("strict direct reader must accept saved bytes");
    }
    let mut cursor = Cursor::new(bytes);
    let mut magic = [0; 8];
    cursor.read_exact(&mut magic).unwrap();
    assert_eq!(&magic, b"TWDPROJ1", "unqualified private framing");
    let mut count = [0; 4];
    cursor.read_exact(&mut count).unwrap();
    let count = u32::from_le_bytes(count);
    assert!(count <= 1024);
    let mut files = Vec::new();
    for _ in 0..count {
        let mut path_len = [0; 2];
        let mut body_len = [0; 4];
        cursor.read_exact(&mut path_len).unwrap();
        cursor.read_exact(&mut body_len).unwrap();
        let mut path = vec![0; usize::from(u16::from_le_bytes(path_len))];
        let mut body = vec![0; usize::try_from(u32::from_le_bytes(body_len)).unwrap()];
        cursor.read_exact(&mut path).unwrap();
        cursor.read_exact(&mut body).unwrap();
        files.push((String::from_utf8(path).unwrap(), body));
    }
    assert_eq!(usize::try_from(cursor.position()).unwrap(), bytes.len());
    let manifest = &files
        .iter()
        .find(|(path, _)| path == "manifest.json")
        .unwrap()
        .1;
    let manifest: serde_json::Value = serde_json::from_slice(manifest).unwrap();
    match manifest["format_version"].as_u64().unwrap() {
        1 => decode_roproj_v1(&CanonicalRoProjectV1::try_from_files(files).unwrap()).unwrap(),
        2 => decode_roproj_v2(&CanonicalRoProjectV2::try_from_files(files).unwrap()).unwrap(),
        3 => {
            let paths = files
                .iter()
                .map(|(path, _)| path.as_str())
                .collect::<Vec<_>>();
            let expected = [
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
            assert_eq!(paths, expected, "v3 uses its pinned 19-file path order");
            decode_roproj_v3(&CanonicalRoProjectV3::try_from_files(files).unwrap()).unwrap()
        }
        version => panic!("unqualified storage reader version {version}"),
    }
}

fn opaque_bundle_entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut cursor = Cursor::new(bytes);
    let mut magic = [0; 8];
    cursor.read_exact(&mut magic).unwrap();
    assert_eq!(&magic, b"TWDPROJ1");
    let mut count = [0; 4];
    cursor.read_exact(&mut count).unwrap();
    let count = u32::from_le_bytes(count);
    assert!(count <= 1024);
    let mut entries = Vec::new();
    for _ in 0..count {
        let mut path_len = [0; 2];
        let mut body_len = [0; 4];
        cursor.read_exact(&mut path_len).unwrap();
        cursor.read_exact(&mut body_len).unwrap();
        let mut path = vec![0; usize::from(u16::from_le_bytes(path_len))];
        let mut body = vec![0; usize::try_from(u32::from_le_bytes(body_len)).unwrap()];
        cursor.read_exact(&mut path).unwrap();
        cursor.read_exact(&mut body).unwrap();
        entries.push((String::from_utf8(path).unwrap(), body));
    }
    assert_eq!(usize::try_from(cursor.position()).unwrap(), bytes.len());
    entries
}

fn assert_ordinary_v3_export_refused(runtime: &mut DesignerRuntime) {
    let before = runtime.observe_occurrence();
    let before_facts = facts(runtime);
    let bootstrap = runtime
        .handle(DesignerRequest::Bootstrap {
            occurrence_id: before.scope.clone(),
        })
        .expect("Bootstrap remains available on a v3-origin occurrence");
    assert!(matches!(bootstrap, DesignerResponse::Bootstrap(_)));
    assert_eq!(runtime.observe_occurrence(), before);
    let error = runtime
        .export_project(&before.revision)
        .expect_err("ordinary export cannot downgrade a v3-origin occurrence");
    assert_eq!(
        error.failure_projection(&before.revision).code,
        "unsupported_project"
    );
    assert_eq!(runtime.observe_occurrence(), before);
    assert_eq!(facts(runtime), before_facts);
}

fn saved_format_version(bytes: &[u8]) -> u64 {
    if bytes.starts_with(b"TWDPROJ2") {
        return 2;
    }
    let entries = opaque_bundle_entries(bytes);
    let manifest: serde_json::Value = serde_json::from_slice(
        &entries
            .iter()
            .find(|(path, _)| path == "manifest.json")
            .unwrap()
            .1,
    )
    .unwrap();
    manifest["format_version"].as_u64().unwrap()
}

fn assert_legacy_origin(
    runtime: &mut DesignerRuntime,
    expected: &Document,
    expected_version: Option<u64>,
) {
    let before = runtime.observe_occurrence();
    let before_facts = facts(runtime);
    let result = runtime.export_project(&before.revision);
    if let Some(version) = expected_version {
        let saved = result.expect("supported legacy-origin profile retains ordinary export");
        assert_eq!(saved.revision, before.revision);
        assert_eq!(saved_format_version(&saved.bytes), version);
        assert_eq!(saved_document(&saved.bytes), *expected);
    } else {
        let error =
            result.expect_err("mixed Date+definition legacy profile keeps its existing refusal");
        assert_eq!(
            error.failure_projection(&before.revision).code,
            "invalid_project"
        );
    }
    assert_eq!(runtime.observe_occurrence(), before);
    assert_eq!(facts(runtime), before_facts);
}

fn frame_bundle(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut bytes = b"TWDPROJ1".to_vec();
    bytes.extend_from_slice(&u32::try_from(entries.len()).unwrap().to_le_bytes());
    for (path, body) in entries {
        bytes.extend_from_slice(&u16::try_from(path.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(path.as_bytes());
        bytes.extend_from_slice(body);
    }
    bytes
}

fn unknown_well_formed_version(valid_v3: &[u8]) -> Vec<u8> {
    let mut entries = opaque_bundle_entries(valid_v3);
    let manifest = entries
        .iter_mut()
        .find(|(path, _)| path == "manifest.json")
        .unwrap();
    let mut json: serde_json::Value = serde_json::from_slice(&manifest.1).unwrap();
    json["format_version"] = serde_json::Value::from(99);
    manifest.1 = serde_json::to_vec(&json).unwrap();
    frame_bundle(&entries)
}

fn edit(runtime: &mut DesignerRuntime, entity: &str, field: &str, input: ScalarEditInput) {
    let revision = runtime.observe_occurrence().revision;
    let DesignerResponse::Published(published) = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: revision.clone(),
            target: format!("{entity}.{field}").as_str().into(),
            input,
        })
        .expect("supported input edit must publish")
    else {
        panic!("expected publication")
    };
    assert_eq!(published.base_revision, revision);
    assert_ne!(published.resulting_revision, revision);
}

fn summary(runtime: &mut DesignerRuntime, stationery: f64) -> DesignerResponse {
    summary_for(runtime, DEFINITION, stationery)
}

fn summary_for(
    runtime: &mut DesignerRuntime,
    definition_id: &str,
    stationery: f64,
) -> DesignerResponse {
    let response = runtime
        .handle(DesignerRequest::QueryKeyedGroupedSum {
            definition_id: definition_id.into(),
        })
        .expect("saved definition must remain executable");
    let DesignerResponse::KeyedGroupedSum(result) = &response else {
        panic!("expected summary")
    };
    assert_eq!(result.definition_id, definition_id);
    assert_eq!(result.revision, runtime.observe_occurrence().revision);
    assert!(result.diagnostics.is_empty());
    let groups: BTreeMap<_, _> = result
        .groups
        .iter()
        .map(|g| (g.category.as_str(), g.value))
        .collect();
    assert_eq!(result.groups.len(), 2);
    assert_eq!(
        groups,
        BTreeMap::from([("Paper", 1000.0), ("Stationery", stationery)])
    );
    response
}

fn facts(runtime: &mut DesignerRuntime) -> Vec<DesignerResponse> {
    vec![
        runtime
            .handle(DesignerRequest::Bootstrap {
                occurrence_id: ORIGINAL.into(),
            })
            .unwrap(),
        runtime
            .handle(DesignerRequest::QueryTable {
                collection: "sheet_1".into(),
            })
            .unwrap(),
    ]
}

fn document_id_in_scope(scope: &str, occurrence: &str) -> DocumentId {
    let prefix = format!("designer-occurrence/{occurrence}/");
    let id = scope
        .strip_prefix(&prefix)
        .expect("trusted scope must bind this occurrence");
    assert_eq!(id, DOCUMENT_ID, "fixture identity is pinned before export");
    DOCUMENT_ID.into()
}

// Test-adapter seam only. The pinned base has no selected-v3 producer method;
// this call is intentionally expected to fail compilation as
// NOTRUN_MISSING_METHOD until an admitted candidate exposes an explicit target.
// Replace only this delegation with the candidate's selected-format request.
fn selected_v3_export(
    runtime: &DesignerRuntime,
    expected_revision: &str,
) -> Result<tachiko_designer_runtime::ProjectExport, tachiko_designer_runtime::DesignerError> {
    runtime.export_project_v3(expected_revision)
}

fn capture_candidate_output(profile: &str, bytes: &[u8]) {
    let root = std::env::var("TACHIKO_V3_ACCEPTANCE_CAPTURE_DIR")
        .expect("PREREQUISITE_NOTRUN_CAPTURE_DIR_UNSET: use the fresh-capture runner");
    let candidate_head = option_env!("TACHIKO_V3_ACCEPTANCE_CANDIDATE_HEAD")
        .expect("PREREQUISITE_NOTRUN_CANDIDATE_HEAD_UNSET: compile with the sealed candidate head");
    let run_id = option_env!("TACHIKO_V3_ACCEPTANCE_RUN_ID")
        .expect("PREREQUISITE_NOTRUN_RUN_ID_UNSET: compile with a new capture run ID");
    assert_eq!(
        candidate_head.len(),
        40,
        "capture must name one exact Git commit"
    );
    let root = std::path::PathBuf::from(root);
    let lease: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("capture-lease.json"))
            .expect("PREREQUISITE_NOTRUN_CAPTURE_LEASE_MISSING"),
    )
    .expect("capture lease must be valid JSON");
    assert_eq!(lease["candidate_head"], candidate_head);
    assert_eq!(lease["run_id"], run_id);
    assert_eq!(lease["state"], "native_in_progress");
    let kind = if profile.starts_with("ordinary-") {
        "ordinary_legacy_export"
    } else if profile.starts_with("constrained-") {
        "valid_storage_v3_constraint_input"
    } else if profile.starts_with("boundary-fresh-65537") {
        "fresh_admission_probe"
    } else {
        "selected_v3_export"
    };
    let write_new = |path: &std::path::Path, contents: &[u8]| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .expect("capture outputs must be new files in the exclusive run directory");
        file.write_all(contents)
            .expect("write exact candidate output");
        file.sync_all().expect("flush candidate output");
    };
    write_new(&root.join(format!("{profile}.twd")), bytes);
    write_new(
        &root.join(format!("{profile}.source.json")),
        &serde_json::to_vec_pretty(&serde_json::json!({
            "producer_head": candidate_head,
            "capture_run_id": run_id,
            "kind": kind,
            "document_id": DOCUMENT_ID,
            "schema_id": SCHEMA_ID,
            "field_ids": FIELD_IDS,
            "entity_ids": ROW_IDS,
            "profile": profile,
        }))
        .unwrap(),
    );
}

// IDs come from trusted pre-export import facts (or immutable legacy provenance).
// All semantic content, keys, types, requiredness and constraints are literal
// fixture expectations, never copied from the encoder/decoder being tested.
fn fixture_document(
    id: DocumentId,
    schema: &str,
    fields: &[String],
    rows: &[String],
    date: bool,
) -> Document {
    assert_eq!(fields.len(), 5);
    assert_eq!(rows.len(), 2);
    let schema_id = SchemaId::from(schema);
    let field_types = [
        FieldType::Text,
        FieldType::Text,
        FieldType::Number,
        FieldType::Number,
        if date {
            FieldType::Date
        } else {
            FieldType::Text
        },
    ];
    let schema = Schema {
        id: schema_id.clone(),
        key: SchemaKey::from("sheet_1"),
        fields: fields
            .iter()
            .zip(field_types)
            .enumerate()
            .map(|(index, (id, field_type))| {
                let id = FieldId::from(id.as_str());
                (
                    id.clone(),
                    FieldDefinition {
                        id,
                        key: FieldKey::from(format!("column_{}", index + 1)),
                        field_type,
                        required: false,
                        constraint: FieldConstraint::None,
                    },
                )
            })
            .collect(),
    };
    let mut document = Document::empty(id, "Imported workbook");
    document.schemas.insert(schema_id.clone(), schema);
    for (index, (row, code, category, quantity, due)) in [
        (&rows[0], "PEN", "Stationery", 4.0, "2026-09-28"),
        (&rows[1], "NOTE", "Paper", 5.0, "2026-09-27"),
    ]
    .into_iter()
    .enumerate()
    {
        let id = EntityId::from(row.as_str());
        let values = [
            Value::Text(code.into()),
            Value::Text(category.into()),
            Value::Number(Number::new(quantity).unwrap()),
            Value::Number(Number::new(200.0).unwrap()),
            if date {
                Value::Date(Date::parse(due).unwrap())
            } else {
                Value::Text(due.into())
            },
        ];
        document.entities.insert(
            id.clone(),
            Entity {
                id,
                key: EntityKey::from(format!("sheet_1_row_{}", index + 1)),
                schema: schema_id.clone(),
                fields: fields
                    .iter()
                    .zip(values)
                    .map(|(id, value)| (FieldId::from(id.as_str()), value))
                    .collect(),
            },
        );
    }
    document
}

fn set_first_date(document: &mut Document, date: &str, typed_date: bool) {
    let entity = document.entities.get_mut(ROW_IDS[0]).unwrap();
    entity.fields.insert(
        FieldId::from(FIELD_IDS[4]),
        if typed_date {
            Value::Date(Date::parse(date).unwrap())
        } else {
            Value::Text(date.to_owned())
        },
    );
}

fn set_first_quantity(document: &mut Document, quantity: f64) {
    document
        .entities
        .get_mut(ROW_IDS[0])
        .unwrap()
        .fields
        .insert(
            FieldId::from(FIELD_IDS[2]),
            Value::Number(Number::new(quantity).unwrap()),
        );
}

fn fixture_definition(schema: &str, fields: &[String]) -> KeyedGroupedSumDefinition {
    KeyedGroupedSumDefinition {
        id: KeyedGroupedSumDefinitionId::from(DEFINITION),
        orders: KeyedGroupedSumOrdersBinding {
            schema: schema.into(),
            lookup_key_field: fields[0].as_str().into(),
            quantity_field: fields[2].as_str().into(),
        },
        products: KeyedGroupedSumProductsBinding {
            schema: schema.into(),
            key_field: fields[0].as_str().into(),
            category_field: fields[1].as_str().into(),
            price_field: fields[3].as_str().into(),
        },
    }
}

fn assert_opened_fixture(opened: &OpenedProjection, expected: &Document) {
    let schema = expected.schemas.values().next().unwrap();
    let collection = CollectionSummary {
        id: schema.id.to_string(),
        key: "sheet_1".into(),
        entity_count: 2,
    };
    assert_eq!(opened.bootstrap.title, "Imported workbook");
    assert_eq!(opened.bootstrap.default_collection, "sheet_1");
    assert_eq!(opened.bootstrap.revision, opened.table.revision);
    assert_eq!(
        opened.bootstrap.keyed_grouped_sum_definition_ids,
        expected
            .keyed_grouped_sum_definitions
            .keys()
            .map(|id| id.as_str().to_owned())
            .collect::<Vec<_>>()
    );
    assert_eq!(opened.bootstrap.collections, vec![collection.clone()]);
    assert_eq!(opened.table.collection, collection);
    assert_eq!(opened.table.columns.len(), 5);
    assert_eq!(
        opened
            .table
            .columns
            .iter()
            .map(|column| FieldId::from(column.id.as_str()))
            .collect::<BTreeSet<_>>(),
        schema.fields.keys().cloned().collect::<BTreeSet<_>>()
    );
    for column in &opened.table.columns {
        let field = &schema.fields[column.id.as_str()];
        assert_eq!(column.key, field.key.as_str());
        assert!(
            !field.required,
            "fixture fields are independently specified as optional"
        );
        assert_eq!(field.constraint, FieldConstraint::None);
        assert_eq!(
            column.field_type,
            match field.field_type {
                FieldType::Text => "text",
                FieldType::Number => "number",
                FieldType::Date => "date",
                _ => panic!("fixture has only Text, Number and Date"),
            }
        );
        assert!(column.dropdown_options.is_none());
    }
    assert_eq!(opened.table.rows.len(), 2);
    let actual: BTreeMap<_, _> = opened
        .table
        .rows
        .iter()
        .map(|row| {
            assert_eq!(row.key, expected.entities[row.id.as_str()].key.as_str());
            assert_eq!(row.fields.len(), 5);
            let values: BTreeMap<_, _> = row
                .fields
                .iter()
                .map(|field| {
                    assert_eq!(field.target.entity, row.id);
                    let expected_entity = &expected.entities[row.id.as_str()];
                    assert_eq!(
                        field.address,
                        format!(
                            "{}.{}",
                            expected_entity.key,
                            expected.schemas[expected_entity.schema.as_str()].fields
                                [field.target.field.as_str()]
                            .key
                        )
                    );
                    assert!(field.formula.is_none());
                    assert!(field.calculated.is_none());
                    assert!(field.diagnostics.is_empty());
                    let expected_field = &expected.schemas[expected_entity.schema.as_str()].fields
                        [field.target.field.as_str()];
                    assert_eq!(
                        field.editable_scalar,
                        Some(match expected_field.field_type {
                            FieldType::Text => ScalarKind::Text,
                            FieldType::Number => ScalarKind::Number,
                            FieldType::Date => ScalarKind::Date,
                            _ => panic!("fixture has only Text, Number and Date"),
                        })
                    );
                    let value = match field.stored.as_ref().expect("fixture has no empty values") {
                        StoredValueProjection::Text { value } => Value::Text(value.clone()),
                        StoredValueProjection::Number { value } => {
                            Value::Number(Number::new(*value).unwrap())
                        }
                        StoredValueProjection::Date { value } => Value::Date(*value),
                        _ => panic!("fixture has only Text, Number and Date"),
                    };
                    (FieldId::from(field.target.field.as_str()), value)
                })
                .collect();
            (EntityId::from(row.id.as_str()), values)
        })
        .collect();
    let wanted: BTreeMap<_, _> = expected
        .entities
        .iter()
        .map(|(id, entity)| (id.clone(), entity.fields.clone()))
        .collect();
    assert_eq!(
        actual, wanted,
        "public ingress must project independently specified typed values"
    );
}

fn assert_live_fixture(
    runtime: &mut DesignerRuntime,
    expected: &Document,
    occurrence: &str,
    revision: &str,
) {
    let observed = runtime.observe_occurrence();
    assert_eq!(
        observed.scope,
        format!("designer-occurrence/{occurrence}/{DOCUMENT_ID}")
    );
    assert_eq!(observed.revision, revision);
    let actual = observed_opened(runtime);
    assert_eq!(actual.bootstrap.revision, revision);
    assert_eq!(actual.table.revision, revision);
    assert_opened_fixture(&actual, expected);
}

fn imported_base(date: bool, occurrence: &str) -> (DesignerRuntime, Document, String) {
    assert_eq!(CSV.len(), 100);
    let source = import_csv(CSV, &ImportOptions::default()).expect("fixture must parse as CSV");
    let selection = ImportSelection {
        column_types: vec![vec![
            ImportFieldType::Text,
            ImportFieldType::Text,
            ImportFieldType::Number,
            ImportFieldType::Number,
            if date {
                ImportFieldType::Date
            } else {
                ImportFieldType::Text
            },
        ]],
        extra_columns: vec![vec![]],
    };
    let (runtime, imported) = import_workbook(&source, &selection, occurrence)
        .expect("real selected-type import must be admitted");
    assert_eq!(
        document_id_in_scope(&runtime.observe_occurrence().scope, occurrence).as_str(),
        DOCUMENT_ID
    );
    assert!(imported.ledger.iter().all(|finding| !finding.blocking));
    let sheet = &imported.metadata.sheets[0];
    assert_eq!(sheet.schema_id, SCHEMA_ID);
    assert_eq!(
        sheet
            .columns
            .iter()
            .map(|column| column.field_id.as_str())
            .collect::<Vec<_>>(),
        FIELD_IDS.to_vec()
    );
    assert_eq!(
        sheet
            .rows
            .iter()
            .map(|row| row.entity_id.as_str())
            .collect::<Vec<_>>(),
        ROW_IDS.to_vec()
    );
    assert_eq!(imported.opened.table.collection.key, "sheet_1");
    assert_eq!(
        sheet
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        vec!["code", "category", "quantity", "price", "event_date"]
    );
    let fields = FIELD_IDS
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    let rows = ROW_IDS
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    let mut expected = fixture_document(DOCUMENT_ID.into(), SCHEMA_ID, &fields, &rows, date);
    set_first_date(&mut expected, "2026-09-26", date);
    assert_opened_fixture(&imported.opened, &expected);
    (runtime, expected, ROW_IDS[0].to_owned())
}

#[allow(clippy::too_many_lines)] // One ordered import/edit/publication/currentness acceptance setup.
fn prepared(date: bool, definition: bool) -> (DesignerRuntime, Document, String, String) {
    let (mut runtime, mut expected, entity) = imported_base(date, ORIGINAL);
    let date_field = FIELD_IDS[4];
    let old_revision = runtime.observe_occurrence().revision;
    edit(
        &mut runtime,
        &entity,
        date_field,
        if date {
            ScalarEditInput::Date {
                value: "2026-09-28".into(),
            }
        } else {
            ScalarEditInput::Text {
                value: "2026-09-28".into(),
            }
        },
    );
    set_first_date(&mut expected, "2026-09-28", date);
    let field_ids = FIELD_IDS
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        saved_document(
            &runtime
                .export_project(&runtime.observe_occurrence().revision)
                .expect("pre-definition supported control must export")
                .bytes,
        ),
        expected,
        "pre-definition export must preserve the independent fixture document"
    );
    let input = KeyedGroupedSumDefinitionInput {
        id: DEFINITION.into(),
        orders_schema: SCHEMA_ID.to_owned(),
        order_lookup_key_field: FIELD_IDS[0].to_owned(),
        order_quantity_field: FIELD_IDS[2].to_owned(),
        products_schema: SCHEMA_ID.to_owned(),
        product_key_field: FIELD_IDS[0].to_owned(),
        product_category_field: FIELD_IDS[1].to_owned(),
        product_price_field: FIELD_IDS[3].to_owned(),
    };
    let before = facts(&mut runtime);
    let stale = runtime
        .handle(DesignerRequest::CreateKeyedGroupedSum {
            expected_revision: old_revision.clone(),
            definition: input.clone(),
        })
        .expect_err("stale definition creation must refuse before publication");
    assert_eq!(
        stale
            .failure_projection(&runtime.observe_occurrence().revision)
            .code,
        "stale_revision"
    );
    assert_eq!(facts(&mut runtime), before);
    if definition {
        let base = runtime.observe_occurrence().revision;
        let response = runtime
            .handle(DesignerRequest::CreateKeyedGroupedSum {
                expected_revision: base.clone(),
                definition: input,
            })
            .expect("same-schema definition must publish through real lifecycle");
        let DesignerResponse::KeyedGroupedSumPublished(published) = response else {
            panic!("expected definition publication")
        };
        assert_eq!(published.publication.base_revision, base);
        assert_eq!(
            published.publication.resulting_revision,
            runtime.observe_occurrence().revision
        );
        summary(&mut runtime, 800.0);
        let definition = fixture_definition(SCHEMA_ID, &field_ids);
        expected
            .keyed_grouped_sum_definitions
            .insert(definition.id.clone(), definition);
    }
    let before = facts(&mut runtime);
    for error in [
        runtime.export_project(&old_revision).unwrap_err(),
        runtime.export_canonical_tree(&old_revision).unwrap_err(),
        runtime.export_portable_ro(&old_revision).unwrap_err(),
    ] {
        assert_eq!(
            error
                .failure_projection(&runtime.observe_occurrence().revision)
                .code,
            "stale_revision"
        );
    }
    assert_eq!(facts(&mut runtime), before);
    (runtime, expected, entity, FIELD_IDS[2].to_owned())
}

#[allow(clippy::too_many_lines)] // One finite legacy and v3 origin/history/reopen trace.
fn roundtrip(date: bool, definition: bool) {
    let (mut runtime, mut expected, entity, quantity) = prepared(date, definition);
    let original_occurrence = runtime.observe_occurrence();
    let base_revision = if definition { 2 } else { 1 };
    assert_live_fixture(
        &mut runtime,
        &expected,
        ORIGINAL,
        &format!("resident/{base_revision}"),
    );
    let expected_legacy = match (date, definition) {
        (true, true) => None,
        (true, false) | (false, true) => Some(2),
        (false, false) => Some(1),
    };
    assert_legacy_origin(&mut runtime, &expected, expected_legacy);

    let stale = selected_v3_export(&runtime, "resident/0")
        .expect_err("previous revision must not select a v3 export");
    assert_eq!(
        stale.failure_projection(&original_occurrence.revision).code,
        "stale_revision"
    );
    assert_eq!(runtime.observe_occurrence(), original_occurrence);
    assert_legacy_origin(&mut runtime, &expected, expected_legacy);

    edit(
        &mut runtime,
        &entity,
        &quantity,
        ScalarEditInput::Number { input: "6".into() },
    );
    set_first_quantity(&mut expected, 6.0);
    assert_live_fixture(
        &mut runtime,
        &expected,
        ORIGINAL,
        &format!("resident/{}", base_revision + 1),
    );
    assert_legacy_origin(&mut runtime, &expected, expected_legacy);
    let revision = runtime.observe_occurrence().revision;
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: revision,
        })
        .expect("pre-existing quantity edit can be placed on Redo");
    set_first_quantity(&mut expected, 4.0);
    let legacy_export_revision = base_revision + 2;
    assert_live_fixture(
        &mut runtime,
        &expected,
        ORIGINAL,
        &format!("resident/{legacy_export_revision}"),
    );
    assert_legacy_origin(&mut runtime, &expected, expected_legacy);

    // Successful selected export at U=[Date/Text], R=[quantity] must leave
    // the legacy origin, revision, projection, and both history entries intact.
    let initial_save = selected_v3_export(&runtime, &format!("resident/{legacy_export_revision}"))
        .expect("selected v3 export accepts the mixed semantic fixture");
    assert_eq!(
        initial_save.revision,
        format!("resident/{legacy_export_revision}")
    );
    assert_eq!(saved_document(&initial_save.bytes), expected);
    assert_live_fixture(
        &mut runtime,
        &expected,
        ORIGINAL,
        &format!("resident/{legacy_export_revision}"),
    );
    assert_legacy_origin(&mut runtime, &expected, expected_legacy);
    let legacy_redo = runtime.observe_occurrence().revision;
    runtime
        .handle(DesignerRequest::Redo {
            expected_revision: legacy_redo,
        })
        .expect("successful selected export preserves quantity Redo");
    set_first_quantity(&mut expected, 6.0);
    assert_live_fixture(
        &mut runtime,
        &expected,
        ORIGINAL,
        &format!("resident/{}", legacy_export_revision + 1),
    );
    assert_legacy_origin(&mut runtime, &expected, expected_legacy);
    let legacy_undo = runtime.observe_occurrence().revision;
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: legacy_undo,
        })
        .expect("successful selected export preserves quantity Undo");
    set_first_quantity(&mut expected, 4.0);
    assert_live_fixture(
        &mut runtime,
        &expected,
        ORIGINAL,
        &format!("resident/{}", legacy_export_revision + 2),
    );
    assert_legacy_origin(&mut runtime, &expected, expected_legacy);
    let legacy_undo_a = runtime.observe_occurrence().revision;
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: legacy_undo_a,
        })
        .expect("successful selected export preserves the original Date/Text Undo");
    set_first_date(&mut expected, "2026-09-26", date);
    assert_live_fixture(
        &mut runtime,
        &expected,
        ORIGINAL,
        &format!("resident/{}", legacy_export_revision + 3),
    );
    assert_legacy_origin(&mut runtime, &expected, expected_legacy);
    let legacy_redo_a = runtime.observe_occurrence().revision;
    runtime
        .handle(DesignerRequest::Redo {
            expected_revision: legacy_redo_a,
        })
        .expect("successful selected export preserves the original Date/Text Redo");
    set_first_date(&mut expected, "2026-09-28", date);
    assert_live_fixture(
        &mut runtime,
        &expected,
        ORIGINAL,
        &format!("resident/{}", legacy_export_revision + 4),
    );
    assert_legacy_origin(&mut runtime, &expected, expected_legacy);

    let profile = match (date, definition) {
        (true, true) => "date-definition",
        (true, false) => "date-only",
        (false, true) => "definition-only",
        (false, false) => "neither",
    };
    capture_candidate_output(profile, &initial_save.bytes);
    let mut slot = Some(runtime);
    close_project(&mut slot);
    assert!(
        slot.is_none(),
        "full close clears the original legacy occurrence"
    );
    let opened = open_project(&mut slot, &initial_save.bytes, FRESH)
        .expect("selected output must be admitted after full close");
    assert_eq!(opened.bootstrap.revision, "resident/0");
    assert_opened_fixture(&opened, &expected);
    let reopened = slot.as_mut().unwrap();
    assert_live_fixture(reopened, &expected, FRESH, "resident/0");
    let fresh_occurrence = reopened.observe_occurrence();
    assert_ne!(fresh_occurrence.scope, original_occurrence.scope);
    assert_ordinary_v3_export_refused(reopened);
    let stale = selected_v3_export(reopened, "resident/999")
        .expect_err("stale token is rejected within the current fresh occurrence");
    assert_eq!(
        stale.failure_projection("resident/0").code,
        "stale_revision"
    );
    assert_eq!(reopened.observe_occurrence(), fresh_occurrence);
    assert_ordinary_v3_export_refused(reopened);

    // H begins at b=0 with U/R empty on every fresh open.
    edit(
        reopened,
        &entity,
        FIELD_IDS[4],
        if date {
            ScalarEditInput::Date {
                value: "2026-09-29".into(),
            }
        } else {
            ScalarEditInput::Text {
                value: "2026-09-29".into(),
            }
        },
    );
    set_first_date(&mut expected, "2026-09-29", date);
    assert_live_fixture(reopened, &expected, FRESH, "resident/1");
    assert_ordinary_v3_export_refused(reopened);
    edit(
        reopened,
        &entity,
        &quantity,
        ScalarEditInput::Number { input: "6".into() },
    );
    set_first_quantity(&mut expected, 6.0);
    assert_live_fixture(reopened, &expected, FRESH, "resident/2");
    if definition {
        summary(reopened, 1200.0);
    }
    assert_ordinary_v3_export_refused(reopened);
    let undo = reopened.observe_occurrence().revision;
    reopened
        .handle(DesignerRequest::Undo {
            expected_revision: undo,
        })
        .expect("Undo B establishes the H probe's real Redo entry");
    set_first_quantity(&mut expected, 4.0);
    assert_live_fixture(reopened, &expected, FRESH, "resident/3");
    if definition {
        summary(reopened, 800.0);
    }
    assert_ordinary_v3_export_refused(reopened);

    // X is a successful selected export while U=[A], R=[B].
    let history_save =
        selected_v3_export(reopened, "resident/3").expect("history-state v3 export succeeds");
    assert_eq!(history_save.revision, "resident/3");
    assert_eq!(saved_document(&history_save.bytes), expected);
    assert_live_fixture(reopened, &expected, FRESH, "resident/3");
    assert_ordinary_v3_export_refused(reopened);

    let redo_b = reopened.observe_occurrence().revision;
    reopened
        .handle(DesignerRequest::Redo {
            expected_revision: redo_b,
        })
        .expect("X preserves pre-existing Redo B");
    set_first_quantity(&mut expected, 6.0);
    assert_live_fixture(reopened, &expected, FRESH, "resident/4");
    if definition {
        summary(reopened, 1200.0);
    }
    assert_ordinary_v3_export_refused(reopened);
    let undo_b = reopened.observe_occurrence().revision;
    reopened
        .handle(DesignerRequest::Undo {
            expected_revision: undo_b,
        })
        .expect("X preserves Undo B");
    set_first_quantity(&mut expected, 4.0);
    assert_live_fixture(reopened, &expected, FRESH, "resident/5");
    if definition {
        summary(reopened, 800.0);
    }
    assert_ordinary_v3_export_refused(reopened);
    let undo_a = reopened.observe_occurrence().revision;
    reopened
        .handle(DesignerRequest::Undo {
            expected_revision: undo_a,
        })
        .expect("X preserves the original Date/Text Undo A");
    set_first_date(&mut expected, "2026-09-28", date);
    assert_live_fixture(reopened, &expected, FRESH, "resident/6");
    assert_ordinary_v3_export_refused(reopened);
    let redo_a = reopened.observe_occurrence().revision;
    reopened
        .handle(DesignerRequest::Redo {
            expected_revision: redo_a,
        })
        .expect("X preserves Redo A");
    set_first_date(&mut expected, "2026-09-29", date);
    assert_live_fixture(reopened, &expected, FRESH, "resident/7");
    assert_ordinary_v3_export_refused(reopened);
    let redo_b = reopened.observe_occurrence().revision;
    reopened
        .handle(DesignerRequest::Redo {
            expected_revision: redo_b,
        })
        .expect("X preserves Redo B after restoring A");
    set_first_quantity(&mut expected, 6.0);
    assert_live_fixture(reopened, &expected, FRESH, "resident/8");
    if definition {
        summary(reopened, 1200.0);
    }
    assert_ordinary_v3_export_refused(reopened);

    let final_save = selected_v3_export(reopened, "resident/8")
        .expect("final v3 export retains literal semantic state");
    assert_eq!(final_save.revision, "resident/8");
    assert_eq!(saved_document(&final_save.bytes), expected);
    assert_live_fixture(reopened, &expected, FRESH, "resident/8");
    assert_ordinary_v3_export_refused(reopened);
    close_project(&mut slot);
    assert!(slot.is_none(), "full close clears the v3-origin occurrence");
    let second_opened = open_project(&mut slot, &final_save.bytes, SECOND_FRESH)
        .expect("second full-close occurrence admits the edited v3 output");
    assert_eq!(second_opened.bootstrap.revision, "resident/0");
    assert_opened_fixture(&second_opened, &expected);
    assert_live_fixture(
        slot.as_mut().unwrap(),
        &expected,
        SECOND_FRESH,
        "resident/0",
    );
    assert_ordinary_v3_export_refused(slot.as_mut().unwrap());
    if definition {
        summary(slot.as_mut().unwrap(), 1200.0);
    }
}

#[test]
fn date_and_saved_definition_lossless_reopen() {
    roundtrip(true, true);
}
#[test]
fn date_only_lossless_reopen() {
    roundtrip(true, false);
}
#[test]
fn definition_only_lossless_reopen() {
    roundtrip(false, true);
}
#[test]
fn neither_lossless_reopen() {
    roundtrip(false, false);
}

const FULL_OPENED_PROJECTION_LIMIT: usize = 65_536;
const FULL_OPENED_PROJECTION_LIMIT_PLUS_ONE: usize = 65_537;

fn independent_opened_projection(
    revision: &str,
    definition_ids: &[String],
    date_value: &str,
    quantity: f64,
) -> OpenedProjection {
    let collection = CollectionSummary {
        id: SCHEMA_ID.to_owned(),
        key: "sheet_1".to_owned(),
        entity_count: 2,
    };
    let columns = [
        ("column_1", "text"),
        ("column_2", "text"),
        ("column_3", "number"),
        ("column_4", "number"),
        ("column_5", "date"),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (key, field_type))| ColumnProjection {
        id: FIELD_IDS[index].to_owned(),
        key: key.to_owned(),
        field_type: field_type.to_owned(),
        dropdown_options: None,
    })
    .collect::<Vec<_>>();
    let quantity_value = quantity.to_string();
    let source_rows = [
        (
            ROW_IDS[0],
            "sheet_1_row_1",
            [
                "PEN",
                "Stationery",
                quantity_value.as_str(),
                "200",
                date_value,
            ],
        ),
        (
            ROW_IDS[1],
            "sheet_1_row_2",
            ["NOTE", "Paper", "5", "200", "2026-09-27"],
        ),
    ];
    let rows = source_rows
        .into_iter()
        .map(|(row_id, row_key, values)| RowProjection {
            id: row_id.to_owned(),
            key: row_key.to_owned(),
            fields: values
                .into_iter()
                .enumerate()
                .map(|(index, value)| {
                    let stored = match index {
                        0 | 1 => StoredValueProjection::Text {
                            value: value.to_owned(),
                        },
                        2 | 3 => StoredValueProjection::Number {
                            value: value.parse().unwrap(),
                        },
                        4 => StoredValueProjection::Date {
                            value: Date::parse(value).unwrap(),
                        },
                        _ => unreachable!(),
                    };
                    FieldProjection {
                        target: FieldTarget {
                            entity: row_id.to_owned(),
                            field: FIELD_IDS[index].to_owned(),
                        },
                        address: format!("{row_key}.column_{}", index + 1),
                        editable_scalar: Some(match index {
                            0 | 1 => ScalarKind::Text,
                            2 | 3 => ScalarKind::Number,
                            4 => ScalarKind::Date,
                            _ => unreachable!(),
                        }),
                        stored: Some(stored),
                        formula: None,
                        calculated: None,
                        diagnostics: vec![],
                    }
                })
                .collect(),
        })
        .collect();
    OpenedProjection {
        bootstrap: BootstrapProjection {
            title: "Imported workbook".to_owned(),
            revision: revision.to_owned(),
            default_collection: "sheet_1".to_owned(),
            collections: vec![collection.clone()],
            keyed_grouped_sum_definition_ids: definition_ids.to_vec(),
        },
        table: TableProjection {
            revision: revision.to_owned(),
            tracker_profile: None,
            native_table_profile: None,
            collection,
            columns,
            rows,
        },
    }
}

fn ids_with_exact_projection_size(
    target: usize,
    revision: &str,
    date_value: &str,
    quantity: f64,
) -> Vec<String> {
    let prefixes = (0..16)
        .map(|index| format!("v3-budget-{index:02}-"))
        .collect::<Vec<_>>();
    let mut ids = prefixes[..13]
        .iter()
        .map(|prefix| format!("{prefix}{}", "x".repeat(4_096 - prefix.len())))
        .collect::<Vec<_>>();
    for prefix in &prefixes[13..] {
        ids.push(prefix.clone());
    }
    let base_size = serde_json::to_vec(&independent_opened_projection(
        revision, &ids, date_value, quantity,
    ))
    .unwrap()
    .len();
    assert!(base_size < target);
    let mut remaining = target - base_size;
    for (index, prefix) in prefixes[13..].iter().enumerate() {
        let filler_len = remaining.min(4_096 - prefix.len());
        ids[13 + index].push_str(&"x".repeat(filler_len));
        remaining -= filler_len;
    }
    assert_eq!(remaining, 0, "bounded definition catalogue reaches target");
    assert_eq!(
        serde_json::to_vec(&independent_opened_projection(
            revision, &ids, date_value, quantity
        ))
        .unwrap()
        .len(),
        target
    );
    ids
}

fn install_budget_catalogue(runtime: &mut DesignerRuntime, ids: &[String]) {
    for id in ids {
        let revision = runtime.observe_occurrence().revision;
        let response = runtime
            .handle(DesignerRequest::CreateKeyedGroupedSum {
                expected_revision: revision.clone(),
                definition: KeyedGroupedSumDefinitionInput {
                    id: id.clone(),
                    orders_schema: SCHEMA_ID.to_owned(),
                    order_lookup_key_field: FIELD_IDS[0].to_owned(),
                    order_quantity_field: FIELD_IDS[2].to_owned(),
                    products_schema: SCHEMA_ID.to_owned(),
                    product_key_field: FIELD_IDS[0].to_owned(),
                    product_category_field: FIELD_IDS[1].to_owned(),
                    product_price_field: FIELD_IDS[3].to_owned(),
                },
            })
            .expect("real grouped-sum publication must accept bounded definition IDs");
        let DesignerResponse::KeyedGroupedSumPublished(published) = response else {
            panic!("expected a real keyed-grouped-sum publication");
        };
        assert_eq!(published.publication.base_revision, revision);
        assert_eq!(
            published.publication.resulting_revision,
            runtime.observe_occurrence().revision
        );
    }
}

fn observed_opened(runtime: &mut DesignerRuntime) -> OpenedProjection {
    let DesignerResponse::Bootstrap(bootstrap) = runtime
        .handle(DesignerRequest::Bootstrap {
            occurrence_id: ORIGINAL.to_owned(),
        })
        .unwrap()
    else {
        panic!("expected bootstrap projection");
    };
    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: "sheet_1".to_owned(),
        })
        .unwrap()
    else {
        panic!("expected table projection");
    };
    OpenedProjection { bootstrap, table }
}

fn assert_budget_summaries(runtime: &mut DesignerRuntime, ids: &[String], stationery: f64) {
    for id in ids {
        summary_for(runtime, id, stationery);
    }
}

fn document_with_budget_catalogue(ids: &[String], date_value: &str, quantity: f64) -> Document {
    let fields = FIELD_IDS
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    let rows = ROW_IDS
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    let mut document = fixture_document(DOCUMENT_ID.into(), SCHEMA_ID, &fields, &rows, true);
    set_first_date(&mut document, date_value, true);
    set_first_quantity(&mut document, quantity);
    for id in ids {
        let mut definition = fixture_definition(SCHEMA_ID, &fields);
        definition.id = id.clone().into();
        document
            .keyed_grouped_sum_definitions
            .insert(definition.id.clone(), definition);
    }
    document
}

#[test]
fn exact_full_catalogue_opened_projection_boundary_and_plus_one() {
    for target in [
        FULL_OPENED_PROJECTION_LIMIT,
        FULL_OPENED_PROJECTION_LIMIT_PLUS_ONE,
    ] {
        let (mut runtime, mut expected, entity) = imported_base(true, ORIGINAL);
        assert_live_fixture(&mut runtime, &expected, ORIGINAL, "resident/0");
        let seed = selected_v3_export(&runtime, "resident/0")
            .expect("small imported Date-only project must allow selected v3 export");
        assert_eq!(saved_document(&seed.bytes), expected);
        let mut slot = Some(runtime);
        close_project(&mut slot);
        assert!(
            slot.is_none(),
            "fresh budget origin follows a real full close"
        );
        let opened = open_project(&mut slot, &seed.bytes, FRESH)
            .expect("small selected v3 project must establish the v3-origin marker");
        assert_eq!(opened.bootstrap.revision, "resident/0");
        assert_opened_fixture(&opened, &expected);
        assert_live_fixture(slot.as_mut().unwrap(), &expected, FRESH, "resident/0");
        assert_ordinary_v3_export_refused(slot.as_mut().unwrap());

        let ids = ids_with_exact_projection_size(target, "resident/0", "2026-09-28", 4.0);
        install_budget_catalogue(slot.as_mut().unwrap(), &ids);
        for id in &ids {
            let mut definition = fixture_definition(
                SCHEMA_ID,
                &FIELD_IDS
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect::<Vec<_>>(),
            );
            definition.id = id.clone().into();
            expected
                .keyed_grouped_sum_definitions
                .insert(definition.id.clone(), definition);
        }
        assert_live_fixture(slot.as_mut().unwrap(), &expected, FRESH, "resident/16");
        assert_budget_summaries(slot.as_mut().unwrap(), &ids, 800.0);
        assert_ordinary_v3_export_refused(slot.as_mut().unwrap());

        let runtime = slot.as_mut().unwrap();
        edit(
            runtime,
            &entity,
            FIELD_IDS[4],
            ScalarEditInput::Date {
                value: "2026-09-28".into(),
            },
        );
        set_first_date(&mut expected, "2026-09-28", true);
        assert_live_fixture(runtime, &expected, FRESH, "resident/17");
        assert_budget_summaries(runtime, &ids, 800.0);
        assert_ordinary_v3_export_refused(runtime);
        edit(
            runtime,
            &entity,
            FIELD_IDS[2],
            ScalarEditInput::Number { input: "6".into() },
        );
        set_first_quantity(&mut expected, 6.0);
        assert_live_fixture(runtime, &expected, FRESH, "resident/18");
        assert_budget_summaries(runtime, &ids, 1200.0);
        assert_ordinary_v3_export_refused(runtime);
        let undo = runtime.observe_occurrence().revision;
        runtime
            .handle(DesignerRequest::Undo {
                expected_revision: undo,
            })
            .expect("Undo B leaves pre-existing Redo B for the admission trace");
        set_first_quantity(&mut expected, 4.0);
        assert_live_fixture(runtime, &expected, FRESH, "resident/19");
        assert_budget_summaries(runtime, &ids, 800.0);

        let fresh_expected = independent_opened_projection("resident/0", &ids, "2026-09-28", 4.0);
        let live_expected = independent_opened_projection("resident/19", &ids, "2026-09-28", 4.0);
        assert_eq!(serde_json::to_vec(&fresh_expected).unwrap().len(), target);
        assert_eq!(
            serde_json::to_vec(&live_expected).unwrap().len() - target,
            2,
            "revision-dependent overhead is measured independently from fresh admission"
        );
        assert_eq!(observed_opened(runtime), live_expected);
        assert_ordinary_v3_export_refused(runtime);
        let x_result = selected_v3_export(runtime, "resident/19");
        let successful_capture = if target == FULL_OPENED_PROJECTION_LIMIT {
            let saved = x_result.expect("exact 65,536-byte fresh projection is admitted");
            assert_eq!(saved.revision, "resident/19");
            assert_eq!(saved_document(&saved.bytes), expected);
            assert_live_fixture(runtime, &expected, FRESH, "resident/19");
            assert_ordinary_v3_export_refused(runtime);
            let inspected = inspect_project(&saved.bytes)
                .expect("at-limit selected v3 Inspect admits exact fresh projection");
            assert_eq!(inspected, fresh_expected);
            assert_live_fixture(runtime, &expected, FRESH, "resident/19");
            assert_budget_summaries(runtime, &ids, 800.0);
            assert_ordinary_v3_export_refused(runtime);
            capture_candidate_output("boundary-fresh-65536", &saved.bytes);
            Some(saved.bytes)
        } else {
            let error = x_result.expect_err("65,537-byte fresh projection is rejected");
            assert!(matches!(
                error,
                tachiko_designer_runtime::DesignerError::ProjectionTooLarge {
                    actual: FULL_OPENED_PROJECTION_LIMIT_PLUS_ONE,
                    maximum: FULL_OPENED_PROJECTION_LIMIT,
                }
            ));
            let probe_document = document_with_budget_catalogue(&ids, "2026-09-28", 4.0);
            let tree =
                encode_roproj_v3(&probe_document).expect("probe is valid canonical storage v3");
            let probe = acceptance_fixtures::frame_v3(&tree);
            capture_candidate_output("boundary-fresh-65537-probe", &probe);
            let inspect_error = inspect_project(&probe).expect_err("over-budget Inspect refuses");
            assert!(matches!(
                inspect_error,
                tachiko_designer_runtime::DesignerError::ProjectionTooLarge {
                    actual: FULL_OPENED_PROJECTION_LIMIT_PLUS_ONE,
                    maximum: FULL_OPENED_PROJECTION_LIMIT,
                }
            ));
            assert_live_fixture(runtime, &expected, FRESH, "resident/19");
            assert_budget_summaries(runtime, &ids, 800.0);
            assert_ordinary_v3_export_refused(runtime);
            let open_error = open_project(&mut slot, &probe, SECOND_FRESH)
                .expect_err("over-budget replacement refuses before slot mutation");
            assert!(matches!(
                open_error,
                tachiko_designer_runtime::DesignerError::ProjectionTooLarge {
                    actual: FULL_OPENED_PROJECTION_LIMIT_PLUS_ONE,
                    maximum: FULL_OPENED_PROJECTION_LIMIT,
                }
            ));
            assert_live_fixture(slot.as_mut().unwrap(), &expected, FRESH, "resident/19");
            assert_budget_summaries(slot.as_mut().unwrap(), &ids, 800.0);
            assert_ordinary_v3_export_refused(slot.as_mut().unwrap());
            None
        };

        // H after X: Redo B, Undo B, Undo A, Redo A, Redo B.
        let runtime = slot.as_mut().unwrap();
        let redo_b = runtime.observe_occurrence().revision;
        runtime
            .handle(DesignerRequest::Redo {
                expected_revision: redo_b,
            })
            .expect("X preserves pre-existing Redo B");
        set_first_quantity(&mut expected, 6.0);
        assert_live_fixture(runtime, &expected, FRESH, "resident/20");
        assert_budget_summaries(runtime, &ids, 1200.0);
        assert_ordinary_v3_export_refused(runtime);
        let undo_b = runtime.observe_occurrence().revision;
        runtime
            .handle(DesignerRequest::Undo {
                expected_revision: undo_b,
            })
            .expect("X preserves Undo B");
        set_first_quantity(&mut expected, 4.0);
        assert_live_fixture(runtime, &expected, FRESH, "resident/21");
        assert_budget_summaries(runtime, &ids, 800.0);
        assert_ordinary_v3_export_refused(runtime);
        let undo_a = runtime.observe_occurrence().revision;
        runtime
            .handle(DesignerRequest::Undo {
                expected_revision: undo_a,
            })
            .expect("X preserves original Date Undo A");
        set_first_date(&mut expected, "2026-09-26", true);
        assert_live_fixture(runtime, &expected, FRESH, "resident/22");
        assert_budget_summaries(runtime, &ids, 800.0);
        assert_ordinary_v3_export_refused(runtime);
        let redo_a = runtime.observe_occurrence().revision;
        runtime
            .handle(DesignerRequest::Redo {
                expected_revision: redo_a,
            })
            .expect("X preserves Redo A");
        set_first_date(&mut expected, "2026-09-28", true);
        assert_live_fixture(runtime, &expected, FRESH, "resident/23");
        assert_budget_summaries(runtime, &ids, 800.0);
        assert_ordinary_v3_export_refused(runtime);
        let redo_b = runtime.observe_occurrence().revision;
        runtime
            .handle(DesignerRequest::Redo {
                expected_revision: redo_b,
            })
            .expect("X preserves Redo B after restoring A");
        set_first_quantity(&mut expected, 6.0);
        assert_live_fixture(runtime, &expected, FRESH, "resident/24");
        assert_budget_summaries(runtime, &ids, 1200.0);
        assert_ordinary_v3_export_refused(runtime);

        if let Some(bytes) = successful_capture {
            close_project(&mut slot);
            assert!(slot.is_none(), "at-limit v3 close leaves no resident");
            let reopened = open_project(&mut slot, &bytes, SECOND_FRESH)
                .expect("at-limit selected-v3 bytes reopen after full close");
            assert_eq!(reopened, fresh_expected);
            assert_live_fixture(
                slot.as_mut().unwrap(),
                &expected_with_budget(&ids, "2026-09-28", 4.0),
                SECOND_FRESH,
                "resident/0",
            );
            assert_eq!(
                serde_json::to_vec(&reopened).unwrap().len(),
                FULL_OPENED_PROJECTION_LIMIT
            );
            assert_budget_summaries(slot.as_mut().unwrap(), &ids, 800.0);
            assert_ordinary_v3_export_refused(slot.as_mut().unwrap());
        }
    }
}

fn expected_with_budget(ids: &[String], date_value: &str, quantity: f64) -> Document {
    document_with_budget_catalogue(ids, date_value, quantity)
}

fn replace_v3_file(bytes: &[u8], path: &str, replacement: Vec<u8>) -> Vec<u8> {
    let mut entries = opaque_bundle_entries(bytes);
    entries
        .iter_mut()
        .find(|(entry_path, _)| entry_path == path)
        .unwrap()
        .1 = replacement;
    frame_bundle(&entries)
}

fn invalid_definition_transfers(valid: &[u8]) -> Vec<(&'static str, Vec<u8>)> {
    let original = opaque_bundle_entries(valid)
        .into_iter()
        .find(|(path, _)| path == "definitions.json")
        .unwrap()
        .1;
    acceptance_fixtures::malformed_definition_payloads(std::str::from_utf8(&original).unwrap())
        .into_iter()
        .map(|(name, body)| (name, replace_v3_file(valid, "definitions.json", body)))
        .collect()
}

fn historied_resident(marked: bool, occurrence: &str) -> (Option<DesignerRuntime>, Document) {
    let (mut slot, mut expected) = if marked {
        let (runtime, document, _, _) = prepared(true, true);
        let source = selected_v3_export(&runtime, "resident/2").unwrap();
        let mut empty = None;
        open_project(&mut empty, &source.bytes, occurrence)
            .expect("fresh marked resident opens valid v3");
        (empty, document)
    } else {
        let mut empty = None;
        let opened = open_project(
            &mut empty,
            include_bytes!(
                "../../../../acceptance/date-definition-save-v3/fixtures/legacy/definition-only.twd"
            ),
            occurrence,
        )
        .expect("fresh legacy resident opens immutable definition-only control");
        let fields = FIELD_IDS
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>();
        let rows = ROW_IDS
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>();
        let mut expected = fixture_document(DOCUMENT_ID.into(), SCHEMA_ID, &fields, &rows, false);
        let definition = fixture_definition(SCHEMA_ID, &fields);
        expected
            .keyed_grouped_sum_definitions
            .insert(definition.id.clone(), definition);
        assert_opened_fixture(&opened, &expected);
        (empty, expected)
    };
    let runtime = slot.as_mut().unwrap();
    let expected_opened = observed_opened(runtime);
    assert_eq!(expected_opened.bootstrap.revision, "resident/0");
    assert_opened_fixture(&expected_opened, &expected);
    if marked {
        assert_ordinary_v3_export_refused(runtime);
    } else {
        assert_legacy_origin(runtime, &expected, Some(2));
    }
    edit(
        runtime,
        ROW_IDS[0],
        FIELD_IDS[4],
        if marked {
            ScalarEditInput::Date {
                value: "2026-09-29".into(),
            }
        } else {
            ScalarEditInput::Text {
                value: "2026-09-29".into(),
            }
        },
    );
    set_first_date(&mut expected, "2026-09-29", marked);
    edit(
        runtime,
        ROW_IDS[0],
        FIELD_IDS[2],
        ScalarEditInput::Number { input: "6".into() },
    );
    set_first_quantity(&mut expected, 6.0);
    let undo = runtime.observe_occurrence().revision;
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: undo,
        })
        .expect("establish actual Redo B");
    set_first_quantity(&mut expected, 4.0);
    assert_live_fixture(runtime, &expected, occurrence, "resident/3");
    if marked {
        assert_ordinary_v3_export_refused(runtime);
    } else {
        assert_legacy_origin(runtime, &expected, Some(2));
    }
    (slot, expected)
}

fn replay_history_after_candidate(
    slot: &mut Option<DesignerRuntime>,
    expected: &mut Document,
    occurrence: &str,
    marked: bool,
) {
    for (operation, revision, date_value, quantity) in [
        (true, 4, "2026-09-29", 6.0),
        (false, 5, "2026-09-29", 4.0),
        (false, 6, "2026-09-28", 4.0),
        (true, 7, "2026-09-29", 4.0),
        (true, 8, "2026-09-29", 6.0),
    ] {
        let runtime = slot.as_mut().unwrap();
        let revision_before = runtime.observe_occurrence().revision;
        let response = if operation {
            runtime.handle(DesignerRequest::Redo {
                expected_revision: revision_before,
            })
        } else {
            runtime.handle(DesignerRequest::Undo {
                expected_revision: revision_before,
            })
        };
        response.expect("failed candidate leaves both pre-existing history stacks usable");
        set_first_date(expected, date_value, marked);
        set_first_quantity(expected, quantity);
        assert_live_fixture(
            runtime,
            expected,
            occurrence,
            &format!("resident/{revision}"),
        );
        if expected
            .keyed_grouped_sum_definitions
            .contains_key(&KeyedGroupedSumDefinitionId::from(DEFINITION))
        {
            summary(runtime, if quantity == 4.0 { 800.0 } else { 1200.0 });
        }
        if marked {
            assert_ordinary_v3_export_refused(runtime);
        } else {
            assert_legacy_origin(runtime, expected, Some(2));
        }
    }
}

#[test]
fn version_gate_and_opaque_transfer_malformed_vectors_are_distinct_and_atomic() {
    let (source, _, _, _) = prepared(true, true);
    let valid = selected_v3_export(&source, "resident/2").unwrap().bytes;
    let expected_valid = saved_document(&valid);
    let mut invalid = vec![
        (
            "complete unknown version",
            unknown_well_formed_version(&valid),
            "unsupported_project",
        ),
        ("wrong magic", b"BADMAGIC".to_vec(), "invalid_project"),
        (
            "zero file count",
            b"TWDPROJ1\x00\x00\x00\x00".to_vec(),
            "invalid_project",
        ),
        (
            "truncated count",
            b"TWDPROJ1\x01\x00".to_vec(),
            "invalid_project",
        ),
    ];
    let mut trailing = valid.clone();
    trailing.push(0);
    invalid.push(("trailing byte", trailing, "invalid_project"));
    invalid.push((
        "truncated payload",
        valid[..valid.len() - 1].to_vec(),
        "invalid_project",
    ));
    let mut truncated = b"TWDPROJ1".to_vec();
    truncated.extend_from_slice(&1_u32.to_le_bytes());
    truncated.extend_from_slice(&4_u16.to_le_bytes());
    truncated.extend_from_slice(&20_u32.to_le_bytes());
    truncated.extend_from_slice(b"ma");
    invalid.push(("truncated path and payload", truncated, "invalid_project"));
    let mut entries = opaque_bundle_entries(&valid);
    entries.swap(0, 1);
    invalid.push(("wrong order", frame_bundle(&entries), "invalid_project"));
    let mut entries = opaque_bundle_entries(&valid);
    entries[1].0 = entries[0].0.clone();
    invalid.push(("duplicate path", frame_bundle(&entries), "invalid_project"));
    let mut entries = opaque_bundle_entries(&valid);
    entries.retain(|(path, _)| path != "entities/f.jsonl");
    invalid.push(("missing shard", frame_bundle(&entries), "invalid_project"));
    let mut entries = opaque_bundle_entries(&valid);
    entries.push(("unexpected.txt".into(), b"x".to_vec()));
    invalid.push(("extra path", frame_bundle(&entries), "invalid_project"));
    let mut entries = opaque_bundle_entries(&valid);
    entries[0].0 = "../manifest.json".into();
    invalid.push(("unsafe path", frame_bundle(&entries), "invalid_project"));
    let mut entries = opaque_bundle_entries(&valid);
    entries[0].0 = "/manifest.json".into();
    invalid.push(("absolute path", frame_bundle(&entries), "invalid_project"));
    invalid.push((
        "malformed legacy direct bytes",
        b"TWDPROJ2not-json".to_vec(),
        "invalid_project",
    ));
    let mut entries = opaque_bundle_entries(&valid);
    entries
        .iter_mut()
        .find(|(path, _)| path == "schemas.json")
        .unwrap()
        .1 = b"{broken".to_vec();
    invalid.push((
        "malformed representation",
        frame_bundle(&entries),
        "invalid_project",
    ));
    invalid.extend(
        invalid_definition_transfers(&valid)
            .into_iter()
            .map(|(name, bytes)| (name, bytes, "invalid_project")),
    );

    let title_document = |length: usize| {
        let fields = FIELD_IDS
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>();
        let rows = ROW_IDS
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>();
        let mut document = fixture_document(DOCUMENT_ID.into(), SCHEMA_ID, &fields, &rows, true);
        document.title = "x".repeat(length);
        let definition = fixture_definition(SCHEMA_ID, &fields);
        document
            .keyed_grouped_sum_definitions
            .insert(definition.id.clone(), definition);
        let tree = encode_roproj_v3(&document)
            .expect("title resource fixture remains strict canonical v3");
        (document, acceptance_fixtures::frame_v3(&tree))
    };
    let (title_at_document, title_at) = title_document(4_096);
    let (_, title_over) = title_document(4_097);
    invalid.push((
        "over title resource profile",
        title_over,
        "unsupported_project",
    ));
    let (constrained_text, constrained_number) = (
        acceptance_fixtures::constrained_v3_transfer(1),
        acceptance_fixtures::constrained_v3_transfer(2),
    );
    invalid.push((
        "valid Text constraint capability",
        constrained_text,
        "unsupported_project",
    ));
    invalid.push((
        "valid Number constraint capability",
        constrained_number,
        "unsupported_project",
    ));

    const TRANSFER_MAX: usize = 64 * 1024 * 1024;
    let exact_transfer = vec![0; TRANSFER_MAX];
    let over_transfer = vec![0; TRANSFER_MAX + 1];
    invalid.push((
        "exact transfer boundary malformed input",
        exact_transfer,
        "invalid_project",
    ));
    invalid.push(("over transfer boundary", over_transfer, "project_too_large"));

    let budget_ids = ids_with_exact_projection_size(
        FULL_OPENED_PROJECTION_LIMIT_PLUS_ONE,
        "resident/0",
        "2026-09-28",
        4.0,
    );
    let over_budget = acceptance_fixtures::frame_v3(
        &encode_roproj_v3(&document_with_budget_catalogue(
            &budget_ids,
            "2026-09-28",
            4.0,
        ))
        .unwrap(),
    );
    invalid.push((
        "over fresh projection boundary",
        over_budget,
        "query_too_large",
    ));

    let mut counter = 10usize;
    for marked in [false, true] {
        counter += 1;
        let stale_occurrence = format!("00000000-0000-4000-8000-{counter:012x}");
        let (mut stale_slot, mut stale_expected) = historied_resident(marked, &stale_occurrence);
        let stale = selected_v3_export(stale_slot.as_ref().unwrap(), "resident/0")
            .expect_err("selected export rejects stale revision on either origin");
        assert_eq!(
            stale.failure_projection("resident/3").code,
            "stale_revision"
        );
        assert_live_fixture(
            stale_slot.as_mut().unwrap(),
            &stale_expected,
            &stale_occurrence,
            "resident/3",
        );
        if marked {
            assert_ordinary_v3_export_refused(stale_slot.as_mut().unwrap());
        } else {
            assert_legacy_origin(stale_slot.as_mut().unwrap(), &stale_expected, Some(2));
        }
        replay_history_after_candidate(
            &mut stale_slot,
            &mut stale_expected,
            &stale_occurrence,
            marked,
        );

        for (name, candidate, expected_code) in &invalid {
            for operation in ["Inspect", "Open"] {
                counter += 1;
                let occurrence = format!("00000000-0000-4000-8000-{counter:012x}");
                let (mut slot, mut expected) = historied_resident(marked, &occurrence);
                let before = slot.as_ref().unwrap().observe_occurrence();
                assert_eq!(before.revision, "resident/3");
                if operation == "Inspect" {
                    let error =
                        inspect_project(candidate).expect_err("candidate must hit its typed gate");
                    assert_eq!(
                        error.failure_projection("resident/3").code,
                        *expected_code,
                        "{name} Inspect code"
                    );
                } else {
                    let error =
                        open_project(&mut slot, candidate, "00000000-0000-4000-8000-000000000999")
                            .expect_err("failed Open must not replace the resident");
                    assert_eq!(
                        error.failure_projection("resident/3").code,
                        *expected_code,
                        "{name} Open code"
                    );
                }
                assert_eq!(
                    slot.as_ref().unwrap().observe_occurrence(),
                    before,
                    "{name} {operation} changed occurrence"
                );
                assert_live_fixture(slot.as_mut().unwrap(), &expected, &occurrence, "resident/3");
                if marked {
                    assert_ordinary_v3_export_refused(slot.as_mut().unwrap());
                } else {
                    assert_legacy_origin(slot.as_mut().unwrap(), &expected, Some(2));
                }
                if expected
                    .keyed_grouped_sum_definitions
                    .contains_key(&KeyedGroupedSumDefinitionId::from(DEFINITION))
                {
                    summary(slot.as_mut().unwrap(), 800.0);
                }
                replay_history_after_candidate(&mut slot, &mut expected, &occurrence, marked);
            }
        }

        // Failed New uses the actual replacement request. Import has no native
        // public slot adapter: candidate failure is checked here; actual preview
        // and install:true replacement preservation belongs to the Worker case.
        for operation in ["New", "ImportCandidateOnly"] {
            counter += 1;
            let occurrence = format!("00000000-0000-4000-8000-{counter:012x}");
            let (mut slot, mut expected) = historied_resident(marked, &occurrence);
            let before = slot.as_ref().unwrap().observe_occurrence();
            let error = if operation == "New" {
                slot.as_mut()
                    .unwrap()
                    .handle(DesignerRequest::NewTable {
                        occurrence_id: FRESH.into(),
                        name: "rejected".into(),
                        columns: vec![tachiko_designer_runtime::NewTableColumnInput {
                            name: "value".into(),
                            field_type: "not-a-type".into(),
                        }],
                    })
                    .expect_err("unsupported New field type must fail before replacement")
            } else {
                let workbook = import_csv(
                    CSV,
                    &ImportOptions {
                        delimiter: ',',
                        header: true,
                    },
                )
                .unwrap();
                let selection = ImportSelection {
                    column_types: vec![],
                    extra_columns: vec![vec![]],
                };
                import_workbook(&workbook, &selection, FRESH)
                    .err()
                    .expect("invalid Import selection cannot construct a candidate")
            };
            let code = if operation == "New" {
                "invalid_table_operation"
            } else {
                "invalid_tracker_operation"
            };
            assert_eq!(error.failure_projection("resident/3").code, code);
            assert_eq!(slot.as_ref().unwrap().observe_occurrence(), before);
            assert_live_fixture(slot.as_mut().unwrap(), &expected, &occurrence, "resident/3");
            summary(slot.as_mut().unwrap(), 800.0);
            if marked {
                assert_ordinary_v3_export_refused(slot.as_mut().unwrap());
            } else {
                assert_legacy_origin(slot.as_mut().unwrap(), &expected, Some(2));
            }
            replay_history_after_candidate(&mut slot, &mut expected, &occurrence, marked);
        }

        // Successful Inspect preserves the complete resident and history.
        for (occurrence_suffix, candidate, expected) in [
            (1usize, valid.clone(), expected_valid.clone()),
            (2usize, title_at.clone(), title_at_document.clone()),
        ] {
            counter += occurrence_suffix;
            let occurrence = format!("00000000-0000-4000-8000-{counter:012x}");
            let (mut slot, mut resident_expected) = historied_resident(marked, &occurrence);
            let before = slot.as_ref().unwrap().observe_occurrence();
            let inspected = inspect_project(&candidate).expect("valid candidate Inspect succeeds");
            assert_eq!(inspected.bootstrap.revision, "resident/0");
            if expected.title == "Imported workbook" {
                assert_opened_fixture(&inspected, &expected);
            } else {
                let literal = independent_opened_projection(
                    "resident/0",
                    &[DEFINITION.to_owned()],
                    "2026-09-28",
                    4.0,
                );
                let mut literal = literal;
                literal.bootstrap.title = "x".repeat(4_096);
                assert_eq!(inspected, literal);
            }
            assert_eq!(
                slot.as_ref().unwrap().observe_occurrence(),
                before,
                "valid Inspect replaced resident"
            );
            assert_live_fixture(
                slot.as_mut().unwrap(),
                &resident_expected,
                &occurrence,
                "resident/3",
            );
            if marked {
                assert_ordinary_v3_export_refused(slot.as_mut().unwrap());
            } else {
                assert_legacy_origin(slot.as_mut().unwrap(), &resident_expected, Some(2));
            }
            replay_history_after_candidate(&mut slot, &mut resident_expected, &occurrence, marked);
        }

        // Successful Open is a replacement (T4), not an Inspect-preservation case.
        counter += 1;
        let occurrence = format!("00000000-0000-4000-8000-{counter:012x}");
        for (candidate, title_at_limit) in [(valid.as_slice(), false), (title_at.as_slice(), true)]
        {
            let (mut slot, _) = historied_resident(marked, &occurrence);
            let replacement_id = format!(
                "00000000-0000-4000-8000-{:012x}",
                counter + usize::from(title_at_limit) + 1
            );
            let replacement = open_project(&mut slot, candidate, &replacement_id)
                .expect("valid candidate Open replaces the resident");
            let mut expected_opened = independent_opened_projection(
                "resident/0",
                &[DEFINITION.to_owned()],
                "2026-09-28",
                4.0,
            );
            if title_at_limit {
                expected_opened.bootstrap.title = "x".repeat(4_096);
            }
            assert_eq!(replacement, expected_opened);
            let runtime = slot.as_mut().unwrap();
            assert_eq!(observed_opened(runtime), expected_opened);
            assert_eq!(
                runtime.observe_occurrence().scope,
                format!("designer-occurrence/{replacement_id}/{DOCUMENT_ID}")
            );
            assert_eq!(runtime.observe_occurrence().revision, "resident/0");
            assert_ordinary_v3_export_refused(runtime);
            for undo in [true, false] {
                let error = if undo {
                    runtime.handle(DesignerRequest::Undo {
                        expected_revision: "resident/0".into(),
                    })
                } else {
                    runtime.handle(DesignerRequest::Redo {
                        expected_revision: "resident/0".into(),
                    })
                }
                .expect_err("successful replacement clears both history stacks");
                assert_eq!(
                    error.failure_projection("resident/0").code,
                    "invalid_tracker_operation"
                );
                assert_eq!(observed_opened(runtime), expected_opened);
                assert_ordinary_v3_export_refused(runtime);
            }
        }
    }
}

#[test]
fn successful_legacy_replacement_clears_v3_origin_marker() {
    let (mut slot, _) = historied_resident(true, ORIGINAL);
    assert_ordinary_v3_export_refused(slot.as_mut().unwrap());
    let fields = FIELD_IDS
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    let rows = ROW_IDS
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    let expected = fixture_document(DOCUMENT_ID.into(), SCHEMA_ID, &fields, &rows, true);
    let reopened = open_project(
        &mut slot,
        include_bytes!(
            "../../../../acceptance/date-definition-save-v3/fixtures/legacy/date-only.twd"
        ),
        SECOND_FRESH,
    )
    .expect("valid legacy replacement must install");
    assert_opened_fixture(&reopened, &expected);
    assert_eq!(
        slot.as_ref().unwrap().observe_occurrence().scope,
        format!("designer-occurrence/{SECOND_FRESH}/{DOCUMENT_ID}")
    );
    assert_legacy_origin(slot.as_mut().unwrap(), &expected, Some(2));

    for undo in [true, false] {
        let runtime = slot.as_mut().unwrap();
        let before_occurrence = runtime.observe_occurrence();
        let before_projection = observed_opened(runtime);
        let error = if undo {
            runtime.handle(DesignerRequest::Undo {
                expected_revision: "resident/0".into(),
            })
        } else {
            runtime.handle(DesignerRequest::Redo {
                expected_revision: "resident/0".into(),
            })
        }
        .expect_err("successful legacy replacement clears both history stacks");
        assert_eq!(
            error.failure_projection("resident/0").code,
            "invalid_tracker_operation"
        );
        assert_eq!(runtime.observe_occurrence(), before_occurrence);
        assert_eq!(observed_opened(runtime), before_projection);
        assert_legacy_origin(runtime, &expected, Some(2));
    }

    let (mut new_slot, _) = historied_resident(true, ORIGINAL);
    assert_ordinary_v3_export_refused(new_slot.as_mut().unwrap());
    let DesignerResponse::Opened(new_opened) = new_slot
        .as_mut()
        .unwrap()
        .handle(DesignerRequest::NewTable {
            occurrence_id: "00000000-0000-4000-8000-000000000006".into(),
            name: "reset".into(),
            columns: vec![tachiko_designer_runtime::NewTableColumnInput {
                name: "value".into(),
                field_type: "text".into(),
            }],
        })
        .unwrap()
    else {
        panic!("NewTable returns the replaced candidate projection");
    };
    let new_document_id = "native_table_document_0001_00000000-0000-4000-8000-000000000006";
    let new_schema_id = "native_table_schema_0002_00000000-0000-4000-8000-000000000006";
    let new_field_id = "native_table_field_0003_00000000-0000-4000-8000-000000000006";
    let expected_new_document = {
        let schema_id = SchemaId::from(new_schema_id);
        let field_id = FieldId::from(new_field_id);
        let schema = Schema {
            id: schema_id.clone(),
            key: SchemaKey::from("reset"),
            fields: BTreeMap::from([(
                field_id.clone(),
                FieldDefinition {
                    id: field_id,
                    key: FieldKey::from("value"),
                    field_type: FieldType::Text,
                    required: true,
                    constraint: FieldConstraint::None,
                },
            )]),
        };
        let mut document = Document::empty(new_document_id, "reset");
        document.schemas.insert(schema_id, schema);
        document
    };
    assert_eq!(
        new_opened.bootstrap,
        BootstrapProjection {
            title: "reset".into(),
            revision: "resident/0".into(),
            default_collection: "reset".into(),
            collections: vec![CollectionSummary {
                id: new_schema_id.into(),
                key: "reset".into(),
                entity_count: 0
            }],
            keyed_grouped_sum_definition_ids: vec![],
        }
    );
    assert_eq!(
        new_opened.table,
        TableProjection {
            revision: "resident/0".into(),
            tracker_profile: None,
            native_table_profile: Some(true),
            collection: CollectionSummary {
                id: new_schema_id.into(),
                key: "reset".into(),
                entity_count: 0
            },
            columns: vec![ColumnProjection {
                id: new_field_id.into(),
                key: "value".into(),
                field_type: "text".into(),
                dropdown_options: None
            }],
            rows: vec![],
        }
    );
    assert_eq!(
        new_slot.as_ref().unwrap().observe_occurrence().scope,
        format!("designer-occurrence/00000000-0000-4000-8000-000000000006/{new_document_id}")
    );
    let new_saved = new_slot
        .as_ref()
        .unwrap()
        .export_project("resident/0")
        .expect("New clears v3 marker");
    assert_eq!(saved_format_version(&new_saved.bytes), 1);
    assert_eq!(saved_document(&new_saved.bytes), expected_new_document);
    for undo in [true, false] {
        let error = if undo {
            new_slot.as_mut().unwrap().handle(DesignerRequest::Undo {
                expected_revision: "resident/0".into(),
            })
        } else {
            new_slot.as_mut().unwrap().handle(DesignerRequest::Redo {
                expected_revision: "resident/0".into(),
            })
        }
        .expect_err("New begins with empty Undo and Redo stacks");
        assert_eq!(
            error.failure_projection("resident/0").code,
            "invalid_tracker_operation"
        );
        assert_eq!(
            new_slot.as_ref().unwrap().observe_occurrence().revision,
            "resident/0"
        );
    }

    let (mut import_slot, _) = historied_resident(true, ORIGINAL);
    assert_ordinary_v3_export_refused(import_slot.as_mut().unwrap());
    let source = import_csv(CSV, &ImportOptions::default()).unwrap();
    let selection = ImportSelection {
        column_types: vec![vec![
            ImportFieldType::Text,
            ImportFieldType::Text,
            ImportFieldType::Number,
            ImportFieldType::Number,
            ImportFieldType::Date,
        ]],
        extra_columns: vec![vec![]],
    };
    let import_occurrence = "00000000-0000-4000-8000-000000000005";
    let (imported_runtime, imported) = import_workbook(&source, &selection, import_occurrence)
        .expect("native importer constructs a fresh candidate from immutable CSV");
    let import_prefix = format!("import_{import_occurrence}_");
    let import_document_id = format!("{import_prefix}0001");
    let import_schema_id = format!("{import_prefix}0002");
    let import_fields = (3..=7)
        .map(|n| format!("{import_prefix}{n:04}"))
        .collect::<Vec<_>>();
    let import_rows = [
        format!("{import_prefix}0008"),
        format!("{import_prefix}0009"),
    ];
    let mut expected_import = fixture_document(
        import_document_id.clone().into(),
        &import_schema_id,
        &import_fields,
        &import_rows,
        true,
    );
    expected_import
        .entities
        .get_mut(import_rows[0].as_str())
        .unwrap()
        .fields
        .insert(
            FieldId::from(import_fields[4].as_str()),
            Value::Date(Date::parse("2026-09-26").unwrap()),
        );
    assert_opened_fixture(&imported.opened, &expected_import);
    // import_workbook has no public resident-slot API. This assignment models
    // the host-owned candidate install; Worker importSpreadsheet covers the
    // actual host/Worker integration transition.
    import_slot = Some(imported_runtime);
    assert_eq!(
        import_slot.as_ref().unwrap().observe_occurrence().scope,
        format!("designer-occurrence/{import_occurrence}/{import_document_id}")
    );
    assert_eq!(imported.opened.bootstrap.revision, "resident/0");
    assert_legacy_origin(import_slot.as_mut().unwrap(), &expected_import, Some(2));
}

#[test]
fn successful_v3_replacement_preserves_origin_marker() {
    let (mut candidate, mut expected_replacement, _, quantity) = prepared(true, true);
    edit(
        &mut candidate,
        ROW_IDS[0],
        &quantity,
        ScalarEditInput::Number { input: "6".into() },
    );
    set_first_quantity(&mut expected_replacement, 6.0);
    let source_bytes = selected_v3_export(&candidate, "resident/3").unwrap();
    let (mut slot, _) = historied_resident(true, ORIGINAL);
    let replaced = open_project(&mut slot, &source_bytes.bytes, SECOND_FRESH)
        .expect("successful v3-to-v3 replacement installs the different q6 candidate");
    assert_opened_fixture(&replaced, &expected_replacement);
    assert_live_fixture(
        slot.as_mut().unwrap(),
        &expected_replacement,
        SECOND_FRESH,
        "resident/0",
    );
    assert_ordinary_v3_export_refused(slot.as_mut().unwrap());
    for undo in [true, false] {
        let error = if undo {
            slot.as_mut().unwrap().handle(DesignerRequest::Undo {
                expected_revision: "resident/0".into(),
            })
        } else {
            slot.as_mut().unwrap().handle(DesignerRequest::Redo {
                expected_revision: "resident/0".into(),
            })
        }
        .expect_err("v3 replacement establishes a fresh empty history");
        assert_eq!(
            error.failure_projection("resident/0").code,
            "invalid_tracker_operation"
        );
        assert_live_fixture(
            slot.as_mut().unwrap(),
            &expected_replacement,
            SECOND_FRESH,
            "resident/0",
        );
        assert_ordinary_v3_export_refused(slot.as_mut().unwrap());
    }
}

#[test]
fn transfer_limit_and_oversized_inspect_preserve_the_current_v3_occurrence() {
    const TRANSFER_MAX: usize = 64 * 1024 * 1024;
    const TRANSFER_MAX_PLUS_ONE: usize = TRANSFER_MAX + 1;
    let (runtime, _, _, _) = prepared(true, false);
    let v3 = selected_v3_export(&runtime, "resident/1").unwrap();
    let mut slot = None;
    open_project(&mut slot, &v3.bytes, FRESH).unwrap();
    let before = slot.as_ref().unwrap().observe_occurrence();
    let before_projection = observed_opened(slot.as_mut().unwrap());

    // This zero-filled exact-transfer-limit vector is transport-only. It is
    // deliberately not called a valid project/open boundary: after transport
    // admission it must reach the ordinary malformed-transfer result.
    let at_limit = vec![0; TRANSFER_MAX];
    let at_limit_error = inspect_project(&at_limit)
        .expect_err("bad magic is parsed at the exact allowed byte limit");
    assert_eq!(
        at_limit_error.failure_projection("resident/0").code,
        "invalid_project"
    );
    let exact_open_error = open_project(&mut slot, &at_limit, SECOND_FRESH)
        .expect_err("exact-limit transport-only input must fail format admission");
    assert_eq!(
        exact_open_error.failure_projection(&before.revision).code,
        "invalid_project"
    );
    assert_eq!(slot.as_ref().unwrap().observe_occurrence(), before);
    assert_ordinary_v3_export_refused(slot.as_mut().unwrap());
    drop(at_limit);

    let over_limit = vec![0; TRANSFER_MAX + 1];
    let inspect_error =
        inspect_project(&over_limit).expect_err("Inspect enforces the private transfer limit");
    assert!(matches!(
        inspect_error,
        tachiko_designer_runtime::DesignerError::ProjectTransferTooLarge {
            actual: TRANSFER_MAX_PLUS_ONE,
            maximum: TRANSFER_MAX,
        }
    ));
    let open_error = open_project(&mut slot, &over_limit, SECOND_FRESH)
        .expect_err("oversized replacement is rejected before candidate installation");
    assert!(matches!(
        open_error,
        tachiko_designer_runtime::DesignerError::ProjectTransferTooLarge {
            actual: TRANSFER_MAX_PLUS_ONE,
            maximum: TRANSFER_MAX,
        }
    ));
    let runtime = slot.as_mut().unwrap();
    assert_eq!(runtime.observe_occurrence(), before);
    assert_eq!(observed_opened(runtime), before_projection);
    assert_ordinary_v3_export_refused(runtime);
}

#[test]
fn ordinary_export_keeps_the_mixed_legacy_refusal_control() {
    let (runtime, expected, _, _) = prepared(true, true);
    assert_eq!(runtime.observe_occurrence().revision, "resident/2");
    assert!(runtime.export_project("resident/2").is_err());
    assert_eq!(runtime.observe_occurrence().revision, "resident/2");
    assert_eq!(
        saved_document(&selected_v3_export(&runtime, "resident/2").unwrap().bytes),
        expected
    );
}

#[test]
fn fresh_candidate_ordinary_legacy_exports_remain_v1_v2_compatible() {
    for (name, date, definition, expected_version) in [
        ("ordinary-date-only", true, false, 2),
        ("ordinary-definition-only", false, true, 2),
        ("ordinary-neither", false, false, 1),
    ] {
        let (runtime, expected, _, _) = prepared(date, definition);
        let revision = runtime.observe_occurrence().revision;
        let saved = runtime
            .export_project(&revision)
            .expect("ordinary current candidate exporter retains each legacy profile");
        assert_eq!(saved_document(&saved.bytes), expected);
        let actual_version = if saved.bytes.starts_with(b"TWDPROJ2") {
            2
        } else {
            let entries = opaque_bundle_entries(&saved.bytes);
            let manifest: serde_json::Value = serde_json::from_slice(
                &entries
                    .iter()
                    .find(|(path, _)| path == "manifest.json")
                    .unwrap()
                    .1,
            )
            .unwrap();
            manifest["format_version"].as_u64().unwrap()
        };
        assert_eq!(actual_version, expected_version);
        capture_candidate_output(name, &saved.bytes);
    }
}

#[test]
fn candidate_inspect_and_open_refuse_valid_v3_text_and_number_constraints() {
    let profiles = [
        (
            "constrained-text",
            acceptance_fixtures::constrained_v3_transfer(1),
        ),
        (
            "constrained-number",
            acceptance_fixtures::constrained_v3_transfer(2),
        ),
        (
            "constrained-nondefault-text",
            acceptance_fixtures::nondefault_constrained_v3_transfer(1),
        ),
        (
            "constrained-nondefault-number",
            acceptance_fixtures::nondefault_constrained_v3_transfer(2),
        ),
    ];
    let mut counter = 400;
    for (name, input) in profiles {
        capture_candidate_output(name, &input);
        for marked in [false, true] {
            for operation in ["Inspect", "Open"] {
                counter += 1;
                let occurrence = format!("00000000-0000-4000-8000-{counter:012x}");
                let (mut slot, mut expected) = historied_resident(marked, &occurrence);
                let before = slot.as_ref().unwrap().observe_occurrence();
                assert_eq!(before.revision, "resident/3");
                let error = if operation == "Inspect" {
                    inspect_project(&input).expect_err("all fields, including non-default unbound fields, must have None constraints")
                } else {
                    open_project(&mut slot, &input, FRESH).expect_err(
                        "valid storage constraints are outside the bounded runtime profile",
                    )
                };
                assert_eq!(
                    error.failure_projection("resident/3").code,
                    "unsupported_project",
                    "{name} {operation}"
                );
                assert_eq!(slot.as_ref().unwrap().observe_occurrence(), before);
                assert_live_fixture(slot.as_mut().unwrap(), &expected, &occurrence, "resident/3");
                summary(slot.as_mut().unwrap(), 800.0);
                if marked {
                    assert_ordinary_v3_export_refused(slot.as_mut().unwrap());
                } else {
                    assert_legacy_origin(slot.as_mut().unwrap(), &expected, Some(2));
                }
                replay_history_after_candidate(&mut slot, &mut expected, &occurrence, marked);
            }
        }
    }
}

fn legacy_ingress(bytes: &[u8], date: bool, definition: bool) {
    // Opaque fixture identities from the immutable baseline capture, not from
    // candidate import/export. The three committed inputs have frozen SHA-256s.
    let fields = FIELD_IDS
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    let rows = ROW_IDS
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    let schema = SCHEMA_ID.to_owned();
    let mut expected = fixture_document(DOCUMENT_ID.into(), &schema, &fields, &rows, date);
    if definition {
        let definition = fixture_definition(&schema, &fields);
        expected
            .keyed_grouped_sum_definitions
            .insert(definition.id.clone(), definition);
    }
    let mut slot = None;
    let opened =
        open_project(&mut slot, bytes, FRESH).expect("valid immutable legacy input must open");
    assert_eq!(opened.bootstrap.revision, "resident/0");
    assert_opened_fixture(&opened, &expected);
    assert_eq!(
        opened.bootstrap.keyed_grouped_sum_definition_ids,
        if definition {
            vec![DEFINITION.to_owned()]
        } else {
            vec![]
        }
    );
    let runtime = slot.as_mut().unwrap();
    assert_eq!(
        document_id_in_scope(&runtime.observe_occurrence().scope, FRESH),
        expected.id
    );
    if definition {
        summary(runtime, 800.0);
    }
    assert_eq!(
        saved_document(&runtime.export_project("resident/0").unwrap().bytes),
        expected
    );
    edit(
        runtime,
        &rows[0],
        &fields[2],
        ScalarEditInput::Number { input: "6".into() },
    );
    expected
        .entities
        .get_mut(rows[0].as_str())
        .unwrap()
        .fields
        .insert(
            FieldId::from(fields[2].as_str()),
            Value::Number(Number::new(6.0).unwrap()),
        );
    if definition {
        summary(runtime, 1200.0);
    }
    let revision = runtime.observe_occurrence().revision;
    let saved = selected_v3_export(runtime, &revision)
        .expect("legacy ingress must re-export through the candidate's explicit v3 target");
    let profile = match (date, definition) {
        (true, false) => "legacy-ingress-date-only",
        (false, true) => "legacy-ingress-definition-only",
        (false, false) => "legacy-ingress-neither",
        (true, true) => unreachable!("there is no mixed legacy control input"),
    };
    capture_candidate_output(profile, &saved.bytes);
    assert_eq!(saved_document(&saved.bytes), expected);
    close_project(&mut slot);
    let reopened = open_project(&mut slot, &saved.bytes, SECOND_FRESH)
        .expect("edited legacy work must reopen");
    assert_opened_fixture(&reopened, &expected);
    assert_eq!(
        document_id_in_scope(
            &slot.as_ref().unwrap().observe_occurrence().scope,
            SECOND_FRESH,
        ),
        expected.id
    );
    if definition {
        summary(slot.as_mut().unwrap(), 1200.0);
    }
    assert!(slot.as_ref().unwrap().export_project("resident/0").is_err());
    println!("LEGACY_INGRESS_EDIT_REEXPORT_PASS date={date} definition={definition}");
}

#[test]
fn legacy_date_only_ingress_edit_reexport() {
    legacy_ingress(
        include_bytes!(
            "../../../../acceptance/date-definition-save-v3/fixtures/legacy/date-only.twd"
        ),
        true,
        false,
    );
}
#[test]
fn legacy_definition_only_ingress_edit_reexport() {
    legacy_ingress(
        include_bytes!(
            "../../../../acceptance/date-definition-save-v3/fixtures/legacy/definition-only.twd"
        ),
        false,
        true,
    );
}
#[test]
fn legacy_neither_ingress_edit_reexport() {
    legacy_ingress(
        include_bytes!(
            "../../../../acceptance/date-definition-save-v3/fixtures/legacy/neither.twd"
        ),
        false,
        false,
    );
}

#[test]
fn frozen_codec_and_bridge_profile_controls() {
    for (date, definition) in [(false, false), (true, false), (false, true), (true, true)] {
        let (runtime, document, _, _) = prepared(date, definition);
        assert_eq!(encode_roproj_v1(&document).is_ok(), !date && !definition);
        assert_eq!(encode_roproj_v2(&document).is_ok(), !date);
        assert_eq!(to_canonical_string(&document).is_ok(), !definition);
        let revision = runtime.observe_occurrence().revision;
        assert_eq!(
            runtime.export_canonical_tree(&revision).is_ok(),
            !date && !definition
        );
        assert_eq!(
            runtime.export_portable_ro(&revision).is_ok(),
            !date && !definition
        );
        if !date && !definition {
            let bridge = runtime.export_canonical_tree(&revision).unwrap();
            let expected_tree = encode_roproj_v1(&document).unwrap();
            let expected_paths = std::iter::once("manifest.json".to_owned())
                .chain(std::iter::once("schemas.json".to_owned()))
                .chain((0..16).map(|index| format!("entities/{index:x}.jsonl")))
                .collect::<Vec<_>>();
            let bridge_files = opaque_bundle_entries(&bridge.bytes);
            assert_eq!(
                bridge_files
                    .iter()
                    .map(|(path, _)| path.clone())
                    .collect::<Vec<_>>(),
                expected_paths
            );
            assert_eq!(
                bridge_files,
                expected_tree
                    .files()
                    .iter()
                    .map(|file| (file.path().to_owned(), file.bytes().to_vec()))
                    .collect::<Vec<_>>()
            );
            let portable = runtime.export_portable_ro(&revision).unwrap();
            tachiko_designer_runtime::verify_portable_ro(&portable.bytes).unwrap();
            let mut slot = None;
            tachiko_designer_runtime::open_portable_ro(&mut slot, &portable.bytes, FRESH).unwrap();
            assert_eq!(
                saved_document(&slot.unwrap().export_project("resident/0").unwrap().bytes),
                document
            );
        }
        if !date {
            let v2 = encode_roproj_v2(&document).unwrap();
            assert_eq!(decode_roproj_v2(&v2).unwrap(), document);
            let files = v2
                .files()
                .iter()
                .map(|f| (f.path().to_owned(), f.bytes().to_vec()))
                .collect();
            assert!(
                CanonicalRoProjectV1::try_from_files(files).is_err(),
                "frozen v1 reader must refuse v2"
            );
        }
        println!("FROZEN_PROFILE_PASS date={date} definition={definition}");
    }
}
