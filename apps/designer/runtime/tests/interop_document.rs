use std::collections::BTreeMap;
use tachiko_designer_runtime::{
    CleanupOperation, CleanupPreview, DesignerRequest, DesignerResponse, DesignerRuntime,
    FieldTarget,
};
use tachiko_workspace_engine::{
    Document, Entity, EntityId, EntityKey, Expression, FieldDefinition, FieldId, FieldKey,
    FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};
const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000000";

fn fixture(dependent: bool) -> DesignerRuntime {
    let mut document = Document::empty("cleanup", "Cleanup");
    let schema = SchemaId::from("items");
    document.schemas.insert(
        schema.clone(),
        Schema {
            id: schema.clone(),
            key: SchemaKey::from("items"),
            fields: [
                ("name", FieldType::Text),
                ("first", FieldType::Text),
                ("last", FieldType::Text),
                ("number_text", FieldType::Text),
                ("amount", FieldType::Number),
                ("missing", FieldType::Text),
                ("converted", FieldType::Number),
            ]
            .into_iter()
            .map(|(name, field_type)| {
                let id = FieldId::from(name);
                (
                    id.clone(),
                    FieldDefinition {
                        id,
                        key: FieldKey::from(name),
                        field_type,
                        required: name != "missing" && name != "converted",
                    },
                )
            })
            .collect(),
        },
    );
    for index in 1..=3 {
        let id = EntityId::from(format!("r{index}"));
        let amount = if dependent && index == 3 {
            Value::Formula(Expression::Reference(FieldRef::new("r2", "amount")))
        } else {
            Value::Number(Number::new(1.0).unwrap())
        };
        document.entities.insert(
            id.clone(),
            Entity {
                id: id.clone(),
                key: EntityKey::from(id.to_string()),
                schema: schema.clone(),
                fields: BTreeMap::from([
                    (
                        FieldId::from("name"),
                        Value::Text(
                            if index == 3 {
                                "Other Person"
                            } else {
                                "  Ada Lovelace  "
                            }
                            .into(),
                        ),
                    ),
                    (FieldId::from("first"), Value::Text(String::new())),
                    (FieldId::from("last"), Value::Text(String::new())),
                    (FieldId::from("number_text"), Value::Text("12.5".into())),
                    (FieldId::from("amount"), amount),
                ]),
            },
        );
    }
    DesignerRuntime::from_document(document, OCCURRENCE).unwrap()
}
fn preview(
    runtime: &mut DesignerRuntime,
    revision: u32,
    operation: CleanupOperation,
) -> CleanupPreview {
    let DesignerResponse::CleanupPreview(preview) = runtime
        .handle(DesignerRequest::PreviewCleanup {
            expected_revision: format!("resident/{revision}"),
            operation,
        })
        .unwrap()
    else {
        panic!("expected cleanup preview")
    };
    preview
}
fn commit(runtime: &mut DesignerRuntime, preview: CleanupPreview) {
    runtime
        .handle(DesignerRequest::CommitCleanup {
            expected_revision: preview.revision,
            preview_id: preview.preview_id,
        })
        .unwrap();
}
fn text(runtime: &mut DesignerRuntime, revision: u32, target: &str) -> String {
    let DesignerResponse::Fields(fields) = runtime
        .handle(DesignerRequest::QueryFields {
            expected_revision: format!("resident/{revision}"),
            fields: vec![target.into()],
        })
        .unwrap()
    else {
        panic!("expected fields")
    };
    let Some(tachiko_designer_runtime::StoredValueProjection::Text { value }) =
        &fields.fields[0].stored
    else {
        panic!("expected text")
    };
    value.clone()
}
#[test]
fn cleanup_preview_is_read_only_and_trim_replace_split_publish_exact_changes() {
    let mut runtime = fixture(false);
    let before = runtime.export_project("resident/0").unwrap().bytes;
    let trim = preview(
        &mut runtime,
        0,
        CleanupOperation::Trim {
            fields: vec!["r1.name".into(), "r2.name".into()],
        },
    );
    assert_eq!(trim.changes.len(), 2);
    assert_eq!(runtime.export_project("resident/0").unwrap().bytes, before);
    commit(&mut runtime, trim);
    assert_eq!(text(&mut runtime, 1, "r1.name"), "Ada Lovelace");
    let replace = preview(
        &mut runtime,
        1,
        CleanupOperation::Replace {
            fields: vec!["r1.name".into()],
            find: "Ada".into(),
            replacement: "Augusta".into(),
        },
    );
    commit(&mut runtime, replace);
    let split = preview(
        &mut runtime,
        2,
        CleanupOperation::Split {
            source: "r1.name".into(),
            destinations: vec!["r1.first".into(), "r1.last".into()],
            separator: " ".into(),
        },
    );
    assert_eq!(split.changes.len(), 2);
    commit(&mut runtime, split);
    assert_eq!(text(&mut runtime, 3, "r1.name"), "Augusta Lovelace");
    assert_eq!(text(&mut runtime, 3, "r1.first"), "Augusta");
    assert_eq!(text(&mut runtime, 3, "r1.last"), "Lovelace");
}
#[test]
fn explicit_conversion_retains_source_and_updates_formula_dependencies() {
    let mut runtime = fixture(true);
    let converted = preview(
        &mut runtime,
        0,
        CleanupOperation::Convert {
            source: "r2.number_text".into(),
            destination: "r2.amount".into(),
        },
    );
    commit(&mut runtime, converted);
    assert_eq!(text(&mut runtime, 1, "r2.number_text"), "12.5");
    let DesignerResponse::Fields(fields) = runtime
        .handle(DesignerRequest::QueryFields {
            expected_revision: "resident/1".into(),
            fields: vec!["r3.amount".into()],
        })
        .unwrap()
    else {
        panic!("expected fields")
    };
    assert_eq!(
        fields.fields[0]
            .calculated
            .as_ref()
            .and_then(tachiko_designer_runtime::CalculationProjection::number),
        Some(12.5)
    );
}
#[test]
fn deduplication_uses_stable_row_order_and_rejects_dangling_formula_atomically() {
    let operation = CleanupOperation::Deduplicate {
        entities: vec!["r2".into(), "r1".into(), "r3".into()],
        key_fields: vec!["name".into()],
    };
    let mut dependent = fixture(true);
    let before = dependent.export_project("resident/0").unwrap().bytes;
    assert!(
        dependent
            .handle(DesignerRequest::PreviewCleanup {
                expected_revision: "resident/0".into(),
                operation: operation.clone()
            })
            .is_err()
    );
    assert_eq!(
        dependent.export_project("resident/0").unwrap().bytes,
        before
    );
    let mut runtime = fixture(false);
    let dedupe = preview(&mut runtime, 0, operation);
    assert_eq!(dedupe.removed_entities, vec!["r2"]);
    commit(&mut runtime, dedupe);
    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: "items".into(),
        })
        .unwrap()
    else {
        panic!("expected table")
    };
    assert_eq!(
        table
            .rows
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        vec!["r1", "r3"]
    );
}
#[test]
fn invalid_cleanup_stale_or_replaced_previews_never_publish_partial_changes() {
    let mut runtime = fixture(false);
    let before = runtime.export_project("resident/0").unwrap().bytes;
    for operation in [
        CleanupOperation::Trim {
            fields: vec!["r1.name".into(), "r2.amount".into()],
        },
        CleanupOperation::Trim {
            fields: vec!["r1.name".into(), "r1.name".into()],
        },
        CleanupOperation::Convert {
            source: "r1.name".into(),
            destination: "r1.amount".into(),
        },
        CleanupOperation::Split {
            source: "r1.name".into(),
            destinations: vec!["r1.name".into()],
            separator: " ".into(),
        },
    ] {
        assert!(
            runtime
                .handle(DesignerRequest::PreviewCleanup {
                    expected_revision: "resident/0".into(),
                    operation
                })
                .is_err()
        );
        assert_eq!(runtime.export_project("resident/0").unwrap().bytes, before);
    }
    let old = preview(
        &mut runtime,
        0,
        CleanupOperation::Trim {
            fields: vec![FieldTarget::from("r1.name")],
        },
    );
    let current = preview(
        &mut runtime,
        0,
        CleanupOperation::Trim {
            fields: vec!["r2.name".into()],
        },
    );
    assert!(
        runtime
            .handle(DesignerRequest::CommitCleanup {
                expected_revision: "resident/0".into(),
                preview_id: old.preview_id
            })
            .is_err()
    );
    commit(&mut runtime, current.clone());
    assert!(
        runtime
            .handle(DesignerRequest::CommitCleanup {
                expected_revision: "resident/0".into(),
                preview_id: current.preview_id
            })
            .is_err()
    );
    assert_eq!(text(&mut runtime, 1, "r1.name"), "  Ada Lovelace  ");
}

#[test]
fn optional_missing_fill_and_conversion_use_declared_slots_without_false_placeholders() {
    let mut runtime = fixture(false);
    let fill = preview(
        &mut runtime,
        0,
        CleanupOperation::Fill {
            fields: vec!["r1.missing".into(), "r2.missing".into(), "r1.name".into()],
            input: tachiko_designer_runtime::ScalarEditInput::Text {
                value: "Unknown".into(),
            },
        },
    );
    assert_eq!(fill.changes.len(), 2);
    assert!(fill.changes.iter().all(|change| change.before.is_none()));
    commit(&mut runtime, fill);
    assert_eq!(text(&mut runtime, 1, "r1.missing"), "Unknown");
    assert_eq!(text(&mut runtime, 1, "r1.name"), "  Ada Lovelace  ");
    let conversion = preview(
        &mut runtime,
        1,
        CleanupOperation::Convert {
            source: "r1.number_text".into(),
            destination: "r1.converted".into(),
        },
    );
    assert!(conversion.changes[0].before.is_none());
    commit(&mut runtime, conversion);
    assert_eq!(text(&mut runtime, 2, "r1.number_text"), "12.5");
}

use tachiko_designer_runtime::interop_adapter::{
    CellStyle, SourceCell, SourceColumn, SourceSheet, SourceValue, SourceWorkbook,
};
use tachiko_designer_runtime::{
    ImportFieldType, ImportSelection, import_workbook, inspect_imported_project,
};
fn source_cell(value: SourceValue, formula: Option<&str>) -> SourceCell {
    SourceCell {
        value,
        formula: formula.map(str::to_owned),
        style: CellStyle::default(),
    }
}
fn workbook_fixture() -> (SourceWorkbook, ImportSelection) {
    let row = |value, formula, label: &str| {
        vec![
            source_cell(SourceValue::Number { value }, None),
            source_cell(SourceValue::Number { value: 2.0 }, None),
            source_cell(SourceValue::Number { value: 999.0 }, formula),
            source_cell(
                SourceValue::Date {
                    value: "2026-09-05".into(),
                },
                None,
            ),
            source_cell(
                SourceValue::Text {
                    value: label.into(),
                },
                None,
            ),
        ]
    };
    let workbook = SourceWorkbook {
        ledger: Vec::new(),
        sheets: vec![
            SourceSheet {
                name: "Data".into(),
                has_header: true,
                columns: ["Input", "Divisor", "Result", "Due", "Label"]
                    .into_iter()
                    .map(|name| SourceColumn {
                        name: name.into(),
                        width: None,
                    })
                    .collect(),
                rows: vec![
                    row(8.0, Some("C4*2"), "duplicate"),
                    row(7.0, None, "duplicate"),
                    row(10.0, Some("A4/B4"), "keep"),
                ],
            },
            SourceSheet {
                name: "Calculations".into(),
                has_header: true,
                columns: vec![SourceColumn {
                    name: "Total".into(),
                    width: None,
                }],
                rows: vec![vec![source_cell(
                    SourceValue::Number { value: 9999.0 },
                    Some("'Data'!C2+MIN('Data'!$C$4,5)"),
                )]],
            },
        ],
    };
    let selection = ImportSelection {
        column_types: vec![
            vec![
                ImportFieldType::Number,
                ImportFieldType::Number,
                ImportFieldType::Number,
                ImportFieldType::Date,
                ImportFieldType::Text,
            ],
            vec![ImportFieldType::Number],
        ],
        extra_columns: vec![vec![], vec![]],
    };
    (workbook, selection)
}
#[test]
fn imported_forward_and_cross_sheet_formulas_rebind_and_export_from_live_ids_after_dedupe() {
    let (source, selection) = workbook_fixture();
    let (mut runtime, imported) = import_workbook(&source, &selection, OCCURRENCE).unwrap();
    let metadata = imported.metadata;
    let exported = runtime.export_workbook("resident/0", &metadata).unwrap();
    assert_eq!(
        exported.sheets[0].rows[0][2].value,
        SourceValue::Number { value: 10.0 }
    );
    assert_eq!(
        exported.sheets[1].rows[0][0].value,
        SourceValue::Number { value: 15.0 }
    );
    assert_eq!(
        exported.sheets[0].rows[2][3].value,
        SourceValue::Date {
            value: "2026-09-05".into()
        }
    );
    let sheet = &metadata.sheets[0];
    let dedupe = preview(
        &mut runtime,
        0,
        CleanupOperation::Deduplicate {
            entities: sheet.rows.iter().map(|row| row.entity_id.clone()).collect(),
            key_fields: vec![sheet.columns[4].field_id.clone()],
        },
    );
    commit(&mut runtime, dedupe);
    let exported = runtime.export_workbook("resident/1", &metadata).unwrap();
    assert_eq!(exported.sheets[0].rows.len(), 2);
    assert_eq!(
        exported.sheets[0].rows[0][2].formula.as_deref(),
        Some("('Data'!$C$3*2)")
    );
    assert_eq!(
        exported.sheets[1].rows[0][0].formula.as_deref(),
        Some("('Data'!$C$2+MIN('Data'!$C$3,5))")
    );
    let bytes = runtime.export_project("resident/1").unwrap().bytes;
    let inspected = inspect_imported_project(&bytes, &metadata).unwrap();
    assert_eq!(inspected.bootstrap.revision, "resident/0");
    let (reimported, _) = import_workbook(
        &exported,
        &selection,
        "00000000-0000-4000-8000-000000000001",
    )
    .unwrap();
    assert!(reimported.export_project("resident/0").is_ok());
}
#[test]
fn imported_formula_cycles_invalid_selection_and_foreign_metadata_fail_closed() {
    let (mut source, selection) = workbook_fixture();
    source.sheets[0].rows[2][2].formula = Some("C2".into());
    assert!(import_workbook(&source, &selection, OCCURRENCE).is_err());
    let (source, mut selection) = workbook_fixture();
    selection.column_types[0][2] = ImportFieldType::Text;
    assert!(import_workbook(&source, &selection, OCCURRENCE).is_err());
    let (source, selection) = workbook_fixture();
    let (runtime, mut imported) = import_workbook(&source, &selection, OCCURRENCE).unwrap();
    let before = runtime.export_project("resident/0").unwrap().bytes;
    imported.metadata.sheets[1].rows[0].entity_id =
        imported.metadata.sheets[0].rows[0].entity_id.clone();
    assert!(
        runtime
            .export_workbook("resident/0", &imported.metadata)
            .is_err()
    );
    assert!(inspect_imported_project(&before, &imported.metadata).is_err());
    assert_eq!(runtime.export_project("resident/0").unwrap().bytes, before);
}

#[test]
fn import_keeps_text_and_true_absence_until_explicit_selection_and_extra_output_slots() {
    let workbook = SourceWorkbook {
        ledger: vec![],
        sheets: vec![SourceSheet {
            name: "Raw CSV".into(),
            has_header: true,
            columns: vec![
                SourceColumn {
                    name: "Identifier".into(),
                    width: None,
                },
                SourceColumn {
                    name: "Ambiguous".into(),
                    width: None,
                },
            ],
            rows: vec![
                vec![
                    source_cell(
                        SourceValue::Text {
                            value: "0012".into(),
                        },
                        None,
                    ),
                    source_cell(
                        SourceValue::Text {
                            value: "03/04/2026".into(),
                        },
                        None,
                    ),
                ],
                vec![source_cell(SourceValue::Empty, None)],
            ],
        }],
    };
    let mut selection = ImportSelection {
        column_types: vec![vec![ImportFieldType::Text, ImportFieldType::Text]],
        extra_columns: vec![vec![tachiko_designer_runtime::ImportColumnSpec {
            name: "Converted".into(),
            field_type: ImportFieldType::Number,
        }]],
    };
    let (runtime, imported) = import_workbook(&workbook, &selection, OCCURRENCE).unwrap();
    let exported = runtime
        .export_workbook("resident/0", &imported.metadata)
        .unwrap();
    assert_eq!(
        exported.sheets[0].rows[0][0].value,
        SourceValue::Text {
            value: "0012".into()
        }
    );
    assert_eq!(
        exported.sheets[0].rows[0][1].value,
        SourceValue::Text {
            value: "03/04/2026".into()
        }
    );
    assert_eq!(exported.sheets[0].rows[0][2].value, SourceValue::Empty);
    assert_eq!(exported.sheets[0].rows[1][0].value, SourceValue::Empty);
    selection.column_types[0][1] = ImportFieldType::Date;
    assert!(import_workbook(&workbook, &selection, OCCURRENCE).is_err());
}

#[test]
fn formula_coordinate_translation_handles_unicode_quotes_scientific_literals_and_headers() {
    let (mut source, selection) = workbook_fixture();
    source.sheets[0].name = "資料".into();
    source.sheets[0].rows[0][2].formula = Some("1e1+C4".into());
    source.sheets[1].rows[0][0].formula = Some("資料!C2+'資料'!C4".into());
    let (runtime, imported) = import_workbook(&source, &selection, OCCURRENCE).unwrap();
    let exported = runtime
        .export_workbook("resident/0", &imported.metadata)
        .unwrap();
    assert_eq!(
        exported.sheets[1].rows[0][0].value,
        SourceValue::Number { value: 20.0 }
    );
    for invalid in [
        "A1",
        "A10000",
        "SUM(A2:A4)",
        "'Missing'!A2",
        "'unfinished!A2",
    ] {
        source.sheets[0].rows[0][2].formula = Some(invalid.into());
        assert!(
            import_workbook(&source, &selection, OCCURRENCE).is_err(),
            "{invalid}"
        );
    }
    source.sheets[0].name = "O'Brien".into();
    source.sheets[0].rows[0][2].formula = Some("C4*2".into());
    source.sheets[1].rows[0][0].formula = Some("'O''Brien'!C2".into());
    let (runtime, imported) = import_workbook(&source, &selection, OCCURRENCE).unwrap();
    assert_eq!(
        runtime
            .export_workbook("resident/0", &imported.metadata)
            .unwrap()
            .sheets[1]
            .rows[0][0]
            .formula
            .as_deref(),
        Some("'O''Brien'!$C$2")
    );
}
