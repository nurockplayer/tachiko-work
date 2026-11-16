//! #813: Cut → Paste plans from originals and publishes one history unit.
use std::{collections::BTreeMap, time::Instant};

use serde_json::json;
use tachiko_designer_runtime::{
    CalculationProjection, CellMove, DesignerRequest, DesignerResponse, DesignerRuntime,
    DesignerWireReply, FieldProjection, FieldTarget, PublicationProjection, StoredValueProjection,
    TableProjection, open_project, process_wire_request,
};
use tachiko_workspace_engine::{
    ColumnRef, Date, Document, Entity, EntityId, EntityKey, Expression, FieldConstraint,
    FieldDefinition, FieldId, FieldKey, FieldRef, FieldType, Number, NumberFormat,
    NumberFormatKind, Schema, SchemaId, SchemaKey, Value,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000813";
type Session = Option<DesignerRuntime>;

fn number(value: u32) -> Value {
    Value::Number(Number::new(f64::from(value)).unwrap())
}

fn document(rows: u32) -> Document {
    let mut document = Document::empty("move_cells", "Move cells");
    let schema = SchemaId::from("rows");
    let definitions = [
        ("n", FieldType::Number),
        ("dep", FieldType::Number),
        ("req", FieldType::Number),
        ("text", FieldType::Text),
        ("date", FieldType::Date),
        ("flag", FieldType::Boolean),
    ];
    document.schemas.insert(
        schema.clone(),
        Schema {
            id: schema.clone(),
            key: SchemaKey::from("rows"),
            display_name: None,
            field_order: None,
            fields: definitions
                .into_iter()
                .map(|(name, field_type)| {
                    let id = FieldId::from(name);
                    (
                        id.clone(),
                        FieldDefinition {
                            id,
                            key: FieldKey::from(name),
                            display_name: None,
                            number_format: None,
                            date_format: None,
                            field_type,
                            required: name == "req",
                            constraint: FieldConstraint::None,
                        },
                    )
                })
                .collect(),
        },
    );
    for row in 0..rows {
        let id = format!("r{row}");
        document.entities.insert(
            EntityId::from(id.clone()),
            Entity {
                id: EntityId::from(id.clone()),
                key: EntityKey::from(id),
                schema: schema.clone(),
                fields: BTreeMap::from([
                    (FieldId::from("n"), number(row + 1)),
                    (FieldId::from("dep"), number(row + 10)),
                    (FieldId::from("req"), number(row + 100)),
                    (FieldId::from("text"), Value::Text(format!("text {row}"))),
                    (
                        FieldId::from("date"),
                        Value::Date(Date::parse("2026-11-16").unwrap()),
                    ),
                    (FieldId::from("flag"), Value::Boolean(row % 2 == 0)),
                ]),
            },
        );
    }
    document
}

#[allow(clippy::unnecessary_wraps)] // The helpers take the runtime out of the Option and put it back.
fn fixture(document: Document) -> Session {
    Some(DesignerRuntime::from_document(document, OCCURRENCE).unwrap())
}

fn set(document: &mut Document, target: &str, value: Option<Value>) {
    let (entity, field) = target.split_once('.').unwrap();
    let fields = &mut document
        .entities
        .get_mut(&EntityId::from(entity))
        .unwrap()
        .fields;
    if let Some(value) = value {
        fields.insert(FieldId::from(field), value);
    } else {
        fields.remove(&FieldId::from(field));
    }
}

fn move_request(revision: &str, pairs: &[(&str, &str)]) -> DesignerRequest {
    DesignerRequest::MoveCells {
        expected_revision: revision.to_owned(),
        moves: pairs
            .iter()
            .map(|(from, to)| CellMove {
                from: FieldTarget::from(*from),
                to: FieldTarget::from(*to),
            })
            .collect(),
    }
}

fn publish(runtime: &mut Session, request: DesignerRequest) -> PublicationProjection {
    let DesignerResponse::Published(publication) =
        runtime.as_mut().unwrap().handle(request).unwrap()
    else {
        panic!("expected one publication");
    };
    publication
}

fn history(runtime: &mut Session, revision: u32, redo: bool) {
    let expected_revision = format!("resident/{revision}");
    let request = if redo {
        DesignerRequest::Redo { expected_revision }
    } else {
        DesignerRequest::Undo { expected_revision }
    };
    let publication = publish(runtime, request);
    assert_eq!(publication.base_revision, format!("resident/{revision}"));
    assert_eq!(
        publication.resulting_revision,
        format!("resident/{}", revision + 1)
    );
}

fn table(runtime: &mut Session, collection: &str) -> TableProjection {
    let DesignerResponse::Table(table) = runtime
        .as_mut()
        .unwrap()
        .handle(DesignerRequest::QueryTable {
            collection: collection.to_owned(),
        })
        .unwrap()
    else {
        panic!("expected table");
    };
    table
}

fn field(runtime: &mut Session, revision: u32, target: &str) -> FieldProjection {
    let DesignerResponse::Fields(mut fields) = runtime
        .as_mut()
        .unwrap()
        .handle(DesignerRequest::QueryFields {
            expected_revision: format!("resident/{revision}"),
            fields: vec![target.into()],
        })
        .unwrap()
    else {
        panic!("expected field");
    };
    fields.fields.remove(0)
}

fn assert_number(runtime: &mut Session, revision: u32, target: &str, value: Option<f64>) {
    let projection = field(runtime, revision, target);
    assert_eq!(
        projection
            .stored
            .as_ref()
            .and_then(StoredValueProjection::number),
        value
    );
    assert!(projection.formula.is_none());
}

fn assert_formula(runtime: &mut Session, revision: u32, target: &str, value: f64, source: &str) {
    let projection = field(runtime, revision, target);
    assert!(
        projection.stored.is_none(),
        "formula must not become its cached Number"
    );
    assert_eq!(projection.formula.unwrap().source, source);
    assert_eq!(
        projection
            .calculated
            .as_ref()
            .and_then(CalculationProjection::number),
        Some(value)
    );
}

fn saved(runtime: &Session, revision: &str) -> Vec<u8> {
    runtime
        .as_ref()
        .unwrap()
        .export_project_v3(revision)
        .unwrap()
        .bytes
}

fn refused(runtime: &mut Session, request: DesignerRequest, code: &str) {
    let before = table(runtime, "rows");
    let bytes = saved(runtime, &before.revision);
    let error = runtime.as_mut().unwrap().handle(request).unwrap_err();
    let failure = error.failure_projection(&before.revision);
    assert_eq!(failure.code, code, "{error}");
    assert_eq!(failure.current_revision, before.revision);
    assert_eq!(table(runtime, "rows"), before);
    assert_eq!(saved(runtime, &before.revision), bytes);
}

#[test]
fn one_move_is_one_revision_and_undo_redo_restores_overwritten_destination() {
    let mut runtime = fixture(document(5));
    let before = saved(&runtime, "resident/0");
    let publication = publish(
        &mut runtime,
        move_request("resident/0", &[("r0.n", "r1.n")]),
    );
    assert_eq!(publication.base_revision, "resident/0");
    assert_eq!(publication.resulting_revision, "resident/1");
    assert_eq!(publication.fields.len(), 2);
    assert!(publication.fields.contains(&"r0.n".into()));
    assert!(publication.fields.contains(&"r1.n".into()));
    assert_number(&mut runtime, 1, "r0.n", None);
    assert_number(&mut runtime, 1, "r1.n", Some(1.0));
    let after = saved(&runtime, "resident/1");
    history(&mut runtime, 1, false);
    assert_eq!(saved(&runtime, "resident/2"), before);
    assert_number(&mut runtime, 2, "r1.n", Some(2.0));
    history(&mut runtime, 2, true);
    assert_eq!(saved(&runtime, "resident/3"), after);
    refused(
        &mut runtime,
        DesignerRequest::Redo {
            expected_revision: "resident/3".into(),
        },
        "invalid_tracker_operation",
    );
    history(&mut runtime, 3, false);
    refused(
        &mut runtime,
        DesignerRequest::Undo {
            expected_revision: "resident/4".into(),
        },
        "invalid_tracker_operation",
    );
}

fn permutation(pairs: &[(&str, &str)], expected: &[Option<f64>]) {
    let mut runtime = fixture(document(5));
    let before = saved(&runtime, "resident/0");
    publish(&mut runtime, move_request("resident/0", pairs));
    for (row, value) in expected.iter().enumerate() {
        assert_number(&mut runtime, 1, &format!("r{row}.n"), *value);
    }
    let after = saved(&runtime, "resident/1");
    history(&mut runtime, 1, false);
    assert_eq!(saved(&runtime, "resident/2"), before);
    history(&mut runtime, 2, true);
    assert_eq!(saved(&runtime, "resident/3"), after);
}

#[test]
fn overlapping_shift_down_over_five_rows_uses_originals() {
    permutation(
        &[
            ("r0.n", "r1.n"),
            ("r1.n", "r2.n"),
            ("r2.n", "r3.n"),
            ("r3.n", "r4.n"),
        ],
        &[None, Some(1.0), Some(2.0), Some(3.0), Some(4.0)],
    );
}

#[test]
fn overlapping_shift_up_over_five_rows_uses_originals() {
    permutation(
        &[
            ("r1.n", "r0.n"),
            ("r2.n", "r1.n"),
            ("r3.n", "r2.n"),
            ("r4.n", "r3.n"),
        ],
        &[Some(2.0), Some(3.0), Some(4.0), Some(5.0), None],
    );
}

#[test]
fn swap_uses_originals() {
    permutation(
        &[("r0.n", "r1.n"), ("r1.n", "r0.n")],
        &[Some(2.0), Some(1.0), Some(3.0), Some(4.0), Some(5.0)],
    );
}

#[test]
fn three_cycle_uses_originals() {
    permutation(
        &[("r0.n", "r1.n"), ("r1.n", "r2.n"), ("r2.n", "r0.n")],
        &[Some(3.0), Some(1.0), Some(2.0), Some(4.0), Some(5.0)],
    );
}

fn formula_history(document: Document, expected_source: &str) {
    let mut runtime = fixture(document);
    let before = saved(&runtime, "resident/0");
    publish(
        &mut runtime,
        move_request("resident/0", &[("r0.n", "r1.n")]),
    );
    assert_formula(&mut runtime, 1, "r1.n", 10.0, expected_source);
    assert_number(&mut runtime, 1, "r0.n", None);
    let after = saved(&runtime, "resident/1");
    history(&mut runtime, 1, false);
    assert_eq!(saved(&runtime, "resident/2"), before);
    history(&mut runtime, 2, true);
    assert_eq!(saved(&runtime, "resident/3"), after);
    assert_formula(&mut runtime, 3, "r1.n", 10.0, expected_source);
}

#[test]
fn same_row_formula_move_keeps_original_source_row() {
    let mut doc = document(5);
    set(
        &mut doc,
        "r0.n",
        Some(Value::Formula(Expression::SameRowReference(FieldId::from(
            "dep",
        )))),
    );
    formula_history(doc, "[r0.dep]");
}

#[test]
fn explicit_reference_formula_move_keeps_reference() {
    let mut doc = document(5);
    set(
        &mut doc,
        "r0.n",
        Some(Value::Formula(Expression::Reference(FieldRef::new(
            "r0", "dep",
        )))),
    );
    formula_history(doc, "[r0.dep]");
}

#[test]
fn formula_moves_into_blank_destination_and_history_restores_absence() {
    let mut doc = document(5);
    set(
        &mut doc,
        "r0.n",
        Some(Value::Formula(Expression::SameRowReference(FieldId::from(
            "dep",
        )))),
    );
    set(&mut doc, "r1.n", None);
    formula_history(doc, "[r0.dep]");
}

#[test]
fn formula_replaces_formula_destination_and_undo_restores_both_formulas() {
    let mut doc = document(5);
    set(
        &mut doc,
        "r0.n",
        Some(Value::Formula(Expression::SameRowReference(FieldId::from(
            "dep",
        )))),
    );
    set(
        &mut doc,
        "r1.n",
        Some(Value::Formula(Expression::Number(
            Number::new(42.0).unwrap(),
        ))),
    );
    formula_history(doc, "[r0.dep]");
}

#[test]
fn nested_same_row_reference_keeps_original_source_row() {
    let mut doc = document(5);
    set(
        &mut doc,
        "r0.n",
        Some(Value::Formula(Expression::Abs(Box::new(
            Expression::SameRowReference(FieldId::from("dep")),
        )))),
    );
    formula_history(doc, "ABS([r0.dep])");
}

#[test]
fn scalar_replaces_formula_destination_and_undo_restores_formula() {
    let mut doc = document(5);
    set(
        &mut doc,
        "r1.n",
        Some(Value::Formula(Expression::SameRowReference(FieldId::from(
            "dep",
        )))),
    );
    let mut runtime = fixture(doc);
    let before = saved(&runtime, "resident/0");
    publish(
        &mut runtime,
        move_request("resident/0", &[("r0.n", "r1.n")]),
    );
    assert_number(&mut runtime, 1, "r1.n", Some(1.0));
    history(&mut runtime, 1, false);
    assert_eq!(saved(&runtime, "resident/2"), before);
    assert_formula(&mut runtime, 2, "r1.n", 11.0, "[.dep]");
    history(&mut runtime, 2, true);
    assert_number(&mut runtime, 3, "r1.n", Some(1.0));
}

#[test]
fn same_row_formula_stays_relative_when_only_column_changes() {
    let mut doc = document(5);
    set(
        &mut doc,
        "r0.n",
        Some(Value::Formula(Expression::SameRowReference(FieldId::from(
            "dep",
        )))),
    );
    let mut runtime = fixture(doc);
    publish(
        &mut runtime,
        move_request("resident/0", &[("r0.n", "r0.req")]),
    );
    assert_formula(&mut runtime, 1, "r0.req", 10.0, "[.dep]");
}

#[test]
fn column_reference_formula_keeps_column_reference() {
    let mut doc = document(5);
    set(
        &mut doc,
        "r0.n",
        Some(Value::Formula(Expression::ColumnTotal(ColumnRef::new(
            "rows", "dep",
        )))),
    );
    let mut runtime = fixture(doc);
    let source = field(&mut runtime, 0, "r0.n").formula.unwrap().source;
    publish(
        &mut runtime,
        move_request("resident/0", &[("r0.n", "r1.n")]),
    );
    assert_formula(&mut runtime, 1, "r1.n", 60.0, &source);
}

#[test]
fn cross_collection_move_and_history_preserve_both_collections() {
    let mut doc = document(5);
    let mut schema = doc.schemas[&SchemaId::from("rows")].clone();
    schema.id = SchemaId::from("other");
    schema.key = SchemaKey::from("other");
    doc.schemas.insert(schema.id.clone(), schema);
    doc.entities.get_mut(&EntityId::from("r4")).unwrap().schema = SchemaId::from("other");
    let mut runtime = fixture(doc);
    let before = saved(&runtime, "resident/0");
    publish(
        &mut runtime,
        move_request("resident/0", &[("r0.n", "r4.n")]),
    );
    assert_number(&mut runtime, 1, "r0.n", None);
    assert_number(&mut runtime, 1, "r4.n", Some(1.0));
    assert_eq!(table(&mut runtime, "other").revision, "resident/1");
    history(&mut runtime, 1, false);
    assert_eq!(saved(&runtime, "resident/2"), before);
}

#[test]
fn blank_source_moves_absence_and_undo_restores_destination() {
    let mut doc = document(5);
    set(&mut doc, "r0.n", None);
    let mut runtime = fixture(doc);
    let before = saved(&runtime, "resident/0");
    publish(
        &mut runtime,
        move_request("resident/0", &[("r0.n", "r1.n")]),
    );
    assert_number(&mut runtime, 1, "r1.n", None);
    history(&mut runtime, 1, false);
    assert_eq!(saved(&runtime, "resident/2"), before);
}

#[test]
fn destination_keeps_its_number_format() {
    let mut doc = document(5);
    let fields = &mut doc.schemas.get_mut(&SchemaId::from("rows")).unwrap().fields;
    fields.get_mut(&FieldId::from("n")).unwrap().number_format = Some(NumberFormat {
        decimals: 3,
        kind: NumberFormatKind::Plain,
        thousands_separator: true,
    });
    fields.get_mut(&FieldId::from("dep")).unwrap().number_format = Some(NumberFormat {
        decimals: 1,
        kind: NumberFormatKind::Percent,
        thousands_separator: false,
    });
    let mut runtime = fixture(doc);
    let before = table(&mut runtime, "rows").columns;
    publish(
        &mut runtime,
        move_request("resident/0", &[("r0.n", "r1.dep")]),
    );
    assert_eq!(table(&mut runtime, "rows").columns, before);
    history(&mut runtime, 1, false);
    assert_eq!(table(&mut runtime, "rows").columns, before);
}

#[test]
fn blank_source_cannot_clear_required_destination() {
    let mut doc = document(5);
    set(&mut doc, "r0.n", None);
    let mut runtime = fixture(doc);
    refused(
        &mut runtime,
        move_request("resident/0", &[("r0.n", "r1.req")]),
        "invalid_table_operation",
    );
}

#[test]
fn type_mismatch_refuses_entire_batch_without_coercion() {
    for pair in [
        ("r2.text", "r3.date"),
        ("r2.n", "r3.text"),
        ("r2.date", "r3.n"),
    ] {
        let mut runtime = fixture(document(5));
        refused(
            &mut runtime,
            move_request("resident/0", &[("r0.n", "r1.n"), pair]),
            "unsupported_edit",
        );
    }
}

#[test]
fn formula_to_text_refuses_without_using_calculated_number() {
    let mut doc = document(5);
    set(
        &mut doc,
        "r0.n",
        Some(Value::Formula(Expression::SameRowReference(FieldId::from(
            "dep",
        )))),
    );
    let mut runtime = fixture(doc);
    refused(
        &mut runtime,
        move_request("resident/0", &[("r0.n", "r1.text")]),
        "invalid_tracker_operation",
    );
}

#[test]
fn required_source_cannot_be_blanked() {
    let mut runtime = fixture(document(5));
    refused(
        &mut runtime,
        move_request("resident/0", &[("r0.req", "r1.n")]),
        "invalid_table_operation",
    );
}

#[test]
fn required_source_that_is_also_destination_can_move() {
    let mut runtime = fixture(document(5));
    publish(
        &mut runtime,
        move_request("resident/0", &[("r0.req", "r1.req"), ("r1.req", "r0.req")]),
    );
    assert_number(&mut runtime, 1, "r0.req", Some(101.0));
    assert_number(&mut runtime, 1, "r1.req", Some(100.0));
    history(&mut runtime, 1, false);
    assert_number(&mut runtime, 2, "r0.req", Some(100.0));
}

#[test]
fn duplicate_source_refuses_atomically() {
    let mut runtime = fixture(document(5));
    refused(
        &mut runtime,
        move_request("resident/0", &[("r0.n", "r1.n"), ("r0.n", "r2.n")]),
        "invalid_tracker_operation",
    );
}

#[test]
fn duplicate_destination_refuses_atomically() {
    let mut runtime = fixture(document(5));
    refused(
        &mut runtime,
        move_request("resident/0", &[("r0.n", "r2.n"), ("r1.n", "r2.n")]),
        "invalid_tracker_operation",
    );
}

#[test]
fn stale_revision_refuses_before_resolving_targets() {
    let mut runtime = fixture(document(5));
    refused(
        &mut runtime,
        move_request("resident/stale", &[("missing.n", "r1.n")]),
        "stale_revision",
    );
}

#[test]
fn unknown_entity_refuses_atomically() {
    for pair in [("missing.n", "r1.n"), ("r0.n", "missing.n")] {
        let mut runtime = fixture(document(5));
        refused(
            &mut runtime,
            move_request("resident/0", &[("r2.n", "r3.n"), pair]),
            "unsupported_edit",
        );
    }
}

#[test]
fn undeclared_field_refuses_atomically() {
    for pair in [("r0.missing", "r1.n"), ("r0.n", "r1.missing")] {
        let mut runtime = fixture(document(5));
        refused(
            &mut runtime,
            move_request("resident/0", &[("r2.n", "r3.n"), pair]),
            "unsupported_edit",
        );
    }
}

#[test]
fn empty_moves_refuse() {
    let mut runtime = fixture(document(5));
    refused(
        &mut runtime,
        move_request("resident/0", &[]),
        "invalid_tracker_operation",
    );
}

#[test]
fn union_over_batch_limit_refuses_even_when_move_count_is_within_limit() {
    let mut runtime = fixture(document(5));
    let moves = (0..513)
        .map(|row| CellMove {
            from: FieldTarget {
                entity: format!("source{row}"),
                field: "n".into(),
            },
            to: FieldTarget {
                entity: format!("destination{row}"),
                field: "n".into(),
            },
        })
        .collect();
    refused(
        &mut runtime,
        DesignerRequest::MoveCells {
            expected_revision: "resident/0".into(),
            moves,
        },
        "invalid_tracker_operation",
    );
}

#[test]
fn all_self_moves_return_no_change_and_leave_history_empty() {
    let mut runtime = fixture(document(5));
    refused(
        &mut runtime,
        move_request("resident/0", &[("r0.n", "r0.n"), ("r1.req", "r1.req")]),
        "no_change",
    );
    refused(
        &mut runtime,
        DesignerRequest::Undo {
            expected_revision: "resident/0".into(),
        },
        "invalid_tracker_operation",
    );
}

#[test]
fn equal_contents_cycle_returns_no_change() {
    let mut doc = document(5);
    set(&mut doc, "r1.n", Some(number(1)));
    let mut runtime = fixture(doc);
    refused(
        &mut runtime,
        move_request("resident/0", &[("r0.n", "r1.n"), ("r1.n", "r0.n")]),
        "no_change",
    );
}

#[test]
fn equal_destination_still_clears_source() {
    let mut doc = document(5);
    set(&mut doc, "r1.n", Some(number(1)));
    let mut runtime = fixture(doc);
    publish(
        &mut runtime,
        move_request("resident/0", &[("r0.n", "r1.n")]),
    );
    assert_number(&mut runtime, 1, "r0.n", None);
    assert_number(&mut runtime, 1, "r1.n", Some(1.0));
}

#[test]
fn text_date_and_boolean_moves_preserve_exact_values() {
    let mut runtime = fixture(document(5));
    let originals =
        ["text", "date", "flag"].map(|name| field(&mut runtime, 0, &format!("r0.{name}")).stored);
    publish(
        &mut runtime,
        move_request(
            "resident/0",
            &[
                ("r0.text", "r1.text"),
                ("r0.date", "r1.date"),
                ("r0.flag", "r1.flag"),
            ],
        ),
    );
    for (name, expected) in ["text", "date", "flag"].into_iter().zip(originals) {
        assert_eq!(
            field(&mut runtime, 1, &format!("r1.{name}")).stored,
            expected
        );
        assert!(
            field(&mut runtime, 1, &format!("r0.{name}"))
                .stored
                .is_none()
        );
    }
}

#[test]
fn inbound_formula_keeps_existing_target_after_move() {
    let mut doc = document(5);
    set(
        &mut doc,
        "r4.dep",
        Some(Value::Formula(Expression::Reference(FieldRef::new(
            "r0", "n",
        )))),
    );
    let mut runtime = fixture(doc);
    publish(
        &mut runtime,
        move_request("resident/0", &[("r0.n", "r1.n")]),
    );
    // A cleared optional Number reads as zero under the ordinary calculation rule.
    assert_formula(&mut runtime, 1, "r4.dep", 0.0, "[r0.n]");
}

#[test]
fn calculation_refusal_does_not_publish_partial_move() {
    let mut doc = document(5);
    set(
        &mut doc,
        "r4.dep",
        Some(Value::Formula(Expression::Divide {
            left: Box::new(Expression::Number(Number::new(1.0).unwrap())),
            right: Box::new(Expression::Reference(FieldRef::new("r0", "n"))),
        })),
    );
    let mut runtime = fixture(doc);
    let before = table(&mut runtime, "rows");
    let bytes = saved(&runtime, "resident/0");
    assert!(
        runtime
            .as_mut()
            .unwrap()
            .handle(move_request("resident/0", &[("r0.n", "r1.n")]))
            .is_err()
    );
    assert_eq!(table(&mut runtime, "rows"), before);
    assert_eq!(saved(&runtime, "resident/0"), bytes);
}

#[test]
fn refusals_preserve_existing_undo_and_redo() {
    let mut runtime = fixture(document(5));
    publish(
        &mut runtime,
        move_request("resident/0", &[("r0.n", "r1.n")]),
    );
    publish(
        &mut runtime,
        move_request("resident/1", &[("r2.n", "r3.n")]),
    );
    history(&mut runtime, 2, false);
    refused(
        &mut runtime,
        move_request("resident/3", &[("r0.req", "r4.n")]),
        "invalid_table_operation",
    );
    history(&mut runtime, 3, true);
    assert_number(&mut runtime, 4, "r3.n", Some(3.0));
    history(&mut runtime, 4, false);
    history(&mut runtime, 5, false);
    assert_number(&mut runtime, 6, "r0.n", Some(1.0));
    assert_number(&mut runtime, 6, "r1.n", Some(2.0));
}

#[test]
fn working_size_moves_ten_thousand_cells_in_one_wire_request() {
    let mut runtime =
        Some(DesignerRuntime::from_document_working_size(document(10_000), OCCURRENCE).unwrap());
    let moves = (0..10_000)
        .map(|row| CellMove {
            from: FieldTarget {
                entity: format!("r{row}"),
                field: "n".into(),
            },
            to: FieldTarget {
                entity: format!("r{row}"),
                field: "dep".into(),
            },
        })
        .collect();
    let request = DesignerRequest::MoveCells {
        expected_revision: "resident/0".into(),
        moves,
    };
    let bytes = serde_json::to_vec(&request).unwrap();
    let start = Instant::now();
    let reply: DesignerWireReply =
        serde_json::from_slice(&process_wire_request(&mut runtime, &bytes)).unwrap();
    println!("move_cells 10000 cells: {} ms", start.elapsed().as_millis());
    let DesignerWireReply::Ok {
        response: DesignerResponse::Published(publication),
    } = reply
    else {
        panic!("working-size move must publish: {reply:?}");
    };
    assert_eq!(publication.base_revision, "resident/0");
    assert_eq!(publication.resulting_revision, "resident/1");
    for row in [0, 5_000, 9_999] {
        assert_number(&mut runtime, 1, &format!("r{row}.n"), None);
        assert_number(
            &mut runtime,
            1,
            &format!("r{row}.dep"),
            Some(f64::from(row + 1)),
        );
    }
    history(&mut runtime, 1, false);
    assert_number(&mut runtime, 2, "r9999.dep", Some(10_009.0));
    history(&mut runtime, 2, true);
    assert_number(&mut runtime, 3, "r9999.dep", Some(10_000.0));
}

#[test]
fn save_and_reopen_keep_moved_contents_and_formula_meaning() {
    let mut doc = document(5);
    set(
        &mut doc,
        "r0.n",
        Some(Value::Formula(Expression::SameRowReference(FieldId::from(
            "dep",
        )))),
    );
    let mut runtime = fixture(doc);
    publish(
        &mut runtime,
        move_request("resident/0", &[("r0.n", "r1.n")]),
    );
    let bytes = saved(&runtime, "resident/1");
    let mut reopened = None;
    open_project(
        &mut reopened,
        &bytes,
        "00000000-0000-4000-8000-000000000814",
    )
    .unwrap();
    assert_number(&mut reopened, 0, "r0.n", None);
    assert_formula(&mut reopened, 0, "r1.n", 10.0, "[r0.dep]");
}

#[test]
fn json_wire_round_trip_preserves_request_name_and_fields() {
    let input = json!({"type":"move_cells", "expected_revision":"resident/0",
        "moves":[{"from":{"entity":"r0","field":"n"}, "to":{"entity":"r1","field":"n"}}]});
    let typed: DesignerRequest = serde_json::from_value(input.clone()).unwrap();
    assert_eq!(typed, move_request("resident/0", &[("r0.n", "r1.n")]));
    assert_eq!(serde_json::to_value(&typed).unwrap(), input);
    let mut runtime = fixture(document(5));
    let reply: DesignerWireReply = serde_json::from_slice(&process_wire_request(
        &mut runtime,
        &serde_json::to_vec(&input).unwrap(),
    ))
    .unwrap();
    assert!(matches!(
        reply,
        DesignerWireReply::Ok {
            response: DesignerResponse::Published(_)
        }
    ));
    assert_number(&mut runtime, 1, "r1.n", Some(1.0));
}
