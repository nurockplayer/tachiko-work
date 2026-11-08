//! `prepare_evaluation` derives a candidate from retained state instead of
//! calculating the whole document, and must still produce exactly the report
//! and calculation full semantic validation produces (#678).

use std::collections::BTreeMap;

use super::{
    ResidentProjectionInvalidation, ResidentWorkspaceSession, TrustedPublicationTimeSource,
    installation_counters,
};
use crate::patch_lifecycle::{
    SemanticPublicationAuthority, SemanticPublicationError, SemanticRevision, TrustedInstant,
};
use crate::{
    Document, Entity, EntityId, EntityKey, Expression, FieldConstraint, FieldDefinition, FieldId,
    FieldRef, FieldType, Number, PreparedInstallation, Schema, SchemaId, Value,
};

fn optional_number(id: &str) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        display_name: None,
        number_format: None,
        field_type: FieldType::Number,
        required: false,
        constraint: FieldConstraint::None,
    }
}

fn optional_text(id: &str) -> FieldDefinition {
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
    Value::Number(Number::new(value).expect("test literals are finite"))
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

fn reference(entity: &str, field: &str) -> Expression {
    Expression::Reference(FieldRef::new(entity, field))
}

fn same_row(field: &str) -> Expression {
    Expression::SameRowReference(FieldId::from(field))
}

fn literal(value: f64) -> Expression {
    Expression::Number(Number::new(value).expect("test literals are finite"))
}

fn row(id: &str, schema: &str, fields: BTreeMap<FieldId, Value>) -> Entity {
    Entity {
        id: EntityId::from(id),
        key: EntityKey::from(id),
        schema: SchemaId::from(schema),
        fields,
    }
}

/// `f` is Text, `g` a Number and `h` a formula reading `g`, on two rows.
fn base_document() -> Document {
    let mut document = Document::empty("doc", "Sheet");
    document.schemas.insert(
        SchemaId::from("s"),
        Schema {
            field_order: None,
            id: "s".into(),
            key: "table".into(),
            display_name: None,
            fields: BTreeMap::from([
                ("f".into(), optional_text("f")),
                ("g".into(), optional_number("g")),
                ("h".into(), optional_number("h")),
            ]),
        },
    );
    document.entities.insert(
        EntityId::from("e1"),
        row(
            "e1",
            "s",
            BTreeMap::from([
                ("f".into(), Value::Text("old".into())),
                ("g".into(), number(1.0)),
                (
                    "h".into(),
                    Value::Formula(add(reference("e1", "g"), literal(1.0))),
                ),
            ]),
        ),
    );
    document.entities.insert(
        EntityId::from("e2"),
        row(
            "e2",
            "s",
            BTreeMap::from([
                ("f".into(), Value::Text("two".into())),
                ("g".into(), number(2.0)),
                (
                    "h".into(),
                    Value::Formula(multiply(reference("e2", "g"), literal(2.0))),
                ),
            ]),
        ),
    );
    document
}

fn set(document: &mut Document, entity: &str, field: &str, value: Value) {
    document
        .entities
        .get_mut(&EntityId::from(entity))
        .expect("entity exists")
        .fields
        .insert(FieldId::from(field), value);
}

fn assert_prepared_matches_full(
    session: &ResidentWorkspaceSession,
    candidate: &Document,
    label: impl std::fmt::Display,
) {
    let (expected_report, expected_calculation) = crate::semantic_validation(candidate);
    let prepared = session.prepare_evaluation(candidate.clone());
    assert_eq!(prepared.report(), &expected_report, "{label}: report");
    assert_eq!(
        prepared.calculation(),
        expected_calculation.as_ref(),
        "{label}: calculation"
    );
}

struct FixedTime;

impl TrustedPublicationTimeSource for FixedTime {
    fn now(&mut self) -> TrustedInstant {
        TrustedInstant::new(1)
    }
}

fn publish(session: &mut ResidentWorkspaceSession, candidate: &Document) {
    let scope = session.document_scope().clone();
    let revision = session.revision().clone();
    let mut time = FixedTime;
    session
        .publication_authority(&mut time)
        .publish_if_current(&scope, &revision, candidate, |_| Some(()))
        .expect("the candidate differs from the installed document");
}

/// Every kind of candidate must reach exactly the report and calculation a
/// full calculation gives, including the ones that fail to calculate.
#[allow(clippy::too_many_lines)] // One case table, each case a few lines.
#[test]
fn prepared_evaluation_matches_full_validation_for_every_candidate_kind() {
    let base = base_document();

    let cases: Vec<(&str, Document, Document)> = {
        let mut cases = Vec::new();

        let mut value_edit = base.clone();
        set(&mut value_edit, "e1", "g", number(9.0));
        cases.push(("value edit", base.clone(), value_edit));

        let mut formula_edit = base.clone();
        set(
            &mut formula_edit,
            "e1",
            "h",
            Value::Formula(multiply(reference("e1", "g"), literal(3.0))),
        );
        cases.push(("formula edit", base.clone(), formula_edit));

        let mut several_edits = base.clone();
        set(&mut several_edits, "e1", "g", number(4.0));
        set(&mut several_edits, "e2", "f", Value::Text("changed".into()));
        several_edits
            .entities
            .get_mut(&EntityId::from("e2"))
            .expect("entity exists")
            .fields
            .remove(&FieldId::from("h"));
        cases.push(("several edits", base.clone(), several_edits));

        let mut failed_base = base.clone();
        set(
            &mut failed_base,
            "e1",
            "h",
            Value::Formula(divide(literal(1.0), literal(0.0))),
        );
        let mut fixed = failed_base.clone();
        set(
            &mut fixed,
            "e1",
            "h",
            Value::Formula(add(reference("e1", "g"), literal(1.0))),
        );
        cases.push(("fixes a failing formula", failed_base.clone(), fixed));

        let mut failing = base.clone();
        set(
            &mut failing,
            "e1",
            "h",
            Value::Formula(divide(literal(1.0), literal(0.0))),
        );
        cases.push(("introduces a division by zero", base.clone(), failing));

        let mut zero_divisor = base.clone();
        set(&mut zero_divisor, "e2", "g", number(0.0));
        set(
            &mut zero_divisor,
            "e1",
            "h",
            Value::Formula(divide(reference("e1", "g"), reference("e2", "g"))),
        );
        cases.push(("division by a zero cell", base.clone(), zero_divisor));

        let mut cyclic = base.clone();
        set(&mut cyclic, "e1", "h", Value::Formula(reference("e1", "h")));
        cases.push(("formula cycle", base.clone(), cyclic));

        let mut missing_reference = base.clone();
        set(
            &mut missing_reference,
            "e1",
            "h",
            Value::Formula(reference("absent", "g")),
        );
        cases.push(("reference error", base.clone(), missing_reference));

        let mut added_entity = base.clone();
        added_entity.entities.insert(
            EntityId::from("e3"),
            row(
                "e3",
                "s",
                BTreeMap::from([
                    ("f".into(), Value::Text("three".into())),
                    ("g".into(), number(3.0)),
                    (
                        "h".into(),
                        Value::Formula(add(reference("e3", "g"), literal(1.0))),
                    ),
                ]),
            ),
        );
        cases.push(("entity added", base.clone(), added_entity));

        let mut with_third = base.clone();
        with_third.entities.insert(
            EntityId::from("e3"),
            row("e3", "s", BTreeMap::from([("g".into(), number(3.0))])),
        );
        let mut removed_entity = with_third.clone();
        removed_entity.entities.remove(&EntityId::from("e3"));
        cases.push(("entity removed", with_third, removed_entity));

        let mut empty_entity_added = base.clone();
        empty_entity_added
            .entities
            .insert(EntityId::from("empty"), row("empty", "s", BTreeMap::new()));
        cases.push(("zero-field entity added", base.clone(), empty_entity_added));

        let mut with_empty = base.clone();
        with_empty
            .entities
            .insert(EntityId::from("empty"), row("empty", "s", BTreeMap::new()));
        let mut empty_entity_removed = with_empty.clone();
        empty_entity_removed
            .entities
            .remove(&EntityId::from("empty"));
        cases.push((
            "zero-field entity removed",
            with_empty,
            empty_entity_removed,
        ));

        let mut field_added = base.clone();
        field_added
            .schemas
            .get_mut(&SchemaId::from("s"))
            .expect("schema exists")
            .fields
            .insert("n".into(), optional_number("n"));
        set(&mut field_added, "e1", "n", number(7.0));
        cases.push(("schema field added", base.clone(), field_added));

        let mut with_field = base.clone();
        with_field
            .schemas
            .get_mut(&SchemaId::from("s"))
            .expect("schema exists")
            .fields
            .insert("n".into(), optional_number("n"));
        set(&mut with_field, "e1", "n", number(7.0));
        let mut field_removed = with_field.clone();
        field_removed
            .schemas
            .get_mut(&SchemaId::from("s"))
            .expect("schema exists")
            .fields
            .remove(&FieldId::from("n"));
        field_removed
            .entities
            .get_mut(&EntityId::from("e1"))
            .expect("entity exists")
            .fields
            .remove(&FieldId::from("n"));
        cases.push(("schema field removed", with_field, field_removed));

        let mut retyped = base.clone();
        retyped
            .schemas
            .get_mut(&SchemaId::from("s"))
            .expect("schema exists")
            .fields
            .get_mut(&FieldId::from("g"))
            .expect("field exists")
            .field_type = FieldType::Text;
        cases.push(("schema field retyped", base.clone(), retyped));

        let mut constrained = base.clone();
        constrained
            .schemas
            .get_mut(&SchemaId::from("s"))
            .expect("schema exists")
            .fields
            .get_mut(&FieldId::from("g"))
            .expect("field exists")
            .constraint = FieldConstraint::NumberInclusiveRange {
            min: Number::new(0.0).expect("finite"),
            max: Number::new(5.0).expect("finite"),
        };
        set(&mut constrained, "e2", "g", number(9.0));
        cases.push(("constraint set", base.clone(), constrained));

        let mut invalid_base = base.clone();
        set(
            &mut invalid_base,
            "e1",
            "g",
            Value::Text("not a number".into()),
        );
        let mut invalid_base_edit = invalid_base.clone();
        set(
            &mut invalid_base_edit,
            "e2",
            "f",
            Value::Text("edited".into()),
        );
        cases.push(("base itself invalid", invalid_base, invalid_base_edit));

        cases.push(("base itself failed", failed_base.clone(), {
            let mut edited = failed_base;
            set(&mut edited, "e1", "g", number(6.0));
            edited
        }));

        cases
    };

    for (label, base, candidate) in cases {
        let session = ResidentWorkspaceSession::new("scope".into(), base);
        assert_prepared_matches_full(&session, &candidate, label);
    }
}

type Edit = (&'static str, fn(&mut Document));

/// A run of published edits moves the retained base each time, so every later
/// preparation derives from state publication left behind.
#[test]
fn prepared_evaluation_matches_full_validation_across_published_edits() {
    let mut session = ResidentWorkspaceSession::new("scope".into(), base_document());

    let edits: Vec<Edit> = vec![
        ("value edit", |document| {
            set(document, "e1", "g", number(5.0));
        }),
        ("failing formula", |document| {
            set(
                document,
                "e1",
                "h",
                Value::Formula(divide(reference("e1", "g"), literal(0.0))),
            );
        }),
        ("fixed formula", |document| {
            set(
                document,
                "e1",
                "h",
                Value::Formula(add(reference("e1", "g"), literal(2.0))),
            );
        }),
        ("entity added", |document| {
            document.entities.insert(
                EntityId::from("e3"),
                row(
                    "e3",
                    "s",
                    BTreeMap::from([
                        ("g".into(), number(3.0)),
                        (
                            "h".into(),
                            Value::Formula(add(reference("e3", "g"), literal(1.0))),
                        ),
                    ]),
                ),
            );
        }),
        ("entity removed", |document| {
            document.entities.remove(&EntityId::from("e3"));
        }),
        ("schema field added", |document| {
            document
                .schemas
                .get_mut(&SchemaId::from("s"))
                .expect("schema exists")
                .fields
                .insert("n".into(), optional_number("n"));
            set(document, "e1", "n", number(7.0));
        }),
        ("constraint set", |document| {
            document
                .schemas
                .get_mut(&SchemaId::from("s"))
                .expect("schema exists")
                .fields
                .get_mut(&FieldId::from("g"))
                .expect("field exists")
                .constraint = FieldConstraint::NumberInclusiveRange {
                min: Number::new(0.0).expect("finite"),
                max: Number::new(4.0).expect("finite"),
            };
        }),
    ];

    for (label, edit) in edits {
        let mut candidate = session.document().clone();
        edit(&mut candidate);
        assert_prepared_matches_full(&session, &candidate, label);
        publish(&mut session, &candidate);
    }
}

fn column_total(schema: &str, field: &str) -> Expression {
    Expression::ColumnTotal(crate::ColumnRef::new(
        SchemaId::from(schema),
        FieldId::from(field),
    ))
}

const ROWS: usize = 200;

fn generated_document() -> Document {
    let mut document = Document::empty("doc", "Sheet");
    document.schemas.insert(
        SchemaId::from("s"),
        Schema {
            field_order: None,
            id: "s".into(),
            key: "table".into(),
            display_name: None,
            fields: BTreeMap::from([
                ("c0".into(), optional_number("c0")),
                ("c1".into(), optional_number("c1")),
                ("c2".into(), optional_number("c2")),
                ("c3".into(), optional_number("c3")),
                ("note".into(), optional_text("note")),
            ]),
        },
    );
    for index in 0..ROWS {
        let id = format!("row-{index:03}");
        let value = f64::from(u32::try_from(index).expect("row count fits u32"));
        document.entities.insert(
            EntityId::from(id.as_str()),
            row(
                &id,
                "s",
                BTreeMap::from([
                    ("c0".into(), number(value / 3.0)),
                    (
                        "c1".into(),
                        Value::Formula(add(same_row("c0"), literal(1.0))),
                    ),
                    ("c2".into(), Value::Formula(column_total("s", "c0"))),
                    (
                        "c3".into(),
                        Value::Formula(divide(same_row("c0"), same_row("c1"))),
                    ),
                ]),
            ),
        );
    }
    document
}

/// The repo's differential generators use `SplitMix64`; this is the same small
/// deterministic generator without another dependency.
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn below(&mut self, bound: usize) -> usize {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        let z = z ^ (z >> 31);
        let bound = u64::try_from(bound).expect("bounds fit u64");
        usize::try_from(z % bound).expect("remainder fits usize")
    }
}

fn random_number(random: &mut SplitMix64) -> f64 {
    let integer = u32::try_from(random.below(200)).expect("small bound");
    f64::from(integer) / 2.0 - 50.0
}

fn random_edit(document: &mut Document, random: &mut SplitMix64) {
    let index = random.below(ROWS);
    let id = EntityId::from(format!("row-{index:03}").as_str());
    let value = random_number(random);
    let entity = document.entities.get_mut(&id).expect("row exists");
    match random.below(8) {
        0 => {
            entity.fields.insert(FieldId::from("c0"), number(value));
        }
        1 => {
            entity
                .fields
                .insert(FieldId::from("c0"), Value::Text("n/a".into()));
        }
        2 => {
            entity.fields.insert(
                FieldId::from("c1"),
                Value::Formula(add(same_row("c0"), literal(value))),
            );
        }
        3 => {
            entity.fields.insert(FieldId::from("c1"), number(value));
        }
        4 => {
            entity.fields.insert(FieldId::from("c2"), number(value));
        }
        5 => {
            entity
                .fields
                .insert(FieldId::from("c2"), Value::Formula(column_total("s", "c0")));
        }
        6 => {
            entity.fields.insert(
                FieldId::from("c3"),
                Value::Formula(divide(same_row("c0"), same_row("c1"))),
            );
        }
        _ => {
            entity.fields.remove(&FieldId::from("c0"));
        }
    }
}

/// Mid-sized document, random edits, published each step: the prepared result
/// must equal full validation at every one of them.
#[test]
fn prepared_evaluation_matches_full_validation_across_random_published_edits() {
    let mut session = ResidentWorkspaceSession::new("scope".into(), generated_document());
    let mut random = SplitMix64 {
        state: 0x6780_0000_dead_beef,
    };
    for step in 0..50 {
        let mut candidate = session.document().clone();
        random_edit(&mut candidate, &mut random);
        if &candidate == session.document() {
            // The edit landed on the value already stored; force a difference
            // so the step still publishes.
            set(
                &mut candidate,
                "row-000",
                "c0",
                number(f64::from(step) + 0.5),
            );
        }
        assert_prepared_matches_full(&session, &candidate, step);
        publish(&mut session, &candidate);
    }
}

/// Two read schemas plus one reader of each column, so a row moving between
/// them changes what a formula totals.
fn column_document() -> Document {
    let mut document = Document::empty("doc", "Sheet");
    for key in ["a", "b"] {
        document.schemas.insert(
            SchemaId::from(key),
            Schema {
                field_order: None,
                id: key.into(),
                key: key.into(),
                display_name: None,
                fields: BTreeMap::from([("c0".into(), optional_number("c0"))]),
            },
        );
    }
    document.schemas.insert(
        SchemaId::from("readers"),
        Schema {
            field_order: None,
            id: "readers".into(),
            key: "readers".into(),
            display_name: None,
            fields: BTreeMap::from([("t".into(), optional_number("t"))]),
        },
    );
    document.entities.insert(
        EntityId::from("ra"),
        row("ra", "a", BTreeMap::from([("c0".into(), number(1.0))])),
    );
    document.entities.insert(
        EntityId::from("rb"),
        row("rb", "b", BTreeMap::from([("c0".into(), number(2.0))])),
    );
    for (id, schema) in [("read-a", "a"), ("read-b", "b")] {
        document.entities.insert(
            EntityId::from(id),
            row(
                id,
                "readers",
                BTreeMap::from([("t".into(), Value::Formula(column_total(schema, "c0")))]),
            ),
        );
    }
    document
}

/// `calculate_complete` is the full oracle; compare the retained outcome to it
/// directly so a failed outcome's failure map is compared too.
fn assert_retained_matches_full(session: &ResidentWorkspaceSession, label: &str) {
    let document = session.document().clone();
    let (expected_report, _) = crate::semantic_validation(&document);
    assert_eq!(
        &session.retained.validation_report, &expected_report,
        "{label}: report"
    );
    assert_eq!(
        session.retained.calculation_state.outcome(),
        crate::calculate_complete(&document),
        "{label}: calculation"
    );
}

/// A zero-field entity owns a blank slot in every read column of its schema
/// and an entry in the retained graph's schema map, so publication must rebuild
/// rather than update incrementally. A later value edit to the column the
/// entity now belongs to is what a stale graph would mis-calculate.
#[test]
fn publication_rebuilds_retained_state_for_a_zero_field_entity() {
    let mut session = ResidentWorkspaceSession::new("scope".into(), column_document());
    assert_retained_matches_full(&session, "initial");

    let mut candidate = session.document().clone();
    candidate
        .entities
        .insert(EntityId::from("empty"), row("empty", "a", BTreeMap::new()));
    publish(&mut session, &candidate);
    assert_retained_matches_full(&session, "zero-field entity added");

    let mut candidate = session.document().clone();
    set(&mut candidate, "ra", "c0", number(5.0));
    publish(&mut session, &candidate);
    assert_retained_matches_full(&session, "value edit after adding");

    let mut candidate = session.document().clone();
    candidate.entities.remove(&EntityId::from("empty"));
    publish(&mut session, &candidate);
    assert_retained_matches_full(&session, "zero-field entity removed");

    let mut candidate = session.document().clone();
    set(&mut candidate, "ra", "c0", number(6.0));
    publish(&mut session, &candidate);
    assert_retained_matches_full(&session, "value edit after removing");

    let mut candidate = session.document().clone();
    candidate
        .entities
        .insert(EntityId::from("empty"), row("empty", "a", BTreeMap::new()));
    publish(&mut session, &candidate);
    assert_retained_matches_full(&session, "zero-field entity re-added");

    let mut candidate = session.document().clone();
    candidate
        .entities
        .get_mut(&EntityId::from("empty"))
        .expect("entity exists")
        .schema = SchemaId::from("b");
    publish(&mut session, &candidate);
    assert_retained_matches_full(&session, "zero-field entity moved");

    let mut candidate = session.document().clone();
    set(&mut candidate, "rb", "c0", number(7.0));
    publish(&mut session, &candidate);
    assert_retained_matches_full(&session, "value edit after moving");

    let mut candidate = session.document().clone();
    candidate
        .entities
        .get_mut(&EntityId::from("empty"))
        .expect("entity exists")
        .schema = SchemaId::from("a");
    publish(&mut session, &candidate);
    assert_retained_matches_full(&session, "zero-field entity moved back");

    let mut candidate = session.document().clone();
    set(&mut candidate, "ra", "c0", number(8.0));
    publish(&mut session, &candidate);
    assert_retained_matches_full(&session, "value edit after moving back");
}

fn schema(id: &str, fields: BTreeMap<FieldId, FieldDefinition>) -> Schema {
    Schema {
        field_order: None,
        id: id.into(),
        key: id.into(),
        display_name: None,
        fields,
    }
}

/// A `readers` row totalling the `n` column of another schema, with no entity
/// in that schema: the reviewer's case for #747.
fn empty_table_document(target_fields: BTreeMap<FieldId, FieldDefinition>) -> Document {
    let mut document = Document::empty("doc", "Sheet");
    document
        .schemas
        .insert(SchemaId::from("target"), schema("target", target_fields));
    document.schemas.insert(
        SchemaId::from("readers"),
        schema(
            "readers",
            BTreeMap::from([("t".into(), optional_number("t"))]),
        ),
    );
    document.entities.insert(
        EntityId::from("r1"),
        row(
            "r1",
            "readers",
            BTreeMap::from([("t".into(), Value::Formula(column_total("target", "n")))]),
        ),
    );
    document
}

fn readers_only_document() -> Document {
    let mut document = Document::empty("doc", "Sheet");
    document.schemas.insert(
        SchemaId::from("readers"),
        schema(
            "readers",
            BTreeMap::from([("t".into(), optional_number("t"))]),
        ),
    );
    document.entities.insert(
        EntityId::from("r1"),
        row(
            "r1",
            "readers",
            BTreeMap::from([("t".into(), Value::Formula(column_total("target", "n")))]),
        ),
    );
    document
}

fn assert_before_and_after_publication(
    session: &mut ResidentWorkspaceSession,
    candidate: &Document,
    label: &str,
) {
    assert_prepared_matches_full(session, candidate, label);
    publish(session, candidate);
    assert_retained_matches_full(session, label);
}

/// The reviewer's case: no entity holds `target.n`, so removing the schema had
/// no field root to invalidate and the retained state kept the old calculation.
#[test]
fn removing_a_schema_no_entity_holds_rebuilds() {
    let document = empty_table_document(BTreeMap::from([("n".into(), optional_number("n"))]));
    let mut session = ResidentWorkspaceSession::new("scope".into(), document);
    let mut candidate = session.document().clone();
    candidate.schemas.remove(&SchemaId::from("target"));
    assert_before_and_after_publication(&mut session, &candidate, "schema removed");
}

#[test]
fn retyping_a_read_field_no_entity_holds_rebuilds() {
    let document = empty_table_document(BTreeMap::from([("n".into(), optional_number("n"))]));
    let mut session = ResidentWorkspaceSession::new("scope".into(), document);
    let mut candidate = session.document().clone();
    candidate
        .schemas
        .get_mut(&SchemaId::from("target"))
        .expect("schema exists")
        .fields
        .insert("n".into(), optional_text("n"));
    assert_before_and_after_publication(&mut session, &candidate, "field retyped");
}

#[test]
fn adding_a_read_field_to_an_empty_schema_rebuilds() {
    let mut session =
        ResidentWorkspaceSession::new("scope".into(), empty_table_document(BTreeMap::new()));
    let mut candidate = session.document().clone();
    candidate
        .schemas
        .get_mut(&SchemaId::from("target"))
        .expect("schema exists")
        .fields
        .insert("n".into(), optional_number("n"));
    assert_before_and_after_publication(&mut session, &candidate, "field added");
}

/// A schema no entity holds is added, first with no fields and then with the
/// read field; before it existed the formula's column had no schema at all.
#[test]
fn adding_an_empty_schema_rebuilds() {
    let mut session = ResidentWorkspaceSession::new("scope".into(), readers_only_document());

    let mut candidate = session.document().clone();
    candidate
        .schemas
        .insert(SchemaId::from("target"), schema("target", BTreeMap::new()));
    assert_before_and_after_publication(&mut session, &candidate, "schema added");

    let mut candidate = session.document().clone();
    candidate
        .schemas
        .get_mut(&SchemaId::from("target"))
        .expect("schema exists")
        .fields
        .insert("n".into(), optional_number("n"));
    assert_before_and_after_publication(&mut session, &candidate, "read field added after");
}

/// `e` holds a value for `f` but leaves `n` blank, and `r`'s formula reads
/// `e.n`. `n` is Number in `s` and Text in `t`, so moving `e` between them
/// changes what the read names.
fn entity_membership_document() -> Document {
    let mut document = Document::empty("doc", "Sheet");
    for (id, n_definition) in [("s", optional_number("n")), ("t", optional_text("n"))] {
        document.schemas.insert(
            SchemaId::from(id),
            schema(
                id,
                BTreeMap::from([
                    ("f".into(), optional_number("f")),
                    ("g".into(), optional_number("g")),
                    ("n".into(), n_definition),
                ]),
            ),
        );
    }
    document.entities.insert(
        EntityId::from("e"),
        row("e", "s", BTreeMap::from([("f".into(), number(1.0))])),
    );
    document.entities.insert(
        EntityId::from("r"),
        row(
            "r",
            "s",
            BTreeMap::from([(
                "g".into(),
                Value::Formula(add(reference("e", "n"), literal(1.0))),
            )]),
        ),
    );
    document
}

#[test]
fn moving_an_entity_to_another_schema_rebuilds() {
    let mut session = ResidentWorkspaceSession::new("scope".into(), entity_membership_document());
    let mut candidate = session.document().clone();
    candidate
        .entities
        .get_mut(&EntityId::from("e"))
        .expect("entity exists")
        .schema = SchemaId::from("t");
    assert_before_and_after_publication(&mut session, &candidate, "entity moved");
}

#[test]
fn deleting_an_entity_with_a_read_blank_field_rebuilds() {
    let mut session = ResidentWorkspaceSession::new("scope".into(), entity_membership_document());
    let mut candidate = session.document().clone();
    candidate.entities.remove(&EntityId::from("e"));
    assert_before_and_after_publication(&mut session, &candidate, "entity removed");
}

#[test]
fn adding_an_entity_with_a_read_blank_field_rebuilds() {
    let mut initial = entity_membership_document();
    initial.entities.remove(&EntityId::from("e"));
    let mut session = ResidentWorkspaceSession::new("scope".into(), initial);
    let mut candidate = session.document().clone();
    candidate.entities.insert(
        EntityId::from("e"),
        row("e", "s", BTreeMap::from([("f".into(), number(1.0))])),
    );
    assert_before_and_after_publication(&mut session, &candidate, "entity added");
}

/// The third review's case: `e1` holds `n` and `e2` leaves it blank, and `r`
/// reads `e2.n`. Retyping `n` roots only `e1.n`, so the reader of the blank
/// cell kept its old result until the definition change rebuilt.
#[test]
fn retyping_a_field_some_entity_leaves_blank_rebuilds() {
    let mut document = Document::empty("doc", "Sheet");
    document.schemas.insert(
        SchemaId::from("s"),
        schema(
            "s",
            BTreeMap::from([
                ("n".into(), optional_number("n")),
                ("x".into(), optional_number("x")),
            ]),
        ),
    );
    document.entities.insert(
        EntityId::from("e1"),
        row("e1", "s", BTreeMap::from([("n".into(), number(2.0))])),
    );
    document
        .entities
        .insert(EntityId::from("e2"), row("e2", "s", BTreeMap::new()));
    document.entities.insert(
        EntityId::from("r"),
        row(
            "r",
            "s",
            BTreeMap::from([(
                "x".into(),
                Value::Formula(add(reference("e2", "n"), literal(1.0))),
            )]),
        ),
    );
    let mut session = ResidentWorkspaceSession::new("scope".into(), document);
    let mut candidate = session.document().clone();
    candidate
        .schemas
        .get_mut(&SchemaId::from("s"))
        .expect("schema exists")
        .fields
        .insert("n".into(), optional_text("n"));
    set(&mut candidate, "e1", "n", Value::Text("two".into()));
    assert_before_and_after_publication(&mut session, &candidate, "field retyped with a blank");
}

type PublishResult = (
    Result<(), SemanticPublicationError>,
    Option<ResidentProjectionInvalidation>,
);

fn publish_against(
    session: &mut ResidentWorkspaceSession,
    expected_revision: &SemanticRevision,
    candidate: &Document,
    installation: Option<PreparedInstallation>,
) -> PublishResult {
    let scope = session.document_scope().clone();
    let mut time = FixedTime;
    let mut publication = session.publication_authority(&mut time);
    if let Some(installation) = installation {
        publication = publication.with_prepared(installation);
    }
    let result = publication
        .publish_if_current(&scope, expected_revision, candidate, |_| Some(()))
        .map(|_| ());
    (result, publication.projection_invalidation.clone())
}

/// Today's path: publication derives the candidate's state itself.
fn publish_unassisted(
    session: &mut ResidentWorkspaceSession,
    candidate: &Document,
) -> PublishResult {
    let revision = session.revision().clone();
    publish_against(session, &revision, candidate, None)
}

/// The prepared path: publication installs what `prepare_evaluation` derived.
fn publish_with_installation(
    session: &mut ResidentWorkspaceSession,
    candidate: &Document,
) -> PublishResult {
    let installation = session
        .prepare_evaluation(candidate.clone())
        .into_installable();
    let revision = session.revision().clone();
    publish_against(session, &revision, candidate, Some(installation))
}

fn assert_sessions_identical(
    prepared: &ResidentWorkspaceSession,
    unassisted: &ResidentWorkspaceSession,
    label: &str,
) {
    assert_eq!(
        prepared.export_snapshot(),
        unassisted.export_snapshot(),
        "{label}: document, scope and revision"
    );
    assert_eq!(
        prepared.retained, unassisted.retained,
        "{label}: calculation and diagnostics state"
    );
    assert_eq!(
        prepared.measurements, unassisted.measurements,
        "{label}: measurements"
    );
}

/// Publish `candidate` from `base` through both paths and require the same
/// session, invalidation and result. The installation must actually be
/// installed, so a regression that never takes the prepared branch fails.
fn assert_installation_publishes_identically(label: &str, base: &Document, candidate: &Document) {
    let mut unassisted = ResidentWorkspaceSession::new("scope".into(), base.clone());
    let mut prepared = ResidentWorkspaceSession::new("scope".into(), base.clone());

    let installs_before = installation_counters::prepared_installations();
    let (unassisted_result, unassisted_invalidation) =
        publish_unassisted(&mut unassisted, candidate);
    let (prepared_result, prepared_invalidation) =
        publish_with_installation(&mut prepared, candidate);
    let installs = installation_counters::prepared_installations() - installs_before;

    assert_eq!(installs, 1, "{label}: the installation must be installed");
    assert_eq!(
        unassisted_result,
        Ok(()),
        "{label}: the unassisted publication succeeds"
    );
    assert_eq!(
        prepared_result, unassisted_result,
        "{label}: publication result"
    );
    assert_eq!(
        prepared_invalidation, unassisted_invalidation,
        "{label}: ResidentProjectionInvalidation"
    );
    assert_sessions_identical(&prepared, &unassisted, label);
    assert_retained_matches_full(&prepared, label);
}

/// Acceptance 1: a value edit, a formula edit, a failing formula and a
/// structural edit publish identically with and without an installation.
#[test]
fn prepared_installation_publishes_identically_to_the_unassisted_path() {
    let base = base_document();

    let mut value_edit = base.clone();
    set(&mut value_edit, "e1", "g", number(9.0));
    assert_installation_publishes_identically("value edit", &base, &value_edit);

    let mut formula_edit = base.clone();
    set(
        &mut formula_edit,
        "e1",
        "h",
        Value::Formula(multiply(reference("e1", "g"), literal(3.0))),
    );
    assert_installation_publishes_identically("formula edit", &base, &formula_edit);

    let mut failing = base.clone();
    set(
        &mut failing,
        "e1",
        "h",
        Value::Formula(divide(literal(1.0), literal(0.0))),
    );
    assert_installation_publishes_identically("failing formula", &base, &failing);

    let mut structural = base.clone();
    structural.entities.insert(
        EntityId::from("e3"),
        row(
            "e3",
            "s",
            BTreeMap::from([
                ("f".into(), Value::Text("three".into())),
                ("g".into(), number(3.0)),
                (
                    "h".into(),
                    Value::Formula(add(reference("e3", "g"), literal(1.0))),
                ),
            ]),
        ),
    );
    assert_installation_publishes_identically("structural edit", &base, &structural);
}

/// Acceptance 1: a sequence of 20 seeded edits stays identical step by step,
/// so every preparation derives from state publication left behind.
#[test]
fn prepared_installation_matches_the_unassisted_path_across_seeded_edits() {
    let mut unassisted = ResidentWorkspaceSession::new("scope".into(), generated_document());
    let mut prepared = ResidentWorkspaceSession::new("scope".into(), generated_document());
    let mut random = SplitMix64 {
        state: 0x6780_0000_5eed_0001,
    };

    for step in 0..20 {
        let mut candidate = prepared.document().clone();
        random_edit(&mut candidate, &mut random);
        if &candidate == prepared.document() {
            set(
                &mut candidate,
                "row-000",
                "c0",
                number(f64::from(step) + 0.5),
            );
        }
        let installs_before = installation_counters::prepared_installations();
        let (unassisted_result, unassisted_invalidation) =
            publish_unassisted(&mut unassisted, &candidate);
        let (prepared_result, prepared_invalidation) =
            publish_with_installation(&mut prepared, &candidate);
        assert_eq!(
            installation_counters::prepared_installations() - installs_before,
            1,
            "step {step}: the installation must be installed"
        );
        assert_eq!(prepared_result, unassisted_result, "step {step}: result");
        assert_eq!(
            prepared_invalidation, unassisted_invalidation,
            "step {step}: invalidation"
        );
        assert_sessions_identical(&prepared, &unassisted, &format!("step {step}"));
        assert_retained_matches_full(&prepared, &format!("step {step}"));
    }
}

/// Acceptance 1: a refused stale publication drops the installation and
/// leaves both sessions and invalidations identical to today's path.
#[test]
fn a_refused_stale_publication_drops_the_installation() {
    let base = base_document();
    let mut unassisted = ResidentWorkspaceSession::new("scope".into(), base.clone());
    let mut prepared = ResidentWorkspaceSession::new("scope".into(), base.clone());
    let stale_revision = unassisted.revision().clone();

    let mut advance = base.clone();
    set(&mut advance, "e1", "g", number(4.0));
    let (unassisted_advance, _) = publish_unassisted(&mut unassisted, &advance);
    let (prepared_advance, _) = publish_with_installation(&mut prepared, &advance);
    assert_eq!(unassisted_advance, Ok(()));
    assert_eq!(prepared_advance, unassisted_advance);
    assert_sessions_identical(&prepared, &unassisted, "advance");

    let mut candidate = prepared.document().clone();
    set(&mut candidate, "e1", "f", Value::Text("stale".into()));
    let installation = prepared
        .prepare_evaluation(candidate.clone())
        .into_installable();
    let installs_before = installation_counters::prepared_installations();
    let (unassisted_result, unassisted_invalidation) =
        publish_against(&mut unassisted, &stale_revision, &candidate, None);
    let (prepared_result, prepared_invalidation) = publish_against(
        &mut prepared,
        &stale_revision,
        &candidate,
        Some(installation),
    );

    assert_eq!(unassisted_result, Err(SemanticPublicationError::Stale));
    assert_eq!(prepared_result, unassisted_result);
    assert_eq!(prepared_invalidation, unassisted_invalidation);
    assert_eq!(prepared_invalidation, None);
    assert_eq!(
        installation_counters::prepared_installations(),
        installs_before,
        "a refused publication must not install the preparation"
    );
    assert_sessions_identical(&prepared, &unassisted, "refused stale edit");
}

/// Acceptance 2: an installation prepared for another candidate falls back
/// to today's path and still publishes the right result.
#[test]
fn an_installation_for_another_candidate_falls_back() {
    let base = base_document();
    let mut unassisted = ResidentWorkspaceSession::new("scope".into(), base.clone());
    let mut prepared = ResidentWorkspaceSession::new("scope".into(), base.clone());

    let mut installed = base.clone();
    set(&mut installed, "e1", "g", number(5.0));
    let installation = prepared.prepare_evaluation(installed).into_installable();

    let mut published = base.clone();
    set(&mut published, "e1", "f", Value::Text("different".into()));
    let installs_before = installation_counters::prepared_installations();
    let (unassisted_result, unassisted_invalidation) =
        publish_unassisted(&mut unassisted, &published);
    let revision = prepared.revision().clone();
    let (prepared_result, prepared_invalidation) =
        publish_against(&mut prepared, &revision, &published, Some(installation));

    assert_eq!(unassisted_result, Ok(()));
    assert_eq!(prepared_result, unassisted_result);
    assert_eq!(prepared_invalidation, unassisted_invalidation);
    assert_eq!(
        installation_counters::prepared_installations(),
        installs_before,
        "a mismatched installation must not be installed"
    );
    assert_sessions_identical(&prepared, &unassisted, "mismatched candidate");
    assert_retained_matches_full(&prepared, "mismatched candidate fallback");
}

/// Acceptance 2: an installation prepared against an older revision falls
/// back to today's path and still publishes the right result.
#[test]
fn an_installation_for_an_older_revision_falls_back() {
    let base = base_document();
    let mut unassisted = ResidentWorkspaceSession::new("scope".into(), base.clone());
    let mut prepared = ResidentWorkspaceSession::new("scope".into(), base.clone());

    // Prepared at the initial revision, then publication moves on before the
    // candidate is published.
    let mut target = base.clone();
    set(&mut target, "e1", "g", number(8.0));
    let installation = prepared
        .prepare_evaluation(target.clone())
        .into_installable();

    let mut advance = base.clone();
    set(&mut advance, "e1", "f", Value::Text("advanced".into()));
    let (unassisted_advance, _) = publish_unassisted(&mut unassisted, &advance);
    let (prepared_advance, _) = publish_unassisted(&mut prepared, &advance);
    assert_eq!(unassisted_advance, Ok(()));
    assert_eq!(prepared_advance, unassisted_advance);
    assert_sessions_identical(&prepared, &unassisted, "advance");

    let installs_before = installation_counters::prepared_installations();
    let (unassisted_result, unassisted_invalidation) = publish_unassisted(&mut unassisted, &target);
    let revision = prepared.revision().clone();
    let (prepared_result, prepared_invalidation) =
        publish_against(&mut prepared, &revision, &target, Some(installation));

    assert_eq!(unassisted_result, Ok(()));
    assert_eq!(prepared_result, unassisted_result);
    assert_eq!(prepared_invalidation, unassisted_invalidation);
    assert_eq!(
        installation_counters::prepared_installations(),
        installs_before,
        "a stale installation must not be installed"
    );
    assert_sessions_identical(&prepared, &unassisted, "older revision");
    assert_retained_matches_full(&prepared, "older-revision fallback");
}
