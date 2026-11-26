use std::collections::BTreeMap;

use super::{
    DesignerRuntime, Document, Entity, EntityId, EntityKey, Expression, FieldDefinition, FieldId,
    FieldKey, FieldRef, FieldTarget, FieldType, FindDirection, FindInViewInput, FindPosition,
    GroupBucket, GroupDimension, GroupScope, ImportFieldType, ImportSelection, InteropMetadata,
    SemanticCommand, SessionProfile, SortDirection, TableView, Value, ViewFilter, ViewSortKey,
    ViewTextOrder, admit_imported_project_working_size, encode_portable_package_v1,
    encode_project_bundle, import_workbook, inspect_project, inspect_project_working_size,
    open_imported_project, open_local_document, open_local_document_working_size, open_portable_ro,
    open_portable_ro_working_size, open_project, open_project_working_size, save_size,
};
use crate::interop_adapter::{
    CellStyle, OutputProfile, SourceCell, SourceColumn, SourceLayout, SourceSheet, SourceValue,
    SourceWorkbook, export_csv_for_profile, export_xlsx_for_profile, import_xlsx,
};
use tachiko_semantic_core::{Number, RowRank, Schema, SchemaId, SchemaKey};
use tachiko_workspace_engine::FieldConstraint;

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000871";
const NEXT_OCCURRENCE: &str = "00000000-0000-4000-8000-000000000872";
const STORED: [&str; 5] = ["e03", "e02", "e04", "e01", "e05"];

fn field_definition(id: &str, field_type: FieldType) -> (FieldId, FieldDefinition) {
    let id = FieldId::from(id);
    let key = FieldKey::from(id.as_str());
    (
        id.clone(),
        FieldDefinition {
            id,
            key,
            display_name: None,
            field_type,
            required: false,
            constraint: FieldConstraint::None,
            number_format: None,
            date_format: None,
        },
    )
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

pub(super) fn ranked_document() -> Document {
    let mut document = Document::empty("row_order", "Row order");
    let schema = SchemaId::from("rows");
    document.schemas.insert(
        schema.clone(),
        Schema {
            headerless: false,
            id: schema.clone(),
            key: SchemaKey::from("rows"),
            display_name: None,
            field_order: None,
            fields: [
                field_definition("amount", FieldType::Number),
                field_definition("calc", FieldType::Number),
                field_definition("label", FieldType::Text),
            ]
            .into_iter()
            .collect(),
        },
    );
    for (id, rank, amount) in [
        ("e01", None, 1e16),
        ("e02", Some("B"), -1e16),
        ("e03", Some("A"), 1.0),
        ("e04", Some("B"), 0.0),
        ("e05", None, 0.0),
    ] {
        document.entities.insert(
            EntityId::from(id),
            Entity {
                id: EntityId::from(id),
                key: EntityKey::from(id),
                schema: schema.clone(),
                rank: rank.map(|rank| RowRank::new(rank).unwrap()),
                fields: BTreeMap::from([
                    (FieldId::from("amount"), number(amount)),
                    (
                        FieldId::from("calc"),
                        Value::Formula(Expression::Reference(FieldRef::new("e02", "amount"))),
                    ),
                    (
                        FieldId::from("label"),
                        Value::Text(if id == "e04" { "hidden" } else { "same" }.into()),
                    ),
                ]),
            },
        );
    }
    document
}

fn open_fixture(document: Document) -> DesignerRuntime {
    DesignerRuntime::from_document(document, OCCURRENCE).unwrap()
}

fn ids(runtime: &DesignerRuntime) -> Vec<String> {
    runtime
        .query_table("rows")
        .unwrap()
        .rows
        .into_iter()
        .map(|row| row.id)
        .collect()
}

fn target(entity: &str, field: &str) -> FieldTarget {
    FieldTarget {
        entity: entity.into(),
        field: field.into(),
    }
}

fn filtered_view() -> TableView {
    TableView {
        sort: vec![ViewSortKey {
            field: "label".into(),
            direction: SortDirection::Asc,
            text_order: ViewTextOrder::Natural,
        }],
        filters: vec![ViewFilter::TextEquals {
            field: "label".into(),
            value: "same".into(),
        }],
        keep_entities: Vec::new(),
    }
}

#[test]
fn ranked_windows_locate_find_and_view_ties_share_positions() {
    let mut current = open_fixture(ranked_document());
    let revision = current.current_revision().to_owned();
    let window = current
        .query_table_window(&revision, "rows", &TableView::default(), 0, 5, true)
        .unwrap();
    assert_eq!(
        window
            .rows
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        STORED
    );
    assert_eq!(
        window
            .rows
            .iter()
            .map(|row| row.position)
            .collect::<Vec<_>>(),
        [Some(1), Some(2), Some(3), Some(4), Some(5)]
    );
    let view = filtered_view();
    let expected = [("e03", 1), ("e02", 2), ("e01", 4), ("e05", 5)];
    let filtered = current
        .query_table_window(&revision, "rows", &view, 0, 5, true)
        .unwrap();
    assert_eq!(
        filtered
            .rows
            .iter()
            .map(|row| (row.id.as_str(), row.position.unwrap()))
            .collect::<Vec<_>>(),
        expected
    );
    let fields = ["label".to_owned()];
    let mut from = None;
    for (index, (id, position)) in expected.iter().enumerate() {
        let located = current
            .locate_in_view(&revision, "rows", &view, id)
            .unwrap();
        assert_eq!(
            (located.index, located.position),
            (Some(index), Some(*position))
        );
        let reply = current
            .find_in_view(FindInViewInput {
                expected_revision: &revision,
                collection: "rows",
                view: &view,
                query: "same",
                fields: &fields,
                from: from.as_ref(),
                direction: FindDirection::Next,
            })
            .unwrap();
        assert_eq!(reply.total, 4);
        let found = reply.found.unwrap();
        assert_eq!(
            (found.entity.as_str(), found.position, found.row),
            (*id, *position, index)
        );
        from = Some(FindPosition {
            row: found.row,
            field: found.field,
        });
    }
    assert_eq!(
        current
            .locate_in_view(&revision, "rows", &view, "e04")
            .unwrap()
            .position,
        None
    );
    let distinct = current
        .query_distinct_values(&revision, "rows", &view, "label", 10)
        .unwrap();
    assert_eq!(distinct.values, ["same"]);
    let previous = current
        .find_in_view(FindInViewInput {
            expected_revision: &revision,
            collection: "rows",
            view: &view,
            query: "same",
            fields: &fields,
            from: Some(&FindPosition {
                row: 0,
                field: "label".into(),
            }),
            direction: FindDirection::Previous,
        })
        .unwrap()
        .found
        .unwrap();
    assert_eq!((previous.entity.as_str(), previous.position), ("e05", 5));
}

#[test]
fn ranked_query_table_and_capacity_rows_follow_stored_order() {
    let current = open_fixture(ranked_document());
    assert_eq!(ids(&current), STORED);
    assert!(
        current
            .query_table("rows")
            .unwrap()
            .rows
            .iter()
            .all(|row| row.position.is_none())
    );
    let (mut capacity, metadata) = imported_fixture(true);
    assert_eq!(capacity.profile, SessionProfile::TextCapacity);
    let document = capacity.session.document().clone();
    let key = capacity.default_collection.clone();
    let spec = &capacity.collection_specs[&key];
    let expected = spec
        .entities
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let canonical = document
        .entities
        .keys()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    assert_ne!(expected, canonical);
    assert_eq!(
        capacity
            .query_table(&key)
            .unwrap()
            .rows
            .iter()
            .map(|row| row.id.clone())
            .collect::<Vec<_>>(),
        expected
    );
    let revision = capacity.current_revision().to_owned();
    let window = capacity
        .query_table_window(&revision, &key, &TableView::default(), 0, 3, false)
        .unwrap();
    assert_eq!(
        window
            .rows
            .iter()
            .map(|row| row.id.clone())
            .collect::<Vec<_>>(),
        expected[..3]
    );
    assert_eq!(
        window.rows[0].fields[0].stored,
        Some(super::StoredValueProjection::Text {
            value: "row128-0".into()
        })
    );
    assert!(capacity.export_workbook(&revision, &metadata).is_ok());
    let mut invalid = metadata;
    invalid.sheets[0].rows[0] = invalid.sheets[0].rows[1].clone();
    assert!(capacity.export_workbook(&revision, &invalid).is_err());
}

#[test]
fn ranked_paste_and_relative_formula_copy_use_displayed_neighbours() {
    let mut current = open_fixture(ranked_document());
    let indices = current.collection_specs["rows"].entities.as_ptr();
    current
        .paste_cells(
            "resident/0",
            "rows",
            Some("e03"),
            "label",
            &[vec!["first".into()], vec!["second".into()]],
        )
        .unwrap();
    let document = current.session.document();
    assert_eq!(
        document.entities["e03"].fields[&FieldId::from("label")],
        Value::Text("first".into())
    );
    assert_eq!(
        document.entities["e02"].fields[&FieldId::from("label")],
        Value::Text("second".into())
    );
    assert_eq!(
        document.entities["e04"].fields[&FieldId::from("label")],
        Value::Text("hidden".into())
    );
    assert_eq!(current.collection_specs["rows"].entities.as_ptr(), indices);
    current
        .copy_formula(
            "resident/1",
            &target("e03", "calc"),
            &[target("e02", "calc")],
            &[],
            true,
            false,
        )
        .unwrap();
    assert_eq!(
        current.session.document().entities["e02"].fields[&FieldId::from("calc")],
        Value::Formula(Expression::Reference(FieldRef::new("e04", "amount")))
    );
    assert_eq!(ids(&current), STORED);
    let saved = current
        .export_project_v3(current.current_revision())
        .unwrap();
    let mut reopened = None;
    open_project(&mut reopened, &saved.bytes, NEXT_OCCURRENCE).unwrap();
    assert_eq!(
        reopened.as_ref().unwrap().session.document(),
        current.session.document()
    );
    assert_eq!(ids(reopened.as_ref().unwrap()), STORED);
    assert_duplicate_order(&mut current);
}

fn assert_duplicate_order(current: &mut DesignerRuntime) {
    let before = current.query_table("rows").unwrap();
    let revision = current.current_revision().to_owned();
    current
        .duplicate_collection(&revision, "rows", "Copy", &BTreeMap::new(), None)
        .unwrap();
    let key = current
        .collection_specs
        .keys()
        .find(|key| key.as_str() != "rows")
        .unwrap();
    let after = current.query_table(key).unwrap();
    assert_eq!(
        before
            .rows
            .iter()
            .map(|row| row.fields[0].stored.clone())
            .collect::<Vec<_>>(),
        after
            .rows
            .iter()
            .map(|row| row.fields[0].stored.clone())
            .collect::<Vec<_>>()
    );
    let document = current.session.document();
    assert_eq!(
        before
            .rows
            .iter()
            .map(|row| document.entities[row.id.as_str()].rank.clone())
            .collect::<Vec<_>>(),
        after
            .rows
            .iter()
            .map(|row| document.entities[row.id.as_str()].rank.clone())
            .collect::<Vec<_>>()
    );
}

fn totals_bits(current: &mut DesignerRuntime) -> Vec<u64> {
    let revision = current.current_revision().to_owned();
    let members = STORED.map(str::to_owned);
    let filtered = TableView {
        filters: vec![ViewFilter::NotBlank {
            field: "amount".into(),
        }],
        ..TableView::default()
    };
    let dimensions = [GroupDimension {
        field: "label".into(),
        bucket: GroupBucket::Value,
    }];
    let grouped = current
        .query_group_totals(
            &revision,
            "rows",
            &dimensions,
            &["amount".into()],
            &GroupScope::All,
        )
        .unwrap();
    let selected = current
        .query_group_totals(
            &revision,
            "rows",
            &dimensions,
            &["amount".into()],
            &GroupScope::Entities {
                entities: members[..4].to_vec(),
            },
        )
        .unwrap();
    let mut result = vec![
        current.query_table("rows").unwrap().column_totals["amount"]
            .unwrap()
            .to_bits(),
        current
            .query_table_window(&revision, "rows", &TableView::default(), 0, 0, false)
            .unwrap()
            .column_totals["amount"]
            .unwrap()
            .to_bits(),
        current
            .query_column_totals(&revision, "rows", &members)
            .unwrap()
            .column_totals["amount"]
            .unwrap()
            .to_bits(),
        current
            .query_column_totals(&revision, "rows", &members[..4])
            .unwrap()
            .column_totals["amount"]
            .unwrap()
            .to_bits(),
        current
            .query_view_totals(&revision, "rows", &filtered)
            .unwrap()
            .column_totals["amount"]
            .unwrap()
            .to_bits(),
        grouped.totals[0].unwrap().to_bits(),
        selected.totals[0].unwrap().to_bits(),
    ];
    result.extend(
        grouped
            .groups
            .iter()
            .map(|group| group.totals[0].unwrap().to_bits()),
    );
    result
}

#[test]
fn ranked_totals_keep_canonical_binary64_fold() {
    let ranked = ranked_document();
    let mut unranked = ranked.clone();
    for entity in unranked.entities.values_mut() {
        entity.rank = None;
    }
    let mut plain = open_fixture(unranked);
    let mut current = open_fixture(ranked);
    let expected = totals_bits(&mut plain);
    assert_eq!(expected[0], 1.0_f64.to_bits());
    assert_eq!(totals_bits(&mut current), expected);
    let displayed = current
        .query_table("rows")
        .unwrap()
        .rows
        .iter()
        .fold(0.0, |sum, row| {
            let Value::Number(value) = &current.session.document().entities[row.id.as_str()].fields
                [&FieldId::from("amount")]
            else {
                panic!("number")
            };
            sum + value.get()
        });
    assert_eq!(displayed.to_bits(), 0.0_f64.to_bits());
}

fn source_workbook(capacity: bool) -> SourceWorkbook {
    let count = if capacity { 129 } else { 3 };
    let width = if capacity { 3 } else { 2 };
    SourceWorkbook {
        ledger: Vec::new(),
        sheets: vec![SourceSheet {
            name: "Input".into(),
            has_header: true,
            layout: SourceLayout::default(),
            columns: (0..width)
                .map(|index| SourceColumn {
                    name: format!("c{index}"),
                    width: None,
                    number_format: None,
                })
                .collect(),
            rows: (0..count)
                .map(|row| {
                    (0..width)
                        .map(|column| SourceCell {
                            value: if capacity {
                                SourceValue::Text {
                                    value: format!("row{row}-{column}"),
                                }
                            } else {
                                SourceValue::Number {
                                    value: if column == 0 {
                                        23.0 + f64::from(row)
                                    } else {
                                        23.0
                                    },
                                }
                            },
                            formula: (!capacity && column == 1).then(|| "$A$2".into()),
                            style: CellStyle {
                                bold: !capacity && row == 0,
                                wrap: !capacity && row == 2,
                                ..CellStyle::default()
                            },
                        })
                        .collect()
                })
                .collect(),
        }],
    }
}

fn unranked_import(capacity: bool) -> (DesignerRuntime, InteropMetadata) {
    let source = source_workbook(capacity);
    let selection = ImportSelection {
        column_types: vec![vec![
            if capacity {
                ImportFieldType::Text
            } else {
                ImportFieldType::Number
            };
            source.sheets[0].columns.len()
        ]],
        extra_columns: vec![vec![]],
    };
    let (current, projection) = import_workbook(&source, &selection, OCCURRENCE).unwrap();
    (current, projection.metadata)
}

pub(super) fn imported_fixture(capacity: bool) -> (DesignerRuntime, InteropMetadata) {
    let (original, mut metadata) = unranked_import(capacity);
    let mut document = original.session.document().clone();
    let count = metadata.sheets[0].rows.len();
    // Source first becomes stored last; all ranks set directly in the fixture.
    for (index, row) in metadata.sheets[0].rows.iter().enumerate() {
        document
            .entities
            .get_mut(row.entity_id.as_str())
            .unwrap()
            .rank = Some(RowRank::new(format!("R{:04}Z", count - index)).unwrap());
    }
    // Capacity accepts complete identity coverage in a noncanonical order too.
    if capacity {
        metadata.sheets[0].rows.reverse();
    }
    let mut current =
        DesignerRuntime::from_document_profile(document, OCCURRENCE, original.profile).unwrap();
    current.set_presentation(original.presentation).unwrap();
    (current, metadata)
}

#[test]
fn ranked_v3_runtime_input_is_admitted_on_every_ingress() {
    let (current, metadata) = imported_fixture(false);
    let document = current.session.document();
    let tree = tachiko_storage::encode_roproj_v3_with_presentation(document, &current.presentation)
        .unwrap();
    let bundle = encode_project_bundle(tree.files().iter().map(|f| (f.path(), f.bytes()))).unwrap();
    let portable = encode_portable_package_v1(&tree).unwrap();
    let mut resident = None;
    open_project(&mut resident, &bundle, NEXT_OCCURRENCE).unwrap();
    assert_eq!(resident.as_ref().unwrap().session.document(), document);
    open_project_working_size(&mut resident, &bundle, OCCURRENCE).unwrap();
    assert_eq!(resident.as_ref().unwrap().session.document(), document);
    open_portable_ro(&mut resident, &portable, NEXT_OCCURRENCE).unwrap();
    assert_eq!(resident.as_ref().unwrap().session.document(), document);
    open_portable_ro_working_size(&mut resident, &portable, OCCURRENCE).unwrap();
    assert_eq!(resident.as_ref().unwrap().session.document(), document);
    open_local_document(&mut resident, &portable, NEXT_OCCURRENCE).unwrap();
    assert_eq!(resident.as_ref().unwrap().session.document(), document);
    open_local_document_working_size(&mut resident, &portable, OCCURRENCE).unwrap();
    assert_eq!(resident.as_ref().unwrap().session.document(), document);
    open_imported_project(&mut resident, &bundle, &metadata, NEXT_OCCURRENCE).unwrap();
    assert_eq!(resident.as_ref().unwrap().session.document(), document);
    let (working, _) = admit_imported_project_working_size(&bundle, OCCURRENCE, &metadata).unwrap();
    assert_eq!(working.session.document(), document);
    inspect_project(&bundle).unwrap();
    inspect_project_working_size(&bundle).unwrap();
    assert_eq!(open_fixture(document.clone()).session.document(), document);
    assert_eq!(
        DesignerRuntime::from_document_working_size(document.clone(), OCCURRENCE)
            .unwrap()
            .session
            .document(),
        document
    );
    let (capacity, _) = imported_fixture(true);
    assert!(capacity.session.document().has_row_ranks());
}

fn assert_file_encodings(workbook: &SourceWorkbook, profile: OutputProfile) {
    let xlsx = export_xlsx_for_profile(workbook, profile).unwrap();
    let decoded = import_xlsx(&xlsx).unwrap();
    assert_eq!(decoded.sheets[0].rows.len(), workbook.sheets[0].rows.len());
    for (actual, expected) in decoded.sheets[0].rows.iter().zip(&workbook.sheets[0].rows) {
        for (actual, expected) in actual.iter().zip(expected) {
            assert_eq!(actual.value, expected.value);
            assert_eq!(actual.formula, expected.formula);
            assert_eq!(actual.style, expected.style);
        }
    }
    let csv = export_csv_for_profile(&workbook.sheets[0], profile).unwrap();
    let lines = String::from_utf8(csv).unwrap();
    assert_eq!(lines.lines().count(), workbook.sheets[0].rows.len() + 1);
}

#[test]
fn ranked_native_and_imported_xlsx_csv_keep_identity_styles_and_addresses() {
    let current = open_fixture(ranked_document());
    let native = current
        .export_native_tables_workbook("resident/0", "rows")
        .unwrap();
    assert_eq!(
        native.sheets[0].rows[0][0].value,
        SourceValue::Number { value: 1.0 }
    );
    assert_eq!(
        native.sheets[0].rows[0][1].formula.as_deref(),
        Some("'rows'!A$3")
    );
    assert_eq!(
        native.sheets[0].rows[0][1].value,
        SourceValue::Number { value: -1e16 }
    );
    assert_file_encodings(&native, OutputProfile::NativeTables);
    for capacity in [false, true] {
        let (current, metadata) = imported_fixture(capacity);
        let workbook = current
            .export_workbook(current.current_revision(), &metadata)
            .unwrap();
        let mut expected = source_workbook(capacity).sheets.remove(0).rows;
        expected.reverse();
        if !capacity {
            for row in &mut expected {
                row[1].formula = Some("'Input'!$A$4".into());
            }
        }
        assert_eq!(workbook.sheets[0].rows, expected);
        assert_file_encodings(&workbook, current.imported_output_profile());
        let csv =
            export_csv_for_profile(&workbook.sheets[0], current.imported_output_profile()).unwrap();
        assert!(String::from_utf8(csv).unwrap().starts_with(if capacity {
            "c0,c1,c2\r\nrow128-0"
        } else {
            "c0,c1\r\n25,23"
        }));
    }
    assert_ranked_tracker_export();
    assert_ranked_budget_export();
}

fn tracker_fixture(ranked: bool) -> (DesignerRuntime, super::NativeTrackerExportPresentation) {
    let mut current = DesignerRuntime::tracker(OCCURRENCE).unwrap();
    current
        .paste_cells(
            "resident/0",
            "tracker",
            None,
            "task",
            &[
                vec!["first".into(), "1".into(), "false".into()],
                vec!["second".into(), "2".into(), "true".into()],
                vec!["third".into(), "3".into(), "false".into()],
            ],
        )
        .unwrap();
    let table = current.query_table("tracker").unwrap();
    let presentation = super::NativeTrackerExportPresentation {
        version: 1,
        rows: table
            .rows
            .iter()
            .rev()
            .enumerate()
            .map(|(index, row)| super::NativeTrackerExportRow {
                entity_id: row.id.clone(),
                styles: vec![
                    CellStyle {
                        bold: index == 0,
                        ..CellStyle::default()
                    };
                    3
                ],
            })
            .collect(),
    };
    if ranked {
        let mut document = current.session.document().clone();
        // Rank the middle source row first, ahead of both presentation and IDs.
        document
            .entities
            .get_mut(table.rows[1].id.as_str())
            .unwrap()
            .rank = Some(RowRank::new("A").unwrap());
        current = open_fixture(document);
    }
    (current, presentation)
}

fn assert_ranked_tracker_export() {
    let (current, presentation) = tracker_fixture(true);
    let workbook = current
        .export_native_tracker_workbook(current.current_revision(), &presentation)
        .unwrap();
    let spec = &current.collection_specs["tracker"];
    for (row, id) in workbook.sheets[0].rows.iter().zip(&spec.entities) {
        let source = presentation
            .rows
            .iter()
            .find(|row| row.entity_id == id.as_str())
            .unwrap();
        assert_eq!(row[0].style, source.styles[0]);
        let Value::Text(task) =
            &current.session.document().entities[id].fields[&FieldId::from("task")]
        else {
            panic!("task")
        };
        assert_eq!(
            row[0].value,
            SourceValue::Text {
                value: task.clone()
            }
        );
    }
    assert_file_encodings(&workbook, OutputProfile::NativeTracker);
}

fn budget_presentation(current: &DesignerRuntime) -> super::NativeBudgetExportPresentation {
    let spec = &current.collection_specs["rows"];
    super::NativeBudgetExportPresentation {
        version: 1,
        active_view: "active".into(),
        views: vec![super::NativeBudgetExportView {
            id: "active".into(),
            name: "Budget".into(),
            collection_id: spec.summary.id.clone(),
        }],
        collections: vec![super::NativeBudgetExportCollection {
            collection_id: spec.summary.id.clone(),
            rows: spec
                .canonical_entities
                .iter()
                .rev()
                .map(|id| super::NativeBudgetExportRow {
                    entity_id: id.to_string(),
                    styles: vec![
                        CellStyle {
                            bold: id.as_str() == "e03",
                            ..CellStyle::default()
                        };
                        3
                    ],
                })
                .collect(),
        }],
    }
}

fn assert_ranked_budget_export() {
    let current = open_fixture(ranked_document());
    let presentation = budget_presentation(&current);
    let workbook = current
        .export_native_budget_workbook("resident/0", &presentation)
        .unwrap();
    assert_eq!(
        workbook.sheets[0].rows[0][0].value,
        SourceValue::Number { value: 1.0 }
    );
    assert!(workbook.sheets[0].rows[0][0].style.bold);
    assert_eq!(
        workbook.sheets[0].rows[0][1].formula.as_deref(),
        Some("'Budget'!A$3")
    );
    assert_file_encodings(&workbook, OutputProfile::Shared);
    let csv = current
        .export_native_budget_csv_workbook("resident/0", &presentation)
        .unwrap();
    assert_eq!(
        csv.sheets[0].rows[0][0].value,
        SourceValue::Number { value: 1.0 }
    );
    assert!(
        String::from_utf8(export_csv_for_profile(&csv.sheets[0], OutputProfile::Shared).unwrap())
            .unwrap()
            .starts_with("amount,calc,label\r\n1,-10000000000000000,same")
    );
}

#[test]
fn unranked_import_and_tracker_export_bytes_unchanged() {
    let (current, mut metadata) = unranked_import(false);
    metadata.sheets[0].rows.reverse();
    let workbook = current
        .export_workbook(current.current_revision(), &metadata)
        .unwrap();
    let mut expected = source_workbook(false);
    expected.sheets[0].rows.reverse();
    for row in &mut expected.sheets[0].rows {
        row[1].formula = Some("'Input'!$A$4".into());
    }
    assert_eq!(workbook.sheets[0].rows, expected.sheets[0].rows);
    assert_export_bytes(&workbook, &expected, OutputProfile::Shared);
    let (capacity, metadata) = unranked_import(true);
    let exported = capacity
        .export_workbook(capacity.current_revision(), &metadata)
        .unwrap();
    assert_export_bytes(
        &exported,
        &source_workbook(true),
        OutputProfile::TextCapacity,
    );
    let (current, presentation) = tracker_fixture(false);
    let actual = current
        .export_native_tracker_workbook(current.current_revision(), &presentation)
        .unwrap();
    let mut expected = actual.clone();
    expected.sheets[0].rows = presentation
        .rows
        .iter()
        .map(|row| {
            let entity = &current.session.document().entities[row.entity_id.as_str()];
            ["task", "estimate", "done"]
                .iter()
                .zip(&row.styles)
                .map(|(field, style)| SourceCell {
                    value: match &entity.fields[&FieldId::from(*field)] {
                        Value::Text(value) => SourceValue::Text {
                            value: value.clone(),
                        },
                        Value::Number(value) => SourceValue::Number { value: value.get() },
                        Value::Boolean(value) => SourceValue::Boolean { value: *value },
                        _ => panic!("scalar Tracker value"),
                    },
                    formula: None,
                    style: style.clone(),
                })
                .collect()
        })
        .collect();
    assert_export_bytes(&actual, &expected, OutputProfile::NativeTracker);
    let document = current.session.document();
    let saved = current
        .export_project_v3(current.current_revision())
        .unwrap();
    let tree = tachiko_storage::encode_roproj_v3(document).unwrap();
    assert_eq!(
        saved.bytes,
        encode_project_bundle(tree.files().iter().map(|file| (file.path(), file.bytes()))).unwrap()
    );
}

fn assert_export_bytes(actual: &SourceWorkbook, expected: &SourceWorkbook, profile: OutputProfile) {
    assert_eq!(
        export_xlsx_for_profile(actual, profile).unwrap(),
        export_xlsx_for_profile(expected, profile).unwrap()
    );
    assert_eq!(
        export_csv_for_profile(&actual.sheets[0], profile).unwrap(),
        export_csv_for_profile(&expected.sheets[0], profile).unwrap()
    );
}

#[test]
fn rank_saved_size_bound_covers_actual_v3_bytes() {
    let mut document = ranked_document();
    for entity in document.entities.values_mut() {
        entity.rank = None;
    }
    let unranked_bound = save_size::saved_bytes_upper_bound(&document);
    let plain = SemanticCommand::AppendEntity {
        entity: document.entities["e01"].clone(),
    };
    for entity in document.entities.values_mut() {
        entity.rank = Some(RowRank::new("Z".repeat(64)).unwrap());
    }
    assert_eq!(
        save_size::saved_bytes_upper_bound(&document) - unranked_bound,
        5 * 74
    );
    let ranked = SemanticCommand::AppendEntity {
        entity: document.entities["e01"].clone(),
    };
    assert_eq!(ranked.approx_bytes() - plain.approx_bytes(), 64);
    assert_saved_bound(&document);
    let (mut near, _, _) = super::working_size_tests::crossing_documents();
    for entity in near.entities.values_mut() {
        entity.rank = Some(RowRank::new("Z".repeat(64)).unwrap());
    }
    let bound = save_size::saved_bytes_upper_bound(&near);
    let maximum = u64::try_from(super::capacity::MAX_WORKING_SAVED_BYTES).unwrap();
    assert!(bound <= maximum && bound > maximum - 512 * 1024);
    assert_saved_bound(&near);
}

fn assert_saved_bound(document: &Document) {
    let current =
        DesignerRuntime::from_document_working_size(document.clone(), OCCURRENCE).unwrap();
    let bound = save_size::saved_bytes_upper_bound(document);
    let project = current
        .export_project_v3(current.current_revision())
        .unwrap();
    assert!(u64::try_from(project.bytes.len()).unwrap() <= bound);
    let portable = current
        .export_portable_ro(current.current_revision())
        .unwrap();
    assert!(u64::try_from(portable.bytes.len()).unwrap() <= bound);
}
