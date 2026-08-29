use std::collections::{BTreeMap, BTreeSet};

use tachiko_formula_engine::{CalculationOutcome, calculate_complete, extract_dependencies};
use tachiko_workspace_engine::{
    CalculationFailure, Document, Entity, EntityId, EntityKey, Expression, FieldDefinition,
    FieldId, FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
    calculate_fields,
    formula_operations::FormulaCalculationOutcome,
    patch_lifecycle::{DocumentScopeId, SemanticPublicationAuthority, TrustedInstant},
    resident_session::{
        ResidentProjectionInvalidation, ResidentRuntimeMeasurements, ResidentWorkspaceSession,
        TrustedPublicationTimeSource,
    },
    validation_report,
};

const PRIMARY: &str = "primary";
const ISOLATED: &str = "isolated";
const SOURCE_A: &str = "source_a";
const SOURCE_B: &str = "source_b";
const MID: &str = "mid";
const MASKED: &str = "masked";
const TAIL: &str = "tail";
const TARGET: &str = "target";
const TARGET_USER: &str = "target_user";
const TARGET_TAIL: &str = "target_tail";
const ISOLATED_INPUT: &str = "isolated_input";
const ISOLATED_FORMULA: &str = "isolated_formula";

struct FixedTime;

impl TrustedPublicationTimeSource for FixedTime {
    fn now(&mut self) -> TrustedInstant {
        TrustedInstant::new(1)
    }
}

#[test]
fn retained_state_matches_fresh_full_oracles_across_generated_revisions() {
    run_scripted_sequence();
    for seed in [0x095_u64, 0x5eed_u64, 0x00c0_ffee_u64] {
        run_generated_sequence(seed);
    }
}

fn run_scripted_sequence() {
    let scope = DocumentScopeId::from("retained-oracle-occurrence");
    let mut session = ResidentWorkspaceSession::new(scope.clone(), oracle_document());
    assert_revision_oracles(&session, "initial");
    let mutations = scripted_mutations();
    let mut checks = SequenceChecks::default();
    run_sequence(&mut session, &scope, &mutations, &mut checks);
    assert_scripted_measurements(session.runtime_measurements(), mutations.len());
}

fn run_generated_sequence(seed: u64) {
    let scope = DocumentScopeId::from(format!("generated-{seed:016x}"));
    let mut session = ResidentWorkspaceSession::new(scope.clone(), oracle_document());
    let mutations = generated_numeric_mutations(seed, 12);
    let mut checks = SequenceChecks::default();
    run_sequence(&mut session, &scope, &mutations, &mut checks);
    let measurements = session.runtime_measurements();
    assert_eq!(measurements.full_calculation_rebuilds, 1, "seed {seed}");
    assert_eq!(measurements.calculation_fallbacks, 0, "seed {seed}");
    assert_eq!(
        measurements.incremental_calculation_updates,
        mutations.len(),
        "seed {seed}"
    );
    assert!(
        measurements.calculation_nodes_reused > measurements.calculation_nodes_recomputed,
        "seed {seed} should reuse more nodes than it recomputes: {measurements:?}"
    );
}

fn run_sequence(
    session: &mut ResidentWorkspaceSession,
    scope: &DocumentScopeId,
    mutations: &[MutationCase],
    checks: &mut SequenceChecks,
) {
    for (revision, mutation) in mutations.iter().enumerate() {
        let before = session.export_snapshot().into_document();
        let mut candidate = before.clone();
        mutation.apply(&mut candidate);

        let warm_invalidation = publish(session, candidate.clone());
        let mut cold_transition = ResidentWorkspaceSession::new(scope.clone(), before);
        let cold_invalidation = publish(&mut cold_transition, candidate);
        assert_invalidation_subjects_equal(
            &warm_invalidation,
            &cold_invalidation,
            revision + 1,
            &mutation.name,
        );
        assert_revision_oracles(
            session,
            &format!("revision {} ({})", revision + 1, mutation.name),
        );
        apply_specific_check(session, &warm_invalidation, mutation, checks);
    }
}

fn apply_specific_check(
    session: &ResidentWorkspaceSession,
    invalidation: &ResidentProjectionInvalidation,
    mutation: &MutationCase,
    checks: &mut SequenceChecks,
) {
    match mutation.check {
        SpecificCheck::None => {}
        SpecificCheck::NormalizedZero => {
            assert_eq!(
                stored_number_bits(session, PRIMARY, SOURCE_A),
                0.0_f64.to_bits()
            );
        }
        SpecificCheck::Subnormal => {
            assert_eq!(stored_number_bits(session, PRIMARY, SOURCE_B), 1);
        }
        SpecificCheck::RebindOldAndNewClosure => {
            assert_eq!(invalidation.entities, []);
            assert_eq!(invalidation.fields, [field(PRIMARY, MID)]);
            assert_eq!(
                invalidation.affected_calculations,
                [field(PRIMARY, MASKED), field(PRIMARY, TAIL)]
            );
        }
        SpecificCheck::RemovedDependency => {
            assert!(
                invalidation.affected_calculations.is_empty(),
                "{} retained a removed dependency: {invalidation:?}",
                mutation.name
            );
        }
        SpecificCheck::OutputEqualDownstream => {
            assert_eq!(invalidation.entities, []);
            assert_eq!(invalidation.fields, [field(PRIMARY, SOURCE_A)]);
            assert_eq!(
                invalidation.affected_calculations,
                [
                    field(PRIMARY, MASKED),
                    field(PRIMARY, MID),
                    field(PRIMARY, TAIL),
                ]
            );
        }
        SpecificCheck::MissingTarget => {
            let target = field(PRIMARY, TARGET);
            assert_missing_projection_matches_fresh(session, &target, &mutation.name);
            assert_failed_dependency_chain(session, &mutation.name);
            assert_independent_formula_is_unavailable(session, &mutation.name);
        }
        SpecificCheck::FailedDependency => {
            assert_failed_dependency_chain(session, &mutation.name);
            assert_independent_formula_is_unavailable(session, &mutation.name);
        }
        SpecificCheck::Cycle => {
            assert_cycle_and_failed_dependency(session, &mutation.name);
            assert_independent_formula_is_unavailable(session, &mutation.name);
        }
        SpecificCheck::InvalidExpression => {
            assert_formula_failure(session, MID, &mutation.name, |failure| {
                matches!(failure, CalculationFailure::InvalidExpression { .. })
            });
            assert_independent_formula_is_unavailable(session, &mutation.name);
        }
        SpecificCheck::NonFinite => {
            assert_formula_failure(session, MID, &mutation.name, |failure| {
                matches!(failure, CalculationFailure::NonFiniteResult)
            });
            assert_independent_formula_is_unavailable(session, &mutation.name);
        }
        SpecificCheck::Recovered => {
            assert_independent_formula_is_value(session, &mutation.name);
        }
        SpecificCheck::InvalidateAddressIndex => {
            let measurements = session.runtime_measurements();
            assert_eq!(measurements.calculation_fallbacks, 1, "{}", mutation.name);
            checks.fallback_count_after_recovery = Some(measurements.calculation_fallbacks);
        }
        SpecificCheck::RecoverAddressIndex => {
            let measurements = session.runtime_measurements();
            assert_eq!(measurements.calculation_fallbacks, 2, "{}", mutation.name);
            checks.fallback_count_after_recovery = Some(measurements.calculation_fallbacks);
            checks.incremental_count_after_recovery =
                Some(measurements.incremental_calculation_updates);
        }
        SpecificCheck::PostFallbackIncremental => {
            let measurements = session.runtime_measurements();
            assert_eq!(
                Some(measurements.calculation_fallbacks),
                checks.fallback_count_after_recovery,
                "a valid follow-up edit must not remain on the fallback path"
            );
            assert_eq!(
                Some(measurements.incremental_calculation_updates),
                checks
                    .incremental_count_after_recovery
                    .map(|count| count + 1),
                "a valid follow-up edit must resume incremental calculation"
            );
        }
    }
}

fn assert_scripted_measurements(measurements: ResidentRuntimeMeasurements, revisions: usize) {
    assert_eq!(measurements.derived_state_rebuilds, revisions + 1);
    assert_eq!(measurements.retained_before_state_reuses, revisions);
    assert_eq!(measurements.full_calculation_rebuilds, 3);
    assert_eq!(measurements.calculation_fallbacks, 2);
    assert_eq!(
        measurements.incremental_calculation_updates,
        revisions - measurements.calculation_fallbacks
    );
    assert!(
        measurements.calculation_nodes_reused > measurements.calculation_nodes_recomputed,
        "generated local sequence should reuse materially more nodes than it recomputes: {measurements:?}"
    );
    assert!(measurements.reverse_edges_traversed > 0);
    assert!(measurements.address_index_reuses > 0);
}

#[derive(Default)]
struct SequenceChecks {
    fallback_count_after_recovery: Option<usize>,
    incremental_count_after_recovery: Option<usize>,
}

#[derive(Clone, Copy)]
enum SpecificCheck {
    None,
    NormalizedZero,
    Subnormal,
    RebindOldAndNewClosure,
    RemovedDependency,
    OutputEqualDownstream,
    MissingTarget,
    FailedDependency,
    Cycle,
    InvalidExpression,
    NonFinite,
    Recovered,
    InvalidateAddressIndex,
    RecoverAddressIndex,
    PostFallbackIncremental,
}

enum MutationOperation {
    Static(fn(&mut Document)),
    SetNumber {
        entity: &'static str,
        field_id: &'static str,
        value: Number,
    },
}

struct MutationCase {
    name: String,
    check: SpecificCheck,
    operation: MutationOperation,
}

impl MutationCase {
    fn static_case(name: &'static str, check: SpecificCheck, mutate: fn(&mut Document)) -> Self {
        Self {
            name: name.to_owned(),
            check,
            operation: MutationOperation::Static(mutate),
        }
    }

    fn generated_number(
        name: String,
        entity: &'static str,
        field_id: &'static str,
        value: Number,
    ) -> Self {
        Self {
            name,
            check: SpecificCheck::None,
            operation: MutationOperation::SetNumber {
                entity,
                field_id,
                value,
            },
        }
    }

    fn apply(&self, document: &mut Document) {
        match self.operation {
            MutationOperation::Static(mutate) => mutate(document),
            MutationOperation::SetNumber {
                entity,
                field_id,
                value,
            } => set_value(document, entity, field_id, Value::Number(value)),
        }
    }
}

fn scripted_mutations() -> Vec<MutationCase> {
    dependency_mutations()
        .into_iter()
        .chain(target_and_presentation_mutations())
        .chain(failure_class_mutations())
        .chain(fallback_mutations())
        .collect()
}

fn dependency_mutations() -> Vec<MutationCase> {
    use SpecificCheck::{
        None, NormalizedZero, OutputEqualDownstream, RebindOldAndNewClosure, RemovedDependency,
        Subnormal,
    };
    vec![
        MutationCase::static_case("normalized-negative-zero", NormalizedZero, |document| {
            set_value(document, PRIMARY, SOURCE_A, number(-0.0));
        }),
        MutationCase::static_case("smallest-positive-subnormal", Subnormal, |document| {
            set_value(document, PRIMARY, SOURCE_B, number(f64::from_bits(1)));
        }),
        MutationCase::static_case(
            "output-equal-rebind-removes-old-dependency",
            RebindOldAndNewClosure,
            |document| set_formula(document, PRIMARY, MID, reference(PRIMARY, SOURCE_B)),
        ),
        MutationCase::static_case("mutate-detached-old-input", RemovedDependency, |document| {
            set_value(document, PRIMARY, SOURCE_A, number(7.0));
        }),
        MutationCase::static_case("restore-formula-dependency", None, |document| {
            set_formula(document, PRIMARY, MID, primary_addition());
        }),
        MutationCase::static_case(
            "output-equal-downstream",
            OutputEqualDownstream,
            |document| {
                set_value(document, PRIMARY, SOURCE_A, number(9.0));
            },
        ),
        MutationCase::static_case("rebind-reference-target", None, |document| {
            set_formula(
                document,
                PRIMARY,
                TARGET_USER,
                add(reference(PRIMARY, SOURCE_B), literal(1.0)),
            );
        }),
        MutationCase::static_case(
            "mutate-former-reference-target",
            RemovedDependency,
            |document| set_value(document, PRIMARY, TARGET, number(8.0)),
        ),
        MutationCase::static_case("restore-reference-target", None, |document| {
            set_formula(document, PRIMARY, TARGET_USER, target_addition());
        }),
    ]
}

fn target_and_presentation_mutations() -> Vec<MutationCase> {
    use SpecificCheck::{FailedDependency, MissingTarget, None, Recovered};
    vec![
        MutationCase::static_case("delete-target-field", MissingTarget, |document| {
            entity_mut(document, PRIMARY).fields.remove(TARGET);
        }),
        MutationCase::static_case("restore-target-field", Recovered, |document| {
            set_value(document, PRIMARY, TARGET, number(8.0));
        }),
        MutationCase::static_case("schema-number-to-text", FailedDependency, |document| {
            field_definition_mut(document, TARGET).field_type = FieldType::Text;
            set_value(document, PRIMARY, TARGET, Value::Text("eight".to_owned()));
        }),
        MutationCase::static_case("schema-text-to-number", Recovered, |document| {
            field_definition_mut(document, TARGET).field_type = FieldType::Number;
            set_value(document, PRIMARY, TARGET, number(8.0));
        }),
        MutationCase::static_case("rename-entity-key", None, |document| {
            entity_mut(document, PRIMARY).key = EntityKey::from("hero");
        }),
        MutationCase::static_case("rename-field-key", None, |document| {
            field_definition_mut(document, SOURCE_A).key = FieldKey::from("power");
        }),
    ]
}

fn failure_class_mutations() -> Vec<MutationCase> {
    use SpecificCheck::{Cycle, FailedDependency, InvalidExpression, NonFinite, None, Recovered};
    vec![
        MutationCase::static_case("introduce-cycle", Cycle, |document| {
            set_formula(document, PRIMARY, MID, reference(PRIMARY, MASKED));
        }),
        MutationCase::static_case("break-cycle", Recovered, |document| {
            set_formula(document, PRIMARY, MID, primary_addition());
        }),
        MutationCase::static_case("output-equal-formula-replacement", None, |document| {
            set_formula(
                document,
                PRIMARY,
                MID,
                maximum(reference(PRIMARY, SOURCE_A), reference(PRIMARY, SOURCE_B)),
            );
        }),
        MutationCase::static_case("restore-addition-formula", None, |document| {
            set_formula(document, PRIMARY, MID, primary_addition());
        }),
        MutationCase::static_case(
            "introduce-invalid-expression",
            InvalidExpression,
            |document| {
                set_formula(document, PRIMARY, MID, overdeep_expression());
            },
        ),
        MutationCase::static_case("recover-invalid-expression", Recovered, |document| {
            set_formula(document, PRIMARY, MID, primary_addition());
        }),
        MutationCase::static_case("introduce-division-failure", FailedDependency, |document| {
            set_formula(
                document,
                PRIMARY,
                TARGET_USER,
                divide(reference(PRIMARY, TARGET), literal(0.0)),
            );
        }),
        MutationCase::static_case("recover-division-failure", Recovered, |document| {
            set_formula(document, PRIMARY, TARGET_USER, target_addition());
        }),
        MutationCase::static_case("introduce-non-finite-result", NonFinite, |document| {
            set_value(document, PRIMARY, SOURCE_A, number(f64::MAX));
            set_value(document, PRIMARY, SOURCE_B, number(f64::MAX));
        }),
        MutationCase::static_case("recover-non-finite-result", Recovered, |document| {
            set_value(document, PRIMARY, SOURCE_A, number(9.0));
            set_value(document, PRIMARY, SOURCE_B, number(f64::from_bits(1)));
        }),
    ]
}

fn fallback_mutations() -> Vec<MutationCase> {
    vec![
        MutationCase::static_case(
            "invalidate-address-index",
            SpecificCheck::InvalidateAddressIndex,
            |document| entity_mut(document, ISOLATED).key = EntityKey::from("hero"),
        ),
        MutationCase::static_case(
            "recover-address-index",
            SpecificCheck::RecoverAddressIndex,
            |document| entity_mut(document, ISOLATED).key = EntityKey::from("side"),
        ),
        MutationCase::static_case(
            "post-fallback-incremental-recovery",
            SpecificCheck::PostFallbackIncremental,
            |document| set_value(document, ISOLATED, ISOLATED_INPUT, number(11.0)),
        ),
    ]
}

fn generated_numeric_mutations(seed: u64, revisions: usize) -> Vec<MutationCase> {
    let targets = [
        (PRIMARY, SOURCE_A),
        (PRIMARY, TARGET),
        (ISOLATED, ISOLATED_INPUT),
    ];
    let mut state = seed;
    (0..revisions)
        .map(|revision| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let (entity, field_id) = targets[revision % targets.len()];
            let raw_value = u32::try_from(((state >> 32) % 10_000) + 1).unwrap();
            let value = Number::new(f64::from(raw_value) / 16.0).unwrap();
            MutationCase::generated_number(
                format!("seed-{seed:016x}-revision-{revision:02}-{entity}-{field_id}"),
                entity,
                field_id,
                value,
            )
        })
        .collect()
}

fn assert_revision_oracles(session: &ResidentWorkspaceSession, context: &str) {
    let snapshot = session.export_snapshot();
    let document = snapshot.document();
    let fields = all_fields(document);
    let warm_fields = session
        .query_fields(&fields)
        .unwrap_or_else(|error| panic!("warm projections failed at {context}: {error:?}"))
        .into_value();
    let cold = ResidentWorkspaceSession::new(
        DocumentScopeId::from("fresh-retained-oracle"),
        document.clone(),
    );
    let cold_fields = cold
        .query_fields(&fields)
        .unwrap_or_else(|error| panic!("cold projections failed at {context}: {error:?}"))
        .into_value();
    assert_eq!(warm_fields, cold_fields, "field projections at {context}");

    let expected_report = validation_report(document);
    let warm_report = session.validation_report().into_value();
    assert_eq!(
        warm_report.stable_observations(),
        expected_report.stable_observations(),
        "validation observations at {context}"
    );
    assert_eq!(
        cold.validation_report().into_value(),
        expected_report,
        "fresh validation report at {context}"
    );

    let full_outcome = calculate_complete(document);
    let projected_dependencies = warm_fields
        .iter()
        .filter_map(|projection| {
            projection
                .formula_definition
                .as_ref()
                .map(|expression| (projection.field.clone(), extract_dependencies(expression)))
        })
        .collect::<BTreeMap<_, _>>();
    let full_dependencies = match &full_outcome {
        CalculationOutcome::Complete(calculation) => calculation.dependencies(),
        CalculationOutcome::Failed(failures) => failures.dependencies(),
    };
    assert_eq!(
        &projected_dependencies, full_dependencies,
        "formula dependency sets at {context}"
    );

    for projection in &warm_fields {
        let Some(_) = projection.formula_definition else {
            continue;
        };
        let expected = match &full_outcome {
            CalculationOutcome::Complete(calculation) => FormulaCalculationOutcome::Value(
                calculation.value(&projection.field).unwrap_or_else(|| {
                    panic!("missing full value for {} at {context}", projection.field)
                }),
            ),
            CalculationOutcome::Failed(failures) => failures
                .failures()
                .get(&projection.field)
                .cloned()
                .map_or(FormulaCalculationOutcome::Unavailable, |failure| {
                    FormulaCalculationOutcome::Failure(failure)
                }),
        };
        assert_eq!(
            projection.calculated_value,
            Some(expected),
            "formula outcome for {} at {context}",
            projection.field
        );
    }

    match (session.calculate_fields(), calculate_fields(document)) {
        (Ok(warm), Ok(expected)) => {
            assert_eq!(
                warm.into_value(),
                expected,
                "calculated projection at {context}"
            );
        }
        (Err(warm), Err(expected)) => {
            assert_eq!(
                format!("{warm:?}"),
                format!("{expected:?}"),
                "calculation failure projection at {context}"
            );
        }
        (warm, expected) => panic!(
            "warm/full calculation completion mismatch at {context}: warm={warm:?}, expected={expected:?}"
        ),
    }
}

fn assert_invalidation_subjects_equal(
    warm: &ResidentProjectionInvalidation,
    cold: &ResidentProjectionInvalidation,
    revision: usize,
    mutation: &str,
) {
    assert_eq!(
        warm.document_scope, cold.document_scope,
        "scope at revision {revision} ({mutation})"
    );
    assert_eq!(
        warm.entities, cold.entities,
        "entities at revision {revision} ({mutation})"
    );
    assert_eq!(
        warm.fields, cold.fields,
        "fields at revision {revision} ({mutation})"
    );
    assert_eq!(
        warm.affected_calculations, cold.affected_calculations,
        "calculations at revision {revision} ({mutation})"
    );
}

fn publish(
    session: &mut ResidentWorkspaceSession,
    candidate: Document,
) -> ResidentProjectionInvalidation {
    let before = session.export_snapshot();
    let mut time = FixedTime;
    let mut authority = session.publication_authority(&mut time);
    let resulting_revision = authority
        .publish_if_current(
            before.document_scope(),
            before.revision(),
            candidate,
            |_| Some(()),
        )
        .unwrap()
        .2;
    authority
        .projection_invalidation_for(
            before.document_scope(),
            before.revision(),
            &resulting_revision,
        )
        .expect("successful publication must expose its invalidation")
        .clone()
}

fn assert_missing_projection_matches_fresh(
    session: &ResidentWorkspaceSession,
    missing: &FieldRef,
    context: &str,
) {
    let document = session.export_snapshot().into_document();
    let cold = ResidentWorkspaceSession::new(DocumentScopeId::from("missing-cold"), document);
    let warm_error = session
        .query_fields(std::slice::from_ref(missing))
        .expect_err("removed target must not project");
    let cold_error = cold
        .query_fields(std::slice::from_ref(missing))
        .expect_err("fresh session must agree that removed target is absent");
    assert_eq!(
        format!("{warm_error:?}"),
        format!("{cold_error:?}"),
        "missing field projection at {context}"
    );
}

fn assert_formula_failure(
    session: &ResidentWorkspaceSession,
    field_id: &str,
    context: &str,
    expected: impl FnOnce(&CalculationFailure) -> bool,
) {
    let projection = session
        .query_fields(&[field(PRIMARY, field_id)])
        .unwrap()
        .into_value()
        .pop()
        .unwrap();
    let Some(FormulaCalculationOutcome::Failure(failure)) = projection.calculated_value else {
        panic!("expected formula failure at {context}: {projection:?}");
    };
    assert!(
        expected(&failure),
        "unexpected failure at {context}: {failure:?}"
    );
}

fn assert_failed_dependency_chain(session: &ResidentWorkspaceSession, context: &str) {
    let projections = session
        .query_fields(&[field(PRIMARY, TARGET_USER), field(PRIMARY, TARGET_TAIL)])
        .unwrap()
        .into_value();
    let primary = projections
        .iter()
        .find(|projection| projection.field == field(PRIMARY, TARGET_USER))
        .unwrap();
    let dependent = projections
        .iter()
        .find(|projection| projection.field == field(PRIMARY, TARGET_TAIL))
        .unwrap();
    assert!(
        matches!(
            primary.calculated_value,
            Some(FormulaCalculationOutcome::Failure(_))
        ),
        "primary failure at {context}: {projections:?}"
    );
    assert!(
        matches!(
            dependent.calculated_value,
            Some(FormulaCalculationOutcome::Failure(
                tachiko_workspace_engine::CalculationFailure::FailedDependencies { .. }
            ))
        ),
        "failed dependency at {context}: {projections:?}"
    );
}

fn assert_cycle_and_failed_dependency(session: &ResidentWorkspaceSession, context: &str) {
    let projections = session
        .query_fields(&[
            field(PRIMARY, MID),
            field(PRIMARY, MASKED),
            field(PRIMARY, TAIL),
        ])
        .unwrap()
        .into_value();
    let expected_members = BTreeSet::from([field(PRIMARY, MID), field(PRIMARY, MASKED)]);
    for projection in &projections[..2] {
        assert_eq!(
            projection.calculated_value,
            Some(FormulaCalculationOutcome::Failure(
                tachiko_workspace_engine::CalculationFailure::Cycle {
                    members: expected_members.clone(),
                }
            )),
            "complete SCC membership at {context}"
        );
    }
    assert!(
        matches!(
            projections[2].calculated_value,
            Some(FormulaCalculationOutcome::Failure(
                tachiko_workspace_engine::CalculationFailure::FailedDependencies { .. }
            ))
        ),
        "cycle downstream must be a failed dependency at {context}"
    );
}

fn assert_independent_formula_is_unavailable(session: &ResidentWorkspaceSession, context: &str) {
    let projection = session
        .query_fields(&[field(ISOLATED, ISOLATED_FORMULA)])
        .unwrap()
        .into_value()
        .pop()
        .unwrap();
    assert_eq!(
        projection.calculated_value,
        Some(FormulaCalculationOutcome::Unavailable),
        "atomic failed outcome at {context}"
    );
}

fn assert_independent_formula_is_value(session: &ResidentWorkspaceSession, context: &str) {
    let projection = session
        .query_fields(&[field(ISOLATED, ISOLATED_FORMULA)])
        .unwrap()
        .into_value()
        .pop()
        .unwrap();
    assert_eq!(
        projection.calculated_value,
        Some(FormulaCalculationOutcome::Value(Number::new(11.0).unwrap())),
        "complete outcome at {context}"
    );
}

fn stored_number_bits(session: &ResidentWorkspaceSession, entity: &str, field_id: &str) -> u64 {
    let projection = session
        .query_fields(&[field(entity, field_id)])
        .unwrap()
        .into_value()
        .pop()
        .unwrap();
    match projection.stored_value {
        Some(Value::Number(number)) => number.to_bits(),
        other => panic!("expected a stored number, got {other:?}"),
    }
}

fn all_fields(document: &Document) -> Vec<FieldRef> {
    document
        .entities
        .iter()
        .flat_map(|(entity, value)| {
            value
                .fields
                .keys()
                .cloned()
                .map(|field_id| FieldRef::new(entity.clone(), field_id))
        })
        .collect()
}

fn oracle_document() -> Document {
    let field_ids = [
        SOURCE_A,
        SOURCE_B,
        MID,
        MASKED,
        TAIL,
        TARGET,
        TARGET_USER,
        TARGET_TAIL,
        ISOLATED_INPUT,
        ISOLATED_FORMULA,
    ];
    let fields = field_ids
        .into_iter()
        .map(|id| {
            (
                FieldId::from(id),
                FieldDefinition {
                    id: FieldId::from(id),
                    key: FieldKey::from(id),
                    field_type: FieldType::Number,
                    required: false,
                },
            )
        })
        .collect();

    let mut primary_fields = BTreeMap::new();
    primary_fields.insert(FieldId::from(SOURCE_A), number(2.0));
    primary_fields.insert(FieldId::from(SOURCE_B), number(3.0));
    primary_fields.insert(
        FieldId::from(MID),
        Value::Formula(add(
            reference(PRIMARY, SOURCE_A),
            reference(PRIMARY, SOURCE_B),
        )),
    );
    primary_fields.insert(
        FieldId::from(MASKED),
        Value::Formula(multiply(reference(PRIMARY, MID), literal(0.0))),
    );
    primary_fields.insert(
        FieldId::from(TAIL),
        Value::Formula(add(reference(PRIMARY, MASKED), literal(1.0))),
    );
    primary_fields.insert(FieldId::from(TARGET), number(4.0));
    primary_fields.insert(
        FieldId::from(TARGET_USER),
        Value::Formula(add(reference(PRIMARY, TARGET), literal(1.0))),
    );
    primary_fields.insert(
        FieldId::from(TARGET_TAIL),
        Value::Formula(add(reference(PRIMARY, TARGET_USER), literal(1.0))),
    );

    let isolated_fields = BTreeMap::from([
        (FieldId::from(ISOLATED_INPUT), number(10.0)),
        (
            FieldId::from(ISOLATED_FORMULA),
            Value::Formula(add(reference(ISOLATED, ISOLATED_INPUT), literal(1.0))),
        ),
    ]);

    Document {
        id: "retained-oracle".into(),
        title: "Retained oracle".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("metrics"),
            Schema {
                id: SchemaId::from("metrics"),
                key: SchemaKey::from("metrics"),
                fields,
            },
        )]),
        entities: BTreeMap::from([
            (
                EntityId::from(PRIMARY),
                Entity {
                    id: EntityId::from(PRIMARY),
                    key: EntityKey::from(PRIMARY),
                    schema: SchemaId::from("metrics"),
                    fields: primary_fields,
                },
            ),
            (
                EntityId::from(ISOLATED),
                Entity {
                    id: EntityId::from(ISOLATED),
                    key: EntityKey::from(ISOLATED),
                    schema: SchemaId::from("metrics"),
                    fields: isolated_fields,
                },
            ),
        ]),
    }
}

fn field(entity: &str, field_id: &str) -> FieldRef {
    FieldRef::new(entity, field_id)
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn literal(value: f64) -> Expression {
    Expression::Number(Number::new(value).unwrap())
}

fn reference(entity: &str, field_id: &str) -> Expression {
    Expression::Reference(field(entity, field_id))
}

fn primary_addition() -> Expression {
    add(reference(PRIMARY, SOURCE_A), reference(PRIMARY, SOURCE_B))
}

fn target_addition() -> Expression {
    add(reference(PRIMARY, TARGET), literal(1.0))
}

fn overdeep_expression() -> Expression {
    (0..65).fold(literal(1.0), |expression, _| add(expression, literal(1.0)))
}

fn add(left: Expression, right: Expression) -> Expression {
    Expression::Add {
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn multiply(left: Expression, right: Expression) -> Expression {
    Expression::Multiply {
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn divide(left: Expression, right: Expression) -> Expression {
    Expression::Divide {
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn maximum(left: Expression, right: Expression) -> Expression {
    Expression::Maximum {
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn entity_mut<'a>(document: &'a mut Document, entity: &str) -> &'a mut Entity {
    document.entities.get_mut(entity).unwrap()
}

fn field_definition_mut<'a>(document: &'a mut Document, field_id: &str) -> &'a mut FieldDefinition {
    document
        .schemas
        .get_mut("metrics")
        .unwrap()
        .fields
        .get_mut(field_id)
        .unwrap()
}

fn set_value(document: &mut Document, entity: &str, field_id: &str, value: Value) {
    entity_mut(document, entity)
        .fields
        .insert(FieldId::from(field_id), value);
}

fn set_formula(document: &mut Document, entity: &str, field_id: &str, value: Expression) {
    set_value(document, entity, field_id, Value::Formula(value));
}
