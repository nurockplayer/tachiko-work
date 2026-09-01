use std::collections::BTreeMap;

use tachiko_workspace_engine::{
    ConflictFacet, ConflictFact, ConflictKind, ConflictTarget, DiagnosticCode, Document,
    DocumentId, Entity, EntityId, EntitySubject, Expression, FieldDefinition, FieldId, FieldKey,
    FieldRef, FieldType, MergeConflict, MergeValue, Number, SEMANTIC_CONFLICT_V1, Schema,
    SchemaFieldSubject, SchemaId, SchemaKey, SchemaSubject, SemanticConflictContract,
    ValidationRole, Value, WorkspaceError, WorkspaceMergeOutcome, diagnostic_codes,
    merge_documents,
};

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn expression_number(value: f64) -> Expression {
    Expression::Number(Number::new(value).unwrap())
}

fn add_reference(entity: &str, field: &str, addend: f64) -> Value {
    Value::Formula(Expression::Add {
        left: Box::new(Expression::Reference(FieldRef::new(entity, field))),
        right: Box::new(expression_number(addend)),
    })
}

fn definition(id: &str, field_type: FieldType, required: bool) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(human_key(id)),
        field_type,
        required,
    }
}

fn schema(id: &str, fields: impl IntoIterator<Item = FieldDefinition>) -> Schema {
    Schema {
        id: SchemaId::from(id),
        key: SchemaKey::from(human_key(id)),
        fields: fields
            .into_iter()
            .map(|field| (field.id.clone(), field))
            .collect(),
    }
}

fn entity(
    id: &str,
    schema: &str,
    fields: impl IntoIterator<Item = (&'static str, Value)>,
) -> Entity {
    Entity {
        id: EntityId::from(id),
        key: human_key(id).into(),
        schema: SchemaId::from(schema),
        fields: fields
            .into_iter()
            .map(|(field, value)| (FieldId::from(field), value))
            .collect(),
    }
}

fn human_key(id: &str) -> &str {
    id.split_once(':').map_or(id, |(_, key)| key)
}

fn arena(
    schemas: impl IntoIterator<Item = Schema>,
    entities: impl IntoIterator<Item = Entity>,
) -> Document {
    Document {
        id: DocumentId::from("d:arena"),
        title: "Arena".to_owned(),
        schemas: schemas
            .into_iter()
            .map(|schema| (schema.id.clone(), schema))
            .collect(),
        entities: entities
            .into_iter()
            .map(|entity| (entity.id.clone(), entity))
            .collect(),
    }
}

fn unit_document(
    fields: impl IntoIterator<Item = FieldDefinition>,
    values: impl IntoIterator<Item = (&'static str, Value)>,
) -> Document {
    arena(
        [schema("s:unit", fields)],
        [entity("e:goblin", "s:unit", values)],
    )
}

fn merged(outcome: Result<WorkspaceMergeOutcome, WorkspaceError>) -> Document {
    match outcome.expect("fixture should pass admission and finalization") {
        WorkspaceMergeOutcome::Merged(preview) => preview.document,
        WorkspaceMergeOutcome::Conflicted(conflicts) => {
            panic!("fixture unexpectedly conflicted: {conflicts:#?}")
        }
    }
}

fn conflicted(outcome: Result<WorkspaceMergeOutcome, WorkspaceError>) -> Vec<MergeConflict> {
    match outcome.expect("fixture should pass admission") {
        WorkspaceMergeOutcome::Conflicted(conflicts) => conflicts,
        WorkspaceMergeOutcome::Merged(_) => panic!("fixture unexpectedly produced a candidate"),
    }
}

#[test]
fn fixture_01_independent_edits_merge_into_one_candidate() {
    let base = unit_document(
        [
            definition("f:hp", FieldType::Number, true),
            definition("f:attack", FieldType::Number, true),
        ],
        [("f:hp", number(180.0)), ("f:attack", number(18.0))],
    );
    let mut left = base.clone();
    left.entities
        .get_mut("e:goblin")
        .unwrap()
        .fields
        .insert("f:hp".into(), number(210.0));
    let mut right = base.clone();
    right
        .entities
        .get_mut("e:goblin")
        .unwrap()
        .fields
        .insert("f:attack".into(), number(21.0));

    let candidate = merged(merge_documents(&base, &left, &right));

    assert_eq!(candidate.entities["e:goblin"].fields["f:hp"], number(210.0));
    assert_eq!(
        candidate.entities["e:goblin"].fields["f:attack"],
        number(21.0)
    );
}

#[test]
fn fixture_02_same_final_value_is_not_a_conflict() {
    let base = unit_document(
        [definition("f:hp", FieldType::Number, true)],
        [("f:hp", number(180.0))],
    );
    let mut changed = base.clone();
    changed
        .entities
        .get_mut("e:goblin")
        .unwrap()
        .fields
        .insert("f:hp".into(), number(210.0));

    let candidate = merged(merge_documents(&base, &changed, &changed));

    assert_eq!(candidate.entities["e:goblin"].fields["f:hp"], number(210.0));
}

#[test]
fn fixture_03_same_fact_conflict_has_typed_target_kind_and_facts() {
    let base = unit_document(
        [definition("f:hp", FieldType::Number, true)],
        [("f:hp", number(180.0))],
    );
    let mut left = base.clone();
    left.entities
        .get_mut("e:goblin")
        .unwrap()
        .fields
        .insert("f:hp".into(), number(210.0));
    let mut right = base.clone();
    right
        .entities
        .get_mut("e:goblin")
        .unwrap()
        .fields
        .insert("f:hp".into(), number(240.0));

    let conflicts = conflicted(merge_documents(&base, &left, &right));

    assert_eq!(conflicts.len(), 1);
    let conflict = &conflicts[0];
    assert_eq!(conflict.contract().as_str(), SEMANTIC_CONFLICT_V1);
    assert_eq!(conflict.document(), &DocumentId::from("d:arena"));
    assert_eq!(
        conflict.target(),
        &ConflictTarget::StoredEntityField {
            entity: "e:goblin".into(),
            schema: "s:unit".into(),
            field: "f:hp".into(),
        }
    );
    assert_eq!(conflict.facet(), ConflictFacet::StoredValue);
    assert_eq!(conflict.kind(), ConflictKind::ConcurrentChange);
    assert_eq!(
        conflict.base(),
        &ConflictFact::Present(MergeValue::FieldValue(number(180.0)))
    );
    assert_eq!(
        conflict.left(),
        &ConflictFact::Present(MergeValue::FieldValue(number(210.0)))
    );
    assert_eq!(
        conflict.right(),
        &ConflictFact::Present(MergeValue::FieldValue(number(240.0)))
    );
}

#[test]
fn fixture_04_delete_update_emits_one_complete_parent_conflict() {
    let base = unit_document(
        [definition("f:hp", FieldType::Number, true)],
        [("f:hp", number(180.0))],
    );
    let mut left = base.clone();
    left.entities.remove("e:goblin");
    let mut right = base.clone();
    right.entities.get_mut("e:goblin").unwrap().key = "goblin_elite".into();

    let conflicts = conflicted(merge_documents(&base, &left, &right));

    assert_eq!(
        conflicts.len(),
        1,
        "the parent conflict suppresses child facts"
    );
    let conflict = &conflicts[0];
    assert_eq!(
        conflict.target(),
        &ConflictTarget::Entity("e:goblin".into())
    );
    assert_eq!(conflict.facet(), ConflictFacet::Subject);
    assert_eq!(conflict.kind(), ConflictKind::DeleteModify);
    assert_eq!(
        conflict.base(),
        &ConflictFact::Present(MergeValue::EntitySubject(EntitySubject {
            key: "goblin".into(),
            schema: "s:unit".into(),
            fields: BTreeMap::from([("f:hp".into(), number(180.0))]),
        }))
    );
    assert_eq!(conflict.left(), &ConflictFact::Absent);
    assert_eq!(
        conflict.right(),
        &ConflictFact::Present(MergeValue::EntitySubject(EntitySubject {
            key: "goblin_elite".into(),
            schema: "s:unit".into(),
            fields: BTreeMap::from([("f:hp".into(), number(180.0))]),
        }))
    );
}

#[test]
fn fixture_05_incompatible_concurrent_addition_uses_complete_schema_subjects() {
    let base = arena([], []);
    let left_schema = schema("s:boss", [definition("f:hp", FieldType::Number, true)]);
    let right_schema = schema("s:boss", [definition("f:hp", FieldType::Text, true)]);
    let left = arena([left_schema.clone()], []);
    let right = arena([right_schema.clone()], []);

    let conflicts = conflicted(merge_documents(&base, &left, &right));

    assert_eq!(conflicts.len(), 1);
    let conflict = &conflicts[0];
    assert_eq!(conflict.target(), &ConflictTarget::Schema("s:boss".into()));
    assert_eq!(conflict.facet(), ConflictFacet::Subject);
    assert_eq!(conflict.kind(), ConflictKind::ConcurrentAddition);
    assert_eq!(conflict.base(), &ConflictFact::Absent);
    assert_eq!(
        conflict.left(),
        &ConflictFact::Present(MergeValue::SchemaSubject(SchemaSubject {
            key: "boss".into(),
            fields: BTreeMap::from([(
                "f:hp".into(),
                SchemaFieldSubject {
                    key: "hp".into(),
                    field_type: FieldType::Number,
                    required: true,
                },
            )]),
        }))
    );
    assert_eq!(
        conflict.right(),
        &ConflictFact::Present(MergeValue::SchemaSubject(SchemaSubject {
            key: "boss".into(),
            fields: BTreeMap::from([(
                "f:hp".into(),
                SchemaFieldSubject {
                    key: "hp".into(),
                    field_type: FieldType::Text,
                    required: true,
                },
            )]),
        }))
    );
}

#[test]
fn fixture_06_rename_continuity_conflicts_on_key_not_identity() {
    let base = unit_document([], []);
    let mut left = base.clone();
    left.entities.get_mut("e:goblin").unwrap().key = "goblin_elite".into();
    let mut right = base.clone();
    right.entities.get_mut("e:goblin").unwrap().key = "goblin_veteran".into();

    let conflicts = conflicted(merge_documents(&base, &left, &right));

    assert_eq!(conflicts.len(), 1);
    assert_eq!(
        conflicts[0].target(),
        &ConflictTarget::Entity("e:goblin".into())
    );
    assert_eq!(conflicts[0].facet(), ConflictFacet::Key);
    assert_eq!(conflicts[0].kind(), ConflictKind::ConcurrentChange);
}

#[test]
fn fixture_07_schema_data_failure_is_candidate_validation_evidence() {
    let base = arena([schema("s:unit", [])], []);
    let left = arena(
        [schema(
            "s:unit",
            [definition("f:armor", FieldType::Number, true)],
        )],
        [],
    );
    let right = arena([schema("s:unit", [])], [entity("e:goblin", "s:unit", [])]);

    let error = merge_documents(&base, &left, &right).unwrap_err();

    let WorkspaceError::InvalidDocument { role, report, .. } = error else {
        panic!("schema/data incompatibility must use existing validation evidence")
    };
    assert_eq!(role, ValidationRole::MergeCandidate);
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::MISSING_REQUIRED_FIELD)
    );
}

#[test]
fn fixture_08_reference_failure_is_candidate_validation_evidence() {
    let unit = schema(
        "s:unit",
        [definition(
            "f:target",
            FieldType::Reference {
                schema: "s:unit".into(),
            },
            false,
        )],
    );
    let base = arena(
        [unit],
        [
            entity(
                "e:source",
                "s:unit",
                [("f:target", Value::Reference("e:old".into()))],
            ),
            entity("e:old", "s:unit", []),
            entity("e:target", "s:unit", []),
        ],
    );
    let mut left = base.clone();
    left.entities
        .get_mut("e:source")
        .unwrap()
        .fields
        .insert("f:target".into(), Value::Reference("e:target".into()));
    let mut right = base.clone();
    right.entities.remove("e:target");

    let error = merge_documents(&base, &left, &right).unwrap_err();

    let WorkspaceError::InvalidDocument { role, report, .. } = error else {
        panic!("dangling reference must use existing validation evidence")
    };
    assert_eq!(role, ValidationRole::MergeCandidate);
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::MISSING_REFERENCE)
    );
}

#[test]
fn fixture_09_bound_formula_conflict_preserves_bound_expressions() {
    let base_formula = add_reference("e:goblin", "f:hp", 1.0);
    let left_formula = add_reference("e:goblin", "f:hp", 2.0);
    let right_formula = add_reference("e:goblin", "f:hp", 3.0);
    let base = unit_document(
        [
            definition("f:hp", FieldType::Number, true),
            definition("f:power", FieldType::Number, true),
        ],
        [("f:hp", number(180.0)), ("f:power", base_formula.clone())],
    );
    let mut left = base.clone();
    left.entities
        .get_mut("e:goblin")
        .unwrap()
        .fields
        .insert("f:power".into(), left_formula.clone());
    let mut right = base.clone();
    right
        .entities
        .get_mut("e:goblin")
        .unwrap()
        .fields
        .insert("f:power".into(), right_formula.clone());

    let conflicts = conflicted(merge_documents(&base, &left, &right));

    assert_eq!(conflicts.len(), 1);
    assert_eq!(
        conflicts[0].target(),
        &ConflictTarget::StoredEntityField {
            entity: "e:goblin".into(),
            schema: "s:unit".into(),
            field: "f:power".into(),
        }
    );
    assert_eq!(
        conflicts[0].base(),
        &ConflictFact::Present(MergeValue::FieldValue(base_formula))
    );
    assert_eq!(
        conflicts[0].left(),
        &ConflictFact::Present(MergeValue::FieldValue(left_formula))
    );
    assert_eq!(
        conflicts[0].right(),
        &ConflictFact::Present(MergeValue::FieldValue(right_formula))
    );
}

#[test]
fn fixture_10_post_merge_formula_cycle_is_not_a_structural_conflict() {
    let base = unit_document(
        [
            definition("f:a", FieldType::Number, true),
            definition("f:b", FieldType::Number, true),
        ],
        [("f:a", number(1.0)), ("f:b", number(2.0))],
    );
    let mut left = base.clone();
    left.entities
        .get_mut("e:goblin")
        .unwrap()
        .fields
        .insert("f:a".into(), add_reference("e:goblin", "f:b", 1.0));
    let mut right = base.clone();
    right
        .entities
        .get_mut("e:goblin")
        .unwrap()
        .fields
        .insert("f:b".into(), add_reference("e:goblin", "f:a", 1.0));

    let error = merge_documents(&base, &left, &right).unwrap_err();

    let WorkspaceError::InvalidDocument { role, report, .. } = error else {
        panic!("formula cycle must use existing finalization evidence")
    };
    assert_eq!(role, ValidationRole::MergeCandidate);
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == diagnostic_codes::FORMULA_CYCLE)
    );
}

#[test]
fn fixture_11_stored_fields_are_qualified_by_each_state_schema() {
    let schemas = [
        schema("s:unit", [definition("f:hp", FieldType::Number, true)]),
        schema("s:boss", [definition("f:hp", FieldType::Number, true)]),
    ];
    let base = arena(
        schemas,
        [entity("e:goblin", "s:unit", [("f:hp", number(180.0))])],
    );
    let mut left = base.clone();
    left.entities.get_mut("e:goblin").unwrap().schema = "s:boss".into();
    let mut right = base.clone();
    right
        .entities
        .get_mut("e:goblin")
        .unwrap()
        .fields
        .insert("f:hp".into(), number(210.0));

    let conflicts = conflicted(merge_documents(&base, &left, &right));

    assert_eq!(conflicts.len(), 1);
    let conflict = &conflicts[0];
    assert_eq!(
        conflict.target(),
        &ConflictTarget::StoredEntityField {
            entity: "e:goblin".into(),
            schema: "s:unit".into(),
            field: "f:hp".into(),
        }
    );
    assert_eq!(conflict.kind(), ConflictKind::DeleteModify);
    assert_eq!(
        conflict.base(),
        &ConflictFact::Present(MergeValue::FieldValue(number(180.0)))
    );
    assert_eq!(conflict.left(), &ConflictFact::Absent);
    assert_eq!(
        conflict.right(),
        &ConflictFact::Present(MergeValue::FieldValue(number(210.0)))
    );
}

#[test]
fn one_sided_old_schema_field_addition_reaches_candidate_validation() {
    let base = arena(
        [
            schema("s:unit", [definition("f:armor", FieldType::Number, false)]),
            schema("s:boss", []),
        ],
        [entity("e:goblin", "s:unit", [])],
    );
    let mut left = base.clone();
    left.entities.get_mut("e:goblin").unwrap().schema = "s:boss".into();
    let mut right = base.clone();
    right
        .entities
        .get_mut("e:goblin")
        .unwrap()
        .fields
        .insert("f:armor".into(), number(12.0));

    let error = merge_documents(&base, &left, &right).unwrap_err();

    let WorkspaceError::InvalidDocument { role, report, .. } = error else {
        panic!("the one-sided old-schema fact must reach candidate finalization")
    };
    assert_eq!(role, ValidationRole::MergeCandidate);
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == diagnostic_codes::MERGE_UNMATERIALIZED_QUALIFIED_FIELD
    }));
}

#[test]
fn one_sided_old_schema_field_is_not_requalified_when_new_schema_reuses_id() {
    let base = arena(
        [
            schema("s:unit", [definition("f:armor", FieldType::Number, false)]),
            schema("s:boss", [definition("f:armor", FieldType::Number, false)]),
        ],
        [entity("e:goblin", "s:unit", [])],
    );
    let mut left = base.clone();
    left.entities.get_mut("e:goblin").unwrap().schema = "s:boss".into();
    let mut right = base.clone();
    right
        .entities
        .get_mut("e:goblin")
        .unwrap()
        .fields
        .insert("f:armor".into(), number(12.0));

    let error = merge_documents(&base, &left, &right).unwrap_err();

    let WorkspaceError::InvalidDocument { role, report, .. } = error else {
        panic!("an old-schema fact must not be retargeted to the selected schema")
    };
    assert_eq!(role, ValidationRole::MergeCandidate);
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == diagnostic_codes::MERGE_UNMATERIALIZED_QUALIFIED_FIELD
    }));
}

#[test]
fn fixture_12_conflicts_follow_canonical_semantic_order() {
    let base = arena(
        [
            schema("s:alpha", []),
            schema("s:unit", [definition("f:hp", FieldType::Number, true)]),
        ],
        [entity("e:goblin", "s:unit", [("f:hp", number(180.0))])],
    );
    let mut left = base.clone();
    left.title = "Left Arena".to_owned();
    left.schemas.get_mut("s:alpha").unwrap().key = "alpha_left".into();
    left.schemas.get_mut("s:unit").unwrap().key = "unit_left".into();
    let left_hp = left.schemas["s:unit"].fields["f:hp"].clone();
    left.schemas.get_mut("s:unit").unwrap().fields.insert(
        "f:hp".into(),
        FieldDefinition {
            key: "hp_left".into(),
            field_type: FieldType::Text,
            ..left_hp
        },
    );
    left.entities.get_mut("e:goblin").unwrap().key = "goblin_left".into();
    left.entities
        .get_mut("e:goblin")
        .unwrap()
        .fields
        .insert("f:hp".into(), Value::Text("210".to_owned()));

    let mut right = base.clone();
    right.title = "Right Arena".to_owned();
    right.schemas.get_mut("s:alpha").unwrap().key = "alpha_right".into();
    right.schemas.get_mut("s:unit").unwrap().key = "unit_right".into();
    let right_hp = right.schemas["s:unit"].fields["f:hp"].clone();
    right.schemas.get_mut("s:unit").unwrap().fields.insert(
        "f:hp".into(),
        FieldDefinition {
            key: "hp_right".into(),
            field_type: FieldType::Boolean,
            ..right_hp
        },
    );
    right.entities.get_mut("e:goblin").unwrap().key = "goblin_right".into();
    right
        .entities
        .get_mut("e:goblin")
        .unwrap()
        .fields
        .insert("f:hp".into(), Value::Boolean(true));

    let conflicts = conflicted(merge_documents(&base, &left, &right));
    let observations = conflicts
        .iter()
        .map(|conflict| (conflict.target().clone(), conflict.facet()))
        .collect::<Vec<_>>();

    assert_eq!(
        observations,
        [
            (
                ConflictTarget::Document("d:arena".into()),
                ConflictFacet::Title,
            ),
            (ConflictTarget::Schema("s:alpha".into()), ConflictFacet::Key,),
            (ConflictTarget::Schema("s:unit".into()), ConflictFacet::Key,),
            (
                ConflictTarget::SchemaField {
                    schema: "s:unit".into(),
                    field: "f:hp".into(),
                },
                ConflictFacet::Key,
            ),
            (
                ConflictTarget::SchemaField {
                    schema: "s:unit".into(),
                    field: "f:hp".into(),
                },
                ConflictFacet::FieldType,
            ),
            (
                ConflictTarget::Entity("e:goblin".into()),
                ConflictFacet::Key,
            ),
            (
                ConflictTarget::StoredEntityField {
                    entity: "e:goblin".into(),
                    schema: "s:unit".into(),
                    field: "f:hp".into(),
                },
                ConflictFacet::StoredValue,
            ),
        ]
    );
}

#[test]
fn canonical_id_order_is_unicode_scalar_order_without_normalization() {
    let decomposed_id = "s:e\u{301}";
    let precomposed_id = "s:\u{e9}";
    let mut decomposed = schema(decomposed_id, []);
    decomposed.key = "decomposed".into();
    let mut precomposed = schema(precomposed_id, []);
    precomposed.key = "precomposed".into();
    let base = arena([decomposed, precomposed], []);

    let mut left = base.clone();
    left.schemas.get_mut(decomposed_id).unwrap().key = "decomposed_left".into();
    left.schemas.get_mut(precomposed_id).unwrap().key = "precomposed_left".into();
    let mut right = base.clone();
    right.schemas.get_mut(decomposed_id).unwrap().key = "decomposed_right".into();
    right.schemas.get_mut(precomposed_id).unwrap().key = "precomposed_right".into();

    let conflicts = conflicted(merge_documents(&base, &left, &right));

    assert_eq!(
        conflicts
            .iter()
            .map(MergeConflict::target)
            .collect::<Vec<_>>(),
        [
            &ConflictTarget::Schema(decomposed_id.into()),
            &ConflictTarget::Schema(precomposed_id.into()),
        ]
    );
    assert_ne!(decomposed_id, precomposed_id);
}

#[test]
fn fixture_13_admission_and_compatibility_fail_closed() {
    let valid = unit_document([], []);
    let mut invalid = valid.clone();
    invalid.entities.get_mut("e:goblin").unwrap().schema = "s:missing".into();

    for (result, expected_role) in [
        (
            merge_documents(&invalid, &valid, &valid),
            ValidationRole::MergeBase,
        ),
        (
            merge_documents(&valid, &invalid, &valid),
            ValidationRole::MergeOurs,
        ),
        (
            merge_documents(&valid, &valid, &invalid),
            ValidationRole::MergeTheirs,
        ),
    ] {
        let WorkspaceError::InvalidDocument { role, .. } = result.unwrap_err() else {
            panic!("invalid input must fail admission without a structural result")
        };
        assert_eq!(role, expected_role);
    }

    let mut other_document = valid.clone();
    other_document.id = "d:other".into();
    assert!(matches!(
        merge_documents(&valid, &other_document, &valid),
        Err(WorkspaceError::DifferentMergeDocument { .. })
    ));

    assert_eq!(
        SemanticConflictContract::try_from(SEMANTIC_CONFLICT_V1).unwrap(),
        SemanticConflictContract::V1
    );
    assert!(SemanticConflictContract::try_from("tachiko.semantic-conflict/v2").is_err());
    assert!(ConflictKind::try_from("cross_fact_invalidity").is_err());
    assert!(
        ConflictTarget::Document("d:arena".into())
            .validate_facet(ConflictFacet::StoredValue)
            .is_err()
    );
}

#[test]
fn complete_subject_facts_exclude_stable_target_ids() {
    let field = definition("f:hp", FieldType::Number, true);
    let schema = schema("s:unit", [field.clone()]);
    let entity = entity("e:goblin", "s:unit", [("f:hp", number(180.0))]);

    assert_eq!(
        SchemaFieldSubject::from(&field),
        SchemaFieldSubject {
            key: "hp".into(),
            field_type: FieldType::Number,
            required: true,
        }
    );
    assert_eq!(
        SchemaSubject::from(&schema),
        SchemaSubject {
            key: "unit".into(),
            fields: BTreeMap::from([(
                "f:hp".into(),
                SchemaFieldSubject {
                    key: "hp".into(),
                    field_type: FieldType::Number,
                    required: true,
                },
            )]),
        }
    );
    assert_eq!(
        EntitySubject::from(&entity),
        EntitySubject {
            key: "goblin".into(),
            schema: "s:unit".into(),
            fields: BTreeMap::from([("f:hp".into(), number(180.0))]),
        }
    );
}

#[test]
fn equivalent_admitted_states_produce_equal_logical_conflicts() {
    let base_a = unit_document(
        [
            definition("f:hp", FieldType::Number, true),
            definition("f:attack", FieldType::Number, true),
        ],
        [("f:hp", number(180.0)), ("f:attack", number(18.0))],
    );
    let mut left_a = base_a.clone();
    left_a
        .entities
        .get_mut("e:goblin")
        .unwrap()
        .fields
        .insert("f:hp".into(), number(210.0));
    let mut right_a = base_a.clone();
    right_a
        .entities
        .get_mut("e:goblin")
        .unwrap()
        .fields
        .insert("f:hp".into(), number(240.0));

    let base_b = unit_document(
        [
            definition("f:attack", FieldType::Number, true),
            definition("f:hp", FieldType::Number, true),
        ],
        [("f:attack", number(18.0)), ("f:hp", number(180.0))],
    );
    let mut left_b = base_b.clone();
    left_b
        .entities
        .get_mut("e:goblin")
        .unwrap()
        .fields
        .insert("f:hp".into(), number(210.0));
    let mut right_b = base_b.clone();
    right_b
        .entities
        .get_mut("e:goblin")
        .unwrap()
        .fields
        .insert("f:hp".into(), number(240.0));

    assert_eq!(
        conflicted(merge_documents(&base_a, &left_a, &right_a)),
        conflicted(merge_documents(&base_b, &left_b, &right_b))
    );
}

#[test]
fn complete_entity_subject_uses_its_own_fields_map() {
    let mut entity = entity("e:goblin", "s:unit", [("f:hp", number(180.0))]);
    entity.fields.insert("f:extra".into(), number(7.0));

    assert_eq!(
        EntitySubject::from(&entity),
        EntitySubject {
            key: "goblin".into(),
            schema: "s:unit".into(),
            fields: BTreeMap::from([
                ("f:extra".into(), number(7.0)),
                ("f:hp".into(), number(180.0)),
            ]),
        }
    );
}
