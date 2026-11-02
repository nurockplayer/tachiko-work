//! Steward acceptance seed for #515. Missing API is intentional; see SEED-515.md.
use std::collections::BTreeMap;

use serde_json::json;

use tachiko_designer_runtime::{
    CalculationProjection, DesignerError, DesignerRequest, DesignerResponse, DesignerRuntime,
    DiagnosticProjection, FieldDependencyProjection, FieldExplanationProjection, FieldProjection,
    FieldTarget, ScalarEditInput, StoredValueProjection,
};
use tachiko_workspace_engine::{
    Document, Entity, EntityId, EntityKey, Expression, FieldConstraint, FieldDefinition, FieldId,
    FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value, WorkspaceError,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000515";
const INTERVAL: &str = "weapon.f01_interval";
const DAMAGE: &str = "weapon.f02_damage";
const DPS: &str = "weapon.f03_dps";
const BURST: &str = "weapon.f04_burst";
const ALPHA: &str = "weapon.f05_alpha";

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn reference(field: &str) -> Expression {
    Expression::Reference(FieldRef::new("weapon", field))
}

fn document(fields: Vec<(&str, &str, FieldType, Value)>) -> Document {
    let mut document = Document::empty("explanation_seed", "Synthetic explanation");
    let schema = SchemaId::from("weapons");
    document.schemas.insert(
        schema.clone(),
        Schema {
            field_order: None,
            id: schema.clone(),
            key: SchemaKey::from("weapons"),
            display_name: None,
            fields: fields
                .iter()
                .map(|(id, key, field_type, _)| {
                    let id = FieldId::from(*id);
                    (
                        id.clone(),
                        FieldDefinition {
                            id,
                            key: FieldKey::from(*key),
                            display_name: None,
                            number_format: None,
                            field_type: field_type.clone(),
                            required: true,
                            constraint: FieldConstraint::None,
                        },
                    )
                })
                .collect(),
        },
    );
    document.entities.insert(
        EntityId::from("weapon"),
        Entity {
            id: EntityId::from("weapon"),
            key: EntityKey::from("weapon"),
            schema,
            fields: fields
                .into_iter()
                .map(|(id, _, _, value)| (FieldId::from(id), value))
                .collect::<BTreeMap<_, _>>(),
        },
    );
    document
}

fn fixture() -> DesignerRuntime {
    // Expression order differs from stable-ID order. Impact keys alpha/burst
    // also differ from stable-ID order. No core analysis is used as the oracle.
    DesignerRuntime::from_document(
        document(vec![
            ("f02_damage", "damage", FieldType::Number, number(120.0)),
            (
                "f01_interval",
                "attack_interval",
                FieldType::Number,
                number(2.0),
            ),
            (
                "f03_dps",
                "dps",
                FieldType::Number,
                Value::Formula(Expression::Divide {
                    left: Box::new(reference("f02_damage")),
                    right: Box::new(reference("f01_interval")),
                }),
            ),
            (
                "f05_alpha",
                "alpha",
                FieldType::Number,
                Value::Formula(reference("f03_dps")),
            ),
            (
                "f04_burst",
                "burst",
                FieldType::Number,
                Value::Formula(reference("f03_dps")),
            ),
        ]),
        OCCURRENCE,
    )
    .expect("synthetic numeric fixture must reach the resident API")
}

fn query(runtime: &mut DesignerRuntime, revision: &str, target: &str) -> FieldProjection {
    let DesignerResponse::Fields(mut batch) = runtime
        .handle(DesignerRequest::QueryFields {
            expected_revision: revision.into(),
            fields: vec![target.into()],
        })
        .unwrap()
    else {
        panic!("expected QueryFields response")
    };
    assert_eq!(batch.revision, revision);
    assert_eq!(batch.fields.len(), 1);
    batch.fields.remove(0)
}

fn request(revision: &str, target: &str) -> DesignerRequest {
    DesignerRequest::ExplainField {
        expected_revision: revision.into(),
        target: target.into(),
    }
}

fn explain(
    runtime: &mut DesignerRuntime,
    revision: &str,
    target: &str,
) -> FieldExplanationProjection {
    let DesignerResponse::FieldExplanation(projection) =
        runtime.handle(request(revision, target)).unwrap()
    else {
        panic!("expected FieldExplanation response")
    };
    assert_eq!(projection.revision, revision);
    assert_eq!(projection.target, FieldTarget::from(target));
    // Pin reuse of the complete QueryFields field and diagnostic DTOs.
    let _: &FieldProjection = &projection.value;
    let _: &Vec<DiagnosticProjection> = &projection.diagnostics;
    let _: &Vec<FieldDependencyProjection> = &projection.direct_dependencies;
    assert_eq!(projection.value, query(runtime, revision, target));
    assert_eq!(projection.diagnostics, projection.value.diagnostics);
    projection
}

#[test]
fn input_has_value_no_rule_or_dependencies_and_one_direct_impact() {
    let mut runtime = fixture();
    let projection = explain(&mut runtime, "resident/0", DAMAGE);
    assert_eq!(
        projection.value.stored,
        Some(StoredValueProjection::Number { value: 120.0 })
    );
    assert_eq!(projection.formula_source, None);
    assert_eq!(
        projection.direct_dependencies,
        [] as [FieldDependencyProjection; 0]
    );
    assert_eq!(projection.downstream_impacts, vec![FieldTarget::from(DPS)]);
    assert_eq!(projection.diagnostics, [] as [DiagnosticProjection; 0]);
}

fn assert_dps(
    runtime: &mut DesignerRuntime,
    revision: &str,
    damage: f64,
) -> FieldExplanationProjection {
    let projection = explain(runtime, revision, DPS);
    assert_eq!(
        projection.value.calculated,
        Some(CalculationProjection::Value {
            value: damage / 2.0
        })
    );
    assert_eq!(
        projection.formula_source.as_deref(),
        Some("([weapon.damage] / [weapon.attack_interval])")
    );
    assert_eq!(
        projection.formula_source,
        projection
            .value
            .formula
            .as_ref()
            .map(|rule| rule.source.clone())
    );
    assert_eq!(
        projection.direct_dependencies,
        vec![
            FieldDependencyProjection {
                target: INTERVAL.into(),
                value: query(runtime, revision, INTERVAL)
            },
            FieldDependencyProjection {
                target: DAMAGE.into(),
                value: query(runtime, revision, DAMAGE)
            },
        ]
    );
    assert_eq!(
        projection.direct_dependencies[0].value.stored,
        Some(StoredValueProjection::Number { value: 2.0 })
    );
    assert_eq!(
        projection.direct_dependencies[1].value.stored,
        Some(StoredValueProjection::Number { value: damage })
    );
    assert_eq!(
        projection.downstream_impacts,
        vec![BURST.into(), ALPHA.into()]
    );
    assert_eq!(projection.diagnostics, [] as [DiagnosticProjection; 0]);
    projection
}

#[test]
fn calculated_field_has_exact_rule_current_direct_values_and_stable_id_order() {
    assert_dps(&mut fixture(), "resident/0", 120.0);
}

#[test]
fn chain_excludes_transitive_impacts_and_upstream_dependencies() {
    let mut runtime = fixture();
    // damage -> dps -> burst: damage must exclude burst and alpha.
    assert_eq!(
        explain(&mut runtime, "resident/0", DAMAGE).downstream_impacts,
        vec![DPS.into()]
    );
    let leaf = explain(&mut runtime, "resident/0", BURST);
    assert_eq!(
        leaf.direct_dependencies,
        vec![FieldDependencyProjection {
            target: DPS.into(),
            value: query(&mut runtime, "resident/0", DPS),
        }]
    );
    assert_eq!(
        leaf.value.calculated,
        Some(CalculationProjection::Value { value: 60.0 })
    );
    assert_eq!(leaf.downstream_impacts, [] as [FieldTarget; 0]);
}

fn edit(runtime: &mut DesignerRuntime) {
    runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/0".into(),
            target: DAMAGE.into(),
            input: ScalarEditInput::Number {
                input: "180".into(),
            },
        })
        .unwrap();
    assert_eq!(runtime.observe_occurrence().revision, "resident/1");
}

#[test]
fn edit_refreshes_values_and_undo_restores_the_explanation_at_its_new_revision() {
    let mut runtime = fixture();
    let mut original = assert_dps(&mut runtime, "resident/0", 120.0);
    edit(&mut runtime);
    assert_dps(&mut runtime, "resident/1", 180.0);
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/1".into(),
        })
        .unwrap();
    original.revision = "resident/2".into();
    assert_eq!(assert_dps(&mut runtime, "resident/2", 120.0), original);
}

#[test]
fn stale_unknown_and_foreign_targets_refuse_without_mutating_state() {
    let mut runtime = fixture();
    edit(&mut runtime);
    let occurrence = runtime.observe_occurrence();
    let bytes = runtime.export_project("resident/1").unwrap().bytes;
    assert!(matches!(runtime.handle(request("resident/0", DPS)),
        Err(DesignerError::StaleQuery { requested, current })
        if requested == "resident/0" && current == "resident/1"));
    for target in ["weapon.unknown", "foreign.f03_dps"] {
        let error = runtime.handle(request("resident/1", target)).unwrap_err();
        assert!(
            matches!(
                error,
                DesignerError::Workspace(
                    WorkspaceError::MissingField { .. } | WorkspaceError::MissingEntityId { .. }
                )
            ),
            "expected existing typed target refusal: {error:?}"
        );
        assert_eq!(runtime.observe_occurrence(), occurrence);
        assert_eq!(runtime.export_project("resident/1").unwrap().bytes, bytes);
    }
    assert_eq!(runtime.observe_occurrence(), occurrence);
    assert_eq!(runtime.export_project("resident/1").unwrap().bytes, bytes);
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/1".into(),
        })
        .unwrap();
    assert_dps(&mut runtime, "resident/2", 120.0);
}

#[test]
fn repeated_reads_do_not_create_undo_availability_on_a_fresh_occurrence() {
    let mut runtime = fixture();
    let occurrence = runtime.observe_occurrence();
    let bytes = runtime.export_project("resident/0").unwrap().bytes;
    for _ in 0..3 {
        assert_dps(&mut runtime, "resident/0", 120.0);
    }
    assert!(matches!(
        runtime.handle(DesignerRequest::Undo {
            expected_revision: "resident/0".into(),
        }),
        Err(DesignerError::InvalidTrackerOperation { .. })
    ));
    assert_eq!(runtime.observe_occurrence(), occurrence);
    assert_eq!(runtime.export_project("resident/0").unwrap().bytes, bytes);
}

#[test]
fn repeated_reads_preserve_occurrence_export_and_both_history_directions() {
    let mut runtime = fixture();
    let original_bytes = runtime.export_project("resident/0").unwrap().bytes;
    edit(&mut runtime);
    let occurrence = runtime.observe_occurrence();
    let edited_bytes = runtime.export_project("resident/1").unwrap().bytes;
    let expected = assert_dps(&mut runtime, "resident/1", 180.0);
    for _ in 0..3 {
        assert_eq!(explain(&mut runtime, "resident/1", DPS), expected);
        assert_eq!(runtime.observe_occurrence(), occurrence);
        assert_eq!(
            runtime.export_project("resident/1").unwrap().bytes,
            edited_bytes
        );
    }
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/1".into(),
        })
        .unwrap();
    let undone = runtime.observe_occurrence();
    for _ in 0..3 {
        assert_dps(&mut runtime, "resident/2", 120.0);
    }
    assert_eq!(runtime.observe_occurrence(), undone);
    assert_eq!(
        runtime.export_project("resident/2").unwrap().bytes,
        original_bytes
    );
    runtime
        .handle(DesignerRequest::Redo {
            expected_revision: "resident/2".into(),
        })
        .unwrap();
    assert_dps(&mut runtime, "resident/3", 180.0);
    assert_eq!(
        runtime.export_project("resident/3").unwrap().bytes,
        edited_bytes
    );
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/3".into(),
        })
        .unwrap();
    assert_eq!(
        runtime.export_project("resident/4").unwrap().bytes,
        original_bytes
    );
}

#[test]
fn oversized_explanation_uses_the_existing_query_budget_and_refuses_atomically() {
    // The existing Tracker profile admits this tiny synthetic table (256 KiB).
    // Each control character occupies six JSON bytes, so its field reply exceeds
    // the ordinary QueryFields response budget without increasing any capacity.
    let mut budget_document = document(vec![
        (
            "task",
            "task",
            FieldType::Text,
            Value::Text("\u{1}".repeat(12_000)),
        ),
        ("estimate", "estimate", FieldType::Number, number(1.0)),
        ("done", "done", FieldType::Boolean, Value::Boolean(false)),
    ]);
    let mut schema = budget_document
        .schemas
        .remove(&SchemaId::from("weapons"))
        .unwrap();
    schema.id = SchemaId::from("tracker");
    schema.key = SchemaKey::from("tracker");
    budget_document.schemas.insert(schema.id.clone(), schema);
    budget_document
        .entities
        .get_mut(&EntityId::from("weapon"))
        .unwrap()
        .schema = SchemaId::from("tracker");
    let mut runtime = DesignerRuntime::from_document(budget_document, OCCURRENCE)
        .expect("existing Tracker profile must admit the budget fixture");
    let occurrence = runtime.observe_occurrence();
    let before = runtime.export_project("resident/0").unwrap().bytes;
    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: "tracker".into(),
        })
        .unwrap()
    else {
        panic!("capacity fixture must be readable through its supported table path")
    };
    assert_eq!(table.tracker_profile, Some(true));
    assert_eq!(
        table.rows[0]
            .fields
            .iter()
            .find(|field| field.target.field == "task")
            .unwrap()
            .stored,
        Some(StoredValueProjection::Text {
            value: "\u{1}".repeat(12_000)
        })
    );
    let query_error = runtime
        .handle(DesignerRequest::QueryFields {
            expected_revision: "resident/0".into(),
            fields: vec!["weapon.task".into()],
        })
        .unwrap_err();
    let DesignerError::ProjectionTooLarge { maximum, .. } = query_error else {
        panic!("expected the existing projection-budget refusal: {query_error:?}")
    };
    let error = runtime
        .handle(request("resident/0", "weapon.task"))
        .unwrap_err();
    assert!(
        matches!(error, DesignerError::ProjectionTooLarge { actual, maximum: limit }
        if actual > maximum && limit == maximum)
    );
    assert_eq!(
        error.failure_projection("resident/0").code,
        "query_too_large"
    );
    assert_eq!(runtime.observe_occurrence(), occurrence);
    assert_eq!(runtime.export_project("resident/0").unwrap().bytes, before);
}

#[test]
fn worker_parity_capture_uses_the_same_native_fixture_and_requests() {
    let mut runtime = fixture();
    let bytes = runtime.export_project("resident/0").unwrap().bytes;
    let requests = [request("resident/0", DAMAGE), request("resident/0", DPS)];
    // Literal semantic assertions precede the native-to-Worker comparison.
    let input = explain(&mut runtime, "resident/0", DAMAGE);
    assert_eq!(input.downstream_impacts, vec![DPS.into()]);
    assert_eq!(input.formula_source, None);
    assert_eq!(
        input.direct_dependencies,
        [] as [FieldDependencyProjection; 0]
    );
    let calculated = assert_dps(&mut runtime, "resident/0", 120.0);
    if let Some(path) = std::env::var_os("TACHIKO_FIELD_EXPLANATION_CAPTURE") {
        let capture = json!({
            "bytes": bytes, "requests": requests,
            "responses": [DesignerResponse::FieldExplanation(input), DesignerResponse::FieldExplanation(calculated)],
        });
        std::fs::write(path, serde_json::to_vec(&capture).unwrap()).unwrap();
    }
}
