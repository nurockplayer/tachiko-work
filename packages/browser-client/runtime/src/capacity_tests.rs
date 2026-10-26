use super::capacity::{CapacityLimit, CapacityLimits, DEFAULT_LIMITS, check_document};
use super::capacity_fixtures::{FixtureSpec, capacity_document};
use super::{
    CapacityProjection, DesignerError, DesignerRequest, DesignerResponse, DesignerRuntime,
    DesignerWireReply, Expression, FieldId, ImportFieldType, ImportSelection, NewTableColumnInput,
    PasteCellInput, RowInitializer, ScalarEditInput, Value, encode_roproj_v3, import_workbook,
    open_project,
};

fn assert_capacity_boundary(
    document: &super::Document,
    limit: CapacityLimit,
    maximum: usize,
    set_maximum: fn(&mut CapacityLimits, usize),
    collection: Option<&str>,
) {
    let mut limits = DEFAULT_LIMITS;
    set_maximum(&mut limits, maximum);
    assert!(check_document(document, limits).is_ok());

    let mut limits = DEFAULT_LIMITS;
    set_maximum(&mut limits, maximum - 1);
    let error = check_document(document, limits).unwrap_err();
    match error {
        DesignerError::CapacityExceeded(error) => {
            assert_eq!(error.limit, limit);
            assert_eq!(error.maximum, maximum - 1);
            assert_eq!(error.attempted, maximum);
            assert_eq!(error.collection.as_deref(), collection);
        }
        other => panic!("expected capacity refusal, got {other:?}"),
    }
}

#[test]
fn every_accounting_budget_admits_at_its_limit_and_refuses_one_under() {
    let plain = capacity_document(FixtureSpec {
        rows: 2,
        columns: 2,
        formula_cells: 0,
        seed: 11,
    });
    let plain_usage = check_document(&plain, DEFAULT_LIMITS).unwrap();
    assert_capacity_boundary(
        &plain,
        CapacityLimit::Collections,
        plain_usage.collections,
        |limits, maximum| limits.collections = maximum,
        None,
    );
    assert_capacity_boundary(
        &plain,
        CapacityLimit::TableFields,
        plain_usage.tables["grid"].fields,
        |limits, maximum| limits.table_fields = maximum,
        Some("grid"),
    );
    assert_capacity_boundary(
        &plain,
        CapacityLimit::TableRows,
        plain_usage.tables["grid"].rows,
        |limits, maximum| limits.table_rows = maximum,
        Some("grid"),
    );
    assert_capacity_boundary(
        &plain,
        CapacityLimit::TotalEntities,
        plain_usage.total_entities,
        |limits, maximum| limits.total_entities = maximum,
        None,
    );
    assert_capacity_boundary(
        &plain,
        CapacityLimit::PopulatedCells,
        plain_usage.populated_cells,
        |limits, maximum| limits.populated_cells = maximum,
        None,
    );

    let formulas = capacity_document(FixtureSpec {
        rows: 2,
        columns: 1,
        formula_cells: 1,
        seed: 12,
    });
    let formula_usage = check_document(&formulas, DEFAULT_LIMITS).unwrap();
    assert_capacity_boundary(
        &formulas,
        CapacityLimit::Formulas,
        formula_usage.formulas,
        |limits, maximum| limits.formulas = maximum,
        None,
    );
    assert_capacity_boundary(
        &formulas,
        CapacityLimit::FormulaAstNodes,
        formula_usage.formula_ast_nodes,
        |limits, maximum| limits.formula_ast_nodes = maximum,
        None,
    );
    assert_capacity_boundary(
        &formulas,
        CapacityLimit::DependencyEdges,
        formula_usage.dependency_edges,
        |limits, maximum| limits.dependency_edges = maximum,
        None,
    );
}

#[test]
fn empty_entities_count_but_missing_cells_do_not() {
    let mut document = capacity_document(FixtureSpec {
        rows: 3,
        columns: 1,
        formula_cells: 0,
        seed: 13,
    });
    for entity in document.entities.values_mut() {
        entity.fields.clear();
    }

    let usage = check_document(&document, DEFAULT_LIMITS).unwrap();
    assert_eq!(usage.total_entities, 3);
    assert_eq!(usage.populated_cells, 0);
    assert_eq!(usage.tables["grid"].rows, 3);
}

#[test]
fn formula_ast_counts_nodes_and_dependency_edges_unique_targets_per_formula() {
    let mut document = capacity_document(FixtureSpec {
        rows: 2,
        columns: 1,
        formula_cells: 1,
        seed: 14,
    });
    let owner = super::capacity_fixtures::entity_id(0);
    let repeated_target = super::FieldRef::new(
        super::capacity_fixtures::entity_id(0),
        super::capacity_fixtures::RATE_FIELD,
    );
    document.entities.get_mut(&owner).unwrap().fields.insert(
        FieldId::from(super::capacity_fixtures::CALC_FIELD),
        Value::Formula(Expression::Add {
            left: Box::new(Expression::Reference(repeated_target.clone())),
            right: Box::new(Expression::Reference(repeated_target)),
        }),
    );

    let usage = check_document(&document, DEFAULT_LIMITS).unwrap();
    assert_eq!(usage.formulas, 2);
    assert_eq!(usage.formula_ast_nodes, 4);
    // The two repeated leaves in the first formula bind to one target; the
    // column-total formula contributes one target of its own.
    assert_eq!(usage.dependency_edges, 2);
}

fn boolean_table() -> (DesignerRuntime, String, String) {
    let runtime = DesignerRuntime::new_table(
        "00000000-0000-4000-8000-000000000529",
        "Capacity",
        &[NewTableColumnInput {
            required: true,
            name: "active".into(),
            field_type: "boolean".into(),
        }],
    )
    .unwrap();
    let collection = runtime.collection_specs.keys().next().unwrap().clone();
    let field = runtime.query_table(&collection).unwrap().columns[0]
        .id
        .clone();
    (runtime, collection, field)
}

fn assert_capacity_refusal_is_atomic(
    runtime: &mut DesignerRuntime,
    request: DesignerRequest,
    limit: CapacityLimit,
    maximum: usize,
    attempted: usize,
) {
    let revision = runtime.current_revision().to_owned();
    let document = runtime.session.export_snapshot().document().clone();
    let undo = format!("{:?}", runtime.undo);
    let redo = format!("{:?}", runtime.redo);
    let error = runtime.handle(request).unwrap_err();
    match error {
        DesignerError::CapacityExceeded(error) => {
            assert_eq!(error.limit, limit);
            assert_eq!(error.maximum, maximum);
            assert_eq!(error.attempted, attempted);
        }
        other => panic!("expected capacity refusal, got {other:?}"),
    }
    assert_eq!(runtime.current_revision(), revision);
    assert_eq!(runtime.session.export_snapshot().document(), &document);
    assert_eq!(format!("{:?}", runtime.undo), undo);
    assert_eq!(format!("{:?}", runtime.redo), redo);
}

#[test]
fn insert_and_paste_row_refusals_leave_revision_and_history_unchanged() {
    let (mut runtime, collection, field) = boolean_table();
    for _ in 0..DEFAULT_LIMITS.table_rows - 2 {
        let expected_revision = runtime.current_revision().to_owned();
        runtime
            .handle(DesignerRequest::InsertRow {
                expected_revision,
                collection: collection.clone(),
                initializers: vec![RowInitializer {
                    field: field.clone(),
                    input: ScalarEditInput::Boolean { value: true },
                }],
            })
            .unwrap();
    }
    let expected_revision = runtime.current_revision().to_owned();
    runtime
        .handle(DesignerRequest::InsertRow {
            expected_revision,
            collection: collection.clone(),
            initializers: vec![RowInitializer {
                field: field.clone(),
                input: ScalarEditInput::Boolean { value: true },
            }],
        })
        .unwrap();
    assert_eq!(
        runtime.query_table(&collection).unwrap().rows.len(),
        DEFAULT_LIMITS.table_rows - 1
    );
    let expected_revision = runtime.current_revision().to_owned();
    runtime
        .handle(DesignerRequest::PasteCells {
            expected_revision,
            collection: collection.clone(),
            start_entity: None,
            start_field: field.clone(),
            rows: vec![vec![PasteCellInput::Typed(ScalarEditInput::Boolean {
                value: true,
            })]],
        })
        .unwrap();
    assert_eq!(
        runtime.query_table(&collection).unwrap().rows.len(),
        DEFAULT_LIMITS.table_rows
    );
    let revision = runtime.current_revision().to_owned();
    assert_capacity_refusal_is_atomic(
        &mut runtime,
        DesignerRequest::InsertRow {
            expected_revision: revision.clone(),
            collection: collection.clone(),
            initializers: vec![RowInitializer {
                field: field.clone(),
                input: ScalarEditInput::Boolean { value: true },
            }],
        },
        CapacityLimit::TableRows,
        DEFAULT_LIMITS.table_rows,
        DEFAULT_LIMITS.table_rows + 1,
    );

    assert_capacity_refusal_is_atomic(
        &mut runtime,
        DesignerRequest::PasteCells {
            expected_revision: revision,
            collection,
            start_entity: None,
            start_field: field,
            rows: vec![vec![PasteCellInput::Typed(ScalarEditInput::Boolean {
                value: true,
            })]],
        },
        CapacityLimit::TableRows,
        DEFAULT_LIMITS.table_rows,
        DEFAULT_LIMITS.table_rows + 1,
    );
}

#[test]
fn add_column_refusal_is_typed_and_atomic() {
    let columns = (0..DEFAULT_LIMITS.table_fields - 1)
        .map(|index| NewTableColumnInput {
            required: true,
            name: format!("column_{index}"),
            field_type: "boolean".into(),
        })
        .collect::<Vec<_>>();
    let mut runtime =
        DesignerRuntime::new_table("00000000-0000-4000-8000-000000000529", "Capacity", &columns)
            .unwrap();
    let collection = runtime.collection_specs.keys().next().unwrap().clone();
    let revision = runtime.current_revision().to_owned();
    runtime
        .handle(DesignerRequest::AddColumn {
            expected_revision: revision,
            collection: collection.clone(),
            name: "last_allowed".into(),
            field_type: "boolean".into(),
            required: true,
            initializers: Vec::new(),
        })
        .unwrap();
    assert_eq!(
        runtime.query_table(&collection).unwrap().columns.len(),
        DEFAULT_LIMITS.table_fields
    );
    let revision = runtime.current_revision().to_owned();
    assert_capacity_refusal_is_atomic(
        &mut runtime,
        DesignerRequest::AddColumn {
            expected_revision: revision,
            collection,
            name: "overflow".into(),
            field_type: "boolean".into(),
            required: true,
            initializers: Vec::new(),
        },
        CapacityLimit::TableFields,
        DEFAULT_LIMITS.table_fields,
        DEFAULT_LIMITS.table_fields + 1,
    );
}

#[test]
fn csv_import_candidate_is_accounted_before_publication() {
    let mut csv = String::from("active\n");
    for _ in 0..super::interop_adapter::MAX_DATA_ROWS {
        csv.push_str("true\n");
    }
    let source = super::interop_adapter::import_csv(
        csv.as_bytes(),
        &super::interop_adapter::ImportOptions::default(),
    )
    .unwrap();
    let selection = ImportSelection {
        column_types: vec![vec![ImportFieldType::Boolean]],
        extra_columns: vec![Vec::new()],
    };
    let (runtime, _) =
        import_workbook(&source, &selection, "00000000-0000-4000-8000-000000000530").unwrap();
    assert_eq!(
        runtime.capacity_usage.total_entities,
        super::interop_adapter::MAX_DATA_ROWS
    );
    assert_eq!(
        runtime.capacity_usage.populated_cells,
        super::interop_adapter::MAX_DATA_ROWS
    );
}

fn host_reply(
    host: &mut super::host_bridge::DesignerHost,
    request: &DesignerRequest,
) -> DesignerWireReply {
    let request = serde_json::to_vec(request).unwrap();
    serde_json::from_slice(&host.request(&request)).unwrap()
}

fn host_capacity(host: &mut super::host_bridge::DesignerHost) -> CapacityProjection {
    match host_reply(
        host,
        &DesignerRequest::QueryCapacity {
            expected_revision: None,
        },
    ) {
        DesignerWireReply::Ok {
            response: DesignerResponse::Capacity(projection),
        } => projection,
        other => panic!("unexpected capacity reply {other:?}"),
    }
}

fn csv_with_data_rows(rows: usize) -> String {
    let mut csv = String::from("value\n");
    for _ in 0..rows {
        csv.push_str("x\n");
    }
    csv
}

#[test]
fn csv_import_admits_8406_rows_and_refuses_8407_atomically() {
    let maximum = super::interop_adapter::MAX_TEXT_CAPACITY_ROWS;
    let source = super::interop_adapter::import_csv(
        csv_with_data_rows(maximum).as_bytes(),
        &super::interop_adapter::ImportOptions::default(),
    )
    .unwrap();
    let selection = ImportSelection {
        column_types: vec![vec![ImportFieldType::Text]],
        extra_columns: vec![Vec::new()],
    };
    let (imported, _) =
        import_workbook(&source, &selection, "00000000-0000-4000-8000-000000000535").unwrap();
    assert!(imported.text_capacity);
    assert_eq!(imported.capacity_usage.total_entities, maximum);
    assert_eq!(imported.capacity_usage.populated_cells, maximum);

    let mut host = super::host_bridge::DesignerHost::new();
    let opened = match host_reply(
        &mut host,
        &DesignerRequest::NewTable {
            occurrence_id: "00000000-0000-4000-8000-000000000536".into(),
            name: "Resident".into(),
            columns: vec![NewTableColumnInput {
                required: true,
                name: "value".into(),
                field_type: "text".into(),
            }],
        },
    ) {
        DesignerWireReply::Ok {
            response: DesignerResponse::Opened(opened),
        } => opened,
        other => panic!("unexpected open reply {other:?}"),
    };
    let revision = opened.bootstrap.revision;
    assert!(matches!(
        host_reply(
            &mut host,
            &DesignerRequest::InsertRow {
                expected_revision: revision,
                collection: opened.table.collection.key,
                initializers: vec![RowInitializer {
                    field: opened.table.columns[0].id.clone(),
                    input: ScalarEditInput::Text {
                        value: "resident".into(),
                    },
                }],
            },
        ),
        DesignerWireReply::Ok {
            response: DesignerResponse::Published(_),
        }
    ));
    let before = host_capacity(&mut host);
    assert_eq!(before.usage.total_entities, 1);
    let operation = br#"{"type":"import","format":"csv","csv_options":{"delimiter":",","header":true},"selection":{"column_types":[["text"]],"extra_columns":[[]]},"occurrence_id":"00000000-0000-4000-8000-000000000537","install":true}"#;
    let reply = host.spreadsheet(operation, csv_with_data_rows(maximum + 1).as_bytes());
    assert!(reply.bytes.is_none());
    match serde_json::from_slice::<DesignerWireReply>(&reply.reply).unwrap() {
        DesignerWireReply::Error { error } => {
            assert_eq!(error.code, "capacity_exceeded");
            let capacity = error.capacity.unwrap();
            assert_eq!(capacity.limit, CapacityLimit::TableRows);
            assert_eq!(capacity.maximum, maximum);
            assert_eq!(capacity.attempted, maximum + 1);
            assert_eq!(capacity.collection.as_deref(), Some("Imported table"));
        }
        other @ DesignerWireReply::Ok { .. } => {
            panic!("expected capacity error reply, got {other:?}")
        }
    }
    assert_eq!(host_capacity(&mut host), before);

    assert!(matches!(
        host_reply(
            &mut host,
            &DesignerRequest::Undo {
                expected_revision: before.revision.clone(),
            },
        ),
        DesignerWireReply::Ok {
            response: DesignerResponse::Published(_),
        }
    ));
    assert_eq!(host_capacity(&mut host).usage.total_entities, 0);
    let revision = host_capacity(&mut host).revision;
    assert!(matches!(
        host_reply(
            &mut host,
            &DesignerRequest::Redo {
                expected_revision: revision,
            },
        ),
        DesignerWireReply::Ok {
            response: DesignerResponse::Published(_),
        }
    ));
    assert_eq!(host_capacity(&mut host).usage.total_entities, 1);
}

fn v3_bundle(document: &super::Document) -> Vec<u8> {
    let tree = encode_roproj_v3(document).unwrap();
    super::encode_project_bundle(tree.files().iter().map(|file| (file.path(), file.bytes())))
        .unwrap()
}

#[test]
fn v3_reopen_refusal_preserves_the_resident_occurrence_and_history() {
    let (resident, _, _) = boolean_table();
    let mut runtime = Some(resident);
    let before_revision = runtime.as_ref().unwrap().current_revision().to_owned();
    let before_document = runtime
        .as_ref()
        .unwrap()
        .session
        .export_snapshot()
        .document()
        .clone();
    let before_undo = format!("{:?}", runtime.as_ref().unwrap().undo);
    let before_redo = format!("{:?}", runtime.as_ref().unwrap().redo);

    let (template_runtime, _, field) = boolean_table();
    let mut document = template_runtime.session.export_snapshot().into_document();
    let schema = document.schemas.keys().next().unwrap().clone();
    for row in 0..=DEFAULT_LIMITS.table_rows {
        let id = super::EntityId::from(format!("capacity_row_{row}"));
        let entity = super::Entity {
            id: id.clone(),
            key: super::EntityKey::from(format!("capacity_row_{row}")),
            schema: schema.clone(),
            fields: std::collections::BTreeMap::from([(
                FieldId::from(field.as_str()),
                Value::Boolean(true),
            )]),
        };
        document.entities.insert(id, entity);
    }
    let bytes = v3_bundle(&document);
    let error =
        open_project(&mut runtime, &bytes, "00000000-0000-4000-8000-000000000531").unwrap_err();
    match error {
        DesignerError::CapacityExceeded(error) => {
            assert_eq!(error.limit, CapacityLimit::TableRows);
            assert_eq!(error.maximum, DEFAULT_LIMITS.table_rows);
            assert_eq!(error.attempted, DEFAULT_LIMITS.table_rows + 1);
        }
        other => panic!("expected capacity refusal, got {other:?}"),
    }

    let (template_runtime, _, accepted_field) = boolean_table();
    let mut accepted_document = template_runtime.session.export_snapshot().into_document();
    let schema = accepted_document.schemas.keys().next().unwrap().clone();
    for row in 0..DEFAULT_LIMITS.table_rows {
        let id = super::EntityId::from(format!("capacity_row_{row}"));
        accepted_document.entities.insert(
            id.clone(),
            super::Entity {
                id: id.clone(),
                key: super::EntityKey::from(format!("capacity_row_{row}")),
                schema: schema.clone(),
                fields: std::collections::BTreeMap::from([(
                    FieldId::from(accepted_field.as_str()),
                    Value::Boolean(true),
                )]),
            },
        );
    }
    let mut accepted_resident = Some(boolean_table().0);
    open_project(
        &mut accepted_resident,
        &v3_bundle(&accepted_document),
        "00000000-0000-4000-8000-000000000532",
    )
    .unwrap();
    let accepted_resident = accepted_resident.unwrap();
    let accepted_collection = accepted_resident.collection_specs.keys().next().unwrap();
    assert_eq!(
        accepted_resident
            .query_table(accepted_collection)
            .unwrap()
            .rows
            .len(),
        DEFAULT_LIMITS.table_rows
    );

    let resident = runtime.unwrap();
    assert_eq!(resident.current_revision(), before_revision);
    assert_eq!(
        resident.session.export_snapshot().document(),
        &before_document
    );
    assert_eq!(format!("{:?}", resident.undo), before_undo);
    assert_eq!(format!("{:?}", resident.redo), before_redo);
}

#[test]
fn capacity_projection_tracks_edit_undo_redo_and_stale_refusal() {
    let (mut runtime, collection, field) = boolean_table();
    let query = |runtime: &mut DesignerRuntime| {
        let expected_revision = runtime.current_revision().to_owned();
        match runtime
            .handle(DesignerRequest::QueryCapacity {
                expected_revision: Some(expected_revision),
            })
            .unwrap()
        {
            DesignerResponse::Capacity(projection) => projection,
            other => panic!("unexpected capacity response {other:?}"),
        }
    };
    let initial = query(&mut runtime);
    assert_eq!(initial.usage.total_entities, 0);
    assert_eq!(initial.limits.populated_cells, 100_000);
    assert_eq!(initial.limits.formula_ast_nodes, 100_000);
    assert_eq!(initial.limits.dependency_edges, 200_000);
    let revision = runtime.current_revision().to_owned();
    runtime
        .handle(DesignerRequest::InsertRow {
            expected_revision: revision,
            collection,
            initializers: vec![RowInitializer {
                field,
                input: ScalarEditInput::Boolean { value: true },
            }],
        })
        .unwrap();
    assert_eq!(query(&mut runtime).usage.total_entities, 1);
    let revision = runtime.current_revision().to_owned();
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: revision,
        })
        .unwrap();
    assert_eq!(query(&mut runtime).usage.total_entities, 0);
    let revision = runtime.current_revision().to_owned();
    runtime
        .handle(DesignerRequest::Redo {
            expected_revision: revision,
        })
        .unwrap();
    assert_eq!(query(&mut runtime).usage.total_entities, 1);
    assert!(matches!(
        runtime.handle(DesignerRequest::QueryCapacity {
            expected_revision: Some("stale/revision".into()),
        }),
        Err(DesignerError::StaleQuery { .. })
    ));
    assert_eq!(query(&mut runtime).usage.total_entities, 1);
}

#[test]
fn text_capacity_usage_refreshes_after_clear_undo_and_redo() {
    let columns = ["first", "second", "third"].map(|name| NewTableColumnInput {
        required: false,
        name: name.into(),
        field_type: "text".into(),
    });
    let mut runtime = DesignerRuntime::new_table(
        "00000000-0000-4000-8000-000000000533",
        "Text Capacity",
        &columns,
    )
    .unwrap();
    let collection = runtime.collection_specs.keys().next().unwrap().clone();
    let fields = runtime
        .query_table(&collection)
        .unwrap()
        .columns
        .into_iter()
        .map(|column| column.id)
        .collect::<Vec<_>>();
    for row in 0..65 {
        let expected_revision = runtime.current_revision().to_owned();
        runtime
            .handle(DesignerRequest::InsertRow {
                expected_revision,
                collection: collection.clone(),
                initializers: fields
                    .iter()
                    .map(|field| RowInitializer {
                        field: field.clone(),
                        input: ScalarEditInput::Text {
                            value: format!("row-{row}"),
                        },
                    })
                    .collect(),
            })
            .unwrap();
    }
    let document = runtime.session.export_snapshot().into_document();
    let mut runtime = DesignerRuntime::from_document_profile(
        document,
        "00000000-0000-4000-8000-000000000534",
        true,
    )
    .unwrap();
    assert!(runtime.text_capacity);
    let row = runtime.query_table(&collection).unwrap().rows[0].id.clone();
    let target = super::FieldTarget {
        entity: row,
        field: fields[0].clone(),
    };
    let query_populated_cells = |runtime: &mut DesignerRuntime| {
        let expected_revision = runtime.current_revision().to_owned();
        match runtime
            .handle(DesignerRequest::QueryCapacity {
                expected_revision: Some(expected_revision),
            })
            .unwrap()
        {
            DesignerResponse::Capacity(projection) => projection.usage.populated_cells,
            other => panic!("unexpected capacity response {other:?}"),
        }
    };
    assert_eq!(query_populated_cells(&mut runtime), 195);

    let revision = runtime.current_revision().to_owned();
    runtime
        .handle(DesignerRequest::EditCells {
            expected_revision: revision,
            edits: vec![super::CellEdit {
                target,
                input: ScalarEditInput::Clear,
            }],
        })
        .unwrap();
    assert_eq!(query_populated_cells(&mut runtime), 194);
    let revision = runtime.current_revision().to_owned();
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: revision,
        })
        .unwrap();
    assert_eq!(query_populated_cells(&mut runtime), 195);
    let revision = runtime.current_revision().to_owned();
    runtime
        .handle(DesignerRequest::Redo {
            expected_revision: revision,
        })
        .unwrap();
    assert_eq!(query_populated_cells(&mut runtime), 194);
}

#[test]
fn xlsx_rows_past_the_text_capacity_profile_are_typed_capacity_refusals() {
    let maximum = super::interop_adapter::MAX_TEXT_CAPACITY_ROWS;
    // Row 1 is the header, so the last admitted data row is `maximum + 1`.
    assert_eq!(
        super::interop_adapter::coordinate(&format!("A{}", maximum + 1)).unwrap(),
        (maximum, 0)
    );
    match super::interop_adapter::coordinate(&format!("A{}", maximum + 2)) {
        Err(super::interop_adapter::InteropError::Capacity(error)) => {
            assert_eq!(error.limit, CapacityLimit::TableRows);
            assert_eq!(error.maximum, maximum);
            assert_eq!(error.attempted, maximum + 1);
            assert_eq!(error.collection.as_deref(), Some("Imported table"));
        }
        other => panic!("expected a capacity refusal, got {other:?}"),
    }
    assert!(matches!(
        super::interop_adapter::coordinate("A0"),
        Err(super::interop_adapter::InteropError::Invalid(_))
    ));
}

#[test]
fn text_capacity_insertion_honors_the_reported_row_limit() {
    let source = super::interop_adapter::import_csv(
        csv_with_data_rows(200).as_bytes(),
        &super::interop_adapter::ImportOptions::default(),
    )
    .unwrap();
    let selection = ImportSelection {
        column_types: vec![vec![ImportFieldType::Text]],
        extra_columns: vec![Vec::new()],
    };
    let (mut runtime, imported) =
        import_workbook(&source, &selection, "00000000-0000-4000-8000-000000000538").unwrap();
    assert!(runtime.text_capacity);
    let capacity = match runtime
        .handle(DesignerRequest::QueryCapacity {
            expected_revision: None,
        })
        .unwrap()
    {
        DesignerResponse::Capacity(projection) => projection,
        other => panic!("unexpected reply {other:?}"),
    };
    assert_eq!(
        capacity.limits.table_rows,
        super::interop_adapter::MAX_TEXT_CAPACITY_ROWS
    );
    // 200 rows is past the strict 128-row profile but inside the reported
    // limit, so one more row is admitted.
    let reply = runtime.handle(DesignerRequest::InsertRow {
        expected_revision: capacity.revision,
        collection: imported.opened.bootstrap.default_collection.clone(),
        initializers: Vec::new(),
    });
    assert!(
        matches!(reply, Ok(DesignerResponse::Published(_))),
        "insert at 201 of {} rows: {reply:?}",
        capacity.limits.table_rows
    );
}

#[test]
fn an_unnamed_oversized_table_is_refused_for_its_name_not_its_capacity() {
    let columns = (0..33)
        .map(|index| NewTableColumnInput {
            required: true,
            name: format!("c{index}"),
            field_type: "boolean".into(),
        })
        .collect::<Vec<_>>();
    // A capacity refusal names its collection, so the name is admitted first.
    let Err(error) =
        DesignerRuntime::new_table("00000000-0000-4000-8000-000000000539", "", &columns)
    else {
        panic!("an unnamed table is refused");
    };
    assert!(
        !matches!(error, DesignerError::CapacityExceeded(_)),
        "{error:?}"
    );
}

#[test]
fn a_column_total_counts_one_dependency_edge_per_row_of_its_table() {
    let mut document = capacity_document(FixtureSpec {
        rows: 3,
        columns: 2,
        formula_cells: 0,
        seed: 15,
    });
    let owner = super::capacity_fixtures::entity_id(0);
    let schema = document.entities[&owner].schema.clone();
    let field = super::capacity_fixtures::field_id(1);
    document.entities.get_mut(&owner).unwrap().fields.insert(
        super::capacity_fixtures::field_id(0),
        Value::Formula(Expression::ColumnTotal(super::ColumnRef::new(
            schema, field,
        ))),
    );
    let usage = check_document(&document, DEFAULT_LIMITS).unwrap();
    assert_eq!(usage.formulas, 1);
    assert_eq!(usage.formula_ast_nodes, 1);
    assert_eq!(usage.dependency_edges, 3);
}
