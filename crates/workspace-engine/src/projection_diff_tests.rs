//! The selective projection invalidation scan still produces exactly what the
//! former full scan produced (A-06a).
//!
//! The legacy collectors below are a verbatim copy of the scan before A-06a.
//! They live only in this test module: production keeps one implementation.

use std::collections::{BTreeMap, BTreeSet};

use tachiko_formula_engine::RetainedCalculationState;

use super::{
    ProjectionChanges, collect_diagnostic_projection_changes, collect_entity_projection_changes,
    collect_schema_projection_changes, index_field_diagnostics,
};
use crate::{
    Diagnostic, Document, Entity, EntityId, EntityKey, Expression, FieldConstraint,
    FieldDefinition, FieldId, FieldRef, FieldType, Number, Schema, SchemaId, Value,
    instrumentation, validation_report_for_retained_calculation,
};

fn number_definition(id: &str, required: bool) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        display_name: None,
        number_format: None,
        field_type: FieldType::Number,
        required,
        constraint: FieldConstraint::None,
    }
}

fn text_definition(id: &str) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        display_name: None,
        number_format: None,
        field_type: FieldType::Text,
        required: false,
        constraint: FieldConstraint::None,
    }
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn formula_dps(damage: &FieldRef, attack_interval: &FieldRef) -> Value {
    Value::Formula(Expression::Divide {
        left: Box::new(Expression::Reference(damage.clone())),
        right: Box::new(Expression::Reference(attack_interval.clone())),
    })
}

fn row(
    id: &str,
    damage: f64,
    attack_interval: f64,
    dps: Value,
    nickname: &str,
) -> (EntityId, Entity) {
    let entity_id = EntityId::from(id);
    (
        entity_id.clone(),
        Entity {
            id: entity_id,
            key: EntityKey::from(id),
            schema: SchemaId::from("weapons"),
            fields: BTreeMap::from([
                ("attack_interval".into(), number(attack_interval)),
                ("damage".into(), number(damage)),
                ("dps".into(), dps),
                ("nickname".into(), Value::Text(nickname.to_owned())),
            ]),
        },
    )
}

fn differential_document() -> Document {
    let mut document = Document::empty("diff", "Diff");
    document.schemas.insert(
        SchemaId::from("weapons"),
        Schema {
            field_order: None,
            id: "weapons".into(),
            key: "weapons".into(),
            display_name: None,
            fields: BTreeMap::from([
                (
                    "attack_interval".into(),
                    number_definition("attack_interval", false),
                ),
                ("damage".into(), number_definition("damage", false)),
                ("dps".into(), number_definition("dps", false)),
                ("nickname".into(), text_definition("nickname")),
            ]),
        },
    );
    let sword_b = FieldRef::new("sword_b", "damage");
    let sword_b_interval = FieldRef::new("sword_b", "attack_interval");
    for (entity_id, entity) in [
        row("sword_a", 10.0, 2.0, number(5.0), "A"),
        row(
            "sword_b",
            20.0,
            2.0,
            formula_dps(&sword_b, &sword_b_interval),
            "B",
        ),
        row("sword_c", 30.0, 3.0, number(10.0), "C"),
        row("sword_d", 40.0, 4.0, number(10.0), "D"),
    ] {
        document.entities.insert(entity_id, entity);
    }
    document
}

fn set_number(document: &mut Document, entity: &str, field: &str, value: f64) {
    document
        .entities
        .get_mut(&EntityId::from(entity))
        .unwrap()
        .fields
        .insert(FieldId::from(field), number(value));
}

type NamedEdit = (&'static str, fn(&mut Document));

fn mixed_edits() -> [NamedEdit; 9] {
    [
        ("set a value", |document| {
            set_number(document, "sword_a", "damage", 50.0);
        }),
        ("unset a value", |document| {
            document
                .entities
                .get_mut(&EntityId::from("sword_b"))
                .unwrap()
                .fields
                .remove(&FieldId::from("nickname"));
        }),
        ("append an entity", |document| {
            let (entity_id, entity) = row("sword_e", 50.0, 5.0, number(10.0), "E");
            document.entities.insert(entity_id, entity);
        }),
        ("remove an entity", |document| {
            document.entities.remove(&EntityId::from("sword_d"));
        }),
        ("rename an entity key", |document| {
            document
                .entities
                .get_mut(&EntityId::from("sword_c"))
                .unwrap()
                .key = EntityKey::from("crystal_blade");
        }),
        ("set a formula", |document| {
            let damage = FieldRef::new("sword_a", "damage");
            let attack_interval = FieldRef::new("sword_a", "attack_interval");
            document
                .entities
                .get_mut(&EntityId::from("sword_a"))
                .unwrap()
                .fields
                .insert(FieldId::from("dps"), formula_dps(&damage, &attack_interval));
        }),
        ("make a formula fail", |document| {
            set_number(document, "sword_b", "attack_interval", 0.0);
        }),
        ("fix the failing formula", |document| {
            set_number(document, "sword_b", "attack_interval", 2.0);
        }),
        ("flip a required definition", |document| {
            let definition = document
                .schemas
                .get_mut(&SchemaId::from("weapons"))
                .unwrap()
                .fields
                .get_mut(&FieldId::from("damage"))
                .unwrap();
            definition.required = !definition.required;
        }),
    ]
}

fn diagnostics_by_field(document: &Document) -> BTreeMap<FieldRef, Vec<Diagnostic>> {
    let (calculation, _) = RetainedCalculationState::rebuild(document);
    index_field_diagnostics(&validation_report_for_retained_calculation(
        document,
        &calculation,
    ))
}

#[derive(Debug, Eq, PartialEq)]
struct ChangesSummary {
    schemas: Vec<SchemaId>,
    schema_fields: Vec<(SchemaId, FieldId)>,
    entities: Vec<EntityId>,
    fields: Vec<FieldRef>,
    calculation_roots: Vec<FieldRef>,
    address_index_changed: bool,
}

fn summarize(changes: &ProjectionChanges) -> ChangesSummary {
    ChangesSummary {
        schemas: changes.schemas.iter().cloned().collect(),
        schema_fields: changes.schema_fields.iter().cloned().collect(),
        entities: changes.entities.iter().cloned().collect(),
        fields: changes.fields.iter().cloned().collect(),
        calculation_roots: changes.calculation_roots.iter().cloned().collect(),
        address_index_changed: changes.address_index_changed,
    }
}

fn legacy_changes(before: &Document, after: &Document) -> ChangesSummary {
    let entity_ids = before
        .entities
        .keys()
        .chain(after.entities.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let before_diagnostics = diagnostics_by_field(before);
    let after_diagnostics = diagnostics_by_field(after);
    let mut changes = ProjectionChanges::default();
    legacy_collect_entity_projection_changes(before, after, &entity_ids, &mut changes);
    legacy_collect_schema_projection_changes(before, after, &entity_ids, &mut changes);
    legacy_collect_diagnostic_projection_changes(
        before,
        &before_diagnostics,
        after,
        &after_diagnostics,
        &entity_ids,
        &mut changes,
    );
    summarize(&changes)
}

fn current_changes(before: &Document, after: &Document) -> ChangesSummary {
    let entity_ids = before
        .entities
        .keys()
        .chain(after.entities.keys())
        .collect::<BTreeSet<_>>();
    let before_diagnostics = diagnostics_by_field(before);
    let after_diagnostics = diagnostics_by_field(after);
    let mut changes = ProjectionChanges::default();
    collect_entity_projection_changes(before, after, &entity_ids, &mut changes);
    collect_schema_projection_changes(before, after, &entity_ids, &mut changes);
    collect_diagnostic_projection_changes(
        before,
        &before_diagnostics,
        after,
        &after_diagnostics,
        &mut changes,
    );
    summarize(&changes)
}

#[test]
fn selective_scan_matches_the_former_full_scan_over_mixed_edits() {
    let mut document = differential_document();
    for (label, edit) in mixed_edits() {
        let mut after = document.clone();
        edit(&mut after);
        assert_eq!(
            current_changes(&document, &after),
            legacy_changes(&document, &after),
            "{label}"
        );
        document = after;
    }
}

fn legacy_collect_entity_projection_changes(
    before: &Document,
    after: &Document,
    entity_ids: &BTreeSet<EntityId>,
    changes: &mut ProjectionChanges,
) {
    instrumentation::record_publication_entities_inspected(
        u64::try_from(entity_ids.len()).unwrap_or(u64::MAX),
    );
    for entity_id in entity_ids {
        match (
            before.entities.get(entity_id),
            after.entities.get(entity_id),
        ) {
            (Some(before_entity), Some(after_entity)) => {
                let field_ids = before_entity
                    .fields
                    .keys()
                    .chain(after_entity.fields.keys())
                    .cloned()
                    .collect::<BTreeSet<_>>();
                let presentation_changed = before_entity.key != after_entity.key;
                let schema_changed = before_entity.schema != after_entity.schema;
                if presentation_changed || schema_changed || before_entity.id != after_entity.id {
                    changes.address_index_changed = true;
                }

                if presentation_changed || schema_changed {
                    changes.entities.insert(entity_id.clone());
                    changes.fields.extend(
                        field_ids
                            .iter()
                            .cloned()
                            .map(|field_id| FieldRef::new(entity_id.clone(), field_id)),
                    );
                }
                if schema_changed {
                    changes.calculation_roots.extend(
                        field_ids
                            .iter()
                            .cloned()
                            .map(|field_id| FieldRef::new(entity_id.clone(), field_id)),
                    );
                }

                instrumentation::record_publication_fields_inspected(
                    u64::try_from(field_ids.len()).unwrap_or(u64::MAX),
                );
                for field_id in field_ids {
                    if before_entity.fields.get(&field_id) != after_entity.fields.get(&field_id) {
                        let field = FieldRef::new(entity_id.clone(), field_id);
                        changes.fields.insert(field.clone());
                        changes.calculation_roots.insert(field);
                    }
                }
                if before_entity.fields.keys().ne(after_entity.fields.keys()) {
                    changes.entities.insert(entity_id.clone());
                }
            }
            (Some(entity), None) | (None, Some(entity)) => {
                changes.address_index_changed = true;
                changes.entities.insert(entity_id.clone());
                let entity_fields = entity
                    .fields
                    .keys()
                    .cloned()
                    .map(|field_id| FieldRef::new(entity_id.clone(), field_id))
                    .collect::<BTreeSet<_>>();
                changes.fields.extend(entity_fields.iter().cloned());
                changes.calculation_roots.extend(entity_fields);
            }
            (None, None) => unreachable!("entity ID came from one document"),
        }
    }
}

fn legacy_collect_schema_projection_changes(
    before: &Document,
    after: &Document,
    entity_ids: &BTreeSet<EntityId>,
    changes: &mut ProjectionChanges,
) {
    let schema_ids = before
        .schemas
        .keys()
        .chain(after.schemas.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for schema_id in schema_ids {
        let before_schema = before.schemas.get(&schema_id);
        let after_schema = after.schemas.get(&schema_id);
        if before_schema.map(|schema| (&schema.id, &schema.key))
            != after_schema.map(|schema| (&schema.id, &schema.key))
        {
            changes.address_index_changed = true;
        }
        if before_schema.map(|schema| {
            (
                &schema.id,
                &schema.key,
                &schema.display_name,
                &schema.field_order,
            )
        }) != after_schema.map(|schema| {
            (
                &schema.id,
                &schema.key,
                &schema.display_name,
                &schema.field_order,
            )
        }) {
            changes.schemas.insert(schema_id.clone());
        }
        let field_ids = before_schema
            .into_iter()
            .flat_map(|schema| schema.fields.keys())
            .chain(
                after_schema
                    .into_iter()
                    .flat_map(|schema| schema.fields.keys()),
            )
            .cloned()
            .collect::<BTreeSet<_>>();

        for field_id in field_ids {
            instrumentation::record_publication_fields_inspected(1);
            let before_definition = before_schema.and_then(|schema| schema.fields.get(&field_id));
            let after_definition = after_schema.and_then(|schema| schema.fields.get(&field_id));
            if before_definition == after_definition {
                continue;
            }
            changes
                .schema_fields
                .insert((schema_id.clone(), field_id.clone()));
            let address_definition_changed = match (before_definition, after_definition) {
                (Some(before), Some(after)) => before.id != after.id || before.key != after.key,
                (Some(_), None) | (None, Some(_)) => true,
                (None, None) => false,
            };
            changes.address_index_changed |= address_definition_changed;
            let semantic_definition_changed = match (before_definition, after_definition) {
                (Some(before), Some(after)) => {
                    before.id != after.id
                        || before.field_type != after.field_type
                        || before.required != after.required
                }
                (Some(_), None) | (None, Some(_)) => true,
                (None, None) => false,
            };

            for entity_id in entity_ids {
                instrumentation::record_publication_fields_inspected(1);
                let before_has_field = before.entities.get(entity_id).is_some_and(|entity| {
                    entity.schema == schema_id && entity.fields.contains_key(&field_id)
                });
                let after_has_field = after.entities.get(entity_id).is_some_and(|entity| {
                    entity.schema == schema_id && entity.fields.contains_key(&field_id)
                });
                if before_has_field || after_has_field {
                    let field = FieldRef::new(entity_id.clone(), field_id.clone());
                    changes.fields.insert(field.clone());
                    if semantic_definition_changed {
                        changes.calculation_roots.insert(field);
                    }
                }
            }
        }
    }
}

fn legacy_collect_diagnostic_projection_changes(
    before: &Document,
    before_diagnostics: &BTreeMap<FieldRef, Vec<Diagnostic>>,
    after: &Document,
    after_diagnostics: &BTreeMap<FieldRef, Vec<Diagnostic>>,
    entity_ids: &BTreeSet<EntityId>,
    changes: &mut ProjectionChanges,
) {
    for entity_id in entity_ids {
        let field_ids = before
            .entities
            .get(entity_id)
            .into_iter()
            .flat_map(|entity| entity.fields.keys())
            .chain(
                after
                    .entities
                    .get(entity_id)
                    .into_iter()
                    .flat_map(|entity| entity.fields.keys()),
            )
            .cloned()
            .collect::<BTreeSet<_>>();

        instrumentation::record_publication_fields_inspected(
            u64::try_from(field_ids.len()).unwrap_or(u64::MAX),
        );
        for field_id in field_ids {
            let field = FieldRef::new(entity_id.clone(), field_id);
            if before_diagnostics.get(&field) != after_diagnostics.get(&field) {
                changes.fields.insert(field);
            }
        }
    }
}
