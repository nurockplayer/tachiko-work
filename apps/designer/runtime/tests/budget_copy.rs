use std::collections::BTreeMap;
use tachiko_designer_runtime::{
    CalculationProjection, DesignerRequest, DesignerResponse, DesignerRuntime, FieldProjection,
    FieldTarget,
};
use tachiko_workspace_engine::{
    Document, Entity, EntityId, EntityKey, FieldDefinition, FieldId, FieldKey, FieldType, Number,
    Schema, SchemaId, SchemaKey, Value,
};
const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000000";

fn fixture() -> DesignerRuntime {
    let mut document = Document::empty("copy_fixture", "Formula copy");
    for (id, names) in [
        ("items", vec!["r1", "r2", "r3"]),
        ("summary", vec!["total"]),
    ] {
        let schema_id = SchemaId::from(id);
        document.schemas.insert(
            schema_id.clone(),
            Schema {
                id: schema_id.clone(),
                key: SchemaKey::from(id),
                fields: ["a", "b", "c", "d"]
                    .into_iter()
                    .map(|name| {
                        let field = FieldId::from(name);
                        (
                            field.clone(),
                            FieldDefinition {
                                id: field,
                                key: FieldKey::from(name),
                                field_type: FieldType::Number,
                                required: true,
                            },
                        )
                    })
                    .collect(),
            },
        );
        for (index, name) in names.into_iter().enumerate() {
            let entity_id = EntityId::from(name);
            document.entities.insert(
                entity_id.clone(),
                Entity {
                    id: entity_id,
                    key: EntityKey::from(name),
                    schema: schema_id.clone(),
                    fields: ["a", "b", "c", "d"]
                        .into_iter()
                        .map(|name| {
                            (
                                FieldId::from(name),
                                Value::Number(
                                    Number::new(
                                        (f64::from(u32::try_from(index).unwrap()) + 1.0) * 10.0,
                                    )
                                    .unwrap(),
                                ),
                            )
                        })
                        .collect::<BTreeMap<_, _>>(),
                },
            );
        }
    }
    DesignerRuntime::from_document(document, OCCURRENCE).unwrap()
}
fn update(runtime: &mut DesignerRuntime, revision: u32, target: &str, source: &str) {
    runtime
        .handle(DesignerRequest::FormulaUpdate {
            expected_revision: format!("resident/{revision}"),
            target: target.into(),
            source: source.into(),
        })
        .unwrap();
}
fn copy(
    revision: u32,
    source: &str,
    destinations: &[&str],
    fixed: &[&str],
    rows: bool,
    columns: bool,
) -> DesignerRequest {
    DesignerRequest::CopyFormula {
        expected_revision: format!("resident/{revision}"),
        source: source.into(),
        destinations: destinations
            .iter()
            .map(|target| FieldTarget::from(*target))
            .collect(),
        fixed_references: fixed
            .iter()
            .map(|target| FieldTarget::from(*target))
            .collect(),
        relative_rows: rows,
        relative_columns: columns,
    }
}
fn field(runtime: &mut DesignerRuntime, revision: u32, target: &str) -> FieldProjection {
    let DesignerResponse::Fields(mut projection) = runtime
        .handle(DesignerRequest::QueryFields {
            expected_revision: format!("resident/{revision}"),
            fields: vec![target.into()],
        })
        .unwrap()
    else {
        panic!("expected fields")
    };
    projection.fields.remove(0)
}
#[test]
fn copy_resolves_relative_axes_fixed_and_cross_collection_references_atomically() {
    let mut runtime = fixture();
    update(&mut runtime, 0, "r1.c", "[r1.a] + [r1.b] + [total.a]");
    runtime
        .handle(copy(1, "r1.c", &["r2.c", "r3.c"], &["r1.b"], true, false))
        .unwrap();
    let second = field(&mut runtime, 2, "r2.c");
    assert_eq!(
        second.formula.unwrap().source,
        "(([r2.a] + [r1.b]) + [total.a])"
    );
    assert_eq!(
        second
            .calculated
            .as_ref()
            .and_then(CalculationProjection::number),
        Some(40.0)
    );
    assert_eq!(
        field(&mut runtime, 2, "r3.c")
            .calculated
            .as_ref()
            .and_then(CalculationProjection::number),
        Some(50.0)
    );
    assert!(
        runtime
            .handle(DesignerRequest::Undo {
                expected_revision: "resident/2".into()
            })
            .is_err()
    );
    let exported = runtime.export_project("resident/2").unwrap();
    let mut reopened = None;
    tachiko_designer_runtime::open_project(
        &mut reopened,
        &exported.bytes,
        "00000000-0000-4000-8000-000000000001",
    )
    .unwrap();
    let mut reopened = reopened.unwrap();
    assert_eq!(
        field(&mut reopened, 0, "r2.c").formula.unwrap().source,
        "(([r2.a] + [r1.b]) + [total.a])"
    );
    assert_eq!(
        runtime.export_project("resident/2").unwrap().bytes,
        reopened.export_project("resident/0").unwrap().bytes
    );
}
#[test]
fn copy_column_offset_uses_canonical_columns_and_absolute_mode_preserves_ids() {
    let mut runtime = fixture();
    update(&mut runtime, 0, "r1.c", "[r1.a] * 2");
    runtime
        .handle(copy(1, "r1.c", &["r2.d"], &[], true, true))
        .unwrap();
    assert_eq!(
        field(&mut runtime, 2, "r2.d").formula.unwrap().source,
        "([r2.b] * 2)"
    );
    runtime
        .handle(copy(2, "r1.c", &["r3.d"], &[], false, false))
        .unwrap();
    assert_eq!(
        field(&mut runtime, 3, "r3.d").formula.unwrap().source,
        "([r1.a] * 2)"
    );
}
#[test]
fn rejected_copy_preserves_the_complete_snapshot_and_previous_history() {
    let mut runtime = fixture();
    update(&mut runtime, 0, "r1.c", "[r2.a] * 2");
    runtime
        .handle(DesignerRequest::EditCells {
            expected_revision: "resident/1".into(),
            edits: vec![tachiko_designer_runtime::CellEdit {
                target: "r1.d".into(),
                input: tachiko_designer_runtime::ScalarEditInput::Number { input: "15".into() },
            }],
        })
        .unwrap();
    let before = runtime.export_project("resident/2").unwrap();
    for request in [
        copy(1, "r1.c", &["r2.c"], &[], true, false),
        copy(2, "r1.c", &["r2.c", "r2.c"], &[], true, false),
        copy(2, "r1.c", &["r2.c", "r3.c"], &[], true, false),
        copy(2, "r1.c", &["r2.c", "absent.c"], &[], true, false),
        copy(2, "r1.c", &["total.c"], &[], false, false),
        copy(2, "r1.c", &["r2.c"], &["r1.b"], true, false),
        copy(2, "r1.a", &["r2.c"], &[], true, false),
        copy(2, "r1.c", &["r2.a"], &[], false, false), // self cycle
        copy(2, "r1.c", &["r2.a"], &[], true, true),   // negative column offset
    ] {
        assert!(runtime.handle(request).is_err());
        assert_eq!(runtime.export_project("resident/2").unwrap(), before);
    }
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/2".into(),
        })
        .unwrap();
    assert!(field(&mut runtime, 3, "r1.d").formula.is_none());
}
#[test]
fn budget_copy_rejects_nonnumeric_destination_or_shifted_reference() {
    let mut runtime = DesignerRuntime::budget(OCCURRENCE).unwrap();
    update(&mut runtime, 0, "rent.variance", "[rent.actual] * 2");
    let before = runtime.export_project("resident/1").unwrap();
    for request in [
        copy(1, "rent.variance", &["utilities.name"], &[], true, false),
        copy(1, "rent.variance", &["utilities.planned"], &[], true, true),
    ] {
        assert!(runtime.handle(request).is_err());
        assert_eq!(runtime.export_project("resident/1").unwrap(), before);
    }
}

#[test]
fn accepted_copy_clears_scalar_history_but_rejected_copy_preserves_redo() {
    let mut runtime = fixture();
    update(&mut runtime, 0, "r1.c", "[r1.a] * 2");
    runtime
        .handle(DesignerRequest::EditCells {
            expected_revision: "resident/1".into(),
            edits: vec![tachiko_designer_runtime::CellEdit {
                target: "r1.b".into(),
                input: tachiko_designer_runtime::ScalarEditInput::Number { input: "15".into() },
            }],
        })
        .unwrap();
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/2".into(),
        })
        .unwrap();
    assert!(
        runtime
            .handle(copy(3, "r1.c", &["absent.c"], &[], true, false))
            .is_err()
    );
    runtime
        .handle(DesignerRequest::Redo {
            expected_revision: "resident/3".into(),
        })
        .unwrap();
    runtime
        .handle(copy(4, "r1.c", &["r2.c"], &[], true, false))
        .unwrap();
    assert!(
        runtime
            .handle(DesignerRequest::Undo {
                expected_revision: "resident/5".into()
            })
            .is_err()
    );
    assert!(
        runtime
            .handle(DesignerRequest::Redo {
                expected_revision: "resident/5".into()
            })
            .is_err()
    );
}

fn formula_capacity_fixture() -> DesignerRuntime {
    let mut document = Document::empty("formula_capacity", "Formula capacity");
    let schema = SchemaId::from("items");
    let field_id = FieldId::from("n");
    document.schemas.insert(
        schema.clone(),
        Schema {
            id: schema.clone(),
            key: SchemaKey::from("items"),
            fields: BTreeMap::from([(
                field_id.clone(),
                FieldDefinition {
                    id: field_id.clone(),
                    key: FieldKey::from("n"),
                    field_type: FieldType::Number,
                    required: true,
                },
            )]),
        },
    );
    for index in 0..33 {
        let id = EntityId::from(format!("r{index:02}"));
        let value = if index < 32 {
            Value::Formula(tachiko_workspace_engine::Expression::Number(
                Number::new(1.0).unwrap(),
            ))
        } else {
            Value::Number(Number::new(2.0).unwrap())
        };
        document.entities.insert(
            id.clone(),
            Entity {
                id: id.clone(),
                key: EntityKey::from(if index == 32 {
                    "long_but_valid_input_reference_key".to_owned()
                } else {
                    id.to_string()
                }),
                schema: schema.clone(),
                fields: BTreeMap::from([(field_id.clone(), value)]),
            },
        );
    }
    DesignerRuntime::from_document(document, OCCURRENCE).unwrap()
}

fn assert_formula_rejection_preserves_reopen(
    runtime: &mut DesignerRuntime,
    revision: u32,
    target: &str,
    source: &str,
) {
    let revision_text = format!("resident/{revision}");
    let before = runtime.export_project(&revision_text).unwrap().bytes;
    let error = runtime
        .handle(DesignerRequest::FormulaUpdate {
            expected_revision: revision_text.clone(),
            target: target.into(),
            source: source.into(),
        })
        .expect_err("unopenable formula candidate must be rejected before publication");
    assert_eq!(
        error.failure_projection(&revision_text).code,
        "unsupported_project"
    );
    assert_eq!(
        runtime.export_project(&revision_text).unwrap().bytes,
        before
    );
    let mut reopened = None;
    tachiko_designer_runtime::open_project(
        &mut reopened,
        &before,
        "00000000-0000-4000-8000-000000000001",
    )
    .unwrap();
    assert_eq!(
        reopened
            .unwrap()
            .export_project("resident/0")
            .unwrap()
            .bytes,
        before
    );
}

#[test]
fn formula_authoring_rejects_33rd_formula_before_publication_and_keeps_32_reopenable() {
    let mut runtime = formula_capacity_fixture();
    assert_formula_rejection_preserves_reopen(&mut runtime, 0, "r32.n", "3");
}

#[test]
fn formula_authoring_preflights_complete_projection_budget_for_long_sources() {
    let mut runtime = formula_capacity_fixture();
    let mut terms = vec!["[long_but_valid_input_reference_key.n]".to_owned(); 80];
    while terms.len() > 1 {
        terms = terms
            .chunks(2)
            .map(|pair| {
                if pair.len() == 2 {
                    format!("({} + {})", pair[0], pair[1])
                } else {
                    pair[0].clone()
                }
            })
            .collect();
    }
    let source = terms.remove(0);
    let mut rejected = false;
    for index in 0..32 {
        let target = format!("r{index:02}.n");
        let before = runtime
            .export_project(&format!("resident/{index}"))
            .unwrap()
            .bytes;
        if runtime
            .handle(DesignerRequest::FormulaUpdate {
                expected_revision: format!("resident/{index}"),
                target: target.as_str().into(),
                source: source.clone(),
            })
            .is_err()
        {
            assert_eq!(
                runtime
                    .export_project(&format!("resident/{index}"))
                    .unwrap()
                    .bytes,
                before
            );
            assert_formula_rejection_preserves_reopen(&mut runtime, index, &target, &source);
            rejected = true;
            break;
        }
    }
    assert!(
        rejected,
        "32 long formulas must exceed the bounded aggregate projection"
    );
}
