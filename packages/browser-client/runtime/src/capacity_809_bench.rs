//! Native working-size regression guard for #809, following #807's ignored release test.
//! The fixture and typed request paths come from scratch PR #816; no phase hooks are needed.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use tachiko_storage::encode_roproj_v3;
use tachiko_workspace_engine::{
    Expression, FieldConstraint, Number, Value, formula_operations::FormulaCalculationOutcome,
};

use super::capacity_fixtures::entity_id;
use super::{
    CellEdit, DesignerRequest, DesignerResponse, Document, Entity, EntityKey, FieldDefinition,
    FieldId, FieldKey, FieldRef, FieldTarget, FieldType, ScalarEditInput, Schema, SchemaId,
    SchemaKey, TableView, encode_project_bundle, open_project_working_size,
};

const ROWS: usize = 10_000;
const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000809";

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn formula_table_809(rows: usize) -> Document {
    let mut document = Document::empty("formula-table-809", "Formula table");
    let types = [
        FieldType::Text,
        FieldType::Number,
        FieldType::Number,
        FieldType::Date,
        FieldType::Boolean,
        FieldType::Text,
        FieldType::Number,
        FieldType::Number,
        FieldType::Number,
        FieldType::Number,
    ];
    let mut schema = Schema {
        id: SchemaId::from("grid"),
        key: SchemaKey::from("grid"),
        display_name: None,
        field_order: None,
        fields: BTreeMap::new(),
    };
    for (column, field_type) in types.into_iter().enumerate() {
        let name = format!("column_{column}");
        schema.fields.insert(
            FieldId::from(name.clone()),
            FieldDefinition {
                id: FieldId::from(name.clone()),
                key: FieldKey::from(name),
                display_name: None,
                field_type,
                required: false,
                constraint: FieldConstraint::None,
                number_format: None,
                date_format: None,
            },
        );
    }
    document.schemas.insert(schema.id.clone(), schema);
    for row in 0..rows {
        let id = entity_id(row);
        let mut fields = [
            Value::Text(format!("Row {}", row + 1)),
            number(f64::from(u32::try_from(row + 1).unwrap())),
            number(2.0),
            Value::Date(tachiko_workspace_engine::Date::new(2026, 10, 20).unwrap()),
            Value::Boolean(true),
            Value::Text("Filled".to_owned()),
            number(3.0),
        ]
        .into_iter()
        .enumerate()
        .map(|(c, v)| (FieldId::from(format!("column_{c}")), v))
        .collect::<BTreeMap<_, _>>();
        for (c, left, right) in [(7, 1, 1), (8, 1, 2), (9, 2, 6)] {
            let reference = |column| {
                Box::new(Expression::Reference(FieldRef::new(
                    id.clone(),
                    FieldId::from(format!("column_{column}")),
                )))
            };
            fields.insert(
                FieldId::from(format!("column_{c}")),
                Value::Formula(Expression::Add {
                    left: reference(left),
                    right: reference(right),
                }),
            );
        }
        document.entities.insert(
            id.clone(),
            Entity {
                rank: None,
                id,
                key: EntityKey::from(format!("r{row:05}")),
                schema: SchemaId::from("grid"),
                fields,
            },
        );
    }
    document
}

fn measure<R>(action: impl FnOnce() -> R) -> (R, Duration) {
    let start = Instant::now();
    let result = action();
    (result, start.elapsed())
}

fn report(step: &str, samples: &mut [Duration]) -> Duration {
    samples.sort_unstable();
    // Nearest-rank percentiles, matching the scratch measurement; no discarded warmup.
    let median = samples[(samples.len() - 1) / 2];
    let p95 = samples[(samples.len() * 95).div_ceil(100) - 1];
    println!(
        "{step}: n={}, p50_ms={:.3}, p95_ms={:.3}, max_ms={:.3}",
        samples.len(),
        median.as_secs_f64() * 1_000.0,
        p95.as_secs_f64() * 1_000.0,
        samples.last().unwrap().as_secs_f64() * 1_000.0,
    );
    median
}

#[test]
#[ignore = "#809 capacity guard; run explicitly in release with --features capacity-bench"]
fn capacity_809_formula_table() {
    assert!(!cfg!(debug_assertions), "run this guard with --release");
    let started = Instant::now();
    let document = formula_table_809(ROWS);
    let tree = encode_roproj_v3(&document).unwrap();
    let bytes =
        encode_project_bundle(tree.files().iter().map(|file| (file.path(), file.bytes()))).unwrap();
    drop(tree);
    drop(document);

    let mut resident = None;
    let mut opens = Vec::new();
    for _ in 0..3 {
        // Replacing the preceding resident (including its destruction) stays inside the timer.
        let (opened, elapsed) =
            measure(|| open_project_working_size(&mut resident, &bytes, OCCURRENCE).unwrap());
        opens.push(elapsed);
        assert_eq!(opened.window.total_rows, ROWS);
        assert_eq!(opened.window.columns.len(), 10);
    }
    let mut runtime = resident.unwrap();
    let mut round_trips = Vec::new();
    for i in 0..30 {
        let row = (i * (ROWS - 1) + 14) / 29;
        let value = 20_001 + u32::try_from(i).unwrap();
        let request = DesignerRequest::EditScalar {
            expected_revision: runtime.current_revision().to_owned(),
            target: FieldTarget {
                entity: entity_id(row).to_string(),
                field: "column_1".into(),
            },
            input: ScalarEditInput::Number {
                input: value.to_string(),
            },
        };
        let (reply, edit_elapsed) = measure(|| runtime.handle(request).unwrap());
        assert!(matches!(reply, DesignerResponse::Published(_)));
        let request = DesignerRequest::QueryTableWindow {
            expected_revision: runtime.current_revision().to_owned(),
            collection: "grid".into(),
            view: TableView::default(),
            start: 0,
            count: 0,
            display: true,
        };
        let (reply, window_elapsed) = measure(|| runtime.handle(request).unwrap());
        // Sum each pair, not the independent edit/window medians.
        round_trips.push(edit_elapsed + window_elapsed);
        let DesignerResponse::TableWindow(window) = reply else {
            panic!("expected zero-row window");
        };
        assert!(window.rows.is_empty());
        assert_eq!(window.total_rows, ROWS);
        assert_eq!(window.revision, runtime.current_revision());
        let facts = runtime
            .session
            .query_fields(&[FieldRef::new(entity_id(row), "column_7")])
            .unwrap();
        assert_eq!(
            facts.value()[0].calculated_value,
            Some(FormulaCalculationOutcome::Value(
                Number::new(2.0 * f64::from(value)).unwrap()
            )),
        );
    }
    drop(runtime);

    let mut clears = Vec::new();
    for _ in 0..3 {
        // Each clear starts with populated cells; admission and request construction are untimed.
        let mut resident = None;
        open_project_working_size(&mut resident, &bytes, OCCURRENCE).unwrap();
        let mut runtime = resident.unwrap();
        let request = DesignerRequest::EditCells {
            expected_revision: runtime.current_revision().to_owned(),
            edits: (0..ROWS)
                .map(|row| CellEdit {
                    target: FieldTarget {
                        entity: entity_id(row).to_string(),
                        field: "column_0".into(),
                    },
                    input: ScalarEditInput::Clear,
                })
                .collect(),
        };
        let (reply, elapsed) = measure(|| runtime.handle(request).unwrap());
        clears.push(elapsed);
        assert!(matches!(reply, DesignerResponse::Published(_)));
        let document = runtime.session.document();
        assert_eq!(document.entities.len(), ROWS);
        assert!(document.entities.values().all(|entity| {
            !entity.fields.contains_key(&FieldId::from("column_0")) && entity.fields.len() == 9
        }));
    }

    let open = report("open request", &mut opens);
    let edit = report("edit + zero-row window", &mut round_trips);
    let clear = report("clear 10,000 cells", &mut clears);
    println!(
        "capacity_809 total runtime: {:.3}s",
        started.elapsed().as_secs_f64()
    );
    assert!(open <= Duration::from_millis(1_500), "open p50: {open:?}");
    // 20% runner-noise margin over the 50 ms target; also the #809 1.0 native edit gate.
    assert!(
        edit <= Duration::from_millis(60),
        "edit round-trip p50: {edit:?}"
    );
    assert!(clear <= Duration::from_millis(500), "clear p50: {clear:?}");
}
