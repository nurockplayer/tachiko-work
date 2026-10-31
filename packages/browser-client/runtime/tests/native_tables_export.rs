//! Issue #545: observable native output contract; no client value/name mapping.
//! These acceptance tests use the native-only host bridge.
#![cfg(not(target_arch = "wasm32"))]

use std::io::{Cursor, Read};

use serde_json::{Value as Json, json};
use tachiko_designer_runtime::host_bridge::{DesignerHost, HostReply};
use tachiko_designer_runtime::interop_adapter::{
    FidelityCategory, ImportOptions, SourceValue, SourceWorkbook, import_csv, import_xlsx,
};
use tachiko_designer_runtime::{
    DesignerRequest, DesignerResponse, DesignerRuntime, ImportFieldType, ImportSelection,
    KeyedGroupedSumDefinitionInput, import_workbook,
};
use tachiko_workspace_engine::{
    Date, Document, Entity, EntityId, EntityKey, Expression, FieldConstraint, FieldDefinition,
    FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};
use zip::ZipArchive;

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000545";
const NEXT: &str = "00000000-0000-4000-8000-000000000546";

fn field(id: &str, kind: FieldType, required: bool) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        display_name: None,
        number_format: None,
        field_type: kind,
        required,
        constraint: FieldConstraint::None,
    }
}

fn add_table(
    document: &mut Document,
    id: &str,
    key: &str,
    name: &str,
    fields: Vec<FieldDefinition>,
    rows: Vec<Vec<Option<Value>>>,
) {
    for (index, values) in rows.into_iter().enumerate() {
        let entity_id = EntityId::from(format!("{id}-r{index:03}"));
        document.entities.insert(
            entity_id.clone(),
            Entity {
                id: entity_id.clone(),
                key: EntityKey::from(entity_id.as_str()),
                schema: SchemaId::from(id),
                fields: fields
                    .iter()
                    .zip(values)
                    .filter_map(|(f, v)| v.map(|v| (f.id.clone(), v)))
                    .collect(),
            },
        );
    }
    let schema = Schema {
        field_order: None,
        id: SchemaId::from(id),
        key: SchemaKey::from(key),
        display_name: Some(name.into()),
        fields: fields.into_iter().map(|f| (f.id.clone(), f)).collect(),
    };
    document.schemas.insert(schema.id.clone(), schema);
}

fn fixture() -> Document {
    let mut document = Document::empty("export-fixture", "Native tables");
    add_table(
        &mut document,
        "z-id",
        "alpha",
        "売上 / Sales",
        vec![
            field("a-text", FieldType::Text, false),
            field("b-number", FieldType::Number, false),
            field("c-bool", FieldType::Boolean, false),
            field("d-date", FieldType::Date, false),
        ],
        vec![
            vec![
                Some(Value::Text("000123 林,\"line\"\nnext".into())),
                Some(Value::Number(Number::new(-1.5).unwrap())),
                Some(Value::Boolean(false)),
                Some(Value::Date(Date::parse("2024-02-29").unwrap())),
            ],
            vec![Some(Value::Text(String::new())), None, None, None],
            vec![
                None,
                Some(Value::Number(Number::new(0.0).unwrap())),
                Some(Value::Boolean(true)),
                None,
            ],
        ],
    );
    add_table(
        &mut document,
        "a-id",
        "beta",
        "Empty",
        vec![field("empty", FieldType::Text, true)],
        vec![],
    );
    document
}

fn host_for(document: Document) -> DesignerHost {
    let runtime = DesignerRuntime::from_document(document, OCCURRENCE).unwrap();
    let bytes = runtime.export_project_v3("resident/0").unwrap().bytes;
    let mut host = DesignerHost::new();
    let reply: Json = serde_json::from_slice(&host.open_project(&bytes, NEXT.as_bytes())).unwrap();
    assert_eq!(reply["status"], "ok", "{reply}");
    host
}

fn export(
    host: &mut DesignerHost,
    revision: &str,
    collection: &str,
    format: &str,
) -> (Json, Option<Vec<u8>>) {
    let HostReply { reply, bytes } = host.spreadsheet(&serde_json::to_vec(&json!({
        "type":"export_native_tables", "expected_revision":revision, "collection":collection, "format":format,
    })).unwrap(), &[]);
    (serde_json::from_slice(&reply).unwrap(), bytes)
}

fn xml(bytes: &[u8], path: &str) -> String {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = String::new();
    archive
        .by_name(path)
        .unwrap()
        .read_to_string(&mut output)
        .unwrap();
    output
}

fn assert_finding(reply: &Json, code: &str, category: &str) {
    assert!(
        reply["response"]["payload"]["ledger"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["code"] == code && f["category"] == category),
        "{code}: {reply}"
    );
}

fn source_values(
    sheet: &tachiko_designer_runtime::interop_adapter::SourceSheet,
) -> Vec<Vec<SourceValue>> {
    sheet
        .rows
        .iter()
        .map(|row| row.iter().map(|cell| cell.value.clone()).collect())
        .collect()
}

#[test]
fn csv_blank_empty_text_disclosure_is_selected_optional_text_and_value_specific() {
    for (kind, required, rows, expected) in [
        (FieldType::Text, false, vec![vec![None]], true),
        (
            FieldType::Text,
            false,
            vec![vec![Some(Value::Text(String::new()))]],
            true,
        ),
        (
            FieldType::Text,
            false,
            vec![vec![Some(Value::Text("present".into()))]],
            false,
        ),
        (FieldType::Text, false, vec![], false),
        (
            FieldType::Text,
            true,
            vec![vec![Some(Value::Text(String::new()))]],
            false,
        ),
        (FieldType::Number, false, vec![vec![None]], false),
    ] {
        let mut document = Document::empty("csv-disclosure", "CSV disclosure");
        add_table(
            &mut document,
            "selected",
            "selected",
            "Selected",
            vec![field("value", kind, required)],
            rows,
        );
        // An excluded optional Text collection cannot trigger the finding.
        add_table(
            &mut document,
            "other",
            "other",
            "Other",
            vec![field("note", FieldType::Text, false)],
            vec![vec![None]],
        );
        let mut host = host_for(document);
        for format in ["csv", "xlsx"] {
            let (reply, bytes) = export(&mut host, "resident/0", "selected", format);
            assert_eq!(reply["status"], "ok", "{reply}");
            assert!(bytes.is_some());
            let findings: Vec<_> = reply["response"]["payload"]["ledger"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|finding| finding["code"] == "native_tables_csv_blank_empty_text")
                .collect();
            assert_eq!(
                findings.len(),
                usize::from(expected && format == "csv"),
                "{reply}"
            );
            for finding in findings {
                assert_eq!(finding["category"], "lossy_on_export");
                assert_eq!(finding["location"], "selected");
                assert_eq!(finding["blocking"], false);
            }
        }
    }
}

#[test]
fn sole_optional_column_keeps_all_blank_rows_and_distinguishes_empty_text_in_xlsx() {
    let mut document = Document::empty("sole", "Sole optional column");
    add_table(
        &mut document,
        "items",
        "items",
        "Items",
        vec![field("note", FieldType::Text, false)],
        vec![
            vec![None],
            vec![Some(Value::Text(String::new()))],
            vec![None],
        ],
    );
    let mut host = host_for(document);
    let (csv_reply, csv) = export(&mut host, "resident/0", "items", "csv");
    assert_eq!(csv.unwrap(), b"note\r\n\"\"\r\n\"\"\r\n\"\"\r\n");
    assert_finding(
        &csv_reply,
        "native_tables_csv_blank_empty_text",
        "lossy_on_export",
    );
    let (reply, xlsx) = export(&mut host, "resident/0", "items", "xlsx");
    assert_eq!(reply["status"], "ok", "{reply}");
    let xlsx = xlsx.unwrap();
    let sheet = xml(&xlsx, "xl/worksheets/sheet1.xml");
    for address in ["A2", "A4"] {
        assert!(
            sheet.contains(&format!("<c r=\"{address}\" s=\"0\" t=\"n\"></c>")),
            "{sheet}"
        );
    }
    assert!(
        sheet.contains(
            "<c r=\"A3\" s=\"0\" t=\"inlineStr\"><is><t xml:space=\"preserve\"></t></is></c>"
        ),
        "{sheet}"
    );
    let imported = import_xlsx(&xlsx).unwrap();
    assert_eq!(
        source_values(&imported.sheets[0]),
        vec![
            vec![SourceValue::Empty],
            vec![SourceValue::Text {
                value: String::new()
            }],
            vec![SourceValue::Empty],
        ]
    );
    let (mut runtime, projection) = import_workbook(
        &imported,
        &ImportSelection {
            column_types: vec![vec![ImportFieldType::Text]],
            extra_columns: vec![vec![]],
        },
        NEXT,
    )
    .unwrap();
    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: projection.opened.bootstrap.default_collection,
        })
        .unwrap()
    else {
        panic!("table")
    };
    assert_eq!(table.rows.len(), 3);
    assert!(!table.columns[0].required);
    assert_eq!(table.rows[0].blank_fields, [table.columns[0].id.clone()]);
    assert_eq!(
        table.rows[0].fields,
        [] as [tachiko_designer_runtime::FieldProjection; 0]
    );
    assert_eq!(table.rows[1].blank_fields, [] as [String; 0]);
    assert_eq!(table.rows[2].blank_fields, [table.columns[0].id.clone()]);
}

#[test]
fn typed_xlsx_roundtrip_rectangular_empty_cells_and_canonical_order() {
    let document = fixture();
    let runtime = DesignerRuntime::from_document(document.clone(), OCCURRENCE).unwrap();
    let mapped = runtime
        .export_native_tables_workbook("resident/0", "z-id")
        .unwrap();
    assert_eq!(
        mapped
            .sheets
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["売上 _ Sales", "Empty"]
    );
    assert_eq!(
        mapped.sheets[0].rows[1]
            .iter()
            .map(|c| c.value.clone())
            .collect::<Vec<_>>(),
        [
            SourceValue::Text {
                value: String::new()
            },
            SourceValue::Empty,
            SourceValue::Empty,
            SourceValue::Empty
        ]
    );
    let mut host = host_for(document);
    let (reply, bytes) = export(&mut host, "resident/0", "z-id", "xlsx");
    assert_eq!(reply["status"], "ok", "{reply}");
    let bytes = bytes.unwrap();
    let (_, again) = export(&mut host, "resident/0", "alpha", "xlsx");
    assert_eq!(again.unwrap(), bytes);
    assert_eq!(reply["response"]["payload"]["byte_length"], bytes.len());
    // Optional artifact for independent application reopening; all acceptance
    // assertions below run regardless of whether evidence output was requested.
    if let Some(path) = std::env::var_os("TACHIKO_NATIVE_TABLES_EXPORT_EVIDENCE") {
        std::fs::write(path, &bytes).unwrap();
    }
    let sheet = xml(&bytes, "xl/worksheets/sheet1.xml");
    for cell in [
        "t=\"inlineStr\"",
        "t=\"n\"><v>-1.5</v>",
        "t=\"b\"><v>0</v>",
        "t=\"d\"><v>2024-02-29</v>",
        "r=\"B3\"",
        "r=\"D4\"",
    ] {
        assert!(sheet.contains(cell), "{cell}: {sheet}");
    }
    assert!(!sheet.contains("<f>"));
    assert!(xml(&bytes, "xl/styles.xml").contains("yyyy-mm-dd"));
    assert!(!xml(&bytes, "xl/workbook.xml").contains("definedName"));
    let imported = import_xlsx(&bytes).unwrap();
    assert!(
        !imported.ledger.iter().any(|f| f.blocking),
        "{:?}",
        imported.ledger
    );
    assert_eq!(imported.sheets.len(), 2);
    assert_eq!(
        source_values(&imported.sheets[0]),
        source_values(&mapped.sheets[0])
    );
    assert_eq!(
        imported.sheets[1].rows,
        Vec::<Vec<tachiko_designer_runtime::interop_adapter::SourceCell>>::new()
    );
    let (_, projection) = import_workbook(
        &imported,
        &ImportSelection {
            column_types: vec![
                vec![
                    ImportFieldType::Text,
                    ImportFieldType::Number,
                    ImportFieldType::Boolean,
                    ImportFieldType::Date,
                ],
                vec![ImportFieldType::Text],
            ],
            extra_columns: vec![vec![], vec![]],
        },
        NEXT,
    )
    .unwrap();
    assert!(!projection.ledger.iter().any(|f| f.blocking));
    for code in [
        "native_tables_grouped_sum_definitions_excluded",
        "native_tables_grouped_sum_results_excluded",
        "native_tables_reports_charts_excluded",
        "native_tables_session_history_excluded",
        "native_tables_sheet_presentation_excluded",
    ] {
        assert_finding(&reply, code, "lossy_on_export");
    }
    assert_finding(&reply, "native_tables_worksheet_name", "converted");
    assert_finding(
        &reply,
        "native_tables_outbound_profile",
        "native_equivalent",
    );
}

#[test]
fn csv_explicit_reimport_preserves_lexical_scalars_and_discloses_missing_empty_ambiguity() {
    let mut host = host_for(fixture());
    let (reply, bytes) = export(&mut host, "resident/0", "alpha", "csv");
    assert_eq!(reply["status"], "ok", "{reply}");
    let workbook = import_csv(
        &bytes.unwrap(),
        &ImportOptions {
            delimiter: ',',
            header: true,
            ..ImportOptions::default()
        },
    )
    .unwrap();
    assert_eq!(workbook.sheets.len(), 1);
    assert_eq!(
        workbook.sheets[0].rows[0][0].value,
        SourceValue::Text {
            value: "000123 林,\"line\"\nnext".into()
        }
    );
    let (mut imported, projection) = import_workbook(
        &workbook,
        &ImportSelection {
            column_types: vec![vec![
                ImportFieldType::Text,
                ImportFieldType::Number,
                ImportFieldType::Boolean,
                ImportFieldType::Date,
            ]],
            extra_columns: vec![vec![]],
        },
        NEXT,
    )
    .unwrap();
    let DesignerResponse::Table(table) = imported
        .handle(DesignerRequest::QueryTable {
            collection: projection.opened.bootstrap.default_collection.clone(),
        })
        .unwrap()
    else {
        panic!("table")
    };
    assert_eq!(table.rows.len(), 3);
    assert_eq!(
        table.rows[1].blank_fields,
        table
            .columns
            .iter()
            .map(|column| column.id.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        table.rows[1].fields.is_empty(),
        "CSV empty source fields remain absent, including collapsed empty Text"
    );
    assert_eq!(
        table.rows[2].blank_fields,
        [table.columns[0].id.clone(), table.columns[3].id.clone()]
    );
    let projected = imported
        .export_native_tables_csv_workbook(
            "resident/0",
            &projection.opened.bootstrap.default_collection,
        )
        .unwrap();
    assert_eq!(
        projected.sheets[0].rows[0][1].value,
        SourceValue::Number { value: -1.5 }
    );
    assert_eq!(
        projected.sheets[0].rows[0][2].value,
        SourceValue::Boolean { value: false }
    );
    assert_eq!(
        projected.sheets[0].rows[0][3].value,
        SourceValue::Date {
            value: "2024-02-29".into()
        }
    );
    assert_eq!(projected.sheets[0].rows[1][0].value, SourceValue::Empty);
    assert_finding(&reply, "native_tables_csv_values_only", "lossy_on_export");
    assert_finding(
        &reply,
        "native_tables_csv_collection_excluded",
        "lossy_on_export",
    );
    let (reply, bytes) = export(&mut host, "resident/0", "beta", "csv");
    assert_eq!(reply["status"], "ok");
    assert_eq!(bytes.unwrap(), b"empty\r\n");
}

#[test]
fn worksheet_and_header_mapping_is_deterministic_unicode_safe_and_collision_complete() {
    let mut document = Document::empty("names", "Names");
    for (index, name) in [
        "Sales/a",
        "sales:a",
        "Sales_a (2)",
        "'   '",
        &"林".repeat(40),
        &"林".repeat(39),
    ]
    .into_iter()
    .enumerate()
    {
        let mut fields = vec![
            field("a", FieldType::Text, true),
            field("b", FieldType::Text, true),
            field("c", FieldType::Text, true),
            field("d", FieldType::Text, true),
        ];
        for (f, label) in
            fields
                .iter_mut()
                .zip(["Amount", "amount", "amount (2)", &"長".repeat(100)])
        {
            f.display_name = Some(label.into());
        }
        add_table(
            &mut document,
            &format!("id-{index}"),
            &format!("key-{index}"),
            name,
            fields,
            vec![],
        );
    }
    let runtime = DesignerRuntime::from_document(document, OCCURRENCE).unwrap();
    let a = runtime
        .export_native_tables_workbook("resident/0", "key-0")
        .unwrap();
    let b = runtime
        .export_native_tables_workbook("resident/0", "key-0")
        .unwrap();
    assert_eq!(a, b);
    assert_eq!(
        a.sheets.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        [
            "Sales_a",
            "sales_a (2)",
            "Sales_a (2) (2)",
            "Table",
            &"林".repeat(31),
            &format!("{} (2)", "林".repeat(27))
        ]
    );
    assert_eq!(
        a.sheets[0]
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["Amount", "amount (2)", "amount (2) (2)", &"長".repeat(100)]
    );
    assert!(
        a.ledger
            .iter()
            .filter(|f| f.code == "native_tables_header_name")
            .all(|f| f.category == FidelityCategory::Converted)
    );
    assert_eq!(
        a.ledger
            .iter()
            .filter(|f| f.code == "native_tables_worksheet_name")
            .count(),
        6
    );
}

#[test]
fn csv_refuses_all_formula_prefixes_in_text_and_original_headers_but_typed_xlsx_preserves_them() {
    for text in ["=1", " +1", "\t-1", "\r\n@SUM(A1)", "\u{1}=1"] {
        for header in [false, true] {
            let mut document = Document::empty("injection", "Injection");
            let mut f = field("text", FieldType::Text, true);
            if header {
                f.display_name = Some(
                    text.trim_start_matches(|c: char| c.is_whitespace() || c.is_control())
                        .into(),
                );
            }
            add_table(
                &mut document,
                "s",
                "s",
                "S",
                vec![f],
                vec![vec![Some(Value::Text(
                    if header { "safe" } else { text }.into(),
                ))]],
            );
            let mut host = host_for(document);
            let (reply, bytes) = export(&mut host, "resident/0", "s", "csv");
            assert_eq!(
                reply["error"]["code"], "native_tables_export_refused",
                "{text:?}: {reply}"
            );
            assert!(bytes.is_none());
            let (reply, _) = export(&mut host, "resident/0", "s", "xlsx");
            // XML-illegal cell Text is refused rather than corrupted.
            assert_eq!(
                reply["status"],
                if text.contains('\u{1}') && !header {
                    "error"
                } else {
                    "ok"
                }
            );
        }
    }
}

#[test]
fn mixed_document_refuses_xlsx_and_allows_only_eligible_csv_collections() {
    let mut document = fixture();
    add_table(
        &mut document,
        "formula",
        "formula",
        "Calculated",
        vec![field("n", FieldType::Number, true)],
        vec![vec![Some(Value::Formula(Expression::Number(
            Number::new(3.0).unwrap(),
        )))]],
    );
    add_table(
        &mut document,
        "reference",
        "reference",
        "References",
        vec![field(
            "ref",
            FieldType::Reference {
                schema: SchemaId::from("z-id"),
            },
            false,
        )],
        vec![],
    );
    let mut host = host_for(document);
    for collection in ["alpha", "formula", "reference"] {
        let (reply, bytes) = export(&mut host, "resident/0", collection, "xlsx");
        assert_eq!(
            reply["error"]["diagnostics"][0]["code"],
            "native_tables_unmapped_collection"
        );
        assert!(bytes.is_none());
    }
    let (reply, bytes) = export(&mut host, "resident/0", "alpha", "csv");
    assert_eq!(reply["status"], "ok");
    assert!(bytes.is_some());
    assert_finding(
        &reply,
        "native_tables_csv_collection_excluded",
        "lossy_on_export",
    );
    let (reply, bytes) = export(&mut host, "resident/0", "formula", "csv");
    assert_eq!(reply["status"], "ok", "{reply}");
    assert_eq!(bytes.unwrap(), b"n\r\n3\r\n");
    for collection in ["reference", "missing"] {
        let (reply, bytes) = export(&mut host, "resident/0", collection, "csv");
        assert_eq!(reply["status"], "error");
        assert!(bytes.is_none());
    }
}

fn geometry(tables: usize, columns: usize, rows: usize) -> Document {
    let mut document = Document::empty("bounds", "Bounds");
    for table in 0..tables {
        add_table(
            &mut document,
            &format!("s{table:02}"),
            &format!("s{table:02}"),
            &format!("Table {table}"),
            (0..columns)
                .map(|c| {
                    field(
                        &char::from(if c < 10 {
                            b'0' + u8::try_from(c).unwrap()
                        } else {
                            b'a' + u8::try_from(c - 10).unwrap()
                        })
                        .to_string(),
                        FieldType::Number,
                        true,
                    )
                })
                .collect(),
            (0..rows)
                .map(|_| {
                    (0..columns)
                        .map(|_| Some(Value::Number(Number::new(1.0).unwrap())))
                        .collect()
                })
                .collect(),
        );
    }
    document.entities = std::mem::take(&mut document.entities)
        .into_values()
        .enumerate()
        .map(|(index, mut entity)| {
            let id = format!("{index:x}");
            entity.id = EntityId::from(id.clone());
            entity.key = EntityKey::from(id);
            (entity.id.clone(), entity)
        })
        .collect();
    document
}

#[test]
fn native_geometry_exact_bounds_export_without_truncation_and_plus_one_is_refused_at_admission() {
    for (tables, columns, rows) in [(32, 1, 0), (1, 32, 1), (1, 1, 128)] {
        let document = geometry(tables, columns, rows);
        let runtime = DesignerRuntime::from_document(document.clone(), OCCURRENCE).unwrap();
        let workbook = runtime
            .export_native_tables_workbook("resident/0", "s00")
            .unwrap();
        assert_eq!(workbook.sheets.len(), tables);
        assert_eq!(
            workbook.sheets.iter().map(|s| s.rows.len()).sum::<usize>(),
            tables * rows
        );
        assert!(workbook.sheets.iter().all(|s| s.columns.len() == columns));
        let mut host = host_for(document);
        let (reply, bytes) = export(&mut host, "resident/0", "s00", "xlsx");
        assert_eq!(reply["status"], "ok", "{reply}");
        assert!(bytes.is_some());
        // Incoming limits remain deliberately narrower.
        if tables > 4 || columns > 16 {
            assert!(import_xlsx(&bytes.unwrap()).is_err());
        }
        let (reply, bytes) = export(&mut host, "resident/0", "s00", "csv");
        assert_eq!(reply["status"], "ok", "{reply}");
        assert!(bytes.is_some());
    }
    // Numeric/Text capacity can admit 129 rows, but this outbound profile must
    // still refuse rather than truncate or inherit that import exception.
    let document = geometry(1, 1, 129);
    let runtime = DesignerRuntime::from_document(document.clone(), OCCURRENCE).unwrap();
    assert!(
        runtime
            .export_native_tables_workbook("resident/0", "s00")
            .is_err()
    );
    let mut host = DesignerHost::new();
    let imported = host.spreadsheet(
        &serde_json::to_vec(&json!({
            "type":"import", "format":"csv", "csv_options":{"delimiter":",", "header":true},
            "selection":{"column_types":[["number"]], "extra_columns":[[]]},
            "occurrence_id":OCCURRENCE, "install":true,
        }))
        .unwrap(),
        format!("n\n{}", "1\n".repeat(129)).as_bytes(),
    );
    let imported: Json = serde_json::from_slice(&imported.reply).unwrap();
    assert_eq!(imported["status"], "ok", "{imported}");
    let selected = imported["response"]["payload"]["opened"]["bootstrap"]["default_collection"]
        .as_str()
        .unwrap();
    for format in ["csv", "xlsx"] {
        let (reply, bytes) = export(&mut host, "resident/0", selected, format);
        assert_eq!(reply["error"]["code"], "native_tables_export_refused");
        assert!(bytes.is_none());
        assert_eq!(
            reply["error"]["diagnostics"][0]["code"],
            if format == "xlsx" {
                "native_tables_xlsx_grid"
            } else {
                "native_tables_output_refused"
            }
        );
    }
    // Admission's complete post-edit refresh cap is stricter than nominal
    // total-entity geometry. The dedicated writer separately proves 1,024.
    assert!(DesignerRuntime::from_document(geometry(8, 1, 128), OCCURRENCE).is_err());
    for document in [geometry(33, 1, 0), geometry(1, 33, 1), geometry(9, 1, 128)] {
        assert!(DesignerRuntime::from_document(document, OCCURRENCE).is_err());
    }
}

#[test]
fn revision_resident_and_history_survive_exports_and_refusals_and_ledger_names_derived_exclusions()
{
    let mut runtime = DesignerRuntime::from_document(fixture(), OCCURRENCE).unwrap();
    let DesignerResponse::Published(p) = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/0".into(),
            target: tachiko_designer_runtime::FieldTarget {
                entity: "z-id-r000".into(),
                field: "a-text".into(),
            },
            input: tachiko_designer_runtime::ScalarEditInput::Text {
                value: "edited".into(),
            },
        })
        .unwrap()
    else {
        panic!("published")
    };
    let revision = p.resulting_revision;
    let before = runtime.export_project_v3(&revision).unwrap().bytes;
    let occurrence = runtime.observe_occurrence();
    runtime
        .export_native_tables_workbook(&revision, "alpha")
        .unwrap();
    runtime
        .export_native_tables_csv_workbook(&revision, "alpha")
        .unwrap();
    assert_eq!(
        runtime
            .export_native_tables_workbook("resident/0", "alpha")
            .unwrap_err()
            .failure_projection(&revision)
            .code,
        "stale_revision"
    );
    assert!(
        runtime
            .export_native_tables_workbook(&revision, "missing")
            .is_err()
    );
    assert_eq!(runtime.observe_occurrence(), occurrence);
    assert_eq!(runtime.export_project_v3(&revision).unwrap().bytes, before);
    let DesignerResponse::Published(p) = runtime
        .handle(DesignerRequest::Undo {
            expected_revision: revision,
        })
        .unwrap()
    else {
        panic!("undo")
    };
    let undone = runtime
        .export_native_tables_workbook(&p.resulting_revision, "alpha")
        .unwrap();
    assert!(
        matches!(&undone.sheets[0].rows[0][0].value, SourceValue::Text {value} if value.starts_with("000123"))
    );
    let DesignerResponse::Published(p) = runtime
        .handle(DesignerRequest::Redo {
            expected_revision: p.resulting_revision,
        })
        .unwrap()
    else {
        panic!("redo")
    };
    assert_eq!(
        runtime
            .export_native_tables_workbook(&p.resulting_revision, "alpha")
            .unwrap()
            .sheets[0]
            .rows[0][0]
            .value,
        SourceValue::Text {
            value: "edited".into()
        }
    );
}

#[test]
fn saved_grouped_sum_definitions_and_results_are_named_exclusions() {
    let mut runtime = DesignerRuntime::tracker(OCCURRENCE).unwrap();
    // Saved summary has no row identity and must never become a worksheet.
    let response = runtime
        .handle(DesignerRequest::CreateKeyedGroupedSum {
            expected_revision: "resident/0".into(),
            definition: KeyedGroupedSumDefinitionInput {
                id: "saved-summary".into(),
                orders_schema: "tracker".into(),
                order_lookup_key_field: "task".into(),
                order_quantity_field: "estimate".into(),
                products_schema: "tracker".into(),
                product_key_field: "task".into(),
                product_category_field: "task".into(),
                product_price_field: "estimate".into(),
            },
        })
        .unwrap();
    assert!(matches!(
        response,
        DesignerResponse::KeyedGroupedSumPublished(_)
    ));
    let workbook = runtime
        .export_native_tables_workbook(&runtime.observe_occurrence().revision, "tracker")
        .unwrap();
    assert_eq!(workbook.sheets.len(), 1);
    assert!(
        workbook
            .ledger
            .iter()
            .any(|f| f.code == "native_tables_saved_grouped_sum_excluded"
                && f.location == "saved-summary")
    );
}

#[test]
fn oversized_mapping_receipt_is_refused_with_a_finding_and_no_bytes() {
    let mut document = Document::empty("receipt-bounds", "Receipt bounds");
    let mut fields: Vec<_> = (0..32)
        .map(|i| field(&format!("f{i}"), FieldType::Text, true))
        .collect();
    for (index, f) in fields.iter_mut().enumerate() {
        // Distinct semantic names which collide under the output's case fold.
        let suffix: String = (0..5)
            .map(|bit| if index & (1 << bit) == 0 { 'a' } else { 'A' })
            .collect();
        f.display_name = Some(format!("{}{suffix}", "Header".repeat(230)));
    }
    add_table(&mut document, "s", "s", "Table", fields, vec![]);
    let mut host = host_for(document);
    for format in ["csv", "xlsx"] {
        let (reply, bytes) = export(&mut host, "resident/0", "s", format);
        assert_eq!(
            reply["error"]["code"], "native_tables_export_refused",
            "{reply}"
        );
        assert_eq!(
            reply["error"]["diagnostics"][0]["code"],
            "native_tables_receipt_bounds"
        );
        assert!(bytes.is_none());
    }
}

fn literal_text_document(text: &str, location: &str) -> Document {
    let mut document = Document::empty("xlsx-text", "XLSX Text");
    let mut column = field("text", FieldType::Text, true);
    if location == "header" {
        column.display_name = Some(text.into());
    }
    add_table(
        &mut document,
        "s",
        "s",
        if location == "sheet-name" {
            text
        } else {
            "Table"
        },
        vec![column],
        vec![vec![Some(Value::Text(
            if location == "cell" { text } else { "safe" }.into(),
        ))]],
    );
    document
}

#[test]
fn native_tables_xlsx_refuses_literal_spreadsheetml_escapes_at_each_location_only() {
    for text in ["_x000D_", "prefix_xabcd_suffix", "_xABCD_", "_xAbCd_"] {
        for location in ["cell", "header", "sheet-name"] {
            let document = literal_text_document(text, location);
            let runtime = DesignerRuntime::from_document(document.clone(), OCCURRENCE).unwrap();
            let error = runtime
                .export_native_tables_workbook("resident/0", "s")
                .unwrap_err();
            let tachiko_designer_runtime::DesignerError::NativeTablesExport { finding } = error
            else {
                panic!("native finding")
            };
            assert!(finding.blocking);
            assert_eq!(finding.category, FidelityCategory::UnsupportedSafeDisabled);
            assert_eq!(finding.code, "native_tables_xlsx_escape_sequence");
            if location == "cell" {
                for coordinate in ["sheet 1", "row 2", "column 1"] {
                    assert!(finding.location.contains(coordinate), "{finding:?}");
                }
            } else {
                assert!(finding.location.contains(location), "{finding:?}");
            }
            let mut host = host_for(document);
            let (reply, bytes) = export(&mut host, "resident/0", "s", "xlsx");
            assert_eq!(reply["error"]["code"], "native_tables_export_refused");
            assert_eq!(reply["error"]["diagnostics"][0]["path"], finding.location);
            assert!(bytes.is_none());
            let (reply, bytes) = export(&mut host, "resident/0", "s", "csv");
            assert_eq!(reply["status"], "ok", "{reply}");
            if location != "sheet-name" {
                assert!(String::from_utf8(bytes.unwrap()).unwrap().contains(text));
            }
        }
    }
    // A source sequence must not disappear silently through name truncation.
    let runtime = DesignerRuntime::from_document(
        literal_text_document(&format!("{}_x000D_", "s".repeat(31)), "sheet-name"),
        OCCURRENCE,
    )
    .unwrap();
    assert!(
        runtime
            .export_native_tables_workbook("resident/0", "s")
            .is_err()
    );
    // Sanitization can also create a sequence; validate the emitted name too.
    let runtime =
        DesignerRuntime::from_document(literal_text_document("_x000D[", "sheet-name"), OCCURRENCE)
            .unwrap();
    assert!(
        runtime
            .export_native_tables_workbook("resident/0", "s")
            .is_err()
    );
}

#[test]
fn native_tables_xlsx_exports_escape_near_misses_unchanged() {
    for text in ["_x00D_", "_x000G_", "_x12345_", "_X000D_"] {
        for location in ["cell", "header", "sheet-name"] {
            let mut host = host_for(literal_text_document(text, location));
            let (reply, bytes) = export(&mut host, "resident/0", "s", "xlsx");
            assert_eq!(reply["status"], "ok", "{text}: {reply}");
            let bytes = bytes.unwrap();
            let path = if location == "sheet-name" {
                "xl/workbook.xml"
            } else {
                "xl/worksheets/sheet1.xml"
            };
            assert!(xml(&bytes, path).contains(text));
        }
    }
}

#[test]
fn native_tables_xlsx_text_limit_has_a_blocking_cell_finding_and_csv_is_unchanged() {
    for units in [32_767, 32_768] {
        let mut host = DesignerHost::new();
        let opened = host.request(
            &serde_json::to_vec(&json!({"type":"new_tracker", "occurrence_id":OCCURRENCE}))
                .unwrap(),
        );
        assert_eq!(
            serde_json::from_slice::<Json>(&opened).unwrap()["status"],
            "ok"
        );
        let pasted = host.request(&serde_json::to_vec(&json!({"type":"paste_cells", "expected_revision":"resident/0",
            "collection":"tracker", "start_entity":null, "start_field":"task", "rows":[["x".repeat(units), "0", "false"]]})).unwrap());
        assert_eq!(
            serde_json::from_slice::<Json>(&pasted).unwrap()["status"],
            "ok"
        );
        let (reply, bytes) = export(&mut host, "resident/1", "tracker", "xlsx");
        if units == 32_767 {
            assert_eq!(reply["status"], "ok", "{reply}");
            assert!(bytes.is_some());
        } else {
            assert!(bytes.is_none());
            assert_eq!(
                reply["error"]["diagnostics"][0]["code"],
                "native_tables_xlsx_text_limit"
            );
            let path = reply["error"]["diagnostics"][0]["path"].as_str().unwrap();
            for coordinate in ["sheet 1", "row 2", "column 1"] {
                assert!(path.contains(coordinate), "{reply}");
            }
        }
        let (reply, bytes) = export(&mut host, "resident/1", "tracker", "csv");
        assert_eq!(reply["status"], "ok", "{reply}");
        assert!(
            bytes
                .unwrap()
                .windows(units)
                .any(|value| value == "x".repeat(units).as_bytes())
        );
    }
}

fn assert_caseless_name_ledger(workbook: &SourceWorkbook, labels: &[&str], expected: &[&str]) {
    for (sheet, (source, output)) in labels.iter().zip(expected).enumerate() {
        let mapping = workbook
            .ledger
            .iter()
            .find(|finding| {
                finding.code == "native_tables_worksheet_name"
                    && finding.location == format!("s{sheet}")
            })
            .unwrap();
        assert_eq!(mapping.category, FidelityCategory::Converted);
        assert!(!mapping.blocking);
        assert!(mapping.message.contains(&format!("{source:?}")));
        assert!(mapping.message.contains(&format!("{output:?}")));
        for (column, (source, output)) in labels.iter().zip(expected).enumerate() {
            if source == output {
                continue;
            }
            let mapping = workbook
                .ledger
                .iter()
                .find(|finding| {
                    finding.code == "native_tables_header_name"
                        && finding.location == format!("s{sheet}/f{column}")
                })
                .unwrap();
            assert_eq!(mapping.category, FidelityCategory::Converted);
            assert!(!mapping.blocking);
            assert!(mapping.message.contains(&format!("{source:?}")));
            assert!(mapping.message.contains(&format!("{output:?}")));
        }
    }
}

#[test]
fn native_tables_unicode_caseless_names_and_headers_are_ledgered_deterministically() {
    for (labels, expected) in [
        (vec!["Σ", "ς"], vec!["Σ", "ς (2)"]),
        (vec!["σ", "ς", "Σ"], vec!["σ", "ς (2)", "Σ (3)"]),
        (vec!["ß", "ẞ"], vec!["ß", "ẞ (2)"]),
        (vec!["ẞ", "ß"], vec!["ẞ", "ß (2)"]),
        (vec!["ß", "SS", "ss"], vec!["ß", "SS (2)", "ss (3)"]),
        (vec!["İ", "I", "ı", "i"], vec!["İ", "I", "ı (2)", "i (3)"]),
        (
            vec!["Sales", "sales", "SALES"],
            vec!["Sales", "sales (2)", "SALES (3)"],
        ),
    ] {
        let mut document = Document::empty("caseless", "Caseless names");
        for (index, label) in labels.iter().enumerate() {
            let columns = labels
                .iter()
                .enumerate()
                .map(|(i, label)| {
                    let mut column = field(&format!("f{i}"), FieldType::Text, true);
                    column.display_name = Some((*label).into());
                    column
                })
                .collect();
            add_table(
                &mut document,
                &format!("s{index}"),
                &format!("s{index}"),
                label,
                columns,
                vec![],
            );
        }
        let runtime = DesignerRuntime::from_document(document.clone(), OCCURRENCE).unwrap();
        let workbook = runtime
            .export_native_tables_workbook("resident/0", "s0")
            .unwrap();
        assert_eq!(
            workbook,
            runtime
                .export_native_tables_workbook("resident/0", "s0")
                .unwrap()
        );
        assert_eq!(
            workbook
                .sheets
                .iter()
                .map(|sheet| sheet.name.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        for sheet in &workbook.sheets {
            assert_eq!(
                sheet
                    .columns
                    .iter()
                    .map(|column| column.name.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
        }
        let changed_headers = labels
            .iter()
            .zip(&expected)
            .filter(|(source, output)| source != output)
            .count();
        assert_eq!(
            workbook
                .ledger
                .iter()
                .filter(|f| f.code == "native_tables_header_name")
                .count(),
            changed_headers * labels.len()
        );
        assert!(
            workbook
                .ledger
                .iter()
                .filter(|f| f.code == "native_tables_worksheet_name"
                    || f.code == "native_tables_header_name")
                .all(|f| f.category == FidelityCategory::Converted && !f.blocking)
        );
        assert_caseless_name_ledger(&workbook, &labels, &expected);
        let csv = runtime
            .export_native_tables_csv_workbook("resident/0", "s0")
            .unwrap();
        assert_eq!(csv.sheets[0].columns, workbook.sheets[0].columns);
        let mut host = host_for(document);
        let (_, first) = export(&mut host, "resident/0", "s0", "xlsx");
        let (_, second) = export(&mut host, "resident/0", "s0", "xlsx");
        assert!(first.is_some());
        assert_eq!(first, second);
    }
}

fn native_number_document(values: &[f64]) -> Document {
    let mut document = Document::empty("numbers", "Numbers");
    add_table(
        &mut document,
        "s",
        "s",
        "Numbers",
        vec![field("n", FieldType::Number, true)],
        values
            .iter()
            .map(|value| vec![Some(Value::Number(Number::new(*value).unwrap()))])
            .collect(),
    );
    document
}

#[test]
fn native_tables_xlsx_subnormal_refusals_are_located_and_csv_retains_the_value() {
    let below = f64::from_bits(f64::MIN_POSITIVE.to_bits() - 1);
    for value in [
        below,
        f64::MIN_POSITIVE,
        -f64::MIN_POSITIVE,
        -below,
        0.0,
        -0.0,
    ] {
        let document = native_number_document(&[value]);
        let runtime = DesignerRuntime::from_document(document.clone(), OCCURRENCE).unwrap();
        let workbook = runtime.export_native_tables_workbook("resident/0", "s");
        let accepted = value == 0.0 || value.abs() >= f64::MIN_POSITIVE;
        if accepted {
            assert!(workbook.is_ok());
        } else {
            let tachiko_designer_runtime::DesignerError::NativeTablesExport { finding } =
                workbook.unwrap_err()
            else {
                panic!("blocking finding")
            };
            assert!(finding.blocking);
            assert_eq!(finding.category, FidelityCategory::UnsupportedSafeDisabled);
            assert_eq!(finding.code, "native_tables_xlsx_subnormal_number");
            for coordinate in ["sheet 1", "row 2", "column 1"] {
                assert!(finding.location.contains(coordinate));
            }
        }
        let mut host = host_for(document);
        let (reply, bytes) = export(&mut host, "resident/0", "s", "xlsx");
        if accepted {
            assert_eq!(reply["status"], "ok", "{reply}");
            if value == 0.0 {
                assert!(xml(&bytes.unwrap(), "xl/worksheets/sheet1.xml").contains("<v>0</v>"));
            }
        } else {
            assert_eq!(
                reply["error"]["diagnostics"][0]["code"],
                "native_tables_xlsx_subnormal_number"
            );
            assert!(bytes.is_none());
        }
        let (reply, bytes) = export(&mut host, "resident/0", "s", "csv");
        assert_eq!(reply["status"], "ok", "{reply}");
        let expected = if value == 0.0 {
            "0".into()
        } else {
            value.to_string()
        };
        assert!(
            String::from_utf8(bytes.unwrap())
                .unwrap()
                .ends_with(&format!("{expected}\r\n"))
        );
        assert_finding(
            &reply,
            "native_tables_csv_number_precision",
            "native_equivalent",
        );
    }
}

#[test]
fn native_tables_xlsx_precision_findings_follow_shortest_significant_digits() {
    for (value, warn) in [
        (0.300_000_000_000_000_04, true),
        (0.1, false),
        (123_456_789_012_345.0, false),
        (1_234_567_890_123_456.0, true),
        (-0.300_000_000_000_000_04, true),
        (-0.1, false),
        (1e20, false),
        (1e-20, false),
        (1.234_567_890_123_456_7e-50, true),
    ] {
        let runtime =
            DesignerRuntime::from_document(native_number_document(&[value]), OCCURRENCE).unwrap();
        let workbook = runtime
            .export_native_tables_workbook("resident/0", "s")
            .unwrap();
        let warnings: Vec<_> = workbook
            .ledger
            .iter()
            .filter(|f| f.code == "native_tables_xlsx_precision_loss")
            .collect();
        assert_eq!(warnings.len(), usize::from(warn), "{value}");
        assert_eq!(
            workbook
                .ledger
                .iter()
                .filter(|f| f.code == "native_tables_xlsx_precision_count")
                .count(),
            usize::from(warn)
        );
        for finding in warnings {
            assert_eq!(finding.category, FidelityCategory::LossyOnExport);
            assert!(!finding.blocking);
            for coordinate in ["sheet 1", "row 2", "column 1"] {
                assert!(finding.location.contains(coordinate));
            }
            assert!(finding.message.contains("15 significant decimal digits"));
        }
        let SourceValue::Number { value: actual } = workbook.sheets[0].rows[0][0].value else {
            panic!("Number")
        };
        assert_eq!(actual.to_bits(), value.to_bits());
    }
}

#[test]
fn native_tables_xlsx_precision_ledger_caps_locations_and_counts_every_affected_cell() {
    for count in [20, 21, 25] {
        let mut host = host_for(native_number_document(&vec![
            0.300_000_000_000_000_04;
            count
        ]));
        let (reply, bytes) = export(&mut host, "resident/0", "s", "xlsx");
        assert_eq!(reply["status"], "ok", "{reply}");
        assert_eq!(
            xml(&bytes.unwrap(), "xl/worksheets/sheet1.xml")
                .matches("<v>0.30000000000000004</v>")
                .count(),
            count
        );
        let ledger = reply["response"]["payload"]["ledger"].as_array().unwrap();
        let warnings: Vec<_> = ledger
            .iter()
            .filter(|f| f["code"] == "native_tables_xlsx_precision_loss")
            .collect();
        assert_eq!(warnings.len(), 20);
        for (index, finding) in warnings.iter().enumerate() {
            assert_eq!(finding["category"], "lossy_on_export");
            assert_eq!(finding["blocking"], false);
            assert!(
                finding["location"]
                    .as_str()
                    .unwrap()
                    .contains(&format!("/row {}/column 1", index + 2))
            );
        }
        let summaries: Vec<_> = ledger
            .iter()
            .filter(|f| f["code"] == "native_tables_xlsx_precision_count")
            .collect();
        assert_eq!(summaries.len(), 1);
        assert!(
            summaries[0]["message"]
                .as_str()
                .unwrap()
                .contains(&format!("{count} Number cells"))
        );
        assert!(
            summaries[0]["message"]
                .as_str()
                .unwrap()
                .contains(&format!("{} additional locations omitted", count - 20))
        );
        let (again, _) = export(&mut host, "resident/0", "s", "xlsx");
        assert_eq!(reply, again);
        let (reply, bytes) = export(&mut host, "resident/0", "s", "csv");
        assert_finding(
            &reply,
            "native_tables_csv_number_precision",
            "native_equivalent",
        );
        assert_eq!(
            String::from_utf8(bytes.unwrap())
                .unwrap()
                .matches("0.30000000000000004")
                .count(),
            count
        );
        assert!(
            !reply["response"]["payload"]["ledger"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["code"] == "native_tables_xlsx_precision_loss")
        );
    }
}

#[test]
fn native_tables_xlsx_precision_cap_and_count_are_global_across_worksheets() {
    let values = vec![0.300_000_000_000_000_04; 15];
    let mut document = native_number_document(&values);
    add_table(
        &mut document,
        "t",
        "t",
        "More Numbers",
        vec![field("n", FieldType::Number, true)],
        values
            .iter()
            .map(|value| vec![Some(Value::Number(Number::new(*value).unwrap()))])
            .collect(),
    );
    let runtime = DesignerRuntime::from_document(document, OCCURRENCE).unwrap();
    let workbook = runtime
        .export_native_tables_workbook("resident/0", "s")
        .unwrap();
    let warnings: Vec<_> = workbook
        .ledger
        .iter()
        .filter(|f| f.code == "native_tables_xlsx_precision_loss")
        .collect();
    assert_eq!(warnings.len(), 20);
    for (index, finding) in warnings.iter().enumerate() {
        let (sheet, row) = if index < 15 {
            (1, index + 2)
        } else {
            (2, index - 15 + 2)
        };
        assert!(finding.location.starts_with(&format!("sheet {sheet} ")));
        assert!(finding.location.contains(&format!("/row {row}/column 1")));
    }
    let summary = workbook
        .ledger
        .iter()
        .find(|f| f.code == "native_tables_xlsx_precision_count")
        .unwrap();
    assert_eq!(summary.category, FidelityCategory::LossyOnExport);
    assert!(!summary.blocking);
    assert!(summary.message.contains("30 Number cells"));
    assert!(summary.message.contains("10 additional locations omitted"));
}

#[test]
fn native_tables_reserved_utf16_name_allocation_has_deterministic_ledger() {
    let mut document = Document::empty("names", "Name inventory");
    let cases = [
        ("History (2)".into(), "History (2)".into()),
        ("History".into(), "History (3)".into()),
        ("hIsToRy".into(), "hIsToRy (4)".into()),
        ("😀".repeat(16), "😀".repeat(15)),
        (
            format!("{}x", "😀".repeat(16)),
            format!("{} (2)", "😀".repeat(13)),
        ),
    ];
    for (i, (source, _)) in cases.iter().enumerate() {
        let id = format!("s{i}");
        add_table(
            &mut document,
            &id,
            &id,
            source,
            vec![field("text", FieldType::Text, false)],
            vec![],
        );
    }
    let runtime = DesignerRuntime::from_document(document.clone(), OCCURRENCE).unwrap();
    let before = runtime.export_project_v3("resident/0").unwrap().bytes;
    let workbook = runtime
        .export_native_tables_workbook("resident/0", "s0")
        .unwrap();
    for (index, (source, expected)) in cases.iter().enumerate() {
        assert_eq!(workbook.sheets[index].name, *expected);
        let mapping = workbook
            .ledger
            .iter()
            .find(|f| f.code == "native_tables_worksheet_name" && f.location == format!("s{index}"))
            .unwrap();
        assert_eq!(mapping.category, FidelityCategory::Converted);
        assert!(!mapping.blocking);
        assert!(mapping.message.contains(&format!("{source:?}")));
        assert!(mapping.message.contains(&format!("{expected:?}")));
    }
    assert_eq!(
        runtime
            .export_native_tables_workbook("resident/0", "s0")
            .unwrap(),
        workbook
    );
    assert_eq!(
        runtime.export_project_v3("resident/0").unwrap().bytes,
        before
    );
    let mut host = host_for(document);
    let (reply, bytes) = export(&mut host, "resident/0", "s0", "xlsx");
    assert_eq!(reply["status"], "ok", "{reply}");
    let xml = xml(&bytes.unwrap(), "xl/workbook.xml");
    for (_, expected) in cases {
        assert!(xml.contains(&expected));
    }
}

#[test]
fn native_tables_date_and_xml_refusals_are_located_and_leave_resident_unchanged() {
    for date in ["0001-01-01", "1899-12-31"] {
        let mut document = Document::empty("dates", "Dates");
        add_table(
            &mut document,
            "s",
            "s",
            "Dates",
            vec![field("date", FieldType::Date, true)],
            vec![vec![Some(Value::Date(Date::parse(date).unwrap()))]],
        );
        let runtime = DesignerRuntime::from_document(document.clone(), OCCURRENCE).unwrap();
        let before = runtime.export_project_v3("resident/0").unwrap().bytes;
        let tachiko_designer_runtime::DesignerError::NativeTablesExport { finding } = runtime
            .export_native_tables_workbook("resident/0", "s")
            .unwrap_err()
        else {
            panic!("located finding")
        };
        assert_eq!(finding.code, "native_tables_xlsx_date_range");
        assert!(finding.blocking);
        assert_eq!(finding.location, "sheet 1 \"Dates\"/row 2/column 1");
        assert_eq!(
            runtime.export_project_v3("resident/0").unwrap().bytes,
            before
        );
        let mut host = host_for(document);
        let (reply, bytes) = export(&mut host, "resident/0", "s", "xlsx");
        assert_eq!(reply["error"]["diagnostics"][0]["code"], finding.code);
        assert_eq!(reply["error"]["diagnostics"][0]["path"], finding.location);
        assert!(bytes.is_none());
        let (reply, bytes) = export(&mut host, "resident/0", "s", "csv");
        assert_eq!(reply["status"], "ok", "{reply}");
        assert!(String::from_utf8(bytes.unwrap()).unwrap().contains(date));
    }
    for text in ["bad\u{1f}character", "bad\0character"] {
        let document = literal_text_document(text, "cell");
        let runtime = DesignerRuntime::from_document(document.clone(), OCCURRENCE).unwrap();
        let before = runtime.export_project_v3("resident/0").unwrap().bytes;
        let tachiko_designer_runtime::DesignerError::NativeTablesExport { finding } = runtime
            .export_native_tables_workbook("resident/0", "s")
            .unwrap_err()
        else {
            panic!("located finding")
        };
        assert_eq!(finding.code, "native_tables_xlsx_xml_character");
        assert!(finding.blocking);
        assert!(finding.location.contains("row 2/column 1"));
        assert_eq!(
            runtime.export_project_v3("resident/0").unwrap().bytes,
            before
        );
        let mut host = host_for(document);
        let (reply, bytes) = export(&mut host, "resident/0", "s", "xlsx");
        assert_eq!(reply["error"]["diagnostics"][0]["code"], finding.code);
        assert_eq!(reply["error"]["diagnostics"][0]["path"], finding.location);
        assert!(bytes.is_none());
    }
}

#[test]
fn native_tables_text_and_header_whitespace_survive_typed_readback() {
    let text = " \t leading\r\ntrailing \t ";
    let header = "Header 漢😀";
    let mut document = literal_text_document(text, "cell");
    document
        .schemas
        .get_mut(&SchemaId::from("s"))
        .unwrap()
        .fields
        .get_mut("text")
        .unwrap()
        .display_name = Some(header.into());
    let mut host = host_for(document);
    let (reply, bytes) = export(&mut host, "resident/0", "s", "xlsx");
    assert_eq!(reply["status"], "ok", "{reply}");
    let bytes = bytes.unwrap();
    let sheet_xml = xml(&bytes, "xl/worksheets/sheet1.xml");
    assert_eq!(sheet_xml.matches("xml:space=\"preserve\"").count(), 2);
    assert!(sheet_xml.contains("&#13;"));
    let imported = import_xlsx(&bytes).unwrap();
    assert!(
        !imported.ledger.iter().any(|f| f.blocking),
        "{:?}",
        imported.ledger
    );
    assert_eq!(imported.sheets[0].columns[0].name, header);
    assert_eq!(
        imported.sheets[0].rows[0][0].value,
        SourceValue::Text { value: text.into() }
    );
}

#[test]
fn native_tables_connector_apostrophe_whitespace_names_collide_and_are_ledgered() {
    let cases = [
        ("Foo", "Foo"),
        ("' Foo '", "Foo (2)"),
        ("'' Foo ''", "Foo (3)"),
        ("'' ''", "Table"),
        ("Table", "Table (2)"),
    ];
    let mut document = Document::empty("trim-names", "Names");
    for (i, (name, _)) in cases.iter().enumerate() {
        let id = format!("s{i}");
        add_table(
            &mut document,
            &id,
            &id,
            name,
            vec![field("text", FieldType::Text, false)],
            vec![],
        );
    }
    let runtime = DesignerRuntime::from_document(document.clone(), OCCURRENCE).unwrap();
    let mapped = runtime
        .export_native_tables_workbook("resident/0", "s0")
        .unwrap();
    for (i, (source, expected)) in cases.iter().enumerate() {
        assert_eq!(mapped.sheets[i].name, *expected);
        let finding = mapped
            .ledger
            .iter()
            .find(|f| f.code == "native_tables_worksheet_name" && f.location == format!("s{i}"))
            .unwrap();
        assert_eq!(finding.category, FidelityCategory::Converted);
        assert!(!finding.blocking);
        assert!(finding.message.contains(&format!("{source:?}")));
        assert!(finding.message.contains(&format!("{expected:?}")));
    }
    assert_eq!(
        runtime
            .export_native_tables_workbook("resident/0", "s0")
            .unwrap(),
        mapped
    );
    let mut host = host_for(document);
    let (reply, bytes) = export(&mut host, "resident/0", "s0", "xlsx");
    assert_eq!(reply["status"], "ok", "{reply}");
    let bytes = bytes.unwrap();
    let workbook_xml = xml(&bytes, "xl/workbook.xml");
    for (_, expected) in cases {
        assert!(workbook_xml.contains(&format!("name=\"{expected}\"")));
    }
}

#[test]
fn native_tables_connector_csv_refuses_xml_illegal_selected_text_with_location() {
    for text in [
        "\u{1}safe",
        "bad\0text",
        "bad\u{fffe}text",
        "bad\u{ffff}text",
    ] {
        let document = literal_text_document(text, "cell");
        let runtime = DesignerRuntime::from_document(document.clone(), OCCURRENCE).unwrap();
        let before = runtime.export_project_v3("resident/0").unwrap().bytes;
        let error = runtime
            .export_native_tables_csv_workbook("resident/0", "s")
            .unwrap_err();
        let tachiko_designer_runtime::DesignerError::NativeTablesExport { finding } = error else {
            panic!("located finding")
        };
        assert_eq!(finding.code, "native_tables_csv_xml_character");
        assert!(finding.blocking);
        assert_eq!(finding.category, FidelityCategory::UnsupportedSafeDisabled);
        assert_eq!(finding.location, "sheet 1 \"Table\"/row 2/column 1");
        assert_eq!(
            runtime.export_project_v3("resident/0").unwrap().bytes,
            before
        );
        let mut host = host_for(document);
        let (reply, bytes) = export(&mut host, "resident/0", "s", "csv");
        assert_eq!(reply["error"]["diagnostics"][0]["code"], finding.code);
        assert_eq!(reply["error"]["diagnostics"][0]["path"], finding.location);
        assert!(bytes.is_none());
    }
    let mut document = literal_text_document("\u{1}safe", "cell");
    add_table(
        &mut document,
        "safe",
        "safe",
        "Safe",
        vec![field("text", FieldType::Text, true)],
        vec![vec![Some(Value::Text("safe\tline\nnext\rfinal".into()))]],
    );
    let mut host = host_for(document);
    let (reply, bytes) = export(&mut host, "resident/0", "safe", "csv");
    assert_eq!(reply["status"], "ok", "{reply}");
    assert!(
        String::from_utf8(bytes.unwrap())
            .unwrap()
            .contains("safe\tline\nnext\rfinal")
    );
}

fn assert_constrained_export(
    runtime: &DesignerRuntime,
    expected: &[Vec<SourceValue>],
    requirement: &str,
) {
    for csv in [true, false] {
        let workbook = if csv {
            runtime
                .export_native_tables_csv_workbook("resident/0", "s")
                .unwrap()
        } else {
            runtime
                .export_native_tables_workbook("resident/0", "s")
                .unwrap()
        };
        let constrained = if csv {
            assert_eq!(workbook.sheets.len(), 1);
            &workbook.sheets[0]
        } else {
            assert_eq!(
                workbook
                    .sheets
                    .iter()
                    .map(|sheet| sheet.name.as_str())
                    .collect::<Vec<_>>(),
                ["Safe", "Constrained"]
            );
            workbook
                .sheets
                .iter()
                .find(|sheet| sheet.name == "Constrained")
                .unwrap()
        };
        assert_eq!(source_values(constrained), expected);
        assert!(
            !workbook.ledger.iter().any(|finding| finding.blocking),
            "{:?}",
            workbook.ledger
        );
        assert!(
            workbook.ledger.iter().any(|finding| {
                finding.code == "native_tables_field_constraint_excluded"
                    && finding.category == FidelityCategory::LossyOnExport
                    && finding.location == "collection s/field constrained"
                    && finding.message.contains(requirement)
                    && !finding.message.contains("TextLiteralSet")
                    && !finding.message.contains("NumberInclusiveRange")
            }),
            "{:?}",
            workbook.ledger
        );
    }
}

#[test]
fn native_tables_constrained_columns_export_their_values_and_disclose_the_rule() {
    let cases = [
        (
            FieldType::Text,
            FieldConstraint::TextLiteralSet {
                values: vec!["allowed".into()],
            },
            Value::Text("allowed".into()),
            SourceValue::Text {
                value: "allowed".into(),
            },
            "allowed values: allowed",
        ),
        (
            FieldType::Number,
            FieldConstraint::NumberInclusiveRange {
                min: Number::new(1.0).unwrap(),
                max: Number::new(10.0).unwrap(),
            },
            Value::Number(Number::new(5.0).unwrap()),
            SourceValue::Number { value: 5.0 },
            "allowed range: 1 to 10",
        ),
    ];
    for (kind, constraint, value, exported, requirement) in cases {
        for populated in [false, true] {
            let mut document = Document::empty("constraints", "Constraints");
            let mut column = field("constrained", kind.clone(), false);
            column.constraint = constraint.clone();
            add_table(
                &mut document,
                "s",
                "z-constrained",
                "Constrained",
                vec![column],
                if populated {
                    vec![vec![Some(value.clone())]]
                } else {
                    vec![]
                },
            );
            add_table(
                &mut document,
                "safe",
                "a-safe",
                "Safe",
                vec![field("text", FieldType::Text, false)],
                vec![],
            );
            let runtime = DesignerRuntime::from_document(document, OCCURRENCE).unwrap();
            let before = serde_json::to_value(runtime.observe_occurrence()).unwrap();
            let expected = if populated {
                vec![vec![exported.clone()]]
            } else {
                vec![]
            };
            assert_constrained_export(&runtime, &expected, requirement);
            let selected = runtime
                .export_native_tables_csv_workbook("resident/0", "safe")
                .unwrap();
            assert_eq!(selected.sheets.len(), 1);
            assert!(selected.ledger.iter().any(|f| f.code == "native_tables_csv_collection_excluded" && f.location == "s"));
            assert_eq!(
                serde_json::to_value(runtime.observe_occurrence()).unwrap(),
                before
            );
        }
    }
}

fn formula_document() -> Document {
    let reference = |entity: &str, field: &str| {
        Expression::Reference(tachiko_workspace_engine::FieldRef::new(entity, field))
    };
    let product = |entity: &str| Expression::Multiply {
        left: Box::new(reference(entity, "a")),
        right: Box::new(reference(entity, "b")),
    };
    let mut document = Document::empty("formula-export", "Formula export");
    add_table(
        &mut document,
        "calc",
        "calc",
        "Calculated",
        vec![
            field("a", FieldType::Number, true),
            field("b", FieldType::Number, true),
            field("total", FieldType::Number, true),
        ],
        vec![
            vec![
                Some(Value::Number(Number::new(2.0).unwrap())),
                Some(Value::Number(Number::new(3.0).unwrap())),
                Some(Value::Formula(product("calc-r000"))),
            ],
            vec![
                Some(Value::Number(Number::new(4.0).unwrap())),
                Some(Value::Number(Number::new(5.0).unwrap())),
                Some(Value::Formula(product("calc-r001"))),
            ],
        ],
    );
    add_table(
        &mut document,
        "plain",
        "plain",
        "Plain",
        vec![field("n", FieldType::Number, true)],
        vec![vec![Some(Value::Number(Number::new(7.0).unwrap()))]],
    );
    document
}

#[test]
fn native_tables_formula_cells_export_current_computed_values_to_csv_and_xlsx() {
    let mut host = host_for(formula_document());
    let (reply, bytes) = export(&mut host, "resident/0", "calc", "xlsx");
    assert_eq!(reply["status"], "ok", "{reply}");
    let bytes = bytes.unwrap();
    let sheet = xml(&bytes, "xl/worksheets/sheet1.xml");
    assert!(!sheet.contains("<f>"), "{sheet}");
    for value in ["6", "20"] {
        assert!(
            sheet.contains(&format!("t=\"n\"><v>{value}</v>")),
            "{value}: {sheet}"
        );
    }
    assert_finding(
        &reply,
        "native_tables_formula_values_only",
        "lossy_on_export",
    );
    let imported = import_xlsx(&bytes).unwrap();
    assert!(!imported.ledger.iter().any(|finding| finding.blocking));
    assert_eq!(imported.sheets.len(), 2);
    assert_eq!(
        source_values(&imported.sheets[0]),
        [
            [
                SourceValue::Number { value: 2.0 },
                SourceValue::Number { value: 3.0 },
                SourceValue::Number { value: 6.0 },
            ],
            [
                SourceValue::Number { value: 4.0 },
                SourceValue::Number { value: 5.0 },
                SourceValue::Number { value: 20.0 },
            ],
        ]
    );

    let (reply, bytes) = export(&mut host, "resident/0", "calc", "csv");
    assert_eq!(reply["status"], "ok", "{reply}");
    assert_eq!(bytes.unwrap(), b"a,b,total\r\n2,3,6\r\n4,5,20\r\n");
    assert_finding(
        &reply,
        "native_tables_formula_values_only",
        "lossy_on_export",
    );
}

#[test]
fn xlsx_blank_cells_keep_exact_positions_beside_present_zero_and_false() {
    let mut host = host_for(fixture());
    let (reply, bytes) = export(&mut host, "resident/0", "alpha", "xlsx");
    assert_eq!(reply["status"], "ok", "{reply}");
    let sheet = xml(&bytes.unwrap(), "xl/worksheets/sheet1.xml");
    for address in ["B3", "C3", "D3", "A4", "D4"] {
        assert!(
            sheet.contains(&format!("<c r=\"{address}\" s=\"0\" t=\"n\"></c>")),
            "{sheet}"
        );
    }
    assert!(
        sheet.contains("r=\"B4\" s=\"0\" t=\"n\"><v>0</v>"),
        "{sheet}"
    );
    assert!(
        sheet.contains("r=\"C2\" s=\"0\" t=\"b\"><v>0</v>"),
        "{sheet}"
    );
}
