//! Research-only fresh-process residency evidence for Issue #175 Arm F.
//!
//! Run the parent test explicitly in release mode. The child arms isolate a
//! complete semantic `Document` from the existing resident runtime's retained
//! address, validation, calculation, dependency, and reverse-dependency state.

use std::{collections::BTreeMap, hint::black_box, process::Command};

use tachiko_formula_engine::{CalculationOutcome, calculate_complete};
use tachiko_workspace_engine::{
    Document, DocumentId, Entity, EntityId, EntityKey, Expression, FieldDefinition, FieldId,
    FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
    patch_lifecycle::DocumentScopeId, resident_session::ResidentWorkspaceSession,
    validation_report,
};

#[test]
fn issue_175_full_oracle_is_stable_for_admission_pressure_documents() {
    let chain = oracle_chain_document(257, false);
    let (before, before_invalid) = stable_full_oracle(&chain);
    assert!(matches!(before, CalculationOutcome::Complete(_)));
    assert!(!before_invalid);

    let mut changed = chain;
    changed
        .entities
        .get_mut(&EntityId::from("issue-175-oracle-chain-00000000"))
        .unwrap()
        .fields
        .insert(
            FieldId::from("value"),
            Value::Number(Number::new(7.0).unwrap()),
        );
    let (after, after_invalid) = stable_full_oracle(&changed);
    assert!(matches!(after, CalculationOutcome::Complete(_)));
    assert!(!after_invalid);
    assert_ne!(
        before, after,
        "cold numeric mutation must change the full outcome"
    );

    let cycle = oracle_chain_document(257, true);
    let (cycle_outcome, cycle_invalid) = stable_full_oracle(&cycle);
    assert!(matches!(cycle_outcome, CalculationOutcome::Failed(_)));
    assert!(
        cycle_invalid,
        "cross-cold SCC must remain in stable diagnostics"
    );

    let mut division_by_zero = synthetic_document(257);
    division_by_zero
        .entities
        .values_mut()
        .next()
        .unwrap()
        .fields
        .insert(
            FieldId::from("computed"),
            Value::Formula(Expression::Divide {
                left: Box::new(Expression::Number(Number::new(1.0).unwrap())),
                right: Box::new(Expression::Number(Number::new(0.0).unwrap())),
            }),
        );
    let (failure, failure_invalid) = stable_full_oracle(&division_by_zero);
    assert!(matches!(failure, CalculationOutcome::Failed(_)));
    assert!(
        failure_invalid,
        "evaluation failure must remain in stable diagnostics"
    );
}

fn stable_full_oracle(document: &Document) -> (CalculationOutcome, bool) {
    let twin = document.clone();
    let outcome = calculate_complete(document);
    assert_eq!(outcome, calculate_complete(&twin));
    let observations = validation_report(document).stable_observations();
    assert_eq!(observations, validation_report(&twin).stable_observations());
    (outcome, !observations.is_empty())
}

#[test]
#[ignore = "internal fresh-process child for Issue #175 retained-state RSS evidence"]
fn issue_175_retained_state_rss_child() {
    require_release_profile();
    let entity_count = std::env::var("TACHIKO_ISSUE_175_RETAINED_RSS_ENTITIES")
        .unwrap()
        .parse::<usize>()
        .unwrap();
    match std::env::var("TACHIKO_ISSUE_175_RETAINED_RSS_ARM")
        .unwrap()
        .as_str()
    {
        "baseline" => emit_steady_rss(),
        "document" => {
            let document = synthetic_document(entity_count);
            emit_steady_rss();
            black_box(&document);
        }
        "resident_document_and_derived" => {
            let session = ResidentWorkspaceSession::new(
                DocumentScopeId::from("issue-175-rss-occurrence"),
                synthetic_document(entity_count),
            );
            emit_steady_rss();
            black_box(&session);
        }
        arm => panic!("unknown Issue #175 retained RSS arm '{arm}'"),
    }
}

#[test]
#[ignore = "run explicitly on macOS in release mode to record Issue #175 retained-state RSS evidence"]
fn issue_175_retained_state_rss_samples() {
    require_release_profile();
    assert_eq!(std::env::consts::OS, "macos", "RSS units are macOS bytes");
    let entity_count = std::env::var("TACHIKO_ISSUE_175_RETAINED_RSS_ENTITIES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(10_000);
    let repetitions = std::env::var("TACHIKO_ISSUE_175_RSS_REPETITIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(5);
    let executable = std::env::current_exe().unwrap();
    println!("arm,workload,entities,repetition,steady_rss_bytes,peak_rss_bytes");
    for repetition in 0..repetitions {
        for arm in ["baseline", "document", "resident_document_and_derived"] {
            let output = Command::new("/usr/bin/time")
                .arg("-l")
                .arg(&executable)
                .args([
                    "--exact",
                    "issue_175_retained_state_rss_child",
                    "--ignored",
                    "--nocapture",
                ])
                .env(
                    "TACHIKO_ISSUE_175_RETAINED_RSS_ENTITIES",
                    entity_count.to_string(),
                )
                .env("TACHIKO_ISSUE_175_RETAINED_RSS_ARM", arm)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "retained RSS child failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let (steady, peak) = parse_child_rss(&output.stdout, &output.stderr);
            println!("{arm},formula_per_entity,{entity_count},{repetition},{steady},{peak}");
        }
    }
}

fn emit_steady_rss() {
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .unwrap();
    assert!(output.status.success());
    let kib = String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .parse::<u64>()
        .unwrap();
    println!("ISSUE_175_STEADY_RSS_BYTES={}", kib * 1024);
}

fn parse_child_rss(stdout: &[u8], stderr: &[u8]) -> (u64, u64) {
    let steady = String::from_utf8_lossy(stdout)
        .lines()
        .find_map(|line| line.strip_prefix("ISSUE_175_STEADY_RSS_BYTES="))
        .unwrap()
        .parse::<u64>()
        .unwrap();
    let peak = String::from_utf8_lossy(stderr)
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_suffix("  maximum resident set size")
                .or_else(|| line.trim().strip_suffix(" maximum resident set size"))
        })
        .unwrap()
        .trim()
        .parse::<u64>()
        .unwrap();
    (steady, peak)
}

#[allow(clippy::assertions_on_constants)]
fn require_release_profile() {
    assert!(
        !cfg!(debug_assertions),
        "Issue #175 measurements require `cargo test --release`"
    );
}

fn synthetic_document(entity_count: usize) -> Document {
    let schema_id = SchemaId::from("issue-175-retained-schema");
    let base = FieldId::from("base");
    let multiplier = FieldId::from("multiplier");
    let computed = FieldId::from("computed");
    let label = FieldId::from("label");
    let fields = BTreeMap::from([
        (base.clone(), number_field(&base, "base")),
        (multiplier.clone(), number_field(&multiplier, "multiplier")),
        (computed.clone(), number_field(&computed, "computed")),
        (
            label.clone(),
            FieldDefinition {
                id: label.clone(),
                key: FieldKey::from("label"),
                field_type: FieldType::Text,
                required: true,
            },
        ),
    ]);
    let entities = (0..entity_count)
        .map(|index| {
            let id = EntityId::from(format!("issue-175-retained-{index:08}"));
            let fields = BTreeMap::from([
                (
                    base.clone(),
                    Value::Number(
                        Number::new(f64::from(u32::try_from(index).unwrap()) + 1.0).unwrap(),
                    ),
                ),
                (multiplier.clone(), Value::Number(Number::new(2.0).unwrap())),
                (
                    computed.clone(),
                    Value::Formula(Expression::Multiply {
                        left: Box::new(Expression::Reference(FieldRef::new(
                            id.clone(),
                            base.clone(),
                        ))),
                        right: Box::new(Expression::Reference(FieldRef::new(
                            id.clone(),
                            multiplier.clone(),
                        ))),
                    }),
                ),
                (label.clone(), Value::Text(format!("Record {index}"))),
            ]);
            (
                id.clone(),
                Entity {
                    id,
                    key: EntityKey::from(format!("retained_{index:08}")),
                    schema: schema_id.clone(),
                    fields,
                },
            )
        })
        .collect();
    Document {
        id: DocumentId::from("issue-175-retained-document"),
        title: "Issue 175 retained-state RSS".to_owned(),
        schemas: BTreeMap::from([(
            schema_id.clone(),
            Schema {
                id: schema_id,
                key: SchemaKey::from("retained_records"),
                fields,
            },
        )]),
        entities,
    }
}

fn oracle_chain_document(entity_count: usize, cycle: bool) -> Document {
    let schema_id = SchemaId::from("issue-175-oracle-chain-schema");
    let value_field = FieldId::from("value");
    let entities = (0..entity_count)
        .map(|index| {
            let id = EntityId::from(format!("issue-175-oracle-chain-{index:08}"));
            let value = if index == 0 && !cycle {
                Value::Number(Number::new(1.0).unwrap())
            } else {
                let target = if index == 0 {
                    entity_count - 1
                } else {
                    index - 1
                };
                Value::Formula(Expression::Reference(FieldRef::new(
                    format!("issue-175-oracle-chain-{target:08}"),
                    value_field.clone(),
                )))
            };
            (
                id.clone(),
                Entity {
                    id,
                    key: EntityKey::from(format!("oracle_chain_{index:08}")),
                    schema: schema_id.clone(),
                    fields: BTreeMap::from([(value_field.clone(), value)]),
                },
            )
        })
        .collect();
    Document {
        id: DocumentId::from("issue-175-oracle-chain-document"),
        title: if cycle {
            "Issue 175 cross-cold oracle cycle"
        } else {
            "Issue 175 oracle dependency chain"
        }
        .to_owned(),
        schemas: BTreeMap::from([(
            schema_id.clone(),
            Schema {
                id: schema_id,
                key: SchemaKey::from("oracle_chain"),
                fields: BTreeMap::from([(
                    value_field.clone(),
                    number_field(&value_field, "value"),
                )]),
            },
        )]),
        entities,
    }
}

fn number_field(id: &FieldId, key: &str) -> FieldDefinition {
    FieldDefinition {
        id: id.clone(),
        key: FieldKey::from(key),
        field_type: FieldType::Number,
        required: true,
    }
}
