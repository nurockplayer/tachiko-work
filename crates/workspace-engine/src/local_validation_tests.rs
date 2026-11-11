//! A value edit on a clean document validates only the fields it touched, and
//! the report it produces is exactly whole-document validation's (#678).
//!
//! Every edit is published through a resident session, so each one derives
//! from the state the previous publication left behind. The test counter shows
//! the local path runs for plain value edits and falls back for every edit
//! whose checks are not local.

use std::collections::BTreeMap;

use tachiko_formula_engine::RetainedCalculationState;

use super::{
    ResidentWorkspaceSession, TrustedPublicationTimeSource, index_field_diagnostics,
    local_validation_counters,
};
use crate::patch_lifecycle::{SemanticPublicationAuthority, TrustedInstant};
use crate::{
    ColumnRef, Document, Entity, EntityId, EntityKey, Expression, FieldConstraint, FieldDefinition,
    FieldId, FieldType, Number, PreparedInstallation, Schema, SchemaId, ValidationReport, Value,
    validation_report_for_retained_calculation,
};

const ROWS: usize = 20;

fn number_definition(id: &str, constraint: FieldConstraint) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        display_name: None,
        number_format: None,
        date_format: None,
        field_type: FieldType::Number,
        required: false,
        constraint,
    }
}

fn text_definition(id: &str) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        display_name: None,
        number_format: None,
        date_format: None,
        field_type: FieldType::Text,
        required: false,
        constraint: FieldConstraint::None,
    }
}

fn reference_definition(id: &str, target: &str) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        display_name: None,
        number_format: None,
        date_format: None,
        field_type: FieldType::Reference {
            schema: SchemaId::from(target),
        },
        required: false,
        constraint: FieldConstraint::None,
    }
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).expect("test literals are finite"))
}

fn in_range(min: f64, max: f64) -> FieldConstraint {
    FieldConstraint::NumberInclusiveRange {
        min: Number::new(min).expect("test literals are finite"),
        max: Number::new(max).expect("test literals are finite"),
    }
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

/// `total` divides the row's amount by its ratio and is constrained to
/// `[0, 1000]`; `share` totals the whole amount column, and `target` points at
/// an entity in another schema. Every stored value satisfies its schema.
fn base_document() -> Document {
    let mut document = Document::empty("doc", "Sheet");
    document.schemas.insert(
        SchemaId::from("rows"),
        schema(
            "rows",
            BTreeMap::from([
                (
                    "amount".into(),
                    number_definition("amount", in_range(0.0, 1000.0)),
                ),
                ("label".into(), text_definition("label")),
                (
                    "ratio".into(),
                    number_definition("ratio", FieldConstraint::None),
                ),
                (
                    "share".into(),
                    number_definition("share", FieldConstraint::None),
                ),
                ("target".into(), reference_definition("target", "targets")),
                (
                    "total".into(),
                    number_definition("total", in_range(0.0, 1000.0)),
                ),
            ]),
        ),
    );
    document.schemas.insert(
        SchemaId::from("targets"),
        schema(
            "targets",
            BTreeMap::from([("name".into(), text_definition("name"))]),
        ),
    );
    document.entities.insert(
        EntityId::from("t1"),
        Entity {
            id: EntityId::from("t1"),
            key: EntityKey::from("t1"),
            schema: SchemaId::from("targets"),
            fields: BTreeMap::from([("name".into(), Value::Text("target".into()))]),
        },
    );
    for index in 0..ROWS {
        let id = format!("r{index:02}");
        let amount = f64::from(u32::try_from(index).expect("row count fits u32")) * 10.0;
        document.entities.insert(
            EntityId::from(id.as_str()),
            Entity {
                id: EntityId::from(id.as_str()),
                key: EntityKey::from(id.as_str()),
                schema: SchemaId::from("rows"),
                fields: BTreeMap::from([
                    ("amount".into(), number(amount)),
                    ("label".into(), Value::Text(format!("row {index}"))),
                    ("ratio".into(), number(2.0)),
                    (
                        "share".into(),
                        Value::Formula(Expression::ColumnTotal(ColumnRef::new(
                            SchemaId::from("rows"),
                            FieldId::from("amount"),
                        ))),
                    ),
                    ("target".into(), Value::Reference(EntityId::from("t1"))),
                    (
                        "total".into(),
                        Value::Formula(Expression::Divide {
                            left: Box::new(Expression::SameRowReference(FieldId::from("amount"))),
                            right: Box::new(Expression::SameRowReference(FieldId::from("ratio"))),
                        }),
                    ),
                ]),
            },
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

    fn amount(&mut self) -> f64 {
        f64::from(u32::try_from(self.below(1001)).expect("small bound"))
    }

    fn ratio(&mut self) -> f64 {
        f64::from(u32::try_from(self.below(5) + 1).expect("small bound"))
    }
}

fn set(document: &mut Document, row: &str, field: &str, value: Value) {
    document
        .entities
        .get_mut(&EntityId::from(row))
        .expect("row exists")
        .fields
        .insert(FieldId::from(field), value);
}

fn stored(document: &Document, row: &str, field: &str) -> Value {
    document.entities[&EntityId::from(row)].fields[&FieldId::from(field)].clone()
}

type Edit = fn(&mut Document, &str, &mut SplitMix64);
/// One edit case and whether the local report can decide it.
type Case = (&'static str, Edit, bool);

fn plain_value_edit(document: &mut Document, row: &str, random: &mut SplitMix64) {
    let value = random.amount();
    // A draw equal to the stored value would not publish; pick the neighbour.
    let value = if stored(document, row, "amount") == number(value) {
        (value + 1.0) % 1001.0
    } else {
        value
    };
    set(document, row, "amount", number(value));
}

fn plain_text_edit(document: &mut Document, row: &str, random: &mut SplitMix64) {
    let mut value = random.below(1_000_000);
    if stored(document, row, "label") == Value::Text(format!("edited {row} {value}")) {
        value += 1;
    }
    set(
        document,
        row,
        "label",
        Value::Text(format!("edited {row} {value}")),
    );
}

fn plain_ratio_edit(document: &mut Document, row: &str, random: &mut SplitMix64) {
    let mut value = random.ratio();
    if stored(document, row, "ratio") == number(value) {
        // A draw equal to the stored value would not publish; pick the next.
        value = value % 5.0 + 1.0;
    }
    set(document, row, "ratio", number(value));
}

fn text_into_number(document: &mut Document, row: &str, _: &mut SplitMix64) {
    set(document, row, "amount", Value::Text("not a number".into()));
}

fn repair_amount(document: &mut Document, row: &str, _: &mut SplitMix64) {
    set(document, row, "amount", number(500.0));
}

fn stored_constraint_violation(document: &mut Document, row: &str, _: &mut SplitMix64) {
    set(document, row, "amount", number(1500.0));
}

fn repair_stored_constraint(document: &mut Document, row: &str, _: &mut SplitMix64) {
    set(document, row, "amount", number(400.0));
}

fn missing_reference(document: &mut Document, row: &str, _: &mut SplitMix64) {
    set(
        document,
        row,
        "target",
        Value::Reference(EntityId::from("ghost")),
    );
}

fn repair_reference(document: &mut Document, row: &str, _: &mut SplitMix64) {
    set(
        document,
        row,
        "target",
        Value::Reference(EntityId::from("t1")),
    );
}

fn failing_formula(document: &mut Document, row: &str, _: &mut SplitMix64) {
    set(document, row, "ratio", number(0.0));
}

fn recovered_formula(document: &mut Document, row: &str, _: &mut SplitMix64) {
    set(document, row, "ratio", number(2.0));
}

fn calculated_constraint_violation(document: &mut Document, row: &str, _: &mut SplitMix64) {
    set(document, row, "amount", number(800.0));
    set(document, row, "ratio", number(0.5));
}

fn repair_calculated_constraint(document: &mut Document, row: &str, _: &mut SplitMix64) {
    set(document, row, "ratio", number(2.0));
}

/// Three plain value edits, then each non-local edit followed by its repair so
/// the base is clean again whenever a plain edit runs.
fn cases() -> [Case; 13] {
    [
        ("plain value edit", plain_value_edit, true),
        ("plain text edit", plain_text_edit, true),
        ("plain ratio edit", plain_ratio_edit, true),
        ("text into a Number field", text_into_number, false),
        ("repair the type mismatch", repair_amount, false),
        (
            "stored constraint violation",
            stored_constraint_violation,
            false,
        ),
        (
            "repair the stored constraint",
            repair_stored_constraint,
            false,
        ),
        ("reference to a missing entity", missing_reference, false),
        ("repair the reference", repair_reference, false),
        ("a formula starts failing", failing_formula, false),
        ("the formula recovers", recovered_formula, false),
        (
            "calculated constraint violation",
            calculated_constraint_violation,
            false,
        ),
        (
            "repair the calculated constraint",
            repair_calculated_constraint,
            false,
        ),
    ]
}

struct FixedTime;

impl TrustedPublicationTimeSource for FixedTime {
    fn now(&mut self) -> TrustedInstant {
        TrustedInstant::new(1)
    }
}

fn publish(
    session: &mut ResidentWorkspaceSession,
    candidate: &Document,
    installation: PreparedInstallation,
) {
    let scope = session.document_scope().clone();
    let revision = session.revision().clone();
    let mut time = FixedTime;
    session
        .publication_authority(&mut time)
        .with_prepared(installation)
        .publish_if_current(&scope, &revision, candidate, |_| Some(()))
        .expect("the candidate differs from the installed document");
}

fn full_report(document: &Document) -> ValidationReport {
    let (calculation, _) = RetainedCalculationState::rebuild(document);
    validation_report_for_retained_calculation(document, &calculation)
}

#[test]
fn retained_formula_sources_match_full_projection_after_publications() {
    let mut session = ResidentWorkspaceSession::new("scope".into(), base_document());
    let original_formula = stored(session.document(), "r00", "total");
    let mut random = SplitMix64 {
        state: 0x6780_0000_50a7_ce55,
    };
    let mut saw_ok = false;
    let mut saw_error = false;

    for step in 0..70 {
        let mut candidate = session.document().clone();
        match step % 10 {
            0 => plain_value_edit(&mut candidate, "r01", &mut random),
            1 => set(
                &mut candidate,
                "r00",
                "total",
                Value::Formula(Expression::SameRowReference(FieldId::from("amount"))),
            ),
            2 => set(&mut candidate, "r00", "total", original_formula.clone()),
            3 => {
                let mut row = candidate.entities[&EntityId::from("r01")].clone();
                row.id = EntityId::from("extra");
                row.key = EntityKey::from("extra");
                candidate.entities.insert(row.id.clone(), row);
            }
            4 => {
                candidate.entities.remove(&EntityId::from("extra"));
            }
            5 => set(&mut candidate, "r00", "ratio", number(0.0)),
            6 => set(&mut candidate, "r00", "ratio", number(2.0)),
            7 => set(
                &mut candidate,
                "r00",
                "amount",
                Value::Text("invalid".into()),
            ),
            8 => set(&mut candidate, "r00", "amount", number(500.0)),
            9 => set(
                &mut candidate,
                "r00",
                "total",
                Value::Formula(Expression::SameRowReference(FieldId::from("total"))),
            ),
            _ => unreachable!(),
        }
        // Repair the cycle on the next publication while retaining a value edit.
        if step % 10 == 0 && step > 0 {
            set(&mut candidate, "r00", "total", original_formula.clone());
        }
        let scope = session.document_scope().clone();
        let revision = session.revision().clone();
        let mut time = FixedTime;
        session
            .publication_authority(&mut time)
            .publish_if_current(&scope, &revision, &candidate, |_| Some(()))
            .expect("each edit publishes");
        let actual = session
            .formula_source_projections()
            .map_err(|error| error.to_string());
        let expected = crate::formula_source_projections(session.document())
            .map_err(|error| error.to_string());
        saw_ok |= expected.is_ok();
        saw_error |= expected.is_err();
        assert_eq!(actual, expected, "publication {step}");
        if matches!(step % 10, 5 | 7 | 9) {
            assert!(expected.is_err(), "publication {step} must be invalid");
        }
        if matches!(step % 10, 6 | 8) {
            assert!(expected.is_ok(), "publication {step} must recover");
        }
    }
    assert!(saw_ok && saw_error);
}

/// Acceptance: 104 seeded edits, all published through one resident session.
/// Every step's report equals a full report of the candidate and of the
/// document publication installed, and the counter shows the known path.
#[test]
fn a_clean_document_value_edit_validates_only_what_it_touched() {
    let mut session = ResidentWorkspaceSession::new("scope".into(), base_document());
    let mut random = SplitMix64 {
        state: 0x6780_0000_da7a_0007,
    };
    let cases = cases();

    // One seeded row per cycle, so every repair lands on the row its earlier
    // edit invalidated.
    let mut row = String::new();
    for step in 0..104 {
        let (label, edit, local_expected) = cases[step % cases.len()];
        if step % cases.len() == 0 {
            row = format!("r{:02}", random.below(ROWS));
        }
        let mut candidate = session.document().clone();
        edit(&mut candidate, &row, &mut random);
        assert_ne!(
            &candidate,
            session.document(),
            "step {step} ({label}): the edit must change the document"
        );
        let expected = full_report(&candidate);

        local_validation_counters::reset();
        let prepared = session.prepare_evaluation(candidate.clone());
        assert_eq!(
            prepared.report(),
            &expected,
            "step {step} ({label}): report"
        );
        assert_eq!(
            local_validation_counters::counts(),
            if local_expected { (1, 0) } else { (0, 1) },
            "step {step} ({label}): local versus full validation"
        );
        let installation = prepared.into_installable();
        publish(&mut session, &candidate, installation);

        let installed = session.document();
        let expected = full_report(installed);
        assert_eq!(
            &session.retained.validation_report, &expected,
            "step {step} ({label}): installed report"
        );
        assert_eq!(
            &session.retained.diagnostics_by_field,
            &index_field_diagnostics(&expected),
            "step {step} ({label}): installed diagnostics by field"
        );
    }
}
