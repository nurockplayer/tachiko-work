//! Reproducible, ignored performance evidence for Issue #95.
//!
//! The synthetic shape matches Issue #91: one independent formula and three
//! numeric calculation nodes per entity. Timings are observations, not SLAs;
//! deterministic work counters carry the regression assertions.

use std::{collections::BTreeMap, time::Instant};

use tachiko_workspace_engine::{
    Document, DocumentId, Entity, EntityId, EntityKey, Expression, FieldDefinition, FieldId,
    FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value, calculate_fields,
    patch_lifecycle::{DocumentScopeId, SemanticPublicationAuthority, TrustedInstant},
    resident_session::{
        ResidentRuntimeMeasurements, ResidentWorkspaceSession, TrustedPublicationTimeSource,
    },
    validation_report,
};

const EDITS: usize = 20;
const ENTITY_COUNTS: [usize; 3] = [10, 100, 1_000];

struct FixedTime;

impl TrustedPublicationTimeSource for FixedTime {
    fn now(&mut self) -> TrustedInstant {
        TrustedInstant::new(1)
    }
}

#[test]
#[ignore = "run explicitly in release mode to record Issue #95 performance evidence"]
fn repeated_local_edits_reuse_material_calculation_work() {
    println!(
        "entities,edits,retained_us,fresh_oracle_us,initial_nodes,recomputed_edit_nodes,reused_edit_nodes,full_rebuilds,fallbacks"
    );
    for entity_count in ENTITY_COUNTS {
        let (retained_micros, fresh_micros, work) = run_case(entity_count);
        let initial_nodes = entity_count * 3;
        let recomputed_edit_nodes = work.calculation_nodes_recomputed - initial_nodes;
        println!(
            "{entity_count},{EDITS},{retained_micros},{fresh_micros},{initial_nodes},{recomputed_edit_nodes},{},{},{}",
            work.calculation_nodes_reused,
            work.full_calculation_rebuilds,
            work.calculation_fallbacks
        );

        assert_eq!(work.full_calculation_rebuilds, 1);
        assert_eq!(work.incremental_calculation_updates, EDITS);
        assert_eq!(recomputed_edit_nodes, EDITS * 2);
        assert_eq!(work.calculation_nodes_reused, EDITS * (initial_nodes - 2));
        assert_eq!(work.address_index_reuses, EDITS);
        assert_eq!(work.address_index_rebuilds, 1);
        assert_eq!(work.calculation_fallbacks, 0);
    }
}

fn run_case(entity_count: usize) -> (u128, u128, ResidentRuntimeMeasurements) {
    let mut document = synthetic_document(entity_count);
    let mut session = ResidentWorkspaceSession::new(
        DocumentScopeId::from(format!("benchmark-{entity_count}")),
        document.clone(),
    );
    let mut retained_nanos = 0_u128;
    let mut fresh_nanos = 0_u128;

    for edit in 0..EDITS {
        set_first_base(&mut document, edit);
        let start = Instant::now();
        let full_validation = validation_report(&document);
        let full_calculation = calculate_fields(&document).unwrap();
        fresh_nanos += start.elapsed().as_nanos();

        let expected_revision = session.revision().clone();
        let mut time = FixedTime;
        let start = Instant::now();
        {
            let mut publication = session.publication_authority(&mut time);
            publication
                .publish_if_current(
                    &DocumentScopeId::from(format!("benchmark-{entity_count}")),
                    &expected_revision,
                    document.clone(),
                    |_| Some(()),
                )
                .unwrap();
        }
        let retained_validation = session.validation_report().into_value();
        let retained_calculation = session.calculate_fields().unwrap().into_value();
        retained_nanos += start.elapsed().as_nanos();

        assert_eq!(retained_validation, full_validation);
        assert_eq!(retained_calculation, full_calculation);
    }

    (
        retained_nanos / 1_000,
        fresh_nanos / 1_000,
        session.runtime_measurements(),
    )
}

fn set_first_base(document: &mut Document, edit: usize) {
    document
        .entities
        .get_mut(&EntityId::from("synthetic-entity-000000"))
        .unwrap()
        .fields
        .insert(
            FieldId::from("synthetic-base-field-id"),
            Value::Number(Number::new(if edit % 2 == 0 { 2.0 } else { 1.0 }).unwrap()),
        );
}

fn synthetic_document(entity_count: usize) -> Document {
    let schema_id = SchemaId::from("synthetic-schema-id");
    let base = FieldId::from("synthetic-base-field-id");
    let multiplier = FieldId::from("synthetic-multiplier-field-id");
    let computed = FieldId::from("synthetic-computed-field-id");
    let label = FieldId::from("synthetic-label-field-id");
    let schemas = BTreeMap::from([(
        schema_id.clone(),
        Schema {
            id: schema_id.clone(),
            key: SchemaKey::from("synthetic_records"),
            fields: synthetic_fields(&base, &multiplier, &computed, &label),
        },
    )]);
    let entities = (0..entity_count)
        .map(|index| {
            let numeric_index = u32::try_from(index).unwrap();
            let id = EntityId::from(format!("synthetic-entity-{index:06}"));
            let entity = Entity {
                id: id.clone(),
                key: EntityKey::from(format!("entity_{index:04}")),
                schema: schema_id.clone(),
                fields: BTreeMap::from([
                    (
                        base.clone(),
                        Value::Number(number(f64::from(numeric_index) + 1.0)),
                    ),
                    (multiplier.clone(), Value::Number(number(2.0))),
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
                ]),
            };
            (id, entity)
        })
        .collect();
    Document {
        id: DocumentId::from("synthetic-document-id"),
        title: format!("Issue 95 synthetic {entity_count}"),
        schemas,
        entities,
    }
}

fn synthetic_fields(
    base: &FieldId,
    multiplier: &FieldId,
    computed: &FieldId,
    label: &FieldId,
) -> BTreeMap<FieldId, FieldDefinition> {
    BTreeMap::from([
        (base.clone(), number_field(base, "base")),
        (multiplier.clone(), number_field(multiplier, "multiplier")),
        (computed.clone(), number_field(computed, "computed")),
        (
            label.clone(),
            FieldDefinition {
                id: label.clone(),
                key: FieldKey::from("label"),
                field_type: FieldType::Text,
                required: true,
            },
        ),
    ])
}

fn number_field(id: &FieldId, key: &str) -> FieldDefinition {
    FieldDefinition {
        id: id.clone(),
        key: FieldKey::from(key),
        field_type: FieldType::Number,
        required: true,
    }
}

fn number(value: f64) -> Number {
    Number::new(value).unwrap()
}
