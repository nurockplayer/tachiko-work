//! Differential and work-bound tests for patch disclosure derivation.
//!
//! The `reference` module is a verbatim copy of the derivation this change
//! replaced. Both paths must produce exactly the same requirement set, and the
//! same failure, for every accepted edit shape. The work bound asserts that one
//! edit to a column read by every row walks that column's member slots a small
//! constant number of times instead of once per row.

use super::*;
use tachiko_semantic_core::{EntityKey, Schema, SchemaKey};

fn lifecycle() -> PatchLifecycle {
    PatchLifecycle::new(
        AuthorizationDomainId::from("domain"),
        DocumentScopeId::from("document-occurrence"),
        DocumentId::from("document"),
        SemanticApiContract::from("semantic-v1"),
        AuthorizationPolicyVersion::from("policy-v1"),
        PolicyMeaningId::from("policy-v1-meaning"),
    )
}

fn number_field(id: &str) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(id),
        display_name: None,
        number_format: None,
        field_type: FieldType::Number,
        // Optional, or a formula into a blank slot is refused before any
        // disclosure could be compared.
        required: false,
        constraint: FieldConstraint::None,
    }
}

fn number_schema(id: &str, fields: &[&str]) -> Schema {
    Schema {
        field_order: None,
        id: SchemaId::from(id),
        key: SchemaKey::from(id),
        display_name: None,
        fields: fields
            .iter()
            .map(|field| (FieldId::from(*field), number_field(field)))
            .collect(),
    }
}

fn entity(id: &str, schema: &str, fields: &[(&str, Value)]) -> Entity {
    Entity {
        id: EntityId::from(id),
        key: EntityKey::from(id),
        schema: SchemaId::from(schema),
        fields: fields
            .iter()
            .map(|(field, value)| (FieldId::from(*field), value.clone()))
            .collect(),
    }
}

fn document_with(schemas: &[Schema], entities: Vec<Entity>) -> Document {
    let mut document = Document::empty("document", "Differential");
    for schema in schemas {
        document.schemas.insert(schema.id.clone(), schema.clone());
    }
    for entity in entities {
        document.entities.insert(entity.id.clone(), entity);
    }
    document
}

fn field(entity: &str, field: &str) -> FieldRef {
    FieldRef::new(entity, field)
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn literal(value: f64) -> Expression {
    Expression::Number(Number::new(value).unwrap())
}

fn reference(entity: &str, field_id: &str) -> Expression {
    Expression::Reference(FieldRef::new(entity, field_id))
}

fn column(schema: &str, field_id: &str) -> Expression {
    Expression::ColumnTotal(ColumnRef::new(schema, field_id))
}

fn add(left: Expression, right: Expression) -> Expression {
    Expression::Add {
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn formula(expression: Expression) -> Value {
    Value::Formula(expression)
}

fn set_value(document: &mut Document, entity_id: &str, field_id: &str, value: Value) {
    document
        .entities
        .get_mut(&EntityId::from(entity_id))
        .unwrap()
        .fields
        .insert(FieldId::from(field_id), value);
}

fn unset_value(document: &mut Document, entity_id: &str, field_id: &str) {
    document
        .entities
        .get_mut(&EntityId::from(entity_id))
        .unwrap()
        .fields
        .remove(&FieldId::from(field_id));
}

struct Case {
    before: Document,
    after: Document,
    body: SemanticPatchBody,
    changes: Vec<SemanticChange>,
}

fn both_derivations(
    case: &Case,
) -> (
    Result<BTreeSet<DisclosureRequirement>, PatchLifecycleError>,
    Result<BTreeSet<DisclosureRequirement>, PatchLifecycleError>,
) {
    let lifecycle = lifecycle();
    let derived =
        lifecycle.derive_disclosures(&case.before, &case.after, &case.body, &case.changes);
    let expected = reference::derive_disclosures(
        &lifecycle,
        &case.before,
        &case.after,
        &case.body,
        &case.changes,
    );
    (derived, expected)
}

/// The case derives successfully and both paths give the same disclosures.
fn assert_matches_reference(case: &Case) {
    let (derived, expected) = both_derivations(case);
    let derived = derived.expect("the case derives");
    assert!(!derived.is_empty(), "the case discloses something");
    assert_eq!(derived, expected.expect("the reference derives"));
}

/// The case is refused and both paths give the same error.
fn assert_matches_reference_error(case: &Case) {
    let (derived, expected) = both_derivations(case);
    let derived = derived.expect_err("the case is refused");
    let expected = expected.expect_err("the reference refuses");
    assert_eq!(format!("{derived:?}"), format!("{expected:?}"));
}

#[test]
fn value_edit_matches_the_reference() {
    let schema = number_schema("s", &["amount", "total"]);
    let before = document_with(
        &[schema],
        vec![entity(
            "row",
            "s",
            &[("amount", number(1.0)), ("total", number(1.0))],
        )],
    );
    let mut after = before.clone();
    set_value(&mut after, "row", "amount", number(5.0));
    let case = Case {
        before,
        after,
        body: SemanticPatchBody::atomic_batch(vec![SemanticCommand::set_field_value(
            field("row", "amount"),
            number(5.0),
        )])
        .unwrap(),
        changes: vec![SemanticChange::FieldChanged {
            field: field("row", "amount"),
            before: number(1.0),
            after: number(5.0),
        }],
    };
    assert_matches_reference(&case);
}

#[test]
fn formula_edit_matches_the_reference() {
    let schema = number_schema("s", &["amount", "total"]);
    let before = document_with(
        &[schema],
        vec![entity(
            "row",
            "s",
            &[("amount", number(1.0)), ("total", number(1.0))],
        )],
    );
    let expression = add(reference("row", "amount"), literal(1.0));
    let mut after = before.clone();
    set_value(&mut after, "row", "total", formula(expression.clone()));
    let case = Case {
        before: before.clone(),
        after,
        body: SemanticPatchBody::command(SemanticCommand::FormulaUpdate(
            FormulaUpdateCommand::new(&before, field("row", "total"), expression.clone()),
        )),
        changes: vec![SemanticChange::FieldChanged {
            field: field("row", "total"),
            before: number(1.0),
            after: formula(expression),
        }],
    };
    assert_matches_reference(&case);
}

#[test]
fn copied_formula_into_blanks_and_over_numbers_matches_the_reference() {
    let schema = number_schema("s", &["amount", "total"]);
    let before = document_with(
        &[schema],
        vec![
            entity("r1", "s", &[("amount", number(1.0))]),
            entity(
                "r2",
                "s",
                &[("amount", number(2.0)), ("total", number(0.0))],
            ),
            entity(
                "r3",
                "s",
                &[("amount", number(3.0)), ("total", number(0.0))],
            ),
        ],
    );
    let expression = column("s", "amount");
    let mut after = before.clone();
    for row in ["r1", "r2", "r3"] {
        set_value(&mut after, row, "total", formula(expression.clone()));
    }
    let case = Case {
        before: before.clone(),
        after,
        body: SemanticPatchBody::atomic_batch(vec![
            SemanticCommand::formula_restore(field("r1", "total"), expression.clone()),
            SemanticCommand::FormulaUpdate(FormulaUpdateCommand::new(
                &before,
                field("r2", "total"),
                expression.clone(),
            )),
            SemanticCommand::FormulaUpdate(FormulaUpdateCommand::new(
                &before,
                field("r3", "total"),
                expression.clone(),
            )),
        ])
        .unwrap(),
        changes: vec![
            SemanticChange::FieldAdded {
                field: field("r1", "total"),
                value: formula(expression.clone()),
            },
            SemanticChange::FieldChanged {
                field: field("r2", "total"),
                before: number(0.0),
                after: formula(expression.clone()),
            },
            SemanticChange::FieldChanged {
                field: field("r3", "total"),
                before: number(0.0),
                after: formula(expression),
            },
        ],
    };
    assert_matches_reference(&case);
}

#[test]
fn unset_matches_the_reference() {
    let schema = number_schema("s", &["amount", "total"]);
    let before = document_with(
        &[schema],
        vec![entity(
            "row",
            "s",
            &[("amount", number(1.0)), ("total", number(1.0))],
        )],
    );
    let mut after = before.clone();
    unset_value(&mut after, "row", "total");
    let case = Case {
        before,
        after,
        body: SemanticPatchBody::command(SemanticCommand::UnsetField {
            field: field("row", "total"),
        }),
        changes: vec![SemanticChange::FieldRemoved {
            field: field("row", "total"),
            value: number(1.0),
        }],
    };
    assert_matches_reference(&case);
}

#[test]
fn row_insert_and_remove_match_the_reference() {
    let schema = number_schema("s", &["amount", "total"]);
    let before = document_with(
        std::slice::from_ref(&schema),
        vec![entity(
            "row-1",
            "s",
            &[("amount", number(1.0)), ("total", number(1.0))],
        )],
    );
    let after = document_with(
        &[schema],
        vec![entity(
            "row-2",
            "s",
            &[("amount", number(2.0)), ("total", number(2.0))],
        )],
    );
    let case = Case {
        before,
        after,
        body: SemanticPatchBody::atomic_batch(vec![
            SemanticCommand::AppendEntity {
                entity: entity(
                    "row-2",
                    "s",
                    &[("amount", number(2.0)), ("total", number(2.0))],
                ),
            },
            SemanticCommand::RemoveEntity {
                entity: EntityId::from("row-1"),
            },
        ])
        .unwrap(),
        changes: vec![
            SemanticChange::EntityAdded {
                entity: EntityId::from("row-2"),
            },
            SemanticChange::EntityRemoved {
                entity: EntityId::from("row-1"),
            },
        ],
    };
    assert_matches_reference(&case);
}

#[test]
fn mixed_batch_matches_the_reference() {
    let schema = number_schema("s", &["amount", "total", "other"]);
    let before = document_with(
        &[schema],
        vec![
            entity(
                "r1",
                "s",
                &[
                    ("amount", number(1.0)),
                    ("total", number(1.0)),
                    ("other", number(1.0)),
                ],
            ),
            entity(
                "r2",
                "s",
                &[
                    ("amount", number(2.0)),
                    ("total", formula(column("s", "amount"))),
                    ("other", number(2.0)),
                ],
            ),
        ],
    );
    let mut after = before.clone();
    set_value(&mut after, "r1", "amount", number(5.0));
    unset_value(&mut after, "r2", "other");
    let case = Case {
        before,
        after,
        body: SemanticPatchBody::atomic_batch(vec![
            SemanticCommand::set_field_value(field("r1", "amount"), number(5.0)),
            SemanticCommand::UnsetField {
                field: field("r2", "other"),
            },
        ])
        .unwrap(),
        changes: vec![
            SemanticChange::FieldChanged {
                field: field("r1", "amount"),
                before: number(1.0),
                after: number(5.0),
            },
            SemanticChange::FieldRemoved {
                field: field("r2", "other"),
                value: number(2.0),
            },
            SemanticChange::FormulaImpact {
                field: field("r2", "total"),
                before: Number::new(2.0).unwrap(),
                after: Number::new(2.0).unwrap(),
                causes: vec![field("r1", "amount")],
            },
        ],
    };
    assert_matches_reference(&case);
}

#[test]
fn set_constraint_matches_the_reference() {
    let schema = number_schema("s", &["amount", "total"]);
    let before = document_with(
        &[schema],
        vec![
            entity(
                "r1",
                "s",
                &[
                    ("amount", number(1.0)),
                    ("total", formula(reference("r1", "amount"))),
                ],
            ),
            entity(
                "r2",
                "s",
                &[
                    ("amount", number(2.0)),
                    ("total", formula(column("s", "amount"))),
                ],
            ),
        ],
    );
    let after = before.clone();
    let case = Case {
        before,
        after,
        body: SemanticPatchBody::command(SemanticCommand::SetFieldConstraint {
            schema: SchemaId::from("s"),
            field: FieldId::from("total"),
            constraint: FieldConstraint::None,
        }),
        // A constraint carries v2 evidence, so the command arm derives alone.
        changes: Vec::new(),
    };
    assert_matches_reference(&case);
}

#[test]
fn column_in_another_schema_matches_the_reference() {
    let schemas = [
        number_schema("s", &["amount", "total"]),
        number_schema("t", &["rate"]),
    ];
    let before = document_with(
        &schemas,
        vec![
            entity(
                "r1",
                "s",
                &[
                    ("amount", number(1.0)),
                    ("total", formula(column("t", "rate"))),
                ],
            ),
            entity("t1", "t", &[("rate", number(4.0))]),
            entity("t2", "t", &[("rate", number(5.0))]),
        ],
    );
    let mut after = before.clone();
    set_value(&mut after, "t1", "rate", number(6.0));
    let case = Case {
        before,
        after,
        body: SemanticPatchBody::command(SemanticCommand::set_field_value(
            field("t1", "rate"),
            number(6.0),
        )),
        changes: vec![
            SemanticChange::FieldChanged {
                field: field("t1", "rate"),
                before: number(4.0),
                after: number(6.0),
            },
            SemanticChange::FormulaImpact {
                field: field("r1", "total"),
                before: Number::new(0.0).unwrap(),
                after: Number::new(0.0).unwrap(),
                causes: vec![field("t1", "rate")],
            },
        ],
    };
    assert_matches_reference(&case);
}

#[test]
fn formula_chain_across_columns_matches_the_reference() {
    let schema = number_schema("s", &["a", "b", "c"]);
    let before = document_with(
        &[schema],
        vec![entity(
            "row",
            "s",
            &[
                ("a", number(1.0)),
                ("b", formula(reference("row", "a"))),
                ("c", formula(reference("row", "b"))),
            ],
        )],
    );
    let mut after = before.clone();
    set_value(&mut after, "row", "a", number(5.0));
    let case = Case {
        before,
        after,
        body: SemanticPatchBody::command(SemanticCommand::set_field_value(
            field("row", "a"),
            number(5.0),
        )),
        changes: vec![
            SemanticChange::FieldChanged {
                field: field("row", "a"),
                before: number(1.0),
                after: number(5.0),
            },
            SemanticChange::FormulaImpact {
                field: field("row", "b"),
                before: Number::new(1.0).unwrap(),
                after: Number::new(5.0).unwrap(),
                causes: vec![field("row", "a")],
            },
            SemanticChange::FormulaImpact {
                field: field("row", "c"),
                before: Number::new(1.0).unwrap(),
                after: Number::new(5.0).unwrap(),
                causes: vec![field("row", "b")],
            },
        ],
    };
    assert_matches_reference(&case);
}

#[test]
fn column_in_an_if_branch_matches_the_reference() {
    let schemas = [
        number_schema("s", &["amount", "total"]),
        number_schema("t", &["rate"]),
    ];
    let expression = Expression::If {
        condition: Box::new(Expression::IsBlank(Box::new(reference("r1", "amount")))),
        then_value: Box::new(column("t", "rate")),
        else_value: Box::new(literal(0.0)),
    };
    let before = document_with(
        &schemas,
        vec![
            entity(
                "r1",
                "s",
                &[("amount", number(1.0)), ("total", formula(expression))],
            ),
            entity("t1", "t", &[("rate", number(2.0))]),
        ],
    );
    let mut after = before.clone();
    set_value(&mut after, "t1", "rate", number(3.0));
    let case = Case {
        before,
        after,
        body: SemanticPatchBody::command(SemanticCommand::set_field_value(
            field("t1", "rate"),
            number(3.0),
        )),
        changes: vec![
            SemanticChange::FieldChanged {
                field: field("t1", "rate"),
                before: number(2.0),
                after: number(3.0),
            },
            SemanticChange::FormulaImpact {
                field: field("r1", "total"),
                before: Number::new(0.0).unwrap(),
                after: Number::new(0.0).unwrap(),
                causes: vec![field("t1", "rate")],
            },
        ],
    };
    assert_matches_reference(&case);
}

#[test]
fn blank_column_slots_match_the_reference() {
    let schema = number_schema("s", &["amount", "total"]);
    let before = document_with(
        &[schema],
        vec![
            entity(
                "r1",
                "s",
                &[
                    ("amount", number(1.0)),
                    ("total", formula(column("s", "amount"))),
                ],
            ),
            entity("r2", "s", &[]),
            entity("r3", "s", &[("amount", number(3.0))]),
        ],
    );
    let mut after = before.clone();
    set_value(&mut after, "r1", "amount", number(2.0));
    let case = Case {
        before,
        after,
        body: SemanticPatchBody::command(SemanticCommand::set_field_value(
            field("r1", "amount"),
            number(2.0),
        )),
        changes: vec![
            SemanticChange::FieldChanged {
                field: field("r1", "amount"),
                before: number(1.0),
                after: number(2.0),
            },
            SemanticChange::FormulaImpact {
                field: field("r1", "total"),
                before: Number::new(1.0).unwrap(),
                after: Number::new(2.0).unwrap(),
                causes: vec![field("r1", "amount")],
            },
        ],
    };
    assert_matches_reference(&case);
}

#[test]
fn missing_entity_derivation_error_matches_the_reference() {
    let schema = number_schema("s", &["amount"]);
    let before = document_with(
        &[schema],
        vec![entity("row", "s", &[("amount", number(1.0))])],
    );
    let after = before.clone();
    let case = Case {
        before,
        after,
        body: SemanticPatchBody::command(SemanticCommand::set_field_value(
            field("ghost", "amount"),
            number(1.0),
        )),
        changes: vec![SemanticChange::FieldChanged {
            field: field("ghost", "amount"),
            before: number(1.0),
            after: number(2.0),
        }],
    };
    assert_matches_reference_error(&case);
    assert!(matches!(
        lifecycle().derive_disclosures(&case.before, &case.after, &case.body, &case.changes),
        Err(PatchLifecycleError::ScopeDerivationFailed)
    ));
}

#[test]
fn undeclared_field_derivation_error_matches_the_reference() {
    let schema = number_schema("s", &["amount"]);
    let before = document_with(
        &[schema],
        vec![entity("row", "s", &[("amount", number(1.0))])],
    );
    let after = before.clone();
    let case = Case {
        before,
        after,
        body: SemanticPatchBody::command(SemanticCommand::set_field_value(
            field("row", "ghost"),
            number(1.0),
        )),
        changes: vec![SemanticChange::FieldChanged {
            field: field("row", "ghost"),
            before: number(1.0),
            after: number(2.0),
        }],
    };
    assert_matches_reference_error(&case);
    assert!(matches!(
        lifecycle().derive_disclosures(&case.before, &case.after, &case.body, &case.changes),
        Err(PatchLifecycleError::ScopeDerivationFailed)
    ));
}

#[test]
fn one_edit_walks_a_read_column_a_bounded_number_of_times() {
    const ROWS: usize = 2_000;
    let schema = number_schema("s", &["amount", "total"]);
    let entities = (0..ROWS)
        .map(|index| {
            entity(
                &format!("row-{index}"),
                "s",
                &[
                    ("amount", number(1.0)),
                    ("total", formula(column("s", "amount"))),
                ],
            )
        })
        .collect::<Vec<_>>();
    let before = document_with(std::slice::from_ref(&schema), entities);
    let mut after = before.clone();
    set_value(&mut after, "row-0", "amount", number(2.0));

    let mut changes = vec![SemanticChange::FieldChanged {
        field: field("row-0", "amount"),
        before: number(1.0),
        after: number(2.0),
    }];
    changes.extend((0..ROWS).map(|index| SemanticChange::FormulaImpact {
        field: field(&format!("row-{index}"), "total"),
        before: Number::new(1.0).unwrap(),
        after: Number::new(2.0).unwrap(),
        causes: vec![field("row-0", "amount")],
    }));

    let case = Case {
        before,
        after,
        body: SemanticPatchBody::command(SemanticCommand::set_field_value(
            field("row-0", "amount"),
            number(2.0),
        )),
        changes,
    };

    disclosure_slot_walk_count::reset();
    let derived =
        lifecycle().derive_disclosures(&case.before, &case.after, &case.body, &case.changes);
    assert!(derived.is_ok(), "{derived:?}");
    let walks = disclosure_slot_walk_count::count();
    assert!(
        walks <= 4,
        "one edit walked the read column's slots {walks} times for {ROWS} rows"
    );
}

/// Verbatim copy of the derivation this change replaced, kept as the
/// differential oracle.
mod reference {
    use super::super::*;

    #[allow(clippy::too_many_lines)] // Faithful copy of the replaced catalogue.
    pub(super) fn derive_disclosures(
        lifecycle: &PatchLifecycle,
        before: &Document,
        after: &Document,
        body: &SemanticPatchBody,
        changes: &[SemanticChange],
    ) -> Result<BTreeSet<DisclosureRequirement>, PatchLifecycleError> {
        let mut disclosures = BTreeSet::new();
        for command in body.commands() {
            insert_command_disclosures(lifecycle, before, after, command, &mut disclosures)?;
        }
        for change in changes {
            insert_change_disclosures(lifecycle, before, after, body, change, &mut disclosures)?;
        }
        Ok(disclosures)
    }

    #[allow(clippy::too_many_lines)] // Faithful copy of the replaced catalogue.
    fn insert_command_disclosures(
        lifecycle: &PatchLifecycle,
        before: &Document,
        after: &Document,
        command: &SemanticCommand,
        disclosures: &mut BTreeSet<DisclosureRequirement>,
    ) -> Result<(), PatchLifecycleError> {
        match command {
            SemanticCommand::AppendCollection { .. } | SemanticCommand::AppendEntity { .. } => {
                insert_document_disclosure(lifecycle, OperationFamily::AppendEntity, disclosures);
                Ok(())
            }
            SemanticCommand::RemoveCollection { .. } | SemanticCommand::RemoveEntity { .. } => {
                insert_document_disclosure(lifecycle, OperationFamily::RemoveEntity, disclosures);
                Ok(())
            }
            SemanticCommand::AppendSchemaField { .. }
            | SemanticCommand::RemoveSchemaField { .. }
            | SemanticCommand::RenameSchemaField { .. }
            | SemanticCommand::SetSchemaDisplayName { .. }
            | SemanticCommand::SetSchemaFieldDisplayName { .. }
            | SemanticCommand::SetFieldNumberFormat { .. }
            | SemanticCommand::SetSchemaFieldOrder { .. }
            | SemanticCommand::SetSchemaFieldRequiredness { .. } => {
                insert_document_disclosure(
                    lifecycle,
                    OperationFamily::SchemaFieldMutation,
                    disclosures,
                );
                Ok(())
            }
            SemanticCommand::SetFieldConstraint { schema, field, .. } => {
                disclosures.insert(DisclosureRequirement {
                    family: OperationFamily::SchemaFieldMutation,
                    scope: lifecycle.schema_field_scope(schema, field),
                });
                for document in [before, after] {
                    for entity in document
                        .entities
                        .values()
                        .filter(|entity| entity.schema == *schema)
                    {
                        if let Some(value) = entity.fields.get(field) {
                            let owner = FieldRef::new(entity.id.clone(), field.clone());
                            insert_value_disclosures_for(
                                lifecycle,
                                OperationFamily::SchemaFieldMutation,
                                before,
                                after,
                                &owner,
                                value,
                                disclosures,
                            )?;
                            if let Value::Formula(expression) = value {
                                insert_calculation_dependency_disclosures(
                                    lifecycle,
                                    OperationFamily::SchemaFieldMutation,
                                    document,
                                    expression_references(document, &owner, expression),
                                    disclosures,
                                )?;
                            }
                        }
                    }
                }
                Ok(())
            }
            SemanticCommand::SetFieldValue { field, value } => {
                insert_field_disclosure(lifecycle, before, after, field, disclosures)?;
                insert_value_disclosures(lifecycle, before, after, field, value, disclosures)
            }
            SemanticCommand::UnsetField { field } => {
                insert_field_disclosure_for(
                    lifecycle,
                    OperationFamily::SetFieldValue,
                    before,
                    after,
                    field,
                    disclosures,
                )?;
                if let Some(value) = before
                    .entities
                    .get(&field.entity)
                    .and_then(|entity| entity.fields.get(&field.field))
                {
                    insert_value_disclosures_for(
                        lifecycle,
                        OperationFamily::SetFieldValue,
                        before,
                        after,
                        field,
                        value,
                        disclosures,
                    )?;
                }
                Ok(())
            }
            SemanticCommand::FormulaUpdate(command) => {
                insert_field_disclosure_for(
                    lifecycle,
                    OperationFamily::FormulaUpdate,
                    before,
                    after,
                    command.target(),
                    disclosures,
                )?;
                for reference in command.references() {
                    insert_field_disclosure_for(
                        lifecycle,
                        OperationFamily::FormulaUpdate,
                        before,
                        after,
                        reference,
                        disclosures,
                    )?;
                }
                Ok(())
            }
            SemanticCommand::FormulaRestore(command) => {
                insert_field_disclosure_for(
                    lifecycle,
                    OperationFamily::FormulaUpdate,
                    before,
                    after,
                    command.target(),
                    disclosures,
                )?;
                for reference in
                    expression_references(after, command.target(), command.expression())
                {
                    insert_field_disclosure_for(
                        lifecycle,
                        OperationFamily::FormulaUpdate,
                        before,
                        after,
                        &reference,
                        disclosures,
                    )?;
                }
                Ok(())
            }
            SemanticCommand::FormulaInverseRestore(command) => {
                insert_field_disclosure_for(
                    lifecycle,
                    OperationFamily::FormulaInverseRestore,
                    before,
                    after,
                    command.target(),
                    disclosures,
                )?;
                insert_value_disclosures_for(
                    lifecycle,
                    OperationFamily::FormulaInverseRestore,
                    before,
                    after,
                    command.target(),
                    &Value::Number(command.value()),
                    disclosures,
                )
            }
            SemanticCommand::UpsertKeyedGroupedSumDefinition { .. }
            | SemanticCommand::RemoveKeyedGroupedSumDefinition { .. } => {
                disclosures.insert(DisclosureRequirement {
                    family: OperationFamily::KeyedGroupedSumDefinition,
                    scope: ScopedSemanticSubject::new(
                        lifecycle.document_scope.clone(),
                        lifecycle.document.clone(),
                        SemanticScope::Document,
                    ),
                });
                Ok(())
            }
        }
    }

    fn insert_document_disclosure(
        lifecycle: &PatchLifecycle,
        family: OperationFamily,
        disclosures: &mut BTreeSet<DisclosureRequirement>,
    ) {
        disclosures.insert(DisclosureRequirement {
            family,
            scope: ScopedSemanticSubject::new(
                lifecycle.document_scope.clone(),
                lifecycle.document.clone(),
                SemanticScope::Document,
            ),
        });
    }

    #[allow(clippy::too_many_lines)] // Faithful copy of the replaced catalogue.
    fn insert_change_disclosures(
        lifecycle: &PatchLifecycle,
        before: &Document,
        after: &Document,
        body: &SemanticPatchBody,
        change: &SemanticChange,
        disclosures: &mut BTreeSet<DisclosureRequirement>,
    ) -> Result<(), PatchLifecycleError> {
        match change {
            SemanticChange::SchemaAdded { .. }
            | SemanticChange::SchemaRemoved { .. }
            | SemanticChange::SchemaFieldAdded { .. }
            | SemanticChange::SchemaFieldRemoved { .. }
            | SemanticChange::EntityAdded { .. }
            | SemanticChange::EntityRemoved { .. } => Ok(()),
            SemanticChange::SchemaFieldChanged { schema, field, .. }
                if body.commands().iter().any(|command| matches!(command,
                    SemanticCommand::SetSchemaFieldRequiredness { schema: command_schema, field: command_field, .. }
                    if command_schema == schema && command_field == field
                )) => {
                insert_document_disclosure(
                    lifecycle,
                    OperationFamily::SchemaFieldMutation,
                    disclosures,
                );
                Ok(())
            }
            SemanticChange::SchemaFieldChanged { schema, field, .. }
                if body.commands().iter().any(|command| matches!(command,
                    SemanticCommand::RemoveSchemaField { schema: command_schema, field: definition, .. }
                    if command_schema == schema && definition.id == *field
                )) && body.commands().iter().any(|command| matches!(command,
                    SemanticCommand::AppendSchemaField { schema: command_schema, field: definition, .. }
                    if command_schema == schema && definition.id == *field
                )) => {
                insert_document_disclosure(
                    lifecycle,
                    OperationFamily::SchemaFieldMutation,
                    disclosures,
                );
                Ok(())
            }
            SemanticChange::FieldKeyChanged { schema, field, .. }
                if body.commands().iter().any(|command| matches!(command,
                    SemanticCommand::RenameSchemaField { schema: command_schema, field: command_field, .. }
                    if command_schema == schema && command_field == field
                )) => {
                insert_document_disclosure(
                    lifecycle,
                    OperationFamily::SchemaFieldMutation,
                    disclosures,
                );
                Ok(())
            }
            SemanticChange::FieldAdded { field, value } => {
                let families = command_families_for_field(body, field);
                if families == BTreeSet::from([OperationFamily::SchemaFieldMutation]) {
                    insert_document_disclosure(
                        lifecycle,
                        OperationFamily::SchemaFieldMutation,
                        disclosures,
                    );
                    return Ok(());
                }
                let definition = crate::field_definition(before, field)
                    .map_err(|_| PatchLifecycleError::ScopeDerivationFailed)?;
                let existing = before.entities[&field.entity].fields.get(&field.field);
                let formula_restore = definition.field_type == FieldType::Number
                    && !definition.required
                    && matches!(value, Value::Formula(_))
                    && body.commands().iter().any(|command| match command {
                        SemanticCommand::FormulaRestore(restore) => restore.target() == field,
                        SemanticCommand::FormulaUpdate(update) => update.target() == field,
                        _ => false,
                    });
                if existing.is_some()
                    || definition.required
                    || (matches!(value, Value::Formula(_)) && !formula_restore)
                    || (!formula_restore
                        && families != BTreeSet::from([OperationFamily::SetFieldValue]))
                {
                    return Err(PatchLifecycleError::ScopeDerivationFailed);
                }
                let family = if formula_restore {
                    OperationFamily::FormulaUpdate
                } else {
                    OperationFamily::SetFieldValue
                };
                insert_field_disclosure_for(lifecycle, family, before, after, field, disclosures)?;
                insert_value_disclosures_for(
                    lifecycle, family, before, after, field, value, disclosures,
                )
            }
            SemanticChange::FieldRemoved { field, value } => {
                let mut families = command_families_for_field(body, field);
                if families == BTreeSet::from([OperationFamily::SchemaFieldMutation]) {
                    insert_document_disclosure(
                        lifecycle,
                        OperationFamily::SchemaFieldMutation,
                        disclosures,
                    );
                    return Ok(());
                }
                if families.is_empty() {
                    families.insert(OperationFamily::SetFieldValue);
                }
                for family in families {
                    insert_field_disclosure_for(
                        lifecycle, family, before, after, field, disclosures,
                    )?;
                    insert_value_disclosures_for(
                        lifecycle, family, before, after, field, value, disclosures,
                    )?;
                }
                Ok(())
            }
            SemanticChange::FieldChanged {
                field,
                before: old_value,
                after: new_value,
            } => {
                let mut families = command_families_for_field(body, field);
                if families.is_empty() {
                    families.insert(OperationFamily::SetFieldValue);
                }
                for family in families {
                    insert_field_disclosure_for(
                        lifecycle, family, before, after, field, disclosures,
                    )?;
                    insert_value_disclosures_for(
                        lifecycle, family, before, after, field, old_value, disclosures,
                    )?;
                    insert_value_disclosures_for(
                        lifecycle, family, before, after, field, new_value, disclosures,
                    )?;
                }
                Ok(())
            }
            SemanticChange::FormulaImpact { field, causes, .. } => {
                let mut family_causes = BTreeMap::<OperationFamily, Vec<&FieldRef>>::new();
                for cause in causes {
                    let mut families = command_families_for_field(body, cause);
                    if families.is_empty() {
                        families.insert(OperationFamily::SetFieldValue);
                    }
                    for family in families {
                        family_causes.entry(family).or_default().push(cause);
                    }
                }
                for (family, causes) in family_causes {
                    insert_field_disclosure_for(
                        lifecycle, family, before, after, field, disclosures,
                    )?;
                    for cause in causes {
                        insert_field_disclosure_for(
                            lifecycle, family, before, after, cause, disclosures,
                        )?;
                    }
                    for document in [before, after] {
                        if let Some(Value::Formula(expression)) = document
                            .entities
                            .get(&field.entity)
                            .and_then(|entity| entity.fields.get(&field.field))
                        {
                            insert_calculation_dependency_disclosures(
                                lifecycle,
                                family,
                                document,
                                expression_references(document, field, expression),
                                disclosures,
                            )?;
                        }
                    }
                }
                Ok(())
            }
            _ => Err(PatchLifecycleError::ScopeDerivationFailed),
        }
    }

    fn insert_field_disclosure(
        lifecycle: &PatchLifecycle,
        before: &Document,
        after: &Document,
        field: &FieldRef,
        disclosures: &mut BTreeSet<DisclosureRequirement>,
    ) -> Result<(), PatchLifecycleError> {
        insert_field_disclosure_for(
            lifecycle,
            OperationFamily::SetFieldValue,
            before,
            after,
            field,
            disclosures,
        )
    }

    fn insert_field_disclosure_for(
        lifecycle: &PatchLifecycle,
        family: OperationFamily,
        before: &Document,
        after: &Document,
        field: &FieldRef,
        disclosures: &mut BTreeSet<DisclosureRequirement>,
    ) -> Result<(), PatchLifecycleError> {
        let scope = lifecycle
            .field_scope(after, field)
            .or_else(|_| lifecycle.field_scope(before, field))
            .map_err(|_| PatchLifecycleError::ScopeDerivationFailed)?;
        disclosures.insert(DisclosureRequirement { family, scope });
        Ok(())
    }

    fn insert_value_disclosures(
        lifecycle: &PatchLifecycle,
        before: &Document,
        after: &Document,
        owner: &FieldRef,
        value: &Value,
        disclosures: &mut BTreeSet<DisclosureRequirement>,
    ) -> Result<(), PatchLifecycleError> {
        insert_value_disclosures_for(
            lifecycle,
            OperationFamily::SetFieldValue,
            before,
            after,
            owner,
            value,
            disclosures,
        )
    }

    fn insert_value_disclosures_for(
        lifecycle: &PatchLifecycle,
        family: OperationFamily,
        before: &Document,
        after: &Document,
        owner: &FieldRef,
        value: &Value,
        disclosures: &mut BTreeSet<DisclosureRequirement>,
    ) -> Result<(), PatchLifecycleError> {
        match value {
            Value::Reference(entity) => {
                let scope = lifecycle
                    .entity_scope(after, entity)
                    .or_else(|_| lifecycle.entity_scope(before, entity))?;
                disclosures.insert(DisclosureRequirement { family, scope });
            }
            Value::Formula(expression) => {
                for field in expression_references(after, owner, expression) {
                    insert_field_disclosure_for(
                        lifecycle,
                        family,
                        before,
                        after,
                        &field,
                        disclosures,
                    )?;
                }
            }
            Value::Number(_) | Value::Text(_) | Value::Boolean(_) | Value::Date(_) => {}
        }
        Ok(())
    }

    fn insert_calculation_dependency_disclosures(
        lifecycle: &PatchLifecycle,
        family: OperationFamily,
        document: &Document,
        roots: impl IntoIterator<Item = FieldRef>,
        disclosures: &mut BTreeSet<DisclosureRequirement>,
    ) -> Result<(), PatchLifecycleError> {
        let mut pending = roots.into_iter().collect::<Vec<_>>();
        let mut visited = BTreeSet::new();
        while let Some(field) = pending.pop() {
            if !visited.insert(field.clone()) {
                continue;
            }
            insert_field_disclosure_for(
                lifecycle,
                family,
                document,
                document,
                &field,
                disclosures,
            )?;
            let entity = document
                .entities
                .get(&field.entity)
                .ok_or(PatchLifecycleError::ScopeDerivationFailed)?;
            let Some(value) = entity.fields.get(&field.field) else {
                let declared = document
                    .schemas
                    .get(&entity.schema)
                    .is_some_and(|schema| schema.fields.contains_key(&field.field));
                if declared {
                    continue;
                }
                return Err(PatchLifecycleError::ScopeDerivationFailed);
            };
            if let Value::Formula(expression) = value {
                pending.extend(expression_references(document, &field, expression));
            }
        }
        Ok(())
    }
}
