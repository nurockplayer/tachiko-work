use std::collections::{BTreeMap, BTreeSet};

use tachiko_diff_engine::{DiffError, SemanticChange, SemanticDiff, diff, diff_with_calculations};
use tachiko_formula_engine::calculate;
use tachiko_semantic_core::{
    ColumnRef, Date, Document, DocumentId, Entity, EntityId, Expression, FieldDefinition, FieldId,
    FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

fn balance_document(damage: f64) -> Document {
    Document {
        id: DocumentId::from("balance"),
        title: "Balance".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("weapon"),
            Schema {
                field_order: None,
                id: SchemaId::from("weapon"),
                key: SchemaKey::from("weapon"),
                display_name: None,
                fields: BTreeMap::from([
                    (FieldId::from("damage"), number_field("damage", true)),
                    (
                        FieldId::from("attack_interval"),
                        number_field("attack_interval", true),
                    ),
                    (FieldId::from("dps"), number_field("dps", true)),
                    (FieldId::from("burst"), number_field("burst", true)),
                    (FieldId::from("rarity"), text_field("rarity", false)),
                    (FieldId::from("name"), text_field("name", true)),
                ]),
            },
        )]),
        entities: BTreeMap::from([(
            EntityId::from("sword"),
            Entity {
                id: EntityId::from("sword"),
                key: "sword".into(),
                schema: SchemaId::from("weapon"),
                fields: BTreeMap::from([
                    (FieldId::from("damage"), number(damage)),
                    (FieldId::from("attack_interval"), number(1.25)),
                    (
                        FieldId::from("dps"),
                        Value::Formula(Expression::Divide {
                            left: Box::new(reference("sword", "damage")),
                            right: Box::new(reference("sword", "attack_interval")),
                        }),
                    ),
                    (
                        FieldId::from("burst"),
                        Value::Formula(Expression::Multiply {
                            left: Box::new(reference("sword", "dps")),
                            right: Box::new(numeric(2.0)),
                        }),
                    ),
                    (FieldId::from("name"), Value::Text("Sword".to_owned())),
                ]),
            },
        )]),
        keyed_grouped_sum_definitions: BTreeMap::new(),
    }
}

fn field(id: &str, field_type: FieldType, required: bool) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(id),
        display_name: None,
        number_format: None,
        field_type,
        required,
        constraint: tachiko_semantic_core::FieldConstraint::None,
    }
}

fn number_field(id: &str, required: bool) -> FieldDefinition {
    field(id, FieldType::Number, required)
}

fn text_field(id: &str, required: bool) -> FieldDefinition {
    field(id, FieldType::Text, required)
}

fn boolean_field(id: &str, required: bool) -> FieldDefinition {
    field(id, FieldType::Boolean, required)
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn numeric(value: f64) -> Expression {
    Expression::Number(Number::new(value).unwrap())
}

fn reference(entity: &str, field: &str) -> Expression {
    Expression::Reference(FieldRef::new(entity, field))
}

fn date_document(value: &str) -> Document {
    let mut document = balance_document(100.0);
    document.schemas.get_mut("weapon").unwrap().fields.insert(
        FieldId::from("release_date"),
        field("release_date", FieldType::Date, false),
    );
    document.entities.get_mut("sword").unwrap().fields.insert(
        FieldId::from("release_date"),
        Value::Date(value.parse::<Date>().unwrap()),
    );
    document
}

#[test]
fn changed_balance_value_reports_direct_and_derived_meaning() {
    let before = balance_document(100.0);
    let after = balance_document(120.0);

    let semantic_diff = diff(&before, &after).unwrap();
    let rendered = semantic_diff.render_text();

    assert!(rendered.contains("Weapon Sword"));
    assert!(rendered.contains("damage: 100 -> 120"));
    assert!(rendered.contains("affected burst: 160 -> 192"));
    assert!(rendered.contains("affected dps: 80 -> 96"));
    assert!(semantic_diff.changes().iter().any(|change| matches!(
        change,
        SemanticChange::FieldChanged { field, before, after }
            if field == &FieldRef::new("sword", "damage")
                && before == &number(100.0)
                && after == &number(120.0)
    )));
    assert!(semantic_diff.changes().iter().any(|change| matches!(
        change,
        SemanticChange::FormulaImpact { field, causes, .. }
            if field == &FieldRef::new("sword", "dps")
                && causes == &vec![FieldRef::new("sword", "damage")]
    )));
}

#[test]
fn date_changes_use_semantic_equality_and_canonical_rendering() {
    let before = date_document("2024-02-29");
    let after = date_document("2025-01-01");

    let semantic_diff = diff(&before, &after).unwrap();

    assert!(
        semantic_diff
            .render_text()
            .contains("release_date: 2024-02-29 -> 2025-01-01")
    );
    assert!(semantic_diff.changes().iter().any(|change| matches!(
        change,
        SemanticChange::FieldChanged { field, before, after }
            if field == &FieldRef::new("sword", "release_date")
                && before == &Value::Date("2024-02-29".parse().unwrap())
                && after == &Value::Date("2025-01-01".parse().unwrap())
    )));
}

#[test]
fn formula_changes_render_in_canonical_copy_paste_syntax() {
    let before = balance_document(100.0);
    let mut after = before.clone();
    after.entities.get_mut("sword").unwrap().fields.insert(
        FieldId::from("dps"),
        Value::Formula(Expression::Add {
            left: Box::new(Expression::Divide {
                left: Box::new(reference("sword", "damage")),
                right: Box::new(reference("sword", "attack_interval")),
            }),
            right: Box::new(numeric(5.0)),
        }),
    );

    let rendered = diff(&before, &after).unwrap().render_text();

    assert!(rendered.contains(
        "dps: ([sword.damage] / [sword.attack_interval]) -> (([sword.damage] / [sword.attack_interval]) + 5)"
    ));
    assert!(rendered.contains("affected dps: 80 -> 85"));
}

#[test]
fn entity_addition_and_removal_are_semantic() {
    let before = balance_document(100.0);
    let mut after = before.clone();
    let mut axe = after.entities.get("sword").unwrap().clone();
    axe.id = EntityId::from("axe");
    axe.key = "axe".into();
    axe.fields
        .insert(FieldId::from("name"), Value::Text("Axe".to_owned()));
    axe.fields.insert(
        FieldId::from("dps"),
        Value::Formula(Expression::Divide {
            left: Box::new(reference("axe", "damage")),
            right: Box::new(reference("axe", "attack_interval")),
        }),
    );
    axe.fields.insert(
        FieldId::from("burst"),
        Value::Formula(Expression::Multiply {
            left: Box::new(reference("axe", "dps")),
            right: Box::new(numeric(2.0)),
        }),
    );
    after.entities.remove("sword");
    after.entities.insert(EntityId::from("axe"), axe);

    let rendered = diff(&before, &after).unwrap().render_text();

    assert_eq!(
        rendered,
        "Weapon Axe\nentity added\n\nWeapon Sword\nentity removed\n"
    );
}

#[test]
fn optional_field_addition_and_removal_are_ordered() {
    let before = balance_document(100.0);
    let mut after = before.clone();
    after
        .entities
        .get_mut("sword")
        .unwrap()
        .fields
        .insert(FieldId::from("rarity"), Value::Text("common".to_owned()));

    let added = diff(&before, &after).unwrap().render_text();
    let removed = diff(&after, &before).unwrap().render_text();

    assert!(added.contains("rarity added: \"common\""));
    assert!(removed.contains("rarity removed: \"common\""));
}

#[test]
fn document_identity_changes_are_typed_and_rendered() {
    let before = balance_document(100.0);
    let mut after = before.clone();
    after.id = DocumentId::from("rebalanced");
    after.title = "Rebalanced".to_owned();

    let semantic_diff = diff(&before, &after).unwrap();

    assert_eq!(
        semantic_diff.changes(),
        [
            SemanticChange::DocumentIdChanged {
                before: DocumentId::from("balance"),
                after: DocumentId::from("rebalanced"),
            },
            SemanticChange::DocumentTitleChanged {
                before: "Balance".to_owned(),
                after: "Rebalanced".to_owned(),
            },
        ]
    );
    assert_eq!(
        semantic_diff.render_text(),
        "Document\nid: balance -> rebalanced\ntitle: \"Balance\" -> \"Rebalanced\"\n"
    );
}

#[test]
fn schema_addition_and_removal_preserve_typed_definitions() {
    let before = balance_document(100.0);
    let mut after = before.clone();
    let armor = Schema {
        field_order: None,
        id: SchemaId::from("armor"),
        key: SchemaKey::from("armor"),
        display_name: None,
        fields: BTreeMap::from([(FieldId::from("defense"), number_field("defense", false))]),
    };
    after.schemas.insert(SchemaId::from("armor"), armor.clone());

    let added = diff(&before, &after).unwrap();
    let removed = diff(&after, &before).unwrap();

    assert_eq!(
        added.changes(),
        [SemanticChange::SchemaAdded {
            schema: SchemaId::from("armor"),
            definition: armor.clone(),
        }]
    );
    assert_eq!(added.render_text(), "Schema armor\nschema added\n");
    assert_eq!(
        removed.changes(),
        [SemanticChange::SchemaRemoved {
            schema: SchemaId::from("armor"),
            definition: armor,
        }]
    );
    assert_eq!(removed.render_text(), "Schema armor\nschema removed\n");
}

#[test]
fn schema_field_changes_preserve_typed_definitions_and_order() {
    let mut before = balance_document(100.0);
    before.schemas.insert(
        SchemaId::from("metadata"),
        Schema {
            field_order: None,
            id: SchemaId::from("metadata"),
            key: SchemaKey::from("metadata"),
            display_name: None,
            fields: BTreeMap::from([
                (FieldId::from("legacy"), text_field("legacy", false)),
                (FieldId::from("mode"), text_field("mode", false)),
            ]),
        },
    );
    let mut after = before.clone();
    let fields = &mut after.schemas.get_mut("metadata").unwrap().fields;
    fields.remove("legacy");
    fields.insert(FieldId::from("mode"), boolean_field("mode", false));
    fields.insert(FieldId::from("weight"), number_field("weight", false));

    let semantic_diff = diff(&before, &after).unwrap();

    assert_eq!(
        semantic_diff.changes(),
        [
            SemanticChange::SchemaFieldRemoved {
                schema: SchemaId::from("metadata"),
                field: FieldId::from("legacy"),
                definition: text_field("legacy", false),
            },
            SemanticChange::SchemaFieldChanged {
                schema: SchemaId::from("metadata"),
                field: FieldId::from("mode"),
                before: text_field("mode", false),
                after: boolean_field("mode", false),
            },
            SemanticChange::SchemaFieldAdded {
                schema: SchemaId::from("metadata"),
                field: FieldId::from("weight"),
                definition: number_field("weight", false),
            },
        ]
    );
    assert_eq!(
        semantic_diff.render_text(),
        "Schema metadata\nlegacy removed: text (optional)\nmode: text (optional) -> boolean (optional)\nweight added: number (optional)\n"
    );
}

fn diff_reusing_calculations(before: &Document, after: &Document) -> SemanticDiff {
    diff_with_calculations(
        before,
        after,
        &calculate(before).unwrap(),
        &calculate(after).unwrap(),
    )
    .unwrap()
}

#[test]
fn diff_with_calculations_matches_diff_for_value_formula_and_entity_changes() {
    let before = balance_document(10.0);

    let changed_value = balance_document(12.0);
    assert_eq!(
        diff_reusing_calculations(&before, &changed_value),
        diff(&before, &changed_value).unwrap()
    );

    let mut changed_formula = balance_document(10.0);
    changed_formula
        .entities
        .get_mut(&EntityId::from("sword"))
        .unwrap()
        .fields
        .insert(
            FieldId::from("burst"),
            Value::Formula(Expression::Multiply {
                left: Box::new(reference("sword", "dps")),
                right: Box::new(numeric(3.0)),
            }),
        );
    assert_eq!(
        diff_reusing_calculations(&before, &changed_formula),
        diff(&before, &changed_formula).unwrap()
    );

    let mut removed = balance_document(10.0);
    removed.entities.remove(&EntityId::from("sword"));
    assert_eq!(
        diff_reusing_calculations(&before, &removed),
        diff(&before, &removed).unwrap()
    );
}

#[test]
fn diff_with_calculations_still_refuses_unsupported_documents() {
    let before = balance_document(10.0);
    let mut named = balance_document(10.0);
    named
        .schemas
        .get_mut(&SchemaId::from("weapon"))
        .unwrap()
        .display_name = Some("Weapon".to_owned());

    let error = diff_with_calculations(
        &before,
        &named,
        &calculate(&before).unwrap(),
        &calculate(&named).unwrap(),
    )
    .unwrap_err();

    assert!(
        matches!(error, DiffError::UnsupportedDisplayName),
        "{error:?}"
    );
}

#[test]
fn unchanged_documents_have_an_explicit_empty_summary() {
    let document = balance_document(100.0);

    let semantic_diff = diff(&document, &document).unwrap();

    assert_eq!(semantic_diff.changes(), []);
    assert_eq!(semantic_diff.render_text(), "No semantic changes.\n");
}

/// Two edited inputs feed one shared formula.
fn shared_input_document(left: f64, right: f64) -> Document {
    Document {
        id: DocumentId::from("shared"),
        title: "Shared".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("pair"),
            Schema {
                field_order: None,
                id: SchemaId::from("pair"),
                key: SchemaKey::from("pair"),
                display_name: None,
                fields: BTreeMap::from([
                    (FieldId::from("left"), number_field("left", true)),
                    (FieldId::from("right"), number_field("right", true)),
                    (FieldId::from("total"), number_field("total", true)),
                ]),
            },
        )]),
        entities: BTreeMap::from([(
            EntityId::from("pair"),
            Entity {
                id: EntityId::from("pair"),
                key: "pair".into(),
                schema: SchemaId::from("pair"),
                fields: BTreeMap::from([
                    (FieldId::from("left"), number(left)),
                    (FieldId::from("right"), number(right)),
                    (
                        FieldId::from("total"),
                        Value::Formula(Expression::Add {
                            left: Box::new(reference("pair", "left")),
                            right: Box::new(reference("pair", "right")),
                        }),
                    ),
                ]),
            },
        )]),
        keyed_grouped_sum_definitions: BTreeMap::new(),
    }
}

/// A summary formula reads a whole plan column, so every edited row reaches it.
fn column_total_document(rent: f64, ads: f64) -> Document {
    let plan_row = |id: &str, reserve: f64| Entity {
        id: EntityId::from(id),
        key: id.into(),
        schema: SchemaId::from("plan"),
        fields: BTreeMap::from([(FieldId::from("reserve"), number(reserve))]),
    };
    Document {
        id: DocumentId::from("monthly-plan"),
        title: "Monthly plan".to_owned(),
        schemas: BTreeMap::from([
            (
                SchemaId::from("plan"),
                Schema {
                    field_order: None,
                    id: SchemaId::from("plan"),
                    key: SchemaKey::from("plan"),
                    display_name: None,
                    fields: BTreeMap::from([(
                        FieldId::from("reserve"),
                        number_field("reserve", true),
                    )]),
                },
            ),
            (
                SchemaId::from("summary"),
                Schema {
                    field_order: None,
                    id: SchemaId::from("summary"),
                    key: SchemaKey::from("summary"),
                    display_name: None,
                    fields: BTreeMap::from([(FieldId::from("total"), number_field("total", true))]),
                },
            ),
        ]),
        entities: BTreeMap::from([
            (EntityId::from("rent"), plan_row("rent", rent)),
            (EntityId::from("ads"), plan_row("ads", ads)),
            (
                EntityId::from("summary"),
                Entity {
                    id: EntityId::from("summary"),
                    key: "summary".into(),
                    schema: SchemaId::from("summary"),
                    fields: BTreeMap::from([(
                        FieldId::from("total"),
                        Value::Formula(Expression::ColumnTotal(ColumnRef::new("plan", "reserve"))),
                    )]),
                },
            ),
        ]),
        keyed_grouped_sum_definitions: BTreeMap::new(),
    }
}

/// The pre-#678 cause calculation: rescan both dependency maps per changed
/// field with `affected_by`.
fn causes_by_affected_by(
    before: &Document,
    after: &Document,
    changed_fields: &BTreeSet<FieldRef>,
    formula: &FieldRef,
) -> Vec<FieldRef> {
    let before_calculation = calculate(before).unwrap();
    let after_calculation = calculate(after).unwrap();
    changed_fields
        .iter()
        .filter(|changed| {
            *changed == formula
                || before_calculation.affected_by(changed).contains(formula)
                || after_calculation.affected_by(changed).contains(formula)
        })
        .cloned()
        .collect()
}

fn assert_formula_impact_causes(
    before: &Document,
    after: &Document,
    changed_fields: &BTreeSet<FieldRef>,
) {
    let before_calculation = calculate(before).unwrap();
    let after_calculation = calculate(after).unwrap();
    for semantic_diff in [
        diff(before, after).unwrap(),
        diff_with_calculations(before, after, &before_calculation, &after_calculation).unwrap(),
    ] {
        for change in semantic_diff.changes() {
            let SemanticChange::FormulaImpact { field, causes, .. } = change else {
                continue;
            };
            assert_eq!(
                causes,
                &causes_by_affected_by(before, after, changed_fields, field),
                "{field:?}"
            );
        }
    }
}

#[test]
fn formula_impact_causes_match_affected_by_for_shared_inputs() {
    let before = shared_input_document(1.0, 2.0);
    let after = shared_input_document(3.0, 5.0);
    let changed_fields = BTreeSet::from([
        FieldRef::new("pair", "left"),
        FieldRef::new("pair", "right"),
    ]);

    assert_formula_impact_causes(&before, &after, &changed_fields);
    assert!(
        diff(&before, &after)
            .unwrap()
            .changes()
            .iter()
            .any(|change| {
                matches!(
                    change,
                    SemanticChange::FormulaImpact { field, causes, .. }
                        if field == &FieldRef::new("pair", "total")
                            && causes == &vec![
                                FieldRef::new("pair", "left"),
                                FieldRef::new("pair", "right"),
                            ]
                )
            })
    );
}

#[test]
fn formula_impact_causes_match_affected_by_for_a_chain() {
    let before = balance_document(100.0);
    let after = balance_document(120.0);
    let changed_fields = BTreeSet::from([FieldRef::new("sword", "damage")]);

    assert_formula_impact_causes(&before, &after, &changed_fields);
}

#[test]
fn formula_impact_causes_match_affected_by_for_a_changed_formula() {
    let before = balance_document(100.0);
    let mut after = balance_document(100.0);
    after.entities.get_mut("sword").unwrap().fields.insert(
        FieldId::from("dps"),
        Value::Formula(Expression::Multiply {
            left: Box::new(reference("sword", "damage")),
            right: Box::new(numeric(2.0)),
        }),
    );
    let changed_fields = BTreeSet::from([FieldRef::new("sword", "dps")]);

    assert_formula_impact_causes(&before, &after, &changed_fields);
}

#[test]
fn formula_impact_causes_match_affected_by_for_a_column_total() {
    let before = column_total_document(12_000.0, 500.0);
    let after = column_total_document(15_000.0, 700.0);
    let changed_fields = BTreeSet::from([
        FieldRef::new("rent", "reserve"),
        FieldRef::new("ads", "reserve"),
    ]);

    assert_formula_impact_causes(&before, &after, &changed_fields);
}

#[test]
fn an_edit_reaching_no_formula_reports_no_formula_impact() {
    let before = balance_document(100.0);
    let mut after = balance_document(100.0);
    after
        .entities
        .get_mut("sword")
        .unwrap()
        .fields
        .insert(FieldId::from("name"), Value::Text("Axe".to_owned()));
    let before_calculation = calculate(&before).unwrap();
    let after_calculation = calculate(&after).unwrap();

    for semantic_diff in [
        diff(&before, &after).unwrap(),
        diff_with_calculations(&before, &after, &before_calculation, &after_calculation).unwrap(),
    ] {
        assert!(
            semantic_diff
                .changes()
                .iter()
                .all(|change| !matches!(change, SemanticChange::FormulaImpact { .. }))
        );
        assert!(semantic_diff.changes().iter().any(|change| matches!(
            change,
            SemanticChange::FieldChanged { field, .. }
                if field == &FieldRef::new("sword", "name")
        )));
    }
}

#[test]
fn a_scope_naming_ids_neither_document_has_reports_nothing() {
    let document = balance_document(4.0);
    let calculation = calculate(&document).expect("document calculates");
    let scope = tachiko_diff_engine::DiffScope {
        schemas: BTreeSet::from([tachiko_semantic_core::SchemaId::from("absent")]),
        entities: BTreeSet::from([tachiko_semantic_core::EntityId::from("absent")]),
    };
    assert_eq!(
        tachiko_diff_engine::diff_changes_scoped(
            &document,
            &document,
            &calculation,
            &calculation,
            &scope,
        ),
        Vec::<SemanticChange>::new()
    );
}
