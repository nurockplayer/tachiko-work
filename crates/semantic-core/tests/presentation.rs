use std::collections::BTreeMap;

use tachiko_semantic_core::*;

fn number(value: f64) -> Number {
    Number::new(value).unwrap()
}

fn field(id: &str, field_type: FieldType) -> (FieldId, FieldDefinition) {
    (
        FieldId::from(id),
        FieldDefinition {
            id: FieldId::from(id),
            key: id.into(),
            display_name: None,
            field_type,
            required: false,
            constraint: FieldConstraint::None,
            number_format: None,
        },
    )
}

fn document() -> Document {
    let schema = Schema {
        field_order: None,
        id: "orders".into(),
        key: "orders".into(),
        display_name: None,
        fields: BTreeMap::from([
            field("amount", FieldType::Number),
            field("due", FieldType::Date),
        ]),
    };
    let mut document = Document::empty("d", "Doc");
    document.schemas.insert(schema.id.clone(), schema);
    document.entities.insert(
        "e1".into(),
        Entity {
            id: "e1".into(),
            key: "e1".into(),
            schema: "orders".into(),
            fields: BTreeMap::new(),
        },
    );
    document
}

fn full_style() -> CellStyle {
    CellStyle {
        bold: Some(true),
        italic: Some(false),
        fill: Some("#FFEE00".into()),
        text_color: Some("#112233".into()),
        alignment: Some(CellAlignment::Center),
        wrap: Some(true),
        borders: Some(vec![BorderSide::Top, BorderSide::Bottom]),
        number_format: Some(NumberFormat {
            decimals: 2,
            kind: NumberFormatKind::Plain,
            thousands_separator: true,
        }),
        date_format: Some(DateFormat::YmdCjk),
    }
}

fn rule(condition: Expression) -> ConditionalRule {
    ConditionalRule {
        schema: "orders".into(),
        rank: "a1".into(),
        fields: vec!["amount".into()],
        condition,
        style: "s2".into(),
    }
}

fn valid_condition() -> Expression {
    Expression::Compare {
        operator: ComparisonOperator::Less,
        left: Box::new(Expression::SameRowReference("amount".into())),
        right: Box::new(Expression::Number(number(0.0))),
    }
}

fn valid_store() -> PresentationStore {
    PresentationStore {
        styles: BTreeMap::from([
            ("s1".into(), full_style()),
            ("s2".into(), CellStyle::default()),
        ]),
        tables: BTreeMap::from([(
            "orders".into(),
            TablePresentation {
                title: Some("Q3 budget".into()),
                footer: BTreeMap::from([("amount".into(), FooterAggregate::Sum)]),
                column_styles: BTreeMap::from([("due".into(), StyleId::from("s1"))]),
            },
        )]),
        rules: BTreeMap::from([("r1".into(), rule(valid_condition()))]),
        cells: BTreeMap::from([(("e1".into(), "due".into()), StyleId::from("s1"))]),
    }
}

#[test]
fn a_valid_store_passes() {
    assert_eq!(validate_presentation(&valid_store(), &document()), Ok(()));
}

#[test]
fn a_same_row_reference_condition_passes() {
    let document = document();
    let store = valid_store();
    assert_eq!(validate_presentation(&store, &document), Ok(()));
}

#[test]
fn an_empty_store_passes() {
    assert_eq!(
        validate_presentation(&PresentationStore::default(), &document()),
        Ok(())
    );
}

#[test]
fn a_cell_naming_a_missing_entity_is_refused() {
    let mut store = valid_store();
    store
        .cells
        .insert(("missing".into(), "due".into()), StyleId::from("s1"));
    assert_eq!(
        validate_presentation(&store, &document()),
        Err(PresentationError::MissingEntity {
            entity: "missing".into(),
        })
    );
}

#[test]
fn a_cell_field_not_on_its_entity_schema_is_refused() {
    let mut store = valid_store();
    store
        .cells
        .insert(("e1".into(), "missing".into()), StyleId::from("s1"));
    assert_eq!(
        validate_presentation(&store, &document()),
        Err(PresentationError::CellFieldNotOnEntitySchema {
            entity: "e1".into(),
            field: "missing".into(),
        })
    );
}

#[test]
fn a_record_naming_a_missing_schema_is_refused() {
    let mut store = valid_store();
    store.tables.clear();
    store.tables.insert(
        "missing".into(),
        TablePresentation {
            title: Some("Title".into()),
            footer: BTreeMap::new(),
            column_styles: BTreeMap::new(),
        },
    );
    assert_eq!(
        validate_presentation(&store, &document()),
        Err(PresentationError::MissingSchema {
            schema: "missing".into(),
        })
    );

    let mut store = valid_store();
    let rule_id = RuleId::from("r1");
    store.rules.get_mut(&rule_id).unwrap().schema = "missing".into();
    assert_eq!(
        validate_presentation(&store, &document()),
        Err(PresentationError::MissingSchema {
            schema: "missing".into(),
        })
    );
}

#[test]
fn a_rule_painting_a_missing_field_is_refused() {
    let mut store = valid_store();
    store.rules.get_mut(&RuleId::from("r1")).unwrap().fields = vec!["missing".into()];
    assert_eq!(
        validate_presentation(&store, &document()),
        Err(PresentationError::MissingField {
            schema: "orders".into(),
            field: "missing".into(),
        })
    );
}

#[test]
fn a_rule_reading_a_missing_field_is_refused() {
    let mut store = valid_store();
    store.rules.get_mut(&RuleId::from("r1")).unwrap().condition = Expression::Compare {
        operator: ComparisonOperator::Less,
        left: Box::new(Expression::SameRowReference("missing".into())),
        right: Box::new(Expression::Number(number(0.0))),
    };
    assert_eq!(
        validate_presentation(&store, &document()),
        Err(PresentationError::MissingField {
            schema: "orders".into(),
            field: "missing".into(),
        })
    );
}

#[test]
fn a_rule_reading_a_missing_column_is_refused() {
    let mut store = valid_store();
    store.rules.get_mut(&RuleId::from("r1")).unwrap().condition = Expression::ColumnTotal(
        ColumnRef::new(SchemaId::from("orders"), FieldId::from("missing")),
    );
    assert_eq!(
        validate_presentation(&store, &document()),
        Err(PresentationError::MissingField {
            schema: "orders".into(),
            field: "missing".into(),
        })
    );
}

#[test]
fn a_style_used_by_nothing_is_refused() {
    let mut store = valid_store();
    store.styles.insert("unused".into(), CellStyle::default());
    assert_eq!(
        validate_presentation(&store, &document()),
        Err(PresentationError::UnusedStyle {
            style: "unused".into(),
        })
    );
}

#[test]
fn a_reference_to_a_missing_style_is_refused() {
    let mut store = valid_store();
    store
        .tables
        .get_mut(&SchemaId::from("orders"))
        .unwrap()
        .column_styles
        .insert("due".into(), "missing".into());
    assert_eq!(
        validate_presentation(&store, &document()),
        Err(PresentationError::MissingStyle {
            style: "missing".into(),
        })
    );

    let mut store = valid_store();
    store.rules.get_mut(&RuleId::from("r1")).unwrap().style = "missing".into();
    assert_eq!(
        validate_presentation(&store, &document()),
        Err(PresentationError::MissingStyle {
            style: "missing".into(),
        })
    );
}

#[test]
fn a_rule_condition_holding_a_reference_is_refused() {
    let mut store = valid_store();
    store.rules.get_mut(&RuleId::from("r1")).unwrap().condition = Expression::Compare {
        operator: ComparisonOperator::Less,
        left: Box::new(Expression::Reference(FieldRef::new(
            EntityId::from("e1"),
            FieldId::from("amount"),
        ))),
        right: Box::new(Expression::Number(number(0.0))),
    };
    assert_eq!(
        validate_presentation(&store, &document()),
        Err(PresentationError::RuleConditionReference { rule: "r1".into() })
    );
}

#[test]
fn a_title_failing_the_spelling_rules_is_refused() {
    for title in ["", " padded ", "\u{0007}bell"] {
        let mut store = valid_store();
        store
            .tables
            .get_mut(&SchemaId::from("orders"))
            .unwrap()
            .title = Some(title.to_owned());
        assert!(matches!(
            validate_presentation(&store, &document()),
            Err(PresentationError::InvalidTitle { .. })
        ));
    }
}

#[test]
fn a_rank_outside_the_grammar_is_refused() {
    let too_long = "a".repeat(65);
    for rank in ["", "a0", "a b", "-1", too_long.as_str()] {
        let mut store = valid_store();
        store.rules.get_mut(&RuleId::from("r1")).unwrap().rank = rank.to_owned();
        assert_eq!(
            validate_presentation(&store, &document()),
            Err(PresentationError::InvalidRank {
                rule: "r1".into(),
                rank: rank.to_owned(),
            })
        );
    }
}

#[test]
fn a_number_format_failing_the_r3_check_is_refused() {
    let mut store = valid_store();
    store.styles.insert(
        "s1".into(),
        CellStyle {
            number_format: Some(NumberFormat {
                decimals: 11,
                kind: NumberFormatKind::Plain,
                thousands_separator: false,
            }),
            ..full_style()
        },
    );
    assert_eq!(
        validate_presentation(&store, &document()),
        Err(PresentationError::InvalidNumberFormat {
            style: "s1".into(),
            error: NumberFormatError::DecimalsOutOfRange,
        })
    );
}

#[test]
fn a_color_without_canonical_spelling_is_refused() {
    let mut store = valid_store();
    store.styles.insert(
        "s1".into(),
        CellStyle {
            fill: Some("#aabbcc".into()),
            ..full_style()
        },
    );
    assert_eq!(
        validate_presentation(&store, &document()),
        Err(PresentationError::InvalidColor {
            style: "s1".into(),
            member: "fill",
            value: "#aabbcc".into(),
        })
    );

    let mut store = valid_store();
    store.styles.insert(
        "s1".into(),
        CellStyle {
            text_color: Some("112233".into()),
            ..full_style()
        },
    );
    assert_eq!(
        validate_presentation(&store, &document()),
        Err(PresentationError::InvalidColor {
            style: "s1".into(),
            member: "text_color",
            value: "112233".into(),
        })
    );
}

#[test]
fn duplicate_or_out_of_order_borders_are_refused() {
    let mut store = valid_store();
    store.styles.insert(
        "s1".into(),
        CellStyle {
            borders: Some(vec![BorderSide::Top, BorderSide::Top]),
            ..full_style()
        },
    );
    assert_eq!(
        validate_presentation(&store, &document()),
        Err(PresentationError::DuplicateBorderSide {
            style: "s1".into(),
            side: BorderSide::Top,
        })
    );

    let mut store = valid_store();
    store.styles.insert(
        "s1".into(),
        CellStyle {
            borders: Some(vec![BorderSide::Bottom, BorderSide::Top]),
            ..full_style()
        },
    );
    assert_eq!(
        validate_presentation(&store, &document()),
        Err(PresentationError::BorderSidesOutOfOrder { style: "s1".into() })
    );
}

#[test]
fn an_empty_rule_field_list_is_refused() {
    let mut store = valid_store();
    store
        .rules
        .get_mut(&RuleId::from("r1"))
        .unwrap()
        .fields
        .clear();
    assert_eq!(
        validate_presentation(&store, &document()),
        Err(PresentationError::EmptyRuleFields { rule: "r1".into() })
    );
}

#[test]
fn a_style_bound_admits_4096_and_refuses_4097() {
    let (document, store) = bounded_store(MAX_STYLES);
    assert_eq!(validate_presentation(&store, &document), Ok(()));
    let (document, store) = bounded_store(MAX_STYLES + 1);
    assert_eq!(
        validate_presentation(&store, &document),
        Err(PresentationError::TooManyStyles {
            found: 4097,
            limit: MAX_STYLES,
        })
    );
}

#[test]
fn a_rule_bound_admits_64_per_table_and_refuses_65() {
    let (document, store) = rules_store(MAX_RULES_PER_TABLE);
    assert_eq!(validate_presentation(&store, &document), Ok(()));
    let (document, store) = rules_store(MAX_RULES_PER_TABLE + 1);
    assert_eq!(
        validate_presentation(&store, &document),
        Err(PresentationError::TooManyRules {
            schema: "orders".into(),
            found: 65,
            limit: MAX_RULES_PER_TABLE,
        })
    );
}

fn bounded_store(count: usize) -> (Document, PresentationStore) {
    let fields = (0..count)
        .map(|index| field(&format!("f{index:04}"), FieldType::Number))
        .collect();
    let schema = Schema {
        field_order: None,
        id: "orders".into(),
        key: "orders".into(),
        display_name: None,
        fields,
    };
    let mut document = Document::empty("d", "Bounds");
    document.schemas.insert(schema.id.clone(), schema);
    document.entities.insert(
        "e1".into(),
        Entity {
            id: "e1".into(),
            key: "e1".into(),
            schema: "orders".into(),
            fields: BTreeMap::new(),
        },
    );
    let mut store = PresentationStore::default();
    for index in 0..count {
        let style_id = StyleId::from(format!("s{index:04}"));
        store.styles.insert(
            style_id.clone(),
            CellStyle {
                fill: Some(format!("#{index:06X}")),
                ..CellStyle::default()
            },
        );
        store.cells.insert(
            (EntityId::from("e1"), FieldId::from(format!("f{index:04}"))),
            style_id,
        );
    }
    (document, store)
}

fn rules_store(count: usize) -> (Document, PresentationStore) {
    let mut document = Document::empty("d", "Rules");
    let schema = Schema {
        field_order: None,
        id: "orders".into(),
        key: "orders".into(),
        display_name: None,
        fields: BTreeMap::from([field("amount", FieldType::Number)]),
    };
    document.schemas.insert(schema.id.clone(), schema);
    let mut store = PresentationStore {
        styles: BTreeMap::from([("s".into(), CellStyle::default())]),
        ..PresentationStore::default()
    };
    for index in 0..count {
        store.rules.insert(
            RuleId::from(format!("r{index:03}")),
            ConditionalRule {
                schema: "orders".into(),
                rank: format!("a{index}x"),
                fields: vec!["amount".into()],
                condition: valid_condition(),
                style: "s".into(),
            },
        );
    }
    (document, store)
}

fn schema(id: &str, fields: &[(&str, FieldType)]) -> Schema {
    Schema {
        field_order: None,
        id: id.into(),
        key: id.into(),
        display_name: None,
        fields: fields
            .iter()
            .map(|(name, field_type)| field(name, field_type.clone()))
            .collect(),
    }
}

#[test]
fn a_store_with_nothing_to_drop_is_returned_unchanged() {
    let store = valid_store();
    assert_eq!(prune_presentation(&store, &document()), store);
}

#[test]
fn a_cell_for_a_missing_entity_or_field_is_dropped() {
    let mut store = valid_store();
    store
        .cells
        .insert(("gone".into(), "due".into()), StyleId::from("s1"));
    assert_eq!(prune_presentation(&store, &document()), valid_store());

    let mut store = valid_store();
    store
        .cells
        .insert(("e1".into(), "gone".into()), StyleId::from("s1"));
    assert_eq!(prune_presentation(&store, &document()), valid_store());
}

#[test]
fn a_cell_off_the_entitys_schema_is_dropped() {
    let mut document = document();
    document.schemas.insert(
        "other".into(),
        schema("other", &[("other_field", FieldType::Number)]),
    );
    document.entities.get_mut(&"e1".into()).unwrap().schema = "other".into();
    let pruned = prune_presentation(&valid_store(), &document);
    assert!(pruned.cells.is_empty());
    assert_eq!(validate_presentation(&pruned, &document), Ok(()));
}

#[test]
fn a_table_for_a_missing_schema_is_dropped() {
    let mut document = document();
    document.schemas.insert(
        "legacy".into(),
        schema("legacy", &[("x", FieldType::Number)]),
    );
    let mut store = valid_store();
    store.tables.insert(
        "legacy".into(),
        TablePresentation {
            title: Some("Legacy".into()),
            ..TablePresentation::default()
        },
    );
    let pruned = prune_presentation(&store, &document);
    assert_eq!(pruned, store);

    document.schemas.remove(&SchemaId::from("legacy"));
    let pruned = prune_presentation(&store, &document);
    assert!(!pruned.tables.contains_key(&"legacy".into()));
    assert_eq!(
        pruned.tables.get(&"orders".into()),
        store.tables.get(&"orders".into())
    );
    assert_eq!(validate_presentation(&pruned, &document), Ok(()));
}

#[test]
fn a_footer_or_column_style_on_a_missing_field_is_dropped() {
    let mut document = document();
    document
        .schemas
        .get_mut(&"orders".into())
        .unwrap()
        .fields
        .remove(&"amount".into());
    let pruned = prune_presentation(&valid_store(), &document);
    let orders = pruned.tables.get(&"orders".into()).unwrap();
    assert!(!orders.footer.contains_key(&"amount".into()));
    assert_eq!(orders.column_styles.get(&"due".into()), Some(&"s1".into()));
    assert!(!pruned.rules.contains_key(&"r1".into()));
    assert!(!pruned.styles.contains_key(&"s2".into()));
    assert_eq!(validate_presentation(&pruned, &document), Ok(()));

    let mut document = document();
    document
        .schemas
        .get_mut(&"orders".into())
        .unwrap()
        .fields
        .remove(&"due".into());
    let pruned = prune_presentation(&valid_store(), &document);
    let orders = pruned.tables.get(&"orders".into()).unwrap();
    assert!(orders.column_styles.is_empty());
    assert!(pruned.cells.is_empty());
    assert!(!pruned.styles.contains_key(&"s1".into()));
    assert_eq!(validate_presentation(&pruned, &document), Ok(()));
}

#[test]
fn a_table_left_with_no_content_is_dropped() {
    let mut store = PresentationStore {
        styles: BTreeMap::from([("s3".into(), CellStyle::default())]),
        ..PresentationStore::default()
    };
    store.tables.insert(
        "orders".into(),
        TablePresentation {
            title: None,
            footer: BTreeMap::from([("amount".into(), FooterAggregate::Sum)]),
            column_styles: BTreeMap::from([("amount".into(), StyleId::from("s3"))]),
        },
    );
    let mut document = document();
    assert_eq!(validate_presentation(&store, &document), Ok(()));
    document
        .schemas
        .get_mut(&"orders".into())
        .unwrap()
        .fields
        .remove(&"amount".into());
    let pruned = prune_presentation(&store, &document);
    assert!(pruned.is_empty());
    assert_eq!(validate_presentation(&pruned, &document), Ok(()));
}

#[test]
fn a_rule_for_a_missing_schema_is_dropped() {
    let mut document = document();
    document
        .schemas
        .insert("other".into(), schema("other", &[("x", FieldType::Number)]));
    let mut store = valid_store();
    store.rules.insert(
        "r2".into(),
        ConditionalRule {
            schema: "other".into(),
            rank: "a1".into(),
            fields: vec!["x".into()],
            condition: Expression::IsBlank(Box::new(Expression::SameRowReference("x".into()))),
            style: "s1".into(),
        },
    );
    assert_eq!(prune_presentation(&store, &document), store);

    document.schemas.remove(&SchemaId::from("other"));
    let pruned = prune_presentation(&store, &document);
    assert!(!pruned.rules.contains_key(&"r2".into()));
    assert_eq!(validate_presentation(&pruned, &document), Ok(()));
}

#[test]
fn a_rule_painting_a_missing_field_is_dropped() {
    let mut store = valid_store();
    store.rules.insert(
        "r2".into(),
        ConditionalRule {
            schema: "orders".into(),
            rank: "a2".into(),
            fields: vec!["amount".into(), "due".into()],
            condition: Expression::IsBlank(Box::new(Expression::SameRowReference("due".into()))),
            style: "s2".into(),
        },
    );
    let mut document = document();
    assert_eq!(validate_presentation(&store, &document), Ok(()));
    document
        .schemas
        .get_mut(&"orders".into())
        .unwrap()
        .fields
        .remove(&"amount".into());
    let pruned = prune_presentation(&store, &document);
    assert!(!pruned.rules.contains_key(&"r2".into()));
    assert!(!pruned.rules.contains_key(&"r1".into()));
    assert_eq!(validate_presentation(&pruned, &document), Ok(()));
}

#[test]
fn a_rule_reading_a_missing_field_is_dropped() {
    let mut store = valid_store();
    store.rules.insert(
        "r2".into(),
        ConditionalRule {
            schema: "orders".into(),
            rank: "a2".into(),
            fields: vec!["due".into()],
            condition: Expression::IsBlank(Box::new(Expression::SameRowReference("due".into()))),
            style: "s2".into(),
        },
    );
    store.rules.insert(
        "r3".into(),
        ConditionalRule {
            schema: "orders".into(),
            rank: "a3".into(),
            fields: vec!["due".into()],
            condition: Expression::ColumnTotal(ColumnRef::new("orders", "amount")),
            style: "s2".into(),
        },
    );
    let mut document = document();
    assert_eq!(validate_presentation(&store, &document), Ok(()));
    document
        .schemas
        .get_mut(&"orders".into())
        .unwrap()
        .fields
        .remove(&"amount".into());
    let pruned = prune_presentation(&store, &document);
    assert!(!pruned.rules.contains_key(&"r1".into()));
    assert!(pruned.rules.contains_key(&"r2".into()));
    assert!(!pruned.rules.contains_key(&"r3".into()));
    assert_eq!(validate_presentation(&pruned, &document), Ok(()));
}

#[test]
fn a_style_nothing_uses_is_dropped() {
    let mut store = valid_store();
    store
        .tables
        .get_mut(&"orders".into())
        .unwrap()
        .column_styles
        .clear();
    store.cells.clear();
    store
        .cells
        .insert(("e1".into(), "due".into()), StyleId::from("s1"));
    let mut document = document();
    assert_eq!(validate_presentation(&store, &document), Ok(()));
    document.entities.clear();
    let pruned = prune_presentation(&store, &document);
    assert!(pruned.cells.is_empty());
    assert!(!pruned.styles.contains_key(&"s1".into()));
    assert_eq!(validate_presentation(&pruned, &document), Ok(()));
}
