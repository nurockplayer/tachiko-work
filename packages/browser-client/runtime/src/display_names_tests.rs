use super::{
    BTreeMap, BTreeSet, DesignerError, DesignerRequest, DesignerRuntime, Entity, Expression,
    FieldId, FieldRef, GrantId, NewTableColumnInput, Number, Value, allocate_key, encode_roproj_v3,
    name_admission, open_portable_ro, open_project, process_wire_request,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000529";

fn columns(names: &[&str]) -> Vec<NewTableColumnInput> {
    names
        .iter()
        .map(|name| NewTableColumnInput {
            name: (*name).to_owned(),
            field_type: "text".to_owned(),
        })
        .collect()
}

#[test]
fn named_native_creation_allocates_keys_in_request_order() {
    let runtime = DesignerRuntime::new_table(
        OCCURRENCE,
        "My Expenses",
        &columns(&["項目", "Item Name", "column_1"]),
    )
    .unwrap();
    let snapshot = runtime.session.export_snapshot();
    let schema = snapshot.document().schemas.values().next().unwrap();
    assert_eq!(schema.key.as_str(), "table_1");
    let keys = schema
        .fields
        .values()
        .map(|field| field.key.as_str())
        .collect::<Vec<_>>();
    assert_eq!(keys, ["column_1", "column_2", "column_3"]);
}

#[test]
fn distinct_case_names_do_not_collide_when_their_derived_keys_do() {
    let runtime =
        DesignerRuntime::new_table(OCCURRENCE, "Orders", &columns(&["Item", "item"])).unwrap();
    let snapshot = runtime.session.export_snapshot();
    let schema = snapshot.document().schemas.values().next().unwrap();
    assert_eq!(
        schema
            .fields
            .values()
            .map(|field| field.key.as_str())
            .collect::<Vec<_>>(),
        ["item", "column_1"]
    );
}

#[test]
fn case_width_and_canonically_equivalent_spellings_stay_distinct() {
    let names = ["Item", "item", "Ｉtem", "é", "e\u{301}", "🧾"];
    let runtime = DesignerRuntime::new_table(OCCURRENCE, "Names", &columns(&names)).unwrap();
    let projection = runtime.query_table("names").unwrap();
    assert_eq!(
        projection
            .columns
            .iter()
            .map(|column| column.display_name.as_str())
            .collect::<Vec<_>>(),
        names
    );
    assert_eq!(
        projection
            .columns
            .iter()
            .map(|column| column.key.as_str())
            .collect::<Vec<_>>(),
        [
            "item", "column_1", "column_2", "column_3", "column_4", "column_5"
        ]
    );
}

#[test]
fn static_named_request_matches_independent_tree_and_package() {
    let runtime = DesignerRuntime::new_table(
        OCCURRENCE,
        "My Expenses",
        &columns(&["項目", "Item Name", "column_1"]),
    )
    .unwrap();
    let tree = encode_roproj_v3(runtime.session.export_snapshot().document()).unwrap();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../acceptance/native-display-names");
    for file in tree.files() {
        assert_eq!(
            file.bytes(),
            std::fs::read(fixture.join("tree").join(file.path())).unwrap()
        );
    }
    let expected = std::fs::read(fixture.join("named.ro")).unwrap();
    assert_eq!(
        runtime.export_portable_ro("resident/0").unwrap().bytes,
        expected
    );
    let carriers = [
        runtime.export_canonical_tree("resident/0").unwrap().bytes,
        runtime.export_project_v3("resident/0").unwrap().bytes,
    ];
    assert_eq!(carriers[0], carriers[1]);
    for carrier in carriers.iter().chain(std::iter::once(&expected)) {
        let mut reopened = None;
        if carrier.starts_with(b"PK") {
            open_portable_ro(
                &mut reopened,
                carrier,
                "00000000-0000-4000-8000-000000000530",
            )
            .unwrap();
        } else {
            open_project(
                &mut reopened,
                carrier,
                "00000000-0000-4000-8000-000000000530",
            )
            .unwrap();
        }
        let reopened = reopened.unwrap();
        assert_eq!(
            reopened.session.export_snapshot().document(),
            runtime.session.export_snapshot().document()
        );
        assert_eq!(
            reopened.export_portable_ro("resident/0").unwrap().bytes,
            expected
        );
        assert_eq!(
            reopened
                .query_table("table_1")
                .unwrap()
                .collection
                .display_name,
            "My Expenses"
        );
        assert!(reopened.export_project("resident/0").is_err());
    }
}

#[test]
fn authoring_trims_frozen_whitespace_keeps_feff_and_checks_utf8_bounds() {
    for name in ["\u{85}項目\u{2029}", "\u{3000}項目\u{a0}"] {
        let runtime = DesignerRuntime::new_table(OCCURRENCE, name, &columns(&[name])).unwrap();
        let projection = runtime.query_table("table_1").unwrap();
        assert_eq!(projection.collection.display_name, "項目");
        assert_eq!(projection.columns[0].display_name, "項目");
    }
    for name in [
        "🧾".repeat(1024),
        "\u{feff}Item\u{feff}".into(),
        "MixedCase".into(),
    ] {
        let authored = format!(" {name} ");
        let runtime =
            DesignerRuntime::new_table(OCCURRENCE, "Names", &columns(&[&authored])).unwrap();
        assert_eq!(
            runtime.query_table("names").unwrap().columns[0].display_name,
            name
        );
    }
    let error =
        DesignerRuntime::new_table(OCCURRENCE, "Names", &columns(&[&("🧾".repeat(1024) + "x")]))
            .err()
            .unwrap();
    assert_eq!(
        error.failure_projection("").name_failures[0].reason,
        "display_name_too_long"
    );
}

#[test]
fn pre_candidate_failures_accumulate_spelling_and_all_duplicate_input_indices() {
    let error = DesignerRuntime::new_table(
        OCCURRENCE,
        " ",
        &columns(&["a\0b", "項目", " 項目 ", "Item", "Item", &"\0".repeat(4097)]),
    )
    .err()
    .unwrap();
    let failure = error.failure_projection("");
    assert_eq!(failure.code, "invalid_table_operation");
    assert_eq!(failure.diagnostics, []);
    assert_eq!(failure.name_failures.len(), 6);
    let duplicates = failure
        .name_failures
        .iter()
        .filter(|finding| finding.reason == "duplicate_display_name")
        .collect::<Vec<_>>();
    assert_eq!(duplicates.len(), 2);
    assert_eq!(
        duplicates[0].inputs,
        [
            name_admission::NameInput::Column {
                index: 3,
                schema: None
            },
            name_admission::NameInput::Column {
                index: 4,
                schema: None
            }
        ]
    );
    assert_eq!(
        duplicates[1].inputs,
        [
            name_admission::NameInput::Column {
                index: 1,
                schema: None
            },
            name_admission::NameInput::Column {
                index: 2,
                schema: None
            }
        ]
    );
    assert!(
        failure
            .name_failures
            .iter()
            .all(|finding| finding.subjects.is_empty())
    );
}

#[test]
fn create_add_rename_undo_redo_keeps_keys_ids_values_and_constraints() {
    let mut runtime =
        DesignerRuntime::new_table(OCCURRENCE, "My Expenses", &columns(&["Item Name", "項目"]))
            .unwrap();
    let initial = runtime.session.export_snapshot().into_document();
    runtime
        .add_column("resident/0", "table_1", "Emoji 🧾", "text", &[])
        .unwrap();
    let added = runtime.session.export_snapshot().into_document();
    let target = added
        .schemas
        .values()
        .next()
        .unwrap()
        .fields
        .values()
        .find(|field| field.display_name.as_deref() == Some("Emoji 🧾"))
        .unwrap()
        .id
        .to_string();
    runtime
        .rename_column("resident/1", "table_1", &target, "  Receipt  ")
        .unwrap();
    let renamed = runtime.session.export_snapshot().into_document();
    let mut expected = added.clone();
    expected
        .schemas
        .values_mut()
        .next()
        .unwrap()
        .fields
        .get_mut(&FieldId::from(target.clone()))
        .unwrap()
        .display_name = Some("Receipt".into());
    assert_eq!(renamed, expected);
    assert_eq!(
        runtime
            .query_table("table_1")
            .unwrap()
            .columns
            .iter()
            .find(|column| column.id == target)
            .unwrap()
            .key,
        "column_3"
    );
    for (kind, expected) in [
        ("undo", &added),
        ("undo", &initial),
        ("redo", &added),
        ("redo", &renamed),
    ] {
        let revision = runtime.current_revision().to_owned();
        runtime
            .handle(if kind == "undo" {
                DesignerRequest::Undo {
                    expected_revision: revision,
                }
            } else {
                DesignerRequest::Redo {
                    expected_revision: revision,
                }
            })
            .unwrap();
        assert_eq!(runtime.session.export_snapshot().document(), expected);
        assert_eq!(
            runtime
                .query_table("table_1")
                .unwrap()
                .collection
                .display_name,
            "My Expenses"
        );
    }
}

#[test]
fn nameless_rename_inverse_restores_absence_and_preserves_formula_bindings() {
    let mut document = DesignerRuntime::new_table(
        OCCURRENCE,
        "Orders",
        &[
            NewTableColumnInput {
                name: "Amount".into(),
                field_type: "number".into(),
            },
            NewTableColumnInput {
                name: "Total".into(),
                field_type: "number".into(),
            },
        ],
    )
    .unwrap()
    .session
    .export_snapshot()
    .into_document();
    let schema = document.schemas.values_mut().next().unwrap();
    schema.display_name = None;
    for field in schema.fields.values_mut() {
        field.display_name = None;
    }
    let ids = schema.fields.keys().cloned().collect::<Vec<_>>();
    let entity = Entity {
        id: "row-id".into(),
        key: "row".into(),
        schema: schema.id.clone(),
        fields: BTreeMap::from([
            (ids[0].clone(), Value::Number(Number::new(3.0).unwrap())),
            (
                ids[1].clone(),
                Value::Formula(Expression::Reference(FieldRef::new(
                    "row-id",
                    ids[0].clone(),
                ))),
            ),
        ]),
    };
    document.entities.insert(entity.id.clone(), entity);
    let before = document.clone();
    let mut runtime = DesignerRuntime::from_document(document, OCCURRENCE).unwrap();
    runtime
        .rename_column("resident/0", "orders", ids[0].as_str(), "Amount USD")
        .unwrap();
    let after = runtime.session.export_snapshot().into_document();
    let mut expected = before.clone();
    expected
        .schemas
        .values_mut()
        .next()
        .unwrap()
        .fields
        .get_mut(&ids[0])
        .unwrap()
        .display_name = Some("Amount USD".into());
    assert_eq!(after, expected);
    assert_eq!(
        tachiko_workspace_engine::calculate_fields(&before).unwrap(),
        tachiko_workspace_engine::calculate_fields(&after).unwrap()
    );
    assert_eq!(
        runtime.query_table("orders").unwrap().rows[0].fields[0].address,
        "row.amount"
    );
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/1".into(),
        })
        .unwrap();
    assert_eq!(runtime.session.export_snapshot().document(), &before);
    runtime
        .handle(DesignerRequest::Redo {
            expected_revision: "resident/2".into(),
        })
        .unwrap();
    assert_eq!(runtime.session.export_snapshot().document(), &after);
}

#[test]
fn stale_invalid_duplicate_and_oversized_projection_edits_preserve_state_and_history() {
    let mut runtime =
        DesignerRuntime::new_table(OCCURRENCE, "Orders", &columns(&["Item", "Other"])).unwrap();
    let field = runtime.collection_specs["orders"].columns[1].id.to_string();
    runtime
        .rename_column("resident/0", "orders", &field, "First Name")
        .unwrap();
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/1".into(),
        })
        .unwrap();
    let before = runtime.session.export_snapshot();
    let history = (runtime.undo.len(), runtime.redo.len());
    for (revision, name, code) in [
        ("resident/0", "Valid", "stale_revision"),
        ("resident/2", " ", "invalid_table_operation"),
        ("resident/2", "a\u{0085}b", "invalid_table_operation"),
        ("resident/2", "Item", "validation_failed"),
    ] {
        let error = runtime
            .rename_column(revision, "orders", &field, name)
            .unwrap_err();
        let failure = error.failure_projection(runtime.current_revision());
        assert_eq!(failure.code, code);
        if code == "validation_failed" {
            assert_eq!(failure.diagnostics[0].subjects.len(), 2);
            assert_eq!(failure.diagnostics[0].related_subjects.len(), 2);
        }
        assert_eq!(runtime.session.export_snapshot(), before);
        assert_eq!((runtime.undo.len(), runtime.redo.len()), history);
    }
    let schema = runtime
        .session
        .export_snapshot()
        .document()
        .schemas
        .values()
        .next()
        .unwrap()
        .id
        .to_string();
    let failure = runtime
        .add_column("resident/2", "orders", " Item ", "text", &[])
        .unwrap_err()
        .failure_projection("resident/2");
    assert_eq!(
        failure.name_failures[0].inputs,
        [name_admission::NameInput::Column {
            index: 0,
            schema: Some(schema)
        }]
    );
    assert_eq!(failure.name_failures[0].subjects.len(), 1);
    assert_eq!(runtime.session.export_snapshot(), before);
    assert_eq!((runtime.undo.len(), runtime.redo.len()), history);
}

#[test]
fn allocation_skips_occupied_suffixes_and_does_not_rederive_after_rename() {
    let occupied = BTreeSet::from(["table_1".into(), "table_3".into()]);
    assert_eq!(
        allocate_key("My Expenses", "table", &occupied).unwrap(),
        "table_2"
    );
    assert_eq!(
        allocate_key("table_1", "table", &occupied).unwrap(),
        "table_2"
    );
    let runtime = DesignerRuntime::new_table(
        OCCURRENCE,
        "T",
        &columns(&["column_1", "Item", "item", "項目", "🧾"]),
    )
    .unwrap();
    assert_eq!(
        runtime
            .query_table("t")
            .unwrap()
            .columns
            .iter()
            .map(|column| column.key.as_str())
            .collect::<Vec<_>>(),
        ["column_1", "item", "column_2", "column_3", "column_4"]
    );
}

#[test]
fn name_edits_retain_revoked_authorization_and_fail_without_publication() {
    let mut runtime =
        DesignerRuntime::new_table(OCCURRENCE, "Orders", &columns(&["Item"])).unwrap();
    let field = runtime.collection_specs["orders"].columns[0].id.to_string();
    runtime
        .lifecycle
        .revoke_grant(&GrantId::from("designer-number-edit"))
        .unwrap();
    let before = runtime.session.export_snapshot();
    for add in [false, true] {
        let error = if add {
            runtime.add_column("resident/0", "orders", "項目", "text", &[])
        } else {
            runtime.rename_column("resident/0", "orders", &field, "Item Name")
        }
        .unwrap_err();
        assert!(matches!(error, DesignerError::Lifecycle(_)));
        assert_eq!(runtime.session.export_snapshot(), before);
        assert!(runtime.undo.is_empty());
        assert!(runtime.redo.is_empty());
    }
}

#[test]
fn oversized_named_projection_refuses_atomically() {
    let inputs = (0..32)
        .map(|index| NewTableColumnInput {
            name: format!("{index:02}{}", "\"".repeat(898)),
            field_type: "text".into(),
        })
        .collect::<Vec<_>>();
    let mut runtime = DesignerRuntime::new_table(OCCURRENCE, "Orders", &inputs).unwrap();
    let field = runtime.collection_specs["orders"].columns[0].id.to_string();
    let before = runtime.session.export_snapshot();
    let error = runtime
        .rename_column("resident/0", "orders", &field, &"\"".repeat(4096))
        .unwrap_err();
    assert!(matches!(
        error,
        DesignerError::ProjectionTooLarge { .. } | DesignerError::UnsupportedProject { .. }
    ));
    assert_eq!(runtime.session.export_snapshot(), before);
    assert!(runtime.undo.is_empty());
    assert!(runtime.redo.is_empty());
}

#[test]
fn accumulated_name_failure_reply_is_bounded_and_retains_every_input() {
    let inputs = (0..8)
        .flat_map(|group| {
            let name = format!("g{group}{}", "\"".repeat(1550));
            [
                NewTableColumnInput {
                    name: name.clone(),
                    field_type: "text".into(),
                },
                NewTableColumnInput {
                    name,
                    field_type: "text".into(),
                },
            ]
        })
        .collect::<Vec<_>>();
    let request = serde_json::json!({"type": "new_table", "occurrence_id": OCCURRENCE, "name": "Names", "columns": inputs});
    let input = serde_json::to_vec(&request).unwrap();
    assert!(input.len() < super::MAX_PROJECTION_BYTES);
    let mut resident = Some(DesignerRuntime::tracker(OCCURRENCE).unwrap());
    let before = resident.as_ref().unwrap().session.export_snapshot();
    let bytes = process_wire_request(&mut resident, &input);
    assert!(
        bytes.len() <= super::MAX_PROJECTION_BYTES,
        "reply grew to {} bytes",
        bytes.len()
    );
    let super::DesignerWireReply::Error { error } = serde_json::from_slice(&bytes).unwrap() else {
        panic!("duplicate groups refuse before candidate creation");
    };
    assert_eq!(error.code, "invalid_table_operation");
    assert_eq!(error.name_failures.len(), 8);
    for (group, finding) in error.name_failures.iter().enumerate() {
        assert_eq!(finding.reason, "duplicate_display_name");
        assert_eq!(
            finding.inputs,
            [
                name_admission::NameInput::Column {
                    index: group * 2,
                    schema: None
                },
                name_admission::NameInput::Column {
                    index: group * 2 + 1,
                    schema: None
                }
            ]
        );
    }
    assert_eq!(resident.as_ref().unwrap().session.export_snapshot(), before);
}
