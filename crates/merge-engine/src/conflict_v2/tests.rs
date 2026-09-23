use std::collections::BTreeMap;

use tachiko_semantic_core::{
    Document, Entity, EntityId, FieldConstraint, FieldDefinition, FieldId, FieldType, Number,
    Schema, SchemaId, Value,
};

use super::*;

fn number(value: f64) -> Number {
    Number::new(value).unwrap()
}

fn number_field(id: &str) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        field_type: FieldType::Number,
        required: false,
        constraint: FieldConstraint::None,
    }
}

fn basic_document() -> Document {
    let schema = Schema {
        id: "schema".into(),
        key: "items".into(),
        fields: BTreeMap::from([("value".into(), number_field("value"))]),
    };
    let entity = Entity {
        id: EntityId::from("row"),
        key: "row".into(),
        schema: "schema".into(),
        fields: BTreeMap::from([("value".into(), Value::Number(number(1.0)))]),
    };
    Document {
        id: "doc".into(),
        title: "base".into(),
        schemas: BTreeMap::from([("schema".into(), schema)]),
        entities: BTreeMap::from([("row".into(), entity)]),
        keyed_grouped_sum_definitions: BTreeMap::new(),
    }
}

fn range(max: f64) -> FieldConstraint {
    FieldConstraint::NumberInclusiveRange {
        min: number(0.0),
        max: number(max),
    }
}

#[test]
fn inherited_v1_one_sided_merge_and_conflict_facts_project_to_v2() {
    let base = basic_document();
    let mut left = base.clone();
    left.title = "left".into();
    let mut right = base.clone();
    right
        .entities
        .get_mut("row")
        .unwrap()
        .fields
        .insert("value".into(), Value::Number(number(2.0)));
    let MergeOutcomeV2::Merged(candidate) = merge_v2(&base, &left, &right) else {
        panic!("independent one-sided facts should merge");
    };
    assert_eq!(candidate.title, "left");
    assert_eq!(
        candidate.entities["row"].fields["value"],
        Value::Number(number(2.0))
    );

    let mut other = base.clone();
    other
        .entities
        .get_mut("row")
        .unwrap()
        .fields
        .insert("value".into(), Value::Number(number(3.0)));
    let MergeOutcomeV2::Conflicted(conflicts) = merge_v2(&base, &right, &other) else {
        panic!("divergent stored values should conflict");
    };
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].facet(), ConflictFacetV2::StoredValue);
    assert_eq!(conflicts[0].kind(), ConflictKind::ConcurrentChange);
    assert_eq!(conflicts[0].contract(), SEMANTIC_CONFLICT_V2);
}

#[test]
fn constraints_merge_unchanged_equal_one_sided_and_concurrent_atomically() {
    let base = basic_document();
    let mut left = base.clone();
    left.schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("value")
        .unwrap()
        .constraint = range(10.0);
    let mut right = base.clone();
    right
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("value")
        .unwrap()
        .constraint = range(20.0);

    let MergeOutcomeV2::Merged(unchanged) = merge_v2(&base, &base, &base) else {
        panic!("unchanged state should merge");
    };
    assert_eq!(
        unchanged.schemas["schema"].fields["value"].constraint,
        FieldConstraint::None
    );
    for (ours, theirs) in [(&left, &base), (&base, &left), (&left, &left)] {
        let MergeOutcomeV2::Merged(candidate) = merge_v2(&base, ours, theirs) else {
            panic!("equal or one-sided constraint should merge");
        };
        assert_eq!(
            candidate.schemas["schema"].fields["value"].constraint,
            range(10.0)
        );
    }
    let MergeOutcomeV2::Conflicted(conflicts) = merge_v2(&base, &left, &right) else {
        panic!("different complete constraints should conflict");
    };
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].facet(), ConflictFacetV2::Constraint);
    assert_eq!(
        conflicts[0].base(),
        &ConflictFactV2::Present(MergeValueV2::Constraint(FieldConstraint::None))
    );
}

#[test]
fn schema_field_target_order_is_unicode_prefix_stable_across_permutations() {
    let ids = ["\u{10000}", "aa", "\u{e000}", "a"];
    let mut base = basic_document();
    let fields = ids
        .iter()
        .map(|id| ((*id).into(), number_field(id)))
        .collect::<BTreeMap<FieldId, _>>();
    base.schemas.get_mut("schema").unwrap().fields = fields;
    let make_side = |reversed: bool, maximum: f64| {
        let mut side = base.clone();
        let mut order = ids.to_vec();
        if reversed {
            order.reverse();
        }
        for id in order {
            side.schemas
                .get_mut("schema")
                .unwrap()
                .fields
                .get_mut(id)
                .unwrap()
                .constraint = range(maximum);
        }
        side
    };
    let left = make_side(true, 10.0);
    let right = make_side(false, 20.0);
    let expected = ["a", "aa", "\u{e000}", "\u{10000}"];
    for _ in 0..3 {
        let MergeOutcomeV2::Conflicted(conflicts) = merge_v2(&base, &left, &right) else {
            panic!("each divergent constraint should conflict");
        };
        let observed = conflicts
            .iter()
            .map(|conflict| match conflict.target() {
                ConflictTarget::SchemaField { field, .. } => field.as_str(),
                target => panic!("unexpected target: {target:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(observed, expected);
        assert_eq!(
            conflicts,
            match merge_v2(&base, &make_side(false, 10.0), &make_side(true, 20.0)) {
                MergeOutcomeV2::Conflicted(conflicts) => conflicts,
                _ => panic!("permuted inputs still conflict"),
            }
        );
    }
}

#[test]
fn schema_qualified_value_transition_preserves_unmaterialized_evidence() {
    let mut base = Document::empty("doc", "title");
    for id in ["old", "new"] {
        base.schemas.insert(
            id.into(),
            Schema {
                id: id.into(),
                key: id.into(),
                fields: BTreeMap::from([("f".into(), number_field("f"))]),
            },
        );
    }
    base.entities.insert(
        "entity".into(),
        Entity {
            id: "entity".into(),
            key: "entity".into(),
            schema: "old".into(),
            fields: BTreeMap::new(),
        },
    );
    let mut left = base.clone();
    left.entities
        .get_mut("entity")
        .unwrap()
        .fields
        .insert("f".into(), Value::Number(number(4.0)));
    let mut right = base.clone();
    right.entities.get_mut("entity").unwrap().schema = "new".into();

    let MergeOutcomeV2::Merged(candidate) = merge_v2(&base, &left, &right) else {
        panic!("qualified one-sided value plus schema change should reconcile");
    };
    assert_eq!(candidate.entities["entity"].schema, SchemaId::from("new"));
    assert!(candidate.entities["entity"].fields.is_empty());
    let (_, unmaterialized) = candidate.into_parts();
    assert_eq!(unmaterialized.len(), 1);
    assert_eq!(unmaterialized[0].entity(), &EntityId::from("entity"));
    assert_eq!(unmaterialized[0].source_schema(), &SchemaId::from("old"));
    assert_eq!(unmaterialized[0].selected_schema(), &SchemaId::from("new"));
}

#[test]
fn frozen_v1_still_refuses_constraint_bearing_documents() {
    let base = basic_document();
    let mut constrained = base.clone();
    constrained
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("value")
        .unwrap()
        .constraint = range(10.0);
    assert_eq!(
        merge(&constrained, &constrained, &constrained),
        MergeOutcome::UnsupportedFieldConstraintChange
    );
    assert!(matches!(
        merge_v2(&constrained, &constrained, &constrained),
        MergeOutcomeV2::Merged(_)
    ));
}
